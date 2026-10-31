## ADDED Requirements

### Requirement: Compilador portátil incluido en el paquete
Antes de ofrecer una descarga, el programa SHALL buscar un compilador portátil incluido junto al ejecutable (carpeta `toolchain/` del paquete) y usarlo si no hay un compilador local. El orden SHALL ser: compilador local, compilador del paquete, descarga con consentimiento. Con un compilador local o del paquete MUST NOT abrir ninguna conexión de red.

#### Scenario: Paquete con compilador y máquina sin compilador
- **WHEN** el paquete lleva TinyCC y la máquina no tiene compilador de C
- **THEN** el juego se construye con el TinyCC del paquete, sin descargar nada ni preguntar

#### Scenario: Compilador local presente
- **WHEN** la máquina tiene Visual Studio Build Tools, gcc o clang
- **THEN** se usa ese y no el del paquete

### Requirement: Informar sin depender de la consola
El constructor SHALL entregar sus mensajes de progreso, error y consentimiento a un receptor que puede ser la consola o la ventana del lanzador, con el mismo contenido y los mismos códigos de salida en ambos.

#### Scenario: Mismo error en consola y ventana
- **WHEN** falta SDL2 al construir
- **THEN** el mensaje y el código de salida 3 son los mismos desde la terminal y desde el lanzador
