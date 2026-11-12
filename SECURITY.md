# Security policy / Política de seguridad

## Reporting a vulnerability / Cómo avisar

Please **do not open a public issue** for a security problem. Use GitHub's private reporting instead:
**Security → Report a vulnerability** in this repository
(<https://github.com/AlekayDevs/snes-recomp/security/advisories/new>).

No abras un issue público para un problema de seguridad: usa el aviso privado de GitHub (enlace de arriba).

Useful details: version, system, what you did, what happened, and whether it needs a crafted ROM, save file, save state, profile or network position.

## What counts / Qué cuenta

- The **recompiler** (`snes-recomp`) or the runtime running code it should not when given a crafted ROM, save, save state or game profile.
- The launcher's **update mechanism** (version check, download, SHA-256 verification, replacing files) and the Android app's installer.
- Anything that sends a ROM, saves or personal data off your device. The only network connection is the optional version check.

## Supported versions / Versiones con soporte

Only the latest release of each game built with the engine, for example [`alttp-recomp`](https://github.com/AlekayDevs/alttp-recomp/releases/latest).

## Out of scope / Fuera de alcance

Problems specific to a game's profile or its own C code go to that game's repository.
