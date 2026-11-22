/*
 * SPDX-License-Identifier: MIT
 *
 * Host test F.9: ocb_proto against the language-neutral conformance vectors
 * in common/proto/vectors/ (AD-14; format in vectors/README.md, story 2.2 D2
 * and D3). Every file's proto_version header must equal OCB_PROTO_VERSION.
 * Failures are reported as vectors-file:line: case: message; the run goes on
 * to the end so one run shows every problem.
 *
 * Usage: test_vectors [VECTORS_DIR]   (default: OCB_VECTORS_DIR)
 */
#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include "ocb_proto/ocb_proto.h"
#include "ocb_test.h"

#ifndef OCB_VECTORS_DIR
#error "OCB_VECTORS_DIR must be defined by the build"
#endif

#define MAX_LINE 4096
#define MAX_BYTES 2048
#define MAX_FIELDS 8
#define MAX_CASES 128
#define MAX_KV 16
#define MAX_STREAM_FRAMES 16

/* ------------------------------------------------------------------ reporting */

static const char *g_file = "";
static int g_cases_total;

static void vfail(int line, const char *case_name, const char *fmt, ...)
{
    va_list ap;
    ocb_test_fail_count++;
    fprintf(stderr, "%s:%d: %s: ", g_file, line, case_name[0] ? case_name : "-");
    va_start(ap, fmt);
    vfprintf(stderr, fmt, ap);
    va_end(ap);
    fprintf(stderr, "\n");
}

static void vfail_mem(int line, const char *case_name, const char *what, const uint8_t *got,
                      size_t got_n, const uint8_t *exp, size_t exp_n)
{
    vfail(line, case_name, "%s differs", what);
    ocb_test_hexdump("got     ", got, got_n);
    ocb_test_hexdump("expected", exp, exp_n);
}

static int mem_eq(const uint8_t *a, size_t an, const uint8_t *b, size_t bn)
{
    return an == bn && (an == 0 || memcmp(a, b, an) == 0);
}

/* ------------------------------------------------------------------ parsing */

typedef struct {
    char field[32];
    char value[MAX_LINE];
    int line;
} vfield;

typedef struct {
    char name[64];
    int line;
    int n;
    vfield f[MAX_FIELDS];
} vcase;

static int hexval(char c)
{
    if (c >= '0' && c <= '9') {
        return c - '0';
    }
    if (c >= 'a' && c <= 'f') {
        return c - 'a' + 10;
    }
    if (c >= 'A' && c <= 'F') {
        return c - 'A' + 10;
    }
    return -1;
}

/* Hex bytes in wire order, optional single spaces between bytes. */
static int parse_hex(const char *s, uint8_t *out, size_t cap, size_t *n)
{
    size_t k = 0;
    const char *p = s;
    while (*p != '\0') {
        if (*p == ' ') {
            if (p == s || p[1] == ' ' || p[1] == '\0') {
                return -1;
            }
            p++;
            continue;
        }
        const int hi = hexval(p[0]);
        const int lo = hi < 0 ? -1 : hexval(p[1]);
        if (hi < 0 || lo < 0 || k >= cap) {
            return -1;
        }
        out[k++] = (uint8_t)((hi << 4) | lo);
        p += 2;
    }
    *n = k;
    return 0;
}

/* A big-endian hex number of exactly `digits` digits (0 = any even count 2..8). */
static int parse_num(const char *s, size_t digits, uint32_t *v)
{
    const size_t len = strlen(s);
    if (digits != 0 ? len != digits : (len < 2 || len > 8 || len % 2 != 0)) {
        return -1;
    }
    uint32_t acc = 0;
    for (size_t i = 0; i < len; i++) {
        const int h = hexval(s[i]);
        if (h < 0) {
            return -1;
        }
        acc = (acc << 4) | (uint32_t)h;
    }
    *v = acc;
    return 0;
}

static const vfield *get(const vcase *c, const char *field)
{
    for (int i = 0; i < c->n; i++) {
        if (strcmp(c->f[i].field, field) == 0) {
            return &c->f[i];
        }
    }
    return NULL;
}

/* Rejects fields not in `allowed` (NULL-terminated). Returns 0 if all known. */
static int check_allowed(const vcase *c, const char *const *allowed)
{
    int bad = 0;
    for (int i = 0; i < c->n; i++) {
        int ok = 0;
        for (const char *const *a = allowed; *a != NULL; a++) {
            if (strcmp(c->f[i].field, *a) == 0) {
                ok = 1;
            }
        }
        if (!ok) {
            vfail(c->f[i].line, c->name, "unknown field '%s'", c->f[i].field);
            bad = 1;
        }
    }
    return bad;
}

static const vfield *need(const vcase *c, const char *field)
{
    const vfield *f = get(c, field);
    if (f == NULL) {
        vfail(c->line, c->name, "missing field '%s'", field);
    }
    return f;
}

static int need_hex(const vcase *c, const char *field, uint8_t *out, size_t cap, size_t *n)
{
    const vfield *f = need(c, field);
    if (f == NULL) {
        return -1;
    }
    if (parse_hex(f->value, out, cap, n) != 0) {
        vfail(f->line, c->name, "%s: bad hex bytes '%s'", field, f->value);
        return -1;
    }
    return 0;
}

/* name=value lists (fields, stats). */
typedef struct {
    char name[32];
    char value[600];
} kv;

static int parse_kv_list(const char *s, kv *out, int cap, int *n)
{
    int k = 0;
    const char *p = s;
    while (*p != '\0') {
        const char *end = strchr(p, ' ');
        const size_t tl = end != NULL ? (size_t)(end - p) : strlen(p);
        const char *eq = memchr(p, '=', tl);
        if (tl == 0 || eq == NULL || k >= cap) {
            return -1;
        }
        const size_t nl = (size_t)(eq - p);
        const size_t vl = tl - nl - 1;
        if (nl == 0 || nl >= sizeof out[k].name || vl == 0 || vl >= sizeof out[k].value) {
            return -1;
        }
        memcpy(out[k].name, p, nl);
        out[k].name[nl] = '\0';
        memcpy(out[k].value, eq + 1, vl);
        out[k].value[vl] = '\0';
        for (int j = 0; j < k; j++) {
            if (strcmp(out[j].name, out[k].name) == 0) {
                return -1;
            }
        }
        k++;
        p += tl;
        if (*p == ' ') {
            p++;
            if (*p == '\0' || *p == ' ') {
                return -1;
            }
        }
    }
    *n = k;
    return 0;
}

