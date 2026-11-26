/* Fixture for check-core-pure.sh: must be flagged (the compiler builtin for a stack allocation). */
void *ocb_fixture_scratch(void);
void *ocb_fixture_scratch(void)
{
    return __builtin_alloca(4);
}
