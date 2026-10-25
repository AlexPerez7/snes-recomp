/* play.c - interactive frontend of the recompiled game: a window with the picture, keyboard and controllers as the
 * two joypads, sound, saved games and the options of a PC port.
 *
 * Build it with the generated C (not with snes_headless.c), SDL2 and the definition SNES_RT_TICK:
 *
 *     snes_play [<rom>] [--scale N] [--fullscreen] [--no-window] [--frames N] [--screenshot FILE.png]
 *               [--frame-blocks N] [--press BUTTONS@A-B]... [--poke ADDR=BYTES@N]... [--hold BUTTONS] [--print-keys]
 *               [--wav FILE.wav] [--mute]
 *
 * Without a ROM on Windows it asks for one with the system's file dialog.
 * --wav writes the audio of the run as 16 bit stereo PCM at 32 kHz; --mute opens no audio device.
 * --hold keeps BUTTONS (as in --press, joined with '+') down for the whole run as if a live source held them; the
 * keyboard adds to them, and --press adds to both.
 *
 * Options: `config.ini` next to the ROM holds them (it is created with the defaults at the first run, keeps the
 * comments and keys it does not know, and is read again at each start; see play_cfg.h for the keys). The command
 * line wins over it for that run without changing it. F1 opens a menu to change the options (image, audio, controls
 * of each player); they apply at once and are saved when the menu closes. The old `<rom>.keys` file is read once, when
 * there is no config.ini yet, and its keys go into it.
 * --print-keys lists the keyboard map of player 1 and exits.
 *
 * Keys (defaults): arrows = d-pad, X = B, Z = Y, S = A, A = X, Q = L, W = R, Enter = START, Right Shift = SELECT,
 * F1 = menu, F11 = full screen, P = pause, Tab (held) = fast forward, F10 = save the picture as snes_NNN.png,
 * Escape = quit. Controllers: the first one connected is player 1, the second player 2.
 * --no-window runs without opening a window (for tests): it needs no display, but still SDL2.dll. For tests of the
 * menu there are also --menu-keys NAME,NAME,... (SDL key names pressed one per frame from frame 3, as if typed),
 * --menu-shot FILE.png and --menu-page N (the menu drawn over the last picture) and --print-config.
 * The ROM is read from the file at run time and is never part of the generated sources.
 */
#define SDL_MAIN_HANDLED
#include <SDL.h>
#include "snes_core.inc"
#include "play_cfg.h"
#ifdef _WIN32
#include <windows.h>
#ifndef __TINYC__ /* TinyCC has no commdlg.h: with it the program takes the ROM from the command line only */
#include <commdlg.h>
#pragma comment(lib, "comdlg32.lib")
#define HAVE_FILE_DIALOG
#endif
#endif

static int opt_no_window, opt_fullscreen, opt_scale;
static int cli_fullscreen, cli_scale, cli_set[O_COUNT], cli_val[O_COUNT];
static SDL_Window *win;
static SDL_Renderer *ren;
static SDL_Texture *tex;
static int paused, fast, fullscreen, shot_n;
static uint64_t t_next, perf_freq;
static uint16_t held; /* --hold */
static int opt_print_keys, opt_stats, opt_print_config;

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

static int parse_buttons(const char *arg);

static const char *button_name(uint16_t b) {
    int i;
    for (i = 0; i < JOY_NAMES; i++) if ((0x8000 >> i) == b) return joy_name[i];
    return "?";
}

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

/* ---- the configuration ---- */

static Config cfg;
static Ini ini;
static char cfg_path[1100];
static int cfg_writable; /* config.ini may be written: a window is open, or a menu test runs */

#define MAX_BINDS 192
static Bind binds[MAX_BINDS];
static int nbinds;

static const struct { SDL_Scancode k; uint16_t b; } default_keys[] = {
    {SDL_SCANCODE_UP, PAD_UP}, {SDL_SCANCODE_DOWN, PAD_DOWN}, {SDL_SCANCODE_LEFT, PAD_LEFT}, {SDL_SCANCODE_RIGHT, PAD_RIGHT},
    {SDL_SCANCODE_X, PAD_B}, {SDL_SCANCODE_Z, PAD_Y}, {SDL_SCANCODE_S, PAD_A}, {SDL_SCANCODE_A, PAD_X},
    {SDL_SCANCODE_Q, PAD_L}, {SDL_SCANCODE_W, PAD_R}, {SDL_SCANCODE_RETURN, PAD_START}, {SDL_SCANCODE_RSHIFT, PAD_SELECT},
};
static const struct { SDL_GameControllerButton c; uint16_t b; } default_pad[] = {
    {SDL_CONTROLLER_BUTTON_A, PAD_B}, {SDL_CONTROLLER_BUTTON_B, PAD_A}, {SDL_CONTROLLER_BUTTON_X, PAD_Y},
    {SDL_CONTROLLER_BUTTON_Y, PAD_X}, {SDL_CONTROLLER_BUTTON_LEFTSHOULDER, PAD_L}, {SDL_CONTROLLER_BUTTON_RIGHTSHOULDER, PAD_R},
    {SDL_CONTROLLER_BUTTON_BACK, PAD_SELECT}, {SDL_CONTROLLER_BUTTON_START, PAD_START},
    {SDL_CONTROLLER_BUTTON_DPAD_UP, PAD_UP}, {SDL_CONTROLLER_BUTTON_DPAD_DOWN, PAD_DOWN},
    {SDL_CONTROLLER_BUTTON_DPAD_LEFT, PAD_LEFT}, {SDL_CONTROLLER_BUTTON_DPAD_RIGHT, PAD_RIGHT},
};

static void bind_add(int dev, int code, int player, int button) {
    if (nbinds < MAX_BINDS) { binds[nbinds].dev = dev; binds[nbinds].code = code; binds[nbinds].player = player; binds[nbinds].button = button; nbinds++; }
}

static void bind_defaults(void) {
    int i, p;
    nbinds = 0;
    for (i = 0; i < (int)(sizeof default_keys / sizeof default_keys[0]); i++) bind_add(BIND_KEY, default_keys[i].k, 0, default_keys[i].b);
    for (p = 0; p < 2; p++)
        for (i = 0; i < (int)(sizeof default_pad / sizeof default_pad[0]); i++) bind_add(BIND_PAD, default_pad[i].c, p, default_pad[i].b);
}

static void bind_clear(int dev, int player, int button) {
    int i, j;
    for (i = j = 0; i < nbinds; i++) if (!(binds[i].dev == dev && binds[i].player == player && binds[i].button == button)) binds[j++] = binds[i];
    nbinds = j;
}

static void bind_name(char *out, size_t n, int dev, int code) {
    snprintf(out, n, "%s", dev == BIND_KEY ? SDL_GetScancodeName((SDL_Scancode)code) : SDL_GameControllerGetStringForButton((SDL_GameControllerButton)code));
}


static int bind_code(int dev, const char *name) {
    return dev == BIND_KEY ? (int)SDL_GetScancodeFromName(name) : (int)SDL_GameControllerGetButtonFromString(name);
}

