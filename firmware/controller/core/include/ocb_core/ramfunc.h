/*
 * SPDX-License-Identifier: MIT
 *
 * OCB_CORE_RAMFUNC (story 2.4 D12): the hook that lets the firmware place
 * the hot path of the controller core in RAM (AD-11) without the core
 * including any SDK header. Empty by default; a build may define it, for
 * example with a section attribute, through a compile definition on the
 * ocb_core target. It precedes the return type of every hot function, in
 * both the declaration and the definition.
 */
#ifndef OCB_CORE_RAMFUNC_H
#define OCB_CORE_RAMFUNC_H

#ifndef OCB_CORE_RAMFUNC
#define OCB_CORE_RAMFUNC
#endif

#endif /* OCB_CORE_RAMFUNC_H */
