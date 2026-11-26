/* Fixture for check-core-pure.sh: a section attribute is legal in ramfunc.h only, must pass. */
#ifndef OCB_FIXTURE_RAMFUNC_H
#define OCB_FIXTURE_RAMFUNC_H
#define OCB_CORE_RAMFUNC __attribute__((section(".time_critical.ocb")))
#endif