static int bind_code_ok(int dev, int code) {
    return dev == BIND_KEY ? code != (int)SDL_SCANCODE_UNKNOWN : code != (int)SDL_CONTROLLER_BUTTON_INVALID;
}

/* The keys and controller buttons of the INI (`p1.key.A = Space, Z`, `p2.pad.B = leftshoulder`) over the defaults. A
 * button whose list has no valid name keeps its defaults (with a warning); an empty list leaves it without any. */
static void bindings_load(const Ini *in) {
    char key[40], val[256];
    int p, b, dev;
    for (p = 0; p < 2; p++) {
        for (b = 0; b < 12; b++) {
            for (dev = 0; dev < 2; dev++) {
                int ids[16], nid = 0, bad = 0, i;
                char *tok;
                snprintf(key, sizeof key, "p%d.%s.%s", p + 1, dev == BIND_KEY ? "key" : "pad", btn_names[b]);
                if (!ini_get(in, key, val, sizeof val)) continue;
                for (tok = strtok(val, ","); tok; tok = strtok(NULL, ",")) {
                    int code;
                    tok = trim(tok);
                    if (!*tok) continue;
                    code = bind_code(dev, tok);
                    if (!bind_code_ok(dev, code)) {
                        fprintf(stderr, "warning: config: %s: ignoring the unknown name '%s'\n", key, tok);
                        bad++;
                    } else if (nid < 16) {
                        ids[nid++] = code;
                    }
                }
                if (nid == 0 && bad) continue;
                bind_clear(dev, p, 0x8000 >> b);
                for (i = 0; i < nid; i++) bind_add(dev, ids[i], p, 0x8000 >> b);
            }
        }
    }
}

/* The key map into the INI, one line per player, device and button. */
static void bindings_store(Ini *out) {
    char key[40], val[256], name[64];
    int p, b, dev, i;
    for (p = 0; p < 2; p++)
        for (b = 0; b < 12; b++)
            for (dev = 0; dev < 2; dev++) {
                val[0] = 0;
                for (i = 0; i < nbinds; i++) {
                    if (binds[i].dev != dev || binds[i].player != p || binds[i].button != (0x8000 >> b)) continue;
                    bind_name(name, sizeof name, dev, binds[i].code);
                    if (strlen(val) + strlen(name) + 3 >= sizeof val) continue;
                    if (val[0]) strcat(val, ", ");
                    strcat(val, name);
                }
                snprintf(key, sizeof key, "p%d.%s.%s", p + 1, dev == BIND_KEY ? "key" : "pad", btn_names[b]);
                ini_set(out, key, val);
            }
}

/* Writes config.ini: the options of the file (not what the command line changed) and the key map. */
static void config_save(void) {
    if (!cfg_writable || !cfg_path[0]) return;
    cfg_store(&cfg.file, &ini);
    bindings_store(&ini);
    if (!ini_save(&ini, cfg_path)) fprintf(stderr, "warning: cannot write %s\n", cfg_path);
}

/* Reads config.ini next to the ROM (or the old `<rom>.keys` when there is no config.ini yet) and creates it with the
 * defaults on the first run of a window. */
static void config_load(const char *rom_path) {
    Settings file;
    char keys_path[1100];
    const char *sep = strrchr(rom_path, '/'), *bsep = strrchr(rom_path, 92);
    size_t dir;
    int existed, o;
    if (bsep && (!sep || bsep > sep)) sep = bsep;
    dir = sep ? (size_t)(sep - rom_path + 1) : 0;
    cfg_path[0] = 0;
    if (dir + 11 <= sizeof cfg_path) { memcpy(cfg_path, rom_path, dir); strcpy(cfg_path + dir, "config.ini"); }
    existed = cfg_path[0] && ini_load(&ini, cfg_path);
    if (!existed) {
        ini.n = 0;
        ini_add(&ini, "# Options of snes_play. Edit by hand or from the menu (F1); unknown lines are kept.");
        ini_add(&ini, "# aspect: 4:3 | 8:7 | free     scaling: integer | fit     filter: nearest | bilinear | sharp");
        ini_add(&ini, "# overscan: lines cut at the top and at the bottom (0-8)     scanlines: 0-100     volume: 0-100");
        ini_add(&ini, "# p1.key.A = Space, Z  (SDL key names)     p1.pad.A = a  (SDL controller button names)");
        if (sibling_path(rom_path, ".keys", keys_path, sizeof keys_path) && keys_migrate(&ini, keys_path) >= 0)
            fprintf(stderr, "config: imported %s\n", keys_path);
    }
    cfg_apply(&file, &ini);
    config_init(&cfg, &file);
    if (cli_fullscreen) config_override(&cfg, O_FULLSCREEN, 1);
    if (cli_scale) config_override(&cfg, O_SCALE, opt_scale);
    for (o = 0; o < O_COUNT; o++) if (cli_set[o]) config_override(&cfg, o, cli_val[o]);
    bind_defaults();
    bindings_load(&ini);
    if (!existed && cfg_writable) config_save();
}

/* ---- input ---- */

static uint16_t read_keyboard(int player) {
    const Uint8 *k = SDL_GetKeyboardState(NULL);
    uint16_t m = 0;
    int i;
    for (i = 0; i < nbinds; i++) if (binds[i].dev == BIND_KEY && binds[i].player == player && k[binds[i].code]) m |= (uint16_t)binds[i].button;
    /* opposite directions do not reach the game */
    if ((m & (PAD_LEFT | PAD_RIGHT)) == (PAD_LEFT | PAD_RIGHT)) m &= (uint16_t)~(PAD_LEFT | PAD_RIGHT);
    if ((m & (PAD_UP | PAD_DOWN)) == (PAD_UP | PAD_DOWN)) m &= (uint16_t)~(PAD_UP | PAD_DOWN);
    return m;
}

/* Game controllers: the first one connected is player 1, the second player 2; hot-plugging is handled by the SDL events. */
static SDL_GameController *pads[2];

static void open_pads(void) {
    int i, slot;
    for (i = 0; i < SDL_NumJoysticks(); i++) {
        if (!SDL_IsGameController(i) || SDL_GameControllerFromInstanceID(SDL_JoystickGetDeviceInstanceID(i))) continue;
        for (slot = 0; slot < 2 && pads[slot]; slot++) {}
        if (slot == 2) return;
        if ((pads[slot] = SDL_GameControllerOpen(i))) printf("controller %d: %s\n", slot + 1, SDL_GameControllerName(pads[slot]));
    }
}

static uint16_t read_pad(int player) {
    uint16_t m = 0;
    int i, x, y;
    SDL_GameController *pad = pads[player];
    if (!pad) return 0;
    for (i = 0; i < nbinds; i++)
        if (binds[i].dev == BIND_PAD && binds[i].player == player && SDL_GameControllerGetButton(pad, (SDL_GameControllerButton)binds[i].code)) m |= (uint16_t)binds[i].button;
    x = SDL_GameControllerGetAxis(pad, SDL_CONTROLLER_AXIS_LEFTX);
    y = SDL_GameControllerGetAxis(pad, SDL_CONTROLLER_AXIS_LEFTY);
    if (x < -16000) m |= PAD_LEFT; else if (x > 16000) m |= PAD_RIGHT;
    if (y < -16000) m |= PAD_UP; else if (y > 16000) m |= PAD_DOWN;
    return m;
}

