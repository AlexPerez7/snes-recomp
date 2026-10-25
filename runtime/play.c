/* play.c - interactive frontend of the recompiled game: a window with the picture, the keyboard as joypad 1.
 *
 * Build it with the generated C (not with snes_headless.c), SDL2 and the definition SNES_RT_TICK:
 *
 *     snes_play <rom> [--scale N] [--fullscreen] [--no-window] [--frames N] [--screenshot FILE.png]
 *               [--frame-blocks N] [--press BUTTONS@A-B]... [--poke ADDR=BYTES@N]... [--hold BUTTONS] [--print-keys] [--wav FILE.wav] [--mute]
 *
 * --wav writes the audio of the run as 16 bit stereo PCM at 32 kHz; --mute opens no audio device.
 * --print-keys lists the keyboard map (see below) and exits.
 * Keys can be changed with a file next to the ROM, `<rom without extension>.keys`, of lines `Space = A`.
 * --hold keeps BUTTONS (as in --press, joined with '+') down for the whole run as if a live source held them; the
 * keyboard adds to them, and --press adds to both.
 *
 * Keys: arrows = d-pad, X = B, Z = Y, S = A, A = X, Q = L, W = R, Enter = START, Right Shift = SELECT,
 * F11 = full screen, P = pause, Tab (held) = fast forward, F10 = save the picture as snes_NNN.png, Escape = quit.
 * --no-window runs without opening a window (for tests): it needs no display, but still SDL2.dll.
 * The ROM is read from the file at run time and is never part of the generated sources.
 */
#define SDL_MAIN_HANDLED
#include <SDL.h>
#include "snes_core.inc"

static int opt_no_window, opt_fullscreen, opt_scale = 3;
static SDL_Window *win;
static SDL_Renderer *ren;
static SDL_Texture *tex;
static int paused, fast, fullscreen, shot_n;
static uint64_t t_next, perf_freq;
static uint16_t held; /* --hold */
static int opt_print_keys, opt_stats;

#define PAD_B 0x8000
#define PAD_Y 0x4000
#define PAD_SELECT 0x2000
#define PAD_START 0x1000
#define PAD_UP 0x0800
#define PAD_DOWN 0x0400
#define PAD_LEFT 0x0200
#define PAD_RIGHT 0x0100
#define PAD_A 0x0080
#define PAD_X 0x0040
#define PAD_L 0x0020
#define PAD_R 0x0010

typedef struct { SDL_Scancode key; uint16_t button; } KeyBind;
static const KeyBind default_keys[] = {
    {SDL_SCANCODE_UP, PAD_UP}, {SDL_SCANCODE_DOWN, PAD_DOWN}, {SDL_SCANCODE_LEFT, PAD_LEFT}, {SDL_SCANCODE_RIGHT, PAD_RIGHT},
    {SDL_SCANCODE_X, PAD_B}, {SDL_SCANCODE_Z, PAD_Y}, {SDL_SCANCODE_S, PAD_A}, {SDL_SCANCODE_A, PAD_X},
    {SDL_SCANCODE_Q, PAD_L}, {SDL_SCANCODE_W, PAD_R}, {SDL_SCANCODE_RETURN, PAD_START}, {SDL_SCANCODE_RSHIFT, PAD_SELECT},
};
#define MAX_BINDS 64
static KeyBind binds[MAX_BINDS];
static int nbinds;

static int parse_buttons(const char *arg);

static const char *button_name(uint16_t b) {
    int i;
    for (i = 0; i < JOY_NAMES; i++) if ((0x8000 >> i) == b) return joy_name[i];
    return "?";
}

/* The keyboard map: the defaults, changed by a file `<rom without extension>.keys` next to the ROM with lines
 * `KEY = BUTTON` (SDL key names, e.g. `Space = A`; `#` starts a comment). A button listed in the file loses its
 * default keys. */
static char *trim(char *s) {
    size_t n;
    while (*s && isspace((unsigned char)*s)) s++;
    n = strlen(s);
    while (n > 0 && isspace((unsigned char)s[n - 1])) s[--n] = 0;
    return s;
}

