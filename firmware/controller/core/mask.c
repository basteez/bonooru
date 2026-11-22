/*
 * SPDX-License-Identifier: MIT
 *
 * Safety mask of PLAYING mode (AD-9, story 2.4 D7). The allow-list is
 * OCB_CORE_ALLOWED_BUTTONS in ocb_core/mask.h, confirmed by U11.
 */
#include <stdint.h>

#include "ocb_core/mask.h"

OCB_CORE_RAMFUNC ocb_state ocb_core_mask_state(const ocb_state *in)
{
    ocb_state s = *in;
    s.buttons = (uint16_t)(s.buttons & (uint16_t)OCB_CORE_ALLOWED_BUTTONS);
    /* hat, lx, ly pass unchanged (AD-9). */
    s.rx = OCB_STICK_CENTER;
    s.ry = OCB_STICK_CENTER;
    s.flags = 0u;
    return s;
}
