/* Fixture for check-core-pure.sh: must be flagged (process exit, the C99 _Exit). */
void ocb_fixture_stop(void);
void ocb_fixture_stop(void)
{
    _Exit(1);
}
