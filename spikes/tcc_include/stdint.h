/* stdint.h mínima para compilar con TinyCC el C del juego en un dispositivo sin cabeceras del sistema (Android).
 * Solo para LP64 (arm64-v8a, x86_64): char 8, short 16, int 32, long y punteros 64 bits. Código propio. */
#ifndef SNES_TCC_STDINT_H
#define SNES_TCC_STDINT_H
typedef signed char int8_t;
typedef short int16_t;
typedef int int32_t;
typedef long int64_t;
typedef unsigned char uint8_t;
typedef unsigned short uint16_t;
typedef unsigned int uint32_t;
typedef unsigned long uint64_t;
typedef long intptr_t;
typedef unsigned long uintptr_t;
typedef long intmax_t;
typedef unsigned long uintmax_t;
#define INT8_MIN (-128)
#define INT16_MIN (-32767 - 1)
#define INT32_MIN (-2147483647 - 1)
#define INT64_MIN (-9223372036854775807L - 1)
#define INT8_MAX 127
#define INT16_MAX 32767
#define INT32_MAX 2147483647
#define INT64_MAX 9223372036854775807L
#define UINT8_MAX 255
#define UINT16_MAX 65535
#define UINT32_MAX 4294967295U
#define UINT64_MAX 18446744073709551615UL
#define SIZE_MAX UINT64_MAX
#define INT8_C(v) v
#define INT16_C(v) v
#define INT32_C(v) v
#define INT64_C(v) v##L
#define UINT8_C(v) v
#define UINT16_C(v) v
#define UINT32_C(v) v##U
#define UINT64_C(v) v##UL
#endif
