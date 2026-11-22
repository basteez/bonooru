/*
 * SPDX-License-Identifier: MIT
 *
 * Host test F.15 (story 2.6 Task 5.8): session start and versioning
 * through the link (AD-7, D5, D8). NAK VERSION before the first matching
 * HELLO and before the seq check; STOP accepted before HELLO; a matching
 * HELLO opens the session from every state with STOP semantics and INFO
 * (every field checked); a mismatched or empty HELLO answers INFO, closes
 * the session and forces the core neutral; duplicate HELLOs; a rebooted
 * controller facing an app in mid-sequence; the session survives aborts,
 * USB events and (story 2.7) watchdog link losses.
 */
#include <stdint.h>
#include <stdio.h>

#include "link_fixture.h"

static void expect_info(lh *h, const uint8_t bridge[3], uint16_t report_period, const char *what)
{
    lh_out o[8];
    const size_t n = lh_replies(h, o, 8u);
    ocb_msg_info m;
    memset(&m, 0, sizeof m);
    const bool ok = n == 1u && o[0].type == OCB_MSG_INFO && o[0].len == OCB_BODY_LEN_INFO &&
                    ocb_msg_info_decode(o[0].body, o[0].len, &m) == OCB_OK;
    OCB_CHECK(ok, "%s: wanted exactly one INFO, got %zu frame(s), first 0x%02x", what, n, n > 0u ? o[0].type : 0u);
    if (!ok) {
        return;
    }
    OCB_CHECK(m.proto_ver == 1u, "%s: INFO proto_ver %u", what, m.proto_ver);
    OCB_CHECK(m.fw_ver[0] == LH_FW_MAJOR && m.fw_ver[1] == LH_FW_MINOR && m.fw_ver[2] == LH_FW_PATCH,
              "%s: INFO fw_ver %u.%u.%u", what, m.fw_ver[0], m.fw_ver[1], m.fw_ver[2]);
    OCB_CHECK(memcmp(m.bridge_ver, bridge, 3u) == 0, "%s: INFO bridge_ver %u.%u.%u", what, m.bridge_ver[0],
              m.bridge_ver[1], m.bridge_ver[2]);
    OCB_CHECK(m.hid_profile == OCB_HID_PROFILE_P2, "%s: INFO hid_profile %u", what, m.hid_profile);
    OCB_CHECK(m.queue_cap == h->core.queue.cap, "%s: INFO queue_cap %u, cap %u", what, m.queue_cap,
              h->core.queue.cap);
    OCB_CHECK(m.app_timeout_ms == LH_APP_TIMEOUT, "%s: INFO app_timeout_ms %u", what, m.app_timeout_ms);
    OCB_CHECK(m.report_period_us == report_period, "%s: INFO report_period_us %u, wanted %u", what,
              m.report_period_us, report_period);
}

static const uint8_t k_zero_ver[3] = {0u, 0u, 0u};

static void expect_neutral_idle(const lh *h, const char *what)
{
    OCB_CHECK(h->core.mode == OCB_MODE_IDLE && !h->core.aborted && ocb_state_is_neutral(&h->core.published) &&
                  ocb_queue_count(&h->core.queue) == 0u,
              "%s: mode %u aborted %d neutral %d queue %u", what, h->core.mode, (int)h->core.aborted,
              (int)ocb_state_is_neutral(&h->core.published), ocb_queue_count(&h->core.queue));
}

