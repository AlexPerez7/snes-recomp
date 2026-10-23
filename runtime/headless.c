/* headless.c - runtime for recompiled 65816 code with no video and no audio.
 *
 * Provides the bus (LoROM memory map, hardware registers that remember what is written), the
 * runtime hooks, a frame clock driven by rt_tick, NMI delivery and a final report. Build it with
 * the generated C, defining SNES_RT_TICK (and SNES_RT_CHECKS to find width mismatches):
 *
 *     headless <rom> [--frames N] [--frame-blocks N] [--dump DIR] [--screenshot FILE.png]
 *              [--trace-frames A-B] [--trace-reads] [--watch ADDR]...
 *
 * The ROM is read from the file at run time and is never part of the generated sources.
 * Time is not cycle accurate: a frame is a fixed number of executed blocks.
 * The IRQ is delivered at the line of $4209/$420A (vertical and combined modes; the horizontal position is
 * ignored), after the NMI of the frame. Execution contexts: an RTI whose S is not the one its handler started
 * with returns to another context (a fibre of its own), as the ALttP intro does between NMI and IRQ.
 * General purpose DMA runs for real; VRAM, CGRAM and OAM are kept and --dump writes them (with the
 * last value of $2100-$213F) to DIR as vram.bin, cgram.bin, oam.bin and ppu_regs.bin. --screenshot draws
 * the final state of the PPU (mode 1 backgrounds and sprites only) as a 256x224 PNG.
 *
 * Exit codes: 0 frames reached, 2 bad usage or ROM, 5 unresolved jump, 6 width mismatch,
 * 7 STP/BRK/COP, 9 unsupported function, 10 interrupt vector is not a function, 11 reset returned, 12 an execution context returned,
 * 13 too many contexts or nested interrupts, 14 a return to an address no call expects.
 */
#include "snes_funcs.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#ifdef _WIN32
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#else
#include <ucontext.h>
#endif

/* ---- Memory ---- */

static uint8_t *rom;
static size_t rom_size;
static uint8_t wram[0x20000];
static uint8_t sram[0x8000];
static uint32_t sram_mask = 0x7FFF;
static uint32_t wram_port; /* $2181-$2183 */

#define IO_FIRST 0x2100u
#define IO_LAST 0x43FFu
static uint8_t hw[0x10000];
static uint32_t hw_reads[IO_LAST + 1], hw_writes[IO_LAST + 1];
static uint32_t dma_starts[8], hdma_starts[8];
static uint8_t fixed_col[3]; /* $2132: red, green, blue, each set by writes whose bits 5-7 select it */
static uint64_t dma_bytes[8];

/* PPU memories and the state of their ports. */
static uint8_t vram[0x10000]; /* 32K words, little endian */
static uint32_t vram_addr;    /* word address */
static uint16_t vram_prefetch;
static int vram_translate;    /* $2115 asked for address translation, which is not modelled */
static uint8_t cgram[512];
static uint32_t cg_index;     /* colour */
static uint8_t cg_latch, cg_write_half, cg_read_half;
static uint8_t oam[544];
static uint32_t oam_byte;     /* internal byte address, 0..1023 */
static uint8_t oam_latch;
static const char *dump_dir;

/* Stand-in for the sound CPU: its ports say $AA,$BB until the upload handshake ($CC on port 0) and
 * then echo what the CPU writes. */
static uint8_t apu_port[4] = {0xAA, 0xBB, 0, 0};
static int apu_started;

/* ---- Clock ---- */

static uint32_t frame_len = 20000; /* blocks per frame */
static uint32_t max_frames = 60;
static uint32_t tick_in_frame;
static uint32_t frames, nmis;
static uint64_t blocks;
static int nmi_flag;    /* $4210 bit 7 */
static int nmi_pending; /* a frame ended with NMI enabled and it has not been delivered yet */
static int in_nmi;
static int irq_flag;    /* $4211 bit 7: the IRQ line is asserted until $4211 is read */
static int irq_fired;   /* the IRQ of this frame has been raised */
static uint32_t irqs;
static unsigned ctx_switches;

static void report(const char *cause);
static void write_scroll(uint32_t off, uint8_t v);

static uint8_t rom_byte(uint32_t a) {
    uint32_t bank = (a >> 16) & 0x7F;
    return rom[(((bank << 15) | (a & 0x7FFF))) % rom_size];
}

static uint16_t vram_word(void) {
    uint32_t i = (vram_addr * 2) & 0xFFFF;
    return (uint16_t)(vram[i] | vram[i + 1] << 8);
}

static uint32_t vram_step(void) {
    static const uint32_t steps[4] = {1, 32, 128, 128};
    return steps[hw[0x2115] & 3];
}

/* $2139/$213A: the port returns the prefetched value and, after the access that increments, moves
 * on and prefetches the next word. */
static uint8_t vram_read(int high) {
    uint8_t v = high ? (uint8_t)(vram_prefetch >> 8) : (uint8_t)vram_prefetch;
    if (high == (hw[0x2115] >> 7 & 1)) {
        vram_addr = (vram_addr + vram_step()) & 0x7FFF;
        vram_prefetch = vram_word();
    }
    return v;
}

static void vram_write(int high, uint8_t v) {
    vram[((vram_addr * 2) & 0xFFFF) + high] = v;
    if (high == (hw[0x2115] >> 7 & 1)) vram_addr = (vram_addr + vram_step()) & 0x7FFF;
}

static void cgram_write(uint8_t v) {
    if (!cg_write_half) {
        cg_latch = v;
        cg_write_half = 1;
    } else {
        cgram[cg_index * 2] = cg_latch;
        cgram[cg_index * 2 + 1] = v & 0x7F;
        cg_index = (cg_index + 1) & 0xFF;
        cg_write_half = 0;
    }
}

static uint8_t cgram_read(void) {
    uint8_t v = cgram[cg_index * 2 + cg_read_half];
    if (cg_read_half) {
        v &= 0x7F;
        cg_index = (cg_index + 1) & 0xFF;
    }
    cg_read_half ^= 1;
    return v;
}

