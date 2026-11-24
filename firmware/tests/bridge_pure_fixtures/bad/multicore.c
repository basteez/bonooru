/* Fixture for check-bridge-pure.sh: must be flagged (no multicore API in the link). */
void ocb_fixture_launch(void (*entry)(void));
void ocb_fixture_launch(void (*entry)(void))
{
    multicore_launch_core1(entry);
}
