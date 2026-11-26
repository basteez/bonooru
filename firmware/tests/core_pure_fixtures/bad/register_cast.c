/* Fixture for check-core-pure.sh: must be flagged (a raw register address cast to a pointer). */
#include <stdint.h>
void ocb_fixture_poke(void);
void ocb_fixture_poke(void)
{
    *(volatile uint32_t *)0x4001c000 = 0;
}
