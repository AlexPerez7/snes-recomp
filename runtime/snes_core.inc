/* snes_core.inc - hardware core of the runtimes: bus, registers, DMA, clock, execution contexts, report.
 * Included by headless.c (and by play.c); it is not compiled on its own.
 *
 * Library mode (SNES_RT_LIBRARY): the runtime is built without the generated C and holds nothing derived from the
 * ROM; the module with the code of the game joins it at run time by calling its snes_rt_module_init (generated in
 * snes_entry.c), which registers the dispatch by address with snes_rt_register_entry. Without the macro the generated
 * C is linked in one piece and the dispatch is snes_call_entry from the start. */
#ifdef SNES_RT_LIBRARY
#include "snes_rt.h"
#else
#include "snes_funcs.h"
#endif
#include <stdio.h>
#include <ctype.h>
#include <math.h>
#include <stdlib.h>
#include <string.h>
#if defined(__ANDROID__) && !defined(SNES_RT_THREAD_CONTEXTS)
#define SNES_RT_THREAD_CONTEXTS /* bionic has no makecontext/swapcontext */
#endif
#ifdef _WIN32
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#elif defined(SNES_RT_THREAD_CONTEXTS)
#include <pthread.h>
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

/* ---- Clock ---- */

static uint32_t frame_len = 20000; /* blocks per frame */
static uint32_t max_frames = 60;
static uint32_t tick_in_frame;
static uint32_t frames, nmis;

#include "snes_spc.inc"
static uint64_t blocks;
static int nmi_flag;    /* $4210 bit 7 */
static int nmi_pending; /* a frame ended with NMI enabled and it has not been delivered yet */
static int in_nmi;
static int irq_flag;    /* $4211 bit 7: the IRQ line is asserted until $4211 is read */
static int irq_fired;   /* the IRQ of this frame has been raised */
static uint32_t irqs;
static unsigned ctx_switches;

/* ---- Joypad 1 ---- */

/* Buttons in the layout of $4219:$4218: B Y Select Start Up Down Left Right A X L R from bit 15 to bit 4. */
#define JOY_NAMES 12
static const char *const joy_name[JOY_NAMES] = {"B", "Y", "SELECT", "START", "UP", "DOWN", "LEFT", "RIGHT", "A", "X", "L", "R"};
#define MAX_PRESS 2048
static struct { uint16_t mask; uint32_t lo, hi; } press[MAX_PRESS];
static int npress;
static int joy_read_done; /* the auto-read of this frame has been done */
static int joy_latched;   /* $4016 bit 0 is 1 */
static uint16_t joy_cap;  /* buttons captured by the last latch */
static int joy_count;     /* bits of joy_cap already read through $4016 */
static uint16_t joy_cap2; /* the same for pad 2 ($4017) */
static int joy_count2;

/* The buttons held down during the current frame. */
/* Set by a frontend: called once per frame, right after the picture of the frame is complete. */
static void (*frame_hook)(void);
static uint16_t live_pad; /* buttons held according to a live source (a keyboard, a pad), as in --press */
static uint16_t live_pad2; /* the same for pad 2: only a live source presses its buttons */

static uint16_t joy_now(void) {
    uint16_t m = live_pad;
    int i;
    for (i = 0; i < npress; i++) if (frames >= press[i].lo && frames <= press[i].hi) m |= press[i].mask;
    return m;
}

/* For the C code of a game (snes_rt.h). */
uint16_t joy1_state(void) { return joy_now(); }

#define MAX_GAME_OPT 16
static struct { char key[32]; long val; } game_opts[MAX_GAME_OPT];
static int ngame_opts;

/* Parses KEY=NUMBER of --game-opt; returns 0 when it is malformed or there are too many. */
static int parse_game_opt(const char *arg) {
    const char *eq = strchr(arg, '=');
    char *end;
    long v;
    if (!eq || eq == arg || (size_t)(eq - arg) >= sizeof game_opts[0].key || ngame_opts == MAX_GAME_OPT || !eq[1]) return 0;
    v = strtol(eq + 1, &end, 0);
    if (*end) return 0;
    memcpy(game_opts[ngame_opts].key, arg, (size_t)(eq - arg));
    game_opts[ngame_opts].key[eq - arg] = 0;
    game_opts[ngame_opts].val = v;
    ngame_opts++;
    return 1;
}

int game_opt(const char *key, int def) {
    int i;
    for (i = ngame_opts - 1; i >= 0; i--) if (!strcmp(game_opts[i].key, key)) return (int)game_opts[i].val;
    return def;
}

