## MODIFIED Requirements

### Requirement: Teclas de control
El ejecutable SHALL ofrecer F11 para pantalla completa, Escape para abrir el menú (con el menú abierto, Escape lo cierra), P para pausar, Tab (mantenida) para avance rápido y F10 para guardar una captura PNG del fotograma actual. Escape MUST NOT cerrar el programa. Cerrar la ventana SHALL seguir terminando el programa.

#### Scenario: Pausa
- **WHEN** se pulsa P
- **THEN** la emulación se detiene y la ventana conserva el último fotograma hasta pulsar P otra vez

#### Scenario: Escape abre el menú
- **WHEN** se pulsa Escape con el menú cerrado
- **THEN** el menú se abre, la emulación se pausa y el programa sigue en marcha

#### Scenario: Escape no sale
- **WHEN** se pulsa Escape dos veces seguidas, una para abrir y otra para cerrar el menú
- **THEN** el juego sigue en marcha tras la segunda y la configuración se ha guardado

#### Scenario: Cerrar la ventana
- **WHEN** el usuario cierra la ventana
- **THEN** el programa termina sin pedir confirmación, guardando la SRAM como al salir por el menú
