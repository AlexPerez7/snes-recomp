/* play_cfg.h - the logic of the interactive frontend's options that needs no SDL: the options table with its
 * defaults, the INI file (comments and unknown keys are kept), the layout of the picture in the window, the screen
 * effects, the master volume and the key map. play.c includes it; the tests compile it on its own. */
#ifndef PLAY_CFG_H
#define PLAY_CFG_H
#include <ctype.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* ---- the options ---- */

enum { O_FULLSCREEN, O_VSYNC, O_ASPECT, O_SCALE, O_SCALING, O_FILTER, O_OVERSCAN, O_SCANLINES, O_GRID, O_FPS, O_VOLUME, O_COUNT };
enum { ASPECT_43, ASPECT_87, ASPECT_FREE };
enum { SCALING_INTEGER, SCALING_FIT };
enum { FILTER_NEAREST, FILTER_BILINEAR, FILTER_SHARP };

/* `names` is "a|b|c" for the options that take a word (the value is the index of the word), NULL for a number
 * between lo and hi. `step` is how much the menu changes a number with one key press. */
typedef struct { const char *key; int def, lo, hi; const char *names; int step; } OptDef;
static const OptDef opt_defs[O_COUNT] = {
    {"fullscreen", 0, 0, 1, "false|true", 1},
    {"vsync", 0, 0, 1, "false|true", 1},
    {"aspect", ASPECT_87, 0, 2, "4:3|8:7|free", 1},
    {"scale", 3, 1, 8, NULL, 1},
    {"scaling", SCALING_INTEGER, 0, 1, "integer|fit", 1},
    {"filter", FILTER_NEAREST, 0, 2, "nearest|bilinear|sharp", 1},
    {"overscan", 0, 0, 8, NULL, 1},
    {"scanlines", 0, 0, 100, NULL, 10},
    {"grid", 0, 0, 1, "false|true", 1},
    {"fps", 0, 0, 1, "false|true", 1},
    {"volume", 100, 0, 100, NULL, 5},
};

typedef struct { int v[O_COUNT]; char audio_device[200]; } Settings;

static void settings_defaults(Settings *s) {
    int i;
    for (i = 0; i < O_COUNT; i++) s->v[i] = opt_defs[i].def;
    s->audio_device[0] = 0;
}

static int cfg_ieq(const char *a, const char *b) {
    for (; *a && *b; a++, b++) if (tolower((unsigned char)*a) != tolower((unsigned char)*b)) return 0;
    return *a == *b;
}

/* The word number `i` of "a|b|c" into out (empty when there is no such word). */
static void opt_word(const char *names, int i, char *out, size_t n) {
    const char *p = names, *e;
    out[0] = 0;
    for (; i > 0 && p; i--) { p = strchr(p, '|'); if (p) p++; }
    if (!p) return;
    e = strchr(p, '|');
    if (!e) e = p + strlen(p);
    if ((size_t)(e - p) >= n) return;
    memcpy(out, p, (size_t)(e - p));
    out[e - p] = 0;
}

/* The value of the text `s` for option `d` into *out; 0 if it is not valid. */
static int opt_parse(const OptDef *d, const char *s, int *out) {
    char *end;
    long v;
    if (d->names) {
        char w[32];
        int i;
        for (i = 0; i <= d->hi; i++) {
            opt_word(d->names, i, w, sizeof w);
            if (cfg_ieq(w, s)) { *out = i; return 1; }
        }
        if (!strcmp(d->names, "false|true")) {
            if (!strcmp(s, "0") || cfg_ieq(s, "no") || cfg_ieq(s, "off")) { *out = 0; return 1; }
            if (!strcmp(s, "1") || cfg_ieq(s, "yes") || cfg_ieq(s, "on")) { *out = 1; return 1; }
        }
        return 0;
    }
    if (!*s) return 0;
    v = strtol(s, &end, 10);
    if (*end || v < d->lo || v > d->hi) return 0;
    *out = (int)v;
    return 1;
}

static void opt_format(const OptDef *d, int v, char *out, size_t n) {
    if (d->names) opt_word(d->names, v, out, n);
    else snprintf(out, n, "%d", v);
}