/* Parses BUTTONS@A-B, BUTTONS being names joined by '+'; returns 0 when it is malformed. */
static int parse_press(const char *arg) {
    char buf[128], *at, *end, *tok, *next;
    uint16_t mask = 0;
    unsigned long lo, hi;
    if (npress == MAX_PRESS || strlen(arg) >= sizeof buf) return 0;
    strcpy(buf, arg);
    at = strchr(buf, '@');
    if (!at) return 0;
    *at++ = 0;
    for (tok = buf; tok; tok = next) {
        int i, found = 0;
        next = strchr(tok, '+');
        if (next) *next++ = 0;
        for (i = 0; i < JOY_NAMES; i++) {
            if (!strcmp(tok, joy_name[i])) { mask |= (uint16_t)(0x8000 >> i); found = 1; }
        }
        if (!found) return 0;
    }
    lo = strtoul(at, &end, 10);
    if (end == at || *end != '-') return 0;
    at = end + 1;
    hi = strtoul(at, &end, 10);
    if (end == at || *end || hi < lo) return 0;
    press[npress].mask = mask;
    press[npress].lo = (uint32_t)lo;
    press[npress].hi = (uint32_t)hi;
    npress++;
    return 1;
}

/* At the start of vblank the console reads the pads on its own when $4200 bit 0 asks for it. */
static void joy_auto_read(void) {
    uint16_t m = joy_now();
    if (!(hw[0x4200] & 1)) return;
    hw[0x4218] = (uint8_t)m;
    hw[0x4219] = (uint8_t)(m >> 8);
    hw[0x421A] = (uint8_t)live_pad2;
    hw[0x421B] = (uint8_t)(live_pad2 >> 8);
}

/* $4212 bit 0: the auto-read takes the first 3 of 262 lines of vblank. */
/* H/V counter latch ($2137): $213C/$213D give the low byte then bit 0 of each counter. */
static uint16_t hv_h, hv_v;
static int hv_latched, hv_read[2];
static uint8_t hv_get(int vertical) {
    uint16_t c = vertical ? hv_v : hv_h;
    return hv_read[vertical]++ & 1 ? (uint8_t)(c >> 8 & 1) : (uint8_t)c;
}

static int joy_busy(void) {
    uint64_t start = (uint64_t)frame_len * 9 / 10, len = (uint64_t)frame_len * 3 / 262;
    return tick_in_frame >= start && tick_in_frame < start + (len ? len : 1);
}

static void report(const char *cause);
static void ppu_sync(void);
static void write_scroll(uint32_t off, uint8_t v);
static void write_m7(uint32_t off, uint8_t v);

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
static uint8_t m7_product_byte(int byte);

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
            case 0x2134: case 0x2135: case 0x2136:
                return m7_product_byte(off - 0x2134);
            case 0x2138:
                v = oam[oam_slot()];
                oam_byte = (oam_byte + 1) & 0x3FF;
                return v;
            case 0x2139: return vram_read(0);
            case 0x213A: return vram_read(1);
            case 0x213B: return cgram_read();
            case 0x2137: /* latch the counters; the value read is open bus */
                hv_v = (uint16_t)((uint64_t)tick_in_frame * 262 / frame_len);
                hv_h = (uint16_t)((uint64_t)tick_in_frame * 262 % frame_len * 340 / frame_len);
                hv_latched = 1;
                return hw[off];
            case 0x213C: case 0x213D:
                v = hv_get(off == 0x213D);
                return v;
            case 0x213F:
                v = (uint8_t)(hv_latched ? 0x40 : 0) | 0x03;
                hv_latched = 0; hv_read[0] = hv_read[1] = 0;
                return v;
            case 0x213E:
                return hw[off];
            case 0x2140: case 0x2141: case 0x2142: case 0x2143:
                return spc_out[off - 0x2140];
            case 0x2180:
                return wram[wram_port++ & 0x1FFFF];
            case 0x4016:
                if (joy_latched) return (uint8_t)(joy_cap >> 15 & 1);
                return (uint8_t)(joy_count < 16 ? joy_cap >> (15 - joy_count++) & 1 : 1);
            case 0x4017:
                if (joy_latched) return (uint8_t)(live_pad2 >> 15 & 1);
                return (uint8_t)(joy_count2 < 16 ? joy_cap2 >> (15 - joy_count2++) & 1 : 0);
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
                if (joy_busy()) v |= 0x01;
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

