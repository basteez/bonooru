/* Fixture for check-link-pure.sh: must be flagged (the link allocates nothing). */
#include <stddef.h>

void *ocb_fixture_alloc(size_t n);
void *ocb_fixture_alloc(size_t n)
{
    return malloc(n);
}
