//! Reference carriers retain ingress provenance and the exact source loan end.
//! A consumer borrows the referent; it does not acquire its linear claims or
//! cleanup. The checked borrow-free storage judgment excludes nested carriers
//! without requiring the referent to be a freely copied owned snapshot.
use super::super::StructuralArgument;
use super::{
    LoweringError, Operation, OperationKind, StructuralMultiplicity, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralTypeId, allocate_dense, lookup_type_id, place_id,
};
use crate::emission::operation_emission::buffer::{OperationBuffer, structural_result_declaration};
use checked_trees::CheckedUnitStructuralResultBindingPlan;

pub(super) fn emit(
    result: &CheckedUnitStructuralResultBindingPlan,
    source: StructuralArgument,
    storage: Option<terminal_psi::ReferenceStorageView>,
    types: &[(String, StructuralTypeId)],
    next_place: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let structural_type = lookup_type_id(types, &result.type_identity)?;
    emit_carrier(structural_type, source, storage, next_place, operations)
}

pub(super) fn emit_carrier(
    structural_type: StructuralTypeId,
    source: StructuralArgument,
    storage: Option<terminal_psi::ReferenceStorageView>,
    next_place: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let id = operations.allocate();
    let place = place_id(allocate_dense(next_place)?);
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id,
        result: structural_result_declaration(
            place,
            structural_type,
            StructuralMultiplicity::Affine,
        ),
        kind: OperationKind::EstablishReference { source, storage },
    });
    Ok(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::OperationResult {
            producer: id,
            structural_type,
        },
    })
}
