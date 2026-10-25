# project-publication Specification

## Purpose
TBD - created by archiving change split-engine-game. Update Purpose after archive.

## Requirements

### Requirement: Licencia, aviso legal y guía de contribución
Cada repositorio público del proyecto SHALL tener un `LICENSE` MIT en su raíz, un `README.md` que diga qué es, que no contiene ninguna ROM ni recurso del juego, que no está afiliado a Nintendo y que cada persona debe aportar su propia copia legal, y un `CONTRIBUTING.md` que explique cómo proponer cambios, las reglas de código propio (clean-room) y la prohibición de subir ROMs o material derivado de ellas.

#### Scenario: Ficheros presentes
- **WHEN** se mira la raíz de `snes-recomp` y de `alttp-recomp`
- **THEN** ambas tienen `LICENSE`, `README.md` y `CONTRIBUTING.md`, y el README contiene el aviso legal

#### Scenario: Sin texto de licencia pendiente
- **WHEN** se busca en los README la frase «licencia: sin elegir»
- **THEN** no aparece

### Requirement: Sin ROMs ni material derivado en el árbol ni en el historial
Un repositorio del proyecto MUST NOT contener, en ningún commit, ROMs, partidas guardadas, binarios ni C generado, ni secuencias de bytes de la ROM (en bruto o como literales hexadecimales). El subcomando `snes-recomp scan <rom> <carpeta>` SHALL buscar esas secuencias con la misma comprobación que usa `snes-port` al construir y terminar con código distinto de cero si encuentra alguna, indicando el fichero.

#### Scenario: Carpeta limpia
- **WHEN** se ejecuta `scan` sobre una carpeta sin bytes de la ROM
- **THEN** termina con código 0

#### Scenario: Bytes de la ROM
- **WHEN** un fichero de la carpeta contiene 32 bytes consecutivos de la ROM, en bruto o como literales hexadecimales de C
- **THEN** termina con código distinto de cero y nombra el fichero

#### Scenario: Historial
- **WHEN** se extrae cada commit de un repositorio y se ejecuta `scan` sobre el resultado
- **THEN** ninguno encuentra bytes de la ROM

### Requirement: Sin ficheros de trabajo rastreados
Un repositorio del proyecto MUST NOT rastrear registros de compilación ni salidas de trabajo (`outb.log`, `outbt.log`, `out/`), y su `.gitignore` SHALL excluir ROMs, partidas guardadas, `out/` y los registros.

#### Scenario: Registros fuera
- **WHEN** se listan los ficheros rastreados
- **THEN** no hay `*.log` en la raíz ni nada bajo `out/`
