/* Fixture for check-core-pure.sh: must be flagged (inline assembly, the GNU spelling). */
void ocb_fixture_irq_off(void);
void ocb_fixture_irq_off(void)
{
    __asm__ __volatile__("cpsid i");
}
