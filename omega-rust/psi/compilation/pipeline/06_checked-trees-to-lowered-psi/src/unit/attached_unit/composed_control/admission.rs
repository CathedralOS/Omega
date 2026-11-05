//! What a composed graph's states may hold and the boundaries their calls
//! name. The graph itself (`state_graph::admission`) decides what else the
//! route admits.
use super::super::operation_frame::OperationFrame;
use super::super::{CheckedBoundaryMachinePlan, CheckedUnitEffectOperationPlan, unsupported};
use super::{CheckedTrees, LoweringError};
use crate::unit::attached_unit::admission::retain_call_boundaries;
use crate::unit::attached_unit::bodies::UnitPlans;
use crate::unit::attached_unit::call_closure::BoundaryIdentity;

/// Collect the boundaries the given states' calls name, and refuse an
/// operation the composed emitter has no route for.
pub(super) fn admit_calls<'a>(
    checked: &'a CheckedTrees,
    call_states: &[&checked_trees::CheckedComposedUnitControlStatePlan],
) -> Result<Vec<(&'a CheckedBoundaryMachinePlan, BoundaryIdentity)>, LoweringError> {
    let plans = UnitPlans::published(&checked.facts.flow);
    let mut boundaries = Vec::new();
    for state in call_states {
        for operation in state.operation_dependencies() {
            retain_call_boundaries(
                checked,
                plans,
                std::iter::once(operation),
                &mut boundaries,
                |_| {},
            )?;
            // A call state retains its invocation and the effects that
            // prepare it: the ones the frame emits (`OperationFrame::lowers`),
            // plus the frame-local storage mints a statement establishes
            // without a callee. Nothing else belongs to a call state.
            match operation {
                _ if OperationFrame::consumes_prepared_operands(operation) => {}
                CheckedUnitEffectOperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::Primitive { .. }
                    | checked_trees::CheckedUnitLocalBindingPlan::TrivialAffine { .. },
                ) => {}
                _ if OperationFrame::lowers(operation) => {}
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
