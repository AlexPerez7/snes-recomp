# Spec Delta

## ADDED Requirements

### Requirement: Imagen en Android
En Android el frontal SHALL presentar la imagen con `SDL_Renderer` o con OpenGL ES 3 y SHALL adaptar la relación de aspecto y el relleno a la pantalla del dispositivo, incluida la pantalla ancha cuando esté activada.

#### Scenario: Pantalla del móvil
- **WHEN** la app corre en un teléfono con pantalla apaisada de otra proporción que 8:7
- **THEN** la imagen se escala sin deformarse y el resto se rellena o se amplía con la pantalla ancha según la opción
