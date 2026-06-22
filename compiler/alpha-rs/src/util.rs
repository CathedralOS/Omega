// Small shared helpers.

pub fn align_up(v: u32, a: u32) -> u32 {
    (v + a - 1) / a * a
}
