/*
 * SPDX-License-Identifier: MIT
 *
 * Core 1 stack high-water mark (story 2.5 D15). Rules in stack_mark.h.
 */
#include "stack_mark.h"

#include <stddef.h>
#include <stdint.h>

#include "pico/stdlib.h"

/* Linker symbols (pico_standard_link section_end.incl), declared as arrays
 * of unknown size: the region is indexed, never a single object. */
extern uint32_t __StackOneBottom[];
extern uint32_t __StackOneTop[];

static uint32_t s_low;  /* lowest modified word index seen (s_words: none) */
static uint32_t s_scan; /* next word index to look at */

static uint32_t words(void)
{
    return (uint32_t)(((uintptr_t)__StackOneTop - (uintptr_t)__StackOneBottom) / sizeof(uint32_t));
}

void stack_mark_paint(void)
{
    volatile uint32_t *p = __StackOneBottom;
    const uint32_t n = words();
    for (uint32_t i = 0u; i < n; i++) {
        p[i] = STACK_MARK_PATTERN;
    }
    s_low = n;
    s_scan = 0u;
}

void __not_in_flash_func(stack_mark_scan_step)(void)
{
    const volatile uint32_t *p = __StackOneBottom;
    for (uint32_t k = 0u; k < STACK_MARK_WORDS_PER_PASS; k++) {
        if (s_scan >= s_low) {
            s_scan = 0u; /* nothing to learn above the mark: start again */
            return;
        }
        if (p[s_scan] != STACK_MARK_PATTERN) {
            s_low = s_scan;
            s_scan = 0u;
            return;
        }
        s_scan++;
    }
}

uint32_t stack_mark_used_bytes(void)
{
    return (words() - s_low) * 4u;
}

uint32_t stack_mark_size_bytes(void)
{
    return words() * 4u;
}
