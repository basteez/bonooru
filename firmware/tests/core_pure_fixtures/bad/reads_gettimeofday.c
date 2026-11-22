/* Fixture for check-core-pure.sh: must be flagged (the core reads no clock). */
void ocb_fixture_now(void *tv);
void ocb_fixture_now(void *tv)
{
    gettimeofday(tv, 0);
}
