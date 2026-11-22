/*
 * SPDX-License-Identifier: MIT
 *
 * The canonical 8-byte controller state (AD-10): buttons u16 LE, hat, lx, ly,
 * rx, ry, flags. The codec does not validate or normalise field ranges (hat
 * 9..255, button bits 14 and 15): the scheduler and the HID profiles decide
 * what to do with them.
 */
#ifndef OCB_PROTO_STATE_H
#define OCB_PROTO_STATE_H

#include <stdbool.h>
#include <stdint.h>

#include "ocb_proto/consts.h"

typedef struct {
    uint16_t buttons; /* OCB_BTN_* bits */
    uint8_t hat;      /* OCB_HAT_* */
    uint8_t lx, ly, rx, ry;
    uint8_t flags; /* OCB_STATE_FLAG_END; other bits reserved */
} ocb_state;

#define OCB_NEUTRAL_INIT                                                                           \
    { 0u, OCB_HAT_NEUTRAL, OCB_STICK_CENTER, OCB_STICK_CENTER, OCB_STICK_CENTER,                   \
      OCB_STICK_CENTER, 0u }

/* NEUTRAL: no button, hat neutral, sticks centred, no flag. On the wire:
 * 00 00 08 80 80 80 80 00. */
extern const ocb_state OCB_NEUTRAL;

void ocb_state_encode(const ocb_state *s, uint8_t out[OCB_STATE_SIZE]);
void ocb_state_decode(const uint8_t in[OCB_STATE_SIZE], ocb_state *s);

/* Every field equals NEUTRAL, flags included (NEUTRAL|END is not neutral). */
bool ocb_state_is_neutral(const ocb_state *s);
/* Every field but flags equals NEUTRAL (AD-8: END only with a neutral pose). */
bool ocb_state_is_neutral_pose(const ocb_state *s);
bool ocb_state_has_end(const ocb_state *s);
/* Reserved flag bits (1..7) are zero. Buttons are not validated. */
bool ocb_state_reserved_bits_ok(const ocb_state *s);

#endif /* OCB_PROTO_STATE_H */