/* `<rom without extension><ext>` into out; 0 if it does not fit. */
static int sibling_path(const char *rom_path, const char *ext, char *out, size_t n) {
    const char *dot = strrchr(rom_path, '.'), *slash = strrchr(rom_path, '/'), *bslash = strrchr(rom_path, 92);
    size_t base = dot && (!slash || dot > slash) && (!bslash || dot > bslash) ? (size_t)(dot - rom_path) : strlen(rom_path);
    if (base + strlen(ext) + 1 > n) return 0;
    memcpy(out, rom_path, base);
    strcpy(out + base, ext);
    return 1;
}

static void load_keys(const char *rom_path) {
    char path[1100], line[200];
    uint16_t reset_done = 0;
    FILE *f;
    nbinds = 0;
    for (; nbinds < (int)(sizeof default_keys / sizeof default_keys[0]); nbinds++) binds[nbinds] = default_keys[nbinds];
    if (!sibling_path(rom_path, ".keys", path, sizeof path)) return;
    f = fopen(path, "r");
    if (!f) return;
    while (fgets(line, sizeof line, f)) {
        char *hash = strchr(line, '#'), *eq, *key, *btn;
        int button, i, j;
        SDL_Scancode sc;
        if (hash) *hash = 0;
        eq = strchr(line, '=');
        if (!eq) continue;
        *eq = 0;
        key = trim(line);
        btn = trim(eq + 1);
        button = parse_buttons(btn);
        sc = SDL_GetScancodeFromName(key);
        if (button <= 0 || (button & (button - 1)) || sc == SDL_SCANCODE_UNKNOWN) {
            fprintf(stderr, "warning: %s: ignoring the line '%s = %s'\n", path, key, btn);
            continue;
        }
        if (!(reset_done & button)) { /* first time this button is listed: drop its default keys */
            for (i = j = 0; i < nbinds; i++) if (binds[i].button != button) binds[j++] = binds[i];
            nbinds = j;
            reset_done |= (uint16_t)button;
        }
        if (nbinds < MAX_BINDS) { binds[nbinds].key = sc; binds[nbinds].button = (uint16_t)button; nbinds++; }
    }
    fclose(f);
}

static uint16_t read_keyboard(void) {
    const Uint8 *k = SDL_GetKeyboardState(NULL);
    uint16_t m = 0;
    int i;
    for (i = 0; i < nbinds; i++) if (k[binds[i].key]) m |= binds[i].button;
    /* opposite directions do not reach the game */
    if ((m & (PAD_LEFT | PAD_RIGHT)) == (PAD_LEFT | PAD_RIGHT)) m &= (uint16_t)~(PAD_LEFT | PAD_RIGHT);
    if ((m & (PAD_UP | PAD_DOWN)) == (PAD_UP | PAD_DOWN)) m &= (uint16_t)~(PAD_UP | PAD_DOWN);
    return m;
}

/* The picture scaled by the largest whole factor that fits, centred; smaller windows get a fractional fit. */
static SDL_Rect picture_rect(int w, int h) {
    SDL_Rect r;
    int s = w / SCREEN_W < h / SCREEN_H ? w / SCREEN_W : h / SCREEN_H;
    if (s >= 1) {
        r.w = SCREEN_W * s;
        r.h = SCREEN_H * s;
    } else if (w * SCREEN_H < h * SCREEN_W) {
        r.w = w;
        r.h = w * SCREEN_H / SCREEN_W;
    } else {
        r.h = h;
        r.w = h * SCREEN_W / SCREEN_H;
    }
    r.x = (w - r.w) / 2;
    r.y = (h - r.h) / 2;
    return r;
}

static uint64_t st_present;

