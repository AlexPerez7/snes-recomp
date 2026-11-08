/* play_touch.h - on-screen touch controls of the interactive frontend, without SDL: the layout of the controls (kept
 * per orientation, in fractions of the window), the hit tests that turn fingers into pressed buttons, the settings file
 * (touch.ini) and the pixel art of the controls, drawn here in code (a gold and green look in the spirit of the game:
 * gold bevelled plates, the Triforce in the middle of the pad, the four buttons in the colours of the console). The
 * sprites are small (the size of the real console's pixels) and play.c scales them up by a whole factor, so they stay
 * crisp. play.c includes it; the tests compile it on its own. All the art is original, made for this program. */
#ifndef PLAY_TOUCH_H
#define PLAY_TOUCH_H
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* ---- the controls ---- */

/* T_SAVE, T_LOAD, T_REWIND and T_SLOT drive the save states (play_state.h): save the slot, load it, rewind while held, and go to the next slot. They only
 * exist for a game that has a resume point (`touch_extra`), and always while the layout is being edited. */
enum { T_DPAD, T_A, T_B, T_X, T_Y, T_L, T_R, T_START, T_SELECT, T_MENU, T_SAVE, T_LOAD, T_REWIND, T_SLOT, T_COUNT };

static int touch_extra = 1; /* the controls from T_SAVE on count: hit tests and the state the fingers give skip them when it is 0 */

/* Directions a finger on the pad asks for. */
enum { TD_UP = 1, TD_DOWN = 2, TD_LEFT = 4, TD_RIGHT = 8 };

/* Where a control is: its centre as a fraction of the window and a scale (1.0 is the size the settings give). */
typedef struct { float x, y, s; } TCtl;
typedef struct { TCtl c[T_COUNT]; } TLayout;
typedef struct {
    TLayout land, port; /* landscape (the window is wider than tall) and portrait */
    int size;           /* percent of the base size, 50 to 150 */
    int opacity;        /* percent, 10 to 100 */
    int haptic;         /* a short vibration when a button goes down */
} TouchCfg;

/* The native size in sprite pixels of each control (w, h) and the share of the short side of the window it takes at
 * scale 1 and size 100%. Round buttons are w == h. */
typedef struct { int w, h; float unit; const char *name; } TSpec;
static const TSpec tspec[T_COUNT] = {
    {65, 65, 0.34f, "dpad"}, {25, 25, 0.13f, "a"}, {25, 25, 0.13f, "b"}, {25, 25, 0.13f, "x"}, {25, 25, 0.13f, "y"},
    {41, 17, 0.20f, "l"}, {41, 17, 0.20f, "r"}, {45, 13, 0.16f, "start"}, {45, 13, 0.16f, "select"}, {33, 13, 0.12f, "menu"},
    {33, 13, 0.12f, "save"}, {33, 13, 0.12f, "load"}, {41, 13, 0.15f, "rewind"}, {33, 13, 0.12f, "slot"},
};

static void touch_defaults(TouchCfg *t) {
    /* Landscape: the picture (4:3, as tall as the window) stays in the middle and every control goes in the side bands,
       on a phone as wide as 20:9. On a narrower window the bands are thinner and the controls reach onto the picture. */
    static const TCtl land[T_COUNT] = {
        {0.135f, 0.62f, 1}, {0.931f, 0.62f, 1}, {0.882f, 0.726f, 1}, {0.882f, 0.514f, 1}, {0.833f, 0.62f, 1},
        {0.10f, 0.12f, 1}, {0.90f, 0.12f, 1}, {0.89f, 0.91f, 1}, {0.11f, 0.91f, 1}, {0.115f, 0.30f, 1},
        {0.90f, 0.26f, 1}, {0.90f, 0.34f, 1}, {0.115f, 0.39f, 1}, {0.90f, 0.20f, 1}};
    static const TCtl port[T_COUNT] = {
        {0.24f, 0.80f, 1}, {0.90f, 0.80f, 1}, {0.78f, 0.89f, 1}, {0.78f, 0.71f, 1}, {0.66f, 0.80f, 1},
        {0.14f, 0.62f, 1}, {0.86f, 0.62f, 1}, {0.62f, 0.96f, 1}, {0.38f, 0.96f, 1}, {0.88f, 0.075f, 1},
        {0.38f, 0.075f, 1}, {0.62f, 0.075f, 1}, {0.14f, 0.075f, 1}, {0.50f, 0.52f, 1}};
    memcpy(t->land.c, land, sizeof land);
    memcpy(t->port.c, port, sizeof port);
    t->size = 100; t->opacity = 85; t->haptic = 1;
}