/* ---- the INI file: lines `key = value`; the other lines (comments, sections, unknown keys) are kept as they are ---- */

#define INI_MAX 512
typedef struct { char *line[INI_MAX]; int n; } Ini;

static void ini_free(Ini *ini) {
    int i;
    for (i = 0; i < ini->n; i++) free(ini->line[i]);
    ini->n = 0;
}

static void ini_add(Ini *ini, const char *text) {
    size_t n = strlen(text);
    char *c;
    if (ini->n == INI_MAX) return;
    c = (char *)malloc(n + 1);
    if (!c) return;
    memcpy(c, text, n + 1);
    ini->line[ini->n++] = c;
}

/* Reads the file at `path`; 0 when it cannot be opened (the Ini stays empty). */
static int ini_load(Ini *ini, const char *path) {
    char buf[512];
    FILE *f = fopen(path, "r");
    ini->n = 0;
    if (!f) return 0;
    while (fgets(buf, sizeof buf, f)) {
        size_t n = strlen(buf);
        while (n > 0 && (buf[n - 1] == '\n' || buf[n - 1] == '\r')) buf[--n] = 0;
        ini_add(ini, buf);
    }
    fclose(f);
    return 1;
}

/* Splits a `key = value` line into key and value (trimmed copies); 0 if the line is not one. */
static int ini_split(const char *line, char *key, size_t kn, char *val, size_t vn) {
    const char *eq, *a, *b;
    while (*line && isspace((unsigned char)*line)) line++;
    if (!*line || *line == '#' || *line == ';' || *line == '[') return 0;
    eq = strchr(line, '=');
    if (!eq) return 0;
    for (b = eq; b > line && isspace((unsigned char)b[-1]); b--) {}
    if (b == line || (size_t)(b - line) >= kn) return 0;
    memcpy(key, line, (size_t)(b - line));
    key[b - line] = 0;
    for (a = eq + 1; *a && isspace((unsigned char)*a); a++) {}
    for (b = a + strlen(a); b > a && isspace((unsigned char)b[-1]); b--) {}
    if ((size_t)(b - a) >= vn) return 0;
    memcpy(val, a, (size_t)(b - a));
    val[b - a] = 0;
    return 1;
}

static int ini_find(const Ini *ini, const char *key) {
    char k[96], v[256];
    int i;
    for (i = 0; i < ini->n; i++) if (ini_split(ini->line[i], k, sizeof k, v, sizeof v) && !strcmp(k, key)) return i;
    return -1;
}

/* The value of `key` into buf; NULL if the key is not there. */
static const char *ini_get(const Ini *ini, const char *key, char *buf, size_t n) {
    char k[96], v[256];
    int i = ini_find(ini, key);
    if (i < 0 || !ini_split(ini->line[i], k, sizeof k, v, sizeof v) || strlen(v) >= n) return NULL;
    strcpy(buf, v);
    return buf;
}

/* Sets `key`: the line of the key is replaced, or a new one is added at the end. */
static void ini_set(Ini *ini, const char *key, const char *value) {
    char line[400], *c;
    int i = ini_find(ini, key);
    snprintf(line, sizeof line, "%s = %s", key, value);
    if (i < 0) { ini_add(ini, line); return; }
    c = (char *)malloc(strlen(line) + 1);
    if (!c) return;
    strcpy(c, line);
    free(ini->line[i]);
    ini->line[i] = c;
}

static void ini_remove(Ini *ini, const char *key) {
    int i = ini_find(ini, key);
    if (i < 0) return;
    free(ini->line[i]);
    memmove(&ini->line[i], &ini->line[i + 1], (size_t)(ini->n - i - 1) * sizeof ini->line[0]);
    ini->n--;
}

/* Writes the file through a temporary one that replaces the old one; 0 on failure. */
static int ini_save(const Ini *ini, const char *path) {
    char tmp[1200];
    FILE *f;
    int i;
    snprintf(tmp, sizeof tmp, "%s.tmp", path);
    f = fopen(tmp, "w");
    if (!f) return 0;
    for (i = 0; i < ini->n; i++) fprintf(f, "%s\n", ini->line[i]);
    if (fclose(f) != 0) return 0;
    remove(path); /* rename does not replace an existing file on every system */
    return rename(tmp, path) == 0;
}

