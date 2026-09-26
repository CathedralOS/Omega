//! Canonical text vocabulary shared with bounded recovery.

pub(in crate::encoding) const MAXIMUM_MARKUP_DEPTH: usize = 1024;
pub(in crate::encoding) const HEADER: &str = "omega_package_policy_text 1\n";

pub(in crate::encoding) fn label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.as_bytes()[0].is_ascii_lowercase()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}
