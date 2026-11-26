/* Fixture for check-proto-pure.sh: allowed includes only, must pass. */
#include <limits.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#include "ocb_proto/consts.h" /* a trailing comment is fine */
#  include "ocb_proto/frame.h"

int ocb_fixture_allowed(void);

/* Look-alikes of the process-exit, section and register-address rules are
 * fine: the abort latch, an exit path, a section attribute in words, and
 * hex literals outside the peripheral windows. */
#include <stdbool.h>
#define OCB_FIXTURE_LIMIT 0x80000000u
#define OCB_FIXTURE_ALL_ONES 0xFFFFFFFFu
#define OCB_FIXTURE_CRC32_POLY 0xEDB88320u
bool ocb_fixture_aborted;
int ocb_exit_code;
void *ocb_fixture_null = (void *)0;