/* ---- the picture ---- */

static uint64_t st_present;
static SDL_Texture *tex_pre, *tex_ovl; /* the integer prescale of `sharp`; the screen effects */
static int pre_w, pre_h, ovl_w, ovl_h, ovl_lines, ovl_sl = -1, ovl_grid = -1;
static uint8_t *ovl_buf;
static size_t ovl_cap;

static int view_lines(void) {
    return SCREEN_H - 2 * cfg.eff.v[O_OVERSCAN];
}

/* Draws the 256x224 picture `fb` in the window with the options: aspect, scaling, filter, overscan and effects. */
static void present(const uint8_t *fb) {
    int w, h, lines = view_lines(), filter = cfg.eff.v[O_FILTER];
    SDL_Rect src, dst;
    Rect r;
    uint64_t p0 = SDL_GetPerformanceCounter();
    SDL_GetRendererOutputSize(ren, &w, &h);
    r = view_rect(w, h, cfg.eff.v[O_ASPECT], cfg.eff.v[O_SCALING], lines);
    dst.x = r.x; dst.y = r.y; dst.w = r.w; dst.h = r.h;
    src.x = 0; src.y = cfg.eff.v[O_OVERSCAN]; src.w = SCREEN_W; src.h = lines;
    SDL_UpdateTexture(tex, NULL, fb, SCREEN_W * 3);
    SDL_SetRenderDrawColor(ren, 0, 0, 0, 255);
    SDL_RenderClear(ren);
    if (filter == FILTER_SHARP) {
        /* scale up by a whole factor with nearest, then down to the final size with bilinear: the edges of the
         * pixels stay sharp and the factor that is not whole is smoothed */
        int k = (dst.w + SCREEN_W - 1) / SCREEN_W;
        if (k < 1) k = 1;
        if (k > 8) k = 8;
        if (!tex_pre || pre_w != SCREEN_W * k || pre_h != lines * k) {
            if (tex_pre) SDL_DestroyTexture(tex_pre);
            tex_pre = SDL_CreateTexture(ren, SDL_PIXELFORMAT_RGB24, SDL_TEXTUREACCESS_TARGET, SCREEN_W * k, lines * k);
            pre_w = SCREEN_W * k;
            pre_h = lines * k;
        }
        if (tex_pre) {
            SDL_SetTextureScaleMode(tex, SDL_ScaleModeNearest);
            SDL_SetRenderTarget(ren, tex_pre);
            SDL_RenderCopy(ren, tex, &src, NULL);
            SDL_SetRenderTarget(ren, NULL);
            SDL_SetTextureScaleMode(tex_pre, SDL_ScaleModeLinear);
            SDL_RenderCopy(ren, tex_pre, NULL, &dst);
        } else {
            SDL_SetTextureScaleMode(tex, SDL_ScaleModeNearest);
            SDL_RenderCopy(ren, tex, &src, &dst);
        }
    } else {
        SDL_SetTextureScaleMode(tex, filter == FILTER_BILINEAR ? SDL_ScaleModeLinear : SDL_ScaleModeNearest);
        SDL_RenderCopy(ren, tex, &src, &dst);
    }
    if (cfg.eff.v[O_SCANLINES] > 0 || cfg.eff.v[O_GRID]) {
        int sl = cfg.eff.v[O_SCANLINES], grid = cfg.eff.v[O_GRID];
        if (!tex_ovl || ovl_w != dst.w || ovl_h != dst.h || ovl_lines != lines || ovl_sl != sl || ovl_grid != grid) {
            size_t need = (size_t)dst.w * dst.h * 4;
            if (need > ovl_cap) { free(ovl_buf); ovl_buf = (uint8_t *)malloc(need); ovl_cap = ovl_buf ? need : 0; }
            if (tex_ovl) SDL_DestroyTexture(tex_ovl);
            tex_ovl = NULL;
            if (ovl_buf && overlay_build(ovl_buf, dst.w, dst.h, SCREEN_W, lines, sl, grid)) {
                tex_ovl = SDL_CreateTexture(ren, SDL_PIXELFORMAT_RGBA32, SDL_TEXTUREACCESS_STATIC, dst.w, dst.h);
                if (tex_ovl) {
                    SDL_UpdateTexture(tex_ovl, NULL, ovl_buf, dst.w * 4);
                    SDL_SetTextureBlendMode(tex_ovl, SDL_BLENDMODE_BLEND);
                }
            }
            ovl_w = dst.w; ovl_h = dst.h; ovl_lines = lines; ovl_sl = sl; ovl_grid = grid;
        }
        if (tex_ovl) SDL_RenderCopy(ren, tex_ovl, NULL, &dst);
    }
    SDL_RenderPresent(ren);
    st_present += SDL_GetPerformanceCounter() - p0;
}

/* --present-shot: the picture as present() draws it in a window of W x H, made with the software renderer into a
 * PNG (so the layout, the filters, the overscan and the effects can be seen without a window). When W x H is the
 * size of the picture itself it also says whether the result is the picture byte for byte. */
static const char *present_shot_path;
static int present_w = 256, present_h = SCREEN_H;

static void present_shot(void) {
    SDL_Surface *surf = SDL_CreateRGBSurfaceWithFormat(0, present_w, present_h, 24, SDL_PIXELFORMAT_RGB24);
    SDL_Renderer *sr, *keep_ren = ren;
    SDL_Texture *keep_tex = tex, *keep_pre = tex_pre, *keep_ovl = tex_ovl;
    uint8_t *pix;
    int lines = view_lines();
    if (!surf || !(sr = SDL_CreateSoftwareRenderer(surf))) { fprintf(stderr, "error: present-shot: %s\n", SDL_GetError()); return; }
    ren = sr;
    tex = SDL_CreateTexture(sr, SDL_PIXELFORMAT_RGB24, SDL_TEXTUREACCESS_STREAMING, SCREEN_W, SCREEN_H);
    tex_pre = tex_ovl = NULL;
    pre_w = ovl_w = 0; ovl_sl = ovl_grid = -1;
    present(fb_front);
    pix = (uint8_t *)malloc((size_t)present_w * present_h * 3);
    if (pix && SDL_RenderReadPixels(sr, NULL, SDL_PIXELFORMAT_RGB24, pix, present_w * 3) == 0) {
        if (write_png(present_shot_path, pix, present_w, present_h)) printf("present-shot: %dx%d\n", present_w, present_h);
        if (present_w == SCREEN_W && present_h == lines) {
            size_t diff = 0, i, n = (size_t)SCREEN_W * lines * 3;
            const uint8_t *src = fb_front + (size_t)cfg.eff.v[O_OVERSCAN] * SCREEN_W * 3;
            for (i = 0; i < n; i++) diff += pix[i] != src[i];
            if (diff) printf("present-shot: differs from the picture in %u bytes\n", (unsigned)diff);
            else printf("present-shot: identical to the picture\n");
        }
    }
    free(pix);
    if (tex_pre) SDL_DestroyTexture(tex_pre);
    if (tex_ovl) SDL_DestroyTexture(tex_ovl);
    SDL_DestroyTexture(tex);
    SDL_DestroyRenderer(sr);
    SDL_FreeSurface(surf);
    ren = keep_ren; tex = keep_tex; tex_pre = keep_pre; tex_ovl = keep_ovl;
}

