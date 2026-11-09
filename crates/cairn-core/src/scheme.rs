//! L5 and L6: dealing, proactive refresh and constant-time reconstruction.
//!
//! Everything here is generic over [`Field`], so the lab can run the exact
//! production algorithms in tiny fields and check the theorems exhaustively.

use alloc::vec;
use alloc::vec::Vec;
use zeroize::{Zeroize, Zeroizing};

use crate::{Error, Field, RandomSource, amd, ipenc, poly};

/// Upper bound on the number of `t`-subsets tried during cheater-tolerant
/// reconstruction. Each failed subset costs the adversary at most
/// `(k + 1) / p` of forging probability, so the union bound stays below
/// `2^16 * (k + 1) / p`.
pub const MAX_SUBSETS: usize = 1 << 16;

/// Public dealing parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Params {
    /// Threshold: number of shares needed to recover the secret.
    pub t: u8,
    /// Number of shares.
    pub n: u8,
    /// Inner-product dimension of the leakage-resilient storage (L4).
    pub ell: u8,
    /// Pad the secret to at least this many limbs to hide its length.
    pub min_limbs: usize,
}

impl Params {
    /// `t`-of-`n` with the default storage dimension `ell = 8`.
    pub fn new(t: u8, n: u8) -> Params {
        Params { t, n, ell: 8, min_limbs: 0 }
    }

    /// Check the parameters for a field of characteristic `F::MODULUS`.
    pub fn validate<F: Field>(&self) -> Result<(), Error> {
        if self.t == 0 {
            return Err(Error::InvalidParams("threshold must be at least 1"));
        }
        if self.t > self.n {
            return Err(Error::InvalidParams("threshold exceeds number of shares"));
        }
        if (self.n as u128) >= F::MODULUS {
            return Err(Error::InvalidParams("field too small for this many shares"));
        }
        if self.ell == 0 || self.ell > 64 {
            return Err(Error::InvalidParams("ell must be between 1 and 64"));
        }
        Ok(())
    }
}

/// Public metadata carried by every share and delta.
///
/// None of these fields is secret. None of them is trusted either: a forged
/// header can only cause a rejection (consistency checks) or an additive
/// error that the AMD layer detects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Meta {
    /// Random identifier of the dealing, shared by all its shares.
    pub set_id: [u8; 16],
    /// Threshold.
    pub t: u8,
    /// Number of shares.
    pub n: u8,
    /// This share's evaluation point, in `1..=n`.
    pub index: u8,
    /// Inner-product dimension.
    pub ell: u8,
    /// Armored dimension `D = k + 2`.
    pub dim: u16,
    /// Refresh epoch, incremented by every proactive refresh.
    pub epoch: u32,
}

impl Meta {
    fn same_dealing(&self, o: &Meta) -> bool {
        self.set_id == o.set_id
            && self.t == o.t
            && self.n == o.n
            && self.ell == o.ell
            && self.dim == o.dim
    }
}

/// One custodian's share, stored as split-state inner-product pairs.
///
/// Component `c` of the share value is `<left[c*ell..(c+1)*ell], right[..]>`.
/// The two halves are separate allocations and are written to separate
/// sections of the share file.
pub struct Share<F: Field> {
    /// Public metadata.
    pub meta: Meta,
    left: Zeroizing<Vec<F>>,
    right: Zeroizing<Vec<F>>,
}

impl<F: Field> Share<F> {
    /// Assemble a share from its parts (used by the file parser).
    pub fn from_parts(meta: Meta, left: Vec<F>, right: Vec<F>) -> Result<Share<F>, Error> {
        let need = meta.dim as usize * meta.ell as usize;
        if left.len() != need || right.len() != need {
            let (mut l, mut r) = (left, right);
            l.zeroize();
            r.zeroize();
            return Err(Error::Malformed("share body has the wrong length"));
        }
        Ok(Share { meta, left: Zeroizing::new(left), right: Zeroizing::new(right) })
    }
    /// The `L` half.
    pub fn left(&self) -> &[F] {
        &self.left
    }
    /// The `R` half.
    pub fn right(&self) -> &[F] {
        &self.right
    }
    /// Mutable access to both halves. Used by the lab to simulate attacks.
    pub fn halves_mut(&mut self) -> (&mut [F], &mut [F]) {
        (&mut self.left, &mut self.right)
    }
    /// The share value (component-wise inner products).
    ///
    /// CAIRN itself never calls this; it exists for tests and the lab.
    pub fn value(&self) -> Zeroizing<Vec<F>> {
        let ell = self.meta.ell as usize;
        Zeroizing::new(
            self.left
                .chunks_exact(ell)
                .zip(self.right.chunks_exact(ell))
                .map(|(l, r)| ipenc::inner(l, r))
                .collect(),
        )
    }
}

