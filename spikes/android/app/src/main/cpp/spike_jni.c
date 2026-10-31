/* spike_jni.c - prueba de factibilidad (android-feasibility-spike) en el teléfono: genera el C del juego desde la ROM
 * del usuario con la biblioteca del generador, lo compila con libtcc (en memoria o, para el plan B, como .so cargado con
 * dlopen), lo une al runtime (libsnesrt) y mide sin vídeo. Las medidas van a <salida>/result.txt y a logcat
 * (etiqueta snesspike). Mismo camino que spikes/tcc_host.c en el PC. No contiene nada de ninguna ROM. */
#define _GNU_SOURCE
#include <android/log.h>
#include <dirent.h>
#include <dlfcn.h>
#include <errno.h>
#include <jni.h>
#include <sched.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/resource.h>
#include <sys/stat.h>
#include <time.h>
#include "libtcc.h"

#define LOG(...) __android_log_print(ANDROID_LOG_INFO, "snesspike", __VA_ARGS__)

/* La biblioteca del generador (src/generate.rs, módulo ffi). */
typedef struct SnesGenResult SnesGenResult;
SnesGenResult *snes_gen_generate(const uint8_t *rom, size_t rom_len, const char *profile_toml,
                                 const char *const *game_names, const char *const *game_texts, size_t game_count);
const char *snes_gen_error(const SnesGenResult *r);
size_t snes_gen_count(const SnesGenResult *r);
const char *snes_gen_name(const SnesGenResult *r, size_t i);
const char *snes_gen_text(const SnesGenResult *r, size_t i);
void snes_gen_free(SnesGenResult *r);

/* El runtime en modo biblioteca (runtime/snes_core.inc, runtime/headless.c). */
typedef struct { const char *name; void *addr; } SnesRtSymbol;
const SnesRtSymbol *snes_rt_symbols(void);
int snes_headless_main(int argc, char **argv);

static double now_ms(void) {
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return (double)t.tv_sec * 1e3 + (double)t.tv_nsec / 1e6;
}

static long peak_mib(void) {
    struct rusage ru;
    getrusage(RUSAGE_SELF, &ru);
    return ru.ru_maxrss / 1024;
}

/* Creates the folders of `path` (not the last part). */
static void make_parents(const char *path) {
    char buf[4096];
    char *p;
    snprintf(buf, sizeof buf, "%s", path);
    for (p = buf + 1; *p; p++) {
        if (*p == '/') {
            *p = 0;
            mkdir(buf, 0700);
            *p = '/';
        }
    }
}

static int write_text(const char *path, const char *text) {
    FILE *f;
    make_parents(path);
    if (!(f = fopen(path, "wb"))) return 0;
    fputs(text, f);
    return fclose(f) == 0;
}

/* ---- Generación ---- */

JNIEXPORT jstring JNICALL Java_io_github_alexperez7_snesspike_Native_generate(JNIEnv *env, jclass cls, jbyteArray rom,
        jstring toml, jobjectArray names, jobjectArray texts, jstring out_dir) {
    jsize n = (*env)->GetArrayLength(env, names), i;
    const char **cn = calloc((size_t)n + 1, sizeof *cn), **ct = calloc((size_t)n + 1, sizeof *ct);
    jbyte *bytes = (*env)->GetByteArrayElements(env, rom, NULL);
    jsize len = (*env)->GetArrayLength(env, rom);
    const char *ctoml = (*env)->GetStringUTFChars(env, toml, NULL);
    const char *out = (*env)->GetStringUTFChars(env, out_dir, NULL);
    char msg[1024], path[4096];
    double t0, t1;
    SnesGenResult *r;
    (void)cls;
    for (i = 0; i < n; i++) {
        cn[i] = (*env)->GetStringUTFChars(env, (jstring)(*env)->GetObjectArrayElement(env, names, i), NULL);
        ct[i] = (*env)->GetStringUTFChars(env, (jstring)(*env)->GetObjectArrayElement(env, texts, i), NULL);
    }
    t0 = now_ms();
    r = snes_gen_generate((const uint8_t *)bytes, (size_t)len, ctoml, cn, ct, (size_t)n);
    t1 = now_ms();
    if (snes_gen_error(r)) {
        snprintf(msg, sizeof msg, "ERROR: %s", snes_gen_error(r));
    } else {
        size_t k, lines = 0, size = 0;
        int ok = 1;
        for (k = 0; k < snes_gen_count(r); k++) {
            const char *text = snes_gen_text(r, k), *c;
            snprintf(path, sizeof path, "%s/%s", out, snes_gen_name(r, k));
            ok &= write_text(path, text);
            size += strlen(text);
            for (c = text; *c; c++) lines += *c == '\n';
        }
        snprintf(msg, sizeof msg, "%s: %zu ficheros, %zu líneas (%.1f MiB) en %.0f ms; memoria pico %ld MiB",
                 ok ? "OK" : "ERROR al escribir", snes_gen_count(r), lines, size / 1048576.0, t1 - t0, peak_mib());
    }
    LOG("generar: %s", msg);
    snes_gen_free(r);
    (*env)->ReleaseByteArrayElements(env, rom, bytes, JNI_ABORT);
    (*env)->ReleaseStringUTFChars(env, toml, ctoml);
    (*env)->ReleaseStringUTFChars(env, out_dir, out);
    free(cn);
    free(ct);
    return (*env)->NewStringUTF(env, msg);
}

