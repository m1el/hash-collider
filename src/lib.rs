mod printer;
mod stats;

use core::fmt::Debug;
use core::hash::Hash;
use core::ops::ControlFlow;
use core::sync::atomic::{AtomicBool, Ordering};
use rand::Rng;
use std::collections::{hash_map::Entry, HashMap};
use std::sync::Mutex;

pub use crate::printer::stat_printer;
use crate::stats::{AtomicStats, Stats};

pub trait StopSignal {
    fn stop(&self);
}

pub trait HashAdapter: Sync {
    type Point: Copy + Eq + Hash + Debug + Send;
    fn make_point<R: Rng>(&self, rng: &mut R) -> Self::Point;
    fn trail_limit(&self) -> u64;
    fn is_distinguishing(&self, x: Self::Point) -> bool;
    fn bifurcation(&self, x: Self::Point) -> bool;
    fn next_point(&self, x: Self::Point, bi: bool) -> Self::Point;
    fn report_collision(&self, a: Self::Point, b: Self::Point) -> ControlFlow<(), ()>;
    fn report_self_collision(&self, _a: Self::Point, _b: Self::Point) -> ControlFlow<(), ()> {
        ControlFlow::Continue(())
    }
}

#[derive(Clone)]
struct TrailInfo<P> {
    start: P,
    length: u64,
}

enum TraceResult<P> {
    GoodCollision(P, P),
    SelfCollision(P, P),
    RobinHood(P),
    NotFound,
}

type TrailRecords<A> =
    HashMap<<A as HashAdapter>::Point, Vec<TrailInfo<<A as HashAdapter>::Point>>>;

pub struct Collider<A: HashAdapter> {
    adapter: A,
    running: AtomicBool,
    stats: AtomicStats,
    /// A map of end point -> [starting points]
    trails: Mutex<TrailRecords<A>>,
}

impl<A: HashAdapter> Collider<A> {
    pub fn new(adapter: A) -> Self {
        Self {
            adapter,
            running: AtomicBool::new(false),
            stats: Default::default(),
            trails: Mutex::new(HashMap::new()),
        }
    }

    pub fn report_stats(&self) -> Stats {
        self.stats.report()
    }

    fn trace_collision(
        adapter: &A,
        a: &TrailInfo<A::Point>,
        b: &TrailInfo<A::Point>,
    ) -> TraceResult<A::Point> {
        let TrailInfo {
            start: mut a,
            length: mut a_len,
        } = a;
        let TrailInfo {
            start: mut b,
            length: mut b_len,
        } = b;

        while b_len > a_len {
            let bifurcation = adapter.bifurcation(b);
            b = adapter.next_point(b, bifurcation);
            b_len -= 1;
        }

        while a_len > b_len {
            let bifurcation = adapter.bifurcation(a);
            a = adapter.next_point(a, bifurcation);
            a_len -= 1;
        }

        if a == b {
            return TraceResult::RobinHood(a);
        }

        for _ in 0..a_len {
            let a_bifurcation = adapter.bifurcation(a);
            let next_a = adapter.next_point(a, a_bifurcation);
            let b_bifurcation = adapter.bifurcation(b);
            let next_b = adapter.next_point(b, b_bifurcation);
            if next_a == next_b {
                return match (a_bifurcation, b_bifurcation) {
                    (false, false) | (true, true) => TraceResult::SelfCollision(a, b),
                    (true, false) => TraceResult::GoodCollision(a, b),
                    (false, true) => TraceResult::GoodCollision(b, a),
                };
            }

            a = next_a;
            b = next_b;
        }

        TraceResult::NotFound
    }

    pub fn run<F: FnOnce(&Self)>(&mut self, count: usize, f: F) {
        std::thread::scope(|s| {
            self.running.store(true, Ordering::Relaxed);

            let threads = (0..count)
                .map(|_| s.spawn(|| self.worker()))
                .collect::<Vec<_>>();

            f(self);

            self.running.store(false, Ordering::Relaxed);

            for t in threads {
                let _ = t.join();
            }
        });
    }

