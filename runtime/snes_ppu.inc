/* snes_ppu.inc - PPU renderer and PNG writer, included by snes_core.inc. */

/* ---- Screenshot: background and sprite renderer, PNG writer ---- */

#define SCREEN_W 256 /* the picture of the console; the field can be wider by WIDE_MAX pixels on each side */
#define SCREEN_H 224
#define WIDE_MAX 96
#define SCREEN_W_MAX (SCREEN_W + 2 * WIDE_MAX)

static int wide_x0;              /* E: extra pixels drawn on each side of the picture (0-WIDE_MAX, a multiple of 8) */
static int screen_w = SCREEN_W;  /* 256 + 2E: width of the rows drawn now (fb_back's row stride) */

/* The result of the last write to each scroll register pair; they are written twice with a shared latch. */
static uint16_t bg_hofs[4], bg_vofs[4];
static uint8_t bgofs_latch;

/* Mode 7: the matrix ($211B-$211E, 8.8 fixed point), the centre ($211F/$2120) and the scroll ($210D/$210E) are 13 or 16
 * bit values written low byte first through a shared latch. */
static int16_t m7_a, m7_b, m7_c, m7_d, m7_x0, m7_y0, m7_hofs, m7_vofs;
static uint8_t m7_latch;

static int16_t sign13(int v) {
    v &= 0x1FFF;
    return (int16_t)(v & 0x1000 ? v - 0x2000 : v);
}

static void write_m7(uint32_t off, uint8_t v) {
    int value = (int16_t)((v << 8) | m7_latch);
    m7_latch = v;
    switch (off) {
    case 0x211B: m7_a = (int16_t)value; break;
    case 0x211C: m7_b = (int16_t)value; break;
    case 0x211D: m7_c = (int16_t)value; break;
    case 0x211E: m7_d = (int16_t)value; break;
    case 0x211F: m7_x0 = sign13(value); break;
    default: m7_y0 = sign13(value); break;
    }
}

/* $2134-$2136: the 24-bit signed product of the matrix A (16 bit) and the last byte written to $211C (8 bit). */
static uint8_t m7_product_byte(int byte) {
    int32_t p = (int32_t)m7_a * (int8_t)(m7_b >> 8);
    return (uint8_t)((uint32_t)p >> (8 * byte));
}