/* Reads the options from the INI into s (which starts at the defaults). An invalid value is reported on stderr and
 * keeps its default; the number of invalid values is returned. */
static int cfg_apply(Settings *s, const Ini *ini) {
    char val[256];
    int i, bad = 0;
    settings_defaults(s);
    for (i = 0; i < O_COUNT; i++) {
        int v;
        if (!ini_get(ini, opt_defs[i].key, val, sizeof val)) continue;
        if (opt_parse(&opt_defs[i], val, &v)) { s->v[i] = v; continue; }
        fprintf(stderr, "warning: config: invalid value '%s' for %s, using the default\n", val, opt_defs[i].key);
        bad++;
    }
    if (ini_get(ini, "audio_device", val, sizeof val) && strlen(val) < sizeof s->audio_device) strcpy(s->audio_device, val);
    return bad;
}

/* Writes the options of s into the INI (the lines of other keys stay where they are). */
static void cfg_store(const Settings *s, Ini *ini) {
    char val[64];
    int i;
    for (i = 0; i < O_COUNT; i++) {
        opt_format(&opt_defs[i], s->v[i], val, sizeof val);
        ini_set(ini, opt_defs[i].key, val);
    }
    ini_set(ini, "audio_device", s->audio_device);
}

/* ---- file values and command line ---- */

/* `file` is what config.ini holds and is what gets saved; `eff` is what this run uses. The bits of `cli` mark the
 * options that the command line set: they are not saved unless the user changes them in the menu. */
typedef struct { Settings file, eff; unsigned cli; } Config;

static void config_init(Config *c, const Settings *file) {
    c->file = *file;
    c->eff = *file;
    c->cli = 0;
}

/* An option given on the command line: it applies to this run only. */
static void config_override(Config *c, int opt, int v) {
    c->eff.v[opt] = v;
    c->cli |= 1u << opt;
}

/* An option changed by the user while running: it applies now and is saved. */
static void config_user_set(Config *c, int opt, int v) {
    c->eff.v[opt] = c->file.v[opt] = v;
    c->cli &= ~(1u << opt);
}

/* ---- from `<rom>.keys` (the old key map file) to the INI ---- */

/* The names of the console buttons, in the order of the bits of the pad ($4218-$4219, from bit 15). */
static const char *const btn_names[12] = {"B", "Y", "SELECT", "START", "UP", "DOWN", "LEFT", "RIGHT", "A", "X", "L", "R"};

static int btn_index(const char *name) {
    int i;
    for (i = 0; i < 12; i++) if (!strcmp(btn_names[i], name)) return i;
    return -1;
}

/* Imports the lines `KEY = BUTTON` of a `.keys` file as `p1.key.BUTTON = KEY[, KEY]` of the INI (the first line of a
 * button replaces what the INI had for it). Returns the number of lines imported, -1 if the file is not there. */
static int keys_migrate(Ini *ini, const char *path) {
    char line[200], key[100], val[100], seen[12] = {0}, cur[256], name[40];
    FILE *f = fopen(path, "r");
    int n = 0;
    if (!f) return -1;
    while (fgets(line, sizeof line, f)) {
        char *hash = strchr(line, '#');
        int b;
        if (hash) *hash = 0;
        if (!ini_split(line, key, sizeof key, val, sizeof val) || (b = btn_index(val)) < 0) {
            if (strchr(line, '=')) fprintf(stderr, "warning: %s: ignoring a line that is not KEY = BUTTON\n", path);
            continue;
        }
        snprintf(name, sizeof name, "p1.key.%s", btn_names[b]);
        if (seen[b] && ini_get(ini, name, cur, sizeof cur) && cur[0] && strlen(cur) + strlen(key) + 3 < sizeof cur) {
            strcat(cur, ", ");
            strcat(cur, key);
            ini_set(ini, name, cur);
        } else {
            ini_set(ini, name, key);
        }
        seen[b] = 1;
        n++;
    }
    fclose(f);
    return n;
}

/* ---- the picture in the window ---- */

typedef struct { int x, y, w, h; } Rect;

