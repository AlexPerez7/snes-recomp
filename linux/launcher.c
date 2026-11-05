/* launcher.c - el lanzador de snes-port en Linux (`linux-launcher`): una ventana pequeña donde se elige la ROM y se juega,
 * sin instalar compilador ni SDL2. Genera el C del juego con la biblioteca del generador, lo compila en memoria con libtcc
 * (con las cabeceras mínimas de assets/tcc_include, sin las del sistema), lo une al runtime (que va en este mismo ejecutable,
 * en modo SNES_RT_LIBRARY), registra el punto de entrada y llama al frontal de play.c. Es el equivalente de
 * android/app/src/main/cpp/launcher.c. No contiene nada de ninguna ROM; la ROM se queda donde está (solo se recuerda su ruta). */
#define _GNU_SOURCE
#include <SDL.h>
#include <dirent.h>
#include <dlfcn.h>
#include <ftw.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>
#include "libtcc.h"
#include <math.h>
#include <pthread.h>
#include "play_ui.h"
#include "play_touch.h"

typedef struct SnesGenResult SnesGenResult;
SnesGenResult *snes_gen_generate(const uint8_t *rom, size_t rom_len, const char *profile_toml,
                                 const char *const *game_names, const char *const *game_texts, size_t game_count);
const char *snes_gen_error(const SnesGenResult *r);
size_t snes_gen_count(const SnesGenResult *r);
const char *snes_gen_name(const SnesGenResult *r, size_t i);
const char *snes_gen_text(const SnesGenResult *r, size_t i);
void snes_gen_free(SnesGenResult *r);

typedef struct { const char *name; void *addr; } SnesRtSymbol;
const SnesRtSymbol *snes_rt_symbols(void);
int snes_play_main(int argc, char **argv);

/* avisos y actualización (src/update.rs): se llaman desde un hilo, porque esperan a la red */
typedef struct SnesUpd SnesUpd;
SnesUpd *snes_upd_version(const char *exe_dir);
int snes_upd_enabled(const char *dir);
void snes_upd_set_enabled(const char *dir, int on);
void snes_upd_skip(const char *dir, const char *tag);
void snes_upd_clean(const char *exe_dir);
SnesUpd *snes_upd_check(const char *dir, const char *exe_dir, int force);
SnesUpd *snes_upd_install(const char *dir, const char *exe_dir);
int snes_upd_kind(const SnesUpd *r);
const char *snes_upd_a(const SnesUpd *r);
const char *snes_upd_b(const SnesUpd *r);
void snes_upd_free(SnesUpd *r);

/* ---- utilidades ---- */

static char exe_path[4096], exe_dir[4096], assets[4096], data_dir[4096];

static double now_ms(void) {
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return (double)t.tv_sec * 1e3 + (double)t.tv_nsec / 1e6;
}

static char *read_file(const char *path, size_t *len) {
    FILE *f = fopen(path, "rb");
    char *buf;
    long n;
    if (!f) return NULL;
    fseek(f, 0, SEEK_END);
    n = ftell(f);
    fseek(f, 0, SEEK_SET);
    buf = malloc((size_t)n + 1);
    if (!buf || fread(buf, 1, (size_t)n, f) != (size_t)n) { fclose(f); free(buf); return NULL; }
    fclose(f);
    buf[n] = 0;
    if (len) *len = (size_t)n;
    return buf;
}

static void make_parents(const char *path) {
    char buf[4096], *p;
    snprintf(buf, sizeof buf, "%s", path);
    for (p = buf + 1; *p; p++)
        if (*p == '/') { *p = 0; mkdir(buf, 0700); *p = '/'; }
}

static int write_text(const char *path, const char *text) {
    FILE *f;
    make_parents(path);
    if (!(f = fopen(path, "wb"))) return 0;
    fputs(text, f);
    return fclose(f) == 0;
}

static int rm_cb(const char *p, const struct stat *sb, int type, struct FTW *ftw) {
    (void)sb; (void)type; (void)ftw;
    return remove(p);
}

