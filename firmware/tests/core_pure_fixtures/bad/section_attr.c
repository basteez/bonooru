/* Fixture for check-core-pure.sh: must be flagged (a section attribute outside ramfunc.h). */
__attribute__((section(".time_critical"))) void ocb_fixture_hot(void);
