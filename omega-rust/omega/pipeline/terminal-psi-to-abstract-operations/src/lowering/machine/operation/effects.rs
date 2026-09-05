use std::collections::BTreeMap;

use abstract_operations::{AbstractOperation, AbstractResult};
use semantic_vocabulary::ScalarType;
use terminal_psi::{Operation, OperationKind, StructuralTypeDeclaration, TerminalMachine};

use crate::lowering::LoweringError;

pub(super) fn lower(
    operation: &Operation,
    machine: &TerminalMachine,
    structural_types: &[StructuralTypeDeclaration],
    value_types: &BTreeMap<semantic_vocabulary::ValueId, ScalarType>,
) -> Result<AbstractOperation, LoweringError> {
    Ok(match operation.kind.clone() {
        OperationKind::PortWrite {
            service,
            port,
            value,
        } => AbstractOperation::PortWrite {
            psi_operation: operation.id,
            service,
            port,
            value,
        },
        OperationKind::WriteOnlyPrimitiveStore { destination, value } => {
            let Some(destination) = machine
                .structural_parameters
                .iter()
                .find(|parameter| parameter.place == destination)
                .cloned()
            else {
                return Err(LoweringError::InvalidWriteOnlyPrimitiveStore(operation.id));
            };
            let Some(scalar_type) = value_types.get(&value).copied() else {
                return Err(LoweringError::InvalidWriteOnlyPrimitiveStore(operation.id));
            };
            let valid_destination = matches!(
                destination.access,
                terminal_psi::StructuralAccess::MutableBorrow
                    | terminal_psi::StructuralAccess::WriteOnlyBorrow
            )
                && destination.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                && destination.qualifications.is_empty()
                && structural_types.iter().any(|declaration| {
                    declaration.id == destination.structural_type
                        && matches!(declaration.shape, terminal_psi::StructuralTypeShape::PrimitiveScalar(expected) if expected == scalar_type)
                });
            if !valid_destination {
                return Err(LoweringError::InvalidWriteOnlyPrimitiveStore(operation.id));
            }
            AbstractOperation::WriteOnlyPrimitiveStore {
                psi_operation: operation.id,
                destination,
                value: AbstractResult { value, scalar_type },
            }
        }
        _ => unreachable!("effect router is exhaustive"),
    })
}
