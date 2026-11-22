/* Fixture for check-proto-pure.sh: allowed includes only, must pass. */
#include <limits.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#include "ocb_proto/consts.h" /* a trailing comment is fine */
#  include "ocb_proto/frame.h"

int ocb_fixture_allowed(void);
