## 1. Opciones y respaldo (sin OpenGL)

- [ ] 1.1 `shader`, `crt_mask`, `crt_curve`, `crt_bloom` en `play_cfg.h` (tabla, `config.ini`, `--opt`, `--print-config`); pruebas del INI con claves nuevas, valores por defecto y claves desconocidas conservadas
- [ ] 1.2 Filas nuevas en la pestaña Image del menú F1 y textos `{inglés, español}` en `play_i18n.h`; las pruebas que miden cada cadena contra su hueco pasan
- [ ] 1.3 Decisión de camino al arrancar y al cambiar `shader`: función pura que, dada la opción y si OpenGL está disponible, dice si se usa el shader o el respaldo; prueba del respaldo

## 2. Ruta con OpenGL

- [ ] 2.1 `runtime/play_gl.h`: carga de funciones con `SDL_GL_GetProcAddress`, creación y comprobación del contexto 3.3, compilación y enlace del programa con el registro de errores
- [ ] 2.2 `present_gl()`: textura de `fb`, cuadrilátero en el rectángulo de `view_rect()`, uniformes (tamaños, opciones), `SDL_GL_SwapWindow`
- [ ] 2.3 Fragment shader escrito desde cero: curvatura, muestreo nítido, bloom, haz, máscara
- [ ] 2.4 Recrear la ventana al cambiar `shader` conservando posición y pantalla completa

## 3. Pruebas

- [ ] 3.1 `--present-shot` con `shader = crt`: ventana oculta, lectura de píxeles a PNG; se omite sin contexto
- [ ] 3.2 Sin efectos y con factor entero, la imagen del shader es idéntica a `nearest` (byte a byte)
- [ ] 3.3 Cada efecto, por separado, cambia la imagen en el sentido esperado (haz y máscara oscurecen, curvatura deja borde negro, bloom suaviza)
- [ ] 3.4 Respaldo: sin OpenGL 3.3 se usa el `SDL_Renderer` y el aviso sale una sola vez

## 4. Documentación y medidas

- [ ] 4.1 Tabla de opciones del `README.md` y `CLAUDE.md` del motor
- [ ] 4.2 Medir el coste con `--stats` (GPU integrada) y anotarlo en `design.md`; ajustar los valores por defecto del efecto a ojo con capturas
- [ ] 4.3 Probar a mano en una partida real con teclado y mando; `plan.md` de `alttp-recomp`, punto 2
