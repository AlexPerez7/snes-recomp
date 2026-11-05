/* stdio.h mínima para compilar con TinyCC el C del juego en un dispositivo sin cabeceras del sistema.
 * Solo lo que usa el C del juego; los símbolos los resuelve libtcc con dlsym en la libc. Código propio. */
#ifndef SNES_TCC_STDIO_H
#define SNES_TCC_STDIO_H
#include <stddef.h>
#include <stdarg.h>
typedef struct snes_tcc_file FILE;
extern FILE *stderr;
#define stderr stderr
FILE *fopen(const char *path, const char *mode);
int fclose(FILE *f);
size_t fread(void *p, size_t size, size_t n, FILE *f);
size_t fwrite(const void *p, size_t size, size_t n, FILE *f);
int fprintf(FILE *f, const char *fmt, ...);
int snprintf(char *s, size_t n, const char *fmt, ...);
int fputs(const char *s, FILE *f);
char *fgets(char *s, int n, FILE *f);
#endif
