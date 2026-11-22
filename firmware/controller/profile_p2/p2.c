/*
 * SPDX-License-Identifier: MIT
 *
 * P2 handshake, subcommands, SPI flash image and reply queue (story 2.3
 * D5 to D10, D17, D26). Written clean-room from the public documents listed
 * in ocb_p2/config.h (S1 to S9); no code of any source was used, and no
 * 2wiCC code. The structure (tables, queue, state) is our own.
 *
 * Fuzz rule (D17): one entry point per input, every length checked before
 * the byte is read, missing argument bytes read as zero, no global state,
 * no assert, no allocation, replies built into a fixed 64-byte slot.
 */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#include "ocb_p2/config.h"
#include "ocb_p2/internal.h"
#include "ocb_p2/p2.h"
#include "ocb_proto/state.h"

/* ------------------------------------------------------------------ */
/* Emulated SPI flash (D7): a function of the address, FF elsewhere.   */
/* ------------------------------------------------------------------ */

typedef struct {
    uint32_t addr;
    uint8_t len;
    const uint8_t *data;
} spi_region;

/* S4 0x6000: serial number, ASCII; 16 bytes, FF padded. */
static const uint8_t k_spi_serial[16] = {'0', '0', '0', '0', '0', '0', '0', '0', '0', '0', '0', '1',
                                         0xFFu, 0xFFu, 0xFFu, 0xFFu};
/* S4 0x6012: device type, Pro Controller. */
static const uint8_t k_spi_type[1] = {OCB_P2_DEVICE_TYPE_PRO};
/* S4 0x601B: colour info absent, default colours. */
static const uint8_t k_spi_colour_flag[1] = {OCB_P2_COLOUR_FLAG};
/* S4 0x6020: 6-axis factory calibration, 12 x int16 LE: accelerometer
 * origin 0, accelerometer sensitivity coefficient 0x4000 (S4 default for
 * +-8 G), gyro origin 0, gyro coefficient 0x343B (S4 default for
 * +-2000 dps). The IMU reports zeros anyway (D7). */
static const uint8_t k_spi_imu_cal[24] = {
    0x00u, 0x00u, 0x00u, 0x00u, 0x00u, 0x00u, /* acc origin X Y Z */
    0x00u, 0x40u, 0x00u, 0x40u, 0x00u, 0x40u, /* acc coefficient 0x4000 (S4) */
    0x00u, 0x00u, 0x00u, 0x00u, 0x00u, 0x00u, /* gyro origin X Y Z */
    0x3Bu, 0x34u, 0x3Bu, 0x34u, 0x3Bu, 0x34u, /* gyro coefficient 0x343B (S4) */
};
/* S4 0x603D, left stick: max above centre (X, Y), centre, min below centre,
 * 12-bit pairs packed in 3 bytes (S4 decode code); values S9: range 0x700,
 * centre 0x800. S4: a Pro Controller needs deltas of at least 0x542 / 0x4D5. */
static const uint8_t k_spi_left_cal[9] = {0x00u, 0x07u, 0x70u, 0x00u, 0x08u, 0x80u, 0x00u, 0x07u, 0x70u};
/* S4 0x6046, right stick: centre, min below, max above (S4 order), S9 values. */
static const uint8_t k_spi_right_cal[9] = {0x00u, 0x08u, 0x80u, 0x00u, 0x07u, 0x70u, 0x00u, 0x07u, 0x70u};
/* S4 0x6080: 6-axis horizontal offsets, Pro Controller defaults X -688
 * (0xFD50), Y 0, Z 4038 (0x0FC6), int16 LE. */
static const uint8_t k_spi_imu_offsets[6] = {0x50u, 0xFDu, 0x00u, 0x00u, 0xC6u, 0x0Fu};
/* S4 0x6086 and 0x6098: stick device parameters 1 and 2 ("normally the
 * same"), S4's sample values 019 4CD, 0AE (dead zone) E14 (range ratio),
 * 2EE 2EE, 2EE 2EE, AB4 AB4, 496 496, packed as the calibration (S4). */
static const uint8_t k_spi_stick_params[18] = {0x19u, 0xD0u, 0x4Cu, 0xAEu, 0x40u, 0xE1u,
                                               0xEEu, 0xE2u, 0x2Eu, 0xEEu, 0xE2u, 0x2Eu,
                                               0xB4u, 0x4Au, 0xABu, 0x96u, 0x64u, 0x49u};