/* The layout for a window of w by h. */
static TLayout *touch_layout(TouchCfg *t, int w, int h) { return w >= h ? &t->land : &t->port; }

/* The controls in a window of w by h as a pixel rectangle (centre and size) and the whole factor of the sprite. */
typedef struct { int cx, cy, w, h, k; } TRect;

static TRect touch_rect(const TouchCfg *t, const TLayout *l, int id, int w, int h) {
    int side = w < h ? w : h, k, target;
    TRect r;
    float scale = (t->size / 100.0f) * l->c[id].s;
    target = (int)(side * tspec[id].unit * scale + 0.5f);
    k = target / tspec[id].w;
    if (k < 1) k = 1;
    if (target % tspec[id].w * 2 >= tspec[id].w && k < 12) k++; /* round to the nearest whole factor */
    r.k = k;
    r.w = tspec[id].w * k; r.h = tspec[id].h * k;
    r.cx = (int)(l->c[id].x * w + 0.5f); r.cy = (int)(l->c[id].y * h + 0.5f);
    return r;
}

/* ---- fingers and hits ---- */

#define TOUCH_FINGERS 10
typedef struct { int active; int64_t id; float x, y; } TFinger; /* x and y as fractions of the window */

/* Which control is under the point (px, py) in pixels, with some margin so that a thumb does not miss; -1 for none.
 * The later controls are on top. */
static int touch_hit(const TouchCfg *t, const TLayout *l, int w, int h, int px, int py) {
    int id;
    for (id = T_COUNT - 1; id >= 0; id--) {
        TRect r = touch_rect(t, l, id, w, h);
        int mx = r.w / 5, my = r.h / 5;
        if (px >= r.cx - r.w / 2 - mx && px <= r.cx + r.w / 2 + mx && py >= r.cy - r.h / 2 - my && py <= r.cy + r.h / 2 + my) return id;
    }
    return -1;
}

/* The directions a thumb at (dx, dy) from the centre of the pad gives: nothing inside the dead circle; otherwise one of
 * eight sectors. A diagonal needs the thumb near 45 degrees (the shorter axis at least 0.6 of the longer, so about
 * 28 degrees wide); the rest of each quadrant is the plain direction. With each axis judged on its own, almost any
 * touch off the centre gave a diagonal and the character walked sideways by accident. */
static unsigned touch_pad_dirs(float dx, float dy, float dead) {
    float ax = dx < 0 ? -dx : dx, ay = dy < 0 ? -dy : dy;
    unsigned d = 0;
    int horiz = 1, vert = 1;
    if (dx * dx + dy * dy <= dead * dead) return 0;
    if (ay < ax * 0.6f) vert = 0;
    else if (ax < ay * 0.6f) horiz = 0;
    if (horiz) d |= dx < 0 ? TD_LEFT : TD_RIGHT;
    if (vert) d |= dy < 0 ? TD_UP : TD_DOWN;
    return d;
}

/* The state the fingers give: `ctl` has bit (1 << T_x) for each pressed control other than the pad, `dirs` the
 * directions of the pad. */
typedef struct { unsigned ctl, dirs; } TState;