/* ---- Medición ---- */

static char g_out[2048];
static char g_mode[64];
static double g_compile_ms, g_link_ms;
static int g_cpu;

static int cmp_double(const void *a, const void *b) {
    double x = *(const double *)a, y = *(const double *)b;
    return x < y ? -1 : x > y;
}

static uint64_t fnv_file(const char *path) {
    uint64_t h = 0xcbf29ce484222325ull;
    FILE *f = fopen(path, "rb");
    int c;
    if (!f) return 0;
    while ((c = fgetc(f)) != EOF) h = (h ^ (uint64_t)c) * 0x100000001b3ull;
    fclose(f);
    return h;
}

/* Al terminar la ejecución (el runtime acaba con exit): estadísticas y hashes a result.txt y a logcat. */
static void write_results(void) {
    char path[2300], line[512], all[4096] = "";
    static double t[100000];
    int n = 0, i;
    double sum = 0;
    FILE *f;
    const char *dumps[] = {"wram.bin", "vram.bin", "cgram.bin", "oam.bin"};
    fflush(NULL); /* los atexit van antes de que stdio vacíe sus ficheros: ft.txt incluido */
    snprintf(path, sizeof path, "%s/ft.txt", g_out);
    if ((f = fopen(path, "r"))) {
        double v;
        int first = 1;
        while (n < 100000 && fscanf(f, "%lf", &v) == 1) {
            if (first) { first = 0; continue; } /* el primer fotograma incluye el arranque */
            t[n++] = v / 1000.0;
            sum += v / 1000.0;
        }
        fclose(f);
    }
    qsort(t, (size_t)n, sizeof t[0], cmp_double);
    snprintf(line, sizeof line, "modo=%s nucleo=%d\ncompilar_ms=%.0f enlazar_ms=%.0f memoria_pico_mib=%ld\n", g_mode, g_cpu,
             g_compile_ms, g_link_ms, peak_mib());
    strcat(all, line);
    if (n) {
        snprintf(line, sizeof line, "fotogramas=%d media_ms=%.3f p99_ms=%.3f max_ms=%.3f\n", n, sum / n,
                 t[(int)(n * 0.99) > 0 ? (int)(n * 0.99) - 1 : 0], t[n - 1]);
        strcat(all, line);
    }
    for (i = 0; i < 4; i++) {
        snprintf(path, sizeof path, "%s/dump/%s", g_out, dumps[i]);
        snprintf(line, sizeof line, "fnv %s %016llx\n", dumps[i], (unsigned long long)fnv_file(path));
        strcat(all, line);
    }
    snprintf(path, sizeof path, "%s/result.txt", g_out);
    write_text(path, all);
    LOG("resultado:\n%s", all);
}

static void on_tcc_error(void *opaque, const char *msg) {
    (void)opaque;
    LOG("tcc: %s", msg);
}

/* Adds every .c of `dir` to `s`; returns how many, -1 on an error. */
static int add_dir(TCCState *s, const char *dir) {
    DIR *d = opendir(dir);
    struct dirent *e;
    char path[4096];
    int n = 0;
    if (!d) return 0;
    while ((e = readdir(d))) {
        size_t len = strlen(e->d_name);
        if (len < 3 || strcmp(e->d_name + len - 2, ".c")) continue;
        snprintf(path, sizeof path, "%s/%s", dir, e->d_name);
        if (tcc_add_file(s, path) < 0) { closedir(d); return -1; }
        n++;
    }
    closedir(d);
    return n;
}

/* mode 0: en memoria (tcc_relocate). mode 1 (plan B): tcc escribe <salida>/game.so y se carga con dlopen.
 * cpu >= 0 fija el hilo (y los que cree) a ese núcleo. No vuelve si la ejecución empieza: termina con exit. */
