//! Exact retained quotient copy/call joins after Terminal admission.
//! These joins preserve source admission; they do not prove theorem truth.
use crate::{AbstractOperation, AbstractStructuralCallResult, StructuralTypeCatalog};
use semantic_vocabulary::{MachineId, OperationId, PlaceId, StructuralTypeId};
use terminal_psi::{StructuralOperationResult, TerminalQuotientAdaptationLicense as License};

pub fn licensed_copy<'a>(
    catalog: &StructuralTypeCatalog,
    machine: MachineId,
    operation: OperationId,
    source: PlaceId,
    source_type: StructuralTypeId,
    result: &StructuralOperationResult,
    operations: impl Iterator<Item = &'a AbstractOperation>,
) -> bool {
    let mut rows = catalog
        .quotients()
        .adaptations
        .iter()
        .filter(|row| row.machine == machine && row.copy == operation);
    let Some(row) = rows.next() else {
        return false;
    };
    if rows.next().is_some()
        || row.source != source
        || row.source_type != source_type
        || row.result != result.place
        || row.result_type != result.structural_type
        || result.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
        || !result.claims.is_empty()
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.qualification_establishments.is_empty()
    {
        return false;
    }
    let Some(representation) = catalog
        .quotients()
        .representations
        .get(row.representation as usize)
    else {
        return false;
    };
    let (operation, position, input) = match row.license {
        License::Mint => {
            return source_type == representation.carrier
                && result.structural_type == representation.quotient;
        }
        License::Input {
            operation,
            position,
        } => (operation, Some(position), true),
        License::Result { operation } => (operation, None, false),
    };
    if (source_type, result.structural_type)
        != if input {
            (representation.quotient, representation.carrier)
        } else {
            (representation.carrier, representation.quotient)
        }
    {
        return false;
    }
    let Some(operation) = catalog.quotients().operations.get(operation as usize) else {
        return false;
    };
    if operation.public_machine != machine {
        return false;
    }
    let mut calls =
        operations.filter(|candidate| candidate.psi_operation() == Some(operation.call));
    let Some(AbstractOperation::CallStructural {
        callee,
        structural_arguments,
        result: call_result,
        ..
    }) = calls.next()
    else {
        return false;
    };
    if calls.next().is_some() || callee.machine() != Some(operation.representative_machine) {
        return false;
    }
    if let Some(position) = position {
        structural_arguments
            .get(position as usize)
            .is_some_and(|argument| argument.place == result.place && argument.path.is_empty())
    } else {
        matches!(call_result, AbstractStructuralCallResult::Structural { result, .. } if result.place == source)
    }
}