static const spi_region k_spi[] = {
    {0x6000u, 16u, k_spi_serial},        {0x6012u, 1u, k_spi_type},
    {0x601Bu, 1u, k_spi_colour_flag},    {0x6020u, 24u, k_spi_imu_cal},
    {0x603Du, 9u, k_spi_left_cal},       {0x6046u, 9u, k_spi_right_cal},
    {0x6080u, 6u, k_spi_imu_offsets},    {0x6086u, 18u, k_spi_stick_params},
    {0x6098u, 18u, k_spi_stick_params},
};

/* One byte of the emulated flash. User calibration (0x8010.., S4) has no
 * magic and reads FF, so a host uses the factory calibration (S6). */
static uint8_t spi_byte(uint32_t addr)
{
    for (size_t i = 0; i < sizeof k_spi / sizeof k_spi[0]; i++) {
        if (addr >= k_spi[i].addr && addr - k_spi[i].addr < (uint32_t)k_spi[i].len) {
            return k_spi[i].data[addr - k_spi[i].addr];
        }
    }
    return OCB_P2_SPI_ERASED;
}

/* ------------------------------------------------------------------ */
/* Reply queue (D6).                                                   */
/* ------------------------------------------------------------------ */

/* A zeroed slot at the tail, or NULL when full. */
static uint8_t *q_slot(ocb_p2 *p)
{
    if (p->q_count >= (uint8_t)OCB_P2_QUEUE_DEPTH) {
        return NULL;
    }
    const uint8_t i = (uint8_t)((p->q_head + p->q_count) % (uint8_t)OCB_P2_QUEUE_DEPTH);
    uint8_t *s = p->q[i];
    memset(s, 0, OCB_P2_REPORT_LEN);
    return s;
}

static void q_commit(ocb_p2 *p)
{
    p->q_count++;
    p->cnt.replies++;
}

static void log_add(ocb_p2_log *log, uint8_t kind, uint8_t id)
{
    if (log != NULL && log->n < 2u) {
        log->kind[log->n] = kind;
        log->id[log->n] = id;
        log->n++;
    }
}

/* A 0x81 reply (S1): 81 <cmd> then the data, zero padded. */
static ocb_p2_rx reply_usb(ocb_p2 *p, uint8_t cmd, const uint8_t *data, size_t n)
{
    uint8_t *s = q_slot(p);
    if (s == NULL) {
        p->cnt.queue_full++;
        return OCB_P2_RX_QUEUE_FULL;
    }
    s[0] = OCB_P2_RID_IN_USB;
    s[1] = cmd;
    if (n > OCB_P2_REPORT_LEN - 2u) {
        n = OCB_P2_REPORT_LEN - 2u;
    }
    if (n > 0u) {
        memcpy(&s[2], data, n);
    }
    q_commit(p);
    return OCB_P2_RX_HANDLED;
}

/* A 0x21 reply (S2): [0] 0x21, [1..12] header filled when sent, [13] ack,
 * [14] subcommand id, [15..49] data. */
static ocb_p2_rx reply_sub(ocb_p2 *p, uint8_t ack, uint8_t id, const uint8_t *data, size_t n)
{
    uint8_t *s = q_slot(p);
    if (s == NULL) {
        p->cnt.queue_full++;
        return OCB_P2_RX_QUEUE_FULL;
    }
    s[0] = OCB_P2_RID_IN_REPLY;
    s[OCB_P2_REPLY_ACK_OFFSET] = ack;
    s[OCB_P2_REPLY_ACK_OFFSET + 1u] = id;
    if (n > OCB_P2_REPLY_DATA_MAX) {
        n = OCB_P2_REPLY_DATA_MAX;
    }
    if (n > 0u) {
        memcpy(&s[OCB_P2_REPLY_DATA_OFFSET], data, n);
    }
    q_commit(p);
    return OCB_P2_RX_HANDLED;
}

/* ------------------------------------------------------------------ */
/* State.                                                              */
/* ------------------------------------------------------------------ */

void ocb_p2_init(ocb_p2 *p, const ocb_p2_cfg *cfg)
{
    memset(p, 0, sizeof *p);
    p->cfg = *cfg;
    ocb_p2_interval_clear_(&p->iv);
}

void ocb_p2_restart(ocb_p2 *p)
{
    p->streaming = false;
    p->handshake_pending = false;
    p->handshake_seen = false;
    p->q_head = 0u;
    p->q_count = 0u;
    ocb_p2_interval_clear_(&p->iv);
    p->cnt.restarts++;
}

