/* play_ui.h: the look of the in-game menu, without SDL: a proportional pixel font drawn for this program, and the
 * pieces of the panel (frame, bar, cursor, arrows). Everything draws into a Canvas (RGB, 3 bytes a pixel) and clips,
 * so the tests compile this file alone and check it against guard bytes. */
#ifndef PLAY_UI_H
#define PLAY_UI_H

#include <stdint.h>
#include <stdio.h>
#include <string.h>

typedef struct { uint8_t *px; int w, h; } Canvas;

/* Colours as 0xRRGGBB. */
#define UI_TEXT    0xF4F0E0u
#define UI_DIM     0x8C94B8u
#define UI_GOLD    0xF8D860u
#define UI_SHADOW  0x101028u
#define UI_GOOD    0x90E890u
#define UI_WARN    0xF89088u

static void cv_put(Canvas *c, int x, int y, uint32_t rgb) {
    uint8_t *p;
    if (x < 0 || y < 0 || x >= c->w || y >= c->h) return;
    p = &c->px[((size_t)y * c->w + x) * 3];
    p[0] = (uint8_t)(rgb >> 16); p[1] = (uint8_t)(rgb >> 8); p[2] = (uint8_t)rgb;
}

static void cv_rect(Canvas *c, int x, int y, int w, int h, uint32_t rgb) {
    int i, j;
    for (j = 0; j < h; j++) for (i = 0; i < w; i++) cv_put(c, x + i, y + j, rgb);
}

/* ---- the font ---- */

/* The shapes of the printable ASCII characters (32 to 126), drawn by hand for this program on a 5x7 grid: seven rows
 * per character, bit 4 of a row is its leftmost pixel. The menu draws them thickened (each pixel up to two wide) with a
 * shadow, and each character is as wide as its ink, so the text is proportional. */
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
    {0x01,0x01,0x0D,0x13,0x11,0x11,0x0F}, {0x00,0x00,0x0E,0x11,0x1F,0x10,0x0E}, {0x06,0x09,0x08,0x1C,0x08,0x08,0x08}, {0x00,0x00,0x0E,0x11,0x0F,0x01,0x0E},
    {0x10,0x10,0x16,0x19,0x11,0x11,0x11}, {0x04,0x00,0x0C,0x04,0x04,0x04,0x0E}, {0x02,0x00,0x06,0x02,0x02,0x12,0x0C}, {0x10,0x10,0x12,0x14,0x18,0x14,0x12},
    {0x0C,0x04,0x04,0x04,0x04,0x04,0x0E}, {0x00,0x00,0x1A,0x15,0x15,0x15,0x15}, {0x00,0x00,0x16,0x19,0x11,0x11,0x11}, {0x00,0x00,0x0E,0x11,0x11,0x11,0x0E},
    {0x00,0x00,0x1E,0x11,0x1E,0x10,0x10}, {0x00,0x00,0x0D,0x13,0x0F,0x01,0x01}, {0x00,0x00,0x16,0x19,0x10,0x10,0x10}, {0x00,0x00,0x0F,0x10,0x0E,0x01,0x1E},
    {0x08,0x08,0x1C,0x08,0x08,0x09,0x06}, {0x00,0x00,0x11,0x11,0x11,0x13,0x0D}, {0x00,0x00,0x11,0x11,0x11,0x0A,0x04}, {0x00,0x00,0x11,0x11,0x15,0x15,0x0A},
    {0x00,0x00,0x11,0x0A,0x04,0x0A,0x11}, {0x00,0x00,0x11,0x11,0x0F,0x01,0x0E}, {0x00,0x00,0x1F,0x02,0x04,0x08,0x1F}, {0x02,0x04,0x04,0x08,0x04,0x04,0x02},
    {0x04,0x04,0x04,0x04,0x04,0x04,0x04}, {0x08,0x04,0x04,0x02,0x04,0x04,0x08}, {0x00,0x00,0x08,0x15,0x02,0x00,0x00},
};

#define UI_LINE 10 /* height of a line of text, the shadow included */

/* A row of a character thickened: each pixel moves one column left and gets a copy on its right, unless that would
 * close a one-pixel gap (so an "m" or a "w" stays legible). Six columns, bit 5 on the left. */
static unsigned glyph_bold(unsigned v) {
    unsigned out = v << 1;
    int k;
    for (k = 0; k < 5; k++)
        if ((v >> k & 1) && !(v >> (k - 1 >= 0 ? k - 1 : 0) & (k > 0)) && !(k > 1 && (v >> (k - 2) & 1))) out |= 1u << k;
    return out;
}

/* The text is UTF-8. The next character of `*ps` as a code point; `*ps` moves past it. A stray continuation byte, a cut
 * sequence or an overlong/invalid one gives '?' and moves one byte. */
