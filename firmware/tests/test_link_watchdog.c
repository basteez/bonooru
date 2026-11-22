/*
 * SPDX-License-Identifier: MIT
 *
 * Host test F.21 (story 2.7 Task 4.2): the controller link watchdog on a
 * fake clock, through the real link and core. Frames are built with the
 * real encoder; every output frame is decoded with
 * ocb_frame_decode(OCB_RX_APP, ...). Expectations are written by hand from
 * docs/link-protocol.md "Rules the controller link applies (Story 2.7)".
 *
 * Covered: the heartbeat timeout boundary and the clamp (AC1, D4); arming
 * by the first heartbeat (D3); the effects of a loss in IDLE, DIRECT and
 * PLAYING (AC1); breaks (D5, D8); the TX release sequence (D6); recovery
 * (D7); the DIRECT app timeout (AC2, D9); the DTR edge (AC2, D10); NAK
 * NO_LINK through the link (AC3, D11); clock wrap and backwards steps (D1);
 * the STATUS debug counter watchdog_trips (D12); independence.
 */
#include <stdint.h>
#include <stdio.h>

#include "link_fixture.h"

#define MS 1000u
#define LINK_US ((uint64_t)OCB_LINK_TIMEOUT_MS * MS)
#define APP_US ((uint64_t)OCB_APP_TIMEOUT_MS_DEFAULT * MS)
#define REL_US ((uint64_t)OCB_LINK_RELEASE_WAIT_US)

/* ------------------------------------------------------------ helpers */

/* Like lh_init_cfg, with the clock at `base` and explicit timeouts. */
static void wh_init(lh *h, uint64_t base, uint16_t cap, uint16_t link_ms, uint16_t app_ms, bool debug)
{
    memset(h, 0, sizeof *h);
    h->now = base;
    h->auto_hb = true;
    h->dtr = true;
    h->hb_period_us = (uint64_t)OCB_HEARTBEAT_PERIOD_MS * MS;
    const int rc = ocb_core_init(&h->core, h->storage, cap, 0u);
    OCB_CHECK(rc == 0, "core init");
    ocb_usb_mailbox_init(&h->mb);
    ocb_link_cfg cfg;
    cfg.fw_ver[0] = LH_FW_MAJOR;
    cfg.fw_ver[1] = LH_FW_MINOR;
    cfg.fw_ver[2] = LH_FW_PATCH;
    cfg.hid_profile = OCB_HID_PROFILE_P2;
    cfg.app_timeout_ms = app_ms;
    cfg.link_timeout_ms = link_ms;
    cfg.debug_counters = debug;
    ocb_link_init(&h->link, &h->core, &h->mb, &cfg, h->now);
}

/* Gate open, console ready, session open, replies drained, link up. */
static void wh_session(lh *h, uint64_t base, uint16_t cap, uint16_t link_ms, uint16_t app_ms, bool debug)
{
    wh_init(h, base, cap, link_ms, app_ms, debug);
    lh_open_gate(h);
    lh_make_ready(h);
    lh_hello(h);
    lh_discard_tx(h);
    lh_link_up(h);
}

static void poll(lh *h)
{
    ocb_link_poll(&h->link, h->now);
}

/* Keeps `ready` alive with an IN report and polls. */
static void poll_ready(lh *h)
{
    ocb_usb_mailbox_in_report(&h->mb);
    ocb_link_poll(&h->link, h->now);
}

static uint8_t trips(const lh *h)
{
    return ocb_link_watchdog_trips(&h->link);
}

static bool gate_open(lh *h)
{
    return ocb_txgate_is_open(ocb_link_gate(&h->link));
}

/* Counts the EVENT frames with `code` among all due output. */
static size_t count_events(lh *h, uint8_t code, size_t *total)
{
    lh_out o[64];
    const size_t n = lh_drain(h, o, 64u);
    size_t k = 0;
    for (size_t i = 0; i < n && i < 64u; i++) {
        if (o[i].type == OCB_MSG_EVENT && o[i].body[0] == code) {
            k++;
        }
    }
    if (total != NULL) {
        *total = n;
    }
    return k;
}

/* Hands out the EVENTs of a pending release, completes it with an idle UART
 * and reopens the gate. */
static void settle_and_reopen(lh *h)
{
    lh_discard_tx(h);
    ocb_link_tx_settled(&h->link, h->now, true);
    OCB_CHECK(!ocb_link_release_pending(&h->link), "release still pending after an idle settle");
    if (!gate_open(h)) {
        lh_open_gate(h);
    }
}

/* A STATUS now (forced due, the white-box poke of test_link_ready). m->extra
 * points into a static buffer, valid until the next call. */
static bool get_status(lh *h, ocb_msg_status *m)
{
    h->link.status_sent = false;
    static lh_out o[64];
    const size_t n = lh_drain(h, o, 64u);
    for (size_t i = 0; i < n && i < 64u; i++) {
        if (lh_status_decode(&o[i], m)) {
            return true;
        }
    }
    return false;
}

static void expect_nak_reason(lh *h, uint8_t reason, const char *what)
{
    lh_out o[8];
    const size_t n = lh_replies(h, o, 8u);
    ocb_msg_nak m = {0u, 0u, 0u};
    const bool ok = n == 1u && o[0].type == OCB_MSG_NAK && ocb_msg_nak_decode(o[0].body, o[0].len, &m) == OCB_OK;
    OCB_CHECK(ok && m.reason == reason, "%s: wanted one NAK 0x%02x, got %zu frame(s), type 0x%02x reason 0x%02x",
              what, reason, n, n > 0u ? o[0].type : 0u, m.reason);
}

static void expect_ack_any(lh *h, const char *what)
{
    lh_out o[8];
    const size_t n = lh_replies(h, o, 8u);
    OCB_CHECK(n == 1u && o[0].type == OCB_MSG_ACK, "%s: wanted one ACK, got %zu frame(s), first 0x%02x", what, n,
              n > 0u ? o[0].type : 0u);
}

/* The time of the last heartbeat the link received. */
static uint64_t last_hb(const lh *h)
{
    return ocb_link_last_heartbeat_us(&h->link);
}

/* Stops the emulated bridge and sets the clock to last heartbeat + gap. */
static void silence_until(lh *h, uint64_t gap_us)
{
    h->auto_hb = false;
    h->now = last_hb(h) + gap_us;
}

/* ---------------------------------------------------------- constants */

static void test_literal_values(void)
{
    /* Task 7.3: a change of the header fails here and must be made on
     * purpose (docs, vectors, probe and this test change together). */
    OCB_CHECK(OCB_LINK_TIMEOUT_MS == 80, "OCB_LINK_TIMEOUT_MS %d", OCB_LINK_TIMEOUT_MS);
    OCB_CHECK(OCB_LINK_TIMEOUT_MAX_MS == 100, "OCB_LINK_TIMEOUT_MAX_MS %d", OCB_LINK_TIMEOUT_MAX_MS);
    OCB_CHECK(OCB_APP_TIMEOUT_MS_DEFAULT == 1000, "OCB_APP_TIMEOUT_MS_DEFAULT %d", OCB_APP_TIMEOUT_MS_DEFAULT);
    OCB_CHECK(OCB_HEARTBEAT_MAX_GAP_MS < OCB_LINK_TIMEOUT_MS, "max heartbeat gap not below the timeout");
    OCB_CHECK(OCB_LINK_RELEASE_WAIT_US == 5000, "OCB_LINK_RELEASE_WAIT_US %d", OCB_LINK_RELEASE_WAIT_US);
}