/* Reads a WRAM address (0000-1FFF, 7E:xxxx, 7F:xxxx or a 17-bit offset) into *out; *end is left after it. */
static int parse_wram_addr(const char *arg, char **end, uint32_t *out) {
    unsigned long v = strtoul(arg, end, 16);
    if (*end == arg) return 0;
    if (**end == ':') {
        unsigned long bank = v;
        const char *rest = *end + 1;
        v = strtoul(rest, end, 16);
        if (*end == rest || v > 0xFFFF || (bank != 0x7E && bank != 0x7F)) return 0;
        v |= (bank & 1) << 16;
    } else if (v > 0x1FFFF) {
        return 0;
    }
    *out = (uint32_t)v;
    return 1;
}

/* --poke ADDR=BYTES@FRAME: BYTES are hex pairs written to consecutive WRAM bytes when FRAME starts. */
#define MAX_POKE 256
static struct { uint32_t addr, frame; uint8_t value; } poke[MAX_POKE];
static int npoke;

static int parse_poke(const char *arg) {
    char *end;
    uint32_t addr, n = 0, frame, first = (uint32_t)npoke;
    const char *p;
    unsigned long f;
    if (!parse_wram_addr(arg, &end, &addr) || *end != '=') return 0;
    for (p = end + 1; isxdigit((unsigned char)p[0]) && isxdigit((unsigned char)p[1]); p += 2, n++) {
        char pair[3] = { p[0], p[1], 0 };
        if (npoke == MAX_POKE || addr + n > 0x1FFFF) return 0;
        poke[npoke].addr = addr + n;
        poke[npoke].value = (uint8_t)strtoul(pair, NULL, 16);
        npoke++;
    }
    if (n == 0 || *p != '@') return 0;
    f = strtoul(p + 1, &end, 10);
    if (end == p + 1 || *end) return 0;
    frame = (uint32_t)f;
    for (; first < (uint32_t)npoke; first++) poke[first].frame = frame;
    return 1;
}

