/*
 * SPDX-License-Identifier: MIT
 *
 * Host test F.0: the generated ocb_version.h is consistent (AD-14): the
 * string is non-empty and starts with OCB_VERSION_MAJOR.MINOR.PATCH
 * ("0.0.0-dev" included).
 */
#include <stdio.h>
#include <string.h>

#include "ocb_version.h"

int main(void)
{
    const char *s = OCB_VERSION_STRING;
    char expect[48];

    if (s[0] == '\0') {
        fprintf(stderr, "FAIL: OCB_VERSION_STRING is empty\n");
        return 1;
    }
    snprintf(expect, sizeof expect, "%d.%d.%d", OCB_VERSION_MAJOR, OCB_VERSION_MINOR,
             OCB_VERSION_PATCH);
    const size_t n = strlen(expect);
    if (strncmp(s, expect, n) != 0 || (s[n] != '\0' && s[n] != '-')) {
        fprintf(stderr, "FAIL: OCB_VERSION_STRING '%s' does not start with '%s'\n", s, expect);
        return 1;
    }
    printf("test_version: ok (%s)\n", s);
    return 0;
}
