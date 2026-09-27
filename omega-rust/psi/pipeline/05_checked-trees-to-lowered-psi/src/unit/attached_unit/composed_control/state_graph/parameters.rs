//! Persistent receivers keep the invocation place and type. A state may restrict
//! mutable access to a shared view, but cannot gain authority on an edge.

use super::super::super::{Multiplicity, unsupported};
use super::super::{CheckedTrees, LoweringError};
use super::CheckedComposedUnitControlMachinePlan;
pub(super) fn validate_receiver(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    source: &checked_trees::signature::StateParameter,
    parameter: &checked_trees::CheckedUnitStructuralParameterPlan,
    access: language_core::ReferenceAccess,
) -> Result<(), LoweringError> {
    let expected_access = match access {
        language_core::ReferenceAccess::Shared => {
            checked_trees::CheckedStructuralAccess::SharedBorrow
        }
        language_core::ReferenceAccess::Mutable => {
            checked_trees::CheckedStructuralAccess::MutableBorrow
        }
        _ => return unsupported("Unit graph receiver requires readable borrowed access"),
    };
    if !source.is_self
        || source.is_const
        || source.is_mutable != (access == language_core::ReferenceAccess::Mutable)
        || parameter.access != expected_access
        || parameter.multiplicity != Multiplicity::Unrestricted
        || !parameter.qualifications.is_empty()
        || parameter.fused_service_erasure.is_some()
        || plan.attachment_type_identity.as_ref() != Some(&parameter.type_identity)
        || !plan
            .states
            .first()
            .and_then(|state| {
                state
                    .structural_parameters
                    .iter()
                    .find(|entry| entry.is_self)
            })
            .is_some_and(|entry| receiver_transfer_matches(entry, parameter))
    {
        return unsupported("Unit graph receiver lost its invocation authority");
    }
    let shapes = &checked.facts.flow.terminal_unit_effects.structural_types;
    let mut matching = shapes
        .iter()
        .filter(|shape| shape.identity == parameter.type_identity);
    if matching.next().is_none() || matching.next().is_some() {
        return unsupported("Unit graph receiver lost its exact structural type");
    }
    Ok(())
}

pub(super) fn receiver_transfer_matches(
    source: &checked_trees::CheckedUnitStructuralParameterPlan,
    target: &checked_trees::CheckedUnitStructuralParameterPlan,
) -> bool {
    if !source.is_self || !target.is_self {
        return false;
    }
    // Compare the complete declaration after the sole permitted restriction,
    // so position, type, domain and service custody cannot change with access.
    let mut transferred = source.clone();
    if transferred.access == checked_trees::CheckedStructuralAccess::MutableBorrow
        && target.access == checked_trees::CheckedStructuralAccess::SharedBorrow
    {
        transferred.access = checked_trees::CheckedStructuralAccess::SharedBorrow;
    }
    transferred == *target
}
