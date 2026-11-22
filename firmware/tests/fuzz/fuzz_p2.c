/*
 * SPDX-License-Identifier: MIT
 *
 * Optional libFuzzer entry over ocb_p2_on_output (story 2.3 Task 5.3,
 * D17). Not built by any preset and not run in CI: configure host-tests
 * with clang and -DOCB_FUZZ=ON, then run build/host-tests/tests/fuzz_p2.
 * The first input byte selects an action (restart, a feature report, an
 * IN report into a short buffer); the rest is one OUT report.
 */
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#include "ocb_p2/p2.h"
#include "ocb_proto/state.h"

int LLVMFuzzerTestOneInput(const uint8_t *data, size_t size);

int LLVMFuzzerTestOneInput(const uint8_t *data, size_t size)
{
    static ocb_p2 p;
    static int inited;
    if (!inited) {
        const ocb_p2_cfg cfg = {{0x02, 0x11, 0x22, 0x33, 0x44, 0x55}};
        ocb_p2_init(&p, &cfg);
        inited = 1;
    }
    if (size == 0u) {
        return 0;
    }
    uint8_t out[64];
    switch (data[0] & 0x03u) {
    case 0:
        ocb_p2_restart(&p);
        break;
    case 1:
        (void)ocb_p2_on_feature(&p, data[0], NULL);
        break;
    case 2:
        (void)ocb_p2_next_in(&p, &OCB_NEUTRAL, out, (size_t)(data[0] >> 2));
        break;
    default:
        break;
    }
    ocb_p2_log log;
    (void)ocb_p2_on_output(&p, data + 1, size - 1u, &log);
    (void)ocb_p2_next_in(&p, &OCB_NEUTRAL, out, sizeof out);
    return 0;
}
