//! An impaired byte pipe, one direction. It works on the chunks the sender
//! hands out (a whole frame, or a lone `0x00`): drop, flip one bit, insert
//! junk, duplicate, swap with the next chunk, fixed latency. Seeded and
//! deterministic.

use std::collections::VecDeque;

/// The longest a chunk is held back for a reorder, µs.
pub const HOLD_US: u64 = 3_000;

/// xorshift64* (Vigna): small, seedable, stable across versions.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    /// A generator; seed 0 is replaced by a fixed non-zero value.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self(if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        })
    }

    /// The next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A uniform value in `0..n` (`n > 0`).
    pub fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n.max(1)
    }

    /// True with probability `p` (0.0 to 1.0).
    pub fn chance(&mut self, p: f64) -> bool {
        // 53 bits give a uniform f64 in [0, 1).
        let x = (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
        x < p
    }
}

/// Impairment probabilities per chunk.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Impair {
    /// Drop the chunk.
    pub drop: f64,
    /// Flip one random bit of the chunk.
    pub flip: f64,
    /// Put 1 to 8 random bytes in front of the chunk.
    pub junk: f64,
    /// Deliver the chunk twice.
    pub dup: f64,
    /// Swap the chunk with the next one.
    pub reorder: f64,
}

impl Impair {
    /// Every impairment at the same rate.
    #[must_use]
    pub fn all(p: f64) -> Self {
        Self {
            drop: p,
            flip: p,
            junk: p,
            dup: p,
            reorder: p,
        }
    }
}

/// Counters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ChannelStats {
    /// Chunks offered.
    pub chunks: u64,
    /// Dropped.
    pub dropped: u64,
    /// Bit-flipped.
    pub flipped: u64,
    /// Junk inserted.
    pub junked: u64,
    /// Duplicated.
    pub duplicated: u64,
    /// Swapped with the next.
    pub reordered: u64,
}

/// One direction of the wire.
#[derive(Debug, Clone)]
pub struct Channel {
    impair: Impair,
    latency_us: u64,
    rng: Rng,
    queue: VecDeque<(u64, Vec<u8>)>,
    hold: Option<(u64, Vec<u8>)>,
    stats: ChannelStats,
}

impl Channel {
    /// A channel with these impairments, latency and seed.
    #[must_use]
    pub fn new(impair: Impair, latency_us: u64, seed: u64) -> Self {
        Self {
            impair,
            latency_us,
            rng: Rng::new(seed),
            queue: VecDeque::new(),
            hold: None,
            stats: ChannelStats::default(),
        }
    }

    /// A perfect channel with this latency.
    #[must_use]
    pub fn clean(latency_us: u64) -> Self {
        Self::new(Impair::default(), latency_us, 1)
    }

    /// Changes the impairments (e.g. clean for setup, lossy for the test).
    pub fn set_impair(&mut self, impair: Impair) {
        self.impair = impair;
    }

    /// The counters.
    #[must_use]
    pub fn stats(&self) -> ChannelStats {
        self.stats
    }

    fn enqueue(&mut self, now_us: u64, bytes: Vec<u8>) {
        self.queue.push_back((now_us + self.latency_us, bytes));
    }

    /// Offers one chunk at `now_us`.
    pub fn send(&mut self, now_us: u64, chunk: Vec<u8>) {
        self.stats.chunks += 1;
        if self.rng.chance(self.impair.drop) {
            self.stats.dropped += 1;
            return;
        }
        let mut chunk = chunk;
        if !chunk.is_empty() && self.rng.chance(self.impair.flip) {
            let bit = self.rng.below(chunk.len() as u64 * 8);
            if let Some(b) = chunk.get_mut((bit / 8) as usize) {
                *b ^= 1 << (bit % 8);
            }
            self.stats.flipped += 1;
        }
        if self.rng.chance(self.impair.junk) {
            let n = 1 + self.rng.below(8) as usize;
            let mut junk: Vec<u8> = (0..n).map(|_| self.rng.next_u64() as u8).collect();
            junk.extend_from_slice(&chunk);
            chunk = junk;
            self.stats.junked += 1;
        }
        let dup = self.rng.chance(self.impair.dup);
        if dup {
            self.stats.duplicated += 1;
        }
        // Reorder: hold this chunk back and send it after the next one.
        if let Some((_, held)) = self.hold.take() {
            self.enqueue(now_us, chunk.clone());
            if dup {
                self.enqueue(now_us, chunk);
            }
            self.enqueue(now_us, held);
            return;
        }
        if self.rng.chance(self.impair.reorder) {
            self.stats.reordered += 1;
            self.hold = Some((now_us, chunk));
            return;
        }
        self.enqueue(now_us, chunk.clone());
        if dup {
            self.enqueue(now_us, chunk);
        }
    }

    /// Releases a chunk held for a reorder once it has waited `HOLD_US`
    /// with nothing to swap with (so a reorder can cross clock steps but
    /// never delays a chunk for long).
    pub fn flush_hold(&mut self, now_us: u64) {
        if let Some((t, _)) = &self.hold
            && now_us >= t + HOLD_US
            && let Some((_, h)) = self.hold.take()
        {
            self.enqueue(now_us, h);
        }
    }

    /// The bytes due by `now_us`, in delivery order.
    pub fn deliver(&mut self, now_us: u64) -> Vec<u8> {
        let mut out = Vec::new();
        while let Some((t, _)) = self.queue.front() {
            if *t > now_us {
                break;
            }
            if let Some((_, b)) = self.queue.pop_front() {
                out.extend(b);
            }
        }
        out
    }

    /// When the next chunk is due.
    #[must_use]
    pub fn next_due(&self) -> Option<u64> {
        let held = self.hold.as_ref().map(|(t, _)| t + HOLD_US);
        [self.queue.front().map(|(t, _)| *t), held]
            .into_iter()
            .flatten()
            .min()
    }
}
