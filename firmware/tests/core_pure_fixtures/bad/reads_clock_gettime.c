/* Fixture for check-core-pure.sh: must be flagged (the core reads no clock). */
void ocb_fixture_now(void *ts);
void ocb_fixture_now(void *ts)
{
    clock_gettime(0, ts);
}
