use core::ops::ControlFlow;
use hash_collider::{stat_printer, Collider, HashAdapter};
use rand::Rng;

struct MyHash;
fn my_hash(data: (u64, u64)) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hasher;
    let mut hasher = DefaultHasher::new();
    hasher.write_u64(data.0);
    hasher.write_u64(data.1);
    hasher.finish()
}

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
        assert_eq!(my_hash((0, a)), my_hash((42, b)));
        println!("found collision! (0, {:x?}) (42, {:x?})", a, b);
        ControlFlow::Continue(())
    }
}

fn main() {
    let mut collider = Collider::new(MyHash);
    let thread_count = num_cpus::get();
    collider.run(thread_count, stat_printer(1, 64, ControlFlow::Continue(())));
}
