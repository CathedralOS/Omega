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
        OperationKind::WriteOnlyPrimitiveStore {
            destination,
            path,
            value,
        } => {
            if let Some(local) = super::primitive_storage::local(machine, destination) {
                let scalar_type =
                    super::primitive_storage::scalar_type(structural_types, local.structural_type)
                        .ok_or(LoweringError::InvalidWriteOnlyPrimitiveStore(operation.id))?;
                if !path.is_empty() || value_types.get(&value) != Some(&scalar_type) {
                    return Err(LoweringError::InvalidWriteOnlyPrimitiveStore(operation.id));
                }
                return Ok(AbstractOperation::PrimitiveLocalStore {
                    psi_operation: operation.id,
                    destination,
                    value: AbstractResult { value, scalar_type },
                });
            }
            let invalid = || LoweringError::InvalidWriteOnlyPrimitiveStore(operation.id);
            let Some(destination) = machine
                .structural_parameters
                .iter()
                .find(|parameter| parameter.place == destination)
                .cloned()
            else {
                return Err(invalid());
            };
            let Some(scalar_type) = value_types.get(&value).copied() else {
                return Err(invalid());
            };
            let valid_destination = matches!(
                destination.access,
                terminal_psi::StructuralAccess::MutableBorrow
                    | terminal_psi::StructuralAccess::WriteOnlyBorrow
            ) && (destination.multiplicity
                == terminal_psi::StructuralMultiplicity::Unrestricted
                || (!path.is_empty()
                    && destination.multiplicity == terminal_psi::StructuralMultiplicity::Affine))
                && destination.qualifications.is_empty()
                && destination.projected_qualifications.is_empty()
                && machine
                    .entry_claims
                    .iter()
                    .all(|claim| claim.input != destination.place)
                && machine
                    .content_entry_claims
                    .iter()
                    .all(|claim| claim.input.root != destination.place);
            if !valid_destination {
                return Err(invalid());
            }
            if super::primitive_projection::leaf_type(
                structural_types,
                destination.structural_type,
                &path,
                |index| value_types.get(&index).copied(),
            ) != Some(scalar_type)
            {
                return Err(invalid());
            }
            // A runtime element keeps the same destination custody as a
            // static store: the selector rides in the path, and its verified
            // obligation stays attached to the segment.
            AbstractOperation::WriteOnlyPrimitiveStore {
                psi_operation: operation.id,
                destination,
                path,
                value: AbstractResult { value, scalar_type },
            }
        }
        _ => unreachable!("effect router is exhaustive"),
    })
}