static int utf8_next(const char **ps) {
    const unsigned char *s = (const unsigned char *)*ps;
    int n, cp, i;
    if (s[0] < 0x80) { *ps += 1; return s[0]; }
    if (s[0] >= 0xC2 && s[0] <= 0xDF) { n = 2; cp = s[0] & 0x1F; }
    else if (s[0] >= 0xE0 && s[0] <= 0xEF) { n = 3; cp = s[0] & 0x0F; }
    else if (s[0] >= 0xF0 && s[0] <= 0xF4) { n = 4; cp = s[0] & 0x07; }
    else { *ps += 1; return '?'; }
    for (i = 1; i < n; i++) {
        if ((s[i] & 0xC0) != 0x80) { *ps += 1; return '?'; }
        cp = cp << 6 | (s[i] & 0x3F);
    }
    *ps += n;
    return cp;
}

/* The Spanish letters: a base ASCII character plus an accent drawn over it (two rows, as in the font: bit 4 is the
 * leftmost pixel). Capitals have no free row, so their accent goes in two extra rows above the seven; lowercase letters
 * have it in their own empty rows 0 and 1. `flip` turns the base upside down (for the opening marks). */
typedef struct { int cp, base; uint8_t acc0, acc1; int high, flip; } ExtGlyph;
#define ACUTE 0x04, 0x08
#define DIAER 0x0A, 0x00
#define TILDE 0x0D, 0x00
static const ExtGlyph ext_glyphs[] = {
    {0xE1,'a',ACUTE,0,0}, {0xE9,'e',ACUTE,0,0}, {0xED,'i',ACUTE,0,0}, {0xF3,'o',ACUTE,0,0}, {0xFA,'u',ACUTE,0,0},
    {0xFC,'u',DIAER,0,0}, {0xF1,'n',TILDE,0,0},
    {0xC1,'A',ACUTE,1,0}, {0xC9,'E',ACUTE,1,0}, {0xCD,'I',ACUTE,1,0}, {0xD3,'O',ACUTE,1,0}, {0xDA,'U',ACUTE,1,0},
    {0xDC,'U',DIAER,1,0}, {0xD1,'N',TILDE,1,0},
    {0xA1,'!',0,0,0,1}, {0xBF,'?',0,0,0,1},
};

/* Symbols with a shape of their own (not a base letter plus an accent): arrows for the keys and a middle dot to tell
 * several assignments apart. Seven rows of the 5x7 grid, bit 4 the leftmost pixel. */
typedef struct { int cp; uint8_t rows[7]; } SymGlyph;
static const SymGlyph sym_glyphs[] = {
    {0x2190, {0x00,0x04,0x08,0x1F,0x08,0x04,0x00}}, {0x2191, {0x04,0x0E,0x15,0x04,0x04,0x04,0x04}},
    {0x2192, {0x00,0x04,0x02,0x1F,0x02,0x04,0x00}}, {0x2193, {0x04,0x04,0x04,0x04,0x15,0x0E,0x04}},
    {0xB7,   {0x00,0x00,0x00,0x04,0x00,0x00,0x00}},
};

#define GLYPH_ROWS 9 /* two extra rows above the 5x7 grid, for the accents of the capitals */
#define GLYPH_TOP 2

/* The nine rows (row GLYPH_TOP is the first of the 5x7 grid) of the character `cp`; '?' for one the font lacks. */
static void glyph_rows(int cp, uint8_t rows[GLYPH_ROWS]) {
    const ExtGlyph *e = NULL;
    int base = cp, r;
    size_t i;
    for (i = 0; i < sizeof sym_glyphs / sizeof sym_glyphs[0]; i++)
        if (sym_glyphs[i].cp == cp) {
            memset(rows, 0, GLYPH_ROWS);
            for (r = 0; r < 7; r++) rows[GLYPH_TOP + r] = sym_glyphs[i].rows[r];
            return;
        }
    for (i = 0; i < sizeof ext_glyphs / sizeof ext_glyphs[0]; i++) if (ext_glyphs[i].cp == cp) e = &ext_glyphs[i];
    if (e) base = e->base;
    else if (cp < 32 || cp > 126) base = '?';
    memset(rows, 0, GLYPH_ROWS);
    for (r = 0; r < 7; r++) rows[GLYPH_TOP + r] = font5x7[base - 32][e && e->flip ? 6 - r : r];
    if (e && e->high) { rows[0] = e->acc0; rows[1] = e->acc1; }
    else if (e) { rows[GLYPH_TOP] |= e->acc0; rows[GLYPH_TOP + 1] |= e->acc1; }
}