static void stream_on(ocb_p2 *p, ocb_p2_log *log)
{
    p->streaming = true;
    if (!p->handshake_seen) {
        p->handshake_seen = true;
        p->handshake_pending = true;
        p->cnt.handshakes++;
        log_add(log, OCB_P2_LOG_HANDSHAKE, 0u);
    }
}

bool ocb_p2_ready(const ocb_p2 *p)
{
    return p->streaming;
}

bool ocb_p2_take_handshake(ocb_p2 *p)
{
    const bool h = p->handshake_pending;
    p->handshake_pending = false;
    return h;
}

bool ocb_p2_reply_pending(const ocb_p2 *p)
{
    return p->q_count > 0u;
}

static ocb_p2_rx refuse(ocb_p2 *p, uint8_t id, ocb_p2_log *log)
{
    p->cnt.refusals++;
    log_add(log, OCB_P2_LOG_REFUSAL, id);
    return OCB_P2_RX_REFUSED;
}

/* ------------------------------------------------------------------ */
/* USB 0x80 commands (S1, S5, S6).                                     */
/* ------------------------------------------------------------------ */

static ocb_p2_rx on_usb(ocb_p2 *p, const uint8_t *buf, size_t len, ocb_p2_log *log)
{
    if (len < 2u) {
        p->cnt.malformed++;
        return OCB_P2_RX_MALFORMED;
    }
    const uint8_t cmd = buf[1];
    log_add(log, OCB_P2_LOG_USB_CMD, cmd);
    switch (cmd) {
    case OCB_P2_USB_STATUS: {
        /* S1: 81 01 00 <type> <MAC reversed>; S6 reads byte 3 as the type
         * and bytes 4..9 as the MAC in reverse order. */
        uint8_t d[8];
        d[0] = 0x00u;
        d[1] = OCB_P2_DEVICE_TYPE_PRO;
        for (size_t i = 0; i < 6u; i++) {
            d[2u + i] = p->cfg.mac[5u - i];
        }
        return reply_usb(p, cmd, d, sizeof d);
    }
    case OCB_P2_USB_HANDSHAKE: /* S1: 81 02, any count (D5) */
    case OCB_P2_USB_BAUD:      /* S1: 81 03; the line stays USB (D5) */
        return reply_usb(p, cmd, NULL, 0u);
    case OCB_P2_USB_NO_TIMEOUT: /* S1, S7: the 0x30 stream starts after it */
        stream_on(p, log);
        return OCB_P2_RX_HANDLED;
    case OCB_P2_USB_TIMEOUT: /* S1, S5: allow the timeout; cautious: stream off (D5) */
        p->streaming = false;
        return OCB_P2_RX_HANDLED;
    case OCB_P2_USB_RESET:    /* S1 "possibly resets" (D9) */
    case OCB_P2_USB_UART_PRE: /* S1 UART pass-through (D9) */
    case OCB_P2_USB_UART_ANY:
        return refuse(p, cmd, log);
    default:
        p->cnt.ignored++;
        return OCB_P2_RX_IGNORED;
    }
}

/* ------------------------------------------------------------------ */
/* Subcommands (S3).                                                   */
/* ------------------------------------------------------------------ */

/* Argument byte i of a 0x01 report, 0 past the end (D17). */
static uint8_t arg(const uint8_t *buf, size_t len, size_t i)
{
    const size_t at = OCB_P2_SUBCMD_ARG_OFFSET + i;
    return at < len ? buf[at] : 0u;
}

static ocb_p2_rx spi_read(ocb_p2 *p, const uint8_t *buf, size_t len)
{
    /* S3: address uint32 LE, size uint8, max 0x1D; reply 90 10, the
     * address and size echoed, then the data (S3 example, S5 offsets). */
    const uint32_t addr = (uint32_t)arg(buf, len, 0u) | ((uint32_t)arg(buf, len, 1u) << 8) |
                          ((uint32_t)arg(buf, len, 2u) << 16) | ((uint32_t)arg(buf, len, 3u) << 24);
    const uint8_t n = arg(buf, len, 4u);
    if (n > OCB_P2_SPI_READ_MAX) {
        return reply_sub(p, OCB_P2_NACK, OCB_P2_SC_SPI_READ, NULL, 0u);
    }
    uint8_t d[5u + OCB_P2_SPI_READ_MAX];
    d[0] = (uint8_t)(addr & 0xFFu);
    d[1] = (uint8_t)((addr >> 8) & 0xFFu);
    d[2] = (uint8_t)((addr >> 16) & 0xFFu);
    d[3] = (uint8_t)((addr >> 24) & 0xFFu);
    d[4] = n;
    for (uint8_t i = 0u; i < n; i++) {
        /* No wrap: past 0xFFFFFFFF the flash is erased, never data. */
        const uint32_t a = addr + (uint32_t)i;
        d[5u + i] = a < addr ? OCB_P2_SPI_ERASED : spi_byte(a);
    }
    return reply_sub(p, OCB_P2_ACK_SPI_READ, OCB_P2_SC_SPI_READ, d, 5u + (size_t)n);
}

