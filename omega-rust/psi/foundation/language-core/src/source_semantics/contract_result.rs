//! The reserved `result` binder of a machine's normal-return guarantees.
//!
//! In a top-level machine's `ensures`, `result` denotes the value the machine
//! produces on normal completion unless a real entry parameter named `result`
//! shadows it (`wiki/spec/terminal-psi/mathematical_values.md`). The
//! occurrence belongs to its contract owner, never to a local or parameter
//! spelled the same way. This module is the only place that spelling lives:
//! producers that emit it and stages holding only rendered text use
//! [`RESERVED_RESULT`] and [`is_reserved_result`]; a stage holding resolved
//! trees asks its representation's resolution-aware query instead, which also
//! sees a shadowing parameter.

/// The spelling of the reserved result binder.
pub const RESERVED_RESULT: &str = "result";

/// Whether `name` spells the reserved result binder.
///
/// This is a spelling test, not a resolution: an entry parameter named
/// `result` shadows the binder, so a resolved occurrence of this spelling may
/// name that parameter instead.
pub fn is_reserved_result(name: &str) -> bool {
    name == RESERVED_RESULT
}

/// Whether a signature whose entry parameters are spelled `parameter_names`
/// keeps `result` reserved for its normal-return value: no entry parameter
/// shadows it.
pub fn signature_reserves_result<'name>(
    mut parameter_names: impl Iterator<Item = &'name str>,
) -> bool {
    !parameter_names.any(is_reserved_result)
}
