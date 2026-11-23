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
 * Story 2.5 D2 decided the placement: the controller is linked with the
 * SDK's copy_to_ram binary type, so the whole program (every library, the
 * SDK, TinyUSB, the ocb_proto decoders, OCB_NEUTRAL and the compiler's
 * switch tables) runs from SRAM and this macro stays empty;
 * tools/check-ram-hot-path.sh proves the hot functions are in RAM. No
 * build defines the macro: the XIP comparison variant
 * (OCB_CONTROLLER_RUN_FROM_RAM=OFF, bench H.20) compares the whole image in
 * RAM with the whole image in flash, where only the functions the
 * platform glue marks with the SDK's RAM attribute stay in RAM. The macro
 * stays as the hook a later build could define.
 */
#ifndef OCB_CORE_RAMFUNC_H
#define OCB_CORE_RAMFUNC_H

#ifndef OCB_CORE_RAMFUNC
#define OCB_CORE_RAMFUNC
#endif

#endif /* OCB_CORE_RAMFUNC_H */
