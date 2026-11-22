/*
 * SPDX-License-Identifier: MIT
 *
 * Helpers for the P2 profile tests (story 2.3): frame builders written
 * from the documents (S1, S2), never from the library.
 */
#ifndef OCB_P2_FIXTURE_H
#define OCB_P2_FIXTURE_H

#include <stddef.h>
#include <stdint.h>
#include <string.h>

#include "ocb_p2/p2.h"
#include "ocb_proto/state.h"

/* MAC of the tests, display order (D10). */
static const uint8_t k_test_mac[6] = {0x11, 0x22, 0x33, 0x44, 0x55, 0x66};

static inline void fx_init(ocb_p2 *p)
{
    ocb_p2_cfg cfg;
    memcpy(cfg.mac, k_test_mac, sizeof cfg.mac);
    ocb_p2_init(p, &cfg);
}

/* A 64-byte 0x80 output report (S1): 80 <cmd>, zero padded. */
static inline ocb_p2_rx fx_usb(ocb_p2 *p, uint8_t cmd, ocb_p2_log *log)
{
    uint8_t b[64] = {0};
    b[0] = 0x80;
    b[1] = cmd;
    return ocb_p2_on_output(p, b, sizeof b, log);
}

/* A 64-byte 0x01 output report (S2): counter, neutral rumble
 * 00 01 40 40 00 01 40 40, subcommand id at [10], arguments from [11]. */
static inline ocb_p2_rx fx_sub(ocb_p2 *p, uint8_t id, const uint8_t *args, size_t nargs, ocb_p2_log *log)
{
    uint8_t b[64] = {0};
    static const uint8_t rumble[8] = {0x00, 0x01, 0x40, 0x40, 0x00, 0x01, 0x40, 0x40};
    b[0] = 0x01;
    b[1] = 0x05;
    memcpy(&b[2], rumble, sizeof rumble);
    b[10] = id;
    if (nargs > 53u) {
        nargs = 53u;
    }
    if (nargs > 0u) {
        memcpy(&b[11], args, nargs);
    }
    return ocb_p2_on_output(p, b, sizeof b, log);
}

/* The next IN report with a NEUTRAL state; 0 when nothing is due. */
static inline size_t fx_in(ocb_p2 *p, uint8_t out[64])
{
    return ocb_p2_next_in(p, &OCB_NEUTRAL, out, 64);
}

#endif /* OCB_P2_FIXTURE_H */