static void write_scroll(uint32_t off, uint8_t v) {
    int bg = (int)((off - 0x210D) / 2);
    if (off == 0x210D) { m7_hofs = sign13((v << 8) | m7_latch); m7_latch = v; }
    if (off == 0x210E) { m7_vofs = sign13((v << 8) | m7_latch); m7_latch = v; }
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

/* Colour depth of BG1-4 in each mode (0 = the background is not shown). */
static const uint8_t bg_bpp[8][4] = {{2, 2, 2, 2}, {4, 4, 2, 0}, {4, 4, 0, 0}, {8, 4, 0, 0}, {8, 2, 0, 0}, {4, 2, 0, 0}, {4, 0, 0, 0}, {0, 0, 0, 0}};

/* The map entry of background `bg` at tile (tx, ty), with `tsz` pixel tiles. */
static uint32_t map_entry(int bg, int tx, int ty, int tsz) {
    uint8_t map = hw[0x2107 + bg];
    int size = map & 3, w = size & 1 ? 64 : 32, h = size & 2 ? 64 : 32;
    int sx, sy;
    uint32_t word;
    (void)tsz;
    tx &= w - 1;
    ty &= h - 1;
    sx = (size & 1) ? (tx >> 5) & 1 : 0;
    sy = (size & 2) ? (ty >> 5) & 1 : 0;
    word = ((uint32_t)(map >> 2) << 10) + (uint32_t)(sx + sy * ((size & 1) ? 2 : 1)) * 0x400 + (uint32_t)((ty & 31) * 32 + (tx & 31));
    return (uint32_t)(vram[(word * 2) & 0xFFFF] | vram[(word * 2 + 1) & 0xFFFF] << 8);
}

/* Modes 2, 4 and 6: the entries of BG3's map give each column of 8 pixels (but the first) of BG1/BG2 a scroll of its own. */
static void offset_per_tile(int mode, int bg, int x, uint16_t *hofs, uint16_t *vofs) {
    int col = (x + (*hofs & 7)) >> 3, tsz3 = (hw[0x2105] >> 6 & 1) ? 16 : 8;
    int bit = bg == 0 ? 0x2000 : 0x4000, tx, ty;
    uint32_t eh, ev;
    if (x >= 0 && col < 1) return;
    tx = ((bg_hofs[2] & 0x3FF) / tsz3) + col - 1;
    ty = (bg_vofs[2] & 0x3FF) / tsz3;
    if (mode == 4) {
        eh = map_entry(2, tx, ty, tsz3);
        if (!(eh & bit)) return;
        if (eh & 0x8000) *vofs = (uint16_t)(eh & 0x3FF);
        else *hofs = (uint16_t)((eh & 0x3F8) | (*hofs & 7));
        return;
    }
    eh = map_entry(2, tx, ty, tsz3);
    ev = map_entry(2, tx, ty + 1, tsz3);
    if (eh & bit) *hofs = (uint16_t)((eh & 0x3F8) | (*hofs & 7));
    if (ev & bit) *vofs = (uint16_t)(ev & 0x3FF);
}

/* CGRAM index of the pixel (x, y) of background `bg` (0-3) in `mode` when its entry has priority `pri`, or -1. In the
 * modes 5 and 6 `x` counts the 512 pixels of a line and a map entry is 16 pixels wide: character N on the left, N+1 on the right. */
static int bg_pixel(int mode, int bg, int x, int y, int pri) {
    int bpp = bg_bpp[mode][bg];
    int tsz = (hw[0x2105] >> (4 + bg) & 1) ? 16 : 8;
    int tw = (mode == 5 || mode == 6) ? 16 : tsz;
    uint8_t map = hw[0x2107 + bg];
    int size = map & 3;
    int width = (size & 1 ? 64 : 32) * tw, height = (size & 2 ? 64 : 32) * tsz;
    uint16_t hofs = bg_hofs[bg] & 0x3FF, vofs = bg_vofs[bg] & 0x3FF;
    int px, py, tx, ty, ix, iy;
    uint32_t entry, tile, chr;
    int pal, c;
    if (!bpp) return -1;
    if ((mode == 2 || mode == 4 || mode == 6) && bg < 2) offset_per_tile(mode, bg, x, &hofs, &vofs);
    px = (x + hofs) & (width - 1);
    py = (y + vofs) & (height - 1);
    tx = px / tw; ty = py / tsz; ix = px % tw; iy = py % tsz;
    entry = map_entry(bg, tx, ty, tsz);
    tile = entry & 0x3FF;
    pal = (int)(entry >> 10 & 7);
    if ((int)(entry >> 13 & 1) != pri) return -1;
    if (entry & 0x4000) ix = tw - 1 - ix;
    if (entry & 0x8000) iy = tsz - 1 - iy;
    if (tw == 16 || tsz == 16) {
        tile += (uint32_t)((tw == 16 && ix >= 8) + (tsz == 16 && iy >= 8) * 16);
        ix &= 7;
        iy &= 7;
    }
    chr = bg == 0 ? (hw[0x210B] & 0xFu) : bg == 1 ? (uint32_t)(hw[0x210B] >> 4) : bg == 2 ? (hw[0x210C] & 0xFu) : (uint32_t)(hw[0x210C] >> 4);
    c = tile_pixel((chr << 13) + tile * (uint32_t)bpp * 8, bpp, ix, iy);
    if (!c) return -1;
    if (bpp == 8) return c;
    if (bpp == 4) return pal * 16 + c;
    return pal * 4 + c + (mode == 0 ? bg * 32 : 0);
}

/* Sprite pixels by line; the column of screen x is x + WIDE_MAX, so that it does not depend on E. */
static int16_t spr_col[SCREEN_H][SCREEN_W_MAX];
static uint8_t spr_pri[SCREEN_H][SCREEN_W_MAX];
static int16_t spr_xoff[128]; /* added to the X of each sprite (ppu_sprite_xoff) */

void ppu_sprite_xoff(int i, int dx) { if (i >= 0 && i < 128) spr_xoff[i] = (int16_t)dx; }
void ppu_sprite_xoff_clear(void) { memset(spr_xoff, 0, sizeof spr_xoff); }

static int wide_blank_l, wide_blank_r; /* pixels of the extra columns drawn black, from the picture outwards (ppu_wide_blank) */

void ppu_wide_blank(int left, int right) {
    wide_blank_l = left < 0 ? 0 : left;
    wide_blank_r = right < 0 ? 0 : right;
}
void ppu_wide_blank_clear(void) { wide_blank_l = wide_blank_r = 0; }

/* Fills row `row` of spr_col/spr_pri with the sprites that cover it (a lower index is in front of a higher one). As on the
 * console, only the first 32 sprites (by index) that touch the line are kept, and only 34 eight pixel slices of them are
 * fetched, taking the sprites from the last of the 32 to the first, so it is the front ones that lose slices. */
static void draw_sprites_row(int row) {
    static const int small_w[8] = {8, 8, 8, 16, 16, 32, 16, 16}, small_h[8] = {8, 8, 8, 16, 16, 32, 32, 32};
    static const int large_w[8] = {16, 32, 64, 32, 64, 64, 32, 32}, large_h[8] = {16, 32, 64, 32, 64, 64, 64, 32};
    int sel = hw[0x2101] >> 5, i, dx, n = 0, k, budget = 34;
    uint32_t name_base = (uint32_t)(hw[0x2101] & 7) << 13;
    uint32_t name_gap = (uint32_t)((hw[0x2101] >> 3 & 3) + 1) << 12;
    int list[32], first_slice[32];
    memset(spr_col[row], 0xFF, sizeof spr_col[row]); /* -1: nothing */
    for (i = 0; i < 128 && n < 32; i++) {
        int hi = oam[512 + i / 4] >> ((i % 4) * 2) & 3;
        int h = (hi & 2) ? large_h[sel] : small_h[sel], y = oam[i * 4 + 1];
        if (((row - y - 1) & 0xFF) < h) list[n++] = i;
    }
    for (k = n - 1; k >= 0; k--) {
        int hi = oam[512 + list[k] / 4] >> ((list[k] % 4) * 2) & 3;
        int w = (hi & 2) ? large_w[sel] : small_w[sel], x = oam[list[k] * 4] | (hi & 1) << 8, s0, used = 0;
        if (x >= 256) x -= 512;
        x += spr_xoff[list[k]];
        for (s0 = 0; s0 < w / 8; s0++) {
            int sx = x + s0 * 8;
            if (sx > -8 && sx < 256) used++;
        }
        first_slice[k] = used <= budget ? used : budget; /* slices kept */
        budget -= first_slice[k];
    }
    for (k = n - 1; k >= 0; k--) {
        int idx = list[k];
        int hi = oam[512 + idx / 4] >> ((idx % 4) * 2) & 3;
        int w = (hi & 2) ? large_w[sel] : small_w[sel], h = (hi & 2) ? large_h[sel] : small_h[sel];
        int x = oam[idx * 4] | (hi & 1) << 8, y = oam[idx * 4 + 1];
        uint8_t tile = oam[idx * 4 + 2], attr = oam[idx * 4 + 3];
        uint32_t table = name_base + ((attr & 1) ? name_gap : 0);
        int dy = (row - y - 1) & 0xFF, iy = (attr & 0x80) ? h - 1 - dy : dy, kept = first_slice[k], slice;
        if (x >= 256) x -= 512;
        x += spr_xoff[idx];
        /* slices are fetched left to right on the screen; the ones past the budget are not drawn. Only the slices over the
         * picture of the console count: the ones in the extra columns are free, so a wider field loses no other sprite. */
        for (slice = 0; slice < w / 8; slice++) {
            int sx0 = x + slice * 8, ix0 = (attr & 0x40) ? w - 8 - slice * 8 : slice * 8;
            if (sx0 <= -8 - wide_x0 || sx0 >= SCREEN_W + wide_x0) continue;
            if (sx0 > -8 && sx0 < SCREEN_W && kept-- <= 0) continue;
            for (dx = 0; dx < 8; dx++) {
                int sx = sx0 + dx, ix = ix0 + ((attr & 0x40) ? 7 - dx : dx), c;
                uint32_t sub;
                if (sx < -wide_x0 || sx >= SCREEN_W + wide_x0) continue;
                sub = (uint32_t)((((tile >> 4) + iy / 8) & 0xF) << 4 | (((tile & 0xF) + ix / 8) & 0xF));
                c = tile_pixel(((table + sub * 16) * 2) & 0xFFFF, 4, ix & 7, iy & 7);
                if (!c) continue;
                spr_col[row][sx + WIDE_MAX] = (int16_t)(128 + (attr >> 1 & 7) * 16 + c);
                spr_pri[row][sx + WIDE_MAX] = (uint8_t)(attr >> 4 & 3);
            }
        }
    }
}

/* CGRAM index (0-255) of the pixel (x, y) of the mode 7 background, or -1 when it is transparent. */
static int clip10(int v) { return v & 0x2000 ? v | ~0x3FF : v & 0x3FF; }

static int m7_pixel(int x, int y) {
    int sx = hw[0x211A] & 1 ? 255 - x : x, sy = hw[0x211A] & 2 ? 255 - y : y, sel = hw[0x211A] >> 6;
    int hx = clip10(m7_hofs - m7_x0), vy = clip10(m7_vofs - m7_y0), px, py, tile;
    int32_t gx = m7_a * (hx + sx) + m7_b * (vy + sy) + ((int32_t)m7_x0 << 8);
    int32_t gy = m7_c * (hx + sx) + m7_d * (vy + sy) + ((int32_t)m7_y0 << 8);
    int c;
    px = gx >> 8;
    py = gy >> 8;
    if (px < 0 || px > 1023 || py < 0 || py > 1023) {
        if (sel == 2) return -1;
        tile = 0;
        if (sel != 3) tile = vram[(((py >> 3 & 127) * 128 + (px >> 3 & 127)) * 2) & 0xFFFF];
    } else {
        tile = vram[(((py >> 3) * 128 + (px >> 3)) * 2) & 0xFFFF];
    }
    c = vram[((tile * 64 + (py & 7) * 8 + (px & 7)) * 2 + 1) & 0xFFFF];
    return c ? c : -1;
}

/* Layers front to back: kind 0 = sprites of priority n, 1 = BGn+1 with tile priority 1, 2 = BGn+1 with 0, 3 = the mode 7 BG1. */
typedef struct { int kind, n; } Layer;

/* Layer ids reported by pixel_index: 0-2 = BG1-3, 4 = sprites, 5 = backdrop. */
#define LAYER_OBJ 4
#define LAYER_BACKDROP 5

/* Windows. `layer` is 0-3 for BG1-4, 4 for the sprites and 5 for the colour window; returns 1 where x is inside the
 * combined window area of that layer ($2123-$212B), 0 where it is outside or when no window is enabled for it. */
static int win_area(int layer, int x) {
    static const uint16_t sel_reg[6] = {0x2123, 0x2123, 0x2124, 0x2124, 0x2125, 0x2125};
    int nib = hw[sel_reg[layer]] >> ((layer & 1) * 4) & 15;
    if (x < 0) x = 0; /* the extra columns of a wider field take the window of the edge of the picture */
    else if (x > SCREEN_W - 1) x = SCREEN_W - 1;
    int log = layer < 4 ? hw[0x212A] >> (layer * 2) & 3 : layer == 4 ? hw[0x212B] & 3 : hw[0x212B] >> 2 & 3;
    int in1 = x >= hw[0x2126] && x <= hw[0x2127], in2 = x >= hw[0x2128] && x <= hw[0x2129];
    int en1 = nib >> 1 & 1, en2 = nib >> 3 & 1;
    if (nib & 1) in1 = !in1;
    if (nib & 4) in2 = !in2;
    if (!en1 && !en2) return 0;
    if (!en2) return in1;
    if (!en1) return in2;
    switch (log) {
    case 0: return in1 | in2;
    case 1: return in1 & in2;
    case 2: return in1 ^ in2;
    default: return !(in1 ^ in2);
    }
}

/* CGRAM index of the front pixel of the layers in `mask` (a $212C/$212D value); `*layer` says which layer gave it. */
static int pixel_index(const Layer *order, int count, int x, int xc, int y, int mode, uint8_t mask, uint8_t wmask, int *layer) {
    int i, c;
    for (i = 0; i < count; i++) {
        int n = order[i].n;
        if (order[i].kind == 0) {
            if (!(mask & 0x10)) continue;
            if (spr_col[y][x + WIDE_MAX] >= 0 && spr_pri[y][x + WIDE_MAX] == n && !((wmask & 0x10) && win_area(4, x))) { *layer = LAYER_OBJ; return spr_col[y][x + WIDE_MAX]; }
        } else if (order[i].kind == 3) {
            if (!(mask & 1) || ((wmask & 1) && win_area(0, x))) continue;
            c = m7_pixel(x, y);
            if (c >= 0) { *layer = 0; return c; }
        } else {
            if (mode == 7 || !bg_bpp[mode][n] || !(mask >> n & 1)) continue;
            if ((wmask >> n & 1) && win_area(n, x)) continue;
            if (hw[0x2106] >> n & 1 && hw[0x2106] >> 4) {
                /* mosaic: the block of (size) x (size) pixels shows the colour at its top left; vertical blocks start at line 1 */
                int size = (hw[0x2106] >> 4) + 1, span = size * (mode == 5 || mode == 6 ? 2 : 1);
                c = bg_pixel(mode, n, xc - ((xc % span) + span) % span, y < 1 ? y : (y - 1) / size * size + 1, order[i].kind == 1);
            } else {
                c = bg_pixel(mode, n, xc, y, order[i].kind == 1);
            }
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

/* ---- HDMA ----
 * The tables the enabled channels ($420C) point at are walked once per scanline, writing to the B bus registers
 * before the line is drawn, as the console does at the start of each H-blank. The writes are real: what they leave
 * in the registers stays there until something writes them again. */

typedef struct {
    int on, indirect, repeat, do_xfer, mode, lines;
    uint8_t dest, ind_bank;
    uint32_t table, ind;
} Hdma;
static Hdma hdma_ch[8];
static uint32_t hdma_transfers;

/* Reads the header of the next group of lines; a zero header ends the channel for the frame. */
static void hdma_header(Hdma *h) {
    uint8_t v = bus_read8_raw(h->table);
    h->table = (h->table & 0xFF0000) | ((h->table + 1) & 0xFFFF);
    if (!v) { h->on = 0; return; }
    h->repeat = v >> 7;
    h->lines = v & 0x7F ? v & 0x7F : 128;
    if (h->indirect) {
        h->ind = bus_read8_raw(h->table) | (uint32_t)bus_read8_raw((h->table & 0xFF0000) | ((h->table + 1) & 0xFFFF)) << 8;
        h->table = (h->table & 0xFF0000) | ((h->table + 2) & 0xFFFF);
    }
    h->do_xfer = 1;
}

static void hdma_start_frame(void) {
    int ch;
    for (ch = 0; ch < 8; ch++) {
        uint32_t base = 0x4300 + (uint32_t)ch * 16;
        Hdma *h = &hdma_ch[ch];
        memset(h, 0, sizeof *h);
        if (!(hw[0x420C] >> ch & 1)) continue;
        h->on = 1;
        h->indirect = hw[base] >> 6 & 1;
        h->mode = hw[base] & 7;
        h->dest = hw[base + 1];
        h->table = (uint32_t)hw[base + 4] << 16 | hw[base + 2] | (uint32_t)hw[base + 3] << 8;
        h->ind_bank = hw[base + 7];
        hdma_header(h);
    }
}

static void hdma_line(void) {
    static const uint8_t pattern[8][4] = {{0, 0, 0, 0}, {0, 1, 0, 1}, {0, 0, 0, 0}, {0, 0, 1, 1},
                                          {0, 1, 2, 3}, {0, 1, 0, 1}, {0, 0, 0, 0}, {0, 0, 1, 1}};
    static const uint8_t unit[8] = {1, 2, 2, 4, 4, 4, 2, 4};
    int ch, i;
    for (ch = 0; ch < 8; ch++) {
        Hdma *h = &hdma_ch[ch];
        if (!h->on) continue;
        if (h->do_xfer) {
            for (i = 0; i < unit[h->mode]; i++) {
                uint8_t v;
                if (h->indirect) {
                    v = bus_read8_raw((uint32_t)h->ind_bank << 16 | h->ind);
                    h->ind = (h->ind + 1) & 0xFFFF;
                } else {
                    v = bus_read8_raw(h->table);
                    h->table = (h->table & 0xFF0000) | ((h->table + 1) & 0xFFFF);
                }
                {
                    uint32_t reg = 0x2100 + ((h->dest + pattern[h->mode][i]) & 0xFF);
                    bus_write8_raw(reg, v);
                    hw_writes[reg]--; /* HDMA writes are not the game's: keep the report's counts */
                }
            }
            hdma_transfers++;
        }
        if (--h->lines == 0) hdma_header(h);
        else h->do_xfer = h->repeat;
    }
}

static const char *shot_path;

/* One scanline of the picture from the PPU state as it is now: RGB triples into `out` (screen_w of them, from column -E to
 * 255+E). */
static void render_row(int y, uint8_t *out) {
    static const Layer mode0[12] = {{0, 3}, {1, 0}, {1, 1}, {0, 2}, {2, 0}, {2, 1}, {0, 1}, {1, 2}, {1, 3}, {0, 0}, {2, 2}, {2, 3}};
    static const Layer mode1_low[10] = {{0, 3}, {1, 0}, {1, 1}, {0, 2}, {2, 0}, {2, 1}, {0, 1}, {1, 2}, {0, 0}, {2, 2}};
    static const Layer mode1_high[10] = {{1, 2}, {0, 3}, {1, 0}, {1, 1}, {0, 2}, {2, 0}, {2, 1}, {0, 1}, {0, 0}, {2, 2}};
    static const Layer mode2_5[8] = {{0, 3}, {1, 0}, {0, 2}, {1, 1}, {0, 1}, {2, 0}, {0, 0}, {2, 1}};
    static const Layer mode6[6] = {{0, 3}, {1, 0}, {0, 2}, {0, 1}, {2, 0}, {0, 0}};
    static const Layer mode7_order[5] = {{0, 3}, {0, 2}, {3, 0}, {0, 1}, {0, 0}};
    int blank = hw[0x2100] >> 7 & 1, bright = hw[0x2100] & 15, mode = hw[0x2105] & 7, x;
    const Layer *order;
    int count;
    switch (mode) {
    case 0: order = mode0; count = 12; break;
    case 1: order = (hw[0x2105] & 8) ? mode1_high : mode1_low; count = 10; break;
    case 6: order = mode6; count = 6; break;
    case 7: order = mode7_order; count = 5; break;
    default: order = mode2_5; count = 8; break;
    }
    draw_sprites_row(y);
    for (x = -wide_x0; x < SCREEN_W + wide_x0; x++) {
        if ((x < 0 && -x <= wide_blank_l) || (x >= SCREEN_W && x - SCREEN_W < wide_blank_r)) {
            uint8_t *q = out + (size_t)(x + wide_x0) * 3;
            q[0] = q[1] = q[2] = 0;
            continue;
        }
        /* The main screen pixel; in the modes 5 and 6 it is the right one of the two 512-pixel columns of x, and the left
         * one comes from the subscreen. */
        int hires = mode == 5 || mode == 6, xc = hires ? 2 * x + 1 : x;
        int layer = LAYER_BACKDROP, sub_layer, idx = blank ? -1 : pixel_index(order, count, x, xc, y, mode, hw[0x212C], hw[0x212E], &layer);
        uint8_t *p = out + (size_t)(x + wide_x0) * 3;
        int c = idx < 0 ? 0 : cgram_color(idx), k, left = -1;
        int in_colour_window = win_area(5, x), black = hw[0x2130] >> 6 & 3, math = hw[0x2130] >> 4 & 3;
        if (black == 3 || (black == 1 && !in_colour_window) || (black == 2 && in_colour_window)) c = 0;
        if (idx >= 0 && (hw[0x2131] & 0x3F) && (math == 0 || (math == 1 && in_colour_window) || (math == 2 && !in_colour_window))) {
            int sub_idx = pixel_index(order, count, x, hires ? 2 * x : x, y, mode, hw[0x212D], hw[0x212F], &sub_layer);
            c = colour_math(c, layer, idx, cgram_color(sub_idx), sub_layer != LAYER_BACKDROP);
        }
        if (hires && idx >= 0) {
            /* The picture is 256 pixels wide: the two columns are blended, as a television does. */
            int sl, sidx = pixel_index(order, count, x, 2 * x, y, mode, hw[0x212D], hw[0x212F], &sl);
            left = cgram_color(sidx);
        }
        for (k = 0; k < 3; k++) {
            int c5 = c >> (5 * k) & 31, c8 = (c5 << 3) | (c5 >> 2);
            if (left >= 0) {
                int l5 = left >> (5 * k) & 31, l8 = (l5 << 3) | (l5 >> 2);
                c8 = (c8 + l8 + 1) / 2;
            }
            p[k] = (uint8_t)(idx < 0 ? 0 : c8 * bright / 15);
        }
    }
}

/* ---- The picture as the frame goes by ----
 * Row y of the picture is scanline y+1. The frame clock starts at the NMI (scanline 225), so scanline L starts at tick
 * frame_len * ((L + 37) % 262) / 262, HDMA is set up at scanline 0 and row y is drawn when its horizontal blank begins
 * (the last fifth of the line). The picture is drawn lazily: nothing happens until the game writes a PPU register (or the
 * frame ends), and then the rows that came due before that moment are drawn with the state the registers had until it. */

/* Rows are packed: the stride is the width of the frame (256 + 2E), which fb_front_w remembers for fb_front. */
static uint8_t fb_back[SCREEN_H * SCREEN_W_MAX * 3], fb_front[SCREEN_H * SCREEN_W_MAX * 3];
static int fb_front_w = SCREEN_W;
static int fb_have;           /* fb_front holds a complete picture */
static int ppu_row;           /* next row to draw in this frame */
static int ppu_inited;        /* HDMA has been set up for this frame */
static int ppu_hold;          /* > 0 while drawing: PPU writes do not trigger drawing */
static uint64_t ppu_next;     /* tick when the next row (or the HDMA set-up) comes due */

static uint64_t ppu_ready(int y) { return (uint64_t)frame_len * (uint64_t)(5 * (y + 38) + 4) / (5 * 262); }
static uint64_t ppu_init_tick(void) { return (uint64_t)frame_len * 37 / 262; }

static void ppu_update_next(void) {
    ppu_next = !ppu_inited ? ppu_init_tick() : ppu_row < SCREEN_H ? ppu_ready(ppu_row) : ~(uint64_t)0;
}

static void ppu_catch_up(void) {
    ppu_hold++;
    if (!ppu_inited && tick_in_frame >= ppu_init_tick()) {
        hdma_start_frame();
        ppu_inited = 1;
        ppu_row = 0;
    }
    while (ppu_inited && ppu_row < SCREEN_H && tick_in_frame >= ppu_ready(ppu_row)) {
        hdma_line();
        render_row(ppu_row, fb_back + (size_t)ppu_row * screen_w * 3);
        ppu_row++;
    }
    ppu_hold--;
    ppu_update_next();
}

/* Called before a write to $2100-$213F takes effect. */
static void ppu_sync(void) {
    if (!ppu_hold && tick_in_frame >= ppu_next) ppu_catch_up();
}

int ppu_wide(void) { return wide_x0; }

static int wide_next;         /* the E asked for, taken at the start of the next frame */

/* Asks for E extra pixels on each side (0-WIDE_MAX, a multiple of 8). It takes effect at once if no row of this frame has
 * been drawn yet, otherwise when the frame ends. */
static void ppu_set_wide(int e) {
    wide_next = e;
    if (ppu_row == 0) {
        wide_x0 = e;
        screen_w = SCREEN_W + 2 * e;
    }
}

/* The frame clock wrapped (tick == frame_len): finish the rows still due, keep the picture and start over. */
static void ppu_frame_end(void) {
    ppu_catch_up();
    if (ppu_row == SCREEN_H) {
        memcpy(fb_front, fb_back, (size_t)SCREEN_H * screen_w * 3);
        fb_front_w = screen_w;
        fb_have = 1;
    }
    wide_x0 = wide_next;
    screen_w = SCREEN_W + 2 * wide_next;
    ppu_row = 0;
    ppu_inited = 0;
    ppu_update_next();
}

/* --screenshot: the last picture the frame clock completed, drawn line by line while the game ran. If the run stopped
 * before the first frame ended, the rows still due are drawn from the state as it is now. */
static void screenshot(void) {
    const uint8_t *img = fb_front;
    if (hw[0x2133] & 3) printf("warning: interlace ($2133=%02X) is not applied\n", hw[0x2133]);
    if (!fb_have) {
        uint32_t t = tick_in_frame;
        tick_in_frame = frame_len;
        ppu_catch_up();
        tick_in_frame = t;
        img = fb_back;
    }
    if (hdma_transfers) printf("hdma line transfers: %u\n", (unsigned)hdma_transfers);
    if (!write_png(shot_path, img, fb_have ? fb_front_w : screen_w, SCREEN_H)) fprintf(stderr, "error: cannot write %s\n", shot_path);
}