static const kv *kv_get(const kv *list, int n, const char *name)
{
    for (int i = 0; i < n; i++) {
        if (strcmp(list[i].name, name) == 0) {
            return &list[i];
        }
    }
    return NULL;
}

static int parse_err_name(const char *s, ocb_err *e)
{
    static const struct {
        const char *name;
        ocb_err err;
    } names[] = {
        {"COBS", OCB_ERR_COBS}, {"CRC", OCB_ERR_CRC}, {"RANGE", OCB_ERR_RANGE},
        {"BODY_LEN", OCB_ERR_BODY_LEN}, {"EMPTY", OCB_ERR_EMPTY}, {"OVERLONG", OCB_ERR_OVERLONG}};
    for (size_t i = 0; i < sizeof names / sizeof names[0]; i++) {
        if (strcmp(s, names[i].name) == 0) {
            *e = names[i].err;
            return 0;
        }
    }
    return -1;
}

static int parse_side(const vcase *c, ocb_rx_side *side)
{
    const vfield *f = get(c, "rx");
    *side = OCB_RX_CONTROLLER;
    if (f == NULL || strcmp(f->value, "CONTROLLER") == 0) {
        return 0;
    }
    if (strcmp(f->value, "APP") == 0) {
        *side = OCB_RX_APP;
        return 0;
    }
    vfail(f->line, c->name, "rx must be CONTROLLER or APP, got '%s'", f->value);
    return -1;
}

/* ------------------------------------------------------------------ areas */

typedef void (*case_fn)(const vcase *c);

static void area_cobs(const vcase *c)
{
    static const char *const allowed[] = {"raw", "enc", "err", NULL};
    uint8_t raw[MAX_BYTES], enc[MAX_BYTES], buf[MAX_BYTES];
    size_t raw_n = 0, enc_n = 0, n = 0;

    if (check_allowed(c, allowed) || need_hex(c, "enc", enc, sizeof enc, &enc_n) != 0) {
        return;
    }
    const vfield *err = get(c, "err");
    if (err != NULL) {
        if (strcmp(err->value, "COBS") != 0 || get(c, "raw") != NULL) {
            vfail(err->line, c->name, "a cobs error case has err: COBS and no raw");
            return;
        }
        if (ocb_cobs_decode(enc, enc_n, buf, sizeof buf, &n) == OCB_COBS_OK) {
            vfail(c->line, c->name, "decode succeeded, expected a COBS error");
        }
        return;
    }
    if (need_hex(c, "raw", raw, sizeof raw, &raw_n) != 0) {
        return;
    }
    n = ocb_cobs_encode(raw, raw_n, buf, sizeof buf);
    if (!mem_eq(buf, n, enc, enc_n)) {
        vfail_mem(c->line, c->name, "encode(raw)", buf, n, enc, enc_n);
    }
    if (n > 0 && ocb_cobs_encode(raw, raw_n, buf, enc_n) != enc_n) {
        vfail(c->line, c->name, "encode with out_cap = encoded length failed");
    }
    if (enc_n > 0 && ocb_cobs_encode(raw, raw_n, buf, enc_n - 1) != 0) {
        vfail(c->line, c->name, "encode with out_cap = encoded length - 1 did not fail");
    }
    if (enc_n > OCB_COBS_MAX_ENCODED(raw_n)) {
        vfail(c->line, c->name, "encoded length above OCB_COBS_MAX_ENCODED");
    }
    const ocb_cobs_result r = ocb_cobs_decode(enc, enc_n, buf, sizeof buf, &n);
    if (r != OCB_COBS_OK) {
        vfail(c->line, c->name, "decode(enc) failed with %d", (int)r);
    } else if (!mem_eq(buf, n, raw, raw_n)) {
        vfail_mem(c->line, c->name, "decode(enc)", buf, n, raw, raw_n);
    }
}

static void area_crc16(const vcase *c)
{
    static const char *const allowed[] = {"data", "crc", NULL};
    uint8_t data[MAX_BYTES], crc[MAX_BYTES];
    size_t data_n = 0, crc_n = 0;

    if (check_allowed(c, allowed) || need_hex(c, "data", data, sizeof data, &data_n) != 0 ||
        need_hex(c, "crc", crc, sizeof crc, &crc_n) != 0) {
        return;
    }
    if (crc_n != 2) {
        vfail(c->line, c->name, "crc must be 2 bytes");
        return;
    }
    const uint16_t v = ocb_crc16(data, data_n);
    const uint8_t got[2] = {(uint8_t)(v & 0xFF), (uint8_t)(v >> 8)};
    if (!mem_eq(got, 2, crc, 2)) {
        vfail_mem(c->line, c->name, "crc16(data) as little-endian bytes", got, 2, crc, 2);
    }
}

/* Encodes payload (type seq body) and compares with wire + delimiter. */
static void check_frame_encode(const vcase *c, const uint8_t *payload, size_t payload_n,
                               const uint8_t *wire, size_t wire_n)
{
    uint8_t out[OCB_MAX_WIRE_FRAME + 8];
    if (payload_n < 2) {
        vfail(c->line, c->name, "payload needs at least type and seq");
        return;
    }
    const size_t n =
        ocb_frame_encode(payload[0], payload[1], payload + 2, payload_n - 2, out, sizeof out);
    if (n == 0 || out[n - 1] != 0x00 || !mem_eq(out, n - 1, wire, wire_n)) {
        vfail_mem(c->line, c->name, "frame_encode(payload) (delimiter stripped)", out,
                  n > 0 ? n - 1 : 0, wire, wire_n);
    }
}

