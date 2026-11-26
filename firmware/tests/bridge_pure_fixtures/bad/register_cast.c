/* Fixture for check-bridge-pure.sh: must be flagged (a raw register address cast to a pointer; proves the gate applies the register rule of tools/pure-rules.sh). */
#include <stdint.h>
void ocb_fixture_poke(void);
void ocb_fixture_poke(void)
{
    *(volatile uint32_t *)0x4001c000 = 0;
}
