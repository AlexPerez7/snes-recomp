/* headless.c - runtime for recompiled 65816 code with no video and no audio.
 *
 * Provides the bus (LoROM memory map, hardware registers that remember what is written), the
 * runtime hooks, a frame clock driven by rt_tick, NMI delivery and a final report. Build it with
 * the generated C, defining SNES_RT_TICK (and SNES_RT_CHECKS to find width mismatches):
 *
 *     headless <rom> [--frames N] [--frame-blocks N] [--dump DIR] [--screenshot FILE.png] [--wav FILE.wav]
 *              [--trace-frames A-B] [--trace-reads] [--watch ADDR]... [--press BUTTONS@A-B]...
 *              [--poke ADDR=BYTES@N]... [--wide N]
 *              [--state-save FILE@N] [--state-load FILE@N]
 *
 * --press holds buttons of joypad 1 down during frames A to B (BUTTONS: B Y SELECT START UP DOWN LEFT RIGHT A X L R
 * joined by '+'). The pad answers the automatic read ($4218-$4219, at the start of vblank, with $4200 bit 0) and the
 * serial read ($4016). With no --press nothing is held.
 *
 * --state-save FILE@N saves a state to FILE at the first pass through the resume point of the profile ([resume]) from frame N on;
 * --state-load FILE@N loads it there. They need a game whose profile has [resume].
 *
 * --game-opt KEY=NUMBER sets an option of the C code of the game (game_opt in snes_rt.h).
 * --game-dir DIR sets the folder of the profile of the game (game_dir in snes_rt.h; by default, the folder of the ROM).
 * --wide N draws N extra columns on each side of the picture (a multiple of 8, 0 to 96; 0 is the 256x224 picture of the
 * console): the screenshot is then 256+2N pixels wide and shows what the PPU draws there (backgrounds go on through their
 * map, sprites land where they fall).
 * --frame-times FILE writes, one line per frame, the microseconds the frame took (CPU, PPU and sound together, as
 * measured by the clock of C11, timespec_get), to compare builds and devices; the time from the start to the first frame goes
 * on the first line.
 * --poke ADDR=BYTES@N writes BYTES (hex pairs) to consecutive WRAM bytes from ADDR (same forms as --watch) when
 * frame N starts, for debugging (e.g. moving Link: --poke 20=A008C007@3000).
 *
 * The ROM is read from the file at run time and is never part of the generated sources.
 * Time is not cycle accurate: a frame is a fixed number of executed blocks.
 * The IRQ is delivered at the line of $4209/$420A (vertical and combined modes; the horizontal position is
 * ignored), after the NMI of the frame. Execution contexts: an RTI whose S is not the one its handler started
 * with returns to another context (a fibre of its own), as the ALttP intro does between NMI and IRQ.
 * General purpose DMA runs for real; VRAM, CGRAM and OAM are kept and --dump writes them (with the
 * last value of $2100-$213F) to DIR as vram.bin, cgram.bin, oam.bin, ppu_regs.bin and wram.bin. The picture is drawn line by
 * line while the frame goes by (the rows that came due are drawn with the registers as they were until each write, HDMA
 * included), and --screenshot writes the last complete one (modes 1 and 7 and sprites only) as a 256x224 PNG (wider with --wide); windows
 * ($2123-$212B, $212E/$212F, colour window rules of $2130) and the BG mosaic ($2106) are applied.
 *
 * Exit codes: 0 frames reached, 2 bad usage or ROM, 5 unresolved jump, 6 width mismatch,
 * 7 STP/BRK/COP, 9 unsupported function, 10 interrupt vector is not a function, 11 reset returned, 12 an execution context returned,
 * 13 too many contexts or nested interrupts, 14 a return to an address no call expects, 15 no game module registered
 * (library mode only).
 *
 * Library mode: compiled with SNES_RT_LIBRARY (see snes_core.inc), main is called snes_headless_main and the host
 * calls the snes_rt_module_init of the game's module before it.
 */
#include "snes_core.inc"
#include <time.h>

/* ---- Time per frame (--frame-times) ---- */

static FILE *frame_times;
static double frame_last;

static double now_us(void) {
    struct timespec t;
    timespec_get(&t, TIME_UTC);
    return (double)t.tv_sec * 1e6 + (double)t.tv_nsec / 1e3;
}