static void check_frame_decode_ok(const vcase *c, ocb_rx_side side, const uint8_t *wire,
                                  size_t wire_n, const uint8_t *payload, size_t payload_n)
{
    ocb_frame fr;
    uint8_t got[2 + OCB_MAX_BODY];
    const ocb_err e = ocb_frame_decode(side, wire, wire_n, &fr);
    if (e != OCB_OK) {
        vfail(c->line, c->name, "frame_decode(wire) failed with %d", (int)e);
        return;
    }
    got[0] = fr.type;
    got[1] = fr.seq;
    memcpy(&got[2], fr.body, fr.len);
    if (!mem_eq(got, 2u + fr.len, payload, payload_n)) {
        vfail_mem(c->line, c->name, "frame_decode(wire) payload", got, 2u + fr.len, payload,
                  payload_n);
    }
}

static void area_frames(const vcase *c)
{
    static const char *const allowed[] = {"payload", "wire", "err", "rx", NULL};
    uint8_t payload[MAX_BYTES], wire[MAX_BYTES];
    size_t payload_n = 0, wire_n = 0;
    ocb_rx_side side;
    ocb_frame fr;

    if (check_allowed(c, allowed) || parse_side(c, &side) != 0 ||
        need_hex(c, "wire", wire, sizeof wire, &wire_n) != 0) {
        return;
    }
    const vfield *errf = get(c, "err");
    const int has_payload = get(c, "payload") != NULL;
    if (has_payload && need_hex(c, "payload", payload, sizeof payload, &payload_n) != 0) {
        return;
    }
    if (errf != NULL) {
        /* An error case must leave *out untouched. */
        memset(&fr, 0x5A, sizeof fr);
        ocb_err want;
        if (parse_err_name(errf->value, &want) != 0 || want == OCB_ERR_BODY_LEN) {
            vfail(errf->line, c->name, "bad frame error name '%s'", errf->value);
            return;
        }
        const ocb_err e = ocb_frame_decode(side, wire, wire_n, &fr);
        if (e != want) {
            vfail(c->line, c->name, "frame_decode returned %d, expected %s (%d)", (int)e,
                  errf->value, (int)want);
        }
        const uint8_t *fb = (const uint8_t *)&fr;
        for (size_t k = 0; k < sizeof fr; k++) {
            if (fb[k] != 0x5A) {
                vfail(c->line, c->name, "frame_decode wrote *out on an error");
                break;
            }
        }
        if (has_payload) {
            check_frame_encode(c, payload, payload_n, wire, wire_n);
        }
        return;
    }
    if (!has_payload) {
        vfail(c->line, c->name, "an ok frame case needs payload");
        return;
    }
    check_frame_encode(c, payload, payload_n, wire, wire_n);
    check_frame_decode_ok(c, side, wire, wire_n, payload, payload_n);
}

/* ---- messages: one descriptor per message, fields by offset */

typedef union {
    ocb_msg_hello hello;
    ocb_msg_info info;
    ocb_msg_set_state set_state;
    ocb_msg_queue_event queue_event;
    ocb_msg_start start;
    ocb_msg_heartbeat heartbeat;
    ocb_msg_status status;
    ocb_msg_ack ack;
    ocb_msg_nak nak;
    ocb_msg_event event;
} msg_u;

typedef enum { K_U8, K_U16, K_U32, K_A3, K_STATE, K_EXTRA } fkind;

typedef struct {
    const char *name;
    fkind kind;
    size_t off;
} fdesc;

typedef int (*enc_fn)(const msg_u *m, uint8_t *body, size_t cap);
typedef int (*dec_fn)(const uint8_t *body, size_t len, msg_u *m);

#define WRAP(x)                                                                                    \
    static int enc_##x(const msg_u *m, uint8_t *b, size_t cap)                                     \
    {                                                                                              \
        return ocb_msg_##x##_encode(&m->x, b, cap);                                                \
    }                                                                                              \
    static int dec_##x(const uint8_t *b, size_t len, msg_u *m)                                     \
    {                                                                                              \
        return ocb_msg_##x##_decode(b, len, &m->x);                                                \
    }
WRAP(hello)
WRAP(info)
WRAP(set_state)
WRAP(queue_event)
WRAP(start)
WRAP(heartbeat)
WRAP(status)
WRAP(ack)
WRAP(nak)
WRAP(event)

static int enc_stop(const msg_u *m, uint8_t *b, size_t cap)
{
    (void)m;
    return ocb_msg_stop_encode(b, cap);
}
static int dec_stop(const uint8_t *b, size_t len, msg_u *m)
{
    (void)m;
    return ocb_msg_stop_decode(b, len);
}
static int enc_ping(const msg_u *m, uint8_t *b, size_t cap)
{
    (void)m;
    return ocb_msg_ping_encode(b, cap);
}
static int dec_ping(const uint8_t *b, size_t len, msg_u *m)
{
    (void)m;
    return ocb_msg_ping_decode(b, len);
}

#define F(msg, field, kind) {#field, kind, offsetof(ocb_msg_##msg, field)}
static const fdesc f_hello[] = {F(hello, proto_ver, K_U8)};
static const fdesc f_info[] = {F(info, proto_ver, K_U8),       F(info, fw_ver, K_A3),
                               F(info, bridge_ver, K_A3),      F(info, hid_profile, K_U8),
                               F(info, queue_cap, K_U16),      F(info, app_timeout_ms, K_U16),
                               F(info, report_period_us, K_U16)};
static const fdesc f_set_state[] = {F(set_state, state, K_STATE)};
static const fdesc f_queue_event[] = {F(queue_event, t_us, K_U32), F(queue_event, state, K_STATE)};
static const fdesc f_start[] = {F(start, lead_us, K_U32)};
static const fdesc f_heartbeat[] = {F(heartbeat, bridge_fw_ver, K_A3), F(heartbeat, flags, K_U8)};
static const fdesc f_status[] = {F(status, free, K_U16),       F(status, now_us, K_U32),
                                 F(status, last_seq, K_U8),    F(status, mode, K_U8),
                                 F(status, abort_count, K_U8), F(status, last_event, K_U8),
                                 F(status, flags, K_U8),       F(status, extra, K_EXTRA)};