/* Where the picture of 256 x `lines` pixels goes in an area of aw x ah: 4:3 shows it with pixels 7/6 wider than
 * tall (as a television did), 8:7 with square pixels, and `free` stretches it over the whole area. `integer`
 * scaling uses the largest whole factor that fits (the whole area, as `fit`, if not even 1x fits). */
static Rect view_rect(int aw, int ah, int aspect, int scaling, int lines) {
    double px = aspect == ASPECT_43 ? 7.0 / 6.0 : 1.0, bw = 256 * px, bh = lines, k;
    Rect r;
    if (aspect == ASPECT_FREE) { r.x = r.y = 0; r.w = aw; r.h = ah; return r; }
    k = aw / bw < ah / bh ? aw / bw : ah / bh;
    if (scaling == SCALING_INTEGER && k >= 1.0) k = (double)(int)k;
    r.w = (int)(bw * k + 0.5);
    r.h = (int)(bh * k + 0.5);
    if (r.w > aw) r.w = aw;
    if (r.h > ah) r.h = ah;
    r.x = (aw - r.w) / 2;
    r.y = (ah - r.h) / 2;
    return r;
}

/* ---- screen effects ---- */

/* Fills `rgba` (w x h screen pixels, black with an alpha) with the effects over a picture of sw x sh pixels scaled to
 * it: `scanlines` (0-100) darkens every second screen line by that percentage; `grid` darkens the first screen
 * line and column of each source pixel (when it is at least 2 screen pixels in that direction). Returns 0, leaving
 * `rgba` alone, when no effect is on. */
static int overlay_build(uint8_t *rgba, int w, int h, int sw, int sh, int scanlines, int grid) {
    int x, y;
    if (scanlines <= 0 && !grid) return 0;
    for (y = 0; y < h; y++) {
        for (x = 0; x < w; x++) {
            int a = scanlines > 0 && (y & 1) ? scanlines * 255 / 100 : 0;
            if (grid) {
                int gx = w >= 2 * sw && (x == 0 || x * sw / w != (x - 1) * sw / w);
                int gy = h >= 2 * sh && (y == 0 || y * sh / h != (y - 1) * sh / h);
                if ((gx || gy) && a < 100) a = 100;
            }
            rgba[(y * w + x) * 4 + 0] = rgba[(y * w + x) * 4 + 1] = rgba[(y * w + x) * 4 + 2] = 0;
            rgba[(y * w + x) * 4 + 3] = (uint8_t)a;
        }
    }
    return 1;
}

/* ---- audio ---- */

static int16_t vol_apply(int16_t sample, int volume) {
    return (int16_t)((int)sample * volume / 100);
}

/* ---- the key map: which key or controller button presses which console button, for each player ---- */

typedef struct { int dev, code, player, button; } Bind; /* dev: 0 keyboard (scancode), 1 controller (button); button: a PAD_* mask */
#define BIND_KEY 0
#define BIND_PAD 1

/* Index of the binding that uses `code` on device `dev`, or -1. */
static int bind_find(const Bind *b, int n, int dev, int code) {
    int i;
    for (i = 0; i < n; i++) if (b[i].dev == dev && b[i].code == code) return i;
    return -1;
}

/* Makes `code` the key (or button) of `button` for `player`, in place of the one it had. If `code` is already used for
 * another button or player, nothing changes and 1 is returned, unless `replace` is set, which frees it from that
 * one. -1 if the table is full. */
static int bind_assign(Bind *b, int *n, int max, int dev, int code, int player, int button, int replace) {
    int i, j, other = bind_find(b, *n, dev, code);
    if (other >= 0 && (b[other].player != player || b[other].button != button)) {
        if (!replace) return 1;
        for (j = other; j < *n - 1; j++) b[j] = b[j + 1];
        (*n)--;
    }
    for (i = j = 0; i < *n; i++) if (!(b[i].dev == dev && b[i].player == player && b[i].button == button)) b[j++] = b[i];
    *n = j;
    if (*n == max) return -1;
    b[*n].dev = dev; b[*n].code = code; b[*n].player = player; b[*n].button = button;
    (*n)++;
    return 0;
}

#endif
