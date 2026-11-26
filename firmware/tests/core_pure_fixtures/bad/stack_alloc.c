/* Fixture for check-core-pure.sh: must be flagged (alloca: an unbounded stack allocation). */
void *ocb_fixture_scratch(void);
void *ocb_fixture_scratch(void)
{
    return alloca(4);
}
