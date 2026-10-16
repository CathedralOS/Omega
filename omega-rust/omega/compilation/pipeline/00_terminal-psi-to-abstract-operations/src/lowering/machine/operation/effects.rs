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
        OperationKind::RegisterAccess { service, event } => AbstractOperation::RegisterAccess {
            psi_operation: operation.id,
            service,
            event,
            result: operation.result.scalar().map(Into::into),
        },
        OperationKind::WriteOnlyPrimitiveStore {
            destination,
            path,
            value,
            range_obligation,
        } => {
            let invalid = || LoweringError::InvalidWriteOnlyPrimitiveStore(operation.id);
            // Whole and projected primitive stores retain the actual declared
            // root and its receiving obligation. An owned primitive home is
            // the empty projection of that same root, not a second operation
            // that erases restricted-store evidence or invents an ABI formal.
            let destination =
                super::structural_scalar_fields::store_destination(machine, destination)
                    .ok_or_else(invalid)?;
            let Some(scalar_type) = value_types.get(&value).copied() else {
                return Err(invalid());
            };
            let valid_destination = destination.parameter().is_none_or(|parameter| {
                matches!(
                    parameter.access,
                    terminal_psi::StructuralAccess::MutableBorrow
                        | terminal_psi::StructuralAccess::WriteOnlyBorrow
                ) && parameter.qualifications.is_empty()
                    && parameter.projected_qualifications.is_empty()
            }) && (destination.multiplicity()
                == terminal_psi::StructuralMultiplicity::Unrestricted
                || (!path.is_empty()
                    && destination.multiplicity() == terminal_psi::StructuralMultiplicity::Affine))
                && machine
                    .entry_claims
                    .iter()
                    .all(|claim| claim.input != destination.place())
                && machine
                    .content_entry_claims
                    .iter()
                    .all(|claim| claim.input.root != destination.place());
            if !valid_destination {
                return Err(invalid());
            }
            if super::primitive_projection::leaf_type(
                structural_types,
                destination.structural_type(),
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
                range_obligation,
            }
        }
        _ => unreachable!("effect router is exhaustive"),
    })
}
