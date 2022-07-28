use core::ops::ControlFlow;
use hash_collider::{stat_printer, Collider, HashAdapter};
use rand::Rng;
use core::hash::Hasher;
use ahash::AHasher;

struct MyHash {
    prefix_a: AHasher,
    prefix_b: AHasher,
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
        let mut hasher = if bi {
            self.prefix_a.clone()
        } else {
            self.prefix_b.clone()
        };
        hasher.write_u64(x);
        hasher.finish()
    }

    fn report_collision(&self, a: Self::Point, b: Self::Point) -> ControlFlow<(), ()> {
        // assert_eq!(my_hash((0, a)), my_hash((42, b)));
        println!("found collision! (0, {:x?}) (42, {:x?})", a, b);
        ControlFlow::Continue(())
    }
}

fn main() {
    let mut prefix_a = AHasher::new_with_keys(1234, 5678);
    prefix_a.write_u64(0);
    let mut prefix_b = AHasher::new_with_keys(1234, 5678);
    prefix_b.write_u64(42);
    let mut collider = Collider::new(MyHash { prefix_a, prefix_b });
    let thread_count = num_cpus::get();
    collider.run(thread_count, stat_printer(1, 64, ControlFlow::Continue(())));
}
