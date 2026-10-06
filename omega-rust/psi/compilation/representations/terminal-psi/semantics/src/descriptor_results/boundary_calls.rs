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
use terminal_psi::{BoundaryContentGuarantee, BoundaryMachineResult, OperationResult};

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
    if declarations.next().is_some()
        || !completion_receipts.is_empty()
        || declaration.structural_parameters.len() != structural_arguments.len()
        || declaration
            .content_guarantees
            .iter()
            .any(|guarantee| matches!(guarantee, BoundaryContentGuarantee::RetainedBorrow(_)))
        || !match (&declaration.result, &operation.result) {
            (BoundaryMachineResult::Unit, OperationResult::Unit) => true,
            (BoundaryMachineResult::Scalar(expected), OperationResult::Scalar(result)) => {
                *expected == result.scalar_type
            }
            (BoundaryMachineResult::Structural(expected), OperationResult::Structural(result)) => {
                expected.structural_type == result.structural_type
                    && super::plain_owned_shape(
                        module.structural_types.iter(),
                        result.structural_type,
                    )
            }
            _ => false,
        }
    {
        return false;
    }
    structural_arguments
        .iter()
        .zip(&declaration.structural_parameters)
        .all(|(argument, parameter)| {
            if argument.access != parameter.access || argument.access == StructuralAccess::Owned {
                return false;
            }
            if argument.access == StructuralAccess::SharedBorrow {
                return true;
            }
            let Some(root) = super::place_type(machine, argument.place) else {
                return false;
            };
            let Some(selected) = crate::runtime_structural_path_tip(
                module.structural_types.iter(),
                root,
                &argument.path,
            ) else {
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
                    StructuralTypeShape::ElementView { element } => {
                        return super::plain_owned_shape(module.structural_types.iter(), *element);
                    }
                    _ => {}
                }
            }
            super::plain_owned_shape(module.structural_types.iter(), selected.id)
        })
}
