/* Fixture for check-core-pure.sh: must be flagged (inline assembly). */
void ocb_fixture_nop(void);
void ocb_fixture_nop(void)
{
    asm volatile("nop");
}
