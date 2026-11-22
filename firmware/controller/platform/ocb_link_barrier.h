/*
 * SPDX-License-Identifier: MIT
 *
 * OCB_LINK_BARRIER() for the RP2040 targets (story 2.5 D5): a full data
 * memory barrier, `dmb` with the SY option (0xF), the same instruction as
 * the SDK's __dmb() (hardware_sync/sync.h). Force-included into the
 * ocb_link sources by controller/CMakeLists.txt in the firmware builds
 * only, so the pure library includes no SDK header and holds no asm token
 * (issue #20); the host build keeps the default of ocb_link/config.h.
 *
 * Why not __atomic_thread_fence(__ATOMIC_SEQ_CST): Arm GNU Toolchain
 * 15.2.Rel1 emits `dmb ish` for it on Cortex-M0+ (checked with objdump for
 * the story); on ARMv6-M only the SY option is defined, the others are
 * reserved and software must not rely on them, so the barrier is spelled
 * out (D24). tools/check-barrier.sh proves `dmb sy` is in the published-
 * state handoff of every controller ELF.
 */
#ifndef OCB_CTRL_OCB_LINK_BARRIER_H
#define OCB_CTRL_OCB_LINK_BARRIER_H

#define OCB_LINK_BARRIER() __asm__ volatile("dmb 0xF" ::: "memory")

#endif /* OCB_CTRL_OCB_LINK_BARRIER_H */
