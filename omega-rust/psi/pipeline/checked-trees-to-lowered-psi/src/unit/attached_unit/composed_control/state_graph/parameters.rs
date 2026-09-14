//! Persistent receivers retain the exact invocation declaration in every state.

use super::*;

pub(super) fn validate_receiver(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    source: &checked_trees::signature::StateParameter,
    parameter: &checked_trees::CheckedUnitStructuralParameterPlan,
    access: language_core::ReferenceAccess,
) -> Result<(), LoweringError> {
    if !source.is_self
        || source.is_const
        || !source.is_mutable
        || access != language_core::ReferenceAccess::Mutable
        || parameter.access != checked_trees::CheckedStructuralAccess::MutableBorrow
        || parameter.multiplicity != Multiplicity::Unrestricted
        || !parameter.qualifications.is_empty()
        || parameter.fused_service_erasure.is_some()
        || plan.attachment_type_identity.as_ref() != Some(&parameter.type_identity)
        || plan.states.first().and_then(|state| {
            state
                .structural_parameters
                .iter()
                .find(|entry| entry.is_self)
        }) != Some(parameter)
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
