//! Exact binary32 payload transport in a binary64 storage cell.
//!
//! This is representation transport, not IEEE arithmetic or FloatMeaning.
//! NaN sign, quiet bit and payload survive without a host float conversion.

pub fn binary32_from_bits(bits: u32) -> f64 {
    let value = f32::from_bits(bits);
    if !value.is_nan() {
        return value as f64;
    }
    let sign = (u64::from(bits) >> 31) << 63;
    let payload = (u64::from(bits) & 0x007f_ffff) << 29;
    f64::from_bits(sign | 0x7ff0_0000_0000_0000 | payload)
}

pub fn binary32_to_bits(value: f64) -> u32 {
    if !value.is_nan() {
        return (value as f32).to_bits();
    }
    let bits = value.to_bits();
    let sign = ((bits >> 63) as u32) << 31;
    let mut payload = ((bits & 0x000f_ffff_ffff_ffff) >> 29) as u32;
    payload &= 0x007f_ffff;
    if payload == 0 {
        payload = 0x0040_0000;
    }
    sign | 0x7f80_0000 | payload
}