static TState touch_state(const TouchCfg *t, const TLayout *l, const TFinger *f, int w, int h) {
    TState s = {0, 0};
    int i, id;
    for (i = 0; i < TOUCH_FINGERS; i++) {
        int px, py;
        if (!f[i].active) continue;
        px = (int)(f[i].x * w); py = (int)(f[i].y * h);
        for (id = 0; id < T_COUNT; id++) {
            TRect r;
            if (id >= T_SAVE && !touch_extra) continue;
            r = touch_rect(t, l, id, w, h);
            if (id == T_DPAD) {
                float dx = (float)(px - r.cx), dy = (float)(py - r.cy), rad = r.w / 2.0f;
                if (sqrtf(dx * dx + dy * dy) > rad * 1.25f) continue;
                s.dirs |= touch_pad_dirs(dx, dy, rad * 0.22f);
            } else if (tspec[id].w == tspec[id].h) { /* round: a circle a little larger than the drawing */
                float dx = (float)(px - r.cx), dy = (float)(py - r.cy), rad = r.w / 2.0f * 1.2f;
                if (dx * dx + dy * dy <= rad * rad) s.ctl |= 1u << id;
            } else {
                int mx = r.w / 6, my = r.h / 2;
                if (px >= r.cx - r.w / 2 - mx && px <= r.cx + r.w / 2 + mx && py >= r.cy - r.h / 2 - my && py <= r.cy + r.h / 2 + my) s.ctl |= 1u << id;
            }
        }
    }
    return s;
}

/* ---- the settings file ---- */

static float clampf(float v, float lo, float hi) { return v < lo ? lo : v > hi ? hi : v; }

static void touch_clamp(TouchCfg *t) {
    int i;
    t->size = t->size < 50 ? 50 : t->size > 150 ? 150 : t->size;
    t->opacity = t->opacity < 10 ? 10 : t->opacity > 100 ? 100 : t->opacity;
    t->haptic = t->haptic != 0;
    for (i = 0; i < T_COUNT; i++) {
        t->land.c[i].x = clampf(t->land.c[i].x, 0.03f, 0.97f); t->land.c[i].y = clampf(t->land.c[i].y, 0.03f, 0.97f);
        t->land.c[i].s = clampf(t->land.c[i].s, 0.5f, 2.0f);
        t->port.c[i].x = clampf(t->port.c[i].x, 0.03f, 0.97f); t->port.c[i].y = clampf(t->port.c[i].y, 0.03f, 0.97f);
        t->port.c[i].s = clampf(t->port.c[i].s, 0.5f, 2.0f);
    }
}

/* Lines "size=100", "opacity=85", "haptic=1" and "land.a=0.89,0.64,1.00" / "port.a=...". Unknown lines are ignored. */
static void touch_load(TouchCfg *t, const char *path) {
    FILE *f = fopen(path, "r");
    char line[160];
    touch_defaults(t);
    if (!f) return;
    while (fgets(line, sizeof line, f)) {
        int v, i;
        float x, y, s;
        if (sscanf(line, "size=%d", &v) == 1) t->size = v;
        else if (sscanf(line, "opacity=%d", &v) == 1) t->opacity = v;
        else if (sscanf(line, "haptic=%d", &v) == 1) t->haptic = v;
        else {
            TLayout *l = !strncmp(line, "land.", 5) ? &t->land : !strncmp(line, "port.", 5) ? &t->port : NULL;
            if (!l) continue;
            for (i = 0; i < T_COUNT; i++) {
                size_t n = strlen(tspec[i].name);
                if (!strncmp(line + 5, tspec[i].name, n) && line[5 + n] == '=' && sscanf(line + 6 + n, "%f,%f,%f", &x, &y, &s) == 3) {
                    l->c[i].x = x; l->c[i].y = y; l->c[i].s = s;
                }
            }
        }
    }
    fclose(f);
    touch_clamp(t);
}

/* Whether two controls are in the same place, as far as the file can tell (it keeps three decimals). */
static int touch_same(const TCtl *a, const TCtl *b) {
    return fabsf(a->x - b->x) < 0.0005f && fabsf(a->y - b->y) < 0.0005f && fabsf(a->s - b->s) < 0.005f;
}

