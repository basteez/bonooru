/* Fixture for check-core-pure.sh: must be flagged (inline assembly, the __asm spelling). */
void ocb_fixture_nop(void);
void ocb_fixture_nop(void)
{
    __asm("nop");
}
