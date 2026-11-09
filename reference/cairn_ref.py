"""CAIRN reference implementation (executable specification).

A deliberately plain, readable, big-integer Python transcription of the
algorithms in the paper. It is NOT constant time and must never protect a
real key. Its only job is to be an independent second implementation:
driven by the same deterministic byte stream, it must produce share files
byte-identical to the Rust implementation (see ``make kat``).

Run ``python3 -I reference/cairn_ref.py`` to self-test.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from itertools import combinations
import struct

P = (1 << 127) - 1
LIMB_BYTES = 15
MAGIC = b"CAIRN"
VERSION = 1
KIND_SHARE, KIND_DELTA = 1, 2


# --------------------------------------------------------------------------
# Deterministic test stream (SplitMix64). NOT a secure RNG.
# --------------------------------------------------------------------------
class SplitMix64:
    MASK = (1 << 64) - 1

    def __init__(self, seed: int):
        self.state = seed & self.MASK
        self.buf = b""

    def next_u64(self) -> int:
        self.state = (self.state + 0x9E3779B97F4A7C15) & self.MASK
        z = self.state
        z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & self.MASK
        z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & self.MASK
        return z ^ (z >> 31)

    def bytes(self, n: int) -> bytes:
        out = bytearray()
        while len(out) < n:
            if not self.buf:
                self.buf = self.next_u64().to_bytes(8, "little")
            take = min(n - len(out), len(self.buf))
            out += self.buf[:take]
            self.buf = self.buf[take:]
        return bytes(out)


# --------------------------------------------------------------------------
# Field GF(2^127 - 1)
# --------------------------------------------------------------------------
def rand_elem(rng: SplitMix64) -> int:
    while True:
        x = int.from_bytes(rng.bytes(16), "little") & P
        if x != P:
            return x


def rand_nonzero(rng: SplitMix64) -> int:
    while True:
        x = rand_elem(rng)
        if x != 0:
            return x


def inv(x: int) -> int:
    return pow(x, P - 2, P)


# --------------------------------------------------------------------------
# L1 encoding
# --------------------------------------------------------------------------
def limbs_for(length: int, min_limbs: int = 0) -> int:
    return max(-(-(length + 2) // LIMB_BYTES), min_limbs, 1)


def encode(secret: bytes, min_limbs: int = 0) -> list[int]:
    assert len(secret) <= 0xFFFF
    k = limbs_for(len(secret), min_limbs)
    buf = struct.pack(">H", len(secret)) + secret
    buf += b"\x00" * (k * LIMB_BYTES - len(buf))
    return [int.from_bytes(buf[i : i + LIMB_BYTES], "big") for i in range(0, len(buf), LIMB_BYTES)]


def decode(limbs: list[int]) -> bytes:
    if any(x >> 120 for x in limbs):
        raise ValueError("limb exceeds 120 bits")
    buf = b"".join(x.to_bytes(LIMB_BYTES, "big") for x in limbs)
    n = struct.unpack(">H", buf[:2])[0]
    if n + 2 > len(buf) or any(buf[2 + n :]):
        raise ValueError("not a CAIRN encoding")
    return buf[2 : 2 + n]


# --------------------------------------------------------------------------
# L2 AMD armor:  tau = x^(k+2) + sum_i s_i x^i
# --------------------------------------------------------------------------
def amd_tag(s: list[int], x: int) -> int:
    k = len(s)
    return (pow(x, k + 2, P) + sum(si * pow(x, i + 1, P) for i, si in enumerate(s))) % P


def armor(s: list[int], rng: SplitMix64) -> list[int]:
    x = rand_elem(rng)
    return s + [x, amd_tag(s, x)]


def amd_ok(a: list[int]) -> bool:
    return amd_tag(a[:-2], a[-2]) == a[-1]


# --------------------------------------------------------------------------
# L3 polynomial sharing
# --------------------------------------------------------------------------
def poly_eval(coeffs: list[int], x: int) -> int:
    return sum(c * pow(x, m, P) for m, c in enumerate(coeffs)) % P


def coefficients(values: list[int], t: int, rng: SplitMix64) -> list[list[int]]:
    return [[v] + [rand_elem(rng) for _ in range(t - 1)] for v in values]


def lagrange_at_zero(xs: list[int]) -> list[int]:
    out = []
    for j, xj in enumerate(xs):
        num = den = 1
        for m, xm in enumerate(xs):
            if m != j:
                num = num * xm % P
                den = den * (xm - xj) % P
        out.append(num * inv(den) % P)
    return out


# --------------------------------------------------------------------------
# L4 inner-product storage
# --------------------------------------------------------------------------
def dot(l: list[int], r: list[int]) -> int:
    return sum(a * b for a, b in zip(l, r)) % P


def ip_encode(sigma: int, ell: int, rng: SplitMix64) -> tuple[list[int], list[int]]:
    l = [rand_nonzero(rng)] + [rand_elem(rng) for _ in range(ell - 1)]
    r_tail = [rand_elem(rng) for _ in range(ell - 1)]
    r1 = (sigma - dot(l[1:], r_tail)) * inv(l[0]) % P
    return l, [r1] + r_tail


def add_skew_product(v: list[int], w: list[int], rng: SplitMix64) -> list[int]:
    """Return v + M w for a fresh uniform skew-symmetric M."""
    ell = len(v)
    d = [0] * ell
    for i in range(ell):
        for j in range(i + 1, ell):
            m = rand_elem(rng)
            d[i] = (d[i] + m * w[j]) % P
            d[j] = (d[j] - m * w[i]) % P
    return [(a + b) % P for a, b in zip(v, d)]


def ip_refresh(l, r, rng):
    """Three Gibbs half-steps: L, then R, then L again."""
    l = add_skew_product(l, r, rng)
    r = add_skew_product(r, l, rng)
    l = add_skew_product(l, r, rng)
    return l, r


# --------------------------------------------------------------------------
# Shares, dealing, reconstruction, refresh
# --------------------------------------------------------------------------
@dataclass
class Share:
    set_id: bytes
    t: int
    n: int
    index: int
    ell: int
    dim: int
    epoch: int
    left: list[list[int]] = field(default_factory=list)  # dim x ell
    right: list[list[int]] = field(default_factory=list)

    def value(self) -> list[int]:
        return [dot(l, r) for l, r in zip(self.left, self.right)]

    def header(self, kind: int = KIND_SHARE) -> bytes:
        return (
            MAGIC
            + bytes([VERSION, kind])
            + self.set_id
            + bytes([self.t, self.n, self.index, self.ell])
            + struct.pack("<HI", self.dim, self.epoch)
        )

    def to_bytes(self) -> bytes:
        body = b"".join(x.to_bytes(16, "little") for row in self.left for x in row)
        body += b"".join(x.to_bytes(16, "little") for row in self.right for x in row)
        return self.header() + body

    @staticmethod
    def from_bytes(b: bytes) -> "Share":
        assert b[:5] == MAGIC and b[5] == VERSION and b[6] == KIND_SHARE
        set_id = b[7:23]
        t, n, index, ell = b[23], b[24], b[25], b[26]
        dim, epoch = struct.unpack("<HI", b[27:33])
        half = dim * ell * 16
        assert len(b) == 33 + 2 * half
        elems = [int.from_bytes(b[33 + 16 * i : 49 + 16 * i], "little") for i in range(2 * dim * ell)]
        assert all(e < P for e in elems)
        rows = [elems[i * ell : (i + 1) * ell] for i in range(2 * dim)]
        return Share(set_id, t, n, index, ell, dim, epoch, rows[:dim], rows[dim:])


def split(secret: bytes, t: int, n: int, rng: SplitMix64, ell: int = 8, min_limbs: int = 0) -> list[Share]:
    set_id = rng.bytes(16)
    a = armor(encode(secret, min_limbs), rng)
    coeffs = coefficients(a, t, rng)
    shares = []
    for j in range(1, n + 1):
        sh = Share(set_id, t, n, j, ell, len(a), 0)
        for row in coeffs:
            l, r = ip_encode(poly_eval(row, j), ell, rng)
            sh.left.append(l)
            sh.right.append(r)
        shares.append(sh)
    return shares


def reconstruct(subset: list[Share]) -> list[int]:
    lam = lagrange_at_zero([s.index for s in subset])
    return [sum(lj * sj.value()[c] for lj, sj in zip(lam, subset)) % P for c in range(subset[0].dim)]


def combine(shares: list[Share]) -> bytes:
    shares = sorted(shares, key=lambda s: s.index)
    t = shares[0].t
    for subset in combinations(shares, t):
        a = reconstruct(list(subset))
        if amd_ok(a):
            return decode(a[:-2])
    raise ValueError("integrity failure")


def refresh_all(shares: list[Share], rng: SplitMix64) -> None:
    m = shares[0]
    zero_coeffs = coefficients([0] * m.dim, m.t, rng)
    deltas = {j: [poly_eval(row, j) for row in zero_coeffs] for j in range(1, m.n + 1)}
    for sh in shares:
        for c in range(sh.dim):
            l, r = sh.left[c], sh.right[c]
            while l[0] == 0:  # probability ~2^-127
                l, r = ip_refresh(l, r, rng)
            r = [(r[0] + deltas[sh.index][c] * inv(l[0])) % P] + r[1:]
            sh.left[c], sh.right[c] = ip_refresh(l, r, rng)
        sh.epoch += 1


# --------------------------------------------------------------------------
# Self-test
# --------------------------------------------------------------------------
if __name__ == "__main__":
    assert SplitMix64(0).next_u64() == 0xE220A8397B1DCDAF
    rng = SplitMix64(2026)
    key = bytes(range(32))
    sh = split(key, 3, 5, rng)
    assert all(combine(list(c)) == key for c in combinations(sh, 3))
    refresh_all(sh, rng)
    assert combine(sh[1:4]) == key
    bad = Share.from_bytes(sh[0].to_bytes())
    bad.right[0][0] = (bad.right[0][0] + 1) % P
    assert combine([bad] + sh[1:]) == key
    try:
        combine([bad] + sh[1:3])
        raise SystemExit("tamper not detected")
    except ValueError:
        pass
    print("cairn_ref: all self-tests passed")