static uint32_t crc32_of(const uint8_t *d, size_t n) {
    uint32_t c = 0xFFFFFFFFu;
    size_t i;
    int k;
    for (i = 0; i < n; i++) {
        c ^= d[i];
        for (k = 0; k < 8; k++) c = (c >> 1) ^ (0xEDB88320u & (0u - (c & 1)));
    }
    return ~c;
}

static void on_tcc_error(void *opaque, const char *msg) {
    (void)opaque;
    fprintf(stderr, "tcc: %s\n", msg);
}

/* nombres de libc que usa el C del juego: con -nostdlib libtcc no los busca, se le dan uno a uno */
static const char *const libc_names[] = {
    "memcpy", "memmove", "memset", "memcmp", "strlen", "strcmp", "strncmp", "strchr", "strrchr", "strstr", "strcspn",
    "strcpy", "strncpy", "malloc", "calloc", "realloc", "free", "exit", "atoi", "strtol", "strtoul", "fopen", "fclose",
    "fread", "fwrite", "fprintf", "snprintf", "fputs", "fputc", "fgets", "stat", "stderr", NULL};

/* ---- ventana ---- */

#define WIN_W 640
#define WIN_H 400

static SDL_Window *win;
static SDL_Renderer *ren;
static SDL_Texture *tex;
static uint8_t pixels[WIN_W * WIN_H * 3];
static Canvas cv = {pixels, WIN_W, WIN_H};

typedef struct { int x, y, w, h; } Rect;
static const Rect btn_main = {170, 256, 300, 52};
static const Rect btn_other = {195, 320, 250, 38};
static const Rect btn_third = {215, 364, 210, 26};
static const Rect chk_rect = {34, 362, 290, 26};

/* lo que el lanzador sabe de las actualizaciones */
static int upd_ok, upd_on;          /* esta instalación se actualiza sola / la casilla está marcada */
static char upd_version[32], upd_status[64];
static const char *g_third;          /* texto del tercer botón, NULL si no hay */
static int g_footer = 1;             /* la casilla y la versión, abajo */

static int inside(Rect r, int x, int y) { return x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h; }

/* un sprite RGBA de play_touch.h (el mismo arte de los botones del móvil) sobre el lienzo */
static void blit(const TImg *im, int x0, int y0) {
    int x, y;
    for (y = 0; y < im->h; y++)
        for (x = 0; x < im->w; x++) {
            const uint8_t *p = im->px + ((size_t)y * im->w + x) * 4;
            if (p[3]) cv_put(&cv, x0 + x, y0 + y, (uint32_t)p[0] << 16 | (uint32_t)p[1] << 8 | p[2]);
        }
}

/* botón: la misma placa dorada y la misma letra que los botones táctiles; con el puntero encima se ilumina */
static void button(Rect r, const char *label, int primary, int hot) {
    TImg im = spr_text(r.w, r.h, label, primary ? 3 : 2, hot, 1);
    blit(&im, r.x, r.y);
    timg_free(&im);
}

/* una pieza de la Trifuerza: triángulo con bisel dorado (luz arriba a la izquierda), de `w` de base y `h` de alto */
static void triangle(int cx, int top, int w, int h) {
    int y, x;
    for (y = 0; y < h; y++) {
        int half = (int)((long)y * (w / 2) / h) + 1;
        for (x = -half; x <= half; x++) {
            uint32_t c = TC_GOLD;
            if (y == h - 1 || x == half) c = TC_GOLD_LO;
            else if (x == -half || y < 2) c = TC_GOLD_HI;
            if (y >= h - 3 || x >= half - 1) c = (y == h - 1) ? TC_OUTLINE : TC_GOLD_LO;
            cv_put(&cv, cx + x, top + y, c);
        }
    }
    for (x = -(w / 2) - 1; x <= w / 2 + 1; x++) cv_put(&cv, cx + x, top + h, TC_OUTLINE);
}

