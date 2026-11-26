/* Fixture for check-core-pure.sh: must be flagged (process exit, the POSIX form). */
void ocb_fixture_stop(void);
void ocb_fixture_stop(void)
{
    _exit(1);
}
