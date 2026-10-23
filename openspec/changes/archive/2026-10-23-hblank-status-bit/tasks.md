## 1. H-blank

- [x] 1.1 Devolver el bit 6 de `$4212` en `runtime/headless.c` según la posición en la línea; verificar con un test que lo lee a 0 al empezar el fotograma y espera hasta verlo a 1, y adaptar el test de vblank para que no dependa del bit 6
- [x] 1.2 Regenerar ALttP y comprobar que `$2100` sale de `80` tras el fotograma ~1054 y que las capturas de 1500 y 3000 fotogramas muestran las pantallas de historia de la intro; `cargo test` completo en verde
- [x] 1.3 Actualizar la memoria de proyecto con la causa real del forced blank

## Resultado

Con el bit 6 modelado, ALttP sale del forced blank (`$2100` pasa a `0E`/`0F`) y la intro muestra «Long ago, in the beautiful kingdom of Hyrule…» (fotograma 1500) y «Golden Land… But no one ever returned. One day evil» (fotograma 3000). Los avisos del informe son de ventanas, que el renderizador aún no aplica.
