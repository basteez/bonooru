/* Fixture for check-core-pure.sh: must be flagged (an SDK stdio call). */
void ocb_fixture_init(void);
void ocb_fixture_init(void)
{
    stdio_init_all();
}
