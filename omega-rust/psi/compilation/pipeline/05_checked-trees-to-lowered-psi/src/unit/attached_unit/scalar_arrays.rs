//! Array construction shares the ordinary structural result namespace: each
//! element is a checked scalar operand, emitted in order into the array's
//! place. Completion uses the sibling structural-completion checker; this
//! module owns array shape and storage.
use super::{
    CheckedTrees, LoweringError, Operation, OperationKind, StructuralMultiplicity,
    StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeId, ValueDeclaration,
    allocate_dense, lookup_type_id, place_id, unsupported,
};
use crate::emission::operation_emission::buffer::{OperationBuffer, structural_result_declaration};
use checked_trees::CheckedUnitStructuralResultBindingPlan;

pub(super) fn validate_shape(
    checked: &CheckedTrees,
    mut reference: checked_trees::types::TypeReferenceHandle,
) -> Result<(), LoweringError> {
    for _ in 0..checked.typed.type_reference_table.type_reference_count() {
        let identity = checked.typed.normalized_type_identity(reference);
        let mut matches = checked
            .facts
            .flow
            .terminal_unit_effects
            .structural_types
            .iter()
            .filter(|plan| plan.identity == identity.as_str());
        match matches.next() {
            Some(_) => {}
            None => {
                // A scalar qualification is a value-domain shell, not a
                // storage carrier: an element spelled `i32 in Saturating`
                // stores under the bare carrier's shape, so walk through the
                // shell the way custody checks do.
                let carrier = checked.facts.resolved_types.carrier(reference);
                if carrier.is_valid() && carrier != reference {
                    reference = carrier;
                    continue;
                }
                return unsupported("array source shape is absent");
            }
        }
        if matches.next().is_some() {
            return unsupported("array source shape identity is duplicated");
        }
        match checked.typed.type_reference_table.type_reference(reference) {
            checked_trees::types::TypeReferenceNode::FixedArray {
                element_type,
                length: checked_trees::types::FixedArrayLength::Literal(_),
            } => reference = *element_type,
            _ => return Ok(()),
        }
    }
    unsupported("array source carrier is cyclic")
}

pub(super) fn emit(
    result: &CheckedUnitStructuralResultBindingPlan,
    elements: &[ValueDeclaration],
    types: &[(String, StructuralTypeId)],
    next_place: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let structural_type = lookup_type_id(types, &result.type_identity)?;
    let id = operations.allocate();
    let place = place_id(allocate_dense(next_place)?);
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id,
        result: structural_result_declaration(
            place,
            structural_type,
            StructuralMultiplicity::Unrestricted,
        ),
        kind: OperationKind::EstablishScalarArray {
            elements: elements.iter().map(|element| element.id).collect(),
        },
    });
    Ok(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::OperationResult {
            producer: id,
            structural_type,
        },
    })
}