static uint32_t oam_slot(void) {
    return oam_byte < 512 ? oam_byte : 512 + (oam_byte & 31);
}

static void oam_write(uint8_t v) {
    if (oam_byte < 512) {
        if (oam_byte & 1) {
            oam[oam_byte - 1] = oam_latch;
            oam[oam_byte] = v;
        } else {
            oam_latch = v;
        }
    } else {
        oam[oam_slot()] = v;
    }
    oam_byte = (oam_byte + 1) & 0x3FF;
}

static int dma_active;
static void bus_write8_raw(uint32_t a, uint8_t v);

static uint8_t bus_read8_raw(uint32_t a) {
    uint32_t bank = (a >> 16) & 0xFF, off = a & 0xFFFF, b = bank & 0x7F;
    if (bank == 0x7E || bank == 0x7F) return wram[((bank & 1) << 16) | off];
    if (b < 0x40) {
        if (off < 0x2000) return wram[off];
        if (off >= 0x8000) return rom_byte(a);
        if (off >= IO_FIRST && off <= IO_LAST) {
            uint8_t v;
            hw_reads[off]++;
            switch (off) {
            case 0x2138:
                v = oam[oam_slot()];
                oam_byte = (oam_byte + 1) & 0x3FF;
                return v;
            case 0x2139: return vram_read(0);
            case 0x213A: return vram_read(1);
            case 0x213B: return cgram_read();
            case 0x213C: case 0x213D: case 0x213E: case 0x213F:
            case 0x2137:
                return hw[off];
            case 0x2140: case 0x2141: case 0x2142: case 0x2143:
                return apu_port[off - 0x2140];
            case 0x2180:
                return wram[wram_port++ & 0x1FFFF];
            case 0x4210:
                v = (uint8_t)((nmi_flag ? 0x80 : 0) | 0x02);
                nmi_flag = 0;
                return v;
            case 0x4211:
                v = (uint8_t)(irq_flag ? 0x80 : 0);
                irq_flag = 0;
                return v;
            case 0x4212:
                /* Vertical blank during the last tenth of the frame; horizontal blank (bit 6) during the last fifth of
                 * each of the 262 lines of the frame; auto joypad read never busy. */
                v = (uint8_t)((uint64_t)tick_in_frame * 10 >= (uint64_t)frame_len * 9 ? 0x80 : 0);
                if ((uint64_t)tick_in_frame * 262 % frame_len * 5 >= (uint64_t)frame_len * 4) v |= 0x40;
                return v;
            default:
                return hw[off];
            }
        }
        return 0;
    }
    if (b < 0x70) return off >= 0x8000 ? rom_byte(a) : 0;
    if (b <= 0x7D) return off >= 0x8000 ? rom_byte(a) : sram[off & sram_mask];
    return 0;
}

/* ---- Trace: hardware reads and watched WRAM bytes, inside a window of frames ---- */

uint32_t rt_block; /* address of the block that started last (set by rt_check_flags with SNES_RT_TRACE) */

static int trace_window, trace_reads_on;
static uint32_t trace_lo, trace_hi;
static uint8_t watch[0x20000];
static int watch_any;

typedef struct { uint32_t block, reg, count; uint8_t val; } TraceRead;
#define TRACE_MAX 512
static TraceRead trace_tab[TRACE_MAX];
static int trace_n;
static uint32_t trace_frame;

static int trace_active(void) {
    return trace_window && frames >= trace_lo && frames <= trace_hi;
}

static void trace_flush(void) {
    int i;
    for (i = 0; i < trace_n; i++) {
        printf("trace read frame %u block %02X:%04X reg $%04X = %02X x%u\n", (unsigned)trace_frame,
               (unsigned)(trace_tab[i].block >> 16), (unsigned)(trace_tab[i].block & 0xFFFF),
               (unsigned)trace_tab[i].reg, trace_tab[i].val, (unsigned)trace_tab[i].count);
    }
    trace_n = 0;
}

static void trace_read(uint32_t reg, uint8_t v) {
    int i;
    if (trace_n && trace_frame != frames) trace_flush();
    trace_frame = frames;
    for (i = 0; i < trace_n; i++) {
        if (trace_tab[i].block == rt_block && trace_tab[i].reg == reg && trace_tab[i].val == v) {
            trace_tab[i].count++;
            return;
        }
    }
    if (trace_n == TRACE_MAX) trace_flush();
    trace_tab[trace_n].block = rt_block;
    trace_tab[trace_n].reg = reg;
    trace_tab[trace_n].val = v;
    trace_tab[trace_n].count = 1;
    trace_n++;
}

uint8_t bus_read8(uint32_t a) {
    uint8_t v = bus_read8_raw(a);
    uint32_t off = a & 0xFFFF;
    if (trace_reads_on && !dma_active && ((a >> 16) & 0x7F) < 0x40 && off >= IO_FIRST && off <= IO_LAST && trace_active()) {
        trace_read(off, v);
    }
    return v;
}

void bus_write8(uint32_t a, uint8_t v) {
    uint32_t bank = (a >> 16) & 0xFF, off = a & 0xFFFF, b = bank & 0x7F;
    int64_t idx = -1;
    uint8_t old = 0;
    if (watch_any) {
        if (bank == 0x7E || bank == 0x7F) idx = ((bank & 1) << 16) | off;
        else if (b < 0x40 && off < 0x2000) idx = off;
        else if (b < 0x40 && off == 0x2180) idx = wram_port & 0x1FFFF;
        if (idx >= 0) old = wram[idx];
    }
    bus_write8_raw(a, v);
    if (idx >= 0 && watch[idx] && wram[idx] != old && trace_active()) {
        printf("trace write frame %u block %02X:%04X wram %05X: %02X -> %02X\n", (unsigned)frames,
               (unsigned)(rt_block >> 16), (unsigned)(rt_block & 0xFFFF), (unsigned)idx, old, wram[idx]);
    }
}