static void present(void) {
    int w, h;
    SDL_Rect dst;
    uint64_t p0 = SDL_GetPerformanceCounter();
    SDL_GetRendererOutputSize(ren, &w, &h);
    dst = picture_rect(w, h);
    SDL_UpdateTexture(tex, NULL, fb_front, SCREEN_W * 3);
    SDL_SetRenderDrawColor(ren, 0, 0, 0, 255);
    SDL_RenderClear(ren);
    SDL_RenderCopy(ren, tex, NULL, &dst);
    SDL_RenderPresent(ren);
    st_present += SDL_GetPerformanceCounter() - p0;
}

static void save_picture(void) {
    char name[64];
    sprintf(name, "snes_%03d.png", ++shot_n);
    if (write_png(name, fb_front, SCREEN_W, SCREEN_H)) printf("picture saved as %s\n", name);
}

/* Game controllers: the first one connected plays; others and hot-plugging are handled by the SDL events. */
static SDL_GameController *pad;

static void open_first_pad(void) {
    int i;
    if (pad) return;
    for (i = 0; i < SDL_NumJoysticks(); i++)
        if (SDL_IsGameController(i) && (pad = SDL_GameControllerOpen(i))) {
            printf("controller: %s\n", SDL_GameControllerName(pad));
            return;
        }
}

static uint16_t read_pad(void) {
    static const struct { SDL_GameControllerButton b; uint16_t m; } map[] = {
        {SDL_CONTROLLER_BUTTON_A, PAD_B}, {SDL_CONTROLLER_BUTTON_B, PAD_A}, {SDL_CONTROLLER_BUTTON_X, PAD_Y},
        {SDL_CONTROLLER_BUTTON_Y, PAD_X}, {SDL_CONTROLLER_BUTTON_LEFTSHOULDER, PAD_L}, {SDL_CONTROLLER_BUTTON_RIGHTSHOULDER, PAD_R},
        {SDL_CONTROLLER_BUTTON_BACK, PAD_SELECT}, {SDL_CONTROLLER_BUTTON_START, PAD_START},
        {SDL_CONTROLLER_BUTTON_DPAD_UP, PAD_UP}, {SDL_CONTROLLER_BUTTON_DPAD_DOWN, PAD_DOWN},
        {SDL_CONTROLLER_BUTTON_DPAD_LEFT, PAD_LEFT}, {SDL_CONTROLLER_BUTTON_DPAD_RIGHT, PAD_RIGHT},
    };
    uint16_t m = 0;
    int i, x, y;
    if (!pad) return 0;
    for (i = 0; i < (int)(sizeof map / sizeof map[0]); i++) if (SDL_GameControllerGetButton(pad, map[i].b)) m |= map[i].m;
    x = SDL_GameControllerGetAxis(pad, SDL_CONTROLLER_AXIS_LEFTX);
    y = SDL_GameControllerGetAxis(pad, SDL_CONTROLLER_AXIS_LEFTY);
    if (x < -16000) m |= PAD_LEFT; else if (x > 16000) m |= PAD_RIGHT;
    if (y < -16000) m |= PAD_UP; else if (y > 16000) m |= PAD_DOWN;
    return m;
}

static void handle_events(void) {
    SDL_Event e;
    while (SDL_PollEvent(&e)) {
        if (e.type == SDL_QUIT) stop(0, "window closed");
        if (e.type == SDL_CONTROLLERDEVICEADDED) open_first_pad();
        if (e.type == SDL_CONTROLLERDEVICEREMOVED && pad &&
            e.cdevice.which == SDL_JoystickInstanceID(SDL_GameControllerGetJoystick(pad))) {
            SDL_GameControllerClose(pad);
            pad = NULL;
            open_first_pad();
        }
        if (e.type == SDL_KEYDOWN && !e.key.repeat) {
            switch (e.key.keysym.scancode) {
            case SDL_SCANCODE_ESCAPE: stop(0, "quit"); break;
            case SDL_SCANCODE_P: paused = !paused; break;
            case SDL_SCANCODE_F10: save_picture(); break;
            case SDL_SCANCODE_F11:
                fullscreen = !fullscreen;
                SDL_SetWindowFullscreen(win, fullscreen ? SDL_WINDOW_FULLSCREEN_DESKTOP : 0);
                break;
            default: break;
            }
        }
    }
}