static const fdesc f_ack[] = {F(ack, seq, K_U8)};
static const fdesc f_nak[] = {F(nak, reason, K_U8), F(nak, expected, K_U8), F(nak, got, K_U8)};
static const fdesc f_event[] = {F(event, code, K_U8)};
#undef F

typedef struct {
    uint8_t type;
    const char *name;
    const fdesc *fields;
    size_t nfields;
    enc_fn enc;
    dec_fn dec;
    int ok_cases;
    int err_cases;
} mdesc;

#define M(t, x, f) {t, #x, f, sizeof f / sizeof f[0], enc_##x, dec_##x, 0, 0}
#define M0(t, x) {t, #x, NULL, 0, enc_##x, dec_##x, 0, 0}
static mdesc g_msgs[] = {
    M(OCB_MSG_HELLO, hello, f_hello),
    M(OCB_MSG_SET_STATE, set_state, f_set_state),
    M(OCB_MSG_QUEUE_EVENT, queue_event, f_queue_event),
    M(OCB_MSG_START, start, f_start),
    M0(OCB_MSG_STOP, stop),
    M0(OCB_MSG_PING, ping),
    M(OCB_MSG_HEARTBEAT, heartbeat, f_heartbeat),
    M(OCB_MSG_INFO, info, f_info),
    M(OCB_MSG_STATUS, status, f_status),
    M(OCB_MSG_ACK, ack, f_ack),
    M(OCB_MSG_NAK, nak, f_nak),
    M(OCB_MSG_EVENT, event, f_event),
};
#undef M
#undef M0
#define N_MSGS (sizeof g_msgs / sizeof g_msgs[0])

static mdesc *find_msg(uint8_t type)
{
    for (size_t i = 0; i < N_MSGS; i++) {
        if (g_msgs[i].type == type) {
            return &g_msgs[i];
        }
    }
    return NULL;
}

static size_t kind_digits(fkind k)
{
    switch (k) {
    case K_U8:
        return 2;
    case K_U16:
        return 4;
    case K_U32:
        return 8;
    default:
        return 0;
    }
}

/* Writes the value text of field `d` into the struct. `extra_buf` keeps the
 * STATUS extra bytes alive. Returns 0 or -1 (reported). */
static int field_set(const vcase *c, int line, const fdesc *d, const char *text, msg_u *m,
                     uint8_t *extra_buf, size_t extra_cap)
{
    char *base = (char *)m;
    uint32_t v = 0;
    uint8_t bytes[MAX_BYTES];
    size_t n = 0;
    switch (d->kind) {
    case K_U8:
    case K_U16:
    case K_U32:
        if (parse_num(text, kind_digits(d->kind), &v) != 0) {
            vfail(line, c->name, "field %s: expected %zu hex digits, got '%s'", d->name,
                  kind_digits(d->kind), text);
            return -1;
        }
        if (d->kind == K_U8) {
            const uint8_t b = (uint8_t)v;
            memcpy(base + d->off, &b, 1);
        } else if (d->kind == K_U16) {
            const uint16_t w = (uint16_t)v;
            memcpy(base + d->off, &w, 2);
        } else {
            memcpy(base + d->off, &v, 4);
        }
        return 0;
    case K_A3:
        if (parse_hex(text, bytes, sizeof bytes, &n) != 0 || n != 3) {
            vfail(line, c->name, "field %s: expected 3 bytes, got '%s'", d->name, text);
            return -1;
        }
        memcpy(base + d->off, bytes, 3);
        return 0;
    case K_STATE:
        if (parse_hex(text, bytes, sizeof bytes, &n) != 0 || n != OCB_STATE_SIZE) {
            vfail(line, c->name, "field %s: expected 8 bytes, got '%s'", d->name, text);
            return -1;
        }
        ocb_state_decode(bytes, (ocb_state *)(void *)(base + d->off));
        return 0;
    case K_EXTRA:
        if (parse_hex(text, extra_buf, extra_cap, &n) != 0) {
            vfail(line, c->name, "field %s: bad hex '%s'", d->name, text);
            return -1;
        }
        m->status.extra = extra_buf;
        m->status.extra_len = n;
        return 0;
    }
    return -1;
}