static void test_elapsed_helper(void)
{
    OCB_CHECK(ocb_wd_elapsed_us(10u, 3u) == 7u, "elapsed 10 - 3");
    OCB_CHECK(ocb_wd_elapsed_us(3u, 10u) == 0u, "backwards is 0");
    OCB_CHECK(ocb_wd_elapsed_us(UINT64_MAX, 0u) == UINT64_MAX, "full range");
    OCB_CHECK(ocb_wd_elapsed_us(0u, UINT64_MAX) == 0u, "backwards from the top is 0");
    OCB_CHECK(ocb_wd_elapsed_us((uint64_t)1 << 32, ((uint64_t)1 << 32) - 1u) == 1u, "across 2^32");
}

/* ------------------------------------------------- AC1: timeout boundary */

static void test_heartbeats_keep_it_up(void)
{
    static lh h;
    wh_session(&h, 1000000u, 16u, 0u, 0u, false);
    const uint8_t aborts = h.core.abort_count;
    for (uint32_t i = 0; i < 1000000u; i++) {
        h.now += (uint64_t)OCB_HEARTBEAT_PERIOD_MS * MS;
        lh_hb_now(&h);
        poll(&h);
    }
    OCB_CHECK(trips(&h) == 0u && h.core.abort_count == aborts && ocb_link_link_ok(&h.link),
              "10^6 heartbeat periods: trips %u aborts %u link_ok %d", trips(&h), h.core.abort_count,
              (int)ocb_link_link_ok(&h.link));

    /* Gaps of 27 ms, 60 ms and exactly 80 ms: alive. */
    const uint64_t gaps[] = {(uint64_t)OCB_HEARTBEAT_MAX_GAP_MS * MS, 60u * MS, LINK_US};
    for (size_t g = 0; g < 3u; g++) {
        h.now = last_hb(&h) + gaps[g];
        poll(&h);
        OCB_CHECK(trips(&h) == 0u && ocb_link_link_ok(&h.link), "gap %llu us tripped",
                  (unsigned long long)gaps[g]);
        lh_hb_now(&h);
    }
    lh_discard_tx(&h);
    /* 80 ms + 1 us: one trip, exactly one LINK_LOST. */
    silence_until(&h, LINK_US + 1u);
    poll(&h);
    OCB_CHECK(trips(&h) == 1u && !ocb_link_link_ok(&h.link), "80 ms + 1 us: trips %u link_ok %d", trips(&h),
              (int)ocb_link_link_ok(&h.link));
    size_t total = 0;
    const size_t ll = count_events(&h, OCB_EVT_LINK_LOST, &total);
    OCB_CHECK(ll == 1u && total == 1u, "80 ms + 1 us: %zu LINK_LOST in %zu frames", ll, total);
    /* Further polls in silence: nothing more. */
    for (int i = 0; i < 100; i++) {
        h.now += 10u * MS;
        poll(&h);
    }
    OCB_CHECK(trips(&h) == 1u && h.core.abort_count == (uint8_t)(aborts + 1u), "silence tripped again");
}

/* cfg link_timeout_ms -> the effective limit in ms. */
static void check_limit(uint16_t cfg_ms, uint16_t want_ms)
{
    static lh h;
    wh_session(&h, 1000000u, 16u, cfg_ms, 0u, false);
    OCB_CHECK(ocb_wd_link_timeout_ms(&h.link.wd) == want_ms && h.link.cfg.link_timeout_ms == want_ms,
              "cfg %u: effective %u, wanted %u", cfg_ms, ocb_wd_link_timeout_ms(&h.link.wd), want_ms);
    silence_until(&h, (uint64_t)want_ms * MS);
    poll(&h);
    OCB_CHECK(trips(&h) == 0u && ocb_link_link_ok(&h.link), "cfg %u: exactly %u ms tripped", cfg_ms, want_ms);
    h.now += 1u;
    poll(&h);
    OCB_CHECK(trips(&h) == 1u && !ocb_link_link_ok(&h.link), "cfg %u: %u ms + 1 us did not trip", cfg_ms, want_ms);
}

static void test_limits_and_clamp(void)
{
    check_limit(0u, 80u);
    check_limit(80u, 80u);
    check_limit(100u, 100u);
    check_limit(101u, 100u);
    check_limit(250u, 100u);
    check_limit(65535u, 100u);
    check_limit(50u, 50u);
}

/* ------------------------------------------------------- D3: arming */

static void test_arming(void)
{
    static lh h;
    wh_init(&h, 1000000u, 16u, 0u, 0u, true);
    lh_open_gate(&h);
    lh_make_ready(&h);
    lh_hello(&h); /* seq 0 */
    lh_discard_tx(&h);
    for (int i = 0; i < 5; i++) {
        h.now += 3600000000u; /* an hour */
        poll_ready(&h);
    }
    lh_discard_tx(&h);
    OCB_CHECK(!h.core.aborted && h.core.abort_count == 0u && h.core.last_event == OCB_EVT_NONE,
              "no heartbeat ever: the core was aborted");
    OCB_CHECK(!ocb_link_link_ok(&h.link) && trips(&h) == 0u && gate_open(&h), "no heartbeat ever: link state");
    ocb_msg_status m;
    OCB_CHECK(get_status(&h, &m) && (m.flags & OCB_STATUS_LINK_OK) == 0u && m.extra_len >= 3u && m.extra[2] == 0u,
              "no heartbeat ever: STATUS link_ok / watchdog_trips");

    lh_set_state(&h, lst_buttons(OCB_BTN_A));
    expect_nak_reason(&h, OCB_NAK_NO_LINK, "boot: SET_STATE");
    lh_queue_event(&h, 1000u, lst_buttons(OCB_BTN_A));
    expect_ack_any(&h, "boot: QUEUE_EVENT");
    lh_start(&h, 0u);
    expect_nak_reason(&h, OCB_NAK_NO_LINK, "boot: START");
    lh_ping(&h);
    expect_ack_any(&h, "boot: PING");
    lh_stop(&h);
    expect_ack_any(&h, "boot: STOP");
    lh_hello(&h);
    {
        lh_out o[8];
        const size_t n = lh_replies(&h, o, 8u);
        OCB_CHECK(n == 1u && o[0].type == OCB_MSG_INFO, "boot: HELLO did not get INFO");
    }

    /* A heartbeat with a short body does not arm. */
    const uint8_t short_body[2] = {0u, 1u};
    lh_send_seq(&h, OCB_MSG_HEARTBEAT, 1u, short_body, sizeof short_body);
    OCB_CHECK(!ocb_link_link_ok(&h.link) && !h.link.wd.armed &&
                  ocb_link_counters_get(&h.link)->heartbeat_bad == 1u,
              "short heartbeat armed the watchdog");
    h.now += 10000000u;
    poll_ready(&h);
    OCB_CHECK(trips(&h) == 0u && !h.core.aborted, "short heartbeat: a later silence tripped");

    /* The first valid heartbeat arms and sets link_ok. */
    lh_link_up(&h);
    OCB_CHECK(ocb_link_link_ok(&h.link) && h.link.wd.armed, "valid heartbeat did not arm");
    lh_set_state(&h, lst_buttons(OCB_BTN_A));
    expect_ack_any(&h, "after the first heartbeat: SET_STATE");
}