/* A refused subcommand (C7, D9): S2 "a NACK is always 00", the id echoed. */
static ocb_p2_rx nack_refuse(ocb_p2 *p, uint8_t id, ocb_p2_log *log)
{
    const ocb_p2_rx r = reply_sub(p, OCB_P2_NACK, id, NULL, 0u);
    (void)refuse(p, id, log);
    return r == OCB_P2_RX_HANDLED ? OCB_P2_RX_REFUSED : r;
}

static ocb_p2_rx on_subcmd(ocb_p2 *p, const uint8_t *buf, size_t len, ocb_p2_log *log)
{
    if (len < OCB_P2_SUBCMD_ARG_OFFSET) {
        p->cnt.malformed++; /* the 11-byte header is missing: no reply */
        return OCB_P2_RX_MALFORMED;
    }
    /* The 8 rumble bytes [2..9] are consumed and ignored (D23). */
    const uint8_t id = buf[OCB_P2_SUBCMD_ID_OFFSET];
    log_add(log, OCB_P2_LOG_SUBCMD, id);
    ocb_p2_rx r;
    switch (id) {
    case OCB_P2_SC_DEVICE_INFO: {
        /* S3: fw version (2), type, 02, MAC big endian (6), 01, colour flag. */
        uint8_t d[12];
        d[0] = OCB_P2_FW_MAJOR;
        d[1] = OCB_P2_FW_MINOR;
        d[2] = OCB_P2_DEVICE_TYPE_PRO;
        d[3] = OCB_P2_INFO_CONST_02;
        memcpy(&d[4], p->cfg.mac, 6u);
        d[10] = OCB_P2_INFO_CONST_01;
        d[11] = OCB_P2_COLOUR_FLAG;
        return reply_sub(p, OCB_P2_ACK_DEVICE_INFO, id, d, sizeof d);
    }
    case OCB_P2_SC_INPUT_MODE: {
        /* S3: one argument; 0x30 = standard full mode starts the stream (D5). */
        const uint8_t mode = arg(buf, len, 0u);
        r = reply_sub(p, OCB_P2_ACK, id, NULL, 0u);
        /* A request whose reply was dropped (QUEUE_FULL) did not happen:
         * the host will resend it, so no setting changes (review fix). */
        if (r == OCB_P2_RX_HANDLED && len > OCB_P2_SUBCMD_ARG_OFFSET) {
            p->input_mode = mode;
            if (mode == OCB_P2_INPUT_MODE_FULL) {
                stream_on(p, log);
            }
        }
        return r;
    }
    case OCB_P2_SC_ELAPSED: {
        /* S3: seven uint16 LE, 10 ms units; all 0 (S9 ack 0x83). */
        const uint8_t d[14] = {0u};
        return reply_sub(p, OCB_P2_ACK_ELAPSED, id, d, sizeof d);
    }
    case OCB_P2_SC_SHIPMENT:     /* S3: ack */
    case OCB_P2_SC_HOME_LIGHT:   /* S3: ack */
    case OCB_P2_SC_IMU_SENS:     /* S3: ack */
        return reply_sub(p, OCB_P2_ACK, id, NULL, 0u);
    case OCB_P2_SC_PLAYER_LIGHTS:
        r = reply_sub(p, OCB_P2_ACK, id, NULL, 0u);
        if (r == OCB_P2_RX_HANDLED) {
            p->player_lights = arg(buf, len, 0u);
        }
        return r;
    case OCB_P2_SC_IMU_ENABLE:
        r = reply_sub(p, OCB_P2_ACK, id, NULL, 0u);
        if (r == OCB_P2_RX_HANDLED) {
            p->imu_enabled = arg(buf, len, 0u) != 0u;
        }
        return r;
    case OCB_P2_SC_VIBRATION:
        r = reply_sub(p, OCB_P2_ACK, id, NULL, 0u);
        if (r == OCB_P2_RX_HANDLED) {
            p->vibration_enabled = arg(buf, len, 0u) != 0u;
        }
        return r;
    case OCB_P2_SC_SPI_READ:
        return spi_read(p, buf, len);
    case OCB_P2_SC_SPI_WRITE:
    case OCB_P2_SC_SPI_ERASE: {
        /* C7: S3 "replies x8011 / x8012 ack and a uint8 status, x01 = write
         * protected"; nothing is written (the image has no writable state). */
        const uint8_t d[1] = {OCB_P2_WRITE_PROTECTED};
        r = reply_sub(p, OCB_P2_ACK, id, d, sizeof d);
        (void)refuse(p, id, log);
        return r == OCB_P2_RX_HANDLED ? OCB_P2_RX_REFUSED : r;
    }
    case OCB_P2_SC_MCU_STATE:
        if (arg(buf, len, 0u) != OCB_P2_MCU_STATE_UPDATE) {
            return reply_sub(p, OCB_P2_ACK, id, NULL, 0u); /* suspend / resume (S3) */
        }
        return nack_refuse(p, id, log); /* 0x02 "resume for update" (S3) */
    case OCB_P2_SC_BT_PAIRING:    /* C7: pairing */
    case OCB_P2_SC_HCI_STATE:     /* reboot / disconnect request */
    case OCB_P2_SC_RESET_PAIRING: /* C7: erases pairing */
    case OCB_P2_SC_MCU_RESET:     /* NFC/IR MCU reset */
    case OCB_P2_SC_MCU_CONFIG:    /* S3: carries MCU firmware update data */
        return nack_refuse(p, id, log);
    default: {
        /* S3: "all subcommands that do nothing reply 80 <id> and 03". */
        const uint8_t d[1] = {OCB_P2_UNUSED_DATA};
        return reply_sub(p, OCB_P2_ACK, id, d, sizeof d);
    }
    }
}