    fn worker(&self) {
        let mut rng = rand::thread_rng();
        let trail_limit = self.adapter.trail_limit();

        'outer: while self.running.load(Ordering::Relaxed) {
            let start = self.adapter.make_point(&mut rng);
            let mut point = start;
            let mut length = 0;

            // Run a trail
            while !self.adapter.is_distinguishing(point) {
                let bifurcation = self.adapter.bifurcation(point);
                point = self.adapter.next_point(point, bifurcation);
                length += 1;
                // The trail is too long, and possibly entered a loop, give up
                if length > trail_limit {
                    self.stats.bailouts.fetch_add(1, Ordering::Relaxed);
                    continue 'outer;
                }
            }

            self.stats.trails.fetch_add(1, Ordering::Relaxed);
            self.stats.hashes.fetch_add(length, Ordering::Relaxed);

            let trail_info = TrailInfo { start, length };
            let mut check_collisions = None;
            let mut trails_lock = if let Ok(lock) = self.trails.try_lock() {
                lock
            } else {
                self.stats.lock_contentions.fetch_add(1, Ordering::Relaxed);
                self.trails
                    .lock()
                    .expect("some other thread has crashed and poisoned a mutex")
            };

            // You ok clippy?  There are no locks inside the match expression.
            #[allow(clippy::significant_drop_in_scrutinee)]
            match trails_lock.entry(point) {
                Entry::Vacant(v) => {
                    v.insert(vec![trail_info.clone()]);
                }
                Entry::Occupied(mut o) => {
                    check_collisions = Some(o.get().clone());
                    o.get_mut().push(trail_info.clone());
                }
            }

            // Release the lock before the next step, since it's CPU-expensive
            core::mem::drop(trails_lock);

            // Find collisions with previous trails.
            if let Some(prev_trails) = check_collisions {
                for previous in prev_trails {
                    match Self::trace_collision(&self.adapter, &previous, &trail_info) {
                        TraceResult::GoodCollision(a, b) => {
                            self.stats.collisions.fetch_add(1, Ordering::Relaxed);
                            if self.adapter.report_collision(a, b).is_break() {
                                self.running.store(false, Ordering::Relaxed);
                            }
                        }
                        TraceResult::SelfCollision(a, b) => {
                            self.stats.self_collisions.fetch_add(1, Ordering::Relaxed);
                            if self.adapter.report_self_collision(a, b).is_break() {
                                self.running.store(false, Ordering::Relaxed);
                            }
                        }
                        TraceResult::RobinHood(_a) => {
                            self.stats.robin_hoods.fetch_add(1, Ordering::Relaxed);
                        }
                        TraceResult::NotFound => {
                            self.stats.errors.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn it_works() {
        struct MyHash;
        fn my_hash(data: (u64, u64)) -> u64 {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::Hasher;
            let mut hasher = DefaultHasher::new();
            hasher.write_u64(data.0);
            hasher.write_u64(data.1);
            // truncate the hash value to 42 bits for faster test
            hasher.finish() & !(!0 << 42)
        }

        assert!(my_hash((0, 0xedcb60beda96782b)) == my_hash((42, 0x9ecd6bc1caefa5f4)));

        impl HashAdapter for MyHash {
            type Point = u64;

            fn trail_limit(&self) -> u64 {
                0x3ffff * 20
            }

            fn make_point<R: Rng>(&self, rng: &mut R) -> Self::Point {
                rng.next_u64()
            }

            fn is_distinguishing(&self, x: Self::Point) -> bool {
                x & 0x3ffff == 0
            }

            fn bifurcation(&self, x: Self::Point) -> bool {
                x & 1 != 0
            }

            fn next_point(&self, x: Self::Point, bi: bool) -> Self::Point {
                let prefix = if bi { 0 } else { 42 };
                my_hash((prefix, x))
            }

            fn report_collision(&self, a: Self::Point, b: Self::Point) -> ControlFlow<(), ()> {
                println!("found collision! {:x?} {:x?}", a, b);
                ControlFlow::Break(())
            }
        }

        let mut collider = Collider::new(MyHash);
        let thread_count = num_cpus::get();
        collider.run(thread_count, stat_printer(1, 42, ControlFlow::Break(())));
    }
}
