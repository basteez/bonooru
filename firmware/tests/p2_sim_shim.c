/*
 * SPDX-License-Identifier: MIT
 *
 * A tiny C API over the real P2 profile (ocb_p2 + ocb_proto), built as a
 * shared library for tests/test_p2_probe.py (story 2.3 F.32): the host
 * side of tools/p2_probe.py talks to the same code the controller runs on
 * core 0, through a fake HID transport, on a fake clock. One instance;
 * the published state is NEUTRAL until p2sim_set_state (story 2.3 D35).
 */
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#include "ocb_p2/desc.h"
#include "ocb_p2/p2.h"
#include "ocb_proto/state.h"

static ocb_p2 s_p2;
static ocb_state s_state = OCB_NEUTRAL_INIT; /* the published state (story 2.3 D35) */

void p2sim_init(const uint8_t *mac);
int p2sim_out(const uint8_t *buf, size_t len);
size_t p2sim_in(uint8_t *out, size_t cap);
void p2sim_restart(void);
int p2sim_ready(void);
uint32_t p2sim_refusals(void);
size_t p2sim_report_desc(uint8_t *out, size_t cap);
void p2sim_set_state(const uint8_t *s8);

void p2sim_init(const uint8_t *mac)
{
    ocb_p2_cfg cfg;
    memcpy(cfg.mac, mac, sizeof cfg.mac);
    ocb_p2_init(&s_p2, &cfg);
    s_state = OCB_NEUTRAL;
}

/* The 8-byte canonical state the profile reports from now on (AD-10). */
void p2sim_set_state(const uint8_t *s8)
{
    ocb_state_decode(s8, &s_state);
}

int p2sim_out(const uint8_t *buf, size_t len)
{
    return (int)ocb_p2_on_output(&s_p2, buf, len, NULL);
}

size_t p2sim_in(uint8_t *out, size_t cap)
{
    return ocb_p2_next_in(&s_p2, &s_state, out, cap);
}

void p2sim_restart(void)
{
    ocb_p2_restart(&s_p2);
}

int p2sim_ready(void)
{
    return ocb_p2_ready(&s_p2) ? 1 : 0;
}

uint32_t p2sim_refusals(void)
{
    return s_p2.cnt.refusals;
}

size_t p2sim_report_desc(uint8_t *out, size_t cap)
{
    if (cap < sizeof ocb_p2_report_desc) {
        return 0u;
    }
    memcpy(out, ocb_p2_report_desc, sizeof ocb_p2_report_desc);
    return sizeof ocb_p2_report_desc;
}
