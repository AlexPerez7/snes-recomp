# Spec Delta

## Purpose

Ofrecer el generador como biblioteca que convierte los bytes de una ROM y un perfil de juego en las fuentes C del juego sin usar el sistema de archivos. Así lo pueden usar tanto `snes-port` en el PC como una app que arma el juego dentro del dispositivo del usuario.

## ADDED Requirements

### Requirement: Generar fuentes desde bytes
La biblioteca del generador SHALL recibir los bytes de la ROM y un perfil de juego (configuración y C propio) y devolver la lista de fuentes del juego (nombre relativo y texto). Esa lista MUST incluir los `rom_*.c`, `snes_funcs.h`, `snes_entry.c` y los ficheros `game/` del perfil, sin leer ni escribir ficheros.

#### Scenario: Fuentes completas
- **WHEN** se llama a la biblioteca con los bytes de la ROM real y el perfil de ALttP
- **THEN** devuelve las mismas fuentes, con los mismos nombres y textos, que `codegen --all` escribe en disco con ese perfil

#### Scenario: Sin acceso a disco
- **WHEN** se llama a la biblioteca desde un proceso cuyo directorio actual no permite escribir
- **THEN** la generación termina igual y no crea ningún fichero

### Requirement: Errores de generación
Si el CRC32 de la ROM no coincide con el del perfil, o un gancho del perfil llama a una función que ningún `.c` del perfil define, la biblioteca SHALL devolver un error, sin devolver fuentes parciales. En el caso del CRC32, el error nombra el CRC32 hallado y el esperado. En el caso del gancho, usa el mismo texto que da hoy `snes-port`.

#### Scenario: ROM de otro juego
- **WHEN** se pasan los bytes de una ROM cuyo CRC32 no es el del perfil
- **THEN** se devuelve un error que nombra el CRC32 hallado y el esperado, y ninguna fuente

#### Scenario: Gancho sin función
- **WHEN** el perfil declara un `[[hook]]` cuya función no aparece en su C
- **THEN** se devuelve el mismo error que hoy da `snes-port` para ese caso

### Requirement: Una sola generación para PC y dispositivo
`snes-port` y `codegen --all` SHALL producir sus fuentes a través de la misma biblioteca, de modo que una mejora del generador llegue igual a todas las plataformas.

#### Scenario: Mismo C en disco
- **WHEN** se ejecuta `codegen --all` antes y después de este cambio con la misma ROM y el mismo perfil
- **THEN** los ficheros escritos son idénticos

### Requirement: Compila para Android arm64
La biblioteca del generador SHALL compilar para `aarch64-linux-android` como biblioteca dinámica con una interfaz C. Esa interfaz recibe un búfer de ROM y el perfil, y entrega las fuentes y un mensaje de error.

#### Scenario: Compilación cruzada
- **WHEN** se compila el crate con el objetivo `aarch64-linux-android` usando el enlazador del NDK
- **THEN** se obtiene un `.so` que exporta la interfaz C de generación