/* el fondo: piedra verde oscura con un damero tenue, marco dorado biselado con esquinas, y la Trifuerza */
static void backdrop(void) {
    int x, y, i;
    for (y = 0; y < WIN_H; y++)
        for (x = 0; x < WIN_W; x++) {
            uint32_t c = ui_mix(0x0C1C14, 0x1A3828, y, WIN_H);
            if ((x / 16 + y / 16) % 2 == 0) c = ui_mix(c, 0x2E5A40, 1, 10);
            cv_put(&cv, x, y, c);
        }
    for (i = 0; i < 3; i++) { /* marco: contorno, oro claro, oro oscuro */
        static const uint32_t col[3] = {TC_OUTLINE, TC_GOLD, TC_GOLD_LO};
        int m = 8 + i * 2;
        cv_rect(&cv, m, m, WIN_W - 2 * m, 2, col[i]);
        cv_rect(&cv, m, WIN_H - m - 2, WIN_W - 2 * m, 2, col[i]);
        cv_rect(&cv, m, m, 2, WIN_H - 2 * m, col[i]);
        cv_rect(&cv, WIN_W - m - 2, m, 2, WIN_H - 2 * m, col[i]);
    }
    for (i = 0; i < 4; i++) { /* rombos en las esquinas */
        int cx = (i & 1) ? WIN_W - 20 : 20, cy = (i & 2) ? WIN_H - 20 : 20, d;
        for (d = 0; d < 7; d++) cv_rect(&cv, cx - (6 - d), cy - d + 0, 2 * (6 - d) + 1, 1, d < 2 ? TC_GOLD_HI : TC_GOLD);
        for (d = 0; d < 7; d++) cv_rect(&cv, cx - (6 - d), cy + d, 2 * (6 - d) + 1, 1, d < 2 ? TC_GOLD : TC_GOLD_LO);
    }
    triangle(WIN_W / 2, 30, 26, 22);
    triangle(WIN_W / 2 - 14, 54, 26, 22);
    triangle(WIN_W / 2 + 14, 54, 26, 22);
}

/* pantalla: mensaje (líneas separadas por \n) y botones (NULL = no hay) */
static void draw(const char *msg, uint32_t msg_color, const char *main_label, const char *other_label, int mx, int my) {
    char line[256];
    const char *p = msg ? msg : "";
    int y = 150, lines = 0;
    backdrop();
    text_draw2x(&cv, (WIN_W - text_w2x("ALttP Recomp")) / 2, 94, "ALttP Recomp", TC_GOLD_HI, TC_SHADOW);
    text_center(&cv, 0, WIN_W, 126, "The Legend of Zelda: A Link to the Past", 0xC8D8B8);
    for (const char *q = p; *q; q += strcspn(q, "\n") + (q[strcspn(q, "\n")] ? 1 : 0)) lines++;
    if (!main_label) y = 215 - lines * 11; /* sin botones, el mensaje va al centro */
    while (*p) {
        size_t n = strcspn(p, "\n");
        snprintf(line, sizeof line, "%.*s", (int)(n > 200 ? 200 : n), p);
        if (text_w(line) > WIN_W - 80) { /* una ruta larga: se queda el final, que es lo que distingue */
            size_t l = strlen(line);
            while (l > 4 && text_w(line + (strlen(line) - l)) > WIN_W - 120) l--;
            snprintf(line, sizeof line, "...%s", line + strlen(line) - l + 3 > line ? line + strlen(line) - l + 3 : line);
        }
        text_center(&cv, 0, WIN_W, y, line, msg_color);
        y += 22;
        p += n + (p[n] ? 1 : 0);
    }
    if (main_label) button(btn_main, main_label, 1, inside(btn_main, mx, my));
    if (other_label) button(btn_other, other_label, 0, inside(btn_other, mx, my));
    if (g_third) {
        TImg im = spr_text(btn_third.w, btn_third.h, g_third, 1, inside(btn_third, mx, my), 0);
        blit(&im, btn_third.x, btn_third.y);
        timg_free(&im);
    }
    if (upd_ok && g_footer) { /* casilla de «buscar actualizaciones», estado y versión */
        TImg box = timg_new(14, 14);
        TBox bb = {14, 14};
        char v[48];
        tplate(&box, tin_plate, &bb, upd_on ? TC_PRESS : TC_STONE, TC_STONE_HI, 0);
        blit(&box, chk_rect.x, chk_rect.y + 4);
        timg_free(&box);
        if (upd_on) { /* la marca */
            cv_rect(&cv, chk_rect.x + 4, chk_rect.y + 9, 2, 4, TC_TEXT);
            cv_rect(&cv, chk_rect.x + 6, chk_rect.y + 11, 2, 2, TC_TEXT);
            cv_rect(&cv, chk_rect.x + 8, chk_rect.y + 6, 2, 6, TC_TEXT);
            cv_rect(&cv, chk_rect.x + 10, chk_rect.y + 4, 2, 4, TC_TEXT);
        }
        text_draw(&cv, chk_rect.x + 22, chk_rect.y + 5, "Buscar actualizaciones (usa Internet)", inside(chk_rect, mx, my) ? UI_GOLD : 0xC8D8B8, 0);
        snprintf(v, sizeof v, "v%s", upd_version);
        text_right(&cv, WIN_W - 28, chk_rect.y + 5, v, UI_DIM);
        if (upd_status[0]) text_right(&cv, WIN_W - 70, chk_rect.y + 5, upd_status, UI_GOLD);
    }
    SDL_UpdateTexture(tex, NULL, pixels, WIN_W * 3);
    SDL_RenderClear(ren);
    SDL_RenderCopy(ren, tex, NULL, NULL);
    SDL_RenderPresent(ren);
}

