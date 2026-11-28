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
    let Some(declaration) =
        exactly_one(module.interfaces.boundary_machines.iter(), |declaration| {
            declaration.id == *boundary
        })
    else {
        return false;
    };
    if declaration.declaration.as_ref().is_some_and(|d| {
        matches!(
            d.execution,
            terminal_psi::CheckedBoundaryExecution::OwnedActivation(
                language_core::sealed_intrinsics::ActivationOperation::Park
            )
        )
    }) {
        // A checked park moves the request and retains its shared anchor.
        // Neither the carrier's pointer nor the byte it names is rewritten.
        return checked_park_anchor(module, machine, operation, declaration).is_some();
    }
    let result = match &operation.result {
        OperationResult::Unit => BoundaryCompletionResult::Unit,
        OperationResult::Scalar(result) => BoundaryCompletionResult::Scalar(result.scalar_type),
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

pub(super) fn checked_park_anchor(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    boundary: &BoundaryMachineDeclaration,
) -> Option<crate::calls::owned_activation::ParkShape> {
    let OperationKind::BoundaryCall {
        boundary: selected,
        structural_arguments,
        completion_receipts,
        ..
    } = &operation.kind
    else {
        return None;
    };
    let [actual] = structural_arguments.as_slice() else {
        return None;
    };
    let [formal] = boundary.structural_parameters.as_slice() else {
        return None;
    };
    if *selected != boundary.id
        || actual.access != StructuralAccess::Owned
        // Elaboration records consumption of the request's one root claim.
        // The ownership checker verifies its identity. It is not a write to
        // either the held reference or the slot to which it points.
        || !(completion_receipts.is_empty() || matches!(completion_receipts.as_slice(),
            [receipt] if receipt.argument_index == 0 && receipt.path.is_empty()))
        || !matches!(
            boundary.declaration.as_ref()?.execution,
            terminal_psi::CheckedBoundaryExecution::OwnedActivation(
                language_core::sealed_intrinsics::ActivationOperation::Park
            )
        )
    {
        return None;
    }
    let shape = crate::calls::owned_activation::park_shape(module, boundary).ok()?;
    let ty = crate::runtime_structural_path_tip(
        module.types.structural_types.iter(),
        machine.structural_place_type(actual.place)?,
        &actual.path,
    )?;
    (ty.id == formal.structural_type).then_some(shape)
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