static int touch_save(const TouchCfg *t, const char *path) {
    FILE *f = fopen(path, "w");
    TouchCfg d;
    int i;
    if (!f) return 0;
    fprintf(f, "# Touch controls of the Android app. Edited from the menu (Touch tab). Only the controls that were moved are listed.\nsize=%d\nopacity=%d\nhaptic=%d\n", t->size, t->opacity, t->haptic);
    /* only the controls that are not where the defaults put them: a control that was never moved follows the defaults of the
       next version, instead of being frozen in the file at the place it had on the day the file was written */
    touch_defaults(&d);
    for (i = 0; i < T_COUNT; i++) if (!touch_same(&t->land.c[i], &d.land.c[i])) fprintf(f, "land.%s=%.3f,%.3f,%.2f\n", tspec[i].name, t->land.c[i].x, t->land.c[i].y, t->land.c[i].s);
    for (i = 0; i < T_COUNT; i++) if (!touch_same(&t->port.c[i], &d.port.c[i])) fprintf(f, "port.%s=%.3f,%.3f,%.2f\n", tspec[i].name, t->port.c[i].x, t->port.c[i].y, t->port.c[i].s);
    return fclose(f) == 0;
}

/* ---- pixel art ---- */

/* A sprite: RGBA bytes, w by h. */
typedef struct { int w, h; uint8_t *px; } TImg;

static TImg timg_new(int w, int h) {
    TImg im;
    im.w = w; im.h = h;
    im.px = (uint8_t *)calloc((size_t)w * h, 4);
    return im;
}

static void timg_put(TImg *im, int x, int y, uint32_t rgb) {
    uint8_t *p;
    if (!im->px || x < 0 || y < 0 || x >= im->w || y >= im->h) return;
    p = im->px + ((size_t)y * im->w + x) * 4;
    p[0] = (uint8_t)(rgb >> 16); p[1] = (uint8_t)(rgb >> 8); p[2] = (uint8_t)rgb; p[3] = 255;
}

/* The palette: dark outline, bevelled gold and the parchment white of the lettering. */
#define TC_OUTLINE 0x2A1A06u
#define TC_GOLD_HI 0xFFF0A0u
#define TC_GOLD    0xF8D860u
#define TC_GOLD_LO 0xC08820u
#define TC_GOLD_DK 0x784C10u
#define TC_STONE   0x1C3A2Au /* the dark green of the pad */
#define TC_STONE_HI 0x2E5A40u
#define TC_PRESS   0x48B070u
#define TC_TEXT    0xFFFCE8u
#define TC_SHADOW  0x28180Au

/* The 5x7 shapes of the letters of the labels: only the ones the controls use (A B X Y L R and the words, SAVE LOAD REWIND PLAY UPDATE among them). */
static const uint8_t tfont[][7] = {
    /* A */ {0x0E,0x11,0x11,0x1F,0x11,0x11,0x11}, /* B */ {0x1E,0x11,0x11,0x1E,0x11,0x11,0x1E}, /* X */ {0x11,0x11,0x0A,0x04,0x0A,0x11,0x11},
    /* Y */ {0x11,0x11,0x0A,0x04,0x04,0x04,0x04}, /* L */ {0x10,0x10,0x10,0x10,0x10,0x10,0x1F}, /* R */ {0x1E,0x11,0x11,0x1E,0x14,0x12,0x11},
    /* S */ {0x0F,0x10,0x10,0x0E,0x01,0x01,0x1E}, /* T */ {0x1F,0x04,0x04,0x04,0x04,0x04,0x04}, /* E */ {0x1F,0x10,0x10,0x1E,0x10,0x10,0x1F},
    /* C */ {0x0E,0x11,0x10,0x10,0x10,0x11,0x0E}, /* M */ {0x11,0x1B,0x15,0x15,0x11,0x11,0x11}, /* N */ {0x11,0x11,0x19,0x15,0x13,0x11,0x11},
    /* U */ {0x11,0x11,0x11,0x11,0x11,0x11,0x0E}, /* - */ {0x00,0x00,0x00,0x1F,0x00,0x00,0x00}, /* + */ {0x00,0x04,0x04,0x1F,0x04,0x04,0x00},
    /* O */ {0x0E,0x11,0x11,0x11,0x11,0x11,0x0E}, /* K */ {0x11,0x12,0x14,0x18,0x14,0x12,0x11}, /* D */ {0x1E,0x11,0x11,0x11,0x11,0x11,0x1E},
    /* I */ {0x0E,0x04,0x04,0x04,0x04,0x04,0x0E}, /* G */ {0x0E,0x11,0x10,0x17,0x11,0x11,0x0F}, /* J */ {0x07,0x02,0x02,0x02,0x02,0x12,0x0C},
    /* H */ {0x11,0x11,0x11,0x1F,0x11,0x11,0x11}, /* V */ {0x11,0x11,0x11,0x11,0x11,0x0A,0x04}, /* Z */ {0x1F,0x01,0x02,0x04,0x08,0x10,0x1F},
    /* W */ {0x11,0x11,0x11,0x15,0x15,0x1B,0x11}, /* P */ {0x1E,0x11,0x11,0x1E,0x10,0x10,0x10},
};

