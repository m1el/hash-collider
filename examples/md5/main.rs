//! demonstrating 96-bit collision on md5
mod md5;

use core::ops::ControlFlow;
use hash_collider::{stat_printer, Collider, HashAdapter};
use md5::compress;
use rand::Rng;

static TRAIL_MASK: u32 = 0xfffff;

struct MyHash {
    ihv_a: [u32; 4],
    ihv_b: [u32; 4],
}
impl MyHash {
    fn new() -> Self {
        Self {
            ihv_a: [0x266e2670, 0x9b8a1b87, 0x923fd523, 0x8c4fcf12],
            ihv_b: [0xa0da787b, 0xb3cb406c, 0xfe644118, 0xd7c59003],
        }
    }
}

impl HashAdapter for MyHash {
    type Point = [u32; 3];

    fn trail_limit(&self) -> u64 {
        TRAIL_MASK as u64 * 20
    }

    fn make_point<R: Rng>(&self, rng: &mut R) -> Self::Point {
        [rng.next_u32(), rng.next_u32(), rng.next_u32()]
    }

    fn is_distinguishing(&self, x: Self::Point) -> bool {
        x[0] & TRAIL_MASK == 0
    }

    fn bifurcation(&self, state: Self::Point) -> bool {
        state[0] < state[1]
    }

    fn next_point(&self, state: Self::Point, bi: bool) -> Self::Point {
        let mut ihv = if bi { self.ihv_a } else { self.ihv_b };
        let mut data = [0_u32; 16];
        /*
        let mut hex_state = [0_u8; 4 * 3 * 2];
        fn hex(x: u32) -> u8 {
            assert!(x <= 15, "YOU LIED");
            (x as u8) + if x <= 9 { b'0' } else { b'A' - 10 }
        }
        for (si, sb) in state.iter().cloned().enumerate() {
            for ii in 0..4 {
                let byte = sb >> ii * 8;
                let lo = hex(byte & 0xf);
                let hi = hex((byte & 0xf0) >> 4);
                hex_state[si * 8 + ii * 2] = lo;
                hex_state[si * 8 + ii * 2 + 1] = hi;
            }
        }
        let hex_state = unsafe { core::mem::transmute::<_, [u32; 6]>(hex_state) };

        data[10..].copy_from_slice(&hex_state);
        */
        // println!("ihv={:x?} data={:x?}", ihv, data);
        data[13..].copy_from_slice(&state);
        compress(&mut ihv, &data);
        ihv[..3].try_into().unwrap()
    }

    fn report_collision(&self, a: Self::Point, b: Self::Point) -> ControlFlow<(), ()> {
        println!("found collision! (0, {:x?}) (42, {:x?})", a, b);
        ControlFlow::Continue(())
    }
}

fn main() {
    let mut collider = Collider::new(MyHash::new());
    let thread_count = num_cpus::get();
    collider.run(thread_count, stat_printer(1, 96, ControlFlow::Continue(())));
}