static void save_picture(void) {
    char name[64];
    sprintf(name, "snes_%03d.png", ++shot_n);
    if (write_png(name, fb_front, SCREEN_W, SCREEN_H)) printf("picture saved as %s\n", name);
}

static void set_fullscreen(int on) {
    fullscreen = on;
    if (win) SDL_SetWindowFullscreen(win, on ? SDL_WINDOW_FULLSCREEN_DESKTOP : 0);
}

/* ---- the menu (F1) ---- */

/* A 5x7 bitmap font of the printable ASCII characters (32 to 126), drawn by hand for this program: seven rows per
 * character, bit 4 of a row is its leftmost pixel. */
static const uint8_t font5x7[95][7] = {
    {0x00,0x00,0x00,0x00,0x00,0x00,0x00}, {0x04,0x04,0x04,0x04,0x04,0x00,0x04}, {0x0A,0x0A,0x0A,0x00,0x00,0x00,0x00}, {0x0A,0x0A,0x1F,0x0A,0x1F,0x0A,0x0A},
    {0x04,0x0F,0x14,0x0E,0x05,0x1E,0x04}, {0x18,0x19,0x02,0x04,0x08,0x13,0x03}, {0x0C,0x12,0x14,0x08,0x15,0x12,0x0D}, {0x04,0x04,0x08,0x00,0x00,0x00,0x00},
    {0x02,0x04,0x08,0x08,0x08,0x04,0x02}, {0x08,0x04,0x02,0x02,0x02,0x04,0x08}, {0x00,0x04,0x15,0x0E,0x15,0x04,0x00}, {0x00,0x04,0x04,0x1F,0x04,0x04,0x00},
    {0x00,0x00,0x00,0x00,0x0C,0x04,0x08}, {0x00,0x00,0x00,0x1F,0x00,0x00,0x00}, {0x00,0x00,0x00,0x00,0x00,0x0C,0x0C}, {0x01,0x01,0x02,0x04,0x08,0x10,0x10},
    {0x0E,0x11,0x13,0x15,0x19,0x11,0x0E}, {0x04,0x0C,0x04,0x04,0x04,0x04,0x0E}, {0x0E,0x11,0x01,0x02,0x04,0x08,0x1F}, {0x1F,0x02,0x04,0x02,0x01,0x11,0x0E},
    {0x02,0x06,0x0A,0x12,0x1F,0x02,0x02}, {0x1F,0x10,0x1E,0x01,0x01,0x11,0x0E}, {0x06,0x08,0x10,0x1E,0x11,0x11,0x0E}, {0x1F,0x01,0x02,0x04,0x08,0x08,0x08},
    {0x0E,0x11,0x11,0x0E,0x11,0x11,0x0E}, {0x0E,0x11,0x11,0x0F,0x01,0x02,0x0C}, {0x00,0x0C,0x0C,0x00,0x0C,0x0C,0x00}, {0x00,0x0C,0x0C,0x00,0x0C,0x04,0x08},
    {0x02,0x04,0x08,0x10,0x08,0x04,0x02}, {0x00,0x00,0x1F,0x00,0x1F,0x00,0x00}, {0x08,0x04,0x02,0x01,0x02,0x04,0x08}, {0x0E,0x11,0x01,0x02,0x04,0x00,0x04},
    {0x0E,0x11,0x17,0x15,0x17,0x10,0x0E}, {0x0E,0x11,0x11,0x1F,0x11,0x11,0x11}, {0x1E,0x11,0x11,0x1E,0x11,0x11,0x1E}, {0x0E,0x11,0x10,0x10,0x10,0x11,0x0E},
    {0x1C,0x12,0x11,0x11,0x11,0x12,0x1C}, {0x1F,0x10,0x10,0x1E,0x10,0x10,0x1F}, {0x1F,0x10,0x10,0x1E,0x10,0x10,0x10}, {0x0E,0x11,0x10,0x17,0x11,0x11,0x0F},
    {0x11,0x11,0x11,0x1F,0x11,0x11,0x11}, {0x0E,0x04,0x04,0x04,0x04,0x04,0x0E}, {0x07,0x02,0x02,0x02,0x02,0x12,0x0C}, {0x11,0x12,0x14,0x18,0x14,0x12,0x11},
    {0x10,0x10,0x10,0x10,0x10,0x10,0x1F}, {0x11,0x1B,0x15,0x15,0x11,0x11,0x11}, {0x11,0x11,0x19,0x15,0x13,0x11,0x11}, {0x0E,0x11,0x11,0x11,0x11,0x11,0x0E},
    {0x1E,0x11,0x11,0x1E,0x10,0x10,0x10}, {0x0E,0x11,0x11,0x11,0x15,0x12,0x0D}, {0x1E,0x11,0x11,0x1E,0x14,0x12,0x11}, {0x0F,0x10,0x10,0x0E,0x01,0x01,0x1E},
    {0x1F,0x04,0x04,0x04,0x04,0x04,0x04}, {0x11,0x11,0x11,0x11,0x11,0x11,0x0E}, {0x11,0x11,0x11,0x11,0x11,0x0A,0x04}, {0x11,0x11,0x11,0x15,0x15,0x15,0x0A},
    {0x11,0x11,0x0A,0x04,0x0A,0x11,0x11}, {0x11,0x11,0x0A,0x04,0x04,0x04,0x04}, {0x1F,0x01,0x02,0x04,0x08,0x10,0x1F}, {0x0E,0x08,0x08,0x08,0x08,0x08,0x0E},
    {0x10,0x10,0x08,0x04,0x02,0x01,0x01}, {0x0E,0x02,0x02,0x02,0x02,0x02,0x0E}, {0x04,0x0A,0x11,0x00,0x00,0x00,0x00}, {0x00,0x00,0x00,0x00,0x00,0x00,0x1F},
    {0x08,0x04,0x02,0x00,0x00,0x00,0x00}, {0x00,0x00,0x0E,0x01,0x0F,0x11,0x0F}, {0x10,0x10,0x16,0x19,0x11,0x11,0x1E}, {0x00,0x00,0x0E,0x10,0x10,0x11,0x0E},
    {0x01,0x01,0x0D,0x13,0x11,0x11,0x0F}, {0x00,0x00,0x0E,0x11,0x1F,0x10,0x0E}, {0x06,0x09,0x08,0x1C,0x08,0x08,0x08}, {0x00,0x0F,0x11,0x11,0x0F,0x01,0x0E},
    {0x10,0x10,0x16,0x19,0x11,0x11,0x11}, {0x04,0x00,0x0C,0x04,0x04,0x04,0x0E}, {0x02,0x00,0x06,0x02,0x02,0x12,0x0C}, {0x10,0x10,0x12,0x14,0x18,0x14,0x12},
    {0x0C,0x04,0x04,0x04,0x04,0x04,0x0E}, {0x00,0x00,0x1A,0x15,0x15,0x15,0x15}, {0x00,0x00,0x16,0x19,0x11,0x11,0x11}, {0x00,0x00,0x0E,0x11,0x11,0x11,0x0E},
    {0x00,0x00,0x1E,0x11,0x1E,0x10,0x10}, {0x00,0x00,0x0D,0x13,0x0F,0x01,0x01}, {0x00,0x00,0x16,0x19,0x10,0x10,0x10}, {0x00,0x00,0x0F,0x10,0x0E,0x01,0x1E},
    {0x08,0x08,0x1C,0x08,0x08,0x09,0x06}, {0x00,0x00,0x11,0x11,0x11,0x13,0x0D}, {0x00,0x00,0x11,0x11,0x11,0x0A,0x04}, {0x00,0x00,0x11,0x11,0x15,0x15,0x0A},
    {0x00,0x00,0x11,0x0A,0x04,0x0A,0x11}, {0x00,0x00,0x11,0x11,0x0F,0x01,0x0E}, {0x00,0x00,0x1F,0x02,0x04,0x08,0x1F}, {0x02,0x04,0x04,0x08,0x04,0x04,0x02},
    {0x04,0x04,0x04,0x04,0x04,0x04,0x04}, {0x08,0x04,0x04,0x02,0x04,0x04,0x08}, {0x00,0x00,0x08,0x15,0x02,0x00,0x00},
};