static const uint8_t *tglyph(char c) {
    static const char order[] = "ABXYLRSTECMNU-+OKDIGJHVZWP";
    const char *p = strchr(order, c);
    return p && c ? tfont[p - order] : NULL;
}

/* A text of the labels, `z` pixels per font pixel, with a shadow; x, y is the top left. */
static int ttext_w(const char *s, int z) { return (int)strlen(s) * 6 * z - z; }

static void ttext(TImg *im, int x, int y, const char *s, int z, uint32_t col, uint32_t shadow) {
    int pass;
    for (pass = 0; pass < 2; pass++) {
        const char *c;
        int cx = x + (pass == 0 ? z : 0), cy = y + (pass == 0 ? z : 0), gx, gy;
        for (c = s; *c; c++, cx += 6 * z) {
            const uint8_t *g = tglyph(*c);
            if (!g) continue;
            for (gy = 0; gy < 7; gy++)
                for (gx = 0; gx < 5; gx++)
                    if (g[gy] & (0x10 >> gx)) {
                        int a, b;
                        for (b = 0; b < z; b++) for (a = 0; a < z; a++) timg_put(im, cx + gx * z + a, cy + gy * z + b, pass == 0 ? shadow : col);
                    }
        }
    }
}

/* The gold plate of a rounded shape: `inside(e)` says whether a point is inside the shape shrunk by e pixels. The layers,
 * from the outside: dark outline, bevelled gold (two pixels), a thin dark line, then the body colour. */
typedef int (*TInside)(const void *shape, int x, int y, int e);

static void tplate(TImg *im, TInside inside, const void *shape, uint32_t body, uint32_t body_hi, int pressed) {
    int x, y;
    for (y = 0; y < im->h; y++)
        for (x = 0; x < im->w; x++) {
            int lit = (x + y) < (im->w + im->h) / 2; /* the top left of the plate catches the light */
            uint32_t hi = pressed ? TC_GOLD : TC_GOLD_HI, mid = pressed ? TC_GOLD_LO : TC_GOLD, lo = pressed ? TC_GOLD_DK : TC_GOLD_LO;
            if (!inside(shape, x, y, 0)) continue;
            if (!inside(shape, x, y, 1)) timg_put(im, x, y, TC_OUTLINE);
            else if (!inside(shape, x, y, 2)) timg_put(im, x, y, lit ? hi : mid);
            else if (!inside(shape, x, y, 3)) timg_put(im, x, y, lit ? mid : lo);
            else if (!inside(shape, x, y, 4)) timg_put(im, x, y, TC_GOLD_DK);
            else {
                /* a little shading of the body: a lighter corner at the top left */
                int rel = (x + y) * 100 / (im->w + im->h);
                timg_put(im, x, y, rel < 38 && !pressed ? body_hi : body);
            }
        }
}

typedef struct { int w, h; } TBox;