/* General purpose DMA of one channel, run to the end as the CPU is halted meanwhile. Both sides go
 * through the bus, so the ports of the B bus behave as they do for the CPU. */
static void run_dma(int ch) {
    static const uint8_t pattern[8][4] = {{0, 0, 0, 0}, {0, 1, 0, 1}, {0, 0, 0, 0}, {0, 0, 1, 1},
                                          {0, 1, 2, 3}, {0, 1, 0, 1}, {0, 0, 0, 0}, {0, 0, 1, 1}};
    static const uint8_t unit[8] = {1, 2, 2, 4, 4, 2, 2, 4};
    uint32_t base = 0x4300 + (uint32_t)ch * 16, i, count, a;
    uint8_t ctl = hw[base], b = hw[base + 1], bank = hw[base + 4];
    int mode = ctl & 7, step = (ctl & 0x08) ? 0 : (ctl & 0x10) ? -1 : 1;
    a = hw[base + 2] | (uint32_t)hw[base + 3] << 8;
    count = hw[base + 5] | (uint32_t)hw[base + 6] << 8;
    if (!count) count = 0x10000;
    dma_active = 1;
    for (i = 0; i < count; i++) {
        uint32_t baddr = 0x2100 + ((b + pattern[mode][i % unit[mode]]) & 0xFF);
        uint32_t aaddr = (uint32_t)bank << 16 | a;
        if (ctl & 0x80) bus_write8(aaddr, bus_read8(baddr));
        else bus_write8(baddr, bus_read8(aaddr));
        a = (a + (uint32_t)step) & 0xFFFF;
    }
    dma_active = 0;
    hw[base + 2] = (uint8_t)a;
    hw[base + 3] = (uint8_t)(a >> 8);
    hw[base + 5] = 0;
    hw[base + 6] = 0;
    dma_starts[ch]++;
    dma_bytes[ch] += count;
}

static void bus_write8_raw(uint32_t a, uint8_t v) {
    uint32_t bank = (a >> 16) & 0xFF, off = a & 0xFFFF, b = bank & 0x7F;
    if (bank == 0x7E || bank == 0x7F) {
        wram[((bank & 1) << 16) | off] = v;
        return;
    }
    if (b < 0x40) {
        if (off < 0x2000) {
            wram[off] = v;
        } else if (off >= IO_FIRST && off <= IO_LAST) {
            int i;
            hw_writes[off]++;
            hw[off] = v;
            switch (off) {
            case 0x2102: oam_byte = (((oam_byte >> 1 & 0x100) | v) << 1) & 0x3FF; break;
            case 0x2103: oam_byte = (((oam_byte >> 1 & 0xFF) | (uint32_t)(v & 1) << 8) << 1) & 0x3FF; break;
            case 0x2104: oam_write(v); break;
            case 0x210D: case 0x210E: case 0x210F: case 0x2110: case 0x2111: case 0x2112: case 0x2113: case 0x2114:
                write_scroll(off, v);
                break;
            case 0x2115:
                if (v & 0x0C) vram_translate = v;
                break;
            case 0x2116: vram_addr = (vram_addr & 0x7F00) | v; vram_prefetch = vram_word(); break;
            case 0x2117: vram_addr = (vram_addr & 0x00FF) | (uint32_t)(v & 0x7F) << 8; vram_prefetch = vram_word(); break;
            case 0x2118: vram_write(0, v); break;
            case 0x2119: vram_write(1, v); break;
            case 0x2121: cg_index = v; cg_write_half = 0; cg_read_half = 0; break;
            case 0x2122: cgram_write(v); break;
            case 0x2132:
                for (i = 0; i < 3; i++) if (v >> (5 + i) & 1) fixed_col[i] = v & 31;
                break;
            case 0x2140: case 0x2141: case 0x2142: case 0x2143:
                if (off == 0x2140 && v == 0xCC) apu_started = 1;
                if (apu_started) apu_port[off - 0x2140] = v;
                break;
            case 0x2180:
                wram[wram_port++ & 0x1FFFF] = v;
                break;
            case 0x2181: wram_port = (wram_port & 0x1FF00) | v; break;
            case 0x2182: wram_port = (wram_port & 0x100FF) | ((uint32_t)v << 8); break;
            case 0x2183: wram_port = (wram_port & 0x0FFFF) | ((uint32_t)(v & 1) << 16); break;
            case 0x420B:
                for (i = 0; i < 8; i++) if (v >> i & 1) run_dma(i);
                break;
            case 0x420C:
                for (i = 0; i < 8; i++) if (v >> i & 1) hdma_starts[i]++;
                break;
            default:
                break;
            }
        }
        return;
    }
    if (b >= 0x70 && b <= 0x7D && off < 0x8000) sram[off & sram_mask] = v;
}

/* ---- Report and stop ---- */

/* ---- Screenshot: background and sprite renderer, PNG writer ---- */

#define SCREEN_W 256
#define SCREEN_H 224

/* The result of the last write to each scroll register pair; they are written twice with a shared latch. */
static uint16_t bg_hofs[4], bg_vofs[4];
static uint8_t bgofs_latch;

static void write_scroll(uint32_t off, uint8_t v) {
    int bg = (int)((off - 0x210D) / 2);
    if ((off - 0x210D) % 2 == 0) {
        bg_hofs[bg] = (uint16_t)((v << 8) | (bgofs_latch & ~7) | ((bg_hofs[bg] >> 8) & 7));
    } else {
        bg_vofs[bg] = (uint16_t)((v << 8) | bgofs_latch);
    }
    bgofs_latch = v;
}

/* One row of a tile as colour indices: `bpp` planes of 8 pixels, planes 0-1 first, then 2-3 sixteen bytes on. */
static int tile_pixel(uint32_t byte_addr, int bpp, int ix, int iy) {
    int plane, c = 0;
    for (plane = 0; plane < bpp; plane++) {
        uint32_t a = (byte_addr + (uint32_t)((plane >> 1) * 16 + iy * 2 + (plane & 1))) & 0xFFFF;
        c |= (vram[a] >> (7 - ix) & 1) << plane;
    }
    return c;
}

