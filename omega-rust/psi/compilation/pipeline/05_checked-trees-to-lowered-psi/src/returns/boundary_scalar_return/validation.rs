//! The checked boundary target a result-bearing boundary return lowers onto.
//!
//! Stage 04 registers each boundary scalar return once, with its retained
//! boundary call, and the boundary target's own plan; this module only finds
//! that target. It does not re-read the authored source or re-confirm the
//! plan against it: a plan that disagrees with its source is a stage 04 bug,
//! found by the corpus (omega-rust/pipeline.md, "Verification placement").

use super::{
    CheckedBoundaryMachinePlan, CheckedBoundaryScalarReturnMachinePlan, CheckedTrees,
    LoweringError, unsupported,
};

pub(crate) fn validate_boundary_scalar_return<'a>(
    checked: &'a CheckedTrees,
    plan: &CheckedBoundaryScalarReturnMachinePlan,
) -> Result<&'a CheckedBoundaryMachinePlan, LoweringError> {
    let call = super::retained_boundary_call(
        plan,
        "result-bearing boundary plan does not contain a boundary call",
    )?;
    let boundary = checked
        .facts
        .flow
        .terminal_boundary_scalar_returns
        .boundary_machines
        .iter()
        .find(|boundary| boundary.machine == call.target.machine)
        .ok_or(LoweringError::Unsupported(
            "result-bearing boundary target is absent from its checked plan",
        ))?;
    // The boundary-return route lowers no scalar requirement discharge.
    if !boundary.scalar_requires.is_empty() {
        return unsupported("boundary scalar return cannot discharge scalar requirements");
    }
    Ok(boundary)
}
