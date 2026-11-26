/* Fixture for check-core-pure.sh: must be flagged (a raw address in parentheses after a pointer cast). */
#include <stdint.h>
#define OCB_FIXTURE_REG (*(volatile uint32_t *)(0x20041000))