/* el diálogo de archivos del sistema (zenity o kdialog); NULL si el usuario cancela o no hay ninguno */
static char *pick_rom_dialog(int *no_dialog) {
    static char out[4096];
    const char *cmds[] = {
        "zenity --file-selection --title='Elige tu ROM de Zelda: A Link to the Past (USA)' --file-filter='ROM de SNES | *.sfc *.smc *.SFC *.SMC' --file-filter='Todos | *' 2>/dev/null",
        "kdialog --getopenfilename \"$HOME\" 'ROM de SNES (*.sfc *.smc)|ROM (*.sfc *.smc)' 2>/dev/null",
        "yad --file --title='Elige tu ROM' 2>/dev/null", NULL};
    int i;
    *no_dialog = 1;
    for (i = 0; cmds[i]; i++) {
        char probe[64];
        FILE *f;
        snprintf(probe, sizeof probe, "command -v %.*s >/dev/null 2>&1", (int)strcspn(cmds[i], " "), cmds[i]);
        if (system(probe) != 0) continue;
        *no_dialog = 0;
        if (!(f = popen(cmds[i], "r"))) continue;
        out[0] = 0;
        if (!fgets(out, sizeof out, f)) out[0] = 0;
        pclose(f);
        out[strcspn(out, "\r\n")] = 0;
        return out[0] ? out : NULL;
    }
    return NULL;
}

/* ---- ROM ---- */

static char want_crc[16];

/* 0 si es la esperada; si no, el mensaje en `why` */
static int check_rom(const char *path, char *why, size_t why_n) {
    size_t len = 0, skip = 0;
    uint8_t *d = (uint8_t *)read_file(path, &len);
    char got[16];
    if (!d) { snprintf(why, why_n, "No puedo leer el archivo:\n%.100s", path); return 1; }
    if (len % 1024 == 512) skip = 512; /* cabecera de copión */
    snprintf(got, sizeof got, "%08X", crc32_of(d + skip, len - skip));
    free(d);
    if (strcasecmp(got, want_crc)) {
        snprintf(why, why_n, "Esta ROM no es la esperada (CRC32 %s, se esperaba %s).\nPrueba con la versión USA.", got, want_crc);
        return 1;
    }
    return 0;
}

static void remember_rom(const char *path) {
    char f[4096];
    snprintf(f, sizeof f, "%s/rom.txt", data_dir);
    write_text(f, path);
}

static int remembered_rom(char *out, size_t n) {
    char f[4096];
    char *t;
    snprintf(f, sizeof f, "%s/rom.txt", data_dir);
    if (!(t = read_file(f, NULL))) return 0;
    t[strcspn(t, "\r\n")] = 0;
    snprintf(out, n, "%s", t);
    free(t);
    return access(out, R_OK) == 0;
}

