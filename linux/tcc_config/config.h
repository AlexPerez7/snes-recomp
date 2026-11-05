/* config.h de TinyCC para compilarlo como biblioteca (libtcc.so) de un solo fuente, igual que el de Android: sin rutas del
 * sistema (el lanzador le da las suyas) y sin predefiniciones incrustadas (lee tccdefs.h de assets/tcc_include). Código propio. */
#define TCC_VERSION "0.9.28rc"
#define CONFIG_TCCDIR "/nonexistent"
#define CONFIG_TCC_PREDEFS 0
#define CONFIG_TCC_SEMLOCK 0