/* Waits until the time of the next frame (60.0988 Hz) unless fast forward is on; a late run does not try to catch up. */
static uint64_t st_wait, st_late, st_resync, st_frames, st_start;

static uint64_t st_dropped, st_queued_sum, st_underruns;
static SDL_AudioDeviceID adev;

static void print_stats(void) {
    double f = (double)perf_freq / 1000.0, total = (double)(SDL_GetPerformanceCounter() - st_start) / f;
    printf("stats: %u frames in %.0f ms (%.2f fps); present %.2f ms/frame, waiting %.2f ms/frame, %u late frames, %u resyncs\n",
           (unsigned)st_frames, total, st_frames * 1000.0 / total, st_present / f / st_frames, st_wait / f / st_frames, (unsigned)st_late, (unsigned)st_resync);
    if (adev) printf("audio: queue %.1f frames on average, %u frames dropped, %u underruns\n", st_queued_sum / 4.0 / 533.0 / st_frames, (unsigned)st_dropped, (unsigned)st_underruns);
}

static void pace(void) {
    uint64_t now = SDL_GetPerformanceCounter(), period = (uint64_t)((double)perf_freq / 60.0988), w0 = now;
    st_frames++;
    if (fast) { t_next = now; return; }
    t_next += period;
    if (now > t_next) st_late++;
    if (now > t_next + 3 * period) {
        if (opt_stats) printf("stall: frame %u, %.0f ms late\n", (unsigned)st_frames, (double)(now - t_next) * 1000.0 / perf_freq);
        t_next = now;
        st_resync++;
        return;
    }
    while ((now = SDL_GetPerformanceCounter()) < t_next) {
        uint64_t left_ms = (t_next - now) * 1000 / perf_freq;
        if (left_ms > 2) SDL_Delay((Uint32)(left_ms - 1));
    }
    st_wait += SDL_GetPerformanceCounter() - w0;
}

/* BUTTONS ("START+RIGHT") as a mask, or -1 if a name is wrong. */
static int parse_buttons(const char *arg) {
    char buf[128], *tok;
    int mask = 0;
    if (strlen(arg) >= sizeof buf) return -1;
    strcpy(buf, arg);
    for (tok = strtok(buf, "+"); tok; tok = strtok(NULL, "+")) {
        int i;
        for (i = 0; i < JOY_NAMES && strcmp(tok, joy_name[i]); i++) {}
        if (i == JOY_NAMES) return -1;
        mask |= 0x8000 >> i;
    }
    return mask;
}

/* Saved games: the cartridge SRAM lives in `<rom without extension>.srm`. It is read at start (a file of another size
 * is ignored with a warning) and written when it changed (checked once per second of game) and at exit, through a
 * temporary file that replaces the old one. */
static char srm_path[1100];
static uint8_t sram_saved[sizeof sram];
static uint32_t sram_len;
static int srm_frames;