/* ---- el juego ---- */

static int prepare_and_play(const char *rom_path, const char *toml, int stats) {
    char path[4096], src[4096], saves[4096], gamedir[4096], include[4096], status[256];
    size_t rom_len = 0, n_game = 0, i;
    char *rom;
    const char **gnames = calloc(256, sizeof *gnames), **gtexts = calloc(256, sizeof *gtexts);
    SnesGenResult *gen;
    TCCState *s;
    void (*init)(void);
    const SnesRtSymbol *sym;
    const char *const *ln;
    DIR *d;
    struct dirent *e;
    double t0, t1, t2, t3;
    char *args[8];
    int n = 0;

    snprintf(src, sizeof src, "%s/src", data_dir);
    snprintf(saves, sizeof saves, "%s/saves", data_dir);
    snprintf(gamedir, sizeof gamedir, "%s/game/c", assets);
    snprintf(include, sizeof include, "%s/tcc_include", assets);
    mkdir(saves, 0700);
    rom = read_file(rom_path, &rom_len);
    if (!rom) { fprintf(stderr, "no puedo leer la ROM\n"); return 1; }

    /* el C propio del juego: game/c/*.c y *.h (no la carpeta lang/) */
    if ((d = opendir(gamedir))) {
        while ((e = readdir(d))) {
            size_t len = strlen(e->d_name);
            if (len < 3 || (strcmp(e->d_name + len - 2, ".c") && strcmp(e->d_name + len - 2, ".h")) || n_game >= 255) continue;
            snprintf(path, sizeof path, "%s/%s", gamedir, e->d_name);
            gnames[n_game] = strdup(e->d_name);
            gtexts[n_game] = read_file(path, NULL);
            if (!gtexts[n_game]) { fprintf(stderr, "no puedo leer %s\n", path); return 1; }
            n_game++;
        }
        closedir(d);
    }

    draw("Preparando el juego (1 de 2)...\nSolo la primera vez de cada arranque; unos segundos.", UI_TEXT, NULL, NULL, -1, -1);
    t0 = now_ms();
    gen = snes_gen_generate((const uint8_t *)rom, rom_len, toml, gnames, gtexts, n_game);
    t1 = now_ms();
    if (snes_gen_error(gen)) {
        snprintf(status, sizeof status, "Esta ROM no sirve: %.200s", snes_gen_error(gen));
        fprintf(stderr, "%s\n", status);
        draw(status, UI_WARN, NULL, NULL, -1, -1);
        SDL_Delay(4000);
        return 1;
    }
    nftw(src, rm_cb, 16, FTW_DEPTH | FTW_PHYS);
    for (i = 0; i < snes_gen_count(gen); i++) {
        snprintf(path, sizeof path, "%s/%s", src, snes_gen_name(gen, i));
        if (!write_text(path, snes_gen_text(gen, i))) { fprintf(stderr, "no puedo escribir el C generado\n"); return 1; }
    }
    snes_gen_free(gen);
    free(rom);
    t2 = now_ms();

    draw("Preparando el juego (2 de 2)...", UI_TEXT, NULL, NULL, -1, -1);
    s = tcc_new();
    tcc_set_error_func(s, NULL, on_tcc_error);
    tcc_set_options(s, "-nostdinc -nostdlib");
    tcc_set_output_type(s, TCC_OUTPUT_MEMORY);
    tcc_add_sysinclude_path(s, include);
    tcc_add_include_path(s, src);
    tcc_define_symbol(s, "SNES_RT_TICK", NULL);
    for (sym = snes_rt_symbols(); sym->name; sym++) tcc_add_symbol(s, sym->name, sym->addr);
    for (ln = libc_names; *ln; ln++) {
        void *a = dlsym(RTLD_DEFAULT, *ln);
        if (a) tcc_add_symbol(s, *ln, a);
    }
    {
        const char *dirs[2];
        int k;
        snprintf(path, sizeof path, "%s/game", src);
        dirs[0] = src; dirs[1] = path;
        for (k = 0; k < 2; k++) {
            if (!(d = opendir(dirs[k]))) continue;
            while ((e = readdir(d))) {
                size_t len = strlen(e->d_name);
                char f[4096];
                if (len < 3 || strcmp(e->d_name + len - 2, ".c")) continue;
                snprintf(f, sizeof f, "%s/%s", dirs[k], e->d_name);
                if (tcc_add_file(s, f) < 0) {
                    closedir(d);
                    draw("El C del juego no compila (detalles en la terminal).", UI_WARN, NULL, NULL, -1, -1);
                    SDL_Delay(4000);
                    return 1;
                }
            }
            closedir(d);
        }
    }
    if (tcc_relocate(s) < 0) {
        draw("No se pudo enlazar el juego.", UI_WARN, NULL, NULL, -1, -1);
        SDL_Delay(4000);
        return 1;
    }
    t3 = now_ms();
    *(void **)&init = tcc_get_symbol(s, "snes_rt_module_init");
    if (!init) { fprintf(stderr, "el juego no tiene snes_rt_module_init\n"); return 1; }
    init();
    if (stats) fprintf(stderr, "preparado: generar %.0f ms, escribir %.0f ms, compilar y enlazar %.0f ms\n", t1 - t0, t2 - t1, t3 - t2);

    SDL_DestroyTexture(tex); SDL_DestroyRenderer(ren); SDL_DestroyWindow(win); /* el frontal abre la suya */
    args[n++] = "snes-port"; args[n++] = (char *)rom_path;
    args[n++] = "--saves-dir"; args[n++] = saves;
    args[n++] = "--game-dir"; args[n++] = gamedir;
    args[n] = NULL;
    return snes_play_main(n, args);
}

