/* headless.c - runtime for recompiled 65816 code with no video and no audio.
 *
 * Provides the bus (LoROM memory map, hardware registers that remember what is written), the
 * runtime hooks, a frame clock driven by rt_tick, NMI delivery and a final report. Build it with
 * the generated C, defining SNES_RT_TICK (and SNES_RT_CHECKS to find width mismatches):
 *
 *     headless <rom> [--frames N] [--frame-blocks N]
 *
 * The ROM is read from the file at run time and is never part of the generated sources.
 * Time is not cycle accurate: a frame is a fixed number of executed blocks.
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
            case 0x2139: case 0x213A: case 0x213B: case 0x213C: case 0x213D: case 0x213E: case 0x213F:
            case 0x2137: case 0x2138:
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
                for (i = 0; i < 8; i++) if (v >> i & 1) dma_starts[i]++;
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
        if (dma_starts[i]) { printf("dma ch%d: %u\n", i, (unsigned)dma_starts[i]); any = 1; }
    }
    for (i = 0; i < 8; i++) {
        if (hdma_starts[i]) { printf("hdma ch%d: %u\n", i, (unsigned)hdma_starts[i]); any = 1; }
    }
    if (!any) printf("dma: none\n");
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
        } else if (argv[i][0] != '-' && !path) {
            path = argv[i];
        } else {
            path = NULL;
            break;
        }
    }
    if (!path || max_frames == 0 || frame_len == 0) {
        fprintf(stderr, "usage: %s <rom> [--frames N] [--frame-blocks N]\n", argv[0]);
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