static void apply_pokes(void) {
    int i;
    for (i = 0; i < npoke; i++) if (poke[i].frame == frames) wram[poke[i].addr] = poke[i].value;
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
            if (off <= 0x213F) ppu_sync(); /* draw the rows that came due with the state they had until this write */
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
            case 0x211B: case 0x211C: case 0x211D: case 0x211E: case 0x211F: case 0x2120: write_m7(off, v); break;
            case 0x2121: cg_index = v; cg_write_half = 0; cg_read_half = 0; break;
            case 0x2122: cgram_write(v); break;
            case 0x4016:
                /* Latch: while the bit is 1 the pad's shift register follows the buttons; when it goes back to 0
                 * the buttons of that moment are what the serial reads return. */
                joy_cap = joy_now();
                joy_count = 0;
                joy_cap2 = live_pad2;
                joy_count2 = 0;
                joy_latched = v & 1;
                break;
            case 0x2132:
                for (i = 0; i < 3; i++) if (v >> (5 + i) & 1) fixed_col[i] = v & 31;
                break;
            case 0x2140: case 0x2141: case 0x2142: case 0x2143:
                spc_in[off - 0x2140] = v;
                break;
            case 0x2180:
                wram[wram_port++ & 0x1FFFF] = v;
                break;
            case 0x2181: wram_port = (wram_port & 0x1FF00) | v; break;
            case 0x2182: wram_port = (wram_port & 0x100FF) | ((uint32_t)v << 8); break;
            case 0x2183: wram_port = (wram_port & 0x0FFFF) | ((uint32_t)(v & 1) << 16); break;
            case 0x4203: { /* unsigned 8x8 multiply: $4202 * $4203 -> $4216/$4217 (the hardware takes 8 cycles; here it is instant) */
                uint16_t p = (uint16_t)(hw[0x4202] * v);
                hw[0x4216] = (uint8_t)p;
                hw[0x4217] = (uint8_t)(p >> 8);
                break;
            }
            case 0x4206: { /* unsigned 16/8 divide: $4204/$4205 / $4206 -> quotient $4214/$4215, remainder $4216/$4217 */
                uint16_t n = (uint16_t)(hw[0x4204] | hw[0x4205] << 8), q, r;
                if (v) { q = (uint16_t)(n / v); r = (uint16_t)(n % v); } else { q = 0xFFFF; r = n; }
                hw[0x4214] = (uint8_t)q;
                hw[0x4215] = (uint8_t)(q >> 8);
                hw[0x4216] = (uint8_t)r;
                hw[0x4217] = (uint8_t)(r >> 8);
                break;
            }
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


#include "snes_ppu.inc"

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
    dump_file("wram.bin", wram, sizeof wram);
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

static int quiet_exit; /* a normal end prints no report (the interactive frontend) */

static void stop(int code, const char *cause) {
    trace_flush();
    if (!(quiet_exit && code == 0)) report(cause);
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
#elif defined(SNES_RT_THREAD_CONTEXTS)
/* Each context is a thread of its own and only one runs at a time: ctx_jump lets `to` go on and waits until another
 * jump lets `from` go on again. For systems without fibres or ucontext (Android); slower to switch, and a game switches
 * a few times per frame at most. */
typedef struct { void (*fn)(void *); int run; } ThreadCtx;
static pthread_mutex_t ctx_mu = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t ctx_cv = PTHREAD_COND_INITIALIZER;
static void *ctx_thread(void *p) {
    ThreadCtx *t = (ThreadCtx *)p;
    pthread_mutex_lock(&ctx_mu);
    while (!t->run) pthread_cond_wait(&ctx_cv, &ctx_mu);
    pthread_mutex_unlock(&ctx_mu);
    t->fn(NULL); /* a context never returns: the run ends through stop() */
    return NULL;
}
static void *ctx_main_handle(void) {
    ThreadCtx *t = (ThreadCtx *)calloc(1, sizeof(ThreadCtx));
    t->run = 1;
    return t;
}
static void *ctx_make(void (*fn)(void *)) {
    ThreadCtx *t = (ThreadCtx *)calloc(1, sizeof(ThreadCtx));
    pthread_t th;
    pthread_attr_t a;
    t->fn = fn;
    pthread_attr_init(&a);
    pthread_attr_setstacksize(&a, CTX_STACK);
    pthread_attr_setdetachstate(&a, PTHREAD_CREATE_DETACHED);
    if (pthread_create(&th, &a, ctx_thread, t)) {
        printf("cannot start the thread of an execution context\n");
        exit(13);
    }
    pthread_attr_destroy(&a);
    return t;
}
static void ctx_jump(Ctx *from, Ctx *to) {
    ThreadCtx *f = (ThreadCtx *)from->handle, *t = (ThreadCtx *)to->handle;
    pthread_mutex_lock(&ctx_mu);
    f->run = 0;
    t->run = 1;
    pthread_cond_broadcast(&ctx_cv);
    while (!f->run) pthread_cond_wait(&ctx_cv, &ctx_mu);
    pthread_mutex_unlock(&ctx_mu);
}
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
#ifdef SNES_RT_TRACE
    printf("last block %06X A=%04X X=%04X Y=%04X DB=%02X\n", (unsigned)rt_block, c->a, c->x, c->y, c->db);
#endif
    stop(14, "return to an address no call expects");
}

/* ---- The code of the game ---- */

/* The dispatch by address into the generated code: snes_call_entry when it is linked in one piece, else what the
 * module of the game registers. */
#ifdef SNES_RT_LIBRARY
static int (*entry_fn)(Cpu *c, uint32_t addr);
#else
static int (*entry_fn)(Cpu *c, uint32_t addr) = snes_call_entry;
#endif

void snes_rt_register_entry(int (*fn)(Cpu *c, uint32_t addr)) { entry_fn = fn; }

#ifdef SNES_RT_LIBRARY
/* What the code of the game may call in the runtime (every function snes_rt.h declares that is not inline), by name,
 * for a host that compiles that code itself and links it by hand (libtcc's tcc_add_symbol), without looking the
 * names up in the loaded libraries. Ends with a NULL name. src/rtlib.rs checks it against snes_rt.h. */
typedef struct { const char *name; void *addr; } SnesRtSymbol;
const SnesRtSymbol *snes_rt_symbols(void);
#define RT_SYM(f) { #f, (void *)f }
static const SnesRtSymbol rt_symbols[] = {
    RT_SYM(bus_read8), RT_SYM(bus_write8), RT_SYM(joy1_state),
    RT_SYM(ppu_sprite_xoff), RT_SYM(ppu_sprite_xoff_clear), RT_SYM(ppu_wide), RT_SYM(ppu_wide_blank),
    RT_SYM(ppu_wide_blank_clear), RT_SYM(game_opt), RT_SYM(snes_rt_register_entry),
    RT_SYM(rt_unresolved), RT_SYM(rt_flag_mismatch), RT_SYM(rt_unsupported), RT_SYM(rt_wai), RT_SYM(rt_rti),
    RT_SYM(rt_lost_return), RT_SYM(rt_stp), RT_SYM(rt_brk), RT_SYM(rt_cop), RT_SYM(rt_wdm),
#ifdef SNES_RT_TICK
    RT_SYM(rt_poll),
#endif
#ifdef SNES_RT_TRACE
    { "rt_block", (void *)&rt_block },
#endif
    { NULL, NULL }
};
#undef RT_SYM
const SnesRtSymbol *snes_rt_symbols(void) { return rt_symbols; }
#endif

/* Calls the function at `addr` as the CPU would enter it from outside the generated code. Such a function
 * ends with RTI, which leaves c->ret as RT_NO_RET; if an RTS or RTL got here instead, nothing accepted it. */
static int call_top(Cpu *c, uint32_t addr) {
    int ok;
    c->ret = RT_NO_RET;
    ok = entry_fn(c, addr);
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
    spc_advance(1);
    if (!joy_read_done && (uint64_t)(tick_in_frame + 1) * 10 >= (uint64_t)frame_len * 9) {
        joy_read_done = 1;
        joy_auto_read();
    }
    if (++tick_in_frame >= frame_len) {
        ppu_frame_end();
        tick_in_frame = 0;
        joy_read_done = 0;
        frames++;
        apply_pokes();
        if (frame_hook) frame_hook();
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
    spc_advance(frame_len - 1 - tick_in_frame);
    tick_in_frame = frame_len - 1;
    rt_poll(c);
}

/* ---- Loading and starting ---- */

/* --wav FILE: the audio of the run as 16 bit stereo PCM at 32 kHz, streamed to the file; the sizes in the header are
 * filled in at exit. */
static FILE *wav_file;
static const char *wav_path;
static uint32_t wav_samples;

static void wav_header(uint8_t *h, uint32_t data) {
    static const uint8_t t[44] = {'R', 'I', 'F', 'F', 0, 0, 0, 0, 'W', 'A', 'V', 'E', 'f', 'm', 't', ' ', 16, 0, 0, 0, 1, 0, 2, 0,
                                  0x00, 0x7D, 0, 0, 0x00, 0xF4, 0x01, 0, 4, 0, 16, 0, 'd', 'a', 't', 'a', 0, 0, 0, 0};
    uint32_t riff = 36 + data;
    int i;
    memcpy(h, t, sizeof t);
    for (i = 0; i < 4; i++) { h[4 + i] = (uint8_t)(riff >> 8 * i); h[40 + i] = (uint8_t)(data >> 8 * i); }
}

static void wav_sample(int16_t l, int16_t r) {
    uint8_t b[4];
    b[0] = (uint8_t)l; b[1] = (uint8_t)((uint16_t)l >> 8); b[2] = (uint8_t)r; b[3] = (uint8_t)((uint16_t)r >> 8);
    fwrite(b, 1, 4, wav_file);
    wav_samples++;
}

static void wav_finish(void) {
    uint8_t h[44];
    if (!wav_file) return;
    wav_header(h, wav_samples * 4);
    fseek(wav_file, 0, SEEK_SET);
    fwrite(h, 1, sizeof h, wav_file);
    fclose(wav_file);
    wav_file = NULL;
    printf("wav: %u samples written\n", (unsigned)wav_samples);
}

static int wav_open(const char *path) {
    uint8_t h[44];
    wav_file = fopen(path, "wb");
    if (!wav_file) {
        fprintf(stderr, "error: cannot write %s\n", path);
        return 0;
    }
    wav_header(h, 0);
    fwrite(h, 1, sizeof h, wav_file);
    atexit(wav_finish);
    audio_hook = wav_sample;
    return 1;
}

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

/* Starts the CPU as after a reset and runs the ROM; the run ends through stop(). */
static int run_rom(void) {
    Cpu c;
    ctx_init();
    spc_reset();
    memset(&c, 0, sizeof c);
    c.e = 1; c.m = 1; c.xf = 1; c.s = 0x01FF; c.irq = 1; /* state right after reset */
    if (!entry_fn) {
        printf("no game module: snes_rt_register_entry was never called (snes_rt_module_init of the game)\n");
        stop(15, "no game module registered");
    }
    if (!call_top(&c, vector(0xFFFC))) {
        printf("reset vector %04X is not the entry of a function\n", (unsigned)vector(0xFFFC));
        stop(10, "interrupt vector is not a function");
    }
    stop(11, "reset returned");
    return 11;
}