/* CGRAM index of the pixel (x, y) of background `bg` (0-2) when its entry has priority `pri`, or -1. */
static int bg_pixel(int bg, int x, int y, int pri) {
    int bpp = bg < 2 ? 4 : 2;
    int tsz = (hw[0x2105] >> (4 + bg) & 1) ? 16 : 8;
    uint8_t map = hw[0x2107 + bg];
    int size = map & 3;
    int width = (size & 1 ? 64 : 32) * tsz, height = (size & 2 ? 64 : 32) * tsz;
    int px = (x + (bg_hofs[bg] & 0x3FF)) & (width - 1), py = (y + (bg_vofs[bg] & 0x3FF)) & (height - 1);
    int tx = px / tsz, ty = py / tsz, ix = px % tsz, iy = py % tsz;
    int sx = (size & 1) ? (tx >> 5) & 1 : 0, sy = (size & 2) ? (ty >> 5) & 1 : 0;
    uint32_t word = ((uint32_t)(map >> 2) << 10) + (uint32_t)(sx + sy * ((size & 1) ? 2 : 1)) * 0x400 + (uint32_t)((ty & 31) * 32 + (tx & 31));
    uint32_t entry = (uint32_t)(vram[(word * 2) & 0xFFFF] | vram[(word * 2 + 1) & 0xFFFF] << 8);
    uint32_t tile = entry & 0x3FF, chr;
    int pal = (int)(entry >> 10 & 7), c;
    if ((int)(entry >> 13 & 1) != pri) return -1;
    if (entry & 0x4000) ix = tsz - 1 - ix;
    if (entry & 0x8000) iy = tsz - 1 - iy;
    if (tsz == 16) {
        tile += (uint32_t)((ix >= 8) + (iy >= 8) * 16);
        ix &= 7;
        iy &= 7;
    }
    chr = bg == 0 ? (hw[0x210B] & 0xFu) : bg == 1 ? (uint32_t)(hw[0x210B] >> 4) : (hw[0x210C] & 0xFu);
    c = tile_pixel((chr << 13) + tile * (uint32_t)bpp * 8, bpp, ix, iy);
    if (!c) return -1;
    return bpp == 4 ? pal * 16 + c : pal * 4 + c;
}

static int16_t spr_col[SCREEN_H][SCREEN_W];
static uint8_t spr_pri[SCREEN_H][SCREEN_W];

static void draw_sprites(void) {
    static const int small_w[8] = {8, 8, 8, 16, 16, 32, 16, 16}, small_h[8] = {8, 8, 8, 16, 16, 32, 32, 32};
    static const int large_w[8] = {16, 32, 64, 32, 64, 64, 32, 32}, large_h[8] = {16, 32, 64, 32, 64, 64, 64, 32};
    int sel = hw[0x2101] >> 5, i, dy, dx;
    uint32_t name_base = (uint32_t)(hw[0x2101] & 7) << 13;
    uint32_t name_gap = (uint32_t)((hw[0x2101] >> 3 & 3) + 1) << 12;
    memset(spr_col, 0xFF, sizeof spr_col); /* -1: nothing */
    for (i = 127; i >= 0; i--) {
        int hi = oam[512 + i / 4] >> ((i % 4) * 2) & 3;
        int w = (hi & 2) ? large_w[sel] : small_w[sel], h = (hi & 2) ? large_h[sel] : small_h[sel];
        int x = oam[i * 4] | (hi & 1) << 8, y = oam[i * 4 + 1];
        uint8_t tile = oam[i * 4 + 2], attr = oam[i * 4 + 3];
        uint32_t table = name_base + ((attr & 1) ? name_gap : 0);
        if (x >= 256) x -= 512;
        for (dy = 0; dy < h; dy++) {
            int line = (y + 1 + dy) & 0xFF, iy = (attr & 0x80) ? h - 1 - dy : dy;
            if (line >= SCREEN_H) continue;
            for (dx = 0; dx < w; dx++) {
                int sx = x + dx, ix = (attr & 0x40) ? w - 1 - dx : dx, c;
                uint32_t sub;
                if (sx < 0 || sx >= SCREEN_W) continue;
                sub = (uint32_t)((((tile >> 4) + iy / 8) & 0xF) << 4 | (((tile & 0xF) + ix / 8) & 0xF));
                c = tile_pixel(((table + sub * 16) * 2) & 0xFFFF, 4, ix & 7, iy & 7);
                if (!c) continue;
                spr_col[line][sx] = (int16_t)(128 + (attr >> 1 & 7) * 16 + c);
                spr_pri[line][sx] = (uint8_t)(attr >> 4 & 3);
            }
        }
    }
}

/* Layers front to back: kind 0 = sprites of priority n, 1 = BGn+1 with tile priority 1, 2 = BGn+1 with 0. */
typedef struct { int kind, n; } Layer;

/* Layer ids reported by pixel_index: 0-2 = BG1-3, 4 = sprites, 5 = backdrop. */
#define LAYER_OBJ 4
#define LAYER_BACKDROP 5

/* CGRAM index of the front pixel of the layers in `mask` (a $212C/$212D value); `*layer` says which layer gave it. */
static int pixel_index(const Layer *order, int count, int x, int y, int mode1, uint8_t mask, int *layer) {
    int i, c;
    for (i = 0; i < count; i++) {
        int n = order[i].n;
        if (order[i].kind == 0) {
            if (!(mask & 0x10)) continue;
            if (spr_col[y][x] >= 0 && spr_pri[y][x] == n) { *layer = LAYER_OBJ; return spr_col[y][x]; }
        } else {
            if (!mode1 || !(mask >> n & 1)) continue;
            c = bg_pixel(n, x, y, order[i].kind == 1);
            if (c >= 0) { *layer = n; return c; }
        }
    }
    *layer = LAYER_BACKDROP;
    return 0;
}

