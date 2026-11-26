/* Fixture for check-proto-pure.sh: must be flagged (a placement attribute outside ramfunc.h; proves the gate applies the section rule of tools/pure-rules.sh). */
__attribute__((section(".time_critical"))) void ocb_fixture_hot(void);
