/* Fixture for check-bridge-pure.sh: allowed includes and names only, must pass. */
#include <limits.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#include "ocb_bridge/bridge.h" /* a trailing comment is fine */
#  include "ocb_proto/consts.h"

/* A name that merely contains a banned word is fine: */
int ocb_fixture_allowed(void);
int ocb_fixture_allowed(void)
{
    int ocb_framer_free_slots = 3;
    return ocb_framer_free_slots;
}
