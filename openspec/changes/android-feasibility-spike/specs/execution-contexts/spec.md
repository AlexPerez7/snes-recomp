# Spec Delta

## ADDED Requirements

### Requirement: Contextos sobre hilos
En sistemas sin fibras ni `ucontext` (Android, o cuando se compila con `SNES_RT_THREAD_CONTEXTS`), el runtime SHALL ejecutar cada contexto sobre un hilo propio del que solo uno corre a la vez, con el mismo resultado que con fibras o `ucontext`.

#### Scenario: Mismo estado con hilos
- **WHEN** se ejecutan 1200 fotogramas de ALttP con `--press START@1100-1110` (la intro cambia de contexto) con contextos sobre `ucontext` y sobre hilos
- **THEN** los volcados de WRAM, VRAM, CGRAM, OAM y registros son idénticos

#### Scenario: Android
- **WHEN** el runtime se compila para Android
- **THEN** usa los contextos sobre hilos sin que haya que definir nada