/* ------------------------------------------------------ AC1: loss effects */

static void enter_mode(lh *h, uint8_t mode)
{
    lh_queue_event(h, 0u, lst_buttons(OCB_BTN_A));
    lh_queue_event(h, 500000u, lst_buttons(OCB_BTN_X));
    lh_queue_event(h, 900000u, lst_end());
    if (mode == OCB_MODE_DIRECT) {
        lh_set_state(h, lst_buttons(OCB_BTN_B));
    } else if (mode == OCB_MODE_PLAYING) {
        lh_start(h, 0u);
        poll_ready(h); /* the t=0 note is held */
        OCB_CHECK(h->core.published.buttons == OCB_BTN_A, "setup: note not held");
    }
    lh_discard_tx(h);
    OCB_CHECK(h->core.mode == mode, "setup: mode %u, wanted %u", h->core.mode, mode);
}

static void test_loss_effects(void)
{
    for (uint8_t mode = OCB_MODE_IDLE; mode <= OCB_MODE_PLAYING; mode++) {
        static lh h;
        wh_session(&h, 1000000u, 16u, 0u, 0u, true);
        enter_mode(&h, mode);
        const uint8_t aborts = h.core.abort_count;
        silence_until(&h, LINK_US + 1u);
        poll_ready(&h);
        OCB_CHECK(ocb_state_is_neutral(&h.core.published) && h.core.mode == OCB_MODE_IDLE &&
                      ocb_queue_free(&h.core.queue) == h.core.queue.cap && h.core.aborted &&
                      h.core.abort_count == (uint8_t)(aborts + 1u) && h.core.last_event == OCB_EVT_LINK_LOST,
                  "mode %u: loss effects (mode %u free %u aborted %d count %u last %u)", mode, h.core.mode,
                  ocb_queue_free(&h.core.queue), (int)h.core.aborted, h.core.abort_count, h.core.last_event);
        OCB_CHECK(count_events(&h, OCB_EVT_LINK_LOST, NULL) == 1u, "mode %u: not one LINK_LOST", mode);
        settle_and_reopen(&h);
        ocb_msg_status m;
        OCB_CHECK(get_status(&h, &m) && (m.flags & OCB_STATUS_LINK_OK) == 0u && m.extra[2] == 1u &&
                      m.mode == OCB_MODE_IDLE && m.last_event == OCB_EVT_LINK_LOST,
                  "mode %u: STATUS after the loss", mode);
        /* The latch comes before NO_LINK. */
        lh_start(&h, 0u);
        expect_nak_reason(&h, OCB_NAK_ABORTED, "after loss: START");
        lh_set_state(&h, lst_buttons(OCB_BTN_A));
        expect_nak_reason(&h, OCB_NAK_ABORTED, "after loss: SET_STATE");
        lh_queue_event(&h, 1000u, lst_buttons(OCB_BTN_A));
        expect_nak_reason(&h, OCB_NAK_ABORTED, "after loss: QUEUE_EVENT");
        if (mode == OCB_MODE_DIRECT) {
            lh_hello(&h);
        } else {
            lh_stop(&h);
        }
        lh_discard_tx(&h);
        OCB_CHECK(!h.core.aborted, "STOP/HELLO did not clear the latch");
        lh_set_state(&h, lst_buttons(OCB_BTN_A));
        expect_nak_reason(&h, OCB_NAK_NO_LINK, "latch cleared, link down: SET_STATE");
        lh_start(&h, 0u);
        expect_nak_reason(&h, OCB_NAK_NO_LINK, "latch cleared, link down: START");
    }
}

static void test_loss_when_latched(void)
{
    static lh h;
    wh_session(&h, 1000000u, 16u, 0u, 0u, false);
    lh_queue_event(&h, 1000u, lst_end());
    lh_start(&h, 0u);
    lh_advance(&h, 2000u);
    poll_ready(&h);
    OCB_CHECK(h.core.aborted && h.core.last_event == OCB_EVT_PLAYBACK_DONE, "setup: END not applied");
    lh_discard_tx(&h);
    const uint8_t aborts = h.core.abort_count;
    silence_until(&h, LINK_US + 1u);
    poll_ready(&h);
    size_t total = 0;
    OCB_CHECK(count_events(&h, OCB_EVT_LINK_LOST, &total) == 0u && total == 0u,
              "latched core: a LINK_LOST EVENT was sent (%zu frames)", total);
    OCB_CHECK(!ocb_link_link_ok(&h.link) && trips(&h) == 1u && h.core.abort_count == aborts &&
                  h.core.last_event == OCB_EVT_PLAYBACK_DONE,
              "latched core: link_ok / trips / abort_count / last_event");
    OCB_CHECK(ocb_link_release_pending(&h.link) || !gate_open(&h), "latched core: TX not being released");
    ocb_link_tx_settled(&h.link, h.now, true);
    OCB_CHECK(!gate_open(&h), "latched core: TX not released");
}

static void test_due_event_same_pass(void)
{
    static lh h;
    wh_session(&h, 1000000u, 16u, 0u, 0u, false);
    lh_queue_event(&h, 0u, lst_buttons(OCB_BTN_A));
    lh_queue_event(&h, (uint32_t)(LINK_US + 1u), lst_end());
    lh_start(&h, 0u); /* t0 = now */
    const uint64_t t0 = h.now;
    lh_hb_now(&h); /* the last heartbeat, at t0 */
    poll_ready(&h);
    lh_discard_tx(&h);
    OCB_CHECK(h.core.published.buttons == OCB_BTN_A, "setup: note");
    const uint32_t pc = h.core.publish_count;
    h.auto_hb = false;
    h.now = t0 + LINK_US + 1u; /* the END is due in the same pass as the trip */
    poll_ready(&h);
    OCB_CHECK(h.core.last_event == OCB_EVT_LINK_LOST && h.core.publish_count == pc + 1u,
              "due event applied in the tripping pass (last_event %u, publishes %u)", h.core.last_event,
              h.core.publish_count - pc);
    size_t total = 0;
    OCB_CHECK(count_events(&h, OCB_EVT_LINK_LOST, &total) == 1u && total == 1u, "same pass: events");
}

/* ----------------------------------------------------------- D5, D8 breaks */

