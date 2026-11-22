/*
 * SPDX-License-Identifier: MIT
 *
 * Core 1 stack high-water mark (story 2.5 D15, Task 12.1; issue #21),
 * debug builds. The SDK's default core 1 stack is PICO_CORE1_STACK_SIZE
 * bytes in .stack1 at the top of SCRATCH_X, from __StackOneBottom to
 * __StackOneTop; multicore_launch_core1 pushes three words at its top
 * (pico_multicore/multicore.c, read for the story).
 *
 *  - core 0, before multicore_launch_core1: stack_mark_paint() fills the
 *    whole region with a pattern (the launch then writes its three words
 *    over the top of it);
 *  - core 1, once per pass: stack_mark_scan_step() looks at no more than
 *    STACK_MARK_WORDS_PER_PASS words, from the bottom up to the lowest word
 *    already seen modified, and keeps that lowest address (never a whole
 *    2 KB scan in one pass);
 *  - stack_mark_used_bytes(): bytes from the lowest modified word to the
 *    top, the deepest use seen since boot (0 until a modified word is
 *    seen). The UART RX interrupt runs on core 1's stack too, so its frame
 *    is included whenever it fired at the deepest point.
 */
#ifndef OCB_CTRL_STACK_MARK_H
#define OCB_CTRL_STACK_MARK_H

#include <stdint.h>

#define STACK_MARK_PATTERN 0xA5A5A5A5u
#define STACK_MARK_WORDS_PER_PASS 8u

void stack_mark_paint(void);
void stack_mark_scan_step(void);
uint32_t stack_mark_used_bytes(void);
/* The size of the painted region, bytes (PICO_CORE1_STACK_SIZE). */
uint32_t stack_mark_size_bytes(void);

#endif /* OCB_CTRL_STACK_MARK_H */
