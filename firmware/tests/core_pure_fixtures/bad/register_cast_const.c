/* Fixture for check-core-pure.sh: must be flagged (a raw address after a qualified pointer cast). */
#include <stdint.h>
void ocb_fixture_poke(void);
void ocb_fixture_poke(void)
{
    *(volatile uint32_t * const)0x20041000 = 0;
}