/// A proactive-refresh delta for one custodian: one evaluation of a random
/// zero-sharing per component. Confidential: an attacker holding an old
/// share and its delta can compute the new share.
pub struct Delta<F: Field> {
    /// Metadata of the target share (`index`, and `epoch` before refresh).
    pub meta: Meta,
    /// One value per armored component.
    pub values: Zeroizing<Vec<F>>,
}

/// Result of a successful reconstruction.
pub struct Recovery<F: Field> {
    /// The recovered secret vector `(s_1, ..., s_k)` (AMD armor removed).
    pub value: Zeroizing<Vec<F>>,
    /// Indices of the shares that produced the accepted reconstruction.
    pub used: Vec<u8>,
    /// Indices of supplied shares that are inconsistent with it.
    pub inconsistent: Vec<u8>,
    /// Number of `t`-subsets evaluated.
    pub subsets_tried: usize,
}

/// Deal shares of the secret vector `s` (L2, L3, L4).
///
/// Normative randomness consumption, in order:
/// 1. 16 bytes of `set_id`;
/// 2. the AMD point `x` (one field element);
/// 3. the polynomial coefficients, component by component;
/// 4. the inner-product encodings, share by share, component by component.
pub fn split_vector<F: Field, R: RandomSource + ?Sized>(
    s: &[F],
    params: Params,
    rng: &mut R,
) -> Result<Vec<Share<F>>, Error> {
    params.validate::<F>()?;
    let mut set_id = [0u8; 16];
    rng.fill_bytes(&mut set_id)?;

    let armored = amd::armor(s, rng)?;
    let dim = armored.len();
    if dim > u16::MAX as usize {
        return Err(Error::SecretTooLong);
    }
    let (t, ell) = (params.t as usize, params.ell as usize);
    let coeffs = poly::coefficients(&armored, t, rng)?;

    let mut shares = Vec::with_capacity(params.n as usize);
    for j in 1..=params.n {
        let sigma = poly::eval_rows(&coeffs, t, F::from_u64(j as u64));
        let mut left = Zeroizing::new(vec![F::zero(); dim * ell]);
        let mut right = Zeroizing::new(vec![F::zero(); dim * ell]);
        for (c, value) in sigma.iter().enumerate() {
            let span = c * ell..(c + 1) * ell;
            ipenc::encode_into(*value, &mut left[span.clone()], &mut right[span], rng)?;
        }
        let meta = Meta {
            set_id,
            t: params.t,
            n: params.n,
            index: j,
            ell: params.ell,
            dim: dim as u16,
            epoch: 0,
        };
        shares.push(Share { meta, left, right });
    }
    Ok(shares)
}

/// Check that a set of shares can be combined. Returns them sorted by index.
fn check_set<F: Field>(shares: &[Share<F>]) -> Result<Vec<&Share<F>>, Error> {
    let first = shares.first().ok_or(Error::NotEnoughShares { needed: 1, got: 0 })?;
    let m0 = first.meta;
    let (mut lo, mut hi) = (m0.epoch, m0.epoch);
    for s in shares {
        if !s.meta.same_dealing(&m0) {
            return Err(Error::Mismatch("different dealing or parameters"));
        }
        if s.meta.index == 0 || s.meta.index > s.meta.n {
            return Err(Error::Malformed("share index out of range"));
        }
        let need = m0.dim as usize * m0.ell as usize;
        if s.left.len() != need || s.right.len() != need {
            return Err(Error::Malformed("share body has the wrong length"));
        }
        lo = lo.min(s.meta.epoch);
        hi = hi.max(s.meta.epoch);
    }
    if lo != hi {
        return Err(Error::EpochMismatch { min: lo, max: hi });
    }
    let mut sorted: Vec<&Share<F>> = shares.iter().collect();
    sorted.sort_by_key(|s| s.meta.index);
    for w in sorted.windows(2) {
        if w[0].meta.index == w[1].meta.index {
            return Err(Error::DuplicateIndex(w[0].meta.index));
        }
    }
    if sorted.len() < m0.t as usize {
        return Err(Error::NotEnoughShares { needed: m0.t, got: sorted.len() });
    }
    if m0.dim < 3 {
        return Err(Error::Malformed("armored dimension below 3"));
    }
    Ok(sorted)
}