static void test_before_hello(void)
{
    static lh h;
    lh_init(&h, 8u);
    lh_open_gate(&h);
    lh_make_ready(&h);
    lh_discard_tx(&h);

    uint8_t qe[OCB_MAX_BODY];
    const size_t nqe = lh_body_queue_event(100u, lst_buttons(OCB_BTN_A), qe);
    uint8_t ss[OCB_STATE_SIZE];
    ocb_state_encode(&OCB_NEUTRAL, ss);
    const uint8_t start[4] = {0u, 0u, 0u, 0u};
    struct {
        uint8_t type;
        const uint8_t *body;
        size_t len;
        uint8_t seq;
        const char *name;
    } cases[] = {
        {OCB_MSG_SET_STATE, ss, sizeof ss, 0u, "SET_STATE"},
        {OCB_MSG_QUEUE_EVENT, qe, nqe, 1u, "QUEUE_EVENT"},
        {OCB_MSG_START, start, sizeof start, 2u, "START"},
        {OCB_MSG_PING, NULL, 0u, 3u, "PING"},
        {0x07u, NULL, 0u, 4u, "unassigned 0x07"},
        {OCB_MSG_PING, NULL, 0u, 200u, "PING with a wild seq"},
        {OCB_MSG_QUEUE_EVENT, qe, nqe, 77u, "QUEUE_EVENT with a wild seq"},
    };
    for (size_t i = 0; i < sizeof cases / sizeof cases[0]; i++) {
        lh_send_seq(&h, cases[i].type, cases[i].seq, cases[i].body, cases[i].len);
        lh_expect_nak(&h, OCB_NAK_VERSION, (uint8_t)(cases[i].seq + 1u), cases[i].seq, cases[i].name);
        OCB_CHECK(h.core.expected == (uint8_t)(cases[i].seq + 1u), "%s: expected %u", cases[i].name,
                  h.core.expected);
        OCB_CHECK(ocb_queue_count(&h.core.queue) == 0u && h.core.mode == OCB_MODE_IDLE, "%s: had an effect",
                  cases[i].name);
    }
    OCB_CHECK(!ocb_link_session_open(&h.link), "session opened without HELLO");

    /* STOP is accepted before HELLO, and does not open the session. */
    lh_send_seq(&h, OCB_MSG_STOP, 90u, NULL, 0u);
    lh_expect_ack(&h, 90u, "STOP before HELLO");
    OCB_CHECK(h.core.expected == 91u, "STOP: expected %u", h.core.expected);
    lh_send_seq(&h, OCB_MSG_PING, 91u, NULL, 0u);
    lh_expect_nak(&h, OCB_NAK_VERSION, 92u, 91u, "PING after STOP, before HELLO");
}

/* Brings the session-open link into `mode` (prefilled, a held note in
 * PLAYING) or aborted (mode 3). */
static void enter_state(lh *h, int state)
{
    lh_queue_event(h, 0u, lst_buttons(OCB_BTN_A));
    lh_queue_event(h, 1000000u, lst_buttons(OCB_BTN_X));
    lh_queue_event(h, 2000000u, lst_end());
    if (state == OCB_MODE_DIRECT) {
        lh_set_state(h, lst_buttons(OCB_BTN_B));
    } else if (state == OCB_MODE_PLAYING) {
        lh_start(h, 0u);
        ocb_link_poll(&h->link, h->now); /* applies the t=0 note: A held */
        OCB_CHECK(h->core.published.buttons == OCB_BTN_A, "setup: note not held");
    } else if (state == 3) {
        ocb_core_out out;
        ocb_core_abort(&h->core, OCB_ABORT_LINK_LOST, &out);
    }
    lh_discard_tx(h);
    if (state <= OCB_MODE_PLAYING) {
        OCB_CHECK(h->core.mode == (uint8_t)state, "setup: mode %u, wanted %d", h->core.mode, state);
    } else {
        OCB_CHECK(h->core.aborted, "setup: not aborted");
    }
}

static void test_matching_hello(void)
{
    static lh h;
    for (int state = 0; state <= 3; state++) {
        lh_session(&h, 16u);
        enter_state(&h, state);
        const uint8_t seq = (uint8_t)(h.seq + 37u); /* any seq */
        uint8_t pv = (uint8_t)OCB_PROTO_VERSION;
        lh_send_seq(&h, OCB_MSG_HELLO, seq, &pv, 1u);
        expect_info(&h, k_zero_ver, 0u, "matching HELLO");
        expect_neutral_idle(&h, "matching HELLO");
        OCB_CHECK(h.core.expected == (uint8_t)(seq + 1u), "state %d: expected %u", state, h.core.expected);
        OCB_CHECK(ocb_link_session_open(&h.link), "state %d: session not open", state);
    }

    /* bridge_ver from the last HEARTBEAT; report_period_us as set. */
    lh_session(&h, 512u);
    lh_heartbeat(&h, 1u, 2u, 3u, 0u);
    lh_heartbeat(&h, 4u, 5u, 6u, OCB_HB_FLAG_DTR);
    ocb_link_set_report_period(&h.link, 8000u);
    lh_hello(&h);
    const uint8_t bv[3] = {4u, 5u, 6u};
    expect_info(&h, bv, 8000u, "INFO after heartbeats");
    OCB_CHECK(h.core.late_us == 8000u, "report period not forwarded to the core");

    /* Longer HELLO body with the matching version: accepted. */
    lh_session(&h, 8u);
    lh_ping(&h);
    lh_discard_tx(&h);
    const uint8_t longer[3] = {(uint8_t)OCB_PROTO_VERSION, 0xAAu, 0xBBu};
    lh_send(&h, OCB_MSG_HELLO, longer, sizeof longer);
    expect_info(&h, k_zero_ver, 0u, "HELLO with trailing bytes");
    OCB_CHECK(ocb_link_session_open(&h.link), "HELLO with trailing bytes refused");

    /* Duplicate HELLOs (the app resends every 100 ms): answered each time. */
    for (int i = 0; i < 3; i++) {
        lh_hello(&h);
        expect_info(&h, k_zero_ver, 0u, "duplicate HELLO");
        OCB_CHECK(ocb_link_session_open(&h.link), "duplicate HELLO closed the session");
    }
    const uint8_t seq = h.seq;
    lh_ping(&h);
    lh_expect_ack(&h, seq, "PING after duplicate HELLOs");
}

