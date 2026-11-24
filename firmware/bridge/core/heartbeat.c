/*
 * SPDX-License-Identifier: MIT
 *
 * The bridge heartbeat frame (AD-4, Story 3.1). See ocb_bridge/heartbeat.h.
 */
#include "ocb_bridge/heartbeat.h"

#include "ocb_proto/consts.h"
#include "ocb_proto/frame.h"
#include "ocb_proto/msgs.h"

size_t ocb_bridge_hb_build(uint8_t seq, const uint8_t ver[3], bool dtr, uint8_t *out, size_t cap)
{
    ocb_msg_heartbeat m;
    uint8_t body[OCB_BODY_LEN_HEARTBEAT];
    m.bridge_fw_ver[0] = ver[0];
    m.bridge_fw_ver[1] = ver[1];
    m.bridge_fw_ver[2] = ver[2];
    m.flags = dtr ? (uint8_t)OCB_HB_FLAG_DTR : 0u;
    const int n = ocb_msg_heartbeat_encode(&m, body, sizeof body);
    if (n < 0) {
        return 0;
    }
    return ocb_frame_encode(OCB_MSG_HEARTBEAT, seq, body, (size_t)n, out, cap);
}
