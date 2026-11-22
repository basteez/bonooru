/*
 * SPDX-License-Identifier: MIT
 *
 * F.26 (story 2.3 Task 3.4, AC3): every subcommand of the table through
 * output report 0x01 (and the rumble-only 0x10), the emulated SPI flash
 * region by region, and the refusals (C7, D9) that must change nothing.
 * Expected bytes hand-written from S2 (0x21 layout, NACK = 00), S3
 * (subcommand replies), S4 (SPI map and packing) and S9 (stick
 * calibration values, ack bytes 82 / 83).
 */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#include "ocb_p2/p2.h"
#include "ocb_test.h"
#include "p2_fixture.h"

/* Header bytes [2..12] of a 0x21 reply with NEUTRAL (S2, S7, D8). */
static const uint8_t k_hdr[11] = {0x91, 0x00, 0x80, 0x00, 0x00, 0x08, 0x80, 0x00, 0x08, 0x80, 0x0A};

/* Sends subcommand `id`, then checks the 0x21 reply: header, ack, echo,
 * data and the zero padding. */
static void expect_sub(ocb_p2 *p, uint8_t id, const uint8_t *args, size_t nargs, uint8_t ack, const uint8_t *data,
                       size_t ndata, const char *what)
{
    (void)fx_sub(p, id, args, nargs, NULL);
    uint8_t got[64];
    const size_t n = fx_in(p, got);
    uint8_t want[64] = {0};
    want[0] = 0x21;
    want[1] = got[1]; /* timer: checked in F.27 */
    memcpy(&want[2], k_hdr, sizeof k_hdr);
    want[13] = ack;
    want[14] = id;
    if (ndata > 0u) {
        memcpy(&want[15], data, ndata);
    }
    OCB_CHECK_MEM(got, n, want, sizeof want, what);
}

/* SPI read of `len` at `addr`; expects ack 90 and `data`. */
static void expect_spi(ocb_p2 *p, uint32_t addr, uint8_t len, const uint8_t *data, const char *what)
{
    uint8_t a[5] = {(uint8_t)addr, (uint8_t)(addr >> 8), (uint8_t)(addr >> 16), (uint8_t)(addr >> 24), len};
    uint8_t d[5 + 0x1D];
    memcpy(d, a, 5);
    if (len > 0u) {
        memcpy(&d[5], data, len);
    }
    expect_sub(p, 0x10, a, sizeof a, 0x90, d, 5u + len, what);
}

static void spi_all_ff(ocb_p2 *p, uint32_t addr, uint8_t len, const char *what)
{
    uint8_t ff[0x1D];
    memset(ff, 0xFF, sizeof ff);
    expect_spi(p, addr, len, ff, what);
}