static void test_breaks(void)
{
    static lh h;
    wh_session(&h, 1000000u, 16u, 0u, 0u, true);
    enter_mode(&h, OCB_MODE_PLAYING);
    /* FE, PE, OE alone abort nothing, in any number. */
    const uint16_t non_breaks[] = {OCB_LINK_RX_FE, OCB_LINK_RX_PE, OCB_LINK_RX_OE,
                                   OCB_LINK_RX_FE | OCB_LINK_RX_PE | OCB_LINK_RX_OE};
    for (int r = 0; r < 50; r++) {
        for (size_t i = 0; i < 4u; i++) {
            ocb_link_rx_entry(&h.link, (uint16_t)(non_breaks[i] | 0x55u), h.now);
        }
        poll_ready(&h);
    }
    OCB_CHECK(trips(&h) == 0u && !h.core.aborted && h.core.mode == OCB_MODE_PLAYING && ocb_link_link_ok(&h.link),
              "FE/PE/OE aborted something");
    lh_discard_tx(&h);
    /* One BE entry while armed: abort at the next poll. */
    const uint8_t aborts = h.core.abort_count;
    ocb_link_rx_entry(&h.link, OCB_LINK_RX_BE, h.now);
    OCB_CHECK(!h.core.aborted && ocb_link_link_ok(&h.link), "break acted before the poll");
    poll_ready(&h);
    OCB_CHECK(h.core.aborted && h.core.mode == OCB_MODE_IDLE && ocb_queue_free(&h.core.queue) == 16u &&
                  !ocb_link_link_ok(&h.link) && trips(&h) == 1u && h.core.abort_count == (uint8_t)(aborts + 1u),
              "break while armed: effects");
    OCB_CHECK(count_events(&h, OCB_EVT_LINK_LOST, NULL) == 1u, "break: not one LINK_LOST");
    ocb_link_tx_settled(&h.link, h.now, true);
    OCB_CHECK(!gate_open(&h), "break: TX not released");
    /* 29 further breaks (a bouncing wire) while the link is down. */
    for (int i = 0; i < 29; i++) {
        ocb_link_rx_entry(&h.link, OCB_LINK_RX_BE, h.now);
        h.now += 100u;
        poll_ready(&h);
        ocb_link_tx_settled(&h.link, h.now, true);
    }
    OCB_CHECK(trips(&h) == 1u && h.core.abort_count == (uint8_t)(aborts + 1u), "29 more breaks tripped again");
    lh_open_gate(&h);
    OCB_CHECK(count_events(&h, OCB_EVT_LINK_LOST, NULL) == 0u, "29 more breaks: a second LINK_LOST");

    /* A break at boot (not armed): no abort, no latch, TX still released. */
    wh_init(&h, 1000000u, 16u, 0u, 0u, false);
    lh_open_gate(&h);
    ocb_link_rx_entry(&h.link, OCB_LINK_RX_BE, h.now);
    poll(&h);
    OCB_CHECK(!h.core.aborted && h.core.abort_count == 0u && trips(&h) == 0u, "boot break latched the core");
    OCB_CHECK(ocb_link_release_pending(&h.link), "boot break: no TX release started");
    ocb_link_tx_settled(&h.link, h.now, true);
    OCB_CHECK(!gate_open(&h) && !ocb_link_release_pending(&h.link), "boot break: TX not released");

    /* A break and a fresh heartbeat in the same drain: the loss stands at the
     * next poll; the following heartbeat sets link_ok again (D7, D26). */
    wh_session(&h, 1000000u, 16u, 0u, 0u, false);
    lh_advance(&h, 10u * MS);
    (void)ocb_rxring_push(ocb_link_rx_ring(&h.link), OCB_LINK_RX_BE);
    {
        ocb_msg_heartbeat hm = {{0u, 0u, 0u}, OCB_HB_FLAG_DTR};
        uint8_t body[OCB_MAX_BODY];
        uint8_t wire[OCB_MAX_WIRE_FRAME];
        const size_t bl = (size_t)ocb_msg_heartbeat_encode(&hm, body, sizeof body);
        const size_t n = lh_wire(OCB_MSG_HEARTBEAT, 1u, body, bl, wire);
        for (size_t i = 0; i < n; i++) {
            (void)ocb_rxring_push(ocb_link_rx_ring(&h.link), wire[i]);
        }
    }
    (void)ocb_link_rx_drain(&h.link, h.now, 1000u);
    poll(&h);
    OCB_CHECK(h.core.aborted && !ocb_link_link_ok(&h.link) && trips(&h) == 1u, "break + heartbeat: no loss");
    OCB_CHECK(count_events(&h, OCB_EVT_LINK_LOST, NULL) == 1u, "break + heartbeat: LINK_LOST");
    lh_hb_now(&h);
    OCB_CHECK(ocb_link_link_ok(&h.link) && h.link.wd.armed, "break + heartbeat: the next heartbeat did not recover");
}

/* ------------------------------------------------------- D6: TX release */

