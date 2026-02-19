use std::cell::Cell;

use super::{format_number, to_number};

thread_local! {
    static RNG_STATE: Cell<u64> = const { Cell::new(0) };
    static RNG_SEEDED: Cell<bool> = const { Cell::new(false) };
}

fn splitmix64(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9e3779b97f4a7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

pub fn rng_next() -> f64 {
    RNG_STATE.with(|state| {
        RNG_SEEDED.with(|seeded| {
            if !seeded.get() {
                let seed = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos() as u64;
                state.set(splitmix64(seed));
                seeded.set(true);
            }
        });
        let mut s = state.get();
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        state.set(s);
        (s >> 11) as f64 / (1u64 << 53) as f64
    })
}

/// Dispatch math built-in functions.
pub fn call(name: &str, args: &[String]) -> String {
    let n = || args.first().map(|s| to_number(s)).unwrap_or(0.0);
    let n2 = || args.get(1).map(|s| to_number(s)).unwrap_or(0.0);
    match name {
        "int" => format_number(n().trunc()),
        "sin" => format!("{:.6}", n().sin()),
        "cos" => format!("{:.6}", n().cos()),
        "sqrt" => format_number(n().sqrt()),
        "log" => format!("{:.6}", n().ln()),
        "exp" => format!("{:.6}", n().exp()),
        "atan2" => format!("{:.6}", n().atan2(n2())),
        "abs" => format_number(n().abs()),
        "ceil" => format_number(n().ceil()),
        "floor" => format_number(n().floor()),
        "round" => format_number(n().round()),
        "log2" => format!("{:.6}", n().log2()),
        "log10" => format!("{:.6}", n().log10()),
        "min" => format_number(n().min(n2())),
        "max" => format_number(n().max(n2())),
        "rand" => format_number(rng_next()),
        "srand" => {
            let prev = RNG_STATE.with(|s| s.get() as f64);
            let seed = if args.is_empty() {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos() as u64
            } else {
                n() as u64
            };
            RNG_STATE.with(|s| s.set(splitmix64(seed)));
            RNG_SEEDED.with(|s| s.set(true));
            format_number(prev)
        }
        _ => String::new(),
    }
}