static void srm_save(void) {
    char tmp[1110];
    FILE *f;
    if (!srm_path[0] || !memcmp(sram, sram_saved, sram_len)) return;
    snprintf(tmp, sizeof tmp, "%s.tmp", srm_path);
    f = fopen(tmp, "wb");
    if (!f || fwrite(sram, 1, sram_len, f) != sram_len || fclose(f) != 0) {
        fprintf(stderr, "warning: cannot write %s\n", tmp);
        return;
    }
#ifdef _WIN32
    if (!MoveFileExA(tmp, srm_path, MOVEFILE_REPLACE_EXISTING)) {
#else
    if (rename(tmp, srm_path) != 0) {
#endif
        fprintf(stderr, "warning: cannot replace %s\n", srm_path);
        return;
    }
    memcpy(sram_saved, sram, sram_len);
    printf("sram: saved %s (%u bytes)\n", srm_path, (unsigned)sram_len);
}

static void srm_load(const char *rom_path) {
    FILE *f;
    long n;
    sram_len = sram_mask + 1;
    if (!sibling_path(rom_path, ".srm", srm_path, sizeof srm_path)) { srm_path[0] = 0; return; }
    atexit(srm_save);
    f = fopen(srm_path, "rb");
    if (!f) return;
    fseek(f, 0, SEEK_END);
    n = ftell(f);
    rewind(f);
    if (n != (long)sram_len) {
        fprintf(stderr, "warning: %s has %ld bytes, the cartridge SRAM has %u: ignoring it\n", srm_path, n, (unsigned)sram_len);
    } else if (fread(sram, 1, sram_len, f) == sram_len) {
        memcpy(sram_saved, sram, sram_len);
        printf("sram: loaded %s\n", srm_path);
    }
    fclose(f);
}

/* Sound: the DSP makes 32 kHz stereo samples while the frame runs; at the end of each frame they are queued to the
 * audio device. The picture sets the pace, so the queue is kept short: if the device falls behind by more than about
 * 5 frames (83 ms) the new samples are dropped, and fast forward and pause empty it. */
static int16_t afr[4096 * 2];
static int nafr;
static int opt_mute;

static void play_audio(int16_t l, int16_t r) {
    if (wav_file) wav_sample(l, r);
    if (adev && nafr < 4096) { afr[nafr * 2] = l; afr[nafr * 2 + 1] = r; nafr++; }
}

static void audio_silence(int frames) {
    static const int16_t zero[533 * 2];
    int i;
    for (i = 0; i < frames; i++) SDL_QueueAudio(adev, zero, sizeof zero);
}

static void flush_audio(void) {
    Uint32 queued;
    if (!adev) return;
    queued = SDL_GetQueuedAudioSize(adev);
    st_queued_sum += queued;
    if (queued == 0) st_underruns++;
    if (fast || paused) SDL_ClearQueuedAudio(adev);
    else if (queued < 5 * 533 * 4) SDL_QueueAudio(adev, afr, (Uint32)nafr * 4);
    else st_dropped++;
    nafr = 0;
}

static void open_audio(void) {
    SDL_AudioSpec want, have;
    if (opt_mute) return;
    if (SDL_InitSubSystem(SDL_INIT_AUDIO) != 0) {
        fprintf(stderr, "warning: no audio: %s\n", SDL_GetError());
        return;
    }
    SDL_zero(want);
    want.freq = 32000;
    want.format = AUDIO_S16SYS;
    want.channels = 2;
    want.samples = 512;
    adev = SDL_OpenAudioDevice(NULL, 0, &want, &have, 0);
    if (!adev) {
        fprintf(stderr, "warning: no audio: %s\n", SDL_GetError());
        return;
    }
    audio_silence(2);
    SDL_PauseAudioDevice(adev, 0);
}

static void play_frame(void) {
    if (++srm_frames >= 60) { srm_frames = 0; srm_save(); }
    if (opt_no_window) {
        live_pad = held;
        return;
    }
    present();
    handle_events();
    fast = SDL_GetKeyboardState(NULL)[SDL_SCANCODE_TAB] != 0;
    flush_audio();
    while (paused) {
        SDL_Delay(20);
        handle_events();
        t_next = SDL_GetPerformanceCounter();
        if (!paused && adev) audio_silence(2);
    }
    live_pad = read_keyboard() | read_pad() | held;
    pace();
}

static int open_window(void) {
    char title[64];
    int i;
    if (SDL_Init(SDL_INIT_VIDEO | SDL_INIT_GAMECONTROLLER) != 0) {
        fprintf(stderr, "error: SDL_Init: %s\n", SDL_GetError());
        return 0;
    }
    for (i = 0; i < 21; i++) title[i] = rom_size > 0x7FD5 && rom[0x7FC0 + i] >= 32 && rom[0x7FC0 + i] < 127 ? (char)rom[0x7FC0 + i] : ' ';
    title[21] = 0;
    win = SDL_CreateWindow(title, SDL_WINDOWPOS_CENTERED, SDL_WINDOWPOS_CENTERED, SCREEN_W * opt_scale, SCREEN_H * opt_scale,
                           SDL_WINDOW_RESIZABLE | (opt_fullscreen ? SDL_WINDOW_FULLSCREEN_DESKTOP : 0));
    fullscreen = opt_fullscreen;
    SDL_SetHint(SDL_HINT_RENDER_SCALE_QUALITY, "nearest");
    if (!win || !(ren = SDL_CreateRenderer(win, -1, SDL_RENDERER_ACCELERATED)) ||
        !(tex = SDL_CreateTexture(ren, SDL_PIXELFORMAT_RGB24, SDL_TEXTUREACCESS_STREAMING, SCREEN_W, SCREEN_H))) {
        fprintf(stderr, "error: cannot open the window: %s\n", SDL_GetError());
        return 0;
    }
    perf_freq = SDL_GetPerformanceFrequency();
    t_next = st_start = SDL_GetPerformanceCounter();
    if (opt_stats) atexit(print_stats);
    return 1;
}

int main(int argc, char **argv) {
    const char *path = NULL;
    int i;
    max_frames = 0xFFFFFFFFu;
    for (i = 1; i < argc; i++) {
        if (!strcmp(argv[i], "--frames") && i + 1 < argc) max_frames = (uint32_t)strtoul(argv[++i], NULL, 10);
        else if (!strcmp(argv[i], "--frame-blocks") && i + 1 < argc) frame_len = (uint32_t)strtoul(argv[++i], NULL, 10);
        else if (!strcmp(argv[i], "--screenshot") && i + 1 < argc) shot_path = argv[++i];
        else if (!strcmp(argv[i], "--wav") && i + 1 < argc) wav_path = argv[++i];
        else if (!strcmp(argv[i], "--mute")) opt_mute = 1;
        else if (!strcmp(argv[i], "--stats")) opt_stats = 1;
        else if (!strcmp(argv[i], "--scale") && i + 1 < argc) opt_scale = atoi(argv[++i]);
        else if (!strcmp(argv[i], "--fullscreen")) opt_fullscreen = 1;
        else if (!strcmp(argv[i], "--print-keys")) opt_print_keys = 1;
        else if (!strcmp(argv[i], "--no-window")) opt_no_window = 1;
        else if (!strcmp(argv[i], "--press") && i + 1 < argc) { if (!parse_press(argv[++i])) { path = NULL; break; } }
        else if (!strcmp(argv[i], "--hold") && i + 1 < argc) { int m = parse_buttons(argv[++i]); if (m < 0) { path = NULL; break; } held |= (uint16_t)m; }
        else if (!strcmp(argv[i], "--poke") && i + 1 < argc) { if (!parse_poke(argv[++i])) { path = NULL; break; } }
        else if (argv[i][0] != '-' && !path) path = argv[i];
        else { path = NULL; break; }
    }
    if (!path || max_frames == 0 || frame_len == 0 || opt_scale < 1) {
        fprintf(stderr, "usage: %s <rom> [--scale N] [--fullscreen] [--no-window] [--frames N] [--screenshot FILE]\n"
                        "       [--wav FILE] [--frame-blocks N] [--press BUTTONS@A-B]... [--poke ADDR=BYTES@N]...\n", argv[0]);
        return 2;
    }
    if (!load_rom(path)) return 2;
    if (wav_path && !wav_open(wav_path)) return 2;
    SDL_SetMainReady();
    load_keys(path);
    srm_load(path);
    if (opt_print_keys) {
        for (i = 0; i < nbinds; i++) printf("%s = %s\n", SDL_GetScancodeName(binds[i].key), button_name(binds[i].button));
        return 0;
    }
    if (!opt_no_window && !open_window()) return 2;
    if (!opt_no_window) {
        quiet_exit = !opt_stats && !shot_path;
        open_audio();
        audio_hook = play_audio;
        t_next = st_start = SDL_GetPerformanceCounter(); /* opening the device can take a while */
    }
    frame_hook = play_frame;
    return run_rom();
}