static void test_release_sequence(void)
{
    static lh h;
    wh_session(&h, 1000000u, 16u, 0u, 0u, false);
    lh_advance(&h, 20u * MS); /* STATUS due */
    lh_ping(&h);              /* ACK pending */
    lh_send_seq(&h, OCB_MSG_PING, (uint8_t)(h.core.expected + 100u), NULL, 0u); /* NAK SEQ pending */
    const uint32_t dropped = ocb_link_counters_get(&h.link)->tx_dropped;
    silence_until(&h, LINK_US + 1u);
    poll(&h);
    OCB_CHECK(ocb_link_release_pending(&h.link) && gate_open(&h), "release not pending, or gate closed too early");
    uint8_t wire[OCB_MAX_WIRE_FRAME];
    size_t n = ocb_link_tx_next(&h.link, h.now, wire, sizeof wire);
    lh_out o;
    OCB_CHECK(n > 0u && lh_decode_out(wire, n, &o) && o.type == OCB_MSG_EVENT && o.body[0] == OCB_EVT_LINK_LOST,
              "the first frame after the loss is not EVENT LINK_LOST");
    n = ocb_link_tx_next(&h.link, h.now, wire, sizeof wire);
    OCB_CHECK(n == 0u, "a frame left after LINK_LOST while the release is pending (%zu bytes)", n);
    OCB_CHECK(ocb_link_counters_get(&h.link)->tx_dropped == dropped + 2u, "ACK and NAK not counted as dropped (%u)",
              ocb_link_counters_get(&h.link)->tx_dropped - dropped);
    /* Not idle: exactly 5 ms is still open, 5 ms + 1 us closes. */
    const uint64_t start = h.link.release_start_us;
    h.now = start + REL_US;
    ocb_link_tx_settled(&h.link, h.now, false);
    OCB_CHECK(gate_open(&h) && ocb_link_release_pending(&h.link), "released at exactly 5 ms");
    h.now += 1u;
    ocb_link_tx_settled(&h.link, h.now, false);
    OCB_CHECK(!gate_open(&h) && !ocb_link_release_pending(&h.link), "not released at 5 ms + 1 us");
    /* Closed: nothing leaves, pending output is discarded. */
    ocb_link_queue_event(&h.link, OCB_EVT_LATE);
    OCB_CHECK(ocb_link_tx_next(&h.link, h.now, wire, sizeof wire) == 0u && h.link.ev_count == 0u,
              "closed gate sent or kept an EVENT");
    /* Reopen: RX high for 1 ms, a low sample restarts the wait. */
    ocb_txgate *g = ocb_link_gate(&h.link);
    (void)ocb_txgate_sample(g, true, h.now);
    (void)ocb_txgate_sample(g, false, h.now + 900u);
    OCB_CHECK(!ocb_txgate_sample(g, true, h.now + 1000u), "low sample did not restart the wait");
    OCB_CHECK(!ocb_txgate_sample(g, true, h.now + 1999u), "reopened before 1 ms of high");
    OCB_CHECK(ocb_txgate_sample(g, true, h.now + 2000u), "not reopened after 1 ms of high");
    h.now += 2000u;
    ocb_msg_status m;
    OCB_CHECK(get_status(&h, &m) && (m.flags & OCB_STATUS_LINK_OK) == 0u, "STATUS after reopening");

    /* EVENT before release with the UART idle: the release waits for the
     * EVENT to be handed out. */
    wh_session(&h, 1000000u, 16u, 0u, 0u, false);
    silence_until(&h, LINK_US + 1u);
    poll(&h);
    ocb_link_tx_settled(&h.link, h.now, true);
    OCB_CHECK(gate_open(&h) && ocb_link_release_pending(&h.link), "released before the EVENT left");
    OCB_CHECK(count_events(&h, OCB_EVT_LINK_LOST, NULL) == 1u, "EVENT lost before the release");
    ocb_link_tx_settled(&h.link, h.now, false);
    OCB_CHECK(gate_open(&h), "released while the UART was busy");
    ocb_link_tx_settled(&h.link, h.now, true);
    OCB_CHECK(!gate_open(&h), "not released once the EVENT left and the UART was idle");

    /* A full EVENT ring still completes the release by the deadline. */
    wh_session(&h, 1000000u, 16u, 0u, 0u, false);
    for (int i = 0; i < OCB_LINK_TX_EVENT_CAP; i++) {
        ocb_link_queue_event(&h.link, OCB_EVT_LATE);
    }
    silence_until(&h, LINK_US + 1u);
    poll(&h);
    h.now += REL_US + 1u;
    ocb_link_tx_settled(&h.link, h.now, true);
    OCB_CHECK(!gate_open(&h) && !ocb_link_release_pending(&h.link), "full EVENT ring blocked the release");

    /* A loss while the gate is already closed completes at once. */
    wh_init(&h, 1000000u, 16u, 0u, 0u, false);
    lh_link_up(&h);
    silence_until(&h, LINK_US + 1u);
    poll(&h);
    OCB_CHECK(trips(&h) == 1u && !ocb_link_release_pending(&h.link) && !gate_open(&h),
              "closed gate: release did not complete at once");

    /* A second loss while a release is pending does not extend the deadline. */
    wh_session(&h, 1000000u, 16u, 0u, 0u, false);
    ocb_link_rx_entry(&h.link, OCB_LINK_RX_BE, h.now);
    poll(&h);
    const uint64_t s0 = h.link.release_start_us;
    h.now += 3000u;
    ocb_link_rx_entry(&h.link, OCB_LINK_RX_BE, h.now);
    poll(&h);
    OCB_CHECK(h.link.release_start_us == s0, "a break while pending moved the deadline");

    /* EVENT is never reordered behind a STATUS: with STATUS due, nothing
     * but EVENTs leaves from the loss to the release. */
    wh_session(&h, 1000000u, 16u, 0u, 0u, false);
    lh_advance(&h, 60u * MS);
    silence_until(&h, LINK_US + 1u);
    poll(&h);
    lh_out all[16];
    const size_t k = lh_drain(&h, all, 16u);
    OCB_CHECK(k == 1u && all[0].type == OCB_MSG_EVENT, "STATUS left during the release (%zu frames, first 0x%02x)", k,
              k > 0u ? all[0].type : 0u);
}

/* ------------------------------------------------------- D7: recovery */

static void test_recovery(void)
{
    static lh h;
    wh_session(&h, 1000000u, 16u, 0u, 0u, true);
    silence_until(&h, LINK_US + 1u);
    poll_ready(&h);
    lh_discard_tx(&h);
    settle_and_reopen(&h);
    OCB_CHECK(!ocb_link_link_ok(&h.link), "setup: loss");
    h.auto_hb = true;
    lh_hb_now(&h);
    OCB_CHECK(ocb_link_link_ok(&h.link) && h.link.wd.armed, "heartbeat after the loss did not recover");
    lh_set_state(&h, lst_buttons(OCB_BTN_A));
    expect_nak_reason(&h, OCB_NAK_ABORTED, "recovered: latch still set");
    lh_ping(&h);
    expect_ack_any(&h, "recovered: session still open (no NAK VERSION)");
    lh_stop(&h);
    lh_discard_tx(&h);
    poll_ready(&h);
    lh_set_state(&h, lst_buttons(OCB_BTN_A));
    expect_ack_any(&h, "recovered + STOP: SET_STATE");
    /* A second loss: a second LINK_LOST and another trip. */
    silence_until(&h, LINK_US + 1u);
    poll_ready(&h);
    OCB_CHECK(count_events(&h, OCB_EVT_LINK_LOST, NULL) == 1u && trips(&h) == 2u && h.core.abort_count == 2u,
              "second loss: trips %u aborts %u", trips(&h), h.core.abort_count);
}

/* ---------------------------------------------------- AC2: app timeout */

/* DIRECT entered now (SET_STATE); heartbeats keep flowing. */
static void enter_direct(lh *h)
{
    lh_set_state(h, lst_buttons(OCB_BTN_B));
    lh_discard_tx(h);
    OCB_CHECK(h->core.mode == OCB_MODE_DIRECT, "setup: DIRECT");
}

/* Expects alive at exactly `limit` after `ref`, abort at + 1 us. */
static void expect_app_timeout_at(lh *h, uint64_t ref, uint64_t limit, const char *what)
{
    const uint8_t aborts = h->core.abort_count;
    const uint8_t t0 = trips(h);
    lh_advance(h, ref + limit - h->now);
    poll_ready(h);
    OCB_CHECK(h->core.mode == OCB_MODE_DIRECT && !h->core.aborted, "%s: aborted at exactly the limit", what);
    lh_advance(h, 1u);
    poll_ready(h);
    size_t total = 0;
    const size_t ev = count_events(h, OCB_EVT_LINK_LOST, &total);
    OCB_CHECK(h->core.mode == OCB_MODE_IDLE && h->core.aborted && h->core.abort_count == (uint8_t)(aborts + 1u) &&
                  ocb_state_is_neutral(&h->core.published) && ocb_link_link_ok(&h->link) &&
                  trips(h) == (uint8_t)(t0 + 1u),
              "%s: limit + 1 us: mode %u aborted %d link_ok %d", what, h->core.mode, (int)h->core.aborted,
              (int)ocb_link_link_ok(&h->link));
    OCB_CHECK(ev == 0u, "%s: APP_TIMEOUT sent an EVENT", what);
}

static void send_corrupted_ping(lh *h)
{
    uint8_t wire[OCB_MAX_WIRE_FRAME];
    const size_t n = lh_wire(OCB_MSG_PING, h->seq, NULL, 0u, wire);
    wire[n - 2u] = (uint8_t)(wire[n - 2u] ^ 0x10u);
    lh_feed(h, wire, n);
}