ocb_p2_rx ocb_p2_on_output(ocb_p2 *p, const uint8_t *buf, size_t len, ocb_p2_log *log)
{
    if (log != NULL) {
        log->n = 0u;
    }
    p->cnt.out_reports++;
    if (buf == NULL || len == 0u) {
        p->cnt.malformed++;
        return OCB_P2_RX_MALFORMED;
    }
    switch (buf[0]) {
    case OCB_P2_RID_OUT_USB:
        return on_usb(p, buf, len, log);
    case OCB_P2_RID_OUT_SUBCMD:
        return on_subcmd(p, buf, len, log);
    case OCB_P2_RID_OUT_RUMBLE:
        /* Rumble only (S2): consumed, no reply, no log (D23). */
        return OCB_P2_RX_HANDLED;
    case OCB_P2_RID_OUT_MCU_FW:
        /* S2: NFC/IR MCU firmware update packet (C7, D26). */
        log_add(log, OCB_P2_LOG_OTHER_REPORT, buf[0]);
        return refuse(p, buf[0], log);
    default:
        log_add(log, OCB_P2_LOG_OTHER_REPORT, buf[0]);
        p->cnt.ignored++;
        return OCB_P2_RX_IGNORED;
    }
}

ocb_p2_rx ocb_p2_on_feature(ocb_p2 *p, uint8_t report_id, ocb_p2_log *log)
{
    if (log != NULL) {
        log->n = 0u;
    }
    log_add(log, OCB_P2_LOG_OTHER_REPORT, report_id);
    return refuse(p, report_id, log);
}

size_t ocb_p2_next_in(ocb_p2 *p, const ocb_state *published, uint8_t *out, size_t cap)
{
    if (out == NULL || published == NULL || cap < OCB_P2_REPORT_LEN) {
        return 0u;
    }
    if (p->q_count > 0u) {
        const uint8_t *s = p->q[p->q_head];
        memcpy(out, s, OCB_P2_REPORT_LEN);
        p->q_head = (uint8_t)((p->q_head + 1u) % (uint8_t)OCB_P2_QUEUE_DEPTH);
        p->q_count--;
        if (out[0] == OCB_P2_RID_IN_REPLY) {
            ocb_p2_header_(p, published, out);
        }
        p->cnt.in_reports++;
        return OCB_P2_REPORT_LEN;
    }
    if (!p->streaming) {
        return 0u;
    }
    const size_t n = ocb_p2_build_report(p, published, out, cap);
    if (n > 0u) {
        p->cnt.in_reports++;
    }
    return n;
}
