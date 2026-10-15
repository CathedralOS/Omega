//! Array construction shares the ordinary structural result namespace: each
//! element is a checked scalar operand, emitted in order into the array's
//! place. Completion uses the sibling structural-completion checker; this
//! module owns array shape and storage.
use super::{
    LoweringError, Operation, OperationKind, StructuralMultiplicity, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralTypeId, ValueDeclaration, allocate_dense, lookup_type_id,
    place_id,
};
use crate::emission::operation_emission::buffer::{OperationBuffer, structural_result_declaration};
use checked_trees::CheckedUnitStructuralResultBindingPlan;

pub(super) fn emit(
    result: &CheckedUnitStructuralResultBindingPlan,
    mut qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
    elements: &[ValueDeclaration],
    types: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
    next_place: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let structural_type = lookup_type_id(types, &result.type_identity)?;
    let id = operations.allocate();
    let place = place_id(allocate_dense(next_place)?);
    qualifications.sort();
    qualifications.dedup();
    let mut result =
        structural_result_declaration(place, structural_type, StructuralMultiplicity::Unrestricted);
    if let terminal_psi::OperationResult::Structural(result) = &mut result {
        result.qualifications = qualifications;
    }
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id,
        result,
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
