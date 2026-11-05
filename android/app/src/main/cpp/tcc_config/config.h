/* config.h de TinyCC para compilarlo con el NDK como un solo fuente (libtcc.c con ONE_SOURCE), en lugar del que genera
 * ./configure: sin rutas del sistema (la app le da las suyas), sin predefiniciones incrustadas (lee tccdefs.h de su
 * carpeta de cabeceras) y sin semáforos entre procesos. Código propio. */
#define TCC_VERSION "0.9.28rc"
#define CONFIG_TCCDIR "/nonexistent"
#define CONFIG_TCC_PREDEFS 0
#define CONFIG_TCC_SEMLOCK 0