/* The regions of the SPI table (D7), byte for byte. */
static void check_spi_regions(ocb_p2 *p)
{
    /* S4 0x6000: serial, ASCII, FF padded. */
    static const uint8_t serial[16] = {'0', '0', '0', '0', '0', '0', '0', '0',
                                       '0', '0', '0', '1', 0xFF, 0xFF, 0xFF, 0xFF};
    expect_spi(p, 0x6000, 16, serial, "SPI 0x6000 serial");
    static const uint8_t type[1] = {0x03};
    expect_spi(p, 0x6012, 1, type, "SPI 0x6012 type");
    static const uint8_t colour[1] = {0x00};
    expect_spi(p, 0x601B, 1, colour, "SPI 0x601B colour flag");
    /* S4: acc coefficient 0x4000, gyro 0x343B, int16 LE, origins 0. */
    static const uint8_t imu[24] = {0, 0, 0, 0, 0, 0, 0x00, 0x40, 0x00, 0x40, 0x00, 0x40,
                                    0, 0, 0, 0, 0, 0, 0x3B, 0x34, 0x3B, 0x34, 0x3B, 0x34};
    expect_spi(p, 0x6020, 24, imu, "SPI 0x6020 IMU calibration");
    /* 0x603D, 18 bytes as S5 / S6 read it: left then right (S4 order, S9 values). */
    static const uint8_t sticks[18] = {0x00, 0x07, 0x70, 0x00, 0x08, 0x80, 0x00, 0x07, 0x70,
                                       0x00, 0x08, 0x80, 0x00, 0x07, 0x70, 0x00, 0x07, 0x70};
    expect_spi(p, 0x603D, 18, sticks, "SPI 0x603D stick calibration");
    /* S4 Pro Controller horizontal offsets X -688, Y 0, Z 4038. */
    static const uint8_t offs[6] = {0x50, 0xFD, 0x00, 0x00, 0xC6, 0x0F};
    expect_spi(p, 0x6080, 6, offs, "SPI 0x6080 IMU offsets");
    /* S4 stick parameters sample 019 4CD AE E14 2EE 2EE 2EE 2EE AB4 AB4 496 496. */
    static const uint8_t params[18] = {0x19, 0xD0, 0x4C, 0xAE, 0x40, 0xE1, 0xEE, 0xE2, 0x2E,
                                       0xEE, 0xE2, 0x2E, 0xB4, 0x4A, 0xAB, 0x96, 0x64, 0x49};
    expect_spi(p, 0x6086, 18, params, "SPI 0x6086 stick parameters 1");
    expect_spi(p, 0x6098, 18, params, "SPI 0x6098 stick parameters 2");
    /* S4: 0x6080 (6) + 0x6086 (18) read as one 24-byte block (mzyy94's S7 read). */
    {
        uint8_t both[24];
        memcpy(both, offs, 6);
        memcpy(&both[6], params, 18);
        expect_spi(p, 0x6080, 24, both, "SPI 0x6080 24 bytes");
    }
    /* User calibration absent: no B2 A1 magic (S4, S6). */
    spi_all_ff(p, 0x8010, 0x16, "SPI 0x8010 user calibration");
    spi_all_ff(p, 0x8026, 0x1A, "SPI 0x8026 user IMU calibration");
    spi_all_ff(p, 0x6050, 13, "SPI 0x6050 colours");
    spi_all_ff(p, 0x2000, 0x1D, "SPI 0x2000 pairing");
    spi_all_ff(p, 0x8000, 0x10, "SPI 0x8000");
}

/* Before / after snapshot of everything a refusal must not change. */
static void check_unchanged(const ocb_p2 *before, const ocb_p2 *after, const char *what)
{
    ocb_p2 a;
    memcpy(&a, before, sizeof a);
    /* Allowed to move: counters, the timer (the NACK reply used it), the
     * queue storage. Everything else is compared byte for byte. */
    a.cnt = after->cnt;
    a.timer = after->timer;
    memcpy(a.q, after->q, sizeof a.q);
    a.q_head = after->q_head;
    OCB_CHECK(memcmp(&a, after, sizeof a) == 0, "%s changed the profile state", what);
    OCB_CHECK(after->cnt.refusals == before->cnt.refusals + 1u, "%s: refusal count %u -> %u", what,
              before->cnt.refusals, after->cnt.refusals);
}