/// Constant-time reconstruction of the armored vector from exactly `t` shares.
///
/// `a_c = sum_j lambda_j <L_j^c, R_j^c> = sum_i sum_j (lambda_j L_{j,i}^c) R_{j,i}^c`.
///
/// The loop runs over the coordinate `i` outside and the share `j` inside, so
/// the running sum is never equal to any single share value: no individual
/// share is ever materialized, in memory or in a register. Loop bounds and
/// Lagrange coefficients depend only on public data.
pub fn reconstruct_into<F: Field>(subset: &[&Share<F>], out: &mut [F]) -> Result<(), Error> {
    let xs: Vec<u8> = subset.iter().map(|s| s.meta.index).collect();
    let lambdas: Vec<F> = poly::lagrange_at_zero(&xs)?;
    let ell = subset[0].meta.ell as usize;
    for (c, slot) in out.iter_mut().enumerate() {
        let mut acc = F::zero();
        for i in 0..ell {
            let pos = c * ell + i;
            for (lam, sh) in lambdas.iter().zip(subset.iter()) {
                acc += (*lam * sh.left[pos]) * sh.right[pos];
            }
        }
        *slot = acc;
    }
    Ok(())
}

/// Advance `idx` to the next `t`-combination of `0..m` in lexicographic order.
fn next_combination(idx: &mut [usize], m: usize) -> bool {
    let t = idx.len();
    let mut i = t;
    while i > 0 {
        i -= 1;
        if idx[i] < m - t + i {
            idx[i] += 1;
            for j in i + 1..t {
                idx[j] = idx[j - 1] + 1;
            }
            return true;
        }
    }
    false
}

/// Reconstruct and check the AMD tag for one subset.
fn try_subset<F: Field>(subset: &[&Share<F>], buf: &mut [F]) -> Result<bool, Error> {
    reconstruct_into(subset, buf)?;
    // Declassifying the verdict is part of the protocol: the combiner must
    // know whether to output the secret.
    Ok(amd::verify(buf).declassify())
}

/// Recover the secret vector from `t` or more shares (L6).
///
/// Tries `t`-subsets in lexicographic order of index until one passes the
/// AMD check, then tests every other supplied share against that subset and
/// reports the ones that are inconsistent (tampered, forged or stale).
pub fn combine_vector<F: Field>(shares: &[Share<F>]) -> Result<Recovery<F>, Error> {
    let sorted = check_set(shares)?;
    let meta = sorted[0].meta;
    let (t, m, dim) = (meta.t as usize, sorted.len(), meta.dim as usize);

    let mut buf: Zeroizing<Vec<F>> = Zeroizing::new(vec![F::zero(); dim]);
    let mut idx: Vec<usize> = (0..t).collect();
    let mut tried = 0usize;
    let mut found = false;
    loop {
        tried += 1;
        let subset: Vec<&Share<F>> = idx.iter().map(|&i| sorted[i]).collect();
        if try_subset(&subset, &mut buf)? {
            found = true;
            break;
        }
        if tried >= MAX_SUBSETS || !next_combination(&mut idx, m) {
            break;
        }
    }
    if !found {
        return Err(Error::IntegrityFailure);
    }

    // Cheater identification: swap each unused share into the accepted set.
    let mut inconsistent = Vec::new();
    let mut probe: Zeroizing<Vec<F>> = Zeroizing::new(vec![F::zero(); dim]);
    for (o, other) in sorted.iter().enumerate() {
        if idx.contains(&o) {
            continue;
        }
        let mut subset: Vec<&Share<F>> = idx[1..].iter().map(|&i| sorted[i]).collect();
        subset.push(other);
        if !try_subset(&subset, &mut probe)? {
            inconsistent.push(other.meta.index);
        }
    }

    let value = Zeroizing::new(amd::payload(&buf).to_vec());
    Ok(Recovery {
        value,
        used: idx.iter().map(|&i| sorted[i].meta.index).collect(),
        inconsistent,
        subsets_tried: tried,
    })
}