static int cgram_color(int idx) { return cgram[idx * 2] | (cgram[idx * 2 + 1] & 0x7F) << 8; }

/* A main screen pixel after colour math, as BGR555; `sub_c` is the subscreen pixel there, if `has_sub`. */
static int colour_math(int main_c, int layer, int main_idx, int sub_c, int has_sub) {
    int chosen = layer == LAYER_OBJ ? (main_idx >= 192 && (hw[0x2131] & 0x10)) : (hw[0x2131] >> layer & 1);
    int half = hw[0x2131] >> 6 & 1, sub = hw[0x2131] >> 7 & 1, k, out = 0;
    if (!chosen || (hw[0x2130] >> 4 & 3) == 3) return main_c;
    if (!(hw[0x2130] & 2) || !has_sub) {
        sub_c = fixed_col[0] | fixed_col[1] << 5 | fixed_col[2] << 10;
        if (hw[0x2130] & 2) half = 0; /* the fixed colour stands in for an empty subscreen and is not halved */
    }
    for (k = 0; k < 3; k++) {
        int a = main_c >> (5 * k) & 31, b = sub_c >> (5 * k) & 31, r = sub ? a - b : a + b;
        if (r < 0) r = 0;
        if (half) r >>= 1;
        if (r > 31) r = 31;
        out |= r << (5 * k);
    }
    return out;
}

static uint32_t crc_table[256];

static uint32_t crc32_update(uint32_t crc, const uint8_t *p, size_t n) {
    size_t i;
    if (!crc_table[1]) {
        uint32_t k, j, c;
        for (k = 0; k < 256; k++) {
            c = k;
            for (j = 0; j < 8; j++) c = (c & 1) ? 0xEDB88320u ^ (c >> 1) : c >> 1;
            crc_table[k] = c;
        }
    }
    crc = ~crc;
    for (i = 0; i < n; i++) crc = crc_table[(crc ^ p[i]) & 0xFF] ^ (crc >> 8);
    return ~crc;
}

static void put32(uint8_t *p, uint32_t v) {
    p[0] = (uint8_t)(v >> 24); p[1] = (uint8_t)(v >> 16); p[2] = (uint8_t)(v >> 8); p[3] = (uint8_t)v;
}

static void write_chunk(FILE *f, const char *type, const uint8_t *data, uint32_t n) {
    uint8_t b[4];
    uint32_t crc;
    put32(b, n);
    fwrite(b, 1, 4, f);
    fwrite(type, 1, 4, f);
    if (n) fwrite(data, 1, n, f);
    crc = crc32_update(0, (const uint8_t *)type, 4);
    crc = crc32_update(crc, data, n);
    put32(b, crc);
    fwrite(b, 1, 4, f);
}

/* RGB image as a PNG whose zlib stream has only stored (uncompressed) blocks. */
static int write_png(const char *path, const uint8_t *rgb, uint32_t w, uint32_t h) {
    static const uint8_t sig[8] = {0x89, 'P', 'N', 'G', 0x0D, 0x0A, 0x1A, 0x0A};
    size_t row = 1 + (size_t)w * 3, raw = row * h, blocks = (raw + 65534) / 65535, z = 2 + raw + blocks * 5 + 4, pos = 2, off = 0, y;
    uint8_t *zbuf = (uint8_t *)malloc(z), *line = (uint8_t *)malloc(raw), ihdr[13];
    uint32_t a = 1, b = 0;
    FILE *f;
    if (!zbuf || !line) { free(zbuf); free(line); return 0; }
    for (y = 0; y < h; y++) {
        line[y * row] = 0; /* filter: none */
        memcpy(line + y * row + 1, rgb + y * w * 3, (size_t)w * 3);
    }
    zbuf[0] = 0x78; zbuf[1] = 0x01;
    while (off < raw) {
        size_t n = raw - off > 65535 ? 65535 : raw - off, i;
        zbuf[pos++] = off + n == raw;
        zbuf[pos++] = (uint8_t)n; zbuf[pos++] = (uint8_t)(n >> 8);
        zbuf[pos++] = (uint8_t)~n; zbuf[pos++] = (uint8_t)(~n >> 8);
        memcpy(zbuf + pos, line + off, n);
        for (i = 0; i < n; i++) { a = (a + line[off + i]) % 65521; b = (b + a) % 65521; }
        pos += n;
        off += n;
    }
    put32(zbuf + pos, (b << 16) | a);
    pos += 4;
    put32(ihdr, w); put32(ihdr + 4, h);
    ihdr[8] = 8; ihdr[9] = 2; ihdr[10] = 0; ihdr[11] = 0; ihdr[12] = 0;
    f = fopen(path, "wb");
    if (f) {
        fwrite(sig, 1, 8, f);
        write_chunk(f, "IHDR", ihdr, 13);
        write_chunk(f, "IDAT", zbuf, (uint32_t)pos);
        write_chunk(f, "IEND", NULL, 0);
        fclose(f);
    }
    free(zbuf);
    free(line);
    return f != NULL;
}

static const char *shot_path;