int main(void)
{
    ocb_p2 p;
    fx_init(&p);
    (void)fx_usb(&p, 0x04, NULL); /* streaming: replies still come first */

    /* Device info (S3): fw 04 00 (D10), type 03, 02, MAC big endian, 01, colour 00. */
    {
        static const uint8_t d[12] = {0x04, 0x00, 0x03, 0x02, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x01, 0x00};
        expect_sub(&p, 0x02, NULL, 0, 0x82, d, sizeof d, "02 device info");
    }
    {
        const uint8_t m[1] = {0x30};
        expect_sub(&p, 0x03, m, 1, 0x80, NULL, 0, "03 input mode 30");
        OCB_CHECK(p.input_mode == 0x30, "input mode stored");
    }
    {
        static const uint8_t z[14] = {0};
        expect_sub(&p, 0x04, NULL, 0, 0x83, z, sizeof z, "04 elapsed time");
    }
    {
        const uint8_t a[1] = {0x00};
        expect_sub(&p, 0x08, a, 1, 0x80, NULL, 0, "08 shipment");
    }
    {
        const uint8_t a[1] = {0x01};
        expect_sub(&p, 0x30, a, 1, 0x80, NULL, 0, "30 player lights");
        OCB_CHECK(p.player_lights == 0x01, "player lights stored");
        const uint8_t b[1] = {0x0F};
        expect_sub(&p, 0x30, b, 1, 0x80, NULL, 0, "30 player lights 0F");
        OCB_CHECK(p.player_lights == 0x0F, "player lights stored again");
    }
    {
        const uint8_t a[1] = {0x01};
        expect_sub(&p, 0x38, a, 1, 0x80, NULL, 0, "38 home light");
        expect_sub(&p, 0x40, a, 1, 0x80, NULL, 0, "40 IMU on");
        OCB_CHECK(p.imu_enabled, "IMU flag");
        const uint8_t s[4] = {0x03, 0x00, 0x00, 0x01};
        expect_sub(&p, 0x41, s, 4, 0x80, NULL, 0, "41 IMU sensitivity");
        expect_sub(&p, 0x48, a, 1, 0x80, NULL, 0, "48 vibration on");
        OCB_CHECK(p.vibration_enabled, "vibration flag");
        const uint8_t z[1] = {0x00};
        expect_sub(&p, 0x48, z, 1, 0x80, NULL, 0, "48 vibration off");
        OCB_CHECK(!p.vibration_enabled, "vibration flag off");
    }
    /* MCU state: 00 and 01 plain ack (S3). */
    {
        const uint8_t a0[1] = {0x00};
        const uint8_t a1[1] = {0x01};
        expect_sub(&p, 0x22, a0, 1, 0x80, NULL, 0, "22 00 suspend");
        expect_sub(&p, 0x22, a1, 1, 0x80, NULL, 0, "22 01 resume");
    }
    /* Unused ids (S3): 80 <id> 03. */
    {
        const uint8_t d[1] = {0x03};
        expect_sub(&p, 0x7F, NULL, 0, 0x80, d, 1, "7F unused");
        expect_sub(&p, 0x00, NULL, 0, 0x80, d, 1, "00 controller state only");
        expect_sub(&p, 0x33, NULL, 0, 0x80, d, 1, "33 unused (S5 sends it)");
        expect_sub(&p, 0xFF, NULL, 0, 0x80, d, 1, "FF unused");
    }

    /* SPI flash. */
    check_spi_regions(&p);
    {
        /* Length limit: 0x1D ok, 0x1E and 0xFF NACK (S3 max x1D). */
        spi_all_ff(&p, 0x0000, 0x1D, "SPI 0x1D bytes");
        const uint8_t l1e[5] = {0x00, 0x60, 0x00, 0x00, 0x1E};
        expect_sub(&p, 0x10, l1e, 5, 0x00, NULL, 0, "SPI 0x1E is a NACK");
        const uint8_t lff[5] = {0x00, 0x60, 0x00, 0x00, 0xFF};
        expect_sub(&p, 0x10, lff, 5, 0x00, NULL, 0, "SPI 0xFF is a NACK");
        /* Zero length: the echo only. */
        expect_spi(&p, 0x6000, 0, NULL, "SPI zero length");
        /* 32-bit edge: 0xFFFFFFF0 + 0x1D never wraps into data at 0x0..0xC. */
        spi_all_ff(&p, 0xFFFFFFF0u, 0x1D, "SPI address edge");
        /* High address bits are not ignored: 0x00016000 is not 0x6000. */
        spi_all_ff(&p, 0x00016000u, 0x10, "SPI 0x16000");
        /* A read straddling a region edge: 0x5FFE..0x6001. */
        static const uint8_t edge[4] = {0xFF, 0xFF, '0', '0'};
        expect_spi(&p, 0x5FFE, 4, edge, "SPI straddling 0x6000");
        /* Missing arguments read as zero (D17): address 0, length 0. */
        expect_spi(&p, 0x0000, 0, NULL, "SPI read without arguments");
    }

    /* Rumble-only 0x10: consumed, no reply, no log (D23). */
    {
        uint8_t b[10] = {0x10, 0x01, 0x00, 0x01, 0x40, 0x40, 0x00, 0x01, 0x40, 0x40};
        ocb_p2_log log;
        OCB_CHECK(ocb_p2_on_output(&p, b, sizeof b, &log) == OCB_P2_RX_HANDLED, "0x10 handled");
        OCB_CHECK(log.n == 0u && !ocb_p2_reply_pending(&p), "0x10: no log, no reply");
        uint8_t got[64];
        OCB_CHECK(fx_in(&p, got) == 64u && got[0] == 0x30, "0x10: the stream continues");
    }
    /* A 0x01 report shorter than its 11-byte header: MALFORMED, no reply. */
    for (size_t n = 1; n < 11u; n++) {
        uint8_t b[11] = {0x01, 0, 0x00, 0x01, 0x40, 0x40, 0x00, 0x01, 0x40, 0x40, 0x02};
        OCB_CHECK(ocb_p2_on_output(&p, b, n, NULL) == OCB_P2_RX_MALFORMED, "0x01 of %zu bytes", n);
        OCB_CHECK(!ocb_p2_reply_pending(&p), "no reply to a short 0x01");
    }
    /* Exactly 11 bytes: the id, no argument. */
    {
        uint8_t b[11] = {0x01, 0, 0x00, 0x01, 0x40, 0x40, 0x00, 0x01, 0x40, 0x40, 0x08};
        OCB_CHECK(ocb_p2_on_output(&p, b, sizeof b, NULL) == OCB_P2_RX_HANDLED, "11-byte 0x01");
        uint8_t got[64];
        OCB_CHECK(fx_in(&p, got) == 64u && got[13] == 0x80 && got[14] == 0x08, "11-byte 0x01 reply");
    }
    /* Report id 0x82 (declared, undocumented) and others: ignored, logged. */
    {
        uint8_t b[2] = {0x82, 0x00};
        ocb_p2_log log;
        OCB_CHECK(ocb_p2_on_output(&p, b, sizeof b, &log) == OCB_P2_RX_IGNORED, "0x82 ignored");
        OCB_CHECK(log.n == 1u && log.kind[0] == 0x03 && log.id[0] == 0x82, "0x82 logged");
    }

    /* Refusals (C7, D9): reply as documented, nothing changes. */
    {
        uint8_t got[64];
        while (fx_in(&p, got) > 0u && got[0] != 0x30) {
        }
        struct {
            uint8_t id;
            uint8_t args[6];
            size_t nargs;
            uint8_t ack;
            uint8_t data;
            size_t ndata;
        } const refused[] = {
            {0x11, {0x00, 0x80, 0x00, 0x00, 0x01, 0x00}, 6, 0x80, 0x01, 1}, /* SPI write at 0x8000: write protected */
            {0x11, {0x3D, 0x60, 0x00, 0x00, 0x01, 0xAA}, 6, 0x80, 0x01, 1}, /* SPI write over the calibration */
            {0x12, {0x00, 0x60, 0x00, 0x00, 0, 0}, 4, 0x80, 0x01, 1},       /* sector erase: write protected (S3) */
            {0x01, {0x01, 0x16, 0x30, 0xAA, 0x82, 0xBB}, 6, 0x00, 0, 0},     /* BT manual pairing */
            {0x07, {0}, 0, 0x00, 0, 0},                                      /* reset pairing */
            {0x06, {0x02}, 1, 0x00, 0, 0},                                   /* HCI: reboot to pair */
            {0x20, {0}, 0, 0x00, 0, 0},                                      /* MCU reset */
            {0x21, {0x21, 0x00, 0x01}, 3, 0x00, 0, 0},                       /* MCU config (firmware data) */
            {0x22, {0x02}, 1, 0x00, 0, 0},                                   /* MCU resume for update */
        };
        for (size_t k = 0; k < sizeof refused / sizeof refused[0]; k++) {
            ocb_p2 before;
            memcpy(&before, &p, sizeof before);
            ocb_p2_log log;
            const ocb_p2_rx r = fx_sub(&p, refused[k].id, refused[k].args, refused[k].nargs, &log);
            OCB_CHECK(r == OCB_P2_RX_REFUSED, "subcommand %02x must be refused (got %d)", refused[k].id, (int)r);
            OCB_CHECK(log.n == 2u && log.kind[0] == 0x02 && log.kind[1] == 0x04 && log.id[1] == refused[k].id,
                      "refusal log of %02x", refused[k].id);
            const size_t n = fx_in(&p, got);
            uint8_t want[64] = {0};
            want[0] = 0x21;
            want[1] = got[1];
            memcpy(&want[2], k_hdr, sizeof k_hdr);
            want[13] = refused[k].ack;
            want[14] = refused[k].id;
            if (refused[k].ndata > 0u) {
                want[15] = refused[k].data;
            }
            OCB_CHECK_MEM(got, n, want, sizeof want, "refusal reply");
            check_unchanged(&before, &p, "refused subcommand");
        }
        /* USB commands 80 06, 80 91, 80 92: no reply, refused, nothing changes. */
        static const uint8_t usb_refused[3] = {0x06, 0x91, 0x92};
        for (size_t k = 0; k < 3u; k++) {
            ocb_p2 before;
            memcpy(&before, &p, sizeof before);
            ocb_p2_log log;
            OCB_CHECK(fx_usb(&p, usb_refused[k], &log) == OCB_P2_RX_REFUSED, "80 %02x refused", usb_refused[k]);
            OCB_CHECK(log.n == 2u && log.kind[0] == 0x01 && log.kind[1] == 0x04 && log.id[1] == usb_refused[k],
                      "log of 80 %02x", usb_refused[k]);
            OCB_CHECK(!ocb_p2_reply_pending(&p), "80 %02x: no reply", usb_refused[k]);
            check_unchanged(&before, &p, "refused USB command");
        }
        /* Output report 0x03 (MCU firmware update packet, S2) and feature
         * reports (S2 0x70..0x74, OTA update): refused, no reply (D26). */
        {
            ocb_p2 before;
            memcpy(&before, &p, sizeof before);
            uint8_t b[64] = {0x03, 0x01, 0x02};
            ocb_p2_log log;
            OCB_CHECK(ocb_p2_on_output(&p, b, sizeof b, &log) == OCB_P2_RX_REFUSED, "output 0x03 refused");
            OCB_CHECK(log.n == 2u && log.kind[1] == 0x04 && log.id[1] == 0x03, "log of output 0x03");
            OCB_CHECK(!ocb_p2_reply_pending(&p), "output 0x03: no reply");
            check_unchanged(&before, &p, "output 0x03");
            for (uint8_t f = 0x70; f <= 0x74; f++) {
                memcpy(&before, &p, sizeof before);
                OCB_CHECK(ocb_p2_on_feature(&p, f, &log) == OCB_P2_RX_REFUSED, "feature %02x refused", f);
                OCB_CHECK(log.n == 2u && log.kind[0] == 0x03 && log.id[0] == f && log.kind[1] == 0x04,
                          "log of feature %02x", f);
                check_unchanged(&before, &p, "feature report");
            }
        }
        /* Every SPI region reads back the same after all refusals. */
        check_spi_regions(&p);
        OCB_CHECK(ocb_p2_ready(&p), "the stream survives every refusal");
    }

    /* The refusals never stall the stream: 4 refusals queue 4 NACKs, the
     * 5th gets QUEUE_FULL but is still counted as a refusal. */
    {
        fx_init(&p);
        for (int k = 0; k < 4; k++) {
            OCB_CHECK(fx_sub(&p, 0x07, NULL, 0, NULL) == OCB_P2_RX_REFUSED, "refusal %d", k);
        }
        OCB_CHECK(fx_sub(&p, 0x07, NULL, 0, NULL) == OCB_P2_RX_QUEUE_FULL, "5th refusal: queue full");
        OCB_CHECK(p.cnt.refusals == 5u, "5 refusals counted");
    }

    return ocb_test_failures() == 0 ? 0 : 1;
}
