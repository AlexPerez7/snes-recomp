/* A Link to the Past (USA): hold R during a message box and the text appears three times faster.
 *
 * 0E:CA6C runs once per frame while a message box is up. Its state byte $1CD5 counts the frames until the next
 * character (it is reloaded from $1CD6, which is 3, after each one): when it is 1 the next frame shows the character.
 * Holding R makes it 1 whenever it is higher, so one character per frame. Off with `--game-opt dialog_skip=0`. */
#include "snes_rt.h"

#define BUTTON_R 0x0010

void alttp_dialog_speed(Cpu *c) {
    (void)c;
    if (!(joy1_state() & BUTTON_R) || !game_opt("dialog_skip", 1)) return;
    if (wram_read8(0x1CD5) > 1) wram_write8(0x1CD5, 1);
}
