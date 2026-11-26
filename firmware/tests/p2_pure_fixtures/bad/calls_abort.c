/* Fixture for check-p2-pure.sh: must be flagged (process exit; proves the gate uses tools/pure-rules.sh). */
void ocb_fixture_stop(void);
void ocb_fixture_stop(void)
{
    abort();
}