static void test_app_timeout(void)
{
    static lh h;
    /* Entering DIRECT starts the clock. */
    wh_session(&h, 1000000u, 16u, 0u, 0u, false);
    lh_advance(&h, 3000u * MS); /* a long IDLE before: irrelevant */
    poll_ready(&h);
    enter_direct(&h);
    expect_app_timeout_at(&h, h.now, APP_US, "entering SET_STATE");

    /* Each kind of valid app frame keeps it alive. */
    for (int kind = 0; kind < 6; kind++) {
        wh_session(&h, 1000000u, 16u, 0u, 0u, false);
        lh_queue_event(&h, 100u, lst_buttons(OCB_BTN_A));
        lh_discard_tx(&h);
        enter_direct(&h);
        lh_advance(&h, 900u * MS);
        poll_ready(&h);
        switch (kind) {
        case 0:
            lh_ping(&h);
            break;
        case 1:
            lh_set_state(&h, lst_buttons(OCB_BTN_X));
            break;
        case 2:
            lh_send_seq(&h, OCB_MSG_PING, (uint8_t)(h.core.expected - 1u), NULL, 0u); /* stale duplicate */
            break;
        case 3:
            lh_queue_event(&h, 50u, lst_buttons(OCB_BTN_A)); /* NAK ORDER */
            break;
        case 4:
            lh_send(&h, 0x07u, NULL, 0u); /* NAK UNKNOWN_TYPE */
            break;
        default:
            lh_set_state(&h, lst_end()); /* NAK BAD_STATE */
            break;
        }
        lh_discard_tx(&h);
        OCB_CHECK(h.core.mode == OCB_MODE_DIRECT, "kind %d: left DIRECT", kind);
        expect_app_timeout_at(&h, h.now, APP_US, "frame kind");
    }

    /* A frame with a bad CRC does not count; a HEARTBEAT does not count. */
    wh_session(&h, 1000000u, 16u, 0u, 0u, false);
    enter_direct(&h);
    const uint64_t entered = h.now;
    lh_advance(&h, 900u * MS);
    send_corrupted_ping(&h);
    lh_discard_tx(&h);
    expect_app_timeout_at(&h, entered, APP_US, "bad CRC / heartbeats only");

    /* HELLO and STOP leave DIRECT: nothing pending. */
    for (int k = 0; k < 2; k++) {
        wh_session(&h, 1000000u, 16u, 0u, 0u, false);
        enter_direct(&h);
        if (k == 0) {
            lh_hello(&h);
        } else {
            lh_stop(&h);
        }
        lh_discard_tx(&h);
        for (int i = 0; i < 300; i++) {
            lh_advance(&h, 10u * MS);
            poll_ready(&h);
        }
        OCB_CHECK(!h.core.aborted && trips(&h) == 0u, "%s: an app timeout after leaving DIRECT",
                  k == 0 ? "HELLO" : "STOP");
    }

    /* A NAK VERSION frame keeps it alive too: session closed by a mismatched
     * HELLO is not DIRECT any more, so check the frame counts at admission:
     * the app-frame time moves for a NAK VERSION frame. */
    wh_session(&h, 1000000u, 16u, 0u, 0u, false);
    lh_hello_v(&h, 0x7Eu); /* closes the session */
    lh_discard_tx(&h);
    lh_advance(&h, 5u * MS);
    lh_ping(&h);
    expect_nak_reason(&h, OCB_NAK_VERSION, "closed session: PING");
    OCB_CHECK(h.link.last_app_frame_us == h.now, "a NAK VERSION frame did not count as an app frame");

    /* Custom limits: 250 and 1 ms honoured; 0 gives 1000 and INFO says so. */
    const uint16_t customs[] = {250u, 1u};
    for (size_t i = 0; i < 2u; i++) {
        wh_session(&h, 1000000u, 16u, 0u, customs[i], false);
        enter_direct(&h);
        expect_app_timeout_at(&h, h.now, (uint64_t)customs[i] * MS, "custom app_timeout_ms");
    }
    wh_init(&h, 1000000u, 16u, 0u, 0u, false);
    lh_open_gate(&h);
    lh_hello(&h);
    {
        lh_out o[8];
        ocb_msg_info info;
        memset(&info, 0, sizeof info);
        const size_t n = lh_replies(&h, o, 8u);
        OCB_CHECK(n == 1u && ocb_msg_info_decode(o[0].body, o[0].len, &info) == OCB_OK &&
                      info.app_timeout_ms == OCB_APP_TIMEOUT_MS_DEFAULT,
                  "app_timeout_ms 0: INFO reports %u", info.app_timeout_ms);
        OCB_CHECK(ocb_wd_app_timeout_ms(&h.link.wd) == 1000u, "app_timeout_ms 0: effective %u",
                  ocb_wd_app_timeout_ms(&h.link.wd));
    }

    /* Not in IDLE: an hour of silence (with heartbeats). */
    wh_session(&h, 1000000u, 16u, 0u, 0u, false);
    for (int i = 0; i < 3600; i++) {
        lh_advance(&h, 1000u * MS);
        poll_ready(&h);
    }
    OCB_CHECK(!h.core.aborted && trips(&h) == 0u, "IDLE: app timeout fired");

    /* Not in PLAYING: a 10 s piece with no app frames plays to its END. */
    wh_session(&h, 1000000u, 16u, 0u, 0u, false);
    lh_queue_event(&h, 0u, lst_buttons(OCB_BTN_A));
    lh_queue_event(&h, 5000000u, lst_buttons(OCB_BTN_X));
    lh_queue_event(&h, 10000000u, lst_end());
    lh_start(&h, 0u);
    const uint64_t t0 = h.now;
    lh_discard_tx(&h);
    while (h.now < t0 + 10000000u - 1u * MS) {
        lh_advance(&h, 1u * MS); /* finer than the LATE threshold */
        poll_ready(&h);
        if (h.core.mode != OCB_MODE_PLAYING) {
            break;
        }
    }
    OCB_CHECK(h.core.mode == OCB_MODE_PLAYING && trips(&h) == 0u && !h.core.aborted,
              "PLAYING: aborted before END (mode %u trips %u)", h.core.mode, trips(&h));
    lh_advance(&h, 1u * MS);
    poll_ready(&h);
    OCB_CHECK(h.core.last_event == OCB_EVT_PLAYBACK_DONE && trips(&h) == 0u, "PLAYING: did not end with END");
}

/* --------------------------------------------------------- AC2: DTR edge */

