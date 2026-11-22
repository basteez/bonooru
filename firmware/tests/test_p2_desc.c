/*
 * SPDX-License-Identifier: MIT
 *
 * F.24 (story 2.3 Task 2.3): the P2 USB descriptors. Expected bytes are
 * hand-written from the hardware dumps (S8: tmk's lsusb -v and its hex
 * lines; S7: mzyy94's usbhid-dump and string hex), never taken from the
 * library. An independent walker in this file parses them.
 */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#include "ocb_p2/desc.h"
#include "ocb_test.h"

/* S8 "Devicer dump": 12 01 00 02 00 00 00 40 7E 05 09 20 10 02 01 02 03 01 */
static const uint8_t k_device[18] = {0x12, 0x01, 0x00, 0x02, 0x00, 0x00, 0x00, 0x40, 0x7E,
                                     0x05, 0x09, 0x20, 0x10, 0x02, 0x01, 0x02, 0x03, 0x01};

/* S8 "Config0 dump" (bInterval 8 at offsets 33 and 40). */
static const uint8_t k_config_8[41] = {
    0x09, 0x02, 0x29, 0x00, 0x01, 0x01, 0x00, 0xA0, 0xFA, 0x09, 0x04, 0x00, 0x00, 0x02,
    0x03, 0x00, 0x00, 0x00, 0x09, 0x21, 0x11, 0x01, 0x00, 0x01, 0x22, 0xCB, 0x00, 0x07,
    0x05, 0x81, 0x03, 0x40, 0x00, 0x08, 0x07, 0x05, 0x01, 0x03, 0x40, 0x00, 0x08};

/* S7 usbhid-dump of the HID report descriptor, 203 bytes, verbatim. */
static const uint8_t k_report[203] = {
    0x05, 0x01, 0x15, 0x00, 0x09, 0x04, 0xA1, 0x01, 0x85, 0x30, 0x05, 0x01, 0x05, 0x09, 0x19, 0x01,
    0x29, 0x0A, 0x15, 0x00, 0x25, 0x01, 0x75, 0x01, 0x95, 0x0A, 0x55, 0x00, 0x65, 0x00, 0x81, 0x02,
    0x05, 0x09, 0x19, 0x0B, 0x29, 0x0E, 0x15, 0x00, 0x25, 0x01, 0x75, 0x01, 0x95, 0x04, 0x81, 0x02,
    0x75, 0x01, 0x95, 0x02, 0x81, 0x03, 0x0B, 0x01, 0x00, 0x01, 0x00, 0xA1, 0x00, 0x0B, 0x30, 0x00,
    0x01, 0x00, 0x0B, 0x31, 0x00, 0x01, 0x00, 0x0B, 0x32, 0x00, 0x01, 0x00, 0x0B, 0x35, 0x00, 0x01,
    0x00, 0x15, 0x00, 0x27, 0xFF, 0xFF, 0x00, 0x00, 0x75, 0x10, 0x95, 0x04, 0x81, 0x02, 0xC0, 0x0B,
    0x39, 0x00, 0x01, 0x00, 0x15, 0x00, 0x25, 0x07, 0x35, 0x00, 0x46, 0x3B, 0x01, 0x65, 0x14, 0x75,
    0x04, 0x95, 0x01, 0x81, 0x02, 0x05, 0x09, 0x19, 0x0F, 0x29, 0x12, 0x15, 0x00, 0x25, 0x01, 0x75,
    0x01, 0x95, 0x04, 0x81, 0x02, 0x75, 0x08, 0x95, 0x34, 0x81, 0x03, 0x06, 0x00, 0xFF, 0x85, 0x21,
    0x09, 0x01, 0x75, 0x08, 0x95, 0x3F, 0x81, 0x03, 0x85, 0x81, 0x09, 0x02, 0x75, 0x08, 0x95, 0x3F,
    0x81, 0x03, 0x85, 0x01, 0x09, 0x03, 0x75, 0x08, 0x95, 0x3F, 0x91, 0x83, 0x85, 0x10, 0x09, 0x04,
    0x75, 0x08, 0x95, 0x3F, 0x91, 0x83, 0x85, 0x80, 0x09, 0x05, 0x75, 0x08, 0x95, 0x3F, 0x91, 0x83,
    0x85, 0x82, 0x09, 0x06, 0x75, 0x08, 0x95, 0x3F, 0x91, 0x83, 0xC0};

/* S7 string hex lines. */
static const uint8_t k_str0[4] = {0x04, 0x03, 0x09, 0x04};
static const uint8_t k_str1[38] = {0x26, 0x03, 0x4E, 0x00, 0x69, 0x00, 0x6E, 0x00, 0x74, 0x00, 0x65, 0x00, 0x6E,
                                   0x00, 0x64, 0x00, 0x6F, 0x00, 0x20, 0x00, 0x43, 0x00, 0x6F, 0x00, 0x2E, 0x00,
                                   0x2C, 0x00, 0x20, 0x00, 0x4C, 0x00, 0x74, 0x00, 0x64, 0x00, 0x2E, 0x00};