static void screenshot(void) {
    static const Layer front_bg3_low[10] = {{0, 3}, {1, 0}, {1, 1}, {0, 2}, {2, 0}, {2, 1}, {0, 1}, {1, 2}, {0, 0}, {2, 2}};
    static const Layer front_bg3_high[10] = {{1, 2}, {0, 3}, {1, 0}, {1, 1}, {0, 2}, {2, 0}, {2, 1}, {0, 1}, {0, 0}, {2, 2}};
    const Layer *order = (hw[0x2105] & 8) ? front_bg3_high : front_bg3_low;
    int mode = hw[0x2105] & 7, bright = hw[0x2100] & 15, blank = hw[0x2100] >> 7 & 1, x, y;
    uint8_t *rgb = (uint8_t *)malloc((size_t)SCREEN_W * SCREEN_H * 3);
    if (mode != 1) printf("warning: graphics mode %d is not drawn\n", mode);
    if (hw[0x212E] | hw[0x212F]) printf("warning: windows are not applied\n");
    if (hw[0x2106] & 0x0F) printf("warning: mosaic is not applied\n");
    if ((hw[0x2131] & 0x3F) && ((hw[0x2130] >> 4 & 3) == 1 || (hw[0x2130] >> 4 & 3) == 2))
        printf("warning: colour math inside or outside the colour window is not applied\n");
    if (!rgb) return;
    draw_sprites();
    for (y = 0; y < SCREEN_H; y++) {
        for (x = 0; x < SCREEN_W; x++) {
            int layer = LAYER_BACKDROP, sub_layer, idx = blank ? -1 : pixel_index(order, 10, x, y, mode == 1, hw[0x212C], &layer);
            uint8_t *p = rgb + ((size_t)y * SCREEN_W + (size_t)x) * 3;
            int c = idx < 0 ? 0 : cgram_color(idx), k;
            if (idx >= 0 && (hw[0x2131] & 0x3F)) {
                int sub_idx = pixel_index(order, 10, x, y, mode == 1, hw[0x212D], &sub_layer);
                c = colour_math(c, layer, idx, cgram_color(sub_idx), sub_layer != LAYER_BACKDROP);
            }
            for (k = 0; k < 3; k++) {
                int c5 = c >> (5 * k) & 31, c8 = (c5 << 3) | (c5 >> 2);
                p[k] = (uint8_t)(idx < 0 ? 0 : c8 * bright / 15);
            }
        }
    }
    if (!write_png(shot_path, rgb, SCREEN_W, SCREEN_H)) fprintf(stderr, "error: cannot write %s\n", shot_path);
    free(rgb);
}

static void dump_file(const char *name, const uint8_t *data, size_t n) {
    char path[1024];
    FILE *f;
    snprintf(path, sizeof path, "%s/%s", dump_dir, name);
    f = fopen(path, "wb");
    if (!f || fwrite(data, 1, n, f) != n) fprintf(stderr, "error: cannot write %s\n", path);
    if (f) fclose(f);
}

static void dump_memories(void) {
    dump_file("vram.bin", vram, sizeof vram);
    dump_file("cgram.bin", cgram, sizeof cgram);
    dump_file("oam.bin", oam, sizeof oam);
    dump_file("ppu_regs.bin", hw + 0x2100, 0x40);
}

static void report(const char *cause) {
    uint32_t a;
    int i, any = 0;
    printf("== report ==\n");
    printf("stop: %s\n", cause);
    printf("frames: %u\nnmis: %u\nirqs: %u\ncontext switches: %u\nblocks: %llu\n", (unsigned)frames, (unsigned)nmis,
           (unsigned)irqs, ctx_switches, (unsigned long long)blocks);
    printf("registers:\n");
    for (a = IO_FIRST; a <= IO_LAST; a++) {
        if (hw_reads[a] || hw_writes[a]) {
            printf("  $%04X reads=%u writes=%u last=%02X\n", (unsigned)a, (unsigned)hw_reads[a], (unsigned)hw_writes[a], hw[a]);
        }
    }
    for (i = 0; i < 8; i++) {
        if (dma_starts[i]) {
            printf("dma ch%d: %u\ndma ch%d bytes: %llu\n", i, (unsigned)dma_starts[i], i, (unsigned long long)dma_bytes[i]);
            any = 1;
        }
    }
    for (i = 0; i < 8; i++) {
        if (hdma_starts[i]) { printf("hdma ch%d: %u\n", i, (unsigned)hdma_starts[i]); any = 1; }
    }
    if (!any) printf("dma: none\n");
    if (vram_translate) printf("warning: VRAM address translation ($2115=%02X) is not modelled\n", vram_translate);
    if (dump_dir) dump_memories();
    if (shot_path) screenshot();
}

static void stop(int code, const char *cause) {
    trace_flush();
    report(cause);
    exit(code);
}

void rt_unresolved(Cpu *c, uint32_t site, uint32_t target) {
    (void)c;
    printf("UNRESOLVED site=%06X target=%06X\n", (unsigned)site, (unsigned)target);
    stop(5, "unresolved indirect jump");
}
void rt_flag_mismatch(Cpu *c, uint32_t addr) {
    printf("FLAG_MISMATCH %06X (cpu has M=%d X=%d)\n", (unsigned)addr, c->m ? 8 : 16, c->xf ? 8 : 16);
    stop(6, "register width mismatch");
}
void rt_unsupported(Cpu *c, uint32_t entry) {
    (void)c;
    printf("UNSUPPORTED %06X\n", (unsigned)entry);
    stop(9, "unsupported function");
}
void rt_stp(Cpu *c) { (void)c; printf("STP\n"); stop(7, "STP"); }
void rt_brk(Cpu *c) { (void)c; printf("BRK\n"); stop(7, "BRK"); }
void rt_cop(Cpu *c) { (void)c; printf("COP\n"); stop(7, "COP"); }
void rt_wdm(Cpu *c) { (void)c; }

/* ---- Frames and interrupts ---- */

static uint16_t vector(uint32_t addr) {
    return (uint16_t)(rom_byte(addr) | (rom_byte(addr + 1) << 8));
}

/* ---- Execution contexts ----
 * A handler may swap the stack and end in RTI, which returns to another context: the one whose
 * interrupt frame is on the new stack. Each context runs on a native stack of its own (a fibre), so a
 * suspended one keeps its pending C calls. An RTI whose S is not the one its handler started with
 * switches to the suspended context whose handler started with that S or, if there is none, starts a
 * new context at the address in the frame. */

#define MAX_CTX 5
#define MAX_REC 4
#define CTX_STACK (1u << 20)

typedef struct { uint16_t s; int nmi; } Rec; /* a handler running: S right after its frame was pushed */
typedef struct { void *handle; Rec rec[MAX_REC]; int depth; int used; } Ctx;