static void area_messages(const vcase *c)
{
    static const char *const allowed[] = {"type", "seq", "fields", "body", "wire",
                                          "trailing", "err", NULL};
    uint8_t tb[4], sb[4], body[MAX_BYTES], wire[MAX_BYTES], trailing[MAX_BYTES];
    uint8_t buf[MAX_BYTES], extra_buf[MAX_BYTES];
    size_t tn = 0, sn = 0, body_n = 0, wire_n = 0, trailing_n = 0;
    kv list[MAX_KV];
    int nkv = 0;

    if (check_allowed(c, allowed) || need_hex(c, "type", tb, sizeof tb, &tn) != 0 ||
        need_hex(c, "body", body, sizeof body, &body_n) != 0) {
        return;
    }
    if (tn != 1) {
        vfail(c->line, c->name, "type must be one byte");
        return;
    }
    mdesc *md = find_msg(tb[0]);
    if (md == NULL) {
        vfail(c->line, c->name, "type %02x is not a known message", tb[0]);
        return;
    }
    msg_u m;
    memset(&m, 0, sizeof m);

    const vfield *errf = get(c, "err");
    if (errf != NULL) {
        ocb_err want;
        if (parse_err_name(errf->value, &want) != 0 || want != OCB_ERR_BODY_LEN) {
            vfail(errf->line, c->name, "message errors must be BODY_LEN");
            return;
        }
        const int r = md->dec(body, body_n, &m);
        if (r != OCB_ERR_BODY_LEN) {
            vfail(c->line, c->name, "%s decode returned %d, expected BODY_LEN", md->name, r);
        }
        md->err_cases++;
        return;
    }

    const vfield *ff = need(c, "fields");
    if (ff == NULL || need_hex(c, "seq", sb, sizeof sb, &sn) != 0 ||
        need_hex(c, "wire", wire, sizeof wire, &wire_n) != 0) {
        return;
    }
    if (sn != 1) {
        vfail(c->line, c->name, "seq must be one byte");
        return;
    }
    if (get(c, "trailing") != NULL &&
        need_hex(c, "trailing", trailing, sizeof trailing, &trailing_n) != 0) {
        return;
    }
    if (parse_kv_list(ff->value, list, MAX_KV, &nkv) != 0) {
        vfail(ff->line, c->name, "malformed fields list");
        return;
    }
    for (int i = 0; i < nkv; i++) {
        int known = 0;
        for (size_t k = 0; k < md->nfields; k++) {
            known |= strcmp(list[i].name, md->fields[k].name) == 0;
        }
        if (!known) {
            vfail(ff->line, c->name, "unknown %s field '%s'", md->name, list[i].name);
            return;
        }
    }

    /* Build the expected struct from the fields. */
    msg_u want;
    memset(&want, 0, sizeof want);
    for (size_t k = 0; k < md->nfields; k++) {
        const kv *e = kv_get(list, nkv, md->fields[k].name);
        if (e == NULL) {
            if (md->fields[k].kind == K_EXTRA) {
                continue;
            }
            vfail(ff->line, c->name, "missing %s field '%s'", md->name, md->fields[k].name);
            return;
        }
        if (field_set(c, ff->line, &md->fields[k], e->value, &want, extra_buf, sizeof extra_buf) !=
            0) {
            return;
        }
    }

    /* encode(fields) + trailing == body */
    const int en = md->enc(&want, buf, sizeof buf);
    if (en < 0 || (size_t)en + trailing_n > sizeof buf) {
        vfail(c->line, c->name, "%s encode failed with %d", md->name, en);
    } else {
        memcpy(buf + en, trailing, trailing_n);
        if (!mem_eq(buf, (size_t)en + trailing_n, body, body_n)) {
            vfail_mem(c->line, c->name, "encode(fields) + trailing", buf, (size_t)en + trailing_n,
                      body, body_n);
        }
    }

    /* decode(body) == fields */
    const int dr = md->dec(body, body_n, &m);
    if (dr != OCB_OK) {
        vfail(c->line, c->name, "%s decode returned %d", md->name, dr);
    } else {
        for (size_t k = 0; k < md->nfields; k++) {
            const fdesc *d = &md->fields[k];
            if (d->kind == K_EXTRA) {
                const size_t wl = want.status.extra_len;
                const uint8_t *gp = m.status.extra;
                const size_t gl = m.status.extra_len;
                if (gl != wl || (wl > 0 && (gp == NULL || memcmp(gp, want.status.extra, wl) != 0))) {
                    vfail(ff->line, c->name, "decoded extra differs (len %zu, expected %zu)", gl,
                          wl);
                }
                continue;
            }
            size_t sz = 0;
            uint8_t ga[OCB_STATE_SIZE], wa[OCB_STATE_SIZE];
            switch (d->kind) {
            case K_U8:
                sz = 1;
                break;
            case K_U16:
                sz = 2;
                break;
            case K_U32:
                sz = 4;
                break;
            case K_A3:
                sz = 3;
                break;
            default:
                sz = 0;
                break;
            }
            if (d->kind == K_STATE) {
                ocb_state_encode((const ocb_state *)(const void *)((const char *)&m + d->off), ga);
                ocb_state_encode((const ocb_state *)(const void *)((const char *)&want + d->off),
                                 wa);
                if (memcmp(ga, wa, sizeof ga) != 0) {
                    vfail_mem(ff->line, c->name, "decoded state", ga, sizeof ga, wa, sizeof wa);
                }
            } else if (memcmp((const char *)&m + d->off, (const char *)&want + d->off, sz) != 0) {
                vfail(ff->line, c->name, "decoded field '%s' differs", d->name);
            }
        }
    }

    /* The frame: encode_frame(type, seq, body) == wire, and it decodes back. */
    uint8_t payload[2 + MAX_BYTES];
    payload[0] = tb[0];
    payload[1] = sb[0];
    memcpy(payload + 2, body, body_n);
    check_frame_encode(c, payload, body_n + 2, wire, wire_n);
    check_frame_decode_ok(c, tb[0] >= OCB_TYPE_CTRL_MIN ? OCB_RX_APP : OCB_RX_CONTROLLER, wire,
                          wire_n, payload, body_n + 2);
    md->ok_cases++;
}