static void test_dtr(void)
{
    static lh h;
    /* 1 -> 0 in DIRECT: abort at that heartbeat, no EVENT. */
    wh_session(&h, 1000000u, 16u, 0u, 0u, false);
    enter_direct(&h);
    lh_advance(&h, 40u * MS);
    h.dtr = false;
    const uint8_t aborts = h.core.abort_count;
    lh_hb_now(&h);
    OCB_CHECK(h.core.aborted && h.core.mode == OCB_MODE_IDLE && h.core.abort_count == (uint8_t)(aborts + 1u) &&
                  trips(&h) == 1u && ocb_link_link_ok(&h.link),
              "DTR 1 -> 0 in DIRECT: no abort at the heartbeat");
    OCB_CHECK(count_events(&h, OCB_EVT_LINK_LOST, NULL) == 0u && h.core.last_event == OCB_EVT_NONE,
              "DTR edge sent an EVENT");

    /* The same edge in IDLE and in PLAYING: nothing. */
    for (int k = 0; k < 2; k++) {
        wh_session(&h, 1000000u, 16u, 0u, 0u, false);
        if (k == 1) {
            enter_mode(&h, OCB_MODE_PLAYING);
        }
        lh_advance(&h, 40u * MS);
        h.dtr = false;
        lh_advance(&h, 40u * MS);
        poll_ready(&h);
        OCB_CHECK(!h.core.aborted && trips(&h) == 0u, "DTR edge in %s aborted", k == 0 ? "IDLE" : "PLAYING");
    }

    /* 0 -> 0, 0 -> 1, 1 -> 1 in DIRECT: nothing. DIRECT entered with DTR
     * already 0 is not aborted by the DTR rule. */
    wh_session(&h, 1000000u, 16u, 0u, 0u, false);
    h.dtr = false;
    lh_advance(&h, 40u * MS);
    enter_direct(&h);
    lh_advance(&h, 100u * MS); /* 0 -> 0 */
    h.dtr = true;
    lh_advance(&h, 40u * MS); /* 0 -> 1 */
    lh_advance(&h, 100u * MS); /* 1 -> 1 */
    poll_ready(&h);
    OCB_CHECK(!h.core.aborted && trips(&h) == 0u && h.core.mode == OCB_MODE_DIRECT, "non-edge DTR patterns aborted");

    /* Flags bits 1 to 7 never matter. */
    wh_session(&h, 1000000u, 16u, 0u, 0u, false);
    enter_direct(&h);
    lh_heartbeat(&h, 0u, 0u, 0u, 0xFFu);
    lh_heartbeat(&h, 0u, 0u, 0u, 0x01u);
    lh_heartbeat(&h, 0u, 0u, 0u, 0xFFu);
    OCB_CHECK(!h.core.aborted, "flags bits 1-7: 0xFF/0x01 aborted");
    lh_heartbeat(&h, 0u, 0u, 0u, 0xFEu);
    OCB_CHECK(h.core.aborted && trips(&h) == 1u, "flags 0xFF -> 0xFE (DTR falling) did not abort");

    /* Unit level: the first heartbeat ever with DTR 0 is not an edge; a loss
     * resets the memory. */
    ocb_wd wd;
    ocb_wd_init(&wd, 0u, 0u, 0u);
    OCB_CHECK(!ocb_wd_on_heartbeat(&wd, 1000u, false, OCB_MODE_DIRECT), "first heartbeat DTR 0 is an edge");
    OCB_CHECK(!ocb_wd_on_heartbeat(&wd, 2000u, true, OCB_MODE_DIRECT), "0 -> 1 is an edge");
    OCB_CHECK(ocb_wd_check(&wd, 2000u + LINK_US + 1u, OCB_MODE_DIRECT, 2000u + LINK_US, true, 0u) ==
                  OCB_WD_LINK_TIMEOUT,
              "unit: timeout verdict");
    OCB_CHECK(!wd.armed, "unit: loss did not disarm");
    OCB_CHECK(!ocb_wd_on_heartbeat(&wd, 3000000u, false, OCB_MODE_DIRECT), "DTR 1, loss, DTR 0 is an edge");
    OCB_CHECK(wd.trips == 1u, "unit trips %u", wd.trips);
}

/* ------------------------------------------- AC3: NAK NO_LINK via the link */

static void test_no_link_semantic(void)
{
    static lh h;
    wh_init(&h, 1000000u, 16u, 0u, 0u, false);
    lh_open_gate(&h);
    lh_make_ready(&h);
    lh_hello(&h);
    lh_discard_tx(&h);
    lh_queue_event(&h, 1000u, lst_buttons(OCB_BTN_A));
    lh_discard_tx(&h);
    const uint8_t seq = h.seq;
    lh_start(&h, 0u);
    lh_expect_nak(&h, OCB_NAK_NO_LINK, (uint8_t)(seq + 1u), seq, "START, link down");
    OCB_CHECK(h.core.expected == (uint8_t)(seq + 1u) && h.core.mode == OCB_MODE_IDLE &&
                  ocb_queue_count(&h.core.queue) == 1u,
              "NO_LINK had an effect");
    {
        const ocb_msg_start sm = {0u};
        uint8_t body[OCB_MAX_BODY];
        const int bl = ocb_msg_start_encode(&sm, body, sizeof body);
        lh_send_seq(&h, OCB_MSG_START, seq, body, (size_t)bl); /* duplicate */
    }
    lh_expect_nak(&h, OCB_NAK_NO_LINK, (uint8_t)(seq + 1u), seq, "duplicate START: cached NAK");
    const uint8_t s2 = h.seq;
    lh_set_state(&h, lst_buttons(OCB_BTN_A));
    lh_expect_nak(&h, OCB_NAK_NO_LINK, (uint8_t)(s2 + 1u), s2, "SET_STATE, link down");
    lh_link_up(&h);
    lh_set_state(&h, lst_buttons(OCB_BTN_A));
    expect_ack_any(&h, "SET_STATE after the first heartbeat (ready)");
}

/* --------------------------------------------------- D1: wrap and clock */

typedef struct {
    bool alive_at_80;
    bool tripped_after;
    bool app_alive_at_limit;
    bool app_tripped_after;
    bool rel_open_at_5;
    bool rel_closed_after;
    uint8_t trips;
    uint8_t aborts;
} scen_result;

/* The timeout, release and app-timeout scenarios starting at `start` (40 ms
 * before a boundary in the wrap cases; each scenario crosses it). */
static scen_result scenario(uint64_t start)
{
    static lh h;
    scen_result r;
    memset(&r, 0, sizeof r);
    /* Heartbeat timeout across the boundary. */
    wh_session(&h, start, 16u, 0u, 0u, false);
    silence_until(&h, LINK_US);
    poll_ready(&h);
    r.alive_at_80 = ocb_link_link_ok(&h.link) && trips(&h) == 0u;
    /* Release with a busy UART. */
    h.now += 1u;
    poll_ready(&h);
    r.tripped_after = !ocb_link_link_ok(&h.link) && trips(&h) == 1u;
    const uint64_t rs = h.now;
    lh_discard_tx(&h);
    h.now = rs + REL_US;
    ocb_link_tx_settled(&h.link, h.now, false);
    r.rel_open_at_5 = gate_open(&h);
    h.now += 1u;
    ocb_link_tx_settled(&h.link, h.now, false);
    r.rel_closed_after = !gate_open(&h);
    /* App timeout across the boundary (entered 960 ms before it). */
    wh_session(&h, start, 16u, 0u, 0u, false);
    enter_direct(&h);
    const uint64_t entered = h.now;
    lh_advance(&h, entered + APP_US - h.now);
    poll_ready(&h);
    r.app_alive_at_limit = h.core.mode == OCB_MODE_DIRECT;
    lh_advance(&h, 1u);
    poll_ready(&h);
    r.app_tripped_after = h.core.mode == OCB_MODE_IDLE && h.core.aborted && ocb_link_link_ok(&h.link);
    r.trips = trips(&h);
    r.aborts = h.core.abort_count;
    return r;
}

