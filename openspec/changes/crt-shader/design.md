## Context

Ver `proposal.md`. Estado actual (`runtime/play.c`, `present()`):

- La imagen del fotograma (`fb`, ya con menú y contador dibujados encima por el propio programa) se sube a una textura `RGB24` y se dibuja con `SDL_RenderCopy` en el rectángulo que calcula `view_rect()` (aspecto, escalado entero o relleno, recorte).
- `sharp` es una ampliación por vecino más cercano hasta un factor entero seguido de una reducción bilineal; las scanlines y la rejilla son una textura RGBA (`overlay_build`) mezclada encima.
- `--present-shot` dibuja lo mismo con el renderizador por software en un PNG, y las pruebas de `src/headless.rs` lo usan para comprobar el diseño sin ventana.
- El motor no tiene dependencias fuera de `serde` y `toml`; el juego construido solo enlaza SDL2.

## Goals / Non-Goals

**Goals:**
- Un aspecto de CRT creíble (haz, máscara, curvatura, bloom) activable por opción, apagado por defecto.
- Sin dependencias nuevas ni cambios en cómo se construye el juego.
- Que el camino actual siga siendo el respaldo y el valor por defecto, y que nada falle si falta OpenGL.

**Non-Goals:**
- Un sistema de shaders configurables por el usuario (se valorará después de tener este).
- Otras API gráficas ni composición con el menú o el contador en la GPU: el menú sigue dibujándose en `fb`.

## Decisions

**1. OpenGL 3.3 core cargado a mano con `SDL_GL_GetProcAddress`.** Unas veinte funciones (`glCreateShader`, `glUniform*`, `glTexImage2D`...) obtenidas al crear el contexto. *Alternativas:* una biblioteca de carga (GLEW/glad) añade una dependencia y un paso de construcción; `SDL_Renderer` con el controlador `opengl` y `SDL_GL_BindTexture` obliga a convivir con el estado interno del renderizador de SDL y es frágil.

**2. Ruta aparte, no mezclada con el `SDL_Renderer`.** Con `shader = crt` la ventana se crea con `SDL_WINDOW_OPENGL` y no hay `SDL_Renderer`; `present_gl()` sube `fb` a una textura (vecino más cercano), dibuja un cuadrilátero en el rectángulo de `view_rect()` y hace `SDL_GL_SwapWindow`. Al cambiar `shader` en el menú se destruye y se vuelve a crear la ventana (la posición y el modo de pantalla completa se conservan); la emulación no se entera. *Alternativa:* crear siempre la ventana con OpenGL y usar `SDL_Renderer` encima: los controladores de Direct3D no admiten ventanas con contexto de OpenGL.

**3. Una pasada de fragmentos a la resolución de salida.** El fragment shader recibe el tamaño de la imagen y el del rectángulo de destino y hace, por este orden: curvatura (deformación de las coordenadas, con borde negro fuera), muestreo nítido (coordenada ajustada al centro del píxel con un suavizado de ancho un píxel de pantalla: con factor entero equivale a `nearest`), bloom (media ponderada de pocos vecinos, barata), haz de scanline (campana en la distancia vertical al centro de la línea, ensanchada por la luminosidad), máscara de fósforo (rejilla de aperturas RGB con la fuerza de `crt_mask`) y compensación de brillo. Sin cadena de pasadas: una sola es suficiente para el aspecto buscado y mantiene el coste constante.

**4. Sin efectos, mismo resultado que `nearest`.** Con `crt_mask = crt_curve = crt_bloom = 0`, `scanlines = 0` y factor entero, el shader debe dar la misma imagen que `nearest`. Es la forma de comprobar la ruta sin ojos: se compara byte a byte.

**5. Respaldo silencioso.** Cualquier fallo (sin `SDL_WINDOW_OPENGL`, contexto menor que 3.3, shader que no compila o no enlaza) imprime un aviso una vez y vuelve al camino del `SDL_Renderer` con `shader = off` en efecto; `config.ini` conserva el valor pedido.

**6. Opciones.** `shader` (`off|crt`), `crt_mask`, `crt_curve`, `crt_bloom` (0-100) en `play_cfg.h` con las demás; `scanlines` se reutiliza para el haz y `grid` no tiene efecto con el shader. Valores por defecto con el shader activado: haz 40, máscara 30, curvatura 15, bloom 20 (se ajustarán a ojo).

## Risks / Trade-offs

- [El contexto de OpenGL se fija al crear la ventana, así que cambiar de camino recrea la ventana y puede parpadear] → solo ocurre al cambiar `shader`; se conservan posición y modo.
- [Controladores de GPU con OpenGL pobre (equipos antiguos, máquinas virtuales)] → respaldo silencioso; el valor por defecto es `off`.
- [Las pruebas de la ruta de OpenGL necesitan un contexto, que puede no haber en CI] → `--present-shot` con `shader = crt` dibuja en una ventana oculta y lee los píxeles; la prueba se omite si no se consigue el contexto, igual que las que necesitan la ROM. La lógica fuera del shader (opciones, filas del menú, textos, respaldo) se prueba sin GL.
- [Coste con GPU integrada] → una sola pasada; el bloom usa pocas muestras. Medir con `--stats` y anotarlo aquí.
- [El código del shader es lo más fácil de copiar de otros proyectos] → se escribe desde cero a partir de las fórmulas (campana gaussiana, rejilla periódica), sin copiar presets de otros emuladores.

## Open Questions

- ¿Basta `crt_curve` o conviene separar curvatura y viñeta?
- ¿Se activa el shader por defecto en un futuro si el respaldo funciona de forma fiable? (Hoy: no.)
