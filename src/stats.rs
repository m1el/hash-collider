use core::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
pub(crate) struct AtomicStats {
    pub(crate) trails: AtomicU64,
    pub(crate) hashes: AtomicU64,
    pub(crate) collisions: AtomicU64,
    pub(crate) robin_hoods: AtomicU64,
    pub(crate) self_collisions: AtomicU64,
    pub(crate) bailouts: AtomicU64,
    pub(crate) errors: AtomicU64,
    pub(crate) lock_contentions: AtomicU64,
}

impl AtomicStats {
    pub(crate) fn report(&self) -> Stats {
        Stats {
            trails: self.trails.load(Ordering::Relaxed),
            hashes: self.hashes.load(Ordering::Relaxed),
            collisions: self.collisions.load(Ordering::Relaxed),
            robin_hoods: self.robin_hoods.load(Ordering::Relaxed),
            self_collisions: self.self_collisions.load(Ordering::Relaxed),
            bailouts: self.bailouts.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
            lock_contentions: self.lock_contentions.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug)]
pub struct Stats {
    pub trails: u64,
    pub hashes: u64,
    pub collisions: u64,
    pub robin_hoods: u64,
    pub self_collisions: u64,
    pub bailouts: u64,
    pub errors: u64,
    pub lock_contentions: u64,
}

impl Stats {
    pub fn estimate_time_to_hash(&self, bits: u8, elapsed_secs: f64) -> f64 {
        // (h + hps*t)^2/2 - h^2/2 = s
        // (h + hps*t)^2 - h^2 = 2*s
        // h + hps*t = q(2*s + h^2)
        // hps*t = q(2*s + h^2) - h
        // t = (q(2*s + h^2) - h) / hps
        let search_space = 2.0_f64.powi(bits as i32);
        let h = self.hashes as f64;
        let hps = h / elapsed_secs;
        ((2.0 * search_space + h * h).sqrt() - h) * 2.0 / hps
    }
}