#define MENU_COLS 42 /* characters of 6 pixels across the 256 of the picture */
static uint8_t fb_menu[SCREEN_H * SCREEN_W * 3];

static void put_px(int x, int y, uint8_t r, uint8_t g, uint8_t b) {
    uint8_t *p;
    if (x < 0 || y < 0 || x >= SCREEN_W || y >= SCREEN_H) return;
    p = &fb_menu[(y * SCREEN_W + x) * 3];
    p[0] = r; p[1] = g; p[2] = b;
}

static void draw_text(int x, int y, const char *s, uint8_t r, uint8_t g, uint8_t b) {
    for (; *s; s++, x += 6) {
        int row, col, c = (unsigned char)*s;
        if (c < 32 || c > 126) c = '?';
        for (row = 0; row < 7; row++)
            for (col = 0; col < 5; col++)
                if (font5x7[c - 32][row] >> (4 - col) & 1) put_px(x + col, y + row, r, g, b);
    }
}

enum { PG_MAIN, PG_IMAGE, PG_AUDIO, PG_CONTROLS, PG_PLAYER, PG_COUNT };
static int menu_open, menu_page, menu_sel[PG_COUNT], menu_player, menu_capture = -1, menu_confirm;
static Bind menu_pending;
static char menu_msg[80];
static const int image_opts[] = {O_FULLSCREEN, O_VSYNC, O_ASPECT, O_SCALING, O_FILTER, O_OVERSCAN, O_SCANLINES, O_GRID};
#define N_IMAGE ((int)(sizeof image_opts / sizeof image_opts[0]))

static int menu_count(int page) {
    switch (page) {
    case PG_MAIN: return 4;
    case PG_IMAGE: return N_IMAGE + 1;
    case PG_AUDIO: return 3;
    case PG_CONTROLS: return 3;
    default: return 13;
    }
}

static const char *aud_dev_name(int i) {
    return i <= 0 ? "" : SDL_GetAudioDeviceName(i - 1, 0);
}

static int aud_dev_count(void) {
    return SDL_WasInit(SDL_INIT_AUDIO) ? SDL_GetNumAudioDevices(0) + 1 : 1;
}

static int aud_dev_index(void) {
    int i, n = aud_dev_count();
    for (i = 1; i < n; i++) if (!strcmp(aud_dev_name(i), cfg.eff.audio_device)) return i;
    return 0;
}

static SDL_AudioDeviceID adev;
static int opt_mute;
static void open_audio(void);

static void close_audio(void) {
    if (adev) { SDL_CloseAudioDevice(adev); adev = 0; }
}

static void menu_audio_device(int dir) {
    int n = aud_dev_count(), i = (aud_dev_index() + dir + n) % n;
    snprintf(cfg.eff.audio_device, sizeof cfg.eff.audio_device, "%s", aud_dev_name(i));
    snprintf(cfg.file.audio_device, sizeof cfg.file.audio_device, "%s", cfg.eff.audio_device);
    if (!opt_mute && !opt_no_window) { close_audio(); open_audio(); }
}

static void apply_option(int opt) {
    if (opt == O_FULLSCREEN) set_fullscreen(cfg.eff.v[O_FULLSCREEN]);
    if (opt == O_VSYNC && ren) SDL_RenderSetVSync(ren, cfg.eff.v[O_VSYNC]);
}

static void opt_change(int opt, int dir) {
    const OptDef *d = &opt_defs[opt];
    int v = cfg.eff.v[opt];
    if (d->names) v = (v + dir + d->hi + 1) % (d->hi + 1);
    else { v += dir * d->step; if (v > d->hi) v = d->hi; if (v < d->lo) v = d->lo; }
    config_user_set(&cfg, opt, v);
    apply_option(opt);
}

static void menu_back(void) {
    if (menu_page == PG_MAIN) { menu_open = 0; config_save(); }
    else menu_page = menu_page == PG_PLAYER ? PG_CONTROLS : PG_MAIN;
}

/* Left or right on the item (dir -1 or +1), or Enter (dir 0 for the items that open something). */
static void menu_change(int dir) {
    int s = menu_sel[menu_page];
    switch (menu_page) {
    case PG_MAIN:
        if (dir == 0) { if (s == 3) menu_back(); else menu_page = s == 0 ? PG_IMAGE : s == 1 ? PG_AUDIO : PG_CONTROLS; }
        break;
    case PG_IMAGE:
        if (s == N_IMAGE) { if (dir == 0) menu_back(); }
        else opt_change(image_opts[s], dir ? dir : 1);
        break;
    case PG_AUDIO:
        if (s == 0) opt_change(O_VOLUME, dir ? dir : 1);
        else if (s == 1) menu_audio_device(dir ? dir : 1);
        else if (dir == 0) menu_back();
        break;
    case PG_CONTROLS:
        if (dir == 0) { if (s == 2) menu_back(); else { menu_player = s; menu_page = PG_PLAYER; menu_sel[PG_PLAYER] = 0; } }
        break;
    default:
        if (dir == 0) { if (s == 12) menu_back(); else { menu_capture = s; snprintf(menu_msg, sizeof menu_msg, "Press a key or a controller button"); } }
        break;
    }
}

