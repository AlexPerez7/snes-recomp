/* The text of the menu in Spanish: original wording written for this program (nothing here comes from a ROM). The menu
 * is drawn through tr(); the keys and values of config.ini, the comments of the file and the console never are.
 * Strings are UTF-8 (see the font in play_ui.h). */
#ifndef PLAY_I18N_H
#define PLAY_I18N_H

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
    /* Game */
    {"Language", "Idioma"}, {"Resume", "Continuar"}, {"Save game now", "Guardar partida ahora"}, {"Restore all defaults", "Restaurar todo"},
    {"Quit", "Salir"},
    /* notices */
    {"Image options restored", "Vídeo restaurado"}, {"Audio options restored", "Audio restaurado"},
    {"Everything restored to the defaults", "Todo restaurado"},
    {"Press a key or a controller button", "Pulsa una tecla o un botón"}, {"Saved game written", "Partida guardada"},
    {"Used by P%d %s: Enter replaces, Esc cancels", "J%d usa %s: Intro cambia"},
    {"Enter restores everything, Esc cancels", "Intro restaura todo, Esc cancela"},
    {"Enter quits the game, Esc cancels", "Intro cierra el juego, Esc cancela"},
    /* the help line */
    {"L/R: tabs   A: select   B: close", "L/R: pestañas  A: ok  B: salir"},
    {"Press the key or button   Esc: cancel", "Pulsa tecla o botón   Esc: cancelar"},
    {"Q/W: tabs   Enter: select   Esc: close", "Q/W: pestañas  Intro: ok  Esc: salir"},
};
#define I18N_COUNT ((int)(sizeof i18n_table / sizeof i18n_table[0]))

static int ui_es; /* 1 when the menu is drawn in Spanish */

/* `en` in the language of the menu; the English text itself if there is no translation. */
static const char *tr(const char *en) {
    int i;
    if (!ui_es) return en;
    for (i = 0; i < I18N_COUNT; i++) if (!strcmp(i18n_table[i].en, en)) return i18n_table[i].es;
    return en;
}

#endif
