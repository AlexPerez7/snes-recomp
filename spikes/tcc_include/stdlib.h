/* stdlib.h mínima para TinyCC en un dispositivo sin cabeceras del sistema. Código propio. */
#ifndef SNES_TCC_STDLIB_H
#define SNES_TCC_STDLIB_H
#include <stddef.h>
void *malloc(size_t n);
void *calloc(size_t n, size_t size);
void *realloc(void *p, size_t n);
void free(void *p);
void exit(int code);
int atoi(const char *s);
long strtol(const char *s, char **end, int base);
unsigned long strtoul(const char *s, char **end, int base);
#endif