static void area_streams(const vcase *c)
{
    static const char *const allowed[] = {"in", "rx", "frames", "stats", NULL};
    static uint8_t in[MAX_BYTES];
    static uint8_t want[MAX_STREAM_FRAMES][2 + OCB_MAX_BODY];
    static size_t want_n[MAX_STREAM_FRAMES];
    size_t in_n = 0;
    int nwant = 0;
    ocb_rx_side side;

    if (check_allowed(c, allowed) || parse_side(c, &side) != 0 ||
        need_hex(c, "in", in, sizeof in, &in_n) != 0) {
        return;
    }
    const vfield *ff = need(c, "frames");
    const vfield *sf = need(c, "stats");
    if (ff == NULL || sf == NULL) {
        return;
    }
    /* frames: ';'-separated payloads */
    if (ff->value[0] != '\0') {
        const char *p = ff->value;
        for (;;) {
            const char *semi = strchr(p, ';');
            const size_t len = semi != NULL ? (size_t)(semi - p) : strlen(p);
            char tmp[MAX_LINE];
            size_t a = 0, b = len;
            while (a < b && p[a] == ' ') {
                a++;
            }
            while (b > a && p[b - 1] == ' ') {
                b--;
            }
            memcpy(tmp, p + a, b - a);
            tmp[b - a] = '\0';
            if (nwant >= MAX_STREAM_FRAMES || b == a ||
                parse_hex(tmp, want[nwant], sizeof want[nwant], &want_n[nwant]) != 0) {
                vfail(ff->line, c->name, "malformed frames list");
                return;
            }
            nwant++;
            if (semi == NULL) {
                break;
            }
            p = semi + 1;
        }
    }
    /* stats: all six keys in order */
    static const char *const keys[] = {"ok", "empty", "cobs", "crc", "range", "overlong"};
    kv list[MAX_KV];
    int nkv = 0;
    uint32_t sv[6];
    if (parse_kv_list(sf->value, list, MAX_KV, &nkv) != 0 || nkv != 6) {
        vfail(sf->line, c->name, "stats needs ok empty cobs crc range overlong");
        return;
    }
    for (int i = 0; i < 6; i++) {
        unsigned long v = 0;
        char *end = NULL;
        if (strcmp(list[i].name, keys[i]) != 0) {
            vfail(sf->line, c->name, "stats key %d must be '%s'", i, keys[i]);
            return;
        }
        for (end = list[i].value; *end >= '0' && *end <= '9'; end++) {
            v = v * 10u + (unsigned long)(*end - '0');
        }
        if (*end != '\0' || end == list[i].value) {
            vfail(sf->line, c->name, "stats value for '%s' is not a decimal", keys[i]);
            return;
        }
        sv[i] = (uint32_t)v;
    }

    static ocb_rx rx;
    ocb_frame fr;
    int got = 0;
    ocb_rx_init(&rx, side);
    for (size_t i = 0; i < in_n; i++) {
        if (ocb_rx_push(&rx, in[i], &fr) == OCB_RX_FRAME) {
            uint8_t pl[2 + OCB_MAX_BODY];
            pl[0] = fr.type;
            pl[1] = fr.seq;
            memcpy(pl + 2, fr.body, fr.len);
            if (got >= nwant) {
                vfail(c->line, c->name, "unexpected extra frame #%d", got + 1);
            } else if (!mem_eq(pl, 2u + fr.len, want[got], want_n[got])) {
                vfail_mem(c->line, c->name, "frame", pl, 2u + fr.len, want[got], want_n[got]);
            }
            got++;
        }
    }
    if (got != nwant) {
        vfail(c->line, c->name, "%d frames delivered, expected %d", got, nwant);
    }
    const uint32_t have[6] = {rx.stats.frames_ok,   rx.stats.empty,        rx.stats.cobs_errors,
                              rx.stats.crc_errors, rx.stats.range_errors, rx.stats.overlong};
    for (int i = 0; i < 6; i++) {
        if (have[i] != sv[i]) {
            vfail(sf->line, c->name, "stats %s = %u, expected %u", keys[i], (unsigned)have[i],
                  (unsigned)sv[i]);
        }
    }
}

static void area_state(const vcase *c)
{
    static const char *const allowed[] = {"bytes", "fields", NULL};
    static const struct {
        const char *name;
        size_t digits;
    } sfields[] = {{"buttons", 4}, {"hat", 2}, {"lx", 2}, {"ly", 2},
                   {"rx", 2},      {"ry", 2},  {"flags", 2}};
    uint8_t bytes[MAX_BYTES], out[OCB_STATE_SIZE];
    size_t n = 0;
    kv list[MAX_KV];
    int nkv = 0;
    uint32_t v[7];

    if (check_allowed(c, allowed) || need_hex(c, "bytes", bytes, sizeof bytes, &n) != 0) {
        return;
    }
    const vfield *ff = need(c, "fields");
    if (ff == NULL) {
        return;
    }
    if (n != OCB_STATE_SIZE) {
        vfail(c->line, c->name, "bytes must be 8 bytes");
        return;
    }
    if (parse_kv_list(ff->value, list, MAX_KV, &nkv) != 0 || nkv != 7) {
        vfail(ff->line, c->name, "fields needs buttons hat lx ly rx ry flags");
        return;
    }
    for (int i = 0; i < 7; i++) {
        if (strcmp(list[i].name, sfields[i].name) != 0 ||
            parse_num(list[i].value, sfields[i].digits, &v[i]) != 0) {
            vfail(ff->line, c->name, "field %d must be %s with %zu hex digits", i, sfields[i].name,
                  sfields[i].digits);
            return;
        }
    }
    ocb_state s;
    ocb_state_decode(bytes, &s);
    if (s.buttons != v[0] || s.hat != v[1] || s.lx != v[2] || s.ly != v[3] || s.rx != v[4] ||
        s.ry != v[5] || s.flags != v[6]) {
        vfail(c->line, c->name, "decode(bytes) differs from fields");
    }
    const ocb_state w = {(uint16_t)v[0], (uint8_t)v[1], (uint8_t)v[2], (uint8_t)v[3],
                         (uint8_t)v[4],  (uint8_t)v[5], (uint8_t)v[6]};
    ocb_state_encode(&w, out);
    if (!mem_eq(out, sizeof out, bytes, n)) {
        vfail_mem(c->line, c->name, "encode(fields)", out, sizeof out, bytes, n);
    }
    if (strcmp(c->name, "neutral") == 0) {
        uint8_t nb[OCB_STATE_SIZE];
        ocb_state_encode(&OCB_NEUTRAL, nb);
        if (!mem_eq(nb, sizeof nb, bytes, n) || !ocb_state_is_neutral(&s)) {
            vfail_mem(c->line, c->name, "OCB_NEUTRAL", nb, sizeof nb, bytes, n);
        }
    }
}

/* ---- constants: NAME: big-endian hex number, both directions */

typedef struct {
    const char *name;
    uint32_t value;
    int seen;
} cdesc;

