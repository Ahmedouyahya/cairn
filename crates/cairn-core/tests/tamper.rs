//! Tampering and parser robustness, property-tested.
//!
//! The strongest integrity claim of CAIRN: whatever an attacker does to the
//! bytes of up to `t - 1` share files, the combiner either returns the
//! correct secret or refuses. It never returns a wrong secret, and the
//! parser never panics.

use cairn_core::format::{share_from_bytes, share_to_bytes};
use cairn_core::testrng::SplitMix64;
use cairn_core::{Params, combine, split};
use proptest::prelude::*;

fn dealt(seed: u64) -> (Vec<u8>, Vec<Vec<u8>>) {
    let mut rng = SplitMix64::new(seed);
    let key: Vec<u8> =
        (0..32).map(|i| (i as u8).wrapping_mul(37).wrapping_add(seed as u8)).collect();
    let shares = split(&key, Params::new(3, 5), &mut rng).unwrap();
    (key, shares.iter().map(|s| share_to_bytes(s).to_vec()).collect())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(3000))]

    /// Random byte edits to one or two of the three combined files.
    #[test]
    fn tampering_never_yields_a_wrong_secret(
        seed in 0u64..50,
        victims in proptest::sample::subsequence(vec![0usize, 1, 2], 1..=2),
        edits in proptest::collection::vec((0usize..1313, 1u8..=255), 1..6),
    ) {
        let (key, files) = dealt(seed);
        let mut chosen: Vec<Vec<u8>> = files[..3].to_vec();
        for &v in &victims {
            for &(pos, x) in &edits {
                let p = pos % chosen[v].len();
                chosen[v][p] ^= x;
            }
        }
        let parsed: Result<Vec<_>, _> = chosen.iter().map(|b| share_from_bytes(b)).collect();
        if let Ok(shares) = parsed {
            // Refusal is a correct outcome; a wrong secret never is.
            if let Ok((secret, _)) = combine(&shares) {
                prop_assert_eq!(&secret[..], &key[..], "wrong secret accepted");
            }
        }
    }

    /// Arbitrary bytes never crash the parser.
    #[test]
    fn parser_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..2000)) {
        let _ = share_from_bytes(&bytes);
        let _ = cairn_core::format::delta_from_bytes(&bytes);
    }

    /// Valid header with random body: parse, then combine must not panic.
    #[test]
    fn random_bodies_are_handled(seed in 0u64..20, body in proptest::collection::vec(any::<u8>(), 1280)) {
        let (_, files) = dealt(seed);
        let mut f = files[0].clone();
        f[33..].copy_from_slice(&body);
        if let Ok(s) = share_from_bytes(&f) {
            let rest: Vec<_> = files[1..3].iter().map(|b| share_from_bytes(b).unwrap()).collect();
            let mut all = vec![s];
            all.extend(rest);
            let _ = combine(&all);
        }
    }
}

/// With four shares and one cheater, the secret is recovered and the cheater named.
#[test]
fn cheater_is_identified_with_redundancy() {
    for seed in 0..20 {
        let (key, files) = dealt(seed);
        let mut four: Vec<Vec<u8>> = files[..4].to_vec();
        let victim = (seed as usize) % 4;
        let pos = 33 + 640 + 16 * (seed as usize % 40); // an R-half element
        four[victim][pos] ^= 1;
        let shares: Vec<_> = four.iter().map(|b| share_from_bytes(b).unwrap()).collect();
        let (secret, rec) = combine(&shares).unwrap();
        assert_eq!(&secret[..], &key[..]);
        assert_eq!(rec.inconsistent, vec![victim as u8 + 1]);
    }
}