static void test_mismatched_hello(void)
{
    static lh h;
    const uint8_t versions[] = {0u, 2u, 255u};
    for (int v = 0; v <= 3; v++) {
        for (int state = 0; state <= 3; state++) {
            lh_session(&h, 16u);
            enter_state(&h, state);
            const uint8_t seq = (uint8_t)(h.seq + 5u);
            if (v < 3) {
                lh_send_seq(&h, OCB_MSG_HELLO, seq, &versions[v], 1u);
            } else {
                lh_send_seq(&h, OCB_MSG_HELLO, seq, NULL, 0u); /* empty body */
            }
            expect_info(&h, k_zero_ver, 0u, "mismatched HELLO");
            expect_neutral_idle(&h, "mismatched HELLO");
            OCB_CHECK(!ocb_link_session_open(&h.link), "v %d state %d: session still open", v, state);
            OCB_CHECK(h.core.expected == (uint8_t)(seq + 1u), "v %d state %d: expected %u", v, state,
                      h.core.expected);
            const uint8_t next = (uint8_t)(seq + 1u);
            lh_send_seq(&h, OCB_MSG_PING, next, NULL, 0u);
            lh_expect_nak(&h, OCB_NAK_VERSION, (uint8_t)(next + 1u), next, "app frame after a mismatched HELLO");
        }
    }

    /* The shared vector hello_trailing (proto_ver 2, two trailing bytes) is
     * a mismatch for this controller. */
    lh_session(&h, 8u);
    const uint8_t hello_trailing[] = {0x09u, 0x01u, 0x09u, 0x03u, 0x02u, 0xaau, 0xbbu, 0x75u, 0xf2u, 0x00u};
    lh_feed(&h, hello_trailing, sizeof hello_trailing);
    expect_info(&h, k_zero_ver, 0u, "vector hello_trailing");
    OCB_CHECK(!ocb_link_session_open(&h.link) && h.core.expected == 10u, "hello_trailing: session %d expected %u",
              (int)ocb_link_session_open(&h.link), h.core.expected);
}

/* The controller rebooted; the app is still at seq 120 and keeps sending:
 * one NAK VERSION each (never NAK SEQ); over the NAK ring limit the TX
 * drops and counts. */
static void test_rebooted(void)
{
    static lh h;
    lh_init(&h, 512u);
    lh_open_gate(&h);
    lh_discard_tx(&h);
    const uint32_t dropped0 = ocb_link_counters_get(&h.link)->tx_dropped;
    uint8_t qe[OCB_MAX_BODY];
    const size_t nqe = lh_body_queue_event(1000u, lst_buttons(OCB_BTN_A), qe);
    for (uint8_t s = 120u; s < 130u; s++) {
        lh_send_seq(&h, OCB_MSG_QUEUE_EVENT, s, qe, nqe);
    }
    lh_out o[16];
    const size_t n = lh_replies(&h, o, 16u);
    OCB_CHECK(n == OCB_LINK_TX_NAK_CAP, "rebooted: %zu replies, wanted %d", n, OCB_LINK_TX_NAK_CAP);
    for (size_t i = 0; i < n && i < 16u; i++) {
        ocb_msg_nak m;
        OCB_CHECK(o[i].type == OCB_MSG_NAK && ocb_msg_nak_decode(o[i].body, o[i].len, &m) == OCB_OK &&
                      m.reason == OCB_NAK_VERSION && m.got == (uint8_t)(120u + i) &&
                      m.expected == (uint8_t)(121u + i),
                  "rebooted: reply %zu is not NAK VERSION for seq %zu", i, 120u + i);
    }
    OCB_CHECK(ocb_link_counters_get(&h.link)->tx_dropped - dropped0 == 10u - OCB_LINK_TX_NAK_CAP,
              "rebooted: tx_dropped +%u", ocb_link_counters_get(&h.link)->tx_dropped - dropped0);
    OCB_CHECK(ocb_queue_count(&h.core.queue) == 0u, "rebooted: something was enqueued");
    lh_hello(&h);
    expect_info(&h, k_zero_ver, 0u, "HELLO after the reboot");
}