static Ctx ctxs[MAX_CTX];
static int cur_ctx;
static Cpu *ctx_cpu;
static uint32_t ctx_entry_addr;

#ifdef _WIN32
static void *ctx_main_handle(void) { return ConvertThreadToFiber(NULL); }
static void *ctx_make(void (*fn)(void *)) { return CreateFiber(CTX_STACK, (LPFIBER_START_ROUTINE)fn, NULL); }
static void ctx_jump(Ctx *from, Ctx *to) { (void)from; SwitchToFiber(to->handle); }
#else
static void (*ctx_fn)(void *);
static void ctx_tramp(void) { ctx_fn(NULL); }
static void *ctx_main_handle(void) { return calloc(1, sizeof(ucontext_t)); }
static void *ctx_make(void (*fn)(void *)) {
    ucontext_t *u = (ucontext_t *)calloc(1, sizeof(ucontext_t));
    getcontext(u);
    u->uc_stack.ss_sp = malloc(CTX_STACK);
    u->uc_stack.ss_size = CTX_STACK;
    u->uc_link = NULL;
    ctx_fn = fn;
    makecontext(u, ctx_tramp, 0);
    return u;
}
static void ctx_jump(Ctx *from, Ctx *to) { swapcontext((ucontext_t *)from->handle, (ucontext_t *)to->handle); }
#endif

static void ctx_init(void) {
    memset(ctxs, 0, sizeof ctxs);
    ctxs[0].used = 1;
    ctxs[0].handle = ctx_main_handle();
    cur_ctx = 0;
}

/* The NMI flag of the running context: it is inside an NMI handler. */
static void recompute_in_nmi(void) {
    int i;
    in_nmi = 0;
    for (i = 0; i < ctxs[cur_ctx].depth; i++) if (ctxs[cur_ctx].rec[i].nmi) in_nmi = 1;
}

static void switch_to(Cpu *c, int to) {
    int from = cur_ctx;
    ctx_cpu = c;
    cur_ctx = to;
    ctx_switches++;
    recompute_in_nmi();
    ctx_jump(&ctxs[from], &ctxs[to]);
}

/* An RTS/RTL returned to an address no active call expects and the return climbed out of the generated code. */
void rt_lost_return(Cpu *c) {
    printf("LOST_RETURN to %04X with S=%04X\n", (unsigned)c->ret, (unsigned)c->s);
    stop(14, "return to an address no call expects");
}

/* Calls the function at `addr` as the CPU would enter it from outside the generated code. Such a function
 * ends with RTI, which leaves c->ret as RT_NO_RET; if an RTS or RTL got here instead, nothing accepted it. */
static int call_top(Cpu *c, uint32_t addr) {
    int ok;
    c->ret = RT_NO_RET;
    ok = snes_call_entry(c, addr);
    if (ok && c->ret != RT_NO_RET) rt_lost_return(c);
    return ok;
}

/* Body of a context started from a frame the code built by hand. */
static void ctx_body(void *unused) {
    (void)unused;
    if (!call_top(ctx_cpu, ctx_entry_addr)) {
        printf("context entry %06X is not the entry of a function\n", (unsigned)ctx_entry_addr);
        stop(10, "context entry is not a function");
    }
    printf("the context started at %06X returned\n", (unsigned)ctx_entry_addr);
    stop(12, "context returned");
}

void rt_rti(Cpu *c) {
    Ctx *cx = &ctxs[cur_ctx];
    int j, k;
    uint32_t pc, pbr;
    if (cx->depth && c->s == cx->rec[cx->depth - 1].s) {
        rt_rti_pop(c);
        return;
    }
    for (j = 0; j < MAX_CTX; j++) {
        if (j != cur_ctx && ctxs[j].used && ctxs[j].depth && ctxs[j].rec[ctxs[j].depth - 1].s == c->s) {
            rt_rti_pop(c);
            switch_to(c, j);
            return;
        }
    }
    /* A frame built by the code: start a context at its address. */
    pc = (uint32_t)bus_read8((uint16_t)(c->s + 2)) | (uint32_t)bus_read8((uint16_t)(c->s + 3)) << 8;
    pbr = c->e ? 0 : bus_read8((uint16_t)(c->s + 4));
    rt_rti_pop(c);
    for (k = 0; k < MAX_CTX && ctxs[k].used; k++) {}
    if (k == MAX_CTX) {
        printf("more than %d execution contexts\n", MAX_CTX);
        stop(13, "too many execution contexts");
    }
    memset(&ctxs[k], 0, sizeof ctxs[k]);
    ctxs[k].used = 1;
    ctxs[k].handle = ctx_make(ctx_body);
    ctx_entry_addr = pbr << 16 | pc;
    switch_to(c, k);
}

/* Enter an interrupt handler as the CPU does: push the (unused) return address and the flags. */
static void deliver_interrupt(Cpu *c, uint16_t v, int is_nmi) {
    int me = cur_ctx;
    Ctx *cx = &ctxs[me];
    if (cx->depth == MAX_REC) stop(13, "interrupts nested too deep");
    if (is_nmi) nmis++; else irqs++;
    if (!c->e) rt_push8(c, 0x00);
    rt_push16(c, 0x0000);
    rt_push8(c, rt_get_p(c));
    cx->rec[cx->depth].s = c->s;
    cx->rec[cx->depth].nmi = is_nmi;
    cx->depth++;
    recompute_in_nmi();
    c->irq = 1;
    c->dec = 0;
    /* The recompiler analyses a handler as entered with 8-bit M and X (its config entry) and the
     * handler sets the widths it needs first thing. The flags pushed above keep the real widths, and
     * the registers keep their high bytes, so the RTI gives everything back as it was. */
    c->m = 1;
    c->xf = 1;
    if (!call_top(c, v)) {
        printf("%s vector %04X is not the entry of a function\n", is_nmi ? "NMI" : "IRQ", (unsigned)v);
        stop(10, "interrupt vector is not a function");
    }
    /* This context is running again (after a switch away and back, if the handler made one). */
    ctxs[me].depth--;
    recompute_in_nmi();
}