static const uint8_t k_str2[30] = {0x1E, 0x03, 0x50, 0x00, 0x72, 0x00, 0x6F, 0x00, 0x20, 0x00,
                                   0x43, 0x00, 0x6F, 0x00, 0x6E, 0x00, 0x74, 0x00, 0x72, 0x00,
                                   0x6F, 0x00, 0x6C, 0x00, 0x6C, 0x00, 0x65, 0x00, 0x72, 0x00};
static const uint8_t k_str3[26] = {0x1A, 0x03, 0x30, 0x00, 0x30, 0x00, 0x30, 0x00, 0x30, 0x00, 0x30, 0x00, 0x30,
                                   0x00, 0x30, 0x00, 0x30, 0x00, 0x30, 0x00, 0x30, 0x00, 0x30, 0x00, 0x31, 0x00};

static uint16_t le16(const uint8_t *p)
{
    return (uint16_t)(p[0] | (p[1] << 8));
}

/* Independent configuration walker. */
static void walk_config(const uint8_t *d, size_t n, uint8_t binterval)
{
    OCB_CHECK(n == 41u, "config length %zu", n);
    OCB_CHECK(d[1] == 0x02 && le16(&d[2]) == n, "wTotalLength %u vs %zu", le16(&d[2]), n);
    int interfaces = 0;
    int endpoints = 0;
    int hid = 0;
    bool seen_in = false;
    bool seen_out = false;
    size_t i = 0;
    while (i < n) {
        const uint8_t len = d[i];
        OCB_CHECK(len >= 2u && i + len <= n, "descriptor at %zu overruns (len %u)", i, len);
        if (len < 2u || i + len > n) {
            return;
        }
        const uint8_t type = d[i + 1u];
        if (type == 0x04) {
            interfaces++;
            OCB_CHECK(d[i + 4u] == 2u, "bNumEndpoints %u", d[i + 4u]);
            OCB_CHECK(d[i + 5u] == 0x03, "class %u", d[i + 5u]);
            OCB_CHECK(d[i + 6u] == 0x00 && d[i + 7u] == 0x00, "no boot subclass/protocol");
        } else if (type == 0x21) {
            hid++;
            OCB_CHECK(le16(&d[i + 2u]) == 0x0111, "bcdHID");
            OCB_CHECK(d[i + 6u] == 0x22, "report descriptor type");
            OCB_CHECK(le16(&d[i + 7u]) == sizeof k_report, "wDescriptorLength %u", le16(&d[i + 7u]));
        } else if (type == 0x05) {
            endpoints++;
            OCB_CHECK(d[i + 3u] == 0x03, "interrupt endpoint");
            OCB_CHECK(le16(&d[i + 4u]) == 64u, "wMaxPacketSize %u", le16(&d[i + 4u]));
            OCB_CHECK(d[i + 6u] == binterval, "bInterval %u expected %u", d[i + 6u], binterval);
            if (d[i + 2u] == 0x81) {
                seen_in = true;
            } else if (d[i + 2u] == 0x01) {
                seen_out = true;
            }
        }
        i += len;
    }
    OCB_CHECK(interfaces == 1 && hid == 1 && endpoints == 2 && seen_in && seen_out,
              "interfaces %d hid %d endpoints %d in %d out %d", interfaces, hid, endpoints, seen_in, seen_out);
}

/* Independent HID item walker: balanced collections, item sizes inside the
 * buffer, and the report ids with their payload bits per direction. */
typedef struct {
    uint8_t id;
    int dir; /* 0 input, 1 output */
    uint32_t bits;
} rep;