#define C(x) {#x, (uint32_t)(OCB_##x), 0}
static cdesc g_consts[] = {
    C(PROTO_VERSION),
    C(MSG_HELLO), C(MSG_SET_STATE), C(MSG_QUEUE_EVENT), C(MSG_START), C(MSG_STOP), C(MSG_PING),
    C(MSG_HEARTBEAT), C(MSG_INFO), C(MSG_STATUS), C(MSG_ACK), C(MSG_NAK), C(MSG_EVENT),
    C(TYPE_APP_MIN), C(TYPE_APP_MAX), C(TYPE_BRIDGE_MIN), C(TYPE_BRIDGE_MAX), C(TYPE_CTRL_MIN),
    C(TYPE_CTRL_MAX),
    C(NAK_CRC), C(NAK_SEQ), C(NAK_FULL), C(NAK_VERSION), C(NAK_BUSY), C(NAK_ORDER),
    C(NAK_BAD_STATE), C(NAK_UNKNOWN_TYPE), C(NAK_NO_LINK), C(NAK_NOT_READY), C(NAK_ABORTED),
    C(NAK_TRANSPORT_MIN), C(NAK_TRANSPORT_MAX), C(NAK_SEMANTIC_MIN), C(NAK_SEMANTIC_MAX),
    C(EVT_NONE), C(EVT_PLAYBACK_DONE), C(EVT_LINK_LOST), C(EVT_LATE), C(EVT_UNDERRUN),
    C(EVT_USB_SUSPENDED),
    C(MODE_IDLE), C(MODE_DIRECT), C(MODE_PLAYING),
    C(HID_PROFILE_UNKNOWN), C(HID_PROFILE_P1), C(HID_PROFILE_P2), C(HID_PROFILE_P3),
    C(HB_FLAG_DTR), C(STATUS_LINK_OK), C(STATUS_CONSOLE_READY), C(STATUS_RESERVED_MASK),
    C(STATE_FLAG_END), C(STATE_RESERVED_MASK),
    C(BTN_Y), C(BTN_B), C(BTN_A), C(BTN_X), C(BTN_L), C(BTN_R), C(BTN_ZL), C(BTN_ZR),
    C(BTN_MINUS), C(BTN_PLUS), C(BTN_L3), C(BTN_R3), C(BTN_HOME), C(BTN_CAPTURE),
    C(HAT_UP), C(HAT_UP_RIGHT), C(HAT_RIGHT), C(HAT_DOWN_RIGHT), C(HAT_DOWN), C(HAT_DOWN_LEFT),
    C(HAT_LEFT), C(HAT_UP_LEFT), C(HAT_NEUTRAL), C(STICK_CENTER),
    C(STATE_SIZE), C(MAX_BODY), C(MIN_RAW_FRAME), C(MAX_RAW_FRAME), C(MAX_ENCODED_FRAME),
    C(MAX_WIRE_FRAME), C(FRAME_DELIMITER), C(FRAME_HEADER_SIZE), C(FRAME_CRC_SIZE),
    C(BODY_LEN_HELLO), C(BODY_LEN_INFO), C(BODY_LEN_SET_STATE), C(BODY_LEN_QUEUE_EVENT),
    C(BODY_LEN_START), C(BODY_LEN_STOP), C(BODY_LEN_PING), C(BODY_LEN_HEARTBEAT),
    C(BODY_LEN_STATUS), C(BODY_LEN_ACK), C(BODY_LEN_NAK), C(BODY_LEN_EVENT),
    C(CRC16_POLY), C(CRC16_INIT), C(CRC16_CHECK),
    C(LINK_BAUD), C(HEARTBEAT_PERIOD_MS), C(HEARTBEAT_MAX_GAP_MS), C(LINK_TIMEOUT_MS),
    C(LINK_TIMEOUT_MAX_MS), C(ACK_TIMEOUT_MS), C(ACK_WINDOW), C(HELLO_RESEND_MS),
    C(APP_TIMEOUT_MS_DEFAULT), C(APP_PING_DIVISOR), C(STATUS_PERIOD_MS), C(STATUS_MIN_GAP_MS),
    C(CONSOLE_POLL_STALL_MS), C(QUEUE_CAP_DEFAULT), C(T_US_LIMIT), C(STOP_NEUTRAL_MS),
};
#undef C
#define N_CONSTS (sizeof g_consts / sizeof g_consts[0])

static void flat_constant(const char *key, const char *value, int line)
{
    uint32_t v = 0;
    if (parse_num(value, 0, &v) != 0) {
        vfail(line, key, "value '%s' is not a big-endian hex number", value);
        return;
    }
    for (size_t i = 0; i < N_CONSTS; i++) {
        if (strcmp(g_consts[i].name, key) == 0) {
            if (g_consts[i].seen) {
                vfail(line, key, "duplicate constant");
            }
            g_consts[i].seen = 1;
            if (g_consts[i].value != v) {
                vfail(line, key, "vector value %08x, OCB_%s = %08x", (unsigned)v, key,
                      (unsigned)g_consts[i].value);
            }
            return;
        }
    }
    vfail(line, key, "no OCB_%s on the C side", key);
}

/* ------------------------------------------------------------------ file reader */

static int valid_name(const char *s, int allow_upper)
{
    if (*s == '\0') {
        return 0;
    }
    for (; *s != '\0'; s++) {
        const char ch = *s;
        if (!((ch >= 'a' && ch <= 'z') || (ch >= '0' && ch <= '9') || ch == '_' ||
              (allow_upper && ch >= 'A' && ch <= 'Z'))) {
            return 0;
        }
    }
    return 1;
}

static char g_seen_cases[MAX_CASES][64];

