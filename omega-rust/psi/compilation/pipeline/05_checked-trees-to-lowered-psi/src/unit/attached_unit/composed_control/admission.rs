//! What a composed graph's states may hold and the boundaries their calls
//! name. The graph itself (`state_graph::admission`) decides what else the
//! route admits.
use super::super::{CheckedBoundaryMachinePlan, CheckedUnitEffectOperationPlan, unsupported};
use super::{CheckedTrees, LoweringError};
use crate::unit::attached_unit::bodies::UnitPlans;
use crate::unit::attached_unit::call_closure::BoundaryIdentity;

/// A composed graph does not lower an attachment or its provider fields yet:
/// stage 04 can plan them (`checked_composed_provider_attachment_requirements`),
/// and this route refuses the machine rather than drop them.
pub(super) fn reject_attachment(
    plan: &checked_trees::CheckedComposedUnitControlMachinePlan,
) -> Result<(), LoweringError> {
    if plan.attachment_type_identity.is_some() || !plan.provider_attachment_requirements.is_empty()
    {
        return unsupported("composed Unit graph does not lower an attachment or provider field");
    }
    Ok(())
}

/// Collect the boundaries the given states' calls name, and refuse an
/// operation the composed emitter has no route for.
pub(super) fn admit_calls<'a>(
    checked: &'a CheckedTrees,
    call_states: &[&'a checked_trees::CheckedComposedUnitControlStatePlan],
) -> Result<Vec<(&'a CheckedBoundaryMachinePlan, BoundaryIdentity)>, LoweringError> {
    let plans = UnitPlans::published(&checked.facts.flow);
    let mut boundaries = Vec::new();
    for state in call_states.iter().copied() {
        for operation in state.operation_dependencies() {
            crate::unit::attached_unit::admission::retain_call_boundaries(
                checked,
                plans,
                std::iter::once(operation),
                &mut boundaries,
            )?;
            match operation {
                _ if operation.call().is_some() => {}
                CheckedUnitEffectOperationPlan::EstablishLocal(checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { .. })
                | CheckedUnitEffectOperationPlan::EstablishLocal(checked_trees::CheckedUnitLocalBindingPlan::Scalar { .. })
                // A store writes storage or a loan this state holds; it
                // names no callee.
                | CheckedUnitEffectOperationPlan::Store(_)
                // An atomic event or port write names no callee.
                | CheckedUnitEffectOperationPlan::AtomicFence { .. }
                | CheckedUnitEffectOperationPlan::AtomicAccess(_)
                | CheckedUnitEffectOperationPlan::RegisterAccess { .. }
                // The paired call carries the callee dependency; cleanup only
                // disposes its discarded result.
                | CheckedUnitEffectOperationPlan::ContinuationCleanup { .. }
                // A displaced field's move-out carries no callee; the
                // replacing call carries its own dependency.
                | checked_trees::structural_field_write_pattern!()
                // A view-subslice local narrows a view the state holds.
                | checked_trees::byte_sequence_establishment_pattern!()
                // Frame-local storage minted at an authored statement names no
                // callee; borrowed references bind storage already owned here.
                | CheckedUnitEffectOperationPlan::EstablishLocal(checked_trees::CheckedUnitLocalBindingPlan::ScalarArray { .. })
                | CheckedUnitEffectOperationPlan::EstablishLocal(checked_trees::CheckedUnitLocalBindingPlan::Primitive { .. })
                | CheckedUnitEffectOperationPlan::EstablishLocal(checked_trees::CheckedUnitLocalBindingPlan::TrivialAffine { .. })
                // A declared structural local's establishment mints
                // frame-local storage at its statement the same way.
                | CheckedUnitEffectOperationPlan::EstablishLocal(checked_trees::CheckedUnitLocalBindingPlan::Structural { .. })
                | checked_trees::reference_hold_pattern!() => {}
                _ => return unsupported("composed Unit call state contains a non-call operation"),
            }
        }
    }
    boundaries.sort_by_key(|(_, identity)| identity.order_key());
    if boundaries.windows(2).any(|pair| pair[0].1 == pair[1].1) {
        return unsupported("composed Unit boundaries have duplicate canonical identities");
    }
    Ok(boundaries)
}

pub(super) fn validate_contract(
    checked: &CheckedTrees,
    plan: &checked_trees::CheckedComposedUnitControlMachinePlan,
) -> Result<(), LoweringError> {
    if checked
        .facts
        .contract_plans
        .for_machine(plan.machine)
        .is_none()
    {
        return unsupported("composed Unit control is missing its canonical checked contract");
    }
    Ok(())
}
