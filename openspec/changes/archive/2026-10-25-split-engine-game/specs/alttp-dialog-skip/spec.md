## REMOVED Requirements

### Requirement: Diálogos acelerados con un botón
**Reason**: es contenido de un juego concreto y el juego pasa a su propio repositorio (`alttp-recomp`); el motor no debe describir comportamientos de ALttP.
**Migration**: el comportamiento (mantener R acelera los mensajes) se documenta en el README de `alttp-recomp` y lo comprueba su `scripts/check.ps1`.

### Requirement: Mejora desactivable
**Reason**: igual que el requisito anterior, pertenece al repositorio del juego.
**Migration**: `--game-opt dialog_skip=0` se documenta en el README de `alttp-recomp`; el mecanismo `--game-opt` sigue en el motor (`game-hooks`).
