/*
 * SPDX-License-Identifier: MIT
 *
 * The debug STATUS (story 2.6 D12, D35; story 2.3 D14; story 2.5 D13): the
 * 11 base bytes, the v1 and v2 debug counters (21 bytes), the v3 P2 debug
 * log window (3 + 2n bytes) and the v4 self-timing block (41 bytes: the
 * core's lateness statistics, core 0's numbers from the USB mailbox, the
 * runner's loop maximum and stack mark, the boot cause from the config). Built only when cfg.debug_counters is set, in its own
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

#define TX_DBG_BODY_MAX (OCB_BODY_LEN_STATUS + OCB_LINK_DEBUG_TAIL_MAX_LEN) /* 108 */
/* header 3 + body 108 + crc 2 = 113 raw bytes, COBS overhead 1, delimiter 1 */
#define TX_DBG_WIRE_MAX (OCB_FRAME_HEADER_SIZE + TX_DBG_BODY_MAX + OCB_FRAME_CRC_SIZE + 2)
/* The UART driver's TX ring (ocb_hw/uart_link.h, OCB_UART_LINK_TX_RING): a
 * pure library cannot include it, so the size is restated here and pinned
 * by F.16 (story 2.5 D13: the largest STATUS must fit an empty ring, never
 * split); platform/link_runner.c asserts the real ring against the same
 * frame size where both are visible. */
#define TX_DBG_UART_RING 128

typedef char ocb_link_tx_dbg_fits[(OCB_LINK_DEBUG_COUNTERS_LEN + OCB_LINK_DEBUG_V3_HEADER_LEN +
                                       2 * OCB_DBGLOG_WINDOW ==
                                   OCB_LINK_DEBUG_V3_MAX_LEN &&
                                   OCB_LINK_DEBUG_V3_MAX_LEN + OCB_LINK_DEBUG_V4_LEN == OCB_LINK_DEBUG_TAIL_MAX_LEN &&
                                   TX_DBG_BODY_MAX <= OCB_MAX_BODY && TX_DBG_WIRE_MAX <= OCB_MAX_WIRE_FRAME &&
                                   TX_DBG_WIRE_MAX <= TX_DBG_UART_RING)
                                      ? 1
                                      : -1];

static OCB_CORE_RAMFUNC void put_u16(uint8_t *p, uint32_t v)
{
    const uint16_t s = v > 0xFFFFu ? (uint16_t)0xFFFFu : (uint16_t)v;
    p[0] = (uint8_t)(s & 0xFFu);
    p[1] = (uint8_t)(s >> 8);
}

static OCB_CORE_RAMFUNC void put_u32(uint8_t *p, uint32_t v)
{
    p[0] = (uint8_t)(v & 0xFFu);
    p[1] = (uint8_t)((v >> 8) & 0xFFu);
    p[2] = (uint8_t)((v >> 16) & 0xFFu);
    p[3] = (uint8_t)(v >> 24);
}

/* v4 (story 2.5 D13, D31): 41 bytes at p. Layout in ocb_link/config.h. */
static OCB_CORE_RAMFUNC void put_v4(const ocb_link *link, uint8_t *p)
{
    const ocb_core_timing_stats *t = &link->core->timing;
    const ocb_usb_mailbox *m = link->mailbox;
    put_u16(&p[0], t->n);
    put_u16(&p[2], t->late_max_us);
    put_u32(&p[4], t->late_sum_us);
    for (unsigned i = 0u; i < (unsigned)OCB_CORE_LATE_BUCKETS; i++) {
        put_u16(&p[8u + 2u * i], t->hist[i]);
    }
    put_u16(&p[20], link->dbg_loop_max_us);
    put_u16(&p[22], m->core0_pass_max_us);
    put_u16(&p[24], m->in_min_us);
    put_u16(&p[26], m->in_p95_us);
    put_u16(&p[28], m->in_max_us);
    put_u16(&p[30], m->in_slow);
    put_u16(&p[32], m->seq_retries);
    const uint32_t fb = m->seq_fallbacks;
    p[34] = fb > 0xFFu ? (uint8_t)0xFFu : (uint8_t)fb;
    put_u16(&p[35], link->dbg_stack_used);
    p[37] = link->cfg.boot_cause;
    p[38] = link->cfg.reboot_count;
    put_u16(&p[39], link->cfg.stall_reset_us / 10u); /* D31: 10 us units, saturating */
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
    put_v4(link, &extra[len]);
    len += (size_t)OCB_LINK_DEBUG_V4_LEN;

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
