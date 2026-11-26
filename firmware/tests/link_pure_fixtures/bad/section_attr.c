/* Fixture for check-link-pure.sh: must be flagged (a section attribute outside ramfunc.h; use OCB_CORE_RAMFUNC). */
__attribute__((section(".time_critical"))) void ocb_fixture_hot(void);
