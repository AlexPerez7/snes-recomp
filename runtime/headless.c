/* headless.c - runtime for recompiled 65816 code with no video and no audio.
 *
 * Provides the bus (LoROM memory map, hardware registers that remember what is written), the
 * runtime hooks, a frame clock driven by rt_tick, NMI delivery and a final report. Build it with
 * the generated C, defining SNES_RT_TICK (and SNES_RT_CHECKS to find width mismatches):
 *
 *     headless <rom> [--frames N] [--frame-blocks N] [--dump DIR]
 *
 * The ROM is read from the file at run time and is never part of the generated sources.
 * Time is not cycle accurate: a frame is a fixed number of executed blocks.
 * General purpose DMA runs for real; VRAM, CGRAM and OAM are kept and --dump writes them (with the
 * last value of $2100-$213F) to DIR as vram.bin, cgram.bin, oam.bin and ppu_regs.bin.
 *
 * Exit codes: 0 frames reached, 2 bad usage or ROM, 5 unresolved jump, 6 width mismatch,
 * 7 STP/BRK/COP, 9 unsupported function, 10 interrupt vector is not a function, 11 reset returned.
 */
#include "snes_funcs.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

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

static void report(const char *cause);

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

uint8_t bus_read8(uint32_t a) {
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
                return 0;
            case 0x4212:
                /* Vertical blank during the last tenth of the frame; auto joypad read never busy. */
                return (uint8_t)((uint64_t)tick_in_frame * 10 >= (uint64_t)frame_len * 9 ? 0x80 : 0);
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
    for (i = 0; i < count; i++) {
        uint32_t baddr = 0x2100 + ((b + pattern[mode][i % unit[mode]]) & 0xFF);
        uint32_t aaddr = (uint32_t)bank << 16 | a;
        if (ctl & 0x80) bus_write8(aaddr, bus_read8(baddr));
        else bus_write8(baddr, bus_read8(aaddr));
        a = (a + (uint32_t)step) & 0xFFFF;
    }
    hw[base + 2] = (uint8_t)a;
    hw[base + 3] = (uint8_t)(a >> 8);
    hw[base + 5] = 0;
    hw[base + 6] = 0;
    dma_starts[ch]++;
    dma_bytes[ch] += count;
}

void bus_write8(uint32_t a, uint8_t v) {
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
            case 0x2115:
                if (v & 0x0C) vram_translate = v;
                break;
            case 0x2116: vram_addr = (vram_addr & 0x7F00) | v; vram_prefetch = vram_word(); break;
            case 0x2117: vram_addr = (vram_addr & 0x00FF) | (uint32_t)(v & 0x7F) << 8; vram_prefetch = vram_word(); break;
            case 0x2118: vram_write(0, v); break;
            case 0x2119: vram_write(1, v); break;
            case 0x2121: cg_index = v; cg_write_half = 0; cg_read_half = 0; break;
            case 0x2122: cgram_write(v); break;
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
    printf("frames: %u\nnmis: %u\nblocks: %llu\n", (unsigned)frames, (unsigned)nmis, (unsigned long long)blocks);
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
}

static void stop(int code, const char *cause) {
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

/* Enter the NMI handler as the CPU does: push the (unused) return address and the flags. */
static void deliver_nmi(Cpu *c) {
    uint16_t v = vector(c->e ? 0xFFFA : 0xFFEA);
    in_nmi = 1;
    nmi_pending = 0;
    nmis++;
    if (!c->e) rt_push8(c, 0x00);
    rt_push16(c, 0x0000);
    rt_push8(c, rt_get_p(c));
    c->irq = 1;
    c->dec = 0;
    /* The recompiler analyses a handler as entered with 8-bit M and X (its config entry) and the
     * handler sets the widths it needs first thing. The flags pushed above keep the real widths, and
     * the registers keep their high bytes, so the RTI gives everything back as it was. */
    c->m = 1;
    c->xf = 1;
    if (!snes_call_entry(c, v)) {
        printf("NMI vector %04X is not the entry of a function\n", (unsigned)v);
        stop(10, "interrupt vector is not a function");
    }
    in_nmi = 0;
}

void rt_poll(Cpu *c) {
    blocks++;
    if (++tick_in_frame >= frame_len) {
        tick_in_frame = 0;
        frames++;
        nmi_flag = 1;
        if (hw[0x4200] & 0x80) nmi_pending = 1;
    }
    if (nmi_pending && !in_nmi) deliver_nmi(c);
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
        fprintf(stderr, "usage: %s <rom> [--frames N] [--frame-blocks N] [--dump DIR]\n", argv[0]);
        return 2;
    }
    if (!load_rom(path)) return 2;

    memset(&c, 0, sizeof c);
    c.e = 1; c.m = 1; c.xf = 1; c.s = 0x01FF; c.irq = 1; /* state right after reset */
    if (!snes_call_entry(&c, vector(0xFFFC))) {
        printf("reset vector %04X is not the entry of a function\n", (unsigned)vector(0xFFFC));
        stop(10, "interrupt vector is not a function");
    }
    stop(11, "reset returned");
    return 11;
}