/* A HELLO drops an ACK still pending from the numbering before it (D26):
 * HELLO(0), PING(1) not yet sent out, HELLO(100) -> only INFO leaves. */
static void test_hello_drops_stale_ack(void)
{
    static lh h;
    for (int v = 0; v < 2; v++) {
        lh_init(&h, 8u);
        lh_open_gate(&h);
        lh_send_seq(&h, OCB_MSG_HELLO, 0u, (const uint8_t[]){(uint8_t)OCB_PROTO_VERSION}, 1u);
        lh_discard_tx(&h);
        lh_send_seq(&h, OCB_MSG_PING, 1u, NULL, 0u); /* ACK(1) pending, not drained */
        const uint8_t pv = v == 0 ? (uint8_t)OCB_PROTO_VERSION : 9u; /* matching, then mismatched */
        lh_send_seq(&h, OCB_MSG_HELLO, 100u, &pv, 1u);
        expect_info(&h, k_zero_ver, 0u, v == 0 ? "matching HELLO after an undrained ACK"
                                               : "mismatched HELLO after an undrained ACK");
    }
}

static void test_session_survives(void)
{
    static lh h;
    lh_session(&h, 8u);
    ocb_core_out out;
    ocb_core_abort(&h.core, OCB_ABORT_LINK_LOST, &out);
    OCB_CHECK(ocb_link_session_open(&h.link), "abort closed the session");
    ocb_usb_mailbox_suspend(&h.mb);
    ocb_link_poll(&h.link, h.now);
    ocb_usb_mailbox_unmount(&h.mb);
    ocb_usb_mailbox_bus_reset(&h.mb);
    ocb_link_poll(&h.link, h.now);
    OCB_CHECK(ocb_link_session_open(&h.link), "USB events closed the session");
    lh_discard_tx(&h);
    const uint8_t seq = h.seq;
    lh_ping(&h);
    lh_expect_ack(&h, seq, "PING after abort and USB events");
    lh_stop(&h);
    OCB_CHECK(ocb_link_session_open(&h.link), "STOP closed the session");

    /* Story 2.7: a watchdog link loss (silence, then a break) does not close
     * the session either (2.6 D8). */
    lh_session(&h, 8u);
    h.auto_hb = false;
    h.now = ocb_link_last_heartbeat_us(&h.link) + (uint64_t)OCB_LINK_TIMEOUT_MS * 1000u + 1u;
    ocb_link_poll(&h.link, h.now);
    OCB_CHECK(!ocb_link_link_ok(&h.link) && ocb_link_session_open(&h.link), "link loss closed the session");
    lh_discard_tx(&h);
    ocb_link_tx_settled(&h.link, h.now, true);
    lh_open_gate(&h);
    lh_discard_tx(&h);
    lh_hb_now(&h);
    ocb_link_rx_entry(&h.link, OCB_LINK_RX_BE, h.now);
    ocb_link_poll(&h.link, h.now);
    OCB_CHECK(!ocb_link_link_ok(&h.link) && ocb_link_session_open(&h.link), "break closed the session");
    lh_discard_tx(&h);
    ocb_link_tx_settled(&h.link, h.now, true);
    lh_open_gate(&h);
    lh_discard_tx(&h);
    const uint8_t seq2 = h.seq;
    lh_ping(&h);
    lh_expect_ack(&h, seq2, "PING after two link losses: no NAK VERSION");
}

int main(void)
{
    test_before_hello();
    test_matching_hello();
    test_mismatched_hello();
    test_rebooted();
    test_hello_drops_stale_ack();
    test_session_survives();
    const int f = ocb_test_failures();
    if (f != 0) {
        fprintf(stderr, "test_link_session (F.15): %d failure(s)\n", f);
        return 1;
    }
    printf("test_link_session (F.15): ok\n");
    return 0;
}
