/* Fixture for check-core-pure.sh: must be flagged (the core reads no clock). */
#include <stdint.h>

uint64_t ocb_fixture_now(void);
uint64_t ocb_fixture_now(void)
{
    return time_us_64();
}
