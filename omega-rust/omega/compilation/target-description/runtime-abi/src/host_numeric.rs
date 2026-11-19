/*
We implement the host C numeric, parsing, and CoreGraphics affine-transform
leaves consumed by Terminal execution (`host_affine_transform`,
`host_integer_absolute`, `host_integer_division`, `parse_host_long`).

Each leaf checks C's undefined-behavior domain boundaries (`i32::MIN` on `abs`,
zero divisor or `i64::MIN / -1` on `lldiv`, non-NUL-terminated input or out-of-range
radix on `strtol`) before invoking the host ABI, and returns plain value
results or byte offsets without exposing guest pointers or storage authority.
*/

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostAffineTransform {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub tx: f64,
    pub ty: f64,
}

#[cfg(target_os = "macos")]
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    #[link_name = "CGAffineTransformMake"]
    fn cg_affine_transform_make(
        a: f64,
        b: f64,
        c: f64,
        d: f64,
        tx: f64,
        ty: f64,
    ) -> HostAffineTransform;
}

pub fn host_affine_transform(components: [f64; 6]) -> Option<HostAffineTransform> {
    #[cfg(target_os = "macos")]
    {
        let [a, b, c, d, tx, ty] = components;
        Some(unsafe { cg_affine_transform_make(a, b, c, d, tx, ty) })
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = components;
        None
    }
}

#[cfg(any(unix, windows))]
#[cfg_attr(windows, link(name = "msvcrt"))]
unsafe extern "C" {
    #[link_name = "abs"]
    fn c_abs(value: i32) -> i32;
}

pub fn host_integer_absolute(value: i32) -> Option<i32> {
    if value == i32::MIN {
        return None;
    }
    #[cfg(any(unix, windows))]
    {
        Some(unsafe { c_abs(value) })
    }
    #[cfg(not(any(unix, windows)))]
    None
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostIntegerDivision {
    pub quotient: i64,
    pub remainder: i64,
}

#[cfg(any(unix, windows))]
#[cfg_attr(all(windows, target_env = "msvc"), link(name = "ucrt"))]
unsafe extern "C" {
    fn lldiv(numerator: i64, denominator: i64) -> HostIntegerDivision;
}

pub fn host_integer_division(numerator: i64, denominator: i64) -> Option<HostIntegerDivision> {
    if denominator == 0 || (numerator == i64::MIN && denominator == -1) {
        return None;
    }
    #[cfg(any(unix, windows))]
    {
        Some(unsafe { lldiv(numerator, denominator) })
    }
    #[cfg(not(any(unix, windows)))]
    None
}

#[cfg(all(unix, target_pointer_width = "64"))]
unsafe extern "C" {
    fn strtol(
        input: *const std::ffi::c_char,
        end: *mut *mut std::ffi::c_char,
        base: std::ffi::c_int,
    ) -> std::ffi::c_long;
}

pub fn parse_host_long(bytes: &[u8], base: i32) -> Option<(i64, usize)> {
    if bytes.last() != Some(&0) || !(base == 0 || (2..=36).contains(&base)) {
        return None;
    }
    #[cfg(all(unix, target_pointer_width = "64"))]
    {
        let mut end = std::ptr::null_mut();
        let value = unsafe { strtol(bytes.as_ptr().cast(), &mut end, base) };
        let offset = end.addr().checked_sub(bytes.as_ptr().addr())?;
        if offset >= bytes.len() {
            return None;
        }
        Some((value, offset))
    }
    #[cfg(not(all(unix, target_pointer_width = "64")))]
    {
        let _ = base;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn actual_c_indirect_result_retains_six_fields_and_earlier_call() {
        assert_eq!(std::mem::size_of::<HostAffineTransform>(), 48);
        assert_eq!(std::mem::align_of::<HostAffineTransform>(), 8);
        let first = host_affine_transform([1.25, -2.5, 3.75, -4.0, 5.5, -6.25]).unwrap();
        let second = host_affine_transform([-11.0, 12.5, -13.25, 14.75, -15.5, 16.0]).unwrap();
        assert_eq!(
            first,
            HostAffineTransform {
                a: 1.25,
                b: -2.5,
                c: 3.75,
                d: -4.0,
                tx: 5.5,
                ty: -6.25
            }
        );
        assert_eq!(
            second,
            HostAffineTransform {
                a: -11.0,
                b: 12.5,
                c: -13.25,
                d: 14.75,
                tx: -15.5,
                ty: 16.0
            }
        );
    }

    #[test]
    fn actual_host_absolute_and_undefined_boundary() {
        assert_eq!(host_integer_absolute(-37), Some(37));
        assert_eq!(host_integer_absolute(0), Some(0));
        assert_eq!(host_integer_absolute(i32::MAX), Some(i32::MAX));
        assert_eq!(host_integer_absolute(i32::MIN), None);
    }

    #[test]
    fn actual_c_results_preserve_signed_fields_and_distinct_calls() {
        assert_eq!(
            host_integer_division(-70_000_000_000, 3),
            Some(HostIntegerDivision {
                quotient: -23_333_333_333,
                remainder: -1,
            })
        );
        assert_eq!(
            host_integer_division(70_000_000_000, 9),
            Some(HostIntegerDivision {
                quotient: 7_777_777_777,
                remainder: 7,
            })
        );
    }

    #[test]
    fn undefined_c_inputs_never_reach_the_host() {
        assert_eq!(host_integer_division(1, 0), None);
        assert_eq!(host_integer_division(i64::MIN, -1), None);
    }
}
