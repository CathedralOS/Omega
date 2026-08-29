//! Native-fuel instrumentation coordination.
//!
//! `general` owns charge insertion and cold dispatch assembly. The ranked leaf
//! classifies the one cyclic carrier and rebases only its three semantic
//! branches after insertion has established exact function-local coordinates.

mod general;
mod ranked_u32_countdown;

pub use general::NativeFuelInstrumentationError;

use omega_installation_evidence::NativeFuelTargetPlanProjection;
use omega_machine_code::{MachineCodePlan, NativeFuelInstrumentedPlan};

/// Classify ranked custody once, then run the shared two-pass instrumenter
/// with an explicit branch-rebase disposition.
pub fn instrument_native_fuel(
    source: MachineCodePlan,
    target_policy: NativeFuelTargetPlanProjection,
) -> Result<NativeFuelInstrumentedPlan, NativeFuelInstrumentationError> {
    if source.target != target_policy.target
        || target_policy.profile.native_target() != target_policy.target
    {
        return Err(NativeFuelInstrumentationError::TargetMismatch);
    }
    let ranked_machine = ranked_u32_countdown::classify(&source)?;
    general::instrument_classified_native_fuel(source, target_policy, ranked_machine)
}
