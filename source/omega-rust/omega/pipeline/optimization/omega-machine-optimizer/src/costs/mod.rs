//! Optimizer module role: executable entrance. Non-authoritative target cost observation.
//!
//! This entrance selects the one current descriptive model and binds it to an
//! exact native target. The lower leaves own the public data vocabulary and
//! stable identity encoding. Optimizer validators must not use this module for
//! semantic admission.

mod identity;
mod model;

pub use model::{
    NonAuthoritativeLatencyCost, NonAuthoritativeMachineCost, NonAuthoritativeMachineSizeCost,
    TargetCostModel, TargetCostModelIdentity, TargetCostModelVersion,
};

use omega_target::NativeTarget;

/// Construct the current target-scoped descriptive cost model.
pub fn target_cost_model(target: NativeTarget) -> TargetCostModel {
    let version = TargetCostModelVersion::MachineKnowledgeV1;
    TargetCostModel::new(
        target,
        version,
        identity::target_cost_model_identity(target, version),
    )
}

#[cfg(test)]
mod tests;
