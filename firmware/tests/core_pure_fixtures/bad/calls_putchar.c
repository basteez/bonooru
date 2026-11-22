/* Fixture for check-core-pure.sh: must be flagged (the core does no I/O). */
void ocb_fixture_say(void);
void ocb_fixture_say(void)
{
    putchar(65);
}
