/*
 * SPDX-License-Identifier: MIT
 *
 * Streaming receive splitter and counters (AD-3, AD-4, story 2.2 D10).
 */
#include "ocb_proto/frame.h"

#include <string.h>

void ocb_rx_init(ocb_rx *rx, ocb_rx_side side)
{
    memset(rx, 0, sizeof *rx);
    rx->side = side;
    rx->last_err = OCB_OK;
}

static void ocb_rx_count(ocb_rx *rx, ocb_err err)
{
    switch (err) {
    case OCB_ERR_EMPTY:
        rx->stats.empty++;
        break;
    case OCB_ERR_COBS:
        rx->stats.cobs_errors++;
        break;
    case OCB_ERR_RANGE:
        rx->stats.range_errors++;
        break;
    case OCB_ERR_OVERLONG:
        rx->stats.overlong++;
        break;
    case OCB_ERR_CRC:
    default:
        /* OCB_ERR_CRC; nothing else can come out of ocb_frame_decode here. */
        rx->stats.crc_errors++;
        break;
    }
    rx->last_err = err;
}

ocb_rx_result ocb_rx_push(ocb_rx *rx, uint8_t byte, ocb_frame *out)
{
    if (byte != OCB_FRAME_DELIMITER) {
        if (rx->discarding) {
            return OCB_RX_NONE;
        }
        if (rx->len >= (size_t)OCB_MAX_ENCODED_FRAME) {
            /* First excess byte: count the overlong run once, drop the rest. */
            ocb_rx_count(rx, OCB_ERR_OVERLONG);
            rx->discarding = true;
            rx->len = 0;
            return OCB_RX_NONE;
        }
        rx->buf[rx->len++] = byte;
        return OCB_RX_NONE;
    }

    if (rx->discarding) {
        rx->discarding = false;
        rx->len = 0;
        return OCB_RX_DROPPED;
    }
    const ocb_err err = ocb_frame_decode(rx->side, rx->buf, rx->len, out);
    rx->len = 0;
    if (err == OCB_OK) {
        rx->stats.frames_ok++;
        return OCB_RX_FRAME;
    }
    ocb_rx_count(rx, err);
    return OCB_RX_DROPPED;
}
