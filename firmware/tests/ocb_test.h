/*
 * SPDX-License-Identifier: MIT
 *
 * Minimal host-test helpers (story 2.2 D7): checks print file:line and a
 * message, count failures and continue. No framework.
 */
#ifndef OCB_TEST_H
#define OCB_TEST_H

#include <stddef.h>
#include <stdio.h>
#include <string.h>

static int ocb_test_fail_count;

static inline void ocb_test_hexdump(const char *label, const void *p, size_t n)
{
    const unsigned char *b = (const unsigned char *)p;
    fprintf(stderr, "    %s (%zu):", label, n);
    for (size_t i = 0; i < n; i++) {
        fprintf(stderr, " %02x", b[i]);
    }
    fprintf(stderr, "\n");
}

#define OCB_CHECK(cond, ...)                                                                       \
    do {                                                                                           \
        if (!(cond)) {                                                                             \
            ocb_test_fail_count++;                                                                 \
            fprintf(stderr, "%s:%d: FAIL: ", __FILE__, __LINE__);                                  \
            fprintf(stderr, __VA_ARGS__);                                                          \
            fprintf(stderr, "\n");                                                                 \
        }                                                                                          \
    } while (0)

#define OCB_CHECK_MEM(a, an, b, bn, what)                                                          \
    do {                                                                                           \
        if ((an) != (bn) || ((an) > 0 && memcmp((a), (b), (an)) != 0)) {                           \
            ocb_test_fail_count++;                                                                 \
            fprintf(stderr, "%s:%d: FAIL: %s differs\n", __FILE__, __LINE__, (what));              \
            ocb_test_hexdump("got     ", (a), (an));                                               \
            ocb_test_hexdump("expected", (b), (bn));                                               \
        }                                                                                          \
    } while (0)

static inline int ocb_test_failures(void)
{
    return ocb_test_fail_count;
}

#endif /* OCB_TEST_H */
