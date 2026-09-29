//! The aliasing checks between every pair of structural arguments.

use super::{
    CallShape, interior_member_borrow, structural_access_is_exclusive, structural_paths_may_overlap,
};
use crate::validation::{
    ModuleError, OperationId, OperationKind, StructuralAccess, StructuralArgument,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPlaceKind, TerminalMachine,
    TerminalModule,
};

/// Every pair of arguments: two loans of the same root may not alias
/// unless both are independent copies, and no owned move overlaps another
/// argument's path.
pub(in crate::validation) fn validate_argument_pairs(
    module: &TerminalModule,
    caller: &TerminalMachine,
    arguments: &[StructuralArgument],
    expected: &[StructuralParameterDeclaration],
    operation: OperationId,
    shape: &CallShape<'_>,
) -> Result<(), ModuleError> {
    let CallShape {
        ordinary_call,
        call_kind,
        ..
    } = *shape;
    let receiver = expected.iter().position(|parameter| parameter.is_self);
    for first in 0..arguments.len() {
        for second in first + 1..arguments.len() {
            let left = &arguments[first];
            let right = &arguments[second];
            // Both arguments receive independent unrestricted array values;
            // neither takes an exclusive loan or moves the caller's payload.
            let copied_array = (ordinary_call
                || matches!(call_kind, Some(OperationKind::BoundaryCall { .. })))
                && left.access == StructuralAccess::Owned
                && right.access == StructuralAccess::Owned
                && left.path.is_empty()
                && right.path.is_empty()
                && crate::validation::scalar::array::plain_return_source(
                    module, caller, left.place,
                );
            // A complete unrestricted case is copied, not moved. Its owned
            // actual may coexist with a whole shared observation, but this
            // grants neither exclusive borrowing nor projected custody.
            let copied_case = ordinary_call
                && matches!(
                    left.access,
                    StructuralAccess::Owned | StructuralAccess::SharedBorrow
                )
                && matches!(
                    right.access,
                    StructuralAccess::Owned | StructuralAccess::SharedBorrow
                )
                && left.path.is_empty()
                && right.path.is_empty()
                && (crate::validation::scalar::case::plain_return_source(
                    module, caller, left.place,
                ) || crate::validation::structural::case::plain_return_source(
                    module, caller, left.place,
                ))
                && crate::validation::structural::result_contracts::source_signature(
                    caller, left.place,
                )
                .is_some_and(|source| source.multiplicity == StructuralMultiplicity::Unrestricted);
            // The receiver's claim is the callee's own context: a borrow
            // strictly inside it is a subloan formed inside that authority —
            // a frozen or suspended child under the loan table — never an
            // independent second claim on the caller's custody.
            let interior_loan = receiver.is_some_and(|index| {
                (first == index && interior_member_borrow(left, right))
                    || (second == index && interior_member_borrow(right, left))
            });
            if left.place == right.place
                && structural_paths_may_overlap(&left.path, &right.path)
                && !copied_array
                && !copied_case
                && !interior_loan
                && (structural_access_is_exclusive(left.access)
                    || structural_access_is_exclusive(right.access)
                    || ((left.access == StructuralAccess::Owned
                        || right.access == StructuralAccess::Owned)
                        && caller.structural_places.iter().any(|place| {
                            place.id == left.place
                                && (matches!(
                                    place.kind,
                                    StructuralPlaceKind::OperationResult { .. }
                                ) || caller
                                    .structural_parameters
                                    .iter()
                                    .chain(
                                        caller
                                            .blocks
                                            .iter()
                                            .flat_map(|block| &block.structural_parameters),
                                    )
                                    .any(|parameter| {
                                        parameter.place == place.id
                                            && parameter.access == StructuralAccess::Owned
                                            && parameter.multiplicity
                                                == StructuralMultiplicity::Affine
                                    }))
                        })))
            {
                return Err(ModuleError::OverlappingExclusiveStructuralArguments {
                    operation,
                    first_argument: first as u32,
                    second_argument: second as u32,
                });
            }
        }
    }
    Ok(())
}
