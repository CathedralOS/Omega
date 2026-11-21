use super::{
    Operation, OperationKind, StructuralAccess, StructuralTypeShape, TerminalMachine,
    TerminalModule,
};
use super::{owned_shared_byte_descriptor_shape, plain_owned_shape};
use crate::exactly_one::exactly_one;
use semantic_vocabulary::{PlaceId, ScalarType, StructuralTypeId};
use terminal_psi::{
    BoundaryContentGuarantee, BoundaryMachineDeclaration, BoundaryMachineResult,
    OperationResult, StructuralArgument, StructuralTypeDeclaration,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryCompletionResult {
    Unit,
    Scalar(ScalarType),
    Structural(StructuralTypeId),
}

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
    let Some(declaration) = exactly_one(module.interfaces.boundary_machines.iter(), |declaration| {
        declaration.id == *boundary
    }) else {
        return false;
    };
    let result = match &operation.result {
        OperationResult::Unit => BoundaryCompletionResult::Unit,
        OperationResult::Scalar(result) => {
            BoundaryCompletionResult::Scalar(result.scalar_type)
        }
        OperationResult::Structural(result) => {
            BoundaryCompletionResult::Structural(result.structural_type)
        }
    };
    boundary_completion_preserves_descriptor_storage(
        module.types.structural_types.iter(),
        declaration,
        result,
        !completion_receipts.is_empty(),
        structural_arguments,
        |place| machine.structural_place_type(place),
    )
}

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
        || declaration.content_guarantees.iter().any(|guarantee| {
            matches!(guarantee, BoundaryContentGuarantee::RetainedBorrow(_))
        })
        || !match (&declaration.result, result) {
            (BoundaryMachineResult::Unit, BoundaryCompletionResult::Unit) => true,
            (
                BoundaryMachineResult::Scalar(expected),
                BoundaryCompletionResult::Scalar(actual),
            ) => expected.scalar_type == actual,
            (
                BoundaryMachineResult::Structural(expected),
                BoundaryCompletionResult::Structural(actual),
            ) => {
                expected.structural_type == actual
                    && (plain_owned_shape(types.clone(), actual)
                        || (declaration.result_lifetimes.is_some()
                            && owned_shared_byte_descriptor_shape(types.clone(), actual)))
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
            if argument.access != parameter.access {
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
            if argument.access == StructuralAccess::Owned {
                // A moved immutable descriptor aggregate carries its
                // loans into the declared result frontier. With no
                // retained-borrow outcome, moving its carrier does not
                // make those loans escape beyond the result's use.
                return declaration.result_lifetimes.is_some()
                    && owned_shared_byte_descriptor_shape(types.clone(), selected.id);
            }
            if argument.path.is_empty() {
                match &selected.shape {
                    StructuralTypeShape::ByteSequence(carrier) if carrier.is_borrowed() => {
                        return true;
                    }
                    StructuralTypeShape::ElementView { element, .. } => {
                        return plain_owned_shape(types.clone(), *element);
                    }
                    _ => {}
                }
            }
            plain_owned_shape(types.clone(), selected.id)
        })
}
