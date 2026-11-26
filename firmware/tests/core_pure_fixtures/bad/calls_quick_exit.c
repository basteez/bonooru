/* Fixture for check-core-pure.sh: must be flagged (process exit, quick_exit). */
void ocb_fixture_stop(void);
void ocb_fixture_stop(void)
{
    quick_exit(1);
}