/* A round shape (circle or, for w > h, a stadium: rounded ends). */
static int tin_round(const void *shape, int x, int y, int e) {
    const TBox *b = (const TBox *)shape;
    float r = b->h / 2.0f - e, cx = (b->w - 1) / 2.0f, cy = (b->h - 1) / 2.0f, dx = (float)x - cx, dy = (float)y - cy;
    float half = (b->w - b->h) / 2.0f; /* the straight part of the stadium */
    if (r < 0) return 0;
    dx = dx > half ? dx - half : dx < -half ? dx + half : 0;
    return dx * dx + dy * dy <= (r + 0.5f) * (r + 0.5f);
}

/* A plate with straight sides and corners cut by two pixels (the small buttons). */
static int tin_plate(const void *shape, int x, int y, int e) {
    const TBox *b = (const TBox *)shape;
    int c = 2 - e > 0 ? 2 - e : 0;
    if (x < e || y < e || x >= b->w - e || y >= b->h - e) return 0;
    if (x < e + c && y < e + c && (x - e) + (y - e) < c) return 0;
    if (x >= b->w - e - c && y < e + c && (b->w - 1 - e - x) + (y - e) < c) return 0;
    if (x < e + c && y >= b->h - e - c && (x - e) + (b->h - 1 - e - y) < c) return 0;
    if (x >= b->w - e - c && y >= b->h - e - c && (b->w - 1 - e - x) + (b->h - 1 - e - y) < c) return 0;
    return 1;
}

/* A round button of the console: gold ring, a gem of its colour and its letter. */
static TImg spr_button(int id, int pressed) {
    static const uint32_t body[] = {0, 0xC02828u, 0xE8B820u, 0x2858C8u, 0x30A048u}; /* A red, B yellow, X blue, Y green */
    static const uint32_t body_hi[] = {0, 0xF05858u, 0xFFE070u, 0x6090F0u, 0x70D880u};
    static const char *letter[] = {"", "A", "B", "X", "Y"};
    TBox b = {tspec[id].w, tspec[id].h};
    TImg im = timg_new(b.w, b.h);
    int bi = id - T_A + 1, off = pressed ? 1 : 0;
    uint32_t col = body[bi];
    if (pressed) col = (col >> 1) & 0x7F7F7Fu; /* darker while it is down */
    tplate(&im, tin_round, &b, col, body_hi[bi], pressed);
    ttext(&im, (b.w - ttext_w(letter[bi], 2)) / 2 + off, (b.h - 14) / 2 + off, letter[bi], 2, TC_TEXT, TC_SHADOW);
    return im;
}

/* A plate of w by h pixels with a label (the shoulder buttons, START, the editor bar...): `round` for the stadium shape. */
static TImg spr_text(int w, int h, const char *label, int z, int pressed, int round) {
    TBox b = {w, h};
    TImg im = timg_new(w, h);
    int off = pressed ? 1 : 0;
    tplate(&im, round ? tin_round : tin_plate, &b, pressed ? TC_PRESS : TC_STONE, TC_STONE_HI, pressed);
    ttext(&im, (w - ttext_w(label, z)) / 2 + off, (h - 7 * z) / 2 + off, label, z, TC_GOLD_HI, TC_SHADOW);
    return im;
}

static TImg spr_label(int id, int pressed, const char *label, int z) {
    return spr_text(tspec[id].w, tspec[id].h, label, z, pressed, id == T_L || id == T_R);
}

/* A cross: the arms are 2*a+1 wide and reach l pixels from the centre; e shrinks it. */
static int tcross(int dx, int dy, int a, int l, int e) {
    int ax = dx < 0 ? -dx : dx, ay = dy < 0 ? -dy : dy;
    a -= e; l -= e;
    return (ax <= a && ay <= l) || (ay <= a && ax <= l);
}

/* The pad: a dark green cross in a gold frame, a small arrow on each arm and the Triforce in the middle. `dir` is
 * 0 for the plate alone or one of TD_* for the lit arm that goes over it. */