/* The key or controller button `code` of `dev` for the button being assigned; asks first if it is already in use. */
static void menu_assign(int dev, int code) {
    int r, mask = 0x8000 >> menu_capture;
    r = bind_assign(binds, &nbinds, MAX_BINDS, dev, code, menu_player, mask, 0);
    if (r == 1) {
        int o = bind_find(binds, nbinds, dev, code);
        menu_pending.dev = dev; menu_pending.code = code; menu_pending.player = menu_player; menu_pending.button = mask;
        menu_confirm = 1;
        snprintf(menu_msg, sizeof menu_msg, "Used by P%d %s: Enter replaces, Esc cancels", binds[o].player + 1, button_name((uint16_t)binds[o].button));
    } else {
        menu_capture = -1;
        menu_msg[0] = 0;
    }
}

static void menu_move(int d) {
    int n = menu_count(menu_page);
    menu_sel[menu_page] = (menu_sel[menu_page] + d + n) % n;
}

/* A key pressed while the menu is open, or while the menu is being opened (F1). */
static void menu_key(SDL_Scancode sc) {
    if (menu_confirm) {
        if (sc == SDL_SCANCODE_RETURN) {
            bind_assign(binds, &nbinds, MAX_BINDS, menu_pending.dev, menu_pending.code, menu_pending.player, menu_pending.button, 1);
            menu_confirm = 0; menu_capture = -1; menu_msg[0] = 0;
        } else if (sc == SDL_SCANCODE_ESCAPE) {
            menu_confirm = 0; menu_capture = -1; menu_msg[0] = 0;
        }
        return;
    }
    if (menu_capture >= 0) {
        if (sc == SDL_SCANCODE_ESCAPE) { menu_capture = -1; menu_msg[0] = 0; }
        else menu_assign(BIND_KEY, sc);
        return;
    }
    switch (sc) {
    case SDL_SCANCODE_UP: menu_move(-1); break;
    case SDL_SCANCODE_DOWN: menu_move(1); break;
    case SDL_SCANCODE_LEFT: menu_change(-1); break;
    case SDL_SCANCODE_RIGHT: menu_change(1); break;
    case SDL_SCANCODE_RETURN: menu_change(0); break;
    case SDL_SCANCODE_ESCAPE: menu_back(); break;
    case SDL_SCANCODE_F1: menu_open = 0; config_save(); break;
    default: break;
    }
}

static void menu_pad(SDL_GameControllerButton b) {
    if (menu_confirm) {
        menu_key(b == SDL_CONTROLLER_BUTTON_A ? SDL_SCANCODE_RETURN : SDL_SCANCODE_ESCAPE);
        return;
    }
    if (menu_capture >= 0) { menu_assign(BIND_PAD, b); return; }
    switch (b) {
    case SDL_CONTROLLER_BUTTON_DPAD_UP: menu_key(SDL_SCANCODE_UP); break;
    case SDL_CONTROLLER_BUTTON_DPAD_DOWN: menu_key(SDL_SCANCODE_DOWN); break;
    case SDL_CONTROLLER_BUTTON_DPAD_LEFT: menu_key(SDL_SCANCODE_LEFT); break;
    case SDL_CONTROLLER_BUTTON_DPAD_RIGHT: menu_key(SDL_SCANCODE_RIGHT); break;
    case SDL_CONTROLLER_BUTTON_A: menu_key(SDL_SCANCODE_RETURN); break;
    case SDL_CONTROLLER_BUTTON_B: menu_key(SDL_SCANCODE_ESCAPE); break;
    default: break;
    }
}

/* The keys and buttons of `button` for the menu line, e.g. "Space, Z | a". */
static void bind_text(char *out, size_t n, int player, int button) {
    char name[64];
    int i, k = 0, any;
    out[0] = 0;
    for (any = 0; any < 2; any++) {
        int first = 1;
        for (i = 0; i < nbinds; i++) {
            if (binds[i].dev != any || binds[i].player != player || binds[i].button != button) continue;
            bind_name(name, sizeof name, any, binds[i].code);
            if (strlen(out) + strlen(name) + 4 >= n) break;
            if (!first) strcat(out, ",");
            else if (any == 1 && k) strcat(out, " | ");
            strcat(out, name);
            first = 0;
        }
        if (!first) k = 1;
    }
}

/* The menu over the picture `fb`, into fb_menu. */
static void menu_draw(const uint8_t *fb) {
    static const char *titles[PG_COUNT] = {"OPTIONS", "IMAGE", "AUDIO", "CONTROLS", "PLAYER"};
    char line[96], val[64];
    int i, n = menu_count(menu_page), y = 30, s = menu_sel[menu_page];
    for (i = 0; i < SCREEN_H * SCREEN_W * 3; i++) fb_menu[i] = (uint8_t)(fb[i] / 3);
    snprintf(line, sizeof line, menu_page == PG_PLAYER ? "%s %d" : "%s", titles[menu_page], menu_player + 1);
    draw_text(8, 10, line, 255, 255, 120);
    draw_text(8, 20, "Arrows: choose, change  Enter  Esc: back", 140, 140, 140);
    for (i = 0; i < n; i++, y += 10) {
        const char *label = "", *v = "";
        int on = i == s;
        val[0] = 0;
        switch (menu_page) {
        case PG_MAIN: label = i == 0 ? "Image" : i == 1 ? "Audio" : i == 2 ? "Controls" : "Close"; break;
        case PG_IMAGE:
            if (i == N_IMAGE) { label = "Back"; break; }
            label = opt_defs[image_opts[i]].key;
            opt_format(&opt_defs[image_opts[i]], cfg.eff.v[image_opts[i]], val, sizeof val);
            v = val;
            break;
        case PG_AUDIO:
            if (i == 0) { label = "volume"; snprintf(val, sizeof val, "%d", cfg.eff.v[O_VOLUME]); v = val; }
            else if (i == 1) { label = "device"; snprintf(val, sizeof val, "%.26s", cfg.eff.audio_device[0] ? cfg.eff.audio_device : "default"); v = val; }
            else label = "Back";
            break;
        case PG_CONTROLS: label = i == 0 ? "Player 1" : i == 1 ? "Player 2" : "Back"; break;
        default:
            if (i == 12) { label = "Back"; break; }
            label = btn_names[i];
            bind_text(val, sizeof val, menu_player, 0x8000 >> i);
            v = menu_capture == i ? "..." : val;
            break;
        }
        draw_text(8, y, on ? ">" : " ", 255, 255, 255);
        draw_text(16, y, label, on ? 255 : 200, on ? 255 : 200, on ? 255 : 200);
        draw_text(96, y, v, on ? 120 : 90, on ? 255 : 200, on ? 120 : 90);
    }
    if (menu_msg[0]) draw_text(8, SCREEN_H - 14, menu_msg, 255, 140, 140);
}

