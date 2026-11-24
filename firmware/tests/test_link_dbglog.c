/*
 * SPDX-License-Identifier: MIT
 *
 * F.31 (story 2.3 Task 7.5, AC5a, D13, D14): the debug log SPSC ring, the
 * STATUS window (the v3 debug tail, compared with hand-written bytes), the
 * release STATUS without any tail, and the measured report period crossing
 * the mailbox into INFO and the core's LATE threshold. Story 2.5 D13: the
 * v4 self-timing block after the window, hand-written bytes at offset
 * 24 + 2 n for n = 0, 1, 16, saturation, and release still 11 bytes.
 */
#include <stdint.h>
#include <stdio.h>

#include "link_fixture.h"
#include "ocb_link/dbglog.h"

static uint16_t rec(uint8_t kind, uint8_t id)
{
    return (uint16_t)((kind << 8) | id);
}

static void test_ring(void)
{
    static ocb_dbglog_ring r;
    ocb_dbglog_ring_init(&r);
    uint16_t v;
    OCB_CHECK(!ocb_dbglog_pop(&r, &v), "empty ring pops");
    /* Order. */
    for (unsigned i = 0; i < 10u; i++) {
        OCB_CHECK(ocb_dbglog_push(&r, 0x02, (uint8_t)i), "push %u", i);
    }
    for (unsigned i = 0; i < 10u; i++) {
        OCB_CHECK(ocb_dbglog_pop(&r, &v) && v == rec(0x02, (uint8_t)i), "pop %u: %04x", i, v);
    }
    /* Full ring: drops and counts, never overwrites. */
    for (unsigned i = 0; i < 64u; i++) {
        OCB_CHECK(ocb_dbglog_push(&r, 0x01, (uint8_t)i), "fill %u", i);
    }
    OCB_CHECK(!ocb_dbglog_push(&r, 0x01, 0xEE), "push into a full ring");
    OCB_CHECK(!ocb_dbglog_push(&r, 0x01, 0xEF), "push into a full ring again");
    OCB_CHECK(r.drops == 2u, "drops %u", r.drops);
    OCB_CHECK(ocb_dbglog_pop(&r, &v) && v == rec(0x01, 0), "the oldest survives: %04x", v);
    /* Index wrap: many push/pop pairs. */
    ocb_dbglog_ring_init(&r);
    r.head = 0xFFFFFFF0u;
    r.tail = 0xFFFFFFF0u;
    for (unsigned i = 0; i < 100u; i++) {
        OCB_CHECK(ocb_dbglog_push(&r, 0x03, (uint8_t)i) && ocb_dbglog_pop(&r, &v) && v == rec(0x03, (uint8_t)i),
                  "wrap %u", i);
    }
}

static void test_window(void)
{
    static ocb_dbglog_ring r;
    static ocb_dbglog_window w;
    ocb_dbglog_ring_init(&r);
    ocb_dbglog_window_init(&w);
    for (unsigned i = 0; i < 20u; i++) {
        (void)ocb_dbglog_push(&r, 0x02, (uint8_t)i);
    }
    OCB_CHECK(ocb_dbglog_drain(&w, &r) == 20u, "drain count");
    OCB_CHECK(w.n == 16u && w.total == 20u, "n %u total %u", w.n, w.total);
    /* The newest 16, oldest first: ids 4..19. */
    for (unsigned i = 0; i < 16u; i++) {
        const uint16_t got = w.rec[(w.start + i) % 16u];
        OCB_CHECK(got == rec(0x02, (uint8_t)(4u + i)), "window %u: %04x", i, got);
    }
    /* Overflow marker after the held records. */
    for (unsigned i = 0; i < 70u; i++) {
        (void)ocb_dbglog_push(&r, 0x01, (uint8_t)i);
    }
    OCB_CHECK(ocb_dbglog_drain(&w, &r) == 65u, "64 records + the overflow marker");
    OCB_CHECK(w.rec[(w.start + 15u) % 16u] == rec(0x05, 6), "overflow marker, 6 lost: %04x",
              w.rec[(w.start + 15u) % 16u]);
    /* total wraps at 65536. */
    w.total = 0xFFFFu;
    ocb_dbglog_window_add(&w, rec(0x07, 1));
    OCB_CHECK(w.total == 0u, "total wraps");
}

/* Story 2.3 (after bench H.15 attempt 1): repeated records are counted, not
 * stored, so a console that loops cannot fill the window. */
