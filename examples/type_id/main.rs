#![feature(hasher_prefixfree_extras, maybe_uninit_uninit_array)]

mod sip128;
mod stable_hasher;

use crate::stable_hasher::{HashStable, HashingControls, StableHasher};
use core::ops::ControlFlow;
use hash_collider::{stat_printer, Collider, HashAdapter};
use rand::Rng;
use std::hash::Hasher;

const IN_PLAYGROUND_WRAPPER: bool = true;
// const I_WANT_TO_DEBUG_DEF_ID: bool = false;
const DEFAULT_TRAIL_MASK: u64 = 0x3ffff;

fn hash_of<T: HashStable<CTX>, CTX>(hcx: &mut CTX, val: T) -> (u64, u64) {
    let mut hasher = StableHasher::new();
    val.hash_stable(hcx, &mut hasher);
    hasher.finalize()
}

fn make_mod_id(
    crate_name: &str,
    is_exe: bool,
    version: &str,
    mut metadata: Vec<String>,
) -> (u64, u64) {
    let mut hcx = HashingControls { hash_spans: false };
    let mut hasher = StableHasher::new();
    hasher.write_str(crate_name);
    metadata.sort();
    metadata.dedup();
    hasher.write(b"metadata");
    for s in &metadata {
        hasher.write_usize(s.len());
        hasher.write(s.as_bytes());
    }

    hasher.write(if is_exe { b"exe" } else { b"lib" });
    hasher.write(version.as_bytes());

    let crate_id = hasher.finalize().0;
    let mod_id = hash_of(&mut hcx, (crate_id, 0_u64, 0_isize, 0_u32)).0;
    if IN_PLAYGROUND_WRAPPER {
        let mut hasher = StableHasher::new();
        (crate_id, mod_id).hash_stable(&mut hcx, &mut hasher);
        hasher.write_isize(6); // discriminator
        hasher.write_str("main");
        hasher.write_u32(0);
        let main_id = hasher.finalize().0;
        (crate_id, main_id)
    } else {
        (crate_id, mod_id)
    }
}

#[allow(dead_code)]
fn type_id_of_struct(mod_id: (u64, u64), name: &str, field: &str) -> u64 {
    let mut hcx = HashingControls { hash_spans: false };
    let crate_id = mod_id.0;
    let struct_did = {
        let mut hasher = StableHasher::new();
        mod_id.hash_stable(&mut hcx, &mut hasher);
        hasher.write_isize(5); // discriminator
        hasher.write_str(name);
        hasher.write_u32(0);
        (crate_id, hasher.finalize().0)
    };
    let field_shuffle_seed = struct_did.0.wrapping_mul(3).wrapping_add(struct_did.1);
    let field_did = {
        let mut hasher = StableHasher::new();
        struct_did.hash_stable(&mut hcx, &mut hasher);
        hasher.write_isize(6); // discriminator
        hasher.write_str(field);
        hasher.write_u32(0);
        (crate_id, hasher.finalize().0)
    };
    // println!("struct_did={:x?}", struct_did);
    // println!("field_did={:x?}", field_did);
    let adt_hash = {
        let mut hasher = StableHasher::new();
        // DefId
        struct_did.hash_stable(&mut hcx, &mut hasher);
        hasher.write_usize(1);
        struct_did.hash_stable(&mut hcx, &mut hasher);
        hasher.write_u8(0);
        name.as_bytes().hash_stable(&mut hcx, &mut hasher);
        // Variants
        hasher.write_isize(1);
        hasher.write_u32(0);
        hasher.write_usize(1);
        field_did.hash_stable(&mut hcx, &mut hasher);
        hasher.write_usize(field.len());
        hasher.write(field.as_bytes());
        // visibility
        hasher.write_isize(0);
        // scope of visibility
        // mod_id.hash_stable(&mut hcx, &mut hasher);
        hasher.write_isize(2);
        hasher.write_u32(0);
        // AdtFlags
        hasher.write_u32(4);
        // ReprOptions
        hasher.write_u8(0);
        hasher.write_u8(0);
        hasher.write_u8(0);
        hasher.write_u8(0);
        hasher.write_u64(field_shuffle_seed);
        hasher.finalize()
    };
    let substs_hash = hash_of(&mut hcx, 0_usize);
    let ty_hash = hash_of(&mut hcx, (5_isize, adt_hash, substs_hash));
    hash_of(&mut hcx, ty_hash).0
}

#[allow(dead_code)]
fn field_did(mod_id: (u64, u64), name: &str, field: &str) -> u64 {
    let mut hcx = HashingControls { hash_spans: false };
    let crate_id = mod_id.0;
    let struct_did = {
        let mut hasher = StableHasher::new();
        mod_id.hash_stable(&mut hcx, &mut hasher);
        hasher.write_isize(5); // discriminator
        hasher.write_str(name);
        hasher.write_u32(0);
        (crate_id, hasher.finalize().0)
    };
    let field_did = {
        let mut hasher = StableHasher::new();
        struct_did.hash_stable(&mut hcx, &mut hasher);
        hasher.write_isize(6); // discriminator
        hasher.write_str(field);
        hasher.write_u32(0);
        (crate_id, hasher.finalize().0)
    };
    field_did.1
}

fn write_hex(dst: &mut [u8], value: u64) {
    fn hex(x: u64) -> u8 {
        assert!(x <= 15, "YOU LIED");
        (x as u8) + if x <= 9 { b'0' } else { b'a' - 10 }
    }
    for ii in 0..8 {
        let byte = value >> (64 - (ii + 1) * 8);
        let lo = hex(byte & 0xf);
        let hi = hex((byte & 0xf0) >> 4);
        dst[ii * 2] = hi;
        dst[ii * 2 + 1] = lo;
    }
}

struct TypeIdHash {
    mod_id: (u64, u64),
}
impl TypeIdHash {
    fn new(name: &str, is_exe: bool, version: &str, metadata: Vec<String>) -> Self {
        Self {
            mod_id: make_mod_id(name, is_exe, version, metadata),
        }
    }
}

impl HashAdapter for TypeIdHash {
    type Point = u64;

    fn trail_limit(&self) -> u64 {
        DEFAULT_TRAIL_MASK * 20
    }

    fn make_point<R: Rng>(&self, rng: &mut R) -> Self::Point {
        rng.next_u64()
    }

    fn is_distinguishing(&self, x: Self::Point) -> bool {
        x & DEFAULT_TRAIL_MASK == 0
    }

    fn bifurcation(&self, x: Self::Point) -> bool {
        x & 1 != 0
    }

    fn next_point(&self, x: Self::Point, bi: bool) -> Self::Point {
        let name = if bi { "Foo" } else { "Bar" };

        let mut data = *b"x0000000000000000";
        write_hex(&mut data[1..], x);

        let field_name = core::str::from_utf8(&data[..]).unwrap();
        // println!("field_name: {}", field_name);
        type_id_of_struct(self.mod_id, name, field_name)
        // field_did(name, field_name)
    }

    fn report_collision(&self, a: Self::Point, b: Self::Point) -> ControlFlow<(), ()> {
        println!(
            "found collision! Foo {{ x{:016x?}: usize }} Bar {{ x{:x?}: usize }}",
            a, b
        );
        ControlFlow::Continue(())
    }
}

fn main() {
    let hash = TypeIdHash::new(
        "playground",
        true,
        "1.64.0-nightly (7665c3543 2022-07-06)",
        vec!["a0ecb98bfb1b38c8".to_owned()],
    );
    let thread_count = num_cpus::get();
    Collider::new(hash).run(thread_count, stat_printer(1, 64, ControlFlow::Continue(())));
}
