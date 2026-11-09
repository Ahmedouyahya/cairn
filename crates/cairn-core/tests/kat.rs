//! Cross-implementation known-answer tests.
//!
//! `kat/vectors.json` is produced by the independent Python reference
//! (`reference/gen_kat.py`). For every case we replay the same deterministic
//! byte stream through the Rust implementation and require byte-identical
//! share files, both freshly dealt and after a proactive refresh. We also
//! check that Rust recovers the secret from the Python-generated files.

use cairn_core::format::{share_from_bytes, share_to_bytes};
use cairn_core::scheme::refresh_all;
use cairn_core::testrng::SplitMix64;
use cairn_core::{Params, combine, split};

fn hex(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}

#[test]
fn rust_matches_python_reference() {
    let text =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../kat/vectors.json"))
            .expect("run `python3 -I reference/gen_kat.py` first");
    let doc: serde_json::Value = serde_json::from_str(&text).unwrap();
    let cases = doc["cases"].as_array().unwrap();
    assert!(!cases.is_empty());

    for case in cases {
        let seed = case["seed"].as_u64().unwrap();
        let params = Params {
            t: case["t"].as_u64().unwrap() as u8,
            n: case["n"].as_u64().unwrap() as u8,
            ell: case["ell"].as_u64().unwrap() as u8,
            min_limbs: case["min_limbs"].as_u64().unwrap() as usize,
        };
        let secret = hex(case["secret"].as_str().unwrap());

        let mut rng = SplitMix64::new(seed);
        let mut shares = split(&secret, params, &mut rng).unwrap();
        for (i, (ours, theirs)) in shares.iter().zip(case["shares"].as_array().unwrap()).enumerate()
        {
            assert_eq!(
                &share_to_bytes(ours)[..],
                &hex(theirs.as_str().unwrap())[..],
                "seed {seed}: fresh share {i} differs"
            );
        }

        refresh_all(&mut shares, &mut rng).unwrap();
        for (i, (ours, theirs)) in
            shares.iter().zip(case["refreshed"].as_array().unwrap()).enumerate()
        {
            assert_eq!(
                &share_to_bytes(ours)[..],
                &hex(theirs.as_str().unwrap())[..],
                "seed {seed}: refreshed share {i} differs"
            );
        }

        let parsed: Vec<_> = case["refreshed"]
            .as_array()
            .unwrap()
            .iter()
            .map(|h| share_from_bytes(&hex(h.as_str().unwrap())).unwrap())
            .collect();
        let (recovered, _) = combine(&parsed).unwrap();
        assert_eq!(&recovered[..], &secret[..], "seed {seed}: recovery differs");
    }
}
