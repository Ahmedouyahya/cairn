//! Deterministic byte stream for test vectors and lab experiments.
//!
//! **This is not a cryptographic random number generator.** It exists only so
//! that the Rust implementation and the Python reference can be driven by the
//! same byte stream and compared byte for byte. It is compiled only under
//! `cfg(test)` or the `insecure-test-rng` feature, which the CLI never enables.
//!
//! The stream is SplitMix64 (Steele, Lea, Flood 2014): each 64-bit output is
//! written little-endian, and bytes are consumed in order.

use crate::{Error, RandomSource};

/// SplitMix64 byte stream.
#[derive(Clone, Debug)]
pub struct SplitMix64 {
    state: u64,
    buf: [u8; 8],
    pos: usize,
}

impl SplitMix64 {
    /// Start a stream from `seed`.
    pub fn new(seed: u64) -> Self {
        SplitMix64 { state: seed, buf: [0; 8], pos: 8 }
    }

    /// Next 64-bit output.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

impl RandomSource for SplitMix64 {
    fn fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Error> {
        for b in dest.iter_mut() {
            if self.pos == 8 {
                self.buf = self.next_u64().to_le_bytes();
                self.pos = 0;
            }
            *b = self.buf[self.pos];
            self.pos += 1;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_first_output() {
        // Reference value of SplitMix64 with seed 0.
        assert_eq!(SplitMix64::new(0).next_u64(), 0xE220_A839_7B1D_CDAF);
    }
}
