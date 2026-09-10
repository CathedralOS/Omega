//! Optimizer module role: executable entrance. Block-local live ranges and interference compute -> validation entrance.
//!
//! Replay the liveness prerequisite before the producer inspects it, then keep
//! those exact immutable inputs through independent range verification. Calling
//! the public raw-plan validator here would replay the prerequisite a second
//! time; skipping the first check would move computation ahead of input errors.

use crate::*;

pub(crate) mod compute;
pub(crate) mod model;
pub(crate) mod validate;

pub use model::{LiveRangeError, LiveRangeValidationReceipt, ValidatedLiveRanges};
pub use validate::validate_live_ranges;

#[cfg(test)]
std::thread_local! {
    pub(crate) static LIVENESS_REPLAYS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Derive block-local live-range fragments and virtual-register interference
/// from an exact selected CFG and validated liveness facts.
pub fn analyze_live_ranges<S: ValidatedSelectedAnalysis>(
    selected: &S,
    liveness: &ValidatedLiveness,
) -> Result<ValidatedLiveRanges, LiveRangeError> {
    let inputs = validate::RevalidatedLiveness::new(selected, liveness)?;
    let plan = compute::compute_terminal_live_ranges(selected, liveness)?;
    inputs.validate(plan)
}

mod staging;
pub use staging::*;