/// Generate proactive-refresh deltas for every index `1..=n` of a dealing.
///
/// For each component `c`, draw `g_c(X) = r_1 X + ... + r_{t-1} X^{t-1}`
/// (so `g_c(0) = 0`) and give custodian `j` the values `g_c(j)`. Adding them
/// moves every share to a fresh, independent sharing of the same secret.
/// The dealer of the deltas never sees the secret or any share.
pub fn refresh_deltas<F: Field, R: RandomSource + ?Sized>(
    meta: &Meta,
    rng: &mut R,
) -> Result<Vec<Delta<F>>, Error> {
    let t = meta.t as usize;
    let zeros = vec![F::zero(); meta.dim as usize];
    let coeffs = poly::coefficients(&zeros, t, rng)?;
    let mut out = Vec::with_capacity(meta.n as usize);
    for j in 1..=meta.n {
        let values = poly::eval_rows(&coeffs, t, F::from_u64(j as u64));
        out.push(Delta { meta: Meta { index: j, ..*meta }, values });
    }
    Ok(out)
}

/// Apply a refresh delta to a share and move it to the next epoch.
///
/// Each component becomes `<L, R> + delta_c` (via [`ipenc::add_scalar`]) and
/// is then re-randomized (via [`ipenc::refresh`]), so the new pair is
/// unlinkable to the old one.
pub fn apply_delta<F: Field, R: RandomSource + ?Sized>(
    share: &mut Share<F>,
    delta: &Delta<F>,
    rng: &mut R,
) -> Result<(), Error> {
    if !share.meta.same_dealing(&delta.meta) {
        return Err(Error::Mismatch("delta belongs to a different dealing"));
    }
    if share.meta.index != delta.meta.index {
        return Err(Error::Mismatch("delta belongs to a different custodian"));
    }
    if share.meta.epoch != delta.meta.epoch {
        return Err(Error::EpochMismatch {
            min: share.meta.epoch.min(delta.meta.epoch),
            max: share.meta.epoch.max(delta.meta.epoch),
        });
    }
    if delta.values.len() != share.meta.dim as usize {
        return Err(Error::Malformed("delta has the wrong length"));
    }
    let next = share.meta.epoch.checked_add(1).ok_or(Error::InvalidParams("epoch overflow"))?;
    let ell = share.meta.ell as usize;
    for (c, d) in delta.values.iter().enumerate() {
        let span = c * ell..(c + 1) * ell;
        let (l, r) = (&mut share.left[span.clone()], &mut share.right[span]);
        ipenc::add_scalar(l, r, *d, rng)?;
        ipenc::refresh(l, r, rng)?;
    }
    share.meta.epoch = next;
    Ok(())
}

/// Re-randomize a share's inner-product storage without changing its value.
/// Local and silent: other custodians are not involved and the epoch is kept.
pub fn refresh_local<F: Field, R: RandomSource + ?Sized>(
    share: &mut Share<F>,
    rng: &mut R,
) -> Result<(), Error> {
    let ell = share.meta.ell as usize;
    for (l, r) in share.left.chunks_exact_mut(ell).zip(share.right.chunks_exact_mut(ell)) {
        ipenc::refresh(l, r, rng)?;
    }
    Ok(())
}

