use core::ops::{ControlFlow};
use crate::{Collider, HashAdapter};

pub fn stat_printer<A: HashAdapter>(interval: u64, on_found: ControlFlow<(), ()>) -> impl Fn(&Collider<A>) {
    move |collider| {
        println!("{t:>9} {h:>14} {hps:>9} {et:>6} {c:>5} {rh:>5} {s:>5} {bo:>5} {l:>6} {e:>5}",
            t="trails", h="hashes", hps="mh/s", et="ETA", c="coll",
            rh="rh", s="self", bo="bail", l="locc", e="err",
        );
        let start = std::time::Instant::now();
        let mut prev_t = start;
        let mut prev_h = 0;

        loop {
            std::thread::sleep(std::time::Duration::from_secs(interval));
            let now = std::time::Instant::now();
            let stats = collider.report_stats();
            let hps = (stats.hashes - prev_h) as f64
                / (now - prev_t).as_secs_f64();
            prev_t = now;
            prev_h = stats.hashes;

            let expected_time = stats.estimate_time_to_hash(64, start.elapsed().as_secs_f64());

            println!("{t:>9} {h:>14} {hps:>9.2} {et:>6.1} {c:>5} {rh:>5} {s:>5} {bo:>5} {l:>6} {e:>5}",
                t=stats.trails, h=stats.hashes, hps=hps/1e6, et=expected_time,
                c=stats.collisions, rh=stats.robin_hoods,
                s=stats.self_collisions, bo=stats.bailouts,
                l=stats.lock_contentions, e=stats.errors,
            );

            if on_found.is_break() && stats.collisions > 0 {
                break;
            }
        }
    }
}
