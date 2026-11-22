/*
 * SPDX-License-Identifier: MIT
 *
 * Internal helpers shared by p2.c, report.c and interval.c. Not part of
 * the API.
 */
#ifndef OCB_P2_INTERNAL_H
#define OCB_P2_INTERNAL_H

#include <stdint.h>

#include "ocb_p2/p2.h"
#include "ocb_proto/state.h"

/* Writes bytes 1..12 of a 0x21 or 0x30 report (timer, battery and
 * connection, buttons, sticks, vibrator byte) from `s` into out[1..12] and
 * advances the timer. out must hold at least 13 bytes. */
void ocb_p2_header_(ocb_p2 *p, const ocb_state *s, uint8_t *out);

/* Empties the interval measurer (report period 0). */
void ocb_p2_interval_clear_(ocb_p2_interval *iv);

#endif /* OCB_P2_INTERNAL_H */
