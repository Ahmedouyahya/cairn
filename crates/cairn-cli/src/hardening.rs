//! Process hardening. This is the only module in CAIRN that uses `unsafe`,
//! and every block is a single libc call on memory we own.
//!
//! * `PR_SET_DUMPABLE = 0`: no core dumps, and other processes of the same
//!   user can no longer `ptrace` us or read `/proc/<pid>/mem`.
//! * `RLIMIT_CORE = 0`: belt and braces for core dumps.
//! * `mlockall(MCL_CURRENT | MCL_FUTURE)` when the memory-lock limit allows
//!   it, so no page holding a share or the key is ever written to swap.
//!   Otherwise each secret buffer is locked individually by [`LockedBytes`].

use zeroize::Zeroize;

/// Outcome of [`harden_process`], reported by `cairn --verbose`.
#[derive(Debug, Clone, Copy)]
pub struct Hardening {
    pub no_dump: bool,
    pub no_core: bool,
    pub all_memory_locked: bool,
}

/// Apply every hardening step that the environment allows.
pub fn harden_process() -> Hardening {
    // SAFETY: prctl with PR_SET_DUMPABLE takes integer arguments only.
    let no_dump = unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) } == 0;
    let zero = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
    // SAFETY: `zero` is a valid rlimit that outlives the call.
    let no_core = unsafe { libc::setrlimit(libc::RLIMIT_CORE, &zero) } == 0;

    let mut lim = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
    // SAFETY: `lim` is a valid, writable rlimit.
    let have = unsafe { libc::getrlimit(libc::RLIMIT_MEMLOCK, &mut lim) } == 0;
    // MCL_FUTURE makes every later allocation fail once the limit is hit, so
    // only use it when the limit is generous (unlimited or >= 256 MiB).
    let generous = have && (lim.rlim_cur == libc::RLIM_INFINITY || lim.rlim_cur >= 256 << 20);
    // SAFETY: flags-only call.
    let all_memory_locked =
        generous && unsafe { libc::mlockall(libc::MCL_CURRENT | libc::MCL_FUTURE) } == 0;
    Hardening { no_dump, no_core, all_memory_locked }
}

/// A byte buffer that is locked in RAM (best effort) and zeroized on drop.
///
/// The capacity is fixed at construction so the allocation never moves:
/// a moved `Vec` would leave an unzeroized copy of the secret behind.
pub struct LockedBytes {
    buf: Vec<u8>,
    locked: bool,
}

impl LockedBytes {
    pub fn with_capacity(cap: usize) -> LockedBytes {
        let buf: Vec<u8> = Vec::with_capacity(cap.max(1));
        // SAFETY: the range is exactly the allocation owned by `buf`.
        let locked = unsafe { libc::mlock(buf.as_ptr().cast(), buf.capacity()) } == 0;
        LockedBytes { buf, locked }
    }

    /// Append bytes. Panics instead of reallocating.
    pub fn extend(&mut self, data: &[u8]) {
        assert!(self.buf.len() + data.len() <= self.buf.capacity(), "locked buffer full");
        self.buf.extend_from_slice(data);
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.buf
    }

    pub fn spare_capacity(&self) -> usize {
        self.buf.capacity() - self.buf.len()
    }
}

impl Drop for LockedBytes {
    fn drop(&mut self) {
        let cap = self.buf.capacity();
        // Zeroize the whole allocation, not just the initialised prefix.
        self.buf.resize(cap, 0);
        self.buf.zeroize();
        if self.locked {
            // SAFETY: same range that was locked in `with_capacity`.
            unsafe { libc::munlock(self.buf.as_ptr().cast(), cap) };
        }
    }
}