static void test_repeats(void)
{
    static ocb_dbglog_window w;
    ocb_dbglog_window_init(&w);
    /* The H.15 loop: sub 01, REFUSED 01, 30 times. */
    ocb_dbglog_window_add(&w, rec(0x02, 0x10));
    for (int i = 0; i < 30; i++) {
        ocb_dbglog_window_add(&w, rec(0x02, 0x01));
        ocb_dbglog_window_add(&w, rec(0x04, 0x01));
    }
    ocb_dbglog_window_add(&w, rec(0x02, 0x30));
    /* sub 10, sub 01, REFUSED 01, sub 01, REPEAT 57, sub 30. */
    static const uint16_t want[6] = {0x0210, 0x0201, 0x0401, 0x0201, 0x0939, 0x0230};
    OCB_CHECK(w.n == 6u && w.total == 6u, "pair loop: n %u total %u", w.n, w.total);
    for (unsigned i = 0; i < 6u && i < w.n; i++) {
        OCB_CHECK(w.rec[(w.start + i) % 16u] == want[i], "pair loop %u: %04x want %04x", i,
                  w.rec[(w.start + i) % 16u], want[i]);
    }
    /* A A A A: A, A, REPEAT 2. */
    ocb_dbglog_window_init(&w);
    for (int i = 0; i < 4; i++) {
        ocb_dbglog_window_add(&w, rec(0x01, 0x02));
    }
    OCB_CHECK(w.n == 3u && w.rec[2] == 0x0902, "same record: n %u last %04x", w.n, w.rec[2]);
    /* The handshake 80 02, 80 03, 80 02 is stored whole (no cycle yet). */
    ocb_dbglog_window_init(&w);
    ocb_dbglog_window_add(&w, rec(0x01, 0x01));
    ocb_dbglog_window_add(&w, rec(0x01, 0x02));
    ocb_dbglog_window_add(&w, rec(0x01, 0x03));
    ocb_dbglog_window_add(&w, rec(0x01, 0x02));
    ocb_dbglog_window_add(&w, rec(0x01, 0x04));
    OCB_CHECK(w.n == 5u && w.total == 5u, "handshake stored whole: n %u", w.n);
    /* The count saturates at 255. */
    ocb_dbglog_window_init(&w);
    for (int i = 0; i < 400; i++) {
        ocb_dbglog_window_add(&w, rec(0x02, 0x01));
    }
    OCB_CHECK(w.n == 3u && w.rec[2] == 0x09FF, "saturation: %04x", w.rec[2]);
}

static bool get_status(lh *h, ocb_msg_status *m)
{
    static lh_out o[16];
    const size_t n = lh_drain(h, o, 16u);
    for (size_t i = 0; i < n && i < 16u; i++) {
        if (lh_status_decode(&o[i], m)) {
            return true;
        }
    }
    return false;
}

