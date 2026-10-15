//! Normal boundary completion preserves call-scoped descriptor carriers.
//!
//! Borrowed bytes and elements expose backing, not the descriptor slot. When
//! every writable input has descriptor-free backing and the invocation returns
//! no custody, no output channel can retain or replace an input descriptor.
//! A structural result counts as no custody exactly when its declared type is
//! descriptor-free (`plain_owned_shape`: no borrowed view, element view or
//! reference anywhere in its fields or cases), like `LineReadResult`'s counts.
//! Results that can carry a descriptor, owned inputs, retained guarantees and
//! unknown writable storage keep the conservative escape treatment. This
//! storage summary grants no access: ordinary argument, bounds, completion and
//! loan checks still apply.
use super::{
    Operation, OperationKind, StructuralAccess, StructuralTypeShape, TerminalMachine,
    TerminalModule,
};
use semantic_vocabulary::{PlaceId, ScalarType, StructuralTypeId};
use terminal_psi::{
    BoundaryContentGuarantee, BoundaryMachineDeclaration, BoundaryMachineResult, OperationResult,
    StructuralArgument, StructuralTypeDeclaration,
};

/// The value a completed boundary call hands back to its caller, by kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryCompletionResult {
    Unit,
    Scalar(ScalarType),
    Structural(StructuralTypeId),
}

/// Terminal's spelling of one boundary call: resolve its unique declaration
/// and argument roots in the module, then apply the shared judgment.
pub fn boundary_call_preserves_descriptor_storage(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> bool {
    let OperationKind::BoundaryCall {
        boundary,
        structural_arguments,
        completion_receipts,
        ..
    } = &operation.kind
    else {
        return false;
    };
    let mut declarations = module
        .boundary_machines
        .iter()
        .filter(|declaration| declaration.id == *boundary);
    let Some(declaration) = declarations.next() else {
        return false;
    };
    if declarations.next().is_some() {
        return false;
    }
    let result = match &operation.result {
        OperationResult::Unit => BoundaryCompletionResult::Unit,
        OperationResult::Scalar(result) => BoundaryCompletionResult::Scalar(result.scalar_type),
        OperationResult::Structural(result) => {
            BoundaryCompletionResult::Structural(result.structural_type)
        }
    };
    boundary_completion_preserves_descriptor_storage(
        module.structural_types.iter(),
        declaration,
        result,
        !completion_receipts.is_empty(),
        structural_arguments,
        |place| super::place_type(machine, place),
    )
}

/// Whether one boundary completion leaves every lent descriptor carrier in
/// place. Terminal and the Omega replay of optimized bodies share this exact
/// judgment, so both end the same call-scoped descriptor loans.
pub fn boundary_completion_preserves_descriptor_storage<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    declaration: &BoundaryMachineDeclaration,
    result: BoundaryCompletionResult,
    has_completion_receipts: bool,
    arguments: &[StructuralArgument],
    place_type: impl Fn(PlaceId) -> Option<StructuralTypeId>,
) -> bool {
    if has_completion_receipts
        || declaration.structural_parameters.len() != arguments.len()
        || declaration
            .content_guarantees
            .iter()
            .any(|guarantee| matches!(guarantee, BoundaryContentGuarantee::RetainedBorrow(_)))
        || !match (&declaration.result, result) {
            (BoundaryMachineResult::Unit, BoundaryCompletionResult::Unit) => true,
            (BoundaryMachineResult::Scalar(expected), BoundaryCompletionResult::Scalar(actual)) => {
                *expected == actual
            }
            (
                BoundaryMachineResult::Structural(expected),
                BoundaryCompletionResult::Structural(actual),
            ) => {
                expected.structural_type == actual
                    && super::plain_owned_shape(types.clone(), actual)
            }
            _ => false,
        }
    {
        return false;
    }
    arguments
        .iter()
        .zip(&declaration.structural_parameters)
        .all(|(argument, parameter)| {
            if argument.access != parameter.access || argument.access == StructuralAccess::Owned {
                return false;
            }
            if argument.access == StructuralAccess::SharedBorrow {
                return true;
            }
            let Some(root) = place_type(argument.place) else {
                return false;
            };
            let Some(selected) =
                crate::runtime_structural_path_tip(types.clone(), root, &argument.path)
            else {
                return false;
            };
            // A root view lends its elements; it does not expose its descriptor as
            // mutable output storage. Nested borrowed carriers need their own exact
            // storage relationship and cannot acquire it from an enclosing shape.
            if argument.path.is_empty() {
                match &selected.shape {
                    StructuralTypeShape::ByteSequence(carrier) if carrier.is_borrowed() => {
                        return true;
                    }
                    StructuralTypeShape::ElementView { element, .. } => {
                        return super::plain_owned_shape(types.clone(), *element);
                    }
                    _ => {}
                }
            }
            super::plain_owned_shape(types.clone(), selected.id)
        })
}
