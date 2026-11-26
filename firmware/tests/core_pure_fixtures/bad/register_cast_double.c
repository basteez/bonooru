/* Fixture for check-core-pure.sh: must be flagged (a raw address through an integer cast, then a pointer cast). */
#include <stdint.h>
void ocb_fixture_poke(void);
void ocb_fixture_poke(void)
{
    *(volatile uint32_t *)(uintptr_t)0x20041000 = 0;
}