JNIEXPORT jstring JNICALL Java_io_github_alexperez7_snesspike_Native_bench(JNIEnv *env, jclass cls, jstring src_dir,
        jstring tcc_include, jstring lib_dir, jstring rom_path, jint frames, jint cpu, jint mode, jstring out_dir) {
    const char *src = (*env)->GetStringUTFChars(env, src_dir, NULL);
    const char *inc = (*env)->GetStringUTFChars(env, tcc_include, NULL);
    const char *libs = (*env)->GetStringUTFChars(env, lib_dir, NULL);
    const char *romp = (*env)->GetStringUTFChars(env, rom_path, NULL);
    const char *out = (*env)->GetStringUTFChars(env, out_dir, NULL);
    char path[4096], msg[4400], nframes[16], ft[2100], dump[2100];
    void (*init)(void) = NULL;
    TCCState *s;
    double t0, t1, t2;
    int n, m;
    (void)cls;
    snprintf(g_out, sizeof g_out, "%s", out);
    snprintf(g_mode, sizeof g_mode, "%s", mode ? "so+dlopen" : "memoria");
    g_cpu = cpu;
    if (cpu >= 0) {
        cpu_set_t set;
        CPU_ZERO(&set);
        CPU_SET(cpu, &set);
        if (sched_setaffinity(0, sizeof set, &set)) LOG("sched_setaffinity(%d): %s", cpu, strerror(errno));
    }
    t0 = now_ms();
    s = tcc_new();
    tcc_set_error_func(s, NULL, on_tcc_error);
    tcc_set_options(s, "-nostdinc -nostdlib"); /* antes del tipo de salida, que añade las rutas por defecto */
    tcc_set_output_type(s, mode ? TCC_OUTPUT_DLL : TCC_OUTPUT_MEMORY);
    tcc_add_sysinclude_path(s, inc);
    tcc_add_include_path(s, src);
    tcc_define_symbol(s, "SNES_RT_TICK", NULL);
    if (mode) {
        tcc_add_library_path(s, libs);
        tcc_add_library(s, "snesrt"); /* el .so del juego depende de libsnesrt.so, ya cargada */
    } else {
        const SnesRtSymbol *sym;
        for (sym = snes_rt_symbols(); sym->name; sym++) tcc_add_symbol(s, sym->name, sym->addr);
    }
    n = add_dir(s, src);
    snprintf(path, sizeof path, "%s/game", src);
    m = add_dir(s, path);
    t1 = now_ms();
    if (n <= 0 || m < 0) {
        snprintf(msg, sizeof msg, "ERROR: el C no compila con tcc (o no hay C en %s); ver logcat", src);
        goto done;
    }
    if (mode) {
        snprintf(path, sizeof path, "%s/game.so", out);
        make_parents(path);
        if (tcc_output_file(s, path) < 0) {
            snprintf(msg, sizeof msg, "ERROR: tcc no pudo escribir %s", path);
            goto done;
        }
        tcc_delete(s);
        s = NULL;
        {
            void *h = dlopen(path, RTLD_NOW);
            if (!h) {
                snprintf(msg, sizeof msg, "ERROR plan B: dlopen(%s): %s", path, dlerror());
                write_text(strcat(strcpy(dump, out), "/result.txt"), msg);
                goto done;
            }
            *(void **)&init = dlsym(h, "snes_rt_module_init");
        }
    } else {
        if (tcc_relocate(s) < 0) {
            snprintf(msg, sizeof msg, "ERROR: tcc_relocate falló (¿memoria ejecutable denegada?); ver logcat");
            write_text(strcat(strcpy(dump, out), "/result.txt"), msg);
            goto done;
        }
        *(void **)&init = tcc_get_symbol(s, "snes_rt_module_init");
    }
    t2 = now_ms();
    g_compile_ms = t1 - t0;
    g_link_ms = t2 - t1;
    LOG("%s: %d ficheros compilados en %.0f ms, enlazados en %.0f ms; memoria pico %ld MiB", g_mode, n + m,
        g_compile_ms, g_link_ms, peak_mib());
    if (!init) {
        snprintf(msg, sizeof msg, "ERROR: no está snes_rt_module_init en el C compilado");
        goto done;
    }
    init();
    snprintf(path, sizeof path, "%s/report.txt", out);
    make_parents(path);
    if (!freopen(path, "w", stdout)) LOG("no se pudo redirigir stdout a %s", path);
    snprintf(nframes, sizeof nframes, "%d", (int)frames);
    snprintf(ft, sizeof ft, "%s/ft.txt", out);
    snprintf(dump, sizeof dump, "%s/dump", out);
    mkdir(dump, 0700);
    atexit(write_results);
    {
        char *argv[] = {"snes", (char *)romp, "--frames", nframes, "--frame-times", ft, "--dump", dump, NULL};
        int code = snes_headless_main(8, argv);
        snprintf(msg, sizeof msg, "ERROR: el runtime volvió con el código %d", code);
    }
done:
    LOG("medir: %s", msg);
    (*env)->ReleaseStringUTFChars(env, src_dir, src);
    (*env)->ReleaseStringUTFChars(env, tcc_include, inc);
    (*env)->ReleaseStringUTFChars(env, lib_dir, libs);
    (*env)->ReleaseStringUTFChars(env, rom_path, romp);
    (*env)->ReleaseStringUTFChars(env, out_dir, out);
    return (*env)->NewStringUTF(env, msg);
}
