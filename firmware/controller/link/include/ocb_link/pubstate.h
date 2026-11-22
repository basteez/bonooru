/*
 * SPDX-License-Identifier: MIT
 *
 * Published-state handoff from core 1 to core 0 (AD-11; story 2.3 D35,
 * story 2.5 D4, D22): a sequence-locked double buffer with one writer
 * (core 1, once per super-loop pass) and one reader (core 0, the USB
 * report pump), in every build.
 *
 * Layout: two slots of the 8-byte state and a generation counter `gen`
 * that counts half publishes: odd while the writer fills a slot, even when
 * the slot is published. After c completed publishes (gen = 2c, or 2c + 1
 * while publish c + 1 is being written) the current state is slot c & 1;
 * publish c + 1 writes the other slot, so a reader is only disturbed when
 * the writer starts a SECOND publish during its copy (publish c + 2
 * rewrites slot c & 1, and it starts when gen becomes 2c + 3).
 *
 * Writer: gen = 2c + 1, barrier, fill slot (c + 1) & 1, barrier,
 * gen = 2c + 2.
 * Reader, one try: g0 = gen, barrier, copy slot (g0 >> 1) & 1, barrier,
 * g1 = gen; the copy is accepted when g1 - (g0 & ~1) <= 2 (at most one
 * complete publish and no start of the next one during the copy). At most
 * OCB_PUBSTATE_TRIES tries, then NEUTRAL (AD-11: "retry <= 3, then
 * NEUTRAL"), which replaces Story 2.3 D35's "keep the last good copy".
 *
 * Why the generation counts half publishes (D22): with a counter advanced
 * once per publish after the slot is written, a reader that copies slot
 * g0 & 1 while the writer rewrites it for publish g0 + 2 reads gen = g0 + 1
 * after its copy and would accept a torn state (F.33 has the interleaving).
 *
 * Every shared field is volatile and OCB_LINK_BARRIER() (a real DMB on the
 * ARM targets, story 2.5 D5; a no-op on the host) orders the generation
 * against the slot. The reader's counters (retries, fallbacks) belong to
 * core 0 and saturate.
 */
#ifndef OCB_LINK_PUBSTATE_H
#define OCB_LINK_PUBSTATE_H

#include <stdbool.h>
#include <stdint.h>

#include "ocb_core/ramfunc.h"
#include "ocb_proto/state.h"

#define OCB_PUBSTATE_TRIES 3u

typedef struct {
    volatile uint32_t gen;        /* half publishes: odd while a slot is written */
    volatile uint16_t buttons[2]; /* slot 0, slot 1 */
    volatile uint8_t b[2][6];     /* hat, lx, ly, rx, ry, flags of each slot */
    /* Writer only (core 1). */
    uint32_t written_count; /* the publish_count last written */
    bool written;           /* written_count is valid */
    /* Reader only (core 0), saturating. */
    uint32_t retries;   /* reads that needed more than one try */
    uint32_t fallbacks; /* reads that returned NEUTRAL after OCB_PUBSTATE_TRIES tries */
} ocb_pubstate;

/* NEUTRAL in both slots, gen 0, counters 0. Before either core uses it. */
void ocb_pubstate_init(ocb_pubstate *ps);

/* The single writer (core 1): publishes unconditionally. */
OCB_CORE_RAMFUNC void ocb_pubstate_write(ocb_pubstate *ps, const ocb_state *s);

/* The single writer (core 1), once per pass: writes only when
 * publish_count (the core's counter, incremented at every publish) differs
 * from the one last written. Returns true when it wrote. */
OCB_CORE_RAMFUNC bool ocb_pubstate_publish(ocb_pubstate *ps, const ocb_state *s, uint32_t publish_count);

/* The single reader (core 0). Returns true with a consistent copy of a
 * published state in *out, or false with NEUTRAL in *out when no try in
 * OCB_PUBSTATE_TRIES succeeded (counted in `fallbacks`). */
OCB_CORE_RAMFUNC bool ocb_pubstate_read(ocb_pubstate *ps, ocb_state *out);

/* Reader counters (core 0). */
uint32_t ocb_pubstate_retries(const ocb_pubstate *ps);
uint32_t ocb_pubstate_fallbacks(const ocb_pubstate *ps);

/* Steps of a write and of one read try, the very functions write and read
 * are made of, exposed so one test thread can interleave the two cores
 * (F.33). A write is begin, fields part 0 (buttons, hat, lx), fields part 1
 * (ly, rx, ry, flags), end. A try is begin (returns g0), copy part 0 and
 * part 1 of the slot g0 names, end (true when the copy is accepted). */
OCB_CORE_RAMFUNC void ocb_pubstate_write_begin(ocb_pubstate *ps);
OCB_CORE_RAMFUNC void ocb_pubstate_write_fields(ocb_pubstate *ps, const ocb_state *s, int part);
OCB_CORE_RAMFUNC void ocb_pubstate_write_end(ocb_pubstate *ps);
OCB_CORE_RAMFUNC uint32_t ocb_pubstate_try_begin(const ocb_pubstate *ps);
OCB_CORE_RAMFUNC void ocb_pubstate_try_copy(const ocb_pubstate *ps, uint32_t g0, ocb_state *c, int part);
OCB_CORE_RAMFUNC bool ocb_pubstate_try_end(const ocb_pubstate *ps, uint32_t g0);
/* The reader's bookkeeping of one read (also a test hook): `retries`
 * counts reads that used more than one try (a fallback included),
 * `fallbacks` the reads that ended in NEUTRAL; both saturate. */
OCB_CORE_RAMFUNC void ocb_pubstate_note_read(ocb_pubstate *ps, uint32_t tries, bool ok);

#endif /* OCB_LINK_PUBSTATE_H */
