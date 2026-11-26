/* Fixture for check-core-pure.sh: must be flagged (process exit: the core never ends the program). */
void ocb_fixture_stop(void);
void ocb_fixture_stop(void)
{
    exit(1);
}
