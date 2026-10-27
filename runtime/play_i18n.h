/* The text of the menu in Spanish: original wording written for this program (nothing here comes from a ROM). The menu
 * is drawn through tr(); the keys and values of config.ini, the comments of the file and the console never are.
 * Strings are UTF-8 (see the font in play_ui.h). */
#ifndef PLAY_I18N_H
#define PLAY_I18N_H

#include <stdio.h>
#include <string.h>

typedef struct { const char *en, *es; } I18n;

static const I18n i18n_table[] = {
    /* tabs and title */
    {"OPTIONS", "OPCIONES"}, {"Image", "Vídeo"}, {"Audio", "Audio"}, {"Controls", "Mandos"}, {"Game", "Juego"},
    /* the rows of the Image tab */
    {"Fullscreen", "Pant. completa"}, {"VSync", "VSync"}, {"Aspect", "Aspecto"}, {"Scaling", "Escalado"}, {"Filter", "Filtro"},
    {"Overscan", "Overscan"}, {"Wide", "Ancho"}, {"Scanlines", "Líneas CRT"}, {"Grid", "Rejilla"}, {"FPS counter", "Mostrar FPS"},
    {"Restore defaults", "Restaurar valores"},
    /* values */
    {"On", "Sí"}, {"Off", "No"}, {"integer", "entero"}, {"fit", "ajustado"}, {"free", "libre"}, {"nearest", "cercano"},
    {"bilinear", "bilineal"}, {"sharp", "nítido"}, {"Default", "Por defecto"},
    /* Audio and Controls */
    {"Volume", "Volumen"}, {"Output device", "Salida"}, {"Controls of", "Controles de"}, {"Player %d", "Jugador %d"},
    {"Keyboard", "Teclado"}, {"Controller", "Mando"}, {"Up", "Arriba"}, {"Down", "Abajo"}, {"Left", "Izquierda"}, {"Right", "Derecha"},
    /* Game */
    {"Fast forward", "Avance rápido"}, {"max", "máx."}, {"Language", "Idioma"}, {"Resume", "Continuar"}, {"Save game now", "Guardar partida ahora"}, {"Restore all defaults", "Restaurar todo"},
    {"Quit", "Salir"},
    /* notices */
    {"Image options restored", "Vídeo restaurado"}, {"Audio options restored", "Audio restaurado"},
    {"Everything restored to the defaults", "Todo restaurado"},
    {"Press a key or a controller button", "Pulsa una tecla o un botón"}, {"Saved game written", "Partida guardada"},
    {"Used by P%d %s: Enter replaces, Esc cancels", "J%d usa %s: Intro cambia"},
    {"Enter restores everything, Esc cancels", "Intro restaura todo, Esc cancela"},
    {"Enter quits the game, Esc cancels", "Intro cierra el juego, Esc cancela"},
    /* save states */
    {"State saved (slot %d)", "Estado guardado (ranura %d)"}, {"State loaded (slot %d)", "Estado cargado (ranura %d)"},
    {"Slot %d", "Ranura %d"}, {"Slot %d is empty", "La ranura %d está vacía"}, {"Cannot save", "No se puede guardar"},
    {"Cannot load", "No se puede cargar"}, {"Cannot read the state", "No se puede leer el estado"},
    {"Cannot write the state", "No se puede escribir el estado"},
    {"Save states are not available in this game", "Este juego no admite estados"},
    {"an interrupt is running", "hay una interrupción en curso"},
    {"the game is switching execution contexts", "el juego cambia de contexto"},
    {"another execution context is running", "corre otro contexto"},
    {"not a state of this version", "estado de otra versión"}, {"state of another game", "estado de otro juego"},
    {"state taken at another stack depth", "estado tomado en otro punto"}, {"out of memory", "sin memoria"},
    /* the help line */
    {"L/R: tabs   A: select   B: close", "L/R: pestañas  A: ok  B: salir"},
    {"Press the key or button   Esc: cancel", "Pulsa tecla o botón   Esc: cancelar"},
    {"Q/W: tabs   Enter: select   Esc: close", "Q/W: pestañas  Intro: ok  Esc: salir"},
};
#define I18N_COUNT ((int)(sizeof i18n_table / sizeof i18n_table[0]))

static int ui_es; /* 1 when the menu is drawn in Spanish */

/* The keys and controller buttons of the Controls tab as the player reads them. `sdl` is the name SDL gives (what
 * config.ini keeps, so it never changes); the menu shows `en` or `es`. A name that is not here (the letters, the digits,
 * an odd key) is shown as SDL spells it. */
typedef struct { const char *sdl, *en, *es; } BindName;
static const BindName bind_names[] = {
    {"Up", "Up Arrow", "Flecha ↑"}, {"Down", "Down Arrow", "Flecha ↓"},
    {"Left", "Left Arrow", "Flecha ←"}, {"Right", "Right Arrow", "Flecha →"},
    {"Return", "Enter", "Intro"}, {"Escape", "Esc", "Esc"}, {"Space", "Space", "Espacio"}, {"Tab", "Tab", "Tab"},
    {"Backspace", "Backspace", "Retroceso"}, {"Delete", "Delete", "Supr"}, {"Insert", "Insert", "Insert"},
    {"Home", "Home", "Inicio"}, {"End", "End", "Fin"}, {"PageUp", "Page Up", "Re Pág"}, {"PageDown", "Page Down", "Av Pág"},
    {"CapsLock", "Caps Lock", "Bloq May."},
    {"Left Shift", "L Shift", "Mayús izq."}, {"Right Shift", "R Shift", "Mayús der."},
    {"Left Ctrl", "L Ctrl", "Ctrl izq."}, {"Right Ctrl", "R Ctrl", "Ctrl der."},
    {"Left Alt", "L Alt", "Alt izq."}, {"Right Alt", "R Alt", "Alt der."},
    /* controller buttons (SDL_GameControllerGetStringForButton) */
    {"a", "A", "A"}, {"b", "B", "B"}, {"x", "X", "X"}, {"y", "Y", "Y"},
    {"back", "Back", "Atrás"}, {"guide", "Guide", "Guía"}, {"start", "Start", "Start"},
    {"leftstick", "L3", "L3"}, {"rightstick", "R3", "R3"}, {"leftshoulder", "LB", "LB"}, {"rightshoulder", "RB", "RB"},
    {"dpup", "D-pad ↑", "Cruceta ↑"}, {"dpdown", "D-pad ↓", "Cruceta ↓"},
    {"dpleft", "D-pad ←", "Cruceta ←"}, {"dpright", "D-pad →", "Cruceta →"},
    {"touchpad", "Touchpad", "Táctil"}, {"misc1", "Misc", "Extra"},
};
#define BIND_NAME_COUNT ((int)(sizeof bind_names / sizeof bind_names[0]))

/* The name of a key or button for the menu, from the name SDL gives it. The keypad keys ("Keypad 5") become "Num 5". */
static const char *bind_label(const char *sdl, char *buf, size_t n) {
    int i;
    for (i = 0; i < BIND_NAME_COUNT; i++)
        if (!strcmp(bind_names[i].sdl, sdl)) return ui_es ? bind_names[i].es : bind_names[i].en;
    if (!strncmp(sdl, "Keypad ", 7)) { snprintf(buf, n, "Num %s", sdl + 7); return buf; }
    return sdl;
}

/* `en` in the language of the menu; the English text itself if there is no translation. */
static const char *tr(const char *en) {
    int i;
    if (!ui_es) return en;
    for (i = 0; i < I18N_COUNT; i++) if (!strcmp(i18n_table[i].en, en)) return i18n_table[i].es;
    return en;
}

#endif