static void deliver_nmi(Cpu *c) {
    nmi_pending = 0;
    deliver_interrupt(c, vector(c->e ? 0xFFFA : 0xFFEA), 1);
}

/* IRQ line: the vertical position is counted from the NMI (line 225), so the IRQ comes after it. */
static int irq_due(void) {
    uint8_t mode = hw[0x4200] >> 4 & 3;
    uint32_t line;
    if (!mode) { irq_flag = 0; return 0; }
    if (!irq_fired) {
        line = mode == 1 ? 0 : (uint32_t)(hw[0x4209] | (hw[0x420A] & 1) << 8);
        if (line < 262 && tick_in_frame >= (uint64_t)frame_len * ((line + 262 - 225) % 262) / 262) {
            irq_fired = 1;
            irq_flag = 1;
        }
    }
    return irq_flag;
}

void rt_poll(Cpu *c) {
    blocks++;
    if (++tick_in_frame >= frame_len) {
        tick_in_frame = 0;
        frames++;
        nmi_flag = 1;
        irq_fired = 0;
        if (hw[0x4200] & 0x80) nmi_pending = 1;
    }
    if (nmi_pending && !in_nmi) deliver_nmi(c);
    if (irq_due() && !c->irq) deliver_interrupt(c, vector(c->e ? 0xFFFE : 0xFFEE), 0);
    /* The last NMI runs to its RTI before the run ends. */
    if (frames >= max_frames && !in_nmi) stop(0, "frames reached");
}

void rt_wai(Cpu *c) {
    tick_in_frame = frame_len - 1;
    rt_poll(c);
}

/* ---- Entry point ---- */

static int load_rom(const char *path) {
    FILE *f = fopen(path, "rb");
    long n;
    size_t skip = 0;
    uint8_t *buf;
    if (!f) { fprintf(stderr, "error: cannot read %s\n", path); return 0; }
    fseek(f, 0, SEEK_END);
    n = ftell(f);
    fseek(f, 0, SEEK_SET);
    if (n % 0x8000 == 0x200) skip = 0x200; /* copier header */
    if (n <= 0 || (size_t)n - skip < 0x8000 || ((size_t)n - skip) % 0x8000 != 0) {
        fprintf(stderr, "error: %s: %ld bytes is not the size of a LoROM image\n", path, n);
        fclose(f);
        return 0;
    }
    buf = (uint8_t *)malloc((size_t)n);
    if (!buf || fread(buf, 1, (size_t)n, f) != (size_t)n) {
        fprintf(stderr, "error: cannot read %s\n", path);
        fclose(f);
        free(buf);
        return 0;
    }
    fclose(f);
    rom = buf + skip;
    rom_size = (size_t)n - skip;
    if (rom_size > 0x7FD8 && rom[0x7FD8] > 0 && rom[0x7FD8] <= 15 && (1024u << rom[0x7FD8]) <= sizeof sram) {
        sram_mask = (1024u << rom[0x7FD8]) - 1;
    }
    return 1;
}

int main(int argc, char **argv) {
    Cpu c;
    const char *path = NULL;
    int i;
    for (i = 1; i < argc; i++) {
        if (!strcmp(argv[i], "--frames") && i + 1 < argc) {
            max_frames = (uint32_t)strtoul(argv[++i], NULL, 10);
        } else if (!strcmp(argv[i], "--frame-blocks") && i + 1 < argc) {
            frame_len = (uint32_t)strtoul(argv[++i], NULL, 10);
        } else if (!strcmp(argv[i], "--screenshot") && i + 1 < argc) {
            shot_path = argv[++i];
        } else if (!strcmp(argv[i], "--trace-frames") && i + 1 < argc) {
            char *end;
            trace_lo = (uint32_t)strtoul(argv[++i], &end, 10);
            if (*end != '-') { path = NULL; break; }
            trace_hi = (uint32_t)strtoul(end + 1, &end, 10);
            if (*end || trace_hi < trace_lo) { path = NULL; break; }
            trace_window = 1;
        } else if (!strcmp(argv[i], "--trace-reads")) {
            trace_reads_on = 1;
        } else if (!strcmp(argv[i], "--watch") && i + 1 < argc) {
            char *end;
            unsigned long v = strtoul(argv[++i], &end, 16);
            if (*end == ':') {
                unsigned long bank = v;
                v = strtoul(end + 1, &end, 16);
                if (*end || v > 0xFFFF || (bank != 0x7E && bank != 0x7F)) { path = NULL; break; }
                v |= (bank & 1) << 16;
            } else if (*end || v > 0x1FFFF) {
                path = NULL;
                break;
            }
            watch[v & 0x1FFFF] = 1;
            watch_any = 1;
        } else if (!strcmp(argv[i], "--dump") && i + 1 < argc) {
            dump_dir = argv[++i];
        } else if (argv[i][0] != '-' && !path) {
            path = argv[i];
        } else {
            path = NULL;
            break;
        }
    }
    if (!path || max_frames == 0 || frame_len == 0) {
        fprintf(stderr, "usage: %s <rom> [--frames N] [--frame-blocks N] [--dump DIR] [--screenshot FILE]\n"
                        "       [--trace-frames A-B] [--trace-reads] [--watch ADDR]...\n", argv[0]);
        return 2;
    }
    if (!load_rom(path)) return 2;
    ctx_init();

    memset(&c, 0, sizeof c);
    c.e = 1; c.m = 1; c.xf = 1; c.s = 0x01FF; c.irq = 1; /* state right after reset */
    if (!call_top(&c, vector(0xFFFC))) {
        printf("reset vector %04X is not the entry of a function\n", (unsigned)vector(0xFFFC));
        stop(10, "interrupt vector is not a function");
    }
    stop(11, "reset returned");
    return 11;
}
