//! Actual C long-long division, retaining its complete two-field return ABI.
//! This leaf owns host calling geometry, not guest types, facts or identities.

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
    // C does not define division by zero or an unrepresentable quotient.
    // Refuse before the host call rather than wrap or fabricate a C result.
    if denominator == 0 || (numerator == i64::MIN && denominator == -1) {
        return None;
    }
    #[cfg(any(unix, windows))]
    {
        // SAFETY: both arguments have C long long's exact 64-bit type. repr(C)
        // retains the ordered quotient/remainder pair and the host's return ABI.
        // lldiv neither borrows nor retains storage. The checks above exclude UB.
        Some(unsafe { lldiv(numerator, denominator) })
    }
    #[cfg(not(any(unix, windows)))]
    None
}

#[cfg(test)]
mod tests {
    use super::*;

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