static TImg spr_dpad(int dir) {
    int n = tspec[T_DPAD].w, c = n / 2, x, y;
    TImg im = timg_new(n, n);
    for (y = 0; y < n; y++)
        for (x = 0; x < n; x++) {
            int dx = x - c, dy = y - c, lit = dx + dy < 0;
            int arm = 0;
            if (dir == TD_UP) arm = dy < 0 && tcross(dx, dy, 9, 31, 4) && (dx < 0 ? -dx : dx) <= 9 - 4;
            else if (dir == TD_DOWN) arm = dy > 0 && tcross(dx, dy, 9, 31, 4) && (dx < 0 ? -dx : dx) <= 9 - 4;
            else if (dir == TD_LEFT) arm = dx < 0 && tcross(dx, dy, 9, 31, 4) && (dy < 0 ? -dy : dy) <= 9 - 4;
            else if (dir == TD_RIGHT) arm = dx > 0 && tcross(dx, dy, 9, 31, 4) && (dy < 0 ? -dy : dy) <= 9 - 4;
            if (dir) { if (arm) { timg_put(&im, x, y, TC_PRESS); im.px[((size_t)y * n + x) * 4 + 3] = 150; } continue; }
            if (!tcross(dx, dy, 10, 32, 0)) continue;
            if (!tcross(dx, dy, 10, 32, 1)) timg_put(&im, x, y, TC_OUTLINE);
            else if (!tcross(dx, dy, 10, 32, 3)) timg_put(&im, x, y, lit ? TC_GOLD_HI : TC_GOLD);
            else if (!tcross(dx, dy, 10, 32, 4)) timg_put(&im, x, y, lit ? TC_GOLD : TC_GOLD_LO);
            else if (!tcross(dx, dy, 10, 32, 5)) timg_put(&im, x, y, TC_GOLD_DK);
            else timg_put(&im, x, y, lit && dx + dy < -10 ? TC_STONE_HI : TC_STONE);
        }
    if (!dir) {
        /* an arrow near the end of each arm */
        int i, k;
        for (i = 0; i < 4; i++) {
            int ax = i == 2 ? -24 : i == 3 ? 24 : 0, ay = i == 0 ? -24 : i == 1 ? 24 : 0;
            for (k = 0; k < 4; k++) { /* a triangle of 4 rows pointing outwards */
                int w;
                for (w = -k; w <= k; w++) {
                    int px = i < 2 ? ax + w : ax + (i == 2 ? 3 - k : k - 3);
                    int py = i < 2 ? ay + (i == 0 ? 3 - k : k - 3) : ay + w;
                    timg_put(&im, c + px, c + py, TC_GOLD);
                }
            }
        }
        /* the Triforce: three triangles of five rows, the top one over the two at the bottom, touching at the corners */
        for (i = 0; i < 3; i++) {
            int ox = i == 0 ? 0 : i == 1 ? -5 : 5, oy = i == 0 ? -8 : -3;
            for (k = 0; k < 5; k++) {
                int w;
                for (w = -k; w <= k; w++) timg_put(&im, c + ox + w, c + oy + k, k == 4 || w == -k || w == k ? TC_GOLD_HI : TC_GOLD);
            }
        }
    }
    return im;
}

/* The sprite of control `id`: state 0 up, 1 down. For the pad, state is 0 for the plate or a TD_* for an arm. */
static TImg touch_sprite(int id, int state) {
    switch (id) {
    case T_DPAD: return spr_dpad(state);
    case T_A: case T_B: case T_X: case T_Y: return spr_button(id, state);
    case T_L: return spr_label(id, state, "L", 1);
    case T_R: return spr_label(id, state, "R", 1);
    case T_START: return spr_label(id, state, "START", 1);
    case T_SELECT: return spr_label(id, state, "SELECT", 1);
    case T_SAVE: return spr_label(id, state, "SAVE", 1);
    case T_LOAD: return spr_label(id, state, "LOAD", 1);
    case T_REWIND: return spr_label(id, state, "REWIND", 1);
    case T_SLOT: return spr_label(id, state, "SLOT", 1);
    default: return spr_label(id, state, "MENU", 1);
    }
}

static void timg_free(TImg *im) { free(im->px); im->px = NULL; }

#endif