static void test_status_tail(void)
{
    static lh h;
    static ocb_dbglog_ring r;
    lh_init_cfg(&h, 8u, true);
    lh_open_gate(&h);
    ocb_dbglog_ring_init(&r);
    /* The handshake the probe would decode: 80 01, 80 02, 80 03, 80 02,
     * 80 04, handshake, subcommand 02, refusal 11. */
    (void)ocb_dbglog_push(&r, 0x07, 0x01);
    (void)ocb_dbglog_push(&r, 0x01, 0x01);
    (void)ocb_dbglog_push(&r, 0x01, 0x02);
    (void)ocb_dbglog_push(&r, 0x01, 0x03);
    (void)ocb_dbglog_push(&r, 0x01, 0x02);
    (void)ocb_dbglog_push(&r, 0x01, 0x04);
    (void)ocb_dbglog_push(&r, 0x06, 0x00);
    (void)ocb_dbglog_push(&r, 0x02, 0x02);
    (void)ocb_dbglog_push(&r, 0x04, 0x11);
    (void)ocb_dbglog_drain(ocb_link_dbglog_window(&h.link), &r);
    ocb_msg_status m;
    OCB_CHECK(get_status(&h, &m), "no STATUS");
    /* 21 counters, then total 9 (09 00), n 9, records kind/id oldest first. */
    static const uint8_t tail[3 + 18] = {0x09, 0x00, 0x09, 0x07, 0x01, 0x01, 0x01, 0x01, 0x02, 0x01, 0x03, 0x01,
                                         0x02, 0x01, 0x04, 0x06, 0x00, 0x02, 0x02, 0x04, 0x11};
    OCB_CHECK(m.extra_len == 21u + sizeof tail + 41u + 5u, "extra %zu", m.extra_len);
    if (m.extra_len == 21u + sizeof tail + 41u + 5u) {
        OCB_CHECK_MEM(&m.extra[21], sizeof tail, tail, sizeof tail, "v3 tail");
    }
    /* A full window: 16 records, the largest STATUS (11 + 56 + 41 + 5 = 113
     * bytes: the story 2.5 v4 block of 41 bytes and the story 3.5 v5 block
     * of 5 included). */
    for (unsigned i = 0; i < 40u; i++) {
        (void)ocb_dbglog_push(&r, 0x02, (uint8_t)(0x40u + i));
    }
    (void)ocb_dbglog_drain(ocb_link_dbglog_window(&h.link), &r);
    lh_advance(&h, 50000u);
    OCB_CHECK(get_status(&h, &m) && m.extra_len == 102u, "full window: extra %zu", m.extra_len);
    if (m.extra_len == 102u) {
        OCB_CHECK(m.extra[21] == 49u && m.extra[22] == 0u && m.extra[23] == 16u, "total 49, n 16");
        OCB_CHECK(m.extra[24] == 0x02 && m.extra[25] == 0x58 && m.extra[54] == 0x02 && m.extra[55] == 0x67,
                  "oldest 0x58, newest 0x67");
    }
    /* STATUS cadence unchanged by the tail: the next one is 50 ms later. */
    lh_advance(&h, 49000u);
    OCB_CHECK(!get_status(&h, &m), "STATUS before 50 ms");
    lh_advance(&h, 1000u);
    OCB_CHECK(get_status(&h, &m), "STATUS at 50 ms");
    /* A too-small cap leaves it pending (never split). */
    lh_advance(&h, 50000u);
    uint8_t wire[40];
    OCB_CHECK(ocb_link_tx_next(&h.link, h.now, wire, sizeof wire) == 0u, "106-byte STATUS into 40 bytes");
    OCB_CHECK(get_status(&h, &m), "still pending");

    /* Release configuration: the 11-byte STATUS, no tail, even with records. */
    static lh rel;
    lh_init_cfg(&rel, 8u, false);
    lh_open_gate(&rel);
    ocb_dbglog_window_add(ocb_link_dbglog_window(&rel.link), rec(0x02, 0x02));
    OCB_CHECK(get_status(&rel, &m) && m.extra_len == 0u, "release STATUS: extra %zu", m.extra_len);
}

/* Story 2.5 D13: the v4 block, hand-written bytes for known inputs, at
 * offset 24 + 2 n of the tail for n = 0, 1 and 16. */
static void fill_v4_inputs(lh *h)
{
    ocb_core_timing_stats *t = &h->core.timing;
    t->n = 62u;
    t->late_max_us = 0x1234u;
    t->late_sum_us = 0x01020304u;
    t->hist[0] = 60u;
    t->hist[1] = 1u;
    t->hist[2] = 0u;
    t->hist[3] = 0u;
    t->hist[4] = 0u;
    t->hist[5] = 70000u; /* saturates to FF FF */
    ocb_usb_mailbox_set_in_spread(&h->mb, 7950u, 8050u, 16000u, 3u);
    ocb_usb_mailbox_set_core0(&h->mb, 412u, 0x10203u, 300u); /* retries saturate, fallbacks saturate to FF */
    ocb_link_set_self_timing(&h->link, 37u, 1100u);
    h->link.cfg.boot_cause = 2u;
    h->link.cfg.reboot_count = 5u;
    h->link.cfg.stall_reset_us = 21349u; /* D31: 2134 units of 10 us */
}

static const uint8_t k_v4[41] = {
    62u,   0u,    /* late_n */
    0x34u, 0x12u, /* late_max_us */
    0x04u, 0x03u, 0x02u, 0x01u, /* late_sum_us */
    60u,   0u,    1u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 0xFFu, 0xFFu, /* late_hist x 6 */
    37u,   0u,    /* loop_max_us */
    0x9Cu, 0x01u, /* core0_pass_max_us 412 */
    0x0Eu, 0x1Fu, /* in_min_us 7950 */
    0x72u, 0x1Fu, /* in_p95_us 8050 */
    0x80u, 0x3Eu, /* in_max_us 16000 */
    3u,    0u,    /* in_slow */
    0xFFu, 0xFFu, /* seq_retries 0x10203 saturated */
    0xFFu,        /* seq_fallbacks 300 saturated to u8 */
    0x4Cu, 0x04u, /* stack_used_max 1100 */
    2u,           /* boot_cause CORE1_STALE */
    5u,           /* reboot_count */
    0x56u, 0x08u, /* stall_reset_10us 2134 */
};

