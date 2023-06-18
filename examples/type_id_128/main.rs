#![feature(specialization, hasher_prefixfree_extras, maybe_uninit_uninit_array)]

mod sip128;
mod stable_hasher;

use crate::stable_hasher::{HashStable, HashingControls, StableHasher};
use core::ops::ControlFlow;
use hash_collider::{stat_printer, Collider, HashAdapter};
use rand::Rng;
use std::hash::Hasher;

const IN_PLAYGROUND_WRAPPER: bool = false;
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
    let root_mod_id = hash_of(&mut hcx, (crate_id, 0_u64, 0_isize, 0_u32)).0;
    if IN_PLAYGROUND_WRAPPER {
        let mut hasher = StableHasher::new();
        (crate_id, root_mod_id).hash_stable(&mut hcx, &mut hasher);
        hasher.write_isize(6); // discriminator
        hasher.write_str("main");
        hasher.write_u32(0);
        let main_id = hasher.finalize().0;
        (crate_id, main_id)
    } else {
        (crate_id, root_mod_id)
    }
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

#[derive(Debug)]
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

fn type_id_of_mod(parent: (u64, u64), name: &str) -> u64 {
    let mut hcx = HashingControls { hash_spans: false };
    let mut hasher = StableHasher::new();
    parent.hash_stable(&mut hcx, &mut hasher);
    hasher.write_isize(5); // discriminator
    hasher.write_str(name);
    hasher.write_u32(0);
    hasher.finalize().0
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
        let name = if bi { "foo" } else { "bar" };

        let mut data = *b"foo_0000000000000000";
        data[..3].copy_from_slice(name.as_bytes());
        write_hex(&mut data[4..], x);

        let mod_name = core::str::from_utf8(&data[..]).unwrap();
        // println!("field_name: {}", field_name);
        type_id_of_mod(self.mod_id, mod_name)
        // field_did(name, field_name)
    }

    fn report_collision(&self, a: Self::Point, b: Self::Point) -> ControlFlow<(), ()> {
        println!(
            "found collision!
            mod foo_{:016x} {{ struct Foo {{ }} }}
            mod bar_{:016x} {{ struct Foo {{ }} }}",
            a, b
        );
        ControlFlow::Continue(())
    }
}

fn main() {
    let hash = TypeIdHash::new(
        "playground",
        true,
        "1.72.0-dev",
        vec!["051dac071847dbb3".to_owned()]
    );
    println!("hash: {:?}", hash);
    let mut collider = Collider::new(hash);
    let thread_count = num_cpus::get();
    collider.run(thread_count, stat_printer(1, 64, ControlFlow::Continue(())));
}

// {"rustc_fingerprint":8333503651157263990,
// "outputs":{"4614504638168534921":{"success":true,"status":"","code":0,"stdout":
// "rustc 1.72.0-nightly (3b2073f07 2023-06-17)\nbinary: rustc\ncommit-hash: 3b2073f0762cff4d3d625bb10017e0ce4e7abe50\ncommit-date: 2023-06-17\nhost: x86_64-unknown-linux-gnu\nrelease: 1.72.0-nightly\nLLVM version: 16.0.5\n","stderr":""},"15729799797837862367":{"success":true,"status":"","code":0,"stdout":"___\nlib___.rlib\nlib___.so\nlib___.so\nlib___.a\nlib___.so\n/playground/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu\noff\npacked\nunpacked\n___\ndebug_assertions\noverflow_checks\npanic=\"unwind\"\nproc_macro\ntarget_abi=\"\"\ntarget_arch=\"x86_64\"\ntarget_endian=\"little\"\ntarget_env=\"gnu\"\ntarget_family=\"unix\"\ntarget_feature=\"fxsr\"\ntarget_feature=\"sse\"\ntarget_feature=\"sse2\"\ntarget_has_atomic\ntarget_has_atomic=\"16\"\ntarget_has_atomic=\"32\"\ntarget_has_atomic=\"64\"\ntarget_has_atomic=\"8\"\ntarget_has_atomic=\"ptr\"\ntarget_has_atomic_equal_alignment=\"16\"\ntarget_has_atomic_equal_alignment=\"32\"\ntarget_has_atomic_equal_alignment=\"64\"\ntarget_has_atomic_equal_alignment=\"8\"\ntarget_has_atomic_equal_alignment=\"ptr\"\ntarget_has_atomic_load_store\ntarget_has_atomic_load_store=\"16\"\ntarget_has_atomic_load_store=\"32\"\ntarget_has_atomic_load_store=\"64\"\ntarget_has_atomic_load_store=\"8\"\ntarget_has_atomic_load_store=\"ptr\"\ntarget_os=\"linux\"\ntarget_pointer_width=\"64\"\ntarget_thread_local\ntarget_vendor=\"unknown\"\nunix\n","stderr":""}},"successes":{}}target/release/deps/playground-051dac071847dbb3
// target/release/deps/playground-051dac071847dbb3.d
