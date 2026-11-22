/* Fixture for check-core-pure.sh: allowed includes and names only, must pass. */
#include <limits.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#include "ocb_core/core.h" /* a trailing comment is fine */
#  include "ocb_proto/consts.h"

/* A name that merely contains a banned word is fine: */
int ocb_fixture_allowed(void);
int ocb_fixture_allowed(void)
{
    int ocb_queue_free_slots = 3;
    return ocb_queue_free_slots;
}
