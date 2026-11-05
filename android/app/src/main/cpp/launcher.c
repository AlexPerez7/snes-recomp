/* launcher.c - SDL_main de la app Android de snes-port. Con la ROM que dejó LauncherActivity en files/rom.sfc: genera
 * el C del juego con la biblioteca del generador, lo compila en memoria con libtcc, lo une al runtime (que va en esta
 * misma biblioteca, en modo SNES_RT_LIBRARY), registra el punto de entrada del juego y llama al frontal de play.c.
 * Los assets (perfil, C propio, cabeceras para TinyCC) los copia GameActivity a files/assets antes de empezar.
 * No contiene nada de ninguna ROM. */
#define _GNU_SOURCE
#include <SDL.h>
#include <android/log.h>
#include <dirent.h>
#include <dlfcn.h>
#include <ftw.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <time.h>
#include "libtcc.h"

#define LOG(...) __android_log_print(ANDROID_LOG_INFO, "snesport", __VA_ARGS__)

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

static double now_ms(void) {
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return (double)t.tv_sec * 1e3 + (double)t.tv_nsec / 1e6;
}

static void fail(const char *msg) {
    LOG("ERROR: %s", msg);
    SDL_ShowSimpleMessageBox(SDL_MESSAGEBOX_ERROR, "SNES Port", msg, NULL);
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

static void on_tcc_error(void *opaque, const char *msg) {
    (void)opaque;
    LOG("tcc: %s", msg);
}

/* libc names that the C of the game uses: with -nostdlib libtcc does not look for them, so they go in one by one. */
static const char *const libc_names[] = {
    "memcpy", "memmove", "memset", "memcmp", "strlen", "strcmp", "strncmp", "strchr", "strrchr", "strstr", "strcspn",
    "strcpy", "strncpy", "malloc", "calloc", "realloc", "free", "exit", "atoi", "strtol", "strtoul", "fopen", "fclose",
    "fread", "fwrite", "fprintf", "snprintf", "fputs", "fputc", "fgets", "stat", "stderr", NULL};

int main(int argc, char **argv) {
    const char *files = SDL_AndroidGetInternalStoragePath();
    char rom_path[4096], assets[4096], path[4096], src[4096], saves[4096], gamedir[4096], include[4096];
    char *rom, *toml, *index;
    size_t rom_len = 0, n_game = 0, i;
    const char **gnames = NULL, **gtexts = NULL;
    SnesGenResult *gen;
    TCCState *s;
    void (*init)(void);
    char *line, *save;
    double t0, t1, t2, t3;
    const SnesRtSymbol *sym;
    const char *const *ln;
    char *args[9];
    (void)argc; (void)argv;
    if (!files) { fail("no encuentro la carpeta de datos de la app"); return 1; }
    snprintf(rom_path, sizeof rom_path, "%s/rom.sfc", files);
    snprintf(assets, sizeof assets, "%s/assets", files);
    snprintf(src, sizeof src, "%s/src", files);
    snprintf(saves, sizeof saves, "%s/saves", files);
    snprintf(gamedir, sizeof gamedir, "%s/game/c", assets);
    snprintf(include, sizeof include, "%s/tcc_include", assets);
    mkdir(saves, 0700);

    rom = read_file(rom_path, &rom_len);
    snprintf(path, sizeof path, "%s/game/profile.toml", assets);
    toml = read_file(path, NULL);
    snprintf(path, sizeof path, "%s/index.txt", assets);
    index = read_file(path, NULL);
    if (!rom || !toml || !index) { fail("faltan la ROM o los ficheros del juego de la app"); return 1; }

    /* the C of the game: game/c/*.c and *.h (not the lang/ folder) */
    gnames = calloc(256, sizeof *gnames);
    gtexts = calloc(256, sizeof *gtexts);
    for (line = strtok_r(index, "\n", &save); line; line = strtok_r(NULL, "\n", &save)) {
        size_t len = strlen(line);
        if (strncmp(line, "game/c/", 7) || strchr(line + 7, '/') || len < 9) continue;
        if (strcmp(line + len - 2, ".c") && strcmp(line + len - 2, ".h")) continue;
        if (n_game >= 255) break;
        snprintf(path, sizeof path, "%s/%s", assets, line);
        gnames[n_game] = strdup(line + 7);
        gtexts[n_game] = read_file(path, NULL);
        if (!gtexts[n_game]) { fail("no puedo leer el C propio del juego"); return 1; }
        n_game++;
    }

    t0 = now_ms();
    gen = snes_gen_generate((const uint8_t *)rom, rom_len, toml, gnames, gtexts, n_game);
    t1 = now_ms();
    if (snes_gen_error(gen)) {
        snprintf(path, sizeof path, "Esta ROM no sirve: %s", snes_gen_error(gen));
        fail(path);
        return 1;
    }
    nftw(src, rm_cb, 16, FTW_DEPTH | FTW_PHYS);
    for (i = 0; i < snes_gen_count(gen); i++) {
        snprintf(path, sizeof path, "%s/%s", src, snes_gen_name(gen, i));
        if (!write_text(path, snes_gen_text(gen, i))) { fail("no puedo escribir el C generado"); return 1; }
    }
    snes_gen_free(gen);
    free(rom);
    t2 = now_ms();

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
        DIR *d;
        struct dirent *e;
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
                if (tcc_add_file(s, f) < 0) { closedir(d); fail("el C del juego no compila (ver logcat)"); return 1; }
            }
            closedir(d);
        }
    }
    if (tcc_relocate(s) < 0) { fail("no se pudo enlazar el juego (¿memoria ejecutable denegada?)"); return 1; }
    t3 = now_ms();
    *(void **)&init = tcc_get_symbol(s, "snes_rt_module_init");
    if (!init) { fail("el juego no tiene snes_rt_module_init"); return 1; }
    init();
    LOG("preparado: generar %.0f ms, escribir %.0f ms, compilar y enlazar %.0f ms", t1 - t0, t2 - t1, t3 - t2);

    args[0] = "snes-port"; args[1] = rom_path;
    args[2] = "--saves-dir"; args[3] = saves;
    args[4] = "--game-dir"; args[5] = gamedir;
    args[6] = NULL;
    return snes_play_main(6, args);
}