static void walk_report(const uint8_t *d, size_t n)
{
    rep reps[16];
    int nrep = 0;
    uint32_t size = 0;
    uint32_t count = 0;
    uint8_t id = 0;
    int depth = 0;
    size_t i = 0;
    while (i < n) {
        const uint8_t b = d[i];
        OCB_CHECK(b != 0xFE, "long item at %zu", i);
        const size_t sz = (b & 3u) == 3u ? 4u : (size_t)(b & 3u);
        OCB_CHECK(i + 1u + sz <= n, "item at %zu overruns", i);
        if (i + 1u + sz > n) {
            return;
        }
        uint32_t v = 0;
        for (size_t k = 0; k < sz; k++) {
            v |= (uint32_t)d[i + 1u + k] << (8u * k);
        }
        const uint8_t type = (uint8_t)((b >> 2) & 3u);
        const uint8_t tag = (uint8_t)(b >> 4);
        if (type == 0) { /* main */
            if (tag == 0x8 || tag == 0x9) {
                const int dir = tag == 0x8 ? 0 : 1;
                int k;
                for (k = 0; k < nrep; k++) {
                    if (reps[k].id == id && reps[k].dir == dir) {
                        break;
                    }
                }
                if (k == nrep && nrep < 16) {
                    reps[nrep].id = id;
                    reps[nrep].dir = dir;
                    reps[nrep].bits = 0;
                    nrep++;
                }
                if (k < 16) {
                    reps[k].bits += size * count;
                }
            } else if (tag == 0xA) {
                depth++;
            } else if (tag == 0xC) {
                depth--;
                OCB_CHECK(depth >= 0, "unbalanced End Collection at %zu", i);
            }
        } else if (type == 1) { /* global */
            if (tag == 0x7) {
                size = v;
            } else if (tag == 0x9) {
                count = v;
            } else if (tag == 0x8) {
                id = (uint8_t)v;
            }
        }
        i += 1u + sz;
    }
    OCB_CHECK(depth == 0, "collections not balanced (%d)", depth);
    /* Expected ids and payload sizes (S7, S8): 63 bytes after the id each. */
    static const struct {
        uint8_t id;
        int dir;
    } want[] = {{0x30, 0}, {0x21, 0}, {0x81, 0}, {0x01, 1}, {0x10, 1}, {0x80, 1}, {0x82, 1}};
    OCB_CHECK(nrep == (int)(sizeof want / sizeof want[0]), "%d reports declared", nrep);
    for (size_t w = 0; w < sizeof want / sizeof want[0]; w++) {
        bool found = false;
        for (int k = 0; k < nrep; k++) {
            if (reps[k].id == want[w].id && reps[k].dir == want[w].dir) {
                found = true;
                OCB_CHECK(reps[k].bits == 63u * 8u, "report %02x dir %d: %u bits", want[w].id, want[w].dir,
                          reps[k].bits);
            }
        }
        OCB_CHECK(found, "report %02x dir %d missing", want[w].id, want[w].dir);
    }
}

int main(void)
{
    OCB_CHECK_MEM(ocb_p2_device_desc, sizeof ocb_p2_device_desc, k_device, sizeof k_device, "device descriptor");
    OCB_CHECK(le16(&ocb_p2_device_desc[2]) == 0x0200 && ocb_p2_device_desc[7] == 64, "bcdUSB / EP0");

    OCB_CHECK_MEM(ocb_p2_report_desc, sizeof ocb_p2_report_desc, k_report, sizeof k_report, "report descriptor");
    walk_report(ocb_p2_report_desc, sizeof ocb_p2_report_desc);

    /* Every bInterval 1..16: S8's bytes with both endpoint intervals patched. */
    for (unsigned bi = 0; bi <= 17u; bi++) {
        uint8_t got[64];
        memset(got, 0xA5, sizeof got);
        const size_t n = ocb_p2_config_desc((uint8_t)bi, got, sizeof got);
        if (bi < 1u || bi > 16u) {
            OCB_CHECK(n == 0u && got[0] == 0xA5, "bInterval %u must be refused", bi);
            continue;
        }
        uint8_t want[41];
        memcpy(want, k_config_8, sizeof want);
        want[33] = (uint8_t)bi;
        want[40] = (uint8_t)bi;
        OCB_CHECK_MEM(got, n, want, sizeof want, "configuration descriptor");
        OCB_CHECK(got[41] == 0xA5, "wrote past 41 bytes");
        walk_config(got, n, (uint8_t)bi);
    }
    {
        uint8_t small[40];
        memset(small, 0xA5, sizeof small);
        OCB_CHECK(ocb_p2_config_desc(1, small, sizeof small) == 0u && small[0] == 0xA5, "short buffer");
        OCB_CHECK(ocb_p2_config_desc(1, NULL, 64) == 0u, "NULL buffer");
    }

    /* Strings: S7's bytes; round trip back to ASCII. */
    const struct {
        const uint8_t *b;
        size_t n;
        const char *ascii;
    } strs[4] = {{k_str0, sizeof k_str0, NULL},
                 {k_str1, sizeof k_str1, "Nintendo Co., Ltd."},
                 {k_str2, sizeof k_str2, "Pro Controller"},
                 {k_str3, sizeof k_str3, "000000000001"}};
    for (uint8_t idx = 0; idx < 4u; idx++) {
        uint8_t got[64];
        const size_t n = ocb_p2_string_desc(idx, got, sizeof got);
        OCB_CHECK_MEM(got, n, strs[idx].b, strs[idx].n, "string descriptor");
        if (strs[idx].ascii != NULL && n >= 2u) {
            char back[32] = {0};
            for (size_t k = 0; k + 2u < n && k / 2u < sizeof back - 1u; k += 2u) {
                back[k / 2u] = (char)got[2u + k];
            }
            OCB_CHECK(strcmp(back, strs[idx].ascii) == 0, "round trip '%s'", back);
        }
        uint8_t tiny[3];
        OCB_CHECK(ocb_p2_string_desc(idx, tiny, sizeof tiny) == 0u, "short buffer for string %u", idx);
    }
    {
        uint8_t got[64];
        OCB_CHECK(ocb_p2_string_desc(4, got, sizeof got) == 0u, "unknown string index");
        OCB_CHECK(ocb_p2_string_desc(1, NULL, 64) == 0u, "NULL string buffer");
    }
    return ocb_test_failures() == 0 ? 0 : 1;
}