/* Any key: F1 opens or closes the menu; with the menu open, the keys drive it. */
static void ui_key(SDL_Scancode sc) {
    if (menu_open) { menu_key(sc); return; }
    if (sc == SDL_SCANCODE_F1) { menu_open = 1; menu_page = PG_MAIN; menu_capture = -1; menu_confirm = 0; menu_msg[0] = 0; }
}

static void handle_events(void) {
    SDL_Event e;
    while (SDL_PollEvent(&e)) {
        if (e.type == SDL_QUIT) stop(0, "window closed");
        if (e.type == SDL_CONTROLLERDEVICEADDED) open_pads();
        if (e.type == SDL_CONTROLLERDEVICEREMOVED) {
            int i;
            for (i = 0; i < 2; i++)
                if (pads[i] && e.cdevice.which == SDL_JoystickInstanceID(SDL_GameControllerGetJoystick(pads[i]))) {
                    SDL_GameControllerClose(pads[i]);
                    pads[i] = NULL;
                }
            open_pads();
        }
        if (e.type == SDL_CONTROLLERBUTTONDOWN && menu_open) menu_pad((SDL_GameControllerButton)e.cbutton.button);
        if (e.type == SDL_KEYDOWN && !e.key.repeat) {
            SDL_Scancode sc = e.key.keysym.scancode;
            if (menu_open || sc == SDL_SCANCODE_F1) { ui_key(sc); continue; }
            switch (sc) {
            case SDL_SCANCODE_ESCAPE: stop(0, "quit"); break;
            case SDL_SCANCODE_P: paused = !paused; break;
            case SDL_SCANCODE_F10: save_picture(); break;
            case SDL_SCANCODE_F11:
                config_user_set(&cfg, O_FULLSCREEN, !fullscreen);
                set_fullscreen(!fullscreen);
                config_save();
                break;
            default: break;
            }
        }
    }
}

/* Waits until the time of the next frame (60.0988 Hz) unless fast forward is on; a late run does not try to catch up. */
static uint64_t st_wait, st_late, st_resync, st_frames, st_start;

static uint64_t st_dropped, st_queued_sum, st_underruns;

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
 * 5 frames (83 ms) the new samples are dropped, and fast forward, pause and the menu empty it. The master volume
 * scales what goes to the device (not the WAV file). */
static int16_t afr[4096 * 2];
static int nafr;

static void play_audio(int16_t l, int16_t r) {
    if (wav_file) wav_sample(l, r);
    if (adev && nafr < 4096) {
        int vol = cfg.eff.v[O_VOLUME];
        afr[nafr * 2] = vol_apply(l, vol);
        afr[nafr * 2 + 1] = vol_apply(r, vol);
        nafr++;
    }
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
    if (fast || paused || menu_open) SDL_ClearQueuedAudio(adev);
    else if (queued < 5 * 533 * 4) SDL_QueueAudio(adev, afr, (Uint32)nafr * 4);
    else st_dropped++;
    nafr = 0;
}

static void open_audio(void) {
    SDL_AudioSpec want, have;
    const char *name = cfg.eff.audio_device[0] ? cfg.eff.audio_device : NULL;
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
    adev = SDL_OpenAudioDevice(name, 0, &want, &have, 0);
    if (!adev && name) {
        fprintf(stderr, "warning: cannot open the audio device '%s' (%s): using the default one\n", name, SDL_GetError());
        adev = SDL_OpenAudioDevice(NULL, 0, &want, &have, 0);
    }
    if (!adev) {
        fprintf(stderr, "warning: no audio: %s\n", SDL_GetError());
        return;
    }
    audio_silence(2);
    SDL_PauseAudioDevice(adev, 0);
}

/* For the menu tests: SDL key names to press, one per frame from frame 3, and a picture of the menu at the end. */
static SDL_Scancode menu_script[64];
static int menu_script_n, menu_script_i;
static const char *menu_shot_path;
static int menu_shot_page;

static void play_frame(void) {
    if (++srm_frames >= 60) { srm_frames = 0; srm_save(); }
    if (menu_script_i < menu_script_n && frames >= 3) ui_key(menu_script[menu_script_i++]);
    if (menu_shot_path && frames + 1 >= max_frames) {
        menu_open = 1;
        menu_page = menu_shot_page;
        menu_draw(fb_front);
        if (!write_png(menu_shot_path, fb_menu, SCREEN_W, SCREEN_H)) fprintf(stderr, "error: cannot write %s\n", menu_shot_path);
        menu_shot_path = NULL;
    }
    if (present_shot_path && frames + 1 >= max_frames) {
        present_shot();
        present_shot_path = NULL;
    }
    if (opt_no_window) {
        live_pad = held;
        live_pad2 = 0;
        return;
    }
    present(fb_front);
    handle_events();
    fast = SDL_GetKeyboardState(NULL)[SDL_SCANCODE_TAB] != 0;
    flush_audio();
    if (paused || menu_open) {
        while (paused || menu_open) {
            SDL_Delay(20);
            handle_events();
            if (menu_open) { menu_draw(fb_front); present(fb_menu); }
            else present(fb_front);
            t_next = SDL_GetPerformanceCounter();
        }
        if (adev) audio_silence(2);
    }
    live_pad = read_keyboard(0) | read_pad(0) | held;
    live_pad2 = read_keyboard(1) | read_pad(1);
    pace();
}

static int open_window(void) {
    char title[64];
    int i, scale = cfg.eff.v[O_SCALE], ww, wh;
    if (SDL_Init(SDL_INIT_VIDEO | SDL_INIT_GAMECONTROLLER) != 0) {
        fprintf(stderr, "error: SDL_Init: %s\n", SDL_GetError());
        return 0;
    }
    for (i = 0; i < 21; i++) title[i] = rom_size > 0x7FD5 && rom[0x7FC0 + i] >= 32 && rom[0x7FC0 + i] < 127 ? (char)rom[0x7FC0 + i] : ' ';
    title[21] = 0;
    ww = (int)(SCREEN_W * scale * (cfg.eff.v[O_ASPECT] == ASPECT_43 ? 7.0 / 6.0 : 1.0) + 0.5);
    wh = SCREEN_H * scale;
    win = SDL_CreateWindow(title, SDL_WINDOWPOS_CENTERED, SDL_WINDOWPOS_CENTERED, ww, wh,
                           SDL_WINDOW_RESIZABLE | (cfg.eff.v[O_FULLSCREEN] ? SDL_WINDOW_FULLSCREEN_DESKTOP : 0));
    fullscreen = cfg.eff.v[O_FULLSCREEN];
    SDL_SetHint(SDL_HINT_RENDER_SCALE_QUALITY, "nearest");
    if (!win || !(ren = SDL_CreateRenderer(win, -1, SDL_RENDERER_ACCELERATED | SDL_RENDERER_TARGETTEXTURE | (cfg.eff.v[O_VSYNC] ? SDL_RENDERER_PRESENTVSYNC : 0))) ||
        !(tex = SDL_CreateTexture(ren, SDL_PIXELFORMAT_RGB24, SDL_TEXTUREACCESS_STREAMING, SCREEN_W, SCREEN_H))) {
        fprintf(stderr, "error: cannot open the window: %s\n", SDL_GetError());
        return 0;
    }
    perf_freq = SDL_GetPerformanceFrequency();
    t_next = st_start = SDL_GetPerformanceCounter();
    if (opt_stats) atexit(print_stats);
    return 1;
}