/* The ink of a character: lowest and highest column after thickening, or hi < lo for a blank. */
static void glyph_ink(int c, int *lo, int *hi) {
    uint8_t rows[GLYPH_ROWS];
    int row, b;
    glyph_rows(c, rows);
    *lo = 99; *hi = -1;
    for (row = 0; row < GLYPH_ROWS; row++) {
        unsigned v = glyph_bold(rows[row]);
        for (b = 0; b < 6; b++) if (v >> b & 1) { if (b < *lo) *lo = b; if (b > *hi) *hi = b; }
    }
}

/* How far the pen moves after the character: its ink plus one pixel; the space is 3 wide. */
static int glyph_adv(int c) {
    int lo, hi;
    glyph_ink(c, &lo, &hi);
    return hi < lo ? 3 : hi - lo + 2;
}

static int text_w(const char *s) {
    int w = 0;
    while (*s) w += glyph_adv(utf8_next(&s));
    return w;
}

/* One character, thickened, at (x, y) in colour `rgb` (y is the top of the 5x7 grid); returns the advance. */
static int glyph_draw(Canvas *c, int x, int y, int ch, uint32_t rgb) {
    uint8_t rows[GLYPH_ROWS];
    int row, b, lo, hi;
    glyph_ink(ch, &lo, &hi);
    glyph_rows(ch, rows);
    if (hi >= lo)
        for (row = 0; row < GLYPH_ROWS; row++) {
            unsigned v = glyph_bold(rows[row]);
            for (b = lo; b <= hi; b++) if (v >> b & 1) cv_put(c, x + (hi - b), y + row - GLYPH_TOP, rgb);
        }
    return glyph_adv(ch);
}

/* Text with its shadow (one pixel down and right); `shadow` 0 draws the plain text. */
static void text_draw(Canvas *c, int x, int y, const char *s, uint32_t rgb, int shadow) {
    const char *p;
    int px = x;
    if (shadow) { for (p = s; *p; ) px += glyph_draw(c, px + 1, y + 1, utf8_next(&p), UI_SHADOW); px = x; }
    for (p = s; *p; ) px += glyph_draw(c, px, y, utf8_next(&p), rgb);
}

static void text_center(Canvas *c, int x, int w, int y, const char *s, uint32_t rgb) {
    text_draw(c, x + (w - text_w(s)) / 2, y, s, rgb, 1);
}

static void text_right(Canvas *c, int xr, int y, const char *s, uint32_t rgb) {
    text_draw(c, xr - text_w(s), y, s, rgb, 1);
}

/* `s` cut to fit in `maxw` pixels, ending in "..." if something was cut; `out` has room for n bytes. The cut falls
 * between characters, never inside a multibyte one. */
static void text_fit(char *out, size_t n, const char *s, int maxw) {
    size_t len = strlen(s), k;
    if (n == 0) return;
    if (text_w(s) <= maxw && len < n) { memcpy(out, s, len + 1); return; }
    for (k = len < n - 4 ? len : n - 4; ; k--) {
        while (k > 0 && ((unsigned char)s[k] & 0xC0) == 0x80) k--;
        memcpy(out, s, k);
        memcpy(out + k, "...", 4);
        if (text_w(out) <= maxw || k == 0) return;
    }
}

/* The text twice as big, for titles: the same shapes with every pixel doubled. */
static void text_draw2x(Canvas *c, int x, int y, const char *s, uint32_t rgb, uint32_t edge) {
    while (*s) {
        uint8_t rows[GLYPH_ROWS];
        int ch = utf8_next(&s), row, b, lo, hi;
        glyph_ink(ch, &lo, &hi);
        glyph_rows(ch, rows);
        if (hi >= lo)
            for (row = 0; row < GLYPH_ROWS; row++) {
                unsigned v = glyph_bold(rows[row]);
                for (b = lo; b <= hi; b++)
                    if (v >> b & 1) {
                        int px = x + (hi - b) * 2, py = y + (row - GLYPH_TOP) * 2;
                        cv_rect(c, px + 1, py + 1, 2, 2, edge);
                        cv_rect(c, px, py, 2, 2, rgb);
                    }
            }
        x += glyph_adv(ch) * 2;
    }
}

static int text_w2x(const char *s) { return text_w(s) * 2; }

/* ---- the pieces of the panel ---- */

static uint32_t ui_mix(uint32_t a, uint32_t b, int t, int max) {
    int r = (int)(a >> 16 & 255) + ((int)(b >> 16 & 255) - (int)(a >> 16 & 255)) * t / max;
    int g = (int)(a >> 8 & 255) + ((int)(b >> 8 & 255) - (int)(a >> 8 & 255)) * t / max;
    int bl = (int)(a & 255) + ((int)(b & 255) - (int)(a & 255)) * t / max;
    return (uint32_t)r << 16 | (uint32_t)g << 8 | (uint32_t)bl;
}

