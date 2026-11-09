//! Experiment 6: ctgrind, secret-dependent control flow and memory access.
//!
//! Technique of Adam Langley's `ctgrind` (2010): tell Valgrind's memcheck
//! that the secret bytes are *uninitialised*. Memcheck then reports every
//! conditional branch and every memory address that depends on them. A
//! constant-time implementation produces zero reports; the only legitimate
//! uses of secret data in control flow are explicit declassifications, which
//! we mark as defined right before the branch.
//!
//! Run with:
//! ```text
//! valgrind --error-exitcode=42 target/release/cairn-lab ctgrind-cairn   # expect 0 errors
//! valgrind --error-exitcode=42 target/release/cairn-lab ctgrind-naive   # expect errors
//! ```
//! Outside Valgrind the client requests below are no-ops.

use cairn_core::scheme::{Share, reconstruct_into, refresh_local, split_vector};
use cairn_core::testrng::SplitMix64;
use cairn_core::{Field, Fp127, Params, amd, ipenc};

const MAKE_MEM_UNDEFINED: u64 = 0x4D43_0001;
const MAKE_MEM_DEFINED: u64 = 0x4D43_0002;

/// Valgrind client request (amd64 magic sequence from `valgrind.h`).
#[allow(unsafe_code)]
fn client_request(req: u64, addr: u64, len: u64) -> u64 {
    #[cfg(target_arch = "x86_64")]
    {
        let args: [u64; 6] = [req, addr, len, 0, 0, 0];
        let mut result: u64 = 0;
        // SAFETY: the rotations of rdi sum to 128 bits (identity) and
        // `xchg rbx, rbx` is a no-op; Valgrind recognises the sequence and
        // reads `args` through rax. Natively nothing happens.
        unsafe {
            core::arch::asm!(
                "rol rdi, 3",
                "rol rdi, 13",
                "rol rdi, 61",
                "rol rdi, 51",
                "xchg rbx, rbx",
                inout("rdx") result,
                in("rax") args.as_ptr(),
                inout("rdi") 0u64 => _,
                options(nostack),
            );
        }
        result
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        let _ = (req, addr, len);
        0
    }
}

fn poison<T>(x: &[T]) {
    client_request(MAKE_MEM_UNDEFINED, x.as_ptr() as u64, std::mem::size_of_val(x) as u64);
}

fn declassify<T>(x: &T) {
    client_request(MAKE_MEM_DEFINED, x as *const T as u64, std::mem::size_of::<T>() as u64);
}

/// Everything CAIRN does with secret data, with the secret poisoned.
pub fn run_cairn() -> bool {
    let mut rng = SplitMix64::new(77);
    let key: Vec<Fp127> = (1..=3).map(|i| Fp127::from_u64(i * 1_000_003)).collect();
    let mut shares: Vec<Share<Fp127>> = split_vector(&key, Params::new(3, 5), &mut rng).unwrap();
    shares.truncate(3);
    for s in &shares {
        poison(s.left());
        poison(s.right());
    }

    // L6: reconstruction and AMD verification.
    let subset: Vec<&Share<Fp127>> = shares.iter().collect();
    let mut out = vec![Fp127::zero(); 5];
    reconstruct_into(&subset, &mut out).unwrap();
    let ok = amd::verify(&out);
    declassify(&ok); // the verdict is public by design
    assert!(ok.declassify());

    // L4/L5: local refresh of poisoned storage, then reconstruct again.
    for s in shares.iter_mut() {
        refresh_local(s, &mut rng).unwrap();
    }
    let subset: Vec<&Share<Fp127>> = shares.iter().collect();
    reconstruct_into(&subset, &mut out).unwrap();
    let ok = amd::verify(&out);
    declassify(&ok);
    assert!(ok.declassify());

    // Field primitives on a poisoned operand.
    let x = [Fp127::from_u64(123_456_789)];
    poison(&x);
    let y = x[0].inv() * x[0] + x[0];
    std::hint::black_box(Fp127::ct_select(y.ct_is_zero(), &x[0], &y));

    // L4 encoding of a poisoned value.
    let sigma = [Fp127::from_u64(42)];
    poison(&sigma);
    let mut l = vec![Fp127::zero(); 8];
    let mut r = vec![Fp127::zero(); 8];
    ipenc::encode_into(sigma[0], &mut l, &mut r, &mut rng).unwrap();
    let back = [ipenc::inner(&l, &r).ct_eq(&sigma[0])];
    declassify(&back);
    assert!(back[0].declassify());

    // Make everything defined again before normal code (Drop, zeroize) runs.
    for s in &shares {
        client_request(
            MAKE_MEM_DEFINED,
            s.left().as_ptr() as u64,
            std::mem::size_of_val(s.left()) as u64,
        );
        client_request(
            MAKE_MEM_DEFINED,
            s.right().as_ptr() as u64,
            std::mem::size_of_val(s.right()) as u64,
        );
    }
    println!("ctgrind-cairn: done (inspect the Valgrind error summary)");
    true
}

/// The naive baseline, with the secret shares poisoned.
pub fn run_naive() -> bool {
    let mut c = 5u128;
    let mut coins = || {
        c = c.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(3);
        c >> 1
    };
    let shares = cairn_naive::split(&[1_000_003, 2_000_006, 3_000_009], 3, 5, &mut coins);
    let work = shares[..3].to_vec();
    for s in &work {
        poison(&s.1);
    }
    let r = cairn_naive::reconstruct(&work);
    declassify(&r);
    println!("ctgrind-naive: done (inspect the Valgrind error summary)");
    true
}
