//! Optimizer module role: executable entrance. Block-local live ranges and interference compute -> validation entrance.

use crate::*;

pub(crate) mod compute;
pub(crate) mod identity;
pub(crate) mod model;
pub(crate) mod validate;

pub use identity::live_range_identity;
pub use model::*;
pub use validate::validate_live_ranges;

/// Derive block-local live-range fragments and virtual-register interference
/// from an exact selected CFG and validated liveness facts.
pub fn analyze_live_ranges<S: ValidatedSelectedAnalysis>(
    selected: &S,
    liveness: &ValidatedLiveness,
) -> Result<ValidatedLiveRanges, LiveRangeError> {
    validate::revalidate_liveness_custody(selected, liveness)?;
    let plan = compute::compute_terminal_live_ranges(selected, liveness)?;
    validate_live_ranges(selected, liveness, plan)
}

mod staging;
pub use staging::*;