/* ---- el trabajo de red en un hilo ---- */

static volatile int job;             /* 0 nada, 1 comprobando al abrir, 2 comprobando a mano, 3 instalando */
static volatile int job_done;
static SnesUpd *volatile job_res;
static int job_force;

static void *job_thread(void *arg) {
    (void)arg;
    job_res = job == 3 ? snes_upd_install(data_dir, exe_dir) : snes_upd_check(data_dir, exe_dir, job_force);
    job_done = 1;
    return NULL;
}

static void start_job(int kind, int force) {
    pthread_t th;
    job = kind; job_force = force; job_done = 0; job_res = NULL;
    pthread_create(&th, NULL, job_thread, NULL);
    pthread_detach(th);
}

int main(int argc, char **argv) {
    char *toml, rom[4096] = "", msg[512] = "", path[4096];
    const char *xdg;
    ssize_t k;
    int stats = 0, autoplay = 0, i, mx = -1, my = -1, running = 1, have = 0, no_dialog = 0, mode = 0;
    char new_tag[64] = "", new_page[512] = "";
    char *crc;
    SDL_Event ev;

    k = readlink("/proc/self/exe", exe_dir, sizeof exe_dir - 1);
    if (k <= 0) { fprintf(stderr, "no encuentro mi carpeta\n"); return 1; }
    exe_dir[k] = 0;
    snprintf(exe_path, sizeof exe_path, "%s", exe_dir); /* la ruta completa: tras actualizar, /proc/self/exe apunta al fichero viejo */
    *strrchr(exe_dir, '/') = 0;
    snprintf(assets, sizeof assets, "%s/assets", exe_dir);
    xdg = getenv("XDG_DATA_HOME");
    if (xdg && *xdg) snprintf(data_dir, sizeof data_dir, "%s/alttp-recomp", xdg);
    else snprintf(data_dir, sizeof data_dir, "%s/.local/share/alttp-recomp", getenv("HOME") ? getenv("HOME") : ".");
    make_parents(data_dir);
    mkdir(data_dir, 0700);

    {
        SnesUpd *v;
        snes_upd_clean(exe_dir); /* restos de una actualización anterior */
        if ((v = snes_upd_version(exe_dir))) {
            upd_ok = 1;
            snprintf(upd_version, sizeof upd_version, "%s", snes_upd_a(v));
            upd_on = snes_upd_enabled(data_dir);
            snes_upd_free(v);
        }
    }
    snprintf(path, sizeof path, "%s/game/profile.toml", assets);
    if (!(toml = read_file(path, NULL))) { fprintf(stderr, "faltan los ficheros del juego en %s\n", assets); return 1; }
    if (!(crc = strstr(toml, "crc32")) || sscanf(strchr(crc, '"') ? strchr(crc, '"') + 1 : "", "%8[0-9A-Fa-f]", want_crc) != 1) {
        fprintf(stderr, "el perfil no tiene crc32\n");
        return 1;
    }
    for (i = 1; i < argc; i++) {
        if (!strcmp(argv[i], "--stats")) stats = 1;
        else if (!strcmp(argv[i], "--play")) autoplay = 1; /* sin pantalla de inicio si la ROM sirve */
        else if (argv[i][0] != '-') snprintf(rom, sizeof rom, "%s", argv[i]);
    }

    SDL_SetHint(SDL_HINT_VIDEO_ALLOW_SCREENSAVER, "1");
    if (SDL_Init(SDL_INIT_VIDEO) != 0) { fprintf(stderr, "SDL: %s\n", SDL_GetError()); return 1; }
    win = SDL_CreateWindow("ALttP Recomp", SDL_WINDOWPOS_CENTERED, SDL_WINDOWPOS_CENTERED, WIN_W, WIN_H, SDL_WINDOW_RESIZABLE);
    ren = win ? SDL_CreateRenderer(win, -1, 0) : NULL;
    if (!ren) { fprintf(stderr, "SDL: %s\n", SDL_GetError()); return 1; }
    SDL_RenderSetLogicalSize(ren, WIN_W, WIN_H);
    tex = SDL_CreateTexture(ren, SDL_PIXELFORMAT_RGB24, SDL_TEXTUREACCESS_STREAMING, WIN_W, WIN_H);

    if (rom[0] && check_rom(rom, msg, sizeof msg)) rom[0] = 0;       /* la de la línea de órdenes, si sirve */
    else if (!rom[0] && remembered_rom(rom, sizeof rom)) {
        if (check_rom(rom, msg, sizeof msg)) rom[0] = 0;
    }
    have = rom[0] != 0;
    if (autoplay && have) running = 0;
    else if (upd_ok && upd_on) start_job(1, 0); /* la comprobación de la casilla, una vez al día como mucho */

    while (running) {
        const char *main_label = have ? "JUGAR" : "ELEGIR ROM";
        const char *other_label = have ? "CAMBIAR ROM" : NULL;
        char shown[640];
        if (job_done) { /* terminó el hilo de red */
            SnesUpd *r = job_res;
            int was = job;
            job = 0; job_done = 0; job_res = NULL;
            if (was == 3) {
                if (snes_upd_kind(r) == 0) {
                    snprintf(shown, sizeof shown, "Actualizado a la versión %s.\nReiniciando...", snes_upd_a(r));
                    g_third = NULL; g_footer = 0;
                    draw(shown, UI_GOOD, NULL, NULL, -1, -1);
                    SDL_Delay(1500);
                    execv(exe_path, argv); /* el ejecutable nuevo */
                    snprintf(upd_status, sizeof upd_status, "Actualizado: vuelve a abrir el juego");
                } else {
                    snprintf(msg, sizeof msg, "No se pudo actualizar:\n%.150s", snes_upd_a(r));
                    snprintf(upd_status, sizeof upd_status, "No se pudo actualizar");
                }
                mode = 0;
            } else if (snes_upd_kind(r) == 1) {
                snprintf(new_tag, sizeof new_tag, "%s", snes_upd_a(r));
                snprintf(new_page, sizeof new_page, "%s", snes_upd_b(r));
                mode = 1;
            } else if (was == 2) {
                if (snes_upd_kind(r) == 3) snprintf(upd_status, sizeof upd_status, "No se pudo comprobar (¿hay Internet?)");
                else snprintf(upd_status, sizeof upd_status, snes_upd_kind(r) == 2 ? "Esa versión nueva la omitiste" : "Estás al día");
            }
            snes_upd_free(r);
        }
        g_third = NULL; g_footer = 1;
        if (mode == 1) { /* hay una versión nueva */
            snprintf(shown, sizeof shown, "Hay una versión nueva: %s (tienes la %s).\nSe descarga, se comprueba y se reemplazan los archivos;\nlas partidas se conservan.", new_tag, upd_version);
            main_label = "ACTUALIZAR"; other_label = "AHORA NO"; g_third = "OMITIR ESTA VERSION"; g_footer = 0;
            draw(shown, UI_TEXT, main_label, other_label, mx, my);
        } else if (job == 3) {
            g_footer = 0;
            draw("Descargando e instalando la versión nueva...\nNo cierres la ventana.", UI_TEXT, NULL, NULL, -1, -1);
        } else {
            if (have) snprintf(shown, sizeof shown, "ROM: %.90s", rom);
            else if (msg[0]) snprintf(shown, sizeof shown, "%s", msg);
            else snprintf(shown, sizeof shown, "Elige tu copia de la ROM (USA, .sfc o .smc).\nSe queda donde está y no se envía a ningún sitio.\nTambién puedes arrastrar el archivo a esta ventana.");
            if (no_dialog) snprintf(shown, sizeof shown, "No hay un selector de archivos (zenity o kdialog).\nArrastra la ROM a esta ventana, o ejecuta:\n./alttp-recomp ruta/a/alttp_usa.sfc");
            draw(shown, (!have && msg[0]) || no_dialog ? UI_WARN : UI_TEXT, main_label, other_label, mx, my);
        }
        if (!SDL_WaitEventTimeout(&ev, 200)) continue;
        do {
            const char *chosen = NULL;
            int act = 0; /* 1 = principal, 2 = otro */
            switch (ev.type) {
            case SDL_QUIT: return 0;
            case SDL_KEYDOWN:
                if (ev.key.keysym.sym == SDLK_ESCAPE) return 0;
                if (job == 3 || mode == 1) break;
                if (ev.key.keysym.sym == SDLK_RETURN || ev.key.keysym.sym == SDLK_KP_ENTER) act = 1;
                break;
            case SDL_MOUSEMOTION: mx = ev.motion.x; my = ev.motion.y; break;
            case SDL_MOUSEBUTTONDOWN:
                if (ev.button.button == SDL_BUTTON_LEFT) {
                    /* con SDL_RenderSetLogicalSize las coordenadas ya vienen en el espacio lógico */
                    if (job == 3) break;
                    if (mode == 1) {
                        if (inside(btn_main, ev.button.x, ev.button.y)) { mode = 0; start_job(3, 0); }
                        else if (inside(btn_other, ev.button.x, ev.button.y)) mode = 0;
                        else if (inside(btn_third, ev.button.x, ev.button.y)) { snes_upd_skip(data_dir, new_tag); mode = 0; }
                        break;
                    }
                    if (upd_ok && inside(chk_rect, ev.button.x, ev.button.y)) {
                        upd_on = !upd_on;
                        snes_upd_set_enabled(data_dir, upd_on);
                        upd_status[0] = 0;
                        if (upd_on && !job) { snprintf(upd_status, sizeof upd_status, "Buscando..."); start_job(2, 1); }
                        break;
                    }
                    if (inside(btn_main, ev.button.x, ev.button.y)) act = 1;
                    else if (have && inside(btn_other, ev.button.x, ev.button.y)) act = 2;
                }
                break;
            case SDL_DROPFILE: chosen = ev.drop.file; break;
            }
            if (act == 1 && have) { running = 0; }
            else if (act == 1 || act == 2) {
                static char picked[4096];
                char *p = pick_rom_dialog(&no_dialog);
                if (p) { snprintf(picked, sizeof picked, "%s", p); chosen = picked; }
            }
            if (chosen) {
                msg[0] = 0;
                if (check_rom(chosen, msg, sizeof msg)) have = 0;
                else { snprintf(rom, sizeof rom, "%s", chosen); have = 1; remember_rom(rom); no_dialog = 0; }
                if (ev.type == SDL_DROPFILE) SDL_free(ev.drop.file);
            }
        } while (running && SDL_PollEvent(&ev));
    }
    remember_rom(rom);
    return prepare_and_play(rom, toml, stats);
}