static void test_wrap(void)
{
    const scen_result ref = scenario(1000000u);
    OCB_CHECK(ref.alive_at_80 && ref.tripped_after && ref.app_alive_at_limit && ref.app_tripped_after &&
                  ref.rel_open_at_5 && ref.rel_closed_after && ref.trips == 1u && ref.aborts == 1u,
              "reference scenario at 1 s: %d %d %d %d %d %d %u %u", ref.alive_at_80, ref.tripped_after,
              ref.app_alive_at_limit, ref.app_tripped_after, ref.rel_open_at_5, ref.rel_closed_after, ref.trips,
              ref.aborts);
    const uint64_t pre = 40u * MS;
    const uint64_t starts[] = {
        ((uint64_t)1 << 32) - pre,           /* u32 us wrap, 71.6 min */
        ((uint64_t)1 << 32) * 1000u - pre,   /* u32 ms counter wrap, 49.7 days */
        ((uint64_t)1 << 53) - pre,
    };
    const char *names[] = {"2^32 us", "2^32 ms", "2^53 us"};
    for (size_t i = 0; i < 3u; i++) {
        const scen_result r = scenario(starts[i]);
        OCB_CHECK(memcmp(&r, &ref, sizeof r) == 0,
                  "scenario across %s differs from the reference: %d %d %d %d %d %d %u %u", names[i], r.alive_at_80,
                  r.tripped_after, r.app_alive_at_limit, r.app_tripped_after, r.rel_open_at_5, r.rel_closed_after,
                  r.trips, r.aborts);
    }

    /* A clock that steps backwards (1 s, 2^40 us): no trip, no abort. */
    const uint64_t backs[] = {1000000u, (uint64_t)1 << 40};
    for (size_t i = 0; i < 2u; i++) {
        static lh h;
        wh_session(&h, ((uint64_t)1 << 41), 16u, 0u, 0u, false);
        enter_direct(&h);
        lh_ping(&h);
        lh_discard_tx(&h);
        h.auto_hb = false;
        const uint64_t t = h.now;
        h.now = t - backs[i];
        poll_ready(&h);
        ocb_link_tx_settled(&h.link, h.now, false);
        OCB_CHECK(trips(&h) == 0u && !h.core.aborted && h.core.mode == OCB_MODE_DIRECT && ocb_link_link_ok(&h.link),
                  "backwards step %llu us: tripped", (unsigned long long)backs[i]);
        /* Back to the real time: still alive (elapsed counted from the
         * stored stamps). */
        h.now = t;
        poll_ready(&h);
        OCB_CHECK(trips(&h) == 0u && !h.core.aborted, "back to now: tripped");
    }
    /* Release deadline with a backwards step: not overdue. */
    {
        static lh h;
        wh_session(&h, ((uint64_t)1 << 41), 16u, 0u, 0u, false);
        ocb_link_rx_entry(&h.link, OCB_LINK_RX_BE, h.now);
        poll(&h);
        ocb_link_tx_settled(&h.link, h.now - ((uint64_t)1 << 40), false);
        OCB_CHECK(gate_open(&h) && ocb_link_release_pending(&h.link), "backwards step: released as overdue");
    }
}

/* --------------------------------------------------- D12: watchdog_trips */

static uint8_t status_trips(lh *h, uint8_t *flags)
{
    settle_and_reopen(h);
    ocb_msg_status m;
    if (!get_status(h, &m) || m.extra_len < 3u) {
        OCB_CHECK(false, "no debug STATUS");
        return 0xEEu;
    }
    OCB_CHECK((m.flags & 0xFCu) == 0u, "STATUS flags bits 2-7 set: 0x%02x", m.flags);
    if (flags != NULL) {
        *flags = m.flags;
    }
    return m.extra[2];
}

static void test_trips_counter(void)
{
    static lh h;
    wh_session(&h, 1000000u, 16u, 0u, 0u, true);
    OCB_CHECK(status_trips(&h, NULL) == 0u, "watchdog_trips not 0 at boot");
    /* Timeout. */
    silence_until(&h, LINK_US + 1u);
    poll_ready(&h);
    OCB_CHECK(status_trips(&h, NULL) == 1u && h.core.abort_count == 1u, "timeout: trips");
    lh_stop(&h);
    lh_discard_tx(&h);
    h.auto_hb = true;
    lh_hb_now(&h);
    /* Break. */
    ocb_link_rx_entry(&h.link, OCB_LINK_RX_BE, h.now);
    poll_ready(&h);
    OCB_CHECK(status_trips(&h, NULL) == 2u && h.core.abort_count == 2u, "break: trips");
    lh_stop(&h);
    lh_discard_tx(&h);
    lh_hb_now(&h);
    /* App timeout. */
    enter_direct(&h);
    lh_advance(&h, APP_US + 1u);
    poll_ready(&h);
    OCB_CHECK(status_trips(&h, NULL) == 3u && h.core.abort_count == 3u, "app timeout: trips");
    lh_stop(&h);
    lh_discard_tx(&h);
    /* DTR edge. */
    enter_direct(&h);
    h.dtr = false;
    lh_hb_now(&h);
    OCB_CHECK(status_trips(&h, NULL) == 4u && h.core.abort_count == 4u, "DTR: trips");
    h.dtr = true;
    /* Wraps at 256: 252 more breaks with a heartbeat after each. */
    for (int i = 0; i < 252; i++) {
        lh_stop(&h);
        lh_hb_now(&h);
        ocb_link_rx_entry(&h.link, OCB_LINK_RX_BE, h.now);
        poll_ready(&h);
        settle_and_reopen(&h);
        lh_discard_tx(&h);
    }
    uint8_t flags = 0u;
    OCB_CHECK(status_trips(&h, &flags) == 0u && trips(&h) == 0u && h.core.abort_count == 0u,
              "watchdog_trips did not wrap with abort_count (trips %u aborts %u)", trips(&h), h.core.abort_count);
    OCB_CHECK((flags & OCB_STATUS_LINK_OK) == 0u, "link_ok set after a loss");
}

/* ------------------------------------------------------- independence */

static void test_independence(void)
{
    static lh a;
    static lh b;
    wh_session(&a, 1000000u, 16u, 0u, 0u, false);
    wh_session(&b, 1000000u, 16u, 0u, 0u, false);
    silence_until(&a, LINK_US + 1u);
    b.now = a.now;
    lh_hb_now(&b);
    poll_ready(&a);
    poll_ready(&b);
    OCB_CHECK(!ocb_link_link_ok(&a.link) && trips(&a) == 1u && a.core.aborted, "a: no loss");
    OCB_CHECK(ocb_link_link_ok(&b.link) && trips(&b) == 0u && !b.core.aborted && !ocb_link_release_pending(&b.link),
              "b: affected by a's loss");
}

int main(void)
{
    test_literal_values();
    test_elapsed_helper();
    test_heartbeats_keep_it_up();
    test_limits_and_clamp();
    test_arming();
    test_loss_effects();
    test_loss_when_latched();
    test_due_event_same_pass();
    test_breaks();
    test_release_sequence();
    test_recovery();
    test_app_timeout();
    test_dtr();
    test_no_link_semantic();
    test_wrap();
    test_trips_counter();
    test_independence();
    const int f = ocb_test_failures();
    if (f != 0) {
        fprintf(stderr, "test_link_watchdog (F.21): %d failure(s)\n", f);
        return 1;
    }
    printf("test_link_watchdog (F.21): ok\n");
    return 0;
}
