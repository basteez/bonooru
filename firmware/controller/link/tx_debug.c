/*
 * SPDX-License-Identifier: MIT
 *
 * The debug STATUS (story 2.6 D12, D35; story 2.3 D14): the 11 base bytes,
 * the v1 and v2 debug counters (21 bytes) and the v3 P2 debug log window
 * (3 + 2n bytes). Built only when cfg.debug_counters is set, in its own
 * function with its own buffers, so the release TX path (tx.c) keeps its
 * stack. Reads the window as plain data: no ocb_dbglog_* function is
 * called here, so a release ELF links none of the debug log.
 */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#include "ocb_link/config.h"
#include "ocb_link/dbglog.h"
#include "ocb_link/internal.h"
#include "ocb_link/link.h"
#include "ocb_proto/consts.h"
#include "ocb_proto/frame.h"
#include "ocb_proto/msgs.h"

#define TX_DBG_BODY_MAX (OCB_BODY_LEN_STATUS + OCB_LINK_DEBUG_TAIL_MAX_LEN) /* 67 */
/* header 3 + body 67 + crc 2 = 72 raw bytes, COBS overhead 1, delimiter 1 */
#define TX_DBG_WIRE_MAX (OCB_FRAME_HEADER_SIZE + TX_DBG_BODY_MAX + OCB_FRAME_CRC_SIZE + 2)

typedef char ocb_link_tx_dbg_fits[(OCB_LINK_DEBUG_COUNTERS_LEN + OCB_LINK_DEBUG_V3_HEADER_LEN +
                                       2 * OCB_DBGLOG_WINDOW ==
                                   OCB_LINK_DEBUG_TAIL_MAX_LEN &&
                                   TX_DBG_BODY_MAX <= OCB_MAX_BODY && TX_DBG_WIRE_MAX <= OCB_MAX_WIRE_FRAME)
                                      ? 1
                                      : -1];

static OCB_CORE_RAMFUNC void put_u16(uint8_t *p, uint32_t v)
{
    const uint16_t s = v > 0xFFFFu ? (uint16_t)0xFFFFu : (uint16_t)v;
    p[0] = (uint8_t)(s & 0xFFu);
    p[1] = (uint8_t)(s >> 8);
}

OCB_CORE_RAMFUNC size_t ocb_link_tx_status_debug_(ocb_link *link, uint64_t now_us, uint8_t *out, size_t cap)
{
    uint8_t extra[OCB_LINK_DEBUG_TAIL_MAX_LEN];
    put_u16(&extra[0], ocb_link_max_app_gap_ms(link));
    extra[2] = link->wd.trips; /* watchdog_trips (story 2.7 D12), wraps */
    put_u16(&extra[3], ocb_link_dropped_bytes(link));
    put_u16(&extra[5], link->cnt.tx_dropped);
    /* v2 (D35): the three causes of dropped_bytes, then the error bits. */
    put_u16(&extra[7], link->cnt.rx_err_bytes);
    put_u16(&extra[9], link->ring.dropped);
    put_u16(&extra[11], link->cnt.overlong_bytes);
    put_u16(&extra[13], link->cnt.rx_fe);
    put_u16(&extra[15], link->cnt.rx_pe);
    put_u16(&extra[17], link->cnt.breaks);
    put_u16(&extra[19], link->cnt.rx_oe);
    /* v3 (story 2.3 D14): the P2 debug log window. total wraps (not
     * saturated): a reader sees a gap as a jump larger than n. */
    const ocb_dbglog_window *w = &link->dbg;
    const uint8_t n = w->n > OCB_DBGLOG_WINDOW ? (uint8_t)OCB_DBGLOG_WINDOW : w->n;
    extra[21] = (uint8_t)(w->total & 0xFFu);
    extra[22] = (uint8_t)(w->total >> 8);
    extra[23] = n;
    size_t len = OCB_LINK_DEBUG_COUNTERS_LEN + OCB_LINK_DEBUG_V3_HEADER_LEN;
    for (uint8_t i = 0u; i < n; i++) {
        const uint16_t rec = w->rec[(uint8_t)((w->start + i) % OCB_DBGLOG_WINDOW)];
        extra[len] = (uint8_t)(rec >> 8);       /* kind */
        extra[len + 1u] = (uint8_t)(rec & 0xFFu); /* id */
        len += 2u;
    }

    uint8_t body[TX_DBG_BODY_MAX];
    const int nb = ocb_link_status_body_(link, now_us, extra, len, body, sizeof body);
    if (nb < 0) {
        return 0u; /* unreachable: the tail fits TX_DBG_BODY_MAX */
    }
    uint8_t wire[TX_DBG_WIRE_MAX];
    const size_t wl = ocb_frame_encode(OCB_MSG_STATUS, link->tx_seq, body, (size_t)nb, wire, sizeof wire);
    if (wl == 0u || wl > cap) {
        return 0u; /* stays due; never split */
    }
    memcpy(out, wire, wl);
    ocb_link_status_sent_(link, now_us);
    return wl;
}
