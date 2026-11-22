/* Fixture for check-p2-pure.sh: must be flagged (the profile allocates nothing). */
#include <stddef.h>

void *ocb_fixture_alloc(size_t n);
void *ocb_fixture_alloc(size_t n)
{
    return malloc(n);
}