/* One pixel of `src` darkened and tinted blue into `dst`, as the backdrop of the menu. */
static void ui_dim_pixel(uint8_t *dst, const uint8_t *src) {
    dst[0] = (uint8_t)(src[0] / 4);
    dst[1] = (uint8_t)(src[1] / 4);
    dst[2] = (uint8_t)(src[2] / 4 + 16);
}

/* The picture `src` (same size as the canvas) darkened and tinted blue, as the backdrop of the menu. */
static void ui_backdrop(Canvas *c, const uint8_t *src) {
    size_t i, n = (size_t)c->w * c->h;
    for (i = 0; i < n; i++) ui_dim_pixel(c->px + i * 3, src + i * 3);
}

/* A panel: night-blue gradient, a double border (light outside, dark inside), a gold dot in each corner. */
static void ui_panel(Canvas *c, int x, int y, int w, int h) {
    int i, j;
    for (j = 0; j < h; j++) {
        uint32_t row = ui_mix(0x1C2C78u, 0x0C1440u, j, h > 1 ? h - 1 : 1);
        for (i = 0; i < w; i++) cv_put(c, x + i, y + j, row);
    }
    for (i = 1; i < w - 1; i++) { cv_put(c, x + i, y, 0xE8ECFAu); cv_put(c, x + i, y + h - 1, 0xE8ECFAu); }
    for (j = 1; j < h - 1; j++) { cv_put(c, x, y + j, 0xE8ECFAu); cv_put(c, x + w - 1, y + j, 0xE8ECFAu); }
    for (i = 2; i < w - 2; i++) { cv_put(c, x + i, y + 1, 0x0A0E30u); cv_put(c, x + i, y + h - 2, 0x0A0E30u); }
    for (j = 2; j < h - 2; j++) { cv_put(c, x + 1, y + j, 0x0A0E30u); cv_put(c, x + w - 2, y + j, 0x0A0E30u); }
    for (i = 3; i < w - 3; i++) { cv_put(c, x + i, y + 2, 0x5068C0u); cv_put(c, x + i, y + h - 3, 0x5068C0u); }
    for (j = 3; j < h - 3; j++) { cv_put(c, x + 2, y + j, 0x5068C0u); cv_put(c, x + w - 3, y + j, 0x5068C0u); }
    cv_rect(c, x + 1, y + 1, 3, 3, UI_GOLD);
    cv_rect(c, x + w - 4, y + 1, 3, 3, UI_GOLD);
    cv_rect(c, x + 1, y + h - 4, 3, 3, UI_GOLD);
    cv_rect(c, x + w - 4, y + h - 4, 3, 3, UI_GOLD);
}

/* A horizontal line of gold with a lighter centre, under titles. */
static void ui_rule(Canvas *c, int x, int y, int w) {
    int i;
    for (i = 0; i < w; i++) {
        int d = i < w - i ? i : w - i;
        cv_put(c, x + i, y, ui_mix(0x806020u, UI_GOLD, d < 16 ? d : 16, 16));
        cv_put(c, x + i, y + 1, 0x101028u);
    }
}

/* A bar `w` by `h` filled to `pct` percent. */
static void ui_bar(Canvas *c, int x, int y, int w, int h, int pct) {
    int i, j, fill;
    if (pct < 0) pct = 0;
    if (pct > 100) pct = 100;
    fill = (w - 2) * pct / 100;
    cv_rect(c, x, y, w, h, 0xE8ECFAu);
    cv_rect(c, x + 1, y + 1, w - 2, h - 2, 0x080C28u);
    for (j = 0; j < h - 2; j++)
        for (i = 0; i < fill; i++) cv_put(c, x + 1 + i, y + 1 + j, j * 2 < h - 2 ? 0xD8F090u : 0x50B040u);
}

/* The pointer in front of the chosen row: an arrow 8 wide, 7 high, with a dark edge. */
static void ui_cursor(Canvas *c, int x, int y) {
    static const uint8_t shape[7] = {0x08, 0x0C, 0xFE, 0xFF, 0xFE, 0x0C, 0x08};
    int row, col;
    for (row = 0; row < 7; row++)
        for (col = 0; col < 8; col++)
            if (shape[row] >> (7 - col) & 1) {
                cv_put(c, x + col + 1, y + row + 1, UI_SHADOW);
                cv_put(c, x + col, y + row, UI_GOLD);
            }
}

/* A triangle 4 wide and 7 high pointing left (dir < 0) or right; `rgb` is its colour. */
static void ui_arrow(Canvas *c, int x, int y, int dir, uint32_t rgb) {
    int row, k;
    for (row = 0; row < 7; row++) {
        int len = row < 4 ? row + 1 : 7 - row;
        for (k = 0; k < len; k++) cv_put(c, dir < 0 ? x + 3 - k : x + k, y + row, rgb);
    }
}

#endif
