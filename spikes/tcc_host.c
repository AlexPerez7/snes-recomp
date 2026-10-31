/* tcc_host.c - prueba de factibilidad (android-feasibility-spike): compila en memoria con libtcc el C de un juego
 * (el que generan `codegen --all` o la biblioteca del generador), lo une al runtime compilado como biblioteca
 * (SNES_RT_LIBRARY) y lo ejecuta sin vídeo. Es el mismo camino que seguiría la app de Android, aquí en el PC para medir.
 *
 *     tcc_host <carpeta con el C> <carpeta de libtcc1.a e include de tcc> [opciones del runtime sin vídeo...]
 *
 * Compila todos los .c de la carpeta y de su subcarpeta game/ (no el runtime: viene en libsnesrt), con -DSNES_RT_TICK.
 * Imprime en stderr el tiempo de compilación y enlace, y la memoria pico del proceso hasta ese momento. Los símbolos
 * del runtime que usa el C del juego se le dan a libtcc uno por uno (snes_rt_symbols), sin buscarlos en las bibliotecas
 * cargadas: en Android cada biblioteca de la app vive en su propio espacio de nombres.
 * Con SNES_TCC_INCLUDE (carpetas separadas por ':') no usa las cabeceras del sistema (-nostdinc) sino esas, como en un
 * dispositivo sin ellas: las de TinyCC y spikes/tcc_include (stdint.h mínima); tampoco enlaza libtcc1.a (-nostdlib),
 * porque el C del juego solo llama al runtime.
 * No contiene nada de ninguna ROM. */
#define _POSIX_C_SOURCE 200809L
#include "libtcc.h"
#include <dirent.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/resource.h>
#include <time.h>

int snes_headless_main(int argc, char **argv);
typedef struct { const char *name; void *addr; } SnesRtSymbol;
const SnesRtSymbol *snes_rt_symbols(void); /* the runtime's functions, for tcc_add_symbol */

static double now_ms(void) {
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return (double)t.tv_sec * 1e3 + (double)t.tv_nsec / 1e6;
}

static void on_error(void *opaque, const char *msg) {
    (void)opaque;
    fprintf(stderr, "tcc: %s\n", msg);
}

/* Adds every .c of `dir` to the compilation; returns how many, or -1 on an error. */
static int add_dir(TCCState *s, const char *dir) {
    DIR *d = opendir(dir);
    struct dirent *e;
    int n = 0;
    char path[4096];
    if (!d) return 0;
    while ((e = readdir(d))) {
        size_t len = strlen(e->d_name);
        if (len < 3 || strcmp(e->d_name + len - 2, ".c")) continue;
        if (!strcmp(e->d_name, "snes_headless.c")) continue; /* the runtime: it is in libsnesrt */
        snprintf(path, sizeof path, "%s/%s", dir, e->d_name);
        if (tcc_add_file(s, path) < 0) { closedir(d); return -1; }
        n++;
    }
    closedir(d);
    return n;
}

int main(int argc, char **argv) {
    TCCState *s;
    void (*init)(void);
    char game[4096];
    double t0, t1, t2;
    struct rusage ru;
    int n, m;
    if (argc < 3) {
        fprintf(stderr, "usage: %s <dir with the C> <tcc lib dir> [runtime options...]\n", argv[0]);
        return 2;
    }
    t0 = now_ms();
    s = tcc_new();
    tcc_set_error_func(s, NULL, on_error);
    tcc_set_lib_path(s, argv[2]);
    if (getenv("SNES_TCC_INCLUDE")) tcc_set_options(s, "-nostdinc -nostdlib"); /* before the output type, which adds the default paths */
    tcc_set_output_type(s, TCC_OUTPUT_MEMORY);
    if (getenv("SNES_TCC_INCLUDE")) {
        char list[4096], *dir;
        snprintf(list, sizeof list, "%s", getenv("SNES_TCC_INCLUDE"));
        for (dir = strtok(list, ":"); dir; dir = strtok(NULL, ":")) tcc_add_sysinclude_path(s, dir);
    }
    {
        const SnesRtSymbol *sym;
        for (sym = snes_rt_symbols(); sym->name; sym++) tcc_add_symbol(s, sym->name, sym->addr);
    }
    tcc_add_include_path(s, argv[1]);
    tcc_define_symbol(s, "SNES_RT_TICK", NULL);
    n = add_dir(s, argv[1]);
    snprintf(game, sizeof game, "%s/game", argv[1]);
    m = add_dir(s, game);
    if (n <= 0 || m < 0) {
        fprintf(stderr, "error: the C does not compile (or there is none in %s)\n", argv[1]);
        return 3;
    }
    t1 = now_ms();
    if (tcc_relocate(s) < 0) {
        fprintf(stderr, "error: tcc_relocate failed\n");
        return 4;
    }
    t2 = now_ms();
    *(void **)&init = tcc_get_symbol(s, "snes_rt_module_init");
    if (!init) {
        fprintf(stderr, "error: no snes_rt_module_init in the compiled C\n");
        return 5;
    }
    init();
    getrusage(RUSAGE_SELF, &ru);
    fprintf(stderr, "tcc: %d files compiled in %.0f ms, relocated in %.0f ms; peak memory %ld MiB\n",
            n + m, t1 - t0, t2 - t1, ru.ru_maxrss / 1024);
    argv[2] = argv[0];
    return snes_headless_main(argc - 2, argv + 2);
}
