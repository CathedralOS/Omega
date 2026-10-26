//! The actual host C `abs` operation. No guest type or claim lives here.

#[cfg(any(unix, windows))]
#[cfg_attr(windows, link(name = "msvcrt"))]
unsafe extern "C" {
    #[link_name = "abs"]
    fn c_abs(value: i32) -> i32;
}

pub fn host_integer_absolute(value: i32) -> Option<i32> {
    // C abs is undefined when the mathematical result does not fit int.
    if value == i32::MIN {
        return None;
    }
    #[cfg(any(unix, windows))]
    {
        // SAFETY: supported hosted C ABIs use signed 32-bit int. The input
        // check excludes the only undefined value; abs retains no storage.
        Some(unsafe { c_abs(value) })
    }
    #[cfg(not(any(unix, windows)))]
    None
}

#[cfg(test)]
mod tests {
    use super::host_integer_absolute;

    #[test]
    fn actual_host_absolute_and_undefined_boundary() {
        assert_eq!(host_integer_absolute(-37), Some(37));
        assert_eq!(host_integer_absolute(0), Some(0));
        assert_eq!(host_integer_absolute(i32::MAX), Some(i32::MAX));
        assert_eq!(host_integer_absolute(i32::MIN), None);
    }
}
