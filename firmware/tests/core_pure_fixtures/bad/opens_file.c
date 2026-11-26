/* Fixture for check-core-pure.sh: must be flagged (file I/O). */
void ocb_fixture_open(void);
void ocb_fixture_open(void)
{
    (void)fopen("x", "r");
}
