## ADDED Requirements

### Requirement: Opciones de juego en el perfil
El formato del perfil SHALL admitir secciones `[[option]]` con `key` (letras, cifras o `_`, hasta 31 caracteres), `label` (texto del menú), `names` (lista de dos o más nombres distintos) y `default` (uno de ellos). El programa MUST rechazar con un error que nombre el perfil y la clave una opción con `key` repetida, `names` con menos de dos entradas o repetidas, o `default` que no esté en `names`.

#### Scenario: Opción válida
- **WHEN** un perfil trae `[[option]] key="language" label="Language" names=["en","es"] default="en"`
- **THEN** el perfil se carga y la opción queda disponible

#### Scenario: Valor por defecto fuera de la lista
- **WHEN** `default = "fr"` y `names = ["en","es"]`
- **THEN** el perfil se rechaza con un error que cita `language` y el fichero

#### Scenario: Clave repetida
- **WHEN** dos `[[option]]` usan `key = "language"`
- **THEN** el perfil se rechaza con un error que cita la clave