static void time_frame(void) {
    double t = now_us();
    fprintf(frame_times, "%.0f\n", t - frame_last);
    frame_last = t;
}

void snes_set_window_icon(const unsigned *argb, int w, int h) { (void)argb; (void)w; (void)h; } /* no window */

/* ---- Entry point ---- */

#ifdef SNES_RT_LIBRARY
int snes_headless_main(int argc, char **argv);
#define main snes_headless_main
#endif

int main(int argc, char **argv) {
    const char *path = NULL;
    int i;
    for (i = 1; i < argc; i++) {
        if (!strcmp(argv[i], "--frames") && i + 1 < argc) {
            max_frames = (uint32_t)strtoul(argv[++i], NULL, 10);
        } else if (!strcmp(argv[i], "--frame-blocks") && i + 1 < argc) {
            frame_len = (uint32_t)strtoul(argv[++i], NULL, 10);
        } else if (!strcmp(argv[i], "--screenshot") && i + 1 < argc) {
            shot_path = argv[++i];
        } else if (!strcmp(argv[i], "--wav") && i + 1 < argc) {
            wav_path = argv[++i];
        } else if (!strcmp(argv[i], "--trace-frames") && i + 1 < argc) {
            char *end;
            trace_lo = (uint32_t)strtoul(argv[++i], &end, 10);
            if (*end != '-') { path = NULL; break; }
            trace_hi = (uint32_t)strtoul(end + 1, &end, 10);
            if (*end || trace_hi < trace_lo) { path = NULL; break; }
            trace_window = 1;
        } else if (!strcmp(argv[i], "--press") && i + 1 < argc) {
            if (!parse_press(argv[++i])) { path = NULL; break; }
        } else if (!strcmp(argv[i], "--trace-reads")) {
            trace_reads_on = 1;
        } else if (!strcmp(argv[i], "--watch") && i + 1 < argc) {
            char *end;
            uint32_t v;
            if (!parse_wram_addr(argv[++i], &end, &v) || *end) { path = NULL; break; }
            watch[v] = 1;
            watch_any = 1;
        } else if (!strcmp(argv[i], "--poke") && i + 1 < argc) {
            if (!parse_poke(argv[++i])) { path = NULL; break; }
        } else if (!strcmp(argv[i], "--state-save") && i + 1 < argc) {
            if (!parse_state_script(argv[++i], 1)) { path = NULL; break; }
        } else if (!strcmp(argv[i], "--state-load") && i + 1 < argc) {
            if (!parse_state_script(argv[++i], 0)) { path = NULL; break; }
        } else if (!strcmp(argv[i], "--game-opt") && i + 1 < argc) {
            if (!parse_game_opt(argv[++i])) { path = NULL; break; }
        } else if (!strcmp(argv[i], "--wide") && i + 1 < argc) {
            char *end;
            unsigned long e = strtoul(argv[++i], &end, 10);
            if (*end || !*argv[i] || e > WIDE_MAX || e % 8) {
                fprintf(stderr, "error: --wide takes a multiple of 8 from 0 to %d\n", WIDE_MAX);
                return 2;
            }
            ppu_set_wide((int)e);
        } else if (!strcmp(argv[i], "--frame-times") && i + 1 < argc) {
            if (!(frame_times = fopen(argv[++i], "w"))) {
                fprintf(stderr, "error: cannot write %s\n", argv[i]);
                return 2;
            }
        } else if (!strcmp(argv[i], "--game-dir") && i + 1 < argc) {
            set_game_dir(argv[++i]);
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
        fprintf(stderr, "usage: %s <rom> [--frames N] [--frame-blocks N] [--dump DIR] [--screenshot FILE] [--wav FILE] [--wide N]\n"
                        "       [--trace-frames A-B] [--trace-reads] [--watch ADDR]...\n"
                        "       [--press BUTTONS@A-B]... [--poke ADDR=BYTES@N]... [--game-opt KEY=NUMBER]... [--game-dir DIR]\n"
                        "       [--frame-times FILE]\n", argv[0]);
        return 2;
    }
    if (!load_rom(path)) return 2;
    if (wav_path && !wav_open(wav_path)) return 2;
    if (frame_times) {
        frame_hook = time_frame;
        frame_last = now_us();
    }
    return run_rom();
}