static void test_v4_block(void)
{
    static const unsigned ns[3] = {0u, 1u, 16u};
    for (int k = 0; k < 3; k++) {
        static lh h;
        static ocb_dbglog_ring r;
        lh_init_cfg(&h, 8u, true);
        lh_open_gate(&h);
        ocb_dbglog_ring_init(&r);
        for (unsigned i = 0; i < ns[k]; i++) {
            (void)ocb_dbglog_push(&r, 0x02, (uint8_t)(0x40u + i));
        }
        (void)ocb_dbglog_drain(ocb_link_dbglog_window(&h.link), &r);
        fill_v4_inputs(&h);
        ocb_msg_status m;
        OCB_CHECK(get_status(&h, &m), "n=%u: no STATUS", ns[k]);
        const size_t off = 24u + 2u * ns[k];
        OCB_CHECK(m.extra_len == off + 41u + 5u, "n=%u: extra %zu, wanted %zu", ns[k], m.extra_len, off + 46u);
        if (m.extra_len == off + 41u + 5u) {
            OCB_CHECK(m.extra[23] == ns[k], "n=%u: p2_log_n %u", ns[k], m.extra[23]);
            OCB_CHECK_MEM(&m.extra[off], 41u, k_v4, sizeof k_v4, "v4 block");
        }
    }
    /* Release: still the 11-byte STATUS, the v4 inputs set or not. */
    static lh rel;
    lh_init_cfg(&rel, 8u, false);
    lh_open_gate(&rel);
    fill_v4_inputs(&rel);
    ocb_msg_status m;
    OCB_CHECK(get_status(&rel, &m) && m.extra_len == 0u, "release STATUS with v4 inputs: extra %zu", m.extra_len);
}

static void expect_info_period(lh *h, uint16_t want, const char *what)
{
    lh_hello(h);
    static lh_out o[8];
    const size_t n = lh_drain(h, o, 8u);
    bool found = false;
    for (size_t i = 0; i < n && i < 8u; i++) {
        ocb_msg_info m;
        if (o[i].type == OCB_MSG_INFO && ocb_msg_info_decode(o[i].body, o[i].len, &m) == OCB_OK) {
            found = true;
            OCB_CHECK(m.report_period_us == want, "%s: INFO %u, wanted %u", what, m.report_period_us, want);
        }
    }
    OCB_CHECK(found, "%s: no INFO", what);
}

static void test_report_period(void)
{
    static lh h;
    lh_init(&h, 8u);
    lh_open_gate(&h);
    OCB_CHECK(h.mb.report_period_us == 0u, "mailbox starts at 0");
    expect_info_period(&h, 0u, "before any measurement");
    OCB_CHECK(h.core.late_us == OCB_CORE_LATE_FALLBACK_US, "fallback LATE");

    ocb_usb_mailbox_set_report_period(&h.mb, 1000u);
    expect_info_period(&h, 0u, "set but not polled yet");
    ocb_link_poll(&h.link, h.now);
    expect_info_period(&h, 1000u, "after the poll");
    OCB_CHECK(h.core.late_us == 1000u, "LATE %u", h.core.late_us);

    /* Back to 0 after a USB-down event (core 0 writes 0). */
    ocb_usb_mailbox_set_report_period(&h.mb, 0u);
    ocb_link_poll(&h.link, h.now);
    expect_info_period(&h, 0u, "after 0");
    OCB_CHECK(h.core.late_us == OCB_CORE_LATE_FALLBACK_US, "LATE fallback again: %u", h.core.late_us);

    /* Saturation above 65535. */
    ocb_usb_mailbox_set_report_period(&h.mb, 70000u);
    ocb_link_poll(&h.link, h.now);
    expect_info_period(&h, 65535u, "saturated");

    /* A direct set stays until the mailbox changes. */
    ocb_link_set_report_period(&h.link, 8000u);
    ocb_link_poll(&h.link, h.now);
    expect_info_period(&h, 8000u, "direct set kept");
    ocb_usb_mailbox_set_report_period(&h.mb, 2000u);
    ocb_link_poll(&h.link, h.now);
    expect_info_period(&h, 2000u, "mailbox change applied");

    /* A new link (restart of core 1) counts from zero: the mailbox's value
     * is applied at its first poll (D28 rebase rule of 2.6). */
    ocb_link_cfg cfg = h.link.cfg;
    ocb_link_init(&h.link, &h.core, &h.mb, &cfg, h.now);
    lh_open_gate(&h);
    ocb_link_poll(&h.link, h.now);
    expect_info_period(&h, 2000u, "new link applies the mailbox value");
}

int main(void)
{
    test_v4_block();
    test_ring();
    test_window();
    test_repeats();
    test_status_tail();
    test_report_period();
    const int f = ocb_test_failures();
    if (f != 0) {
        fprintf(stderr, "test_link_dbglog (F.31): %d failure(s)\n", f);
        return 1;
    }
    printf("test_link_dbglog (F.31): ok\n");
    return 0;
}
