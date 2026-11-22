/*
 * SPDX-License-Identifier: MIT
 *
 * Safety mask of PLAYING mode (AD-9, story 2.4 D7, test F.6).
 *
 * The allow-list is compile-time and this macro is the only place that
 * lists it: buttons L, R, A, X, Y, plus the hat and the left stick (lx, ly),
 * which pass unchanged. Every other button is cleared (B, ZL, ZR, Minus,
 * Plus, L3, R3, Home, Capture and the undefined bits 14 and 15), the right
 * stick is centred and the flags are cleared (flags are never published).
 *
 * The list is AD-9 as written, confirmed unchanged by the U11 result of
 * Story 1.1 (docs/ocarina-input-mechanics.md, table "Unknowns", row U11):
 * ZL opens the song menu, ZR does nothing, the right stick moves the camera,
 * and no other input plays a note, so masking ZL and the right stick is
 * desirable. Adding an input later takes a firmware change (AD-9): one line
 * here plus its row in tests/test_core_mask.c.
 *
 * DIRECT mode (SET_STATE) is not masked (AD-9); the core applies this mask
 * only to states published in PLAYING.
 */
#ifndef OCB_CORE_MASK_H
#define OCB_CORE_MASK_H

#include "ocb_core/ramfunc.h"
#include "ocb_proto/consts.h"
#include "ocb_proto/state.h"

#define OCB_CORE_ALLOWED_BUTTONS (OCB_BTN_L | OCB_BTN_R | OCB_BTN_A | OCB_BTN_X | OCB_BTN_Y)

/* Returns a masked copy of *in (in must be non-NULL). */
OCB_CORE_RAMFUNC ocb_state ocb_core_mask_state(const ocb_state *in);

#endif /* OCB_CORE_MASK_H */