/* Reads one vectors file. fn != NULL: case file; fn == NULL: constants. */
static int read_file(const char *dir, const char *fname, case_fn fn)
{
    static char line[MAX_LINE + 2];
    static vcase cur;
    char path[1024];
    int lineno = 0, header = 0, ncases = 0, nseen = 0, have_cur = 0;

    g_file = fname;
    snprintf(path, sizeof path, "%s/%s", dir, fname);
    FILE *fp = fopen(path, "r");
    if (fp == NULL) {
        vfail(0, "", "cannot open %s", path);
        return 0;
    }
    while (fgets(line, sizeof line, fp) != NULL) {
        lineno++;
        size_t len = strlen(line);
        if (len > 0 && line[len - 1] == '\n') {
            line[--len] = '\0';
        } else if (!feof(fp)) {
            vfail(lineno, "", "line longer than %d characters", MAX_LINE);
            int ch;
            while ((ch = fgetc(fp)) != EOF && ch != '\n') {
            }
            continue;
        }
        int bad = 0;
        for (size_t i = 0; i < len; i++) {
            const unsigned char ch = (unsigned char)line[i];
            if (ch < 0x20 || ch > 0x7e) {
                bad = 1;
            }
        }
        if (bad) {
            vfail(lineno, "", "non-printable or non-ASCII character (tabs and CR are not allowed)");
            continue;
        }
        char *p = line;
        while (*p == ' ') {
            p++;
        }
        if (*p == '\0' || *p == '#') {
            continue;
        }
        char *colon = strchr(p, ':');
        if (colon == NULL) {
            vfail(lineno, "", "not a 'key: value' line");
            continue;
        }
        char *kend = colon;
        while (kend > p && kend[-1] == ' ') {
            kend--;
        }
        *kend = '\0';
        char *val = colon + 1;
        while (*val == ' ') {
            val++;
        }
        size_t vl = strlen(val);
        while (vl > 0 && val[vl - 1] == ' ') {
            val[--vl] = '\0';
        }
        if (strchr(val, '#') != NULL) {
            vfail(lineno, "", "'#' inside a value (no trailing comments)");
            continue;
        }

        if (!header) {
            uint8_t b[4];
            size_t bn = 0;
            if (strcmp(p, "proto_version") != 0) {
                vfail(lineno, "", "the first key must be proto_version");
                fclose(fp);
                return 0;
            }
            if (parse_hex(val, b, sizeof b, &bn) != 0 || bn != 1 || b[0] != OCB_PROTO_VERSION) {
                vfail(lineno, "", "proto_version '%s' differs from OCB_PROTO_VERSION %02x", val,
                      (unsigned)OCB_PROTO_VERSION);
            }
            header = 1;
            continue;
        }
        if (strcmp(p, "proto_version") == 0) {
            vfail(lineno, "", "duplicate proto_version");
            continue;
        }

        if (fn == NULL) {
            if (!valid_name(p, 1) || strchr(p, '.') != NULL) {
                vfail(lineno, "", "bad constant name '%s'", p);
                continue;
            }
            flat_constant(p, val, lineno);
            ncases++;
            continue;
        }

        char *dot = strrchr(p, '.');
        if (dot == NULL) {
            vfail(lineno, "", "key '%s' is not case.field", p);
            continue;
        }
        *dot = '\0';
        const char *cname = p, *field = dot + 1;
        if (!valid_name(cname, 0) || !valid_name(field, 0) || strlen(cname) >= sizeof cur.name ||
            strlen(field) >= sizeof cur.f[0].field) {
            vfail(lineno, "", "bad case or field name '%s.%s'", cname, field);
            continue;
        }
        if (!have_cur || strcmp(cur.name, cname) != 0) {
            if (have_cur) {
                fn(&cur);
                ncases++;
            }
            for (int i = 0; i < nseen; i++) {
                if (strcmp(g_seen_cases[i], cname) == 0) {
                    vfail(lineno, cname, "case name used twice (fields must be consecutive)");
                }
            }
            if (nseen < MAX_CASES) {
                strcpy(g_seen_cases[nseen++], cname);
            } else {
                vfail(lineno, cname, "too many cases (raise MAX_CASES)");
            }
            memset(&cur, 0, sizeof cur);
            strcpy(cur.name, cname);
            cur.line = lineno;
            have_cur = 1;
        }
        if (get(&cur, field) != NULL) {
            vfail(lineno, cname, "duplicate field '%s'", field);
            continue;
        }
        if (cur.n >= MAX_FIELDS) {
            vfail(lineno, cname, "too many fields");
            continue;
        }
        strcpy(cur.f[cur.n].field, field);
        strcpy(cur.f[cur.n].value, val);
        cur.f[cur.n].line = lineno;
        cur.n++;
    }
    if (have_cur) {
        fn(&cur);
        ncases++;
    }
    fclose(fp);
    if (!header) {
        vfail(lineno, "", "missing proto_version header");
    }
    return ncases;
}

/* ------------------------------------------------------------------ main */

static const struct {
    const char *name;
    case_fn fn;
    int min_cases;
} kFiles[] = {
    /* Adding an area means adding its file here (vectors/README.md). */
    {"constants.txt", NULL, 100}, {"cobs.txt", area_cobs, 14},       {"crc16.txt", area_crc16, 5},
    {"frames.txt", area_frames, 18}, {"messages.txt", area_messages, 18},
    {"streams.txt", area_streams, 10}, {"state.txt", area_state, 17},
};

int main(int argc, char **argv)
{
    const char *dir = argc > 1 ? argv[1] : OCB_VECTORS_DIR;
    const size_t nfiles = sizeof kFiles / sizeof kFiles[0];

#ifdef OCB_VECTORS_TXT_COUNT
    if (argc <= 1 && (size_t)OCB_VECTORS_TXT_COUNT != nfiles) {
        OCB_CHECK(0, "vectors/ holds %d .txt files at configure time, kFiles lists %zu",
                  OCB_VECTORS_TXT_COUNT, nfiles);
    }
#endif
    for (size_t i = 0; i < nfiles; i++) {
        const int n = read_file(dir, kFiles[i].name, kFiles[i].fn);
        g_file = kFiles[i].name;
        if (n < kFiles[i].min_cases) {
            vfail(0, "", "%d cases, at least %d required", n, kFiles[i].min_cases);
        }
        g_cases_total += n;
    }
    g_file = "constants.txt";
    for (size_t i = 0; i < N_CONSTS; i++) {
        if (!g_consts[i].seen) {
            vfail(0, g_consts[i].name, "OCB_%s missing from constants.txt", g_consts[i].name);
        }
    }
    g_file = "messages.txt";
    for (size_t i = 0; i < N_MSGS; i++) {
        if (g_msgs[i].ok_cases == 0) {
            vfail(0, g_msgs[i].name, "no ok case for message %s", g_msgs[i].name);
        }
    }

    if (ocb_test_failures() != 0) {
        fprintf(stderr, "test_vectors: %d failure(s)\n", ocb_test_failures());
        return 1;
    }
    printf("test_vectors: %d cases in %zu files ok (proto_version %02x)\n", g_cases_total, nfiles,
           (unsigned)OCB_PROTO_VERSION);
    return 0;
}
