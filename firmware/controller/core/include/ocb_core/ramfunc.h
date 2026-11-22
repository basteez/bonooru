/*
 * SPDX-License-Identifier: MIT
 *
 * OCB_CORE_RAMFUNC (story 2.4 D12): the hook that lets the firmware place
 * the hot path of the controller core in RAM (AD-11) without the core
 * including any SDK header. Empty by default; a build may define it, for
 * example with a section attribute, through a compile definition on the
 * ocb_core target. It precedes the return type of every hot function, in
 * both the declaration and the definition, and follows `static` on the
 * private helpers of the hot path.
 *
 * Not covered by this macro (Story 2.5 must review their placement when it
 * runs the core from RAM): the ocb_proto functions the core calls (message
 * decoders, state predicates), the OCB_NEUTRAL constant, and any constant
 * table the compiler emits for a switch (read-only data, in flash).
 */
#ifndef OCB_CORE_RAMFUNC_H
#define OCB_CORE_RAMFUNC_H

#ifndef OCB_CORE_RAMFUNC
#define OCB_CORE_RAMFUNC
#endif

#endif /* OCB_CORE_RAMFUNC_H */
