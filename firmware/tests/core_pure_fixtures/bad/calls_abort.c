/* Fixture for check-core-pure.sh: must be flagged (process exit: abort, even with a space before the parenthesis). */
void ocb_fixture_stop(void);
void ocb_fixture_stop(void)
{
    abort ();
}
