/* Fixture for check-core-pure.sh: must be flagged (an aligned variant of the compiler builtin for a stack allocation). */
void *ocb_fixture_scratch(void);
void *ocb_fixture_scratch(void)
{
    return __builtin_alloca_with_align(4, 8);
}