/* Without a ROM argument, asks for one with the system's file dialog (Windows only). */
static const char *pick_rom(void) {
#ifdef HAVE_FILE_DIALOG
    static char buf[MAX_PATH];
    OPENFILENAMEA ofn;
    memset(&ofn, 0, sizeof ofn);
    buf[0] = 0;
    ofn.lStructSize = sizeof ofn;
    ofn.lpstrFilter = "SNES ROM (*.sfc;*.smc)\0*.sfc;*.smc\0All files\0*.*\0";
    ofn.lpstrFile = buf;
    ofn.nMaxFile = MAX_PATH;
    ofn.lpstrTitle = "Select a SNES ROM";
    ofn.Flags = OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR;
    if (GetOpenFileNameA(&ofn)) return buf;
#endif
    return NULL;
}


static void print_config(void) {
    int i;
    for (i = 0; i < ini.n; i++) printf("%s\n", ini.line[i]);
}

int main(int argc, char **argv) {
    const char *path = NULL;
    int i, bad = 0;
    max_frames = 0xFFFFFFFFu;
    opt_scale = 3;
    for (i = 1; i < argc; i++) {
        if (!strcmp(argv[i], "--frames") && i + 1 < argc) max_frames = (uint32_t)strtoul(argv[++i], NULL, 10);
        else if (!strcmp(argv[i], "--frame-blocks") && i + 1 < argc) frame_len = (uint32_t)strtoul(argv[++i], NULL, 10);
        else if (!strcmp(argv[i], "--screenshot") && i + 1 < argc) shot_path = argv[++i];
        else if (!strcmp(argv[i], "--wav") && i + 1 < argc) wav_path = argv[++i];
        else if (!strcmp(argv[i], "--mute")) opt_mute = 1;
        else if (!strcmp(argv[i], "--stats")) opt_stats = 1;
        else if (!strcmp(argv[i], "--scale") && i + 1 < argc) { opt_scale = atoi(argv[++i]); cli_scale = 1; }
        else if (!strcmp(argv[i], "--fullscreen")) { opt_fullscreen = 1; cli_fullscreen = 1; }
        else if (!strcmp(argv[i], "--print-keys")) opt_print_keys = 1;
        else if (!strcmp(argv[i], "--print-config")) opt_print_config = 1;
        else if (!strcmp(argv[i], "--no-window")) opt_no_window = 1;
        else if (!strcmp(argv[i], "--present-shot") && i + 1 < argc) present_shot_path = argv[++i];
        else if (!strcmp(argv[i], "--present-size") && i + 1 < argc) {
            if (sscanf(argv[++i], "%dx%d", &present_w, &present_h) != 2 || present_w < 1 || present_h < 1 || present_w > 4096 || present_h > 4096) { bad = 1; break; }
        }
        else if (!strcmp(argv[i], "--opt") && i + 1 < argc) { /* --opt filter=sharp: any option of config.ini, for this run only */
            char *eq = strchr(argv[++i], '=');
            int o, v = 0;
            if (!eq) { bad = 1; break; }
            for (o = 0; o < O_COUNT; o++) if (strlen(opt_defs[o].key) == (size_t)(eq - argv[i]) && !strncmp(opt_defs[o].key, argv[i], (size_t)(eq - argv[i]))) break;
            if (o == O_COUNT || !opt_parse(&opt_defs[o], eq + 1, &v)) { bad = 1; break; }
            cli_set[o] = 1; cli_val[o] = v;
        }
        else if (!strcmp(argv[i], "--menu-shot") && i + 1 < argc) menu_shot_path = argv[++i];
        else if (!strcmp(argv[i], "--menu-page") && i + 1 < argc) { menu_shot_page = atoi(argv[++i]); if (menu_shot_page < 0 || menu_shot_page >= PG_COUNT) { bad = 1; break; } }
        else if (!strcmp(argv[i], "--menu-keys") && i + 1 < argc) {
            char *tok, list[1024];
            snprintf(list, sizeof list, "%s", argv[++i]);
            for (tok = strtok(list, ","); tok; tok = strtok(NULL, ",")) {
                SDL_Scancode sc = SDL_GetScancodeFromName(trim(tok));
                if (sc == SDL_SCANCODE_UNKNOWN || menu_script_n == 64) { bad = 1; break; }
                menu_script[menu_script_n++] = sc;
            }
            if (bad) break;
        }
        else if (!strcmp(argv[i], "--press") && i + 1 < argc) { if (!parse_press(argv[++i])) { bad = 1; break; } }
        else if (!strcmp(argv[i], "--hold") && i + 1 < argc) { int m = parse_buttons(argv[++i]); if (m < 0) { bad = 1; break; } held |= (uint16_t)m; }
        else if (!strcmp(argv[i], "--poke") && i + 1 < argc) { if (!parse_poke(argv[++i])) { bad = 1; break; } }
        else if (argv[i][0] != '-' && !path) path = argv[i];
        else { bad = 1; break; }
    }
    if (!bad && !path && !opt_no_window && !opt_print_keys) path = pick_rom();
    if (bad || !path || max_frames == 0 || frame_len == 0 || (cli_scale && opt_scale < 1)) {
        fprintf(stderr, "usage: %s [<rom>] [--scale N] [--fullscreen] [--no-window] [--frames N] [--screenshot FILE]\n"
                        "       [--wav FILE] [--frame-blocks N] [--press BUTTONS@A-B]... [--poke ADDR=BYTES@N]...\n"
                        "       [--opt KEY=VALUE]... [--present-shot FILE --present-size WxH] [--menu-keys NAME,...] [--menu-shot FILE] [--menu-page N] [--print-config]\n", argv[0]);
        return 2;
    }
    if (!load_rom(path)) return 2;
    if (wav_path && !wav_open(wav_path)) return 2;
    SDL_SetMainReady();
    cfg_writable = !opt_no_window || menu_script_n > 0;
    config_load(path);
    srm_load(path);
    if (opt_print_keys) {
        for (i = 0; i < nbinds; i++)
            if (binds[i].dev == BIND_KEY && binds[i].player == 0) printf("%s = %s\n", SDL_GetScancodeName((SDL_Scancode)binds[i].code), button_name((uint16_t)binds[i].button));
        return 0;
    }
    if (opt_print_config) atexit(print_config);
    if (!opt_no_window && !open_window()) return 2;
    if (!opt_no_window) {
        quiet_exit = !opt_stats && !shot_path;
        open_pads();
        open_audio();
        audio_hook = play_audio;
        t_next = st_start = SDL_GetPerformanceCounter(); /* opening the device can take a while */
    }
    frame_hook = play_frame;
    return run_rom();
}