/// Proactively refresh a complete set of shares held together.
pub fn refresh_all<F: Field, R: RandomSource + ?Sized>(
    shares: &mut [Share<F>],
    rng: &mut R,
) -> Result<(), Error> {
    let meta = shares.first().ok_or(Error::NotEnoughShares { needed: 1, got: 0 })?.meta;
    for s in shares.iter() {
        if !s.meta.same_dealing(&meta) || s.meta.epoch != meta.epoch {
            return Err(Error::Mismatch("refresh_all needs shares of one dealing and epoch"));
        }
    }
    let deltas = refresh_deltas::<F, R>(&meta, rng)?;
    for s in shares.iter_mut() {
        let d = &deltas[s.meta.index as usize - 1];
        apply_delta(s, d, rng)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::Fp127;
    use crate::testrng::SplitMix64;

    fn secret() -> Vec<Fp127> {
        (1..=3).map(|i| Fp127::from_u64(i * 0x1111_1111)).collect()
    }

    fn same(a: &[Fp127], b: &[Fp127]) -> bool {
        a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.ct_eq(y).declassify())
    }

    #[test]
    fn every_t_subset_recovers() {
        let mut rng = SplitMix64::new(11);
        let shares = split_vector(&secret(), Params::new(3, 5), &mut rng).unwrap();
        let mut idx = vec![0usize, 1, 2];
        loop {
            let subset: Vec<&Share<Fp127>> = idx.iter().map(|&i| &shares[i]).collect();
            let mut buf = vec![Fp127::zero(); 5];
            reconstruct_into(&subset, &mut buf).unwrap();
            assert!(amd::verify(&buf).declassify());
            assert!(same(amd::payload(&buf), &secret()));
            if !next_combination(&mut idx, 5) {
                break;
            }
        }
    }

    #[test]
    fn tampered_share_is_identified() {
        let mut rng = SplitMix64::new(12);
        let mut shares = split_vector(&secret(), Params::new(3, 5), &mut rng).unwrap();
        shares[0].halves_mut().1[0] += Fp127::one();
        let rec = combine_vector(&shares).unwrap();
        assert!(same(&rec.value, &secret()));
        assert_eq!(rec.inconsistent, vec![1]);
        assert!(!rec.used.contains(&1));
    }

    #[test]
    fn tamper_with_exactly_t_is_detected() {
        let mut rng = SplitMix64::new(13);
        let mut shares = split_vector(&secret(), Params::new(3, 5), &mut rng).unwrap();
        shares.truncate(3);
        shares[2].halves_mut().0[4] += Fp127::one();
        assert_eq!(combine_vector(&shares).err(), Some(Error::IntegrityFailure));
    }

    #[test]
    fn refresh_keeps_secret_and_changes_shares() {
        let mut rng = SplitMix64::new(14);
        let mut shares = split_vector(&secret(), Params::new(2, 4), &mut rng).unwrap();
        let before: Vec<_> = shares.iter().map(|s| s.value()).collect();
        refresh_all(&mut shares, &mut rng).unwrap();
        refresh_local(&mut shares[1], &mut rng).unwrap();
        for (b, s) in before.iter().zip(&shares) {
            assert!(!same(b, &s.value()), "share value must change");
            assert_eq!(s.meta.epoch, 1);
        }
        let rec = combine_vector(&shares[2..]).unwrap();
        assert!(same(&rec.value, &secret()));
    }

    #[test]
    fn mixed_epochs_are_rejected_even_if_relabelled() {
        let mut rng = SplitMix64::new(15);
        let mut shares = split_vector(&secret(), Params::new(2, 3), &mut rng).unwrap();
        let old = Share::from_parts(
            shares[0].meta,
            shares[0].left().to_vec(),
            shares[0].right().to_vec(),
        )
        .unwrap();
        refresh_all(&mut shares, &mut rng).unwrap();
        // Honest metadata: refused before any arithmetic.
        let mixed = [
            old,
            Share::from_parts(
                shares[1].meta,
                shares[1].left().to_vec(),
                shares[1].right().to_vec(),
            )
            .unwrap(),
        ];
        assert!(matches!(combine_vector(&mixed), Err(Error::EpochMismatch { .. })));
        // Forged metadata: caught by the AMD layer.
        let [mut old, new] = mixed;
        old.meta.epoch = new.meta.epoch;
        assert_eq!(combine_vector(&[old, new]).err(), Some(Error::IntegrityFailure));
    }

    #[test]
    fn rejects_bad_sets() {
        let mut rng = SplitMix64::new(16);
        let shares = split_vector(&secret(), Params::new(3, 4), &mut rng).unwrap();
        assert!(matches!(combine_vector(&shares[..2]), Err(Error::NotEnoughShares { .. })));
        let other = split_vector(&secret(), Params::new(3, 4), &mut rng).unwrap();
        let clone = |s: &Share<Fp127>| {
            Share::from_parts(s.meta, s.left().to_vec(), s.right().to_vec()).unwrap()
        };
        let mixed = [clone(&shares[0]), clone(&shares[1]), clone(&other[2])];
        assert!(matches!(combine_vector(&mixed), Err(Error::Mismatch(_))));
        let dup = [clone(&shares[0]), clone(&shares[0]), clone(&shares[1])];
        assert_eq!(combine_vector(&dup).err(), Some(Error::DuplicateIndex(1)));
    }

    #[test]
    fn param_validation() {
        assert!(Params::new(0, 3).validate::<Fp127>().is_err());
        assert!(Params::new(4, 3).validate::<Fp127>().is_err());
        assert!(Params { ell: 0, ..Params::new(2, 3) }.validate::<Fp127>().is_err());
        assert!(Params::new(2, 13).validate::<crate::Toy<13>>().is_err());
    }

    #[test]
    fn combinations_enumerate_all() {
        let mut idx = vec![0, 1];
        let mut count = 1;
        while next_combination(&mut idx, 5) {
            count += 1;
        }
        assert_eq!(count, 10);
    }
}
