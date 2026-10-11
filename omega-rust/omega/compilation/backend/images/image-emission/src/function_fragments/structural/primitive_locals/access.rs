//! Primitive establishment and replacement publication over local storage.
use super::establishment::{establishment, primitive_size, producer};
use abstract_operations::{AbstractFunction, AbstractOperation};
use selected_instructions::{
    SelectedFunction, SelectedInstructionKind, SelectedMemoryAccessOrigin, SelectedMemoryAccessRole,
};

pub(in crate::function_fragments) fn operation_retained(
    function: &AbstractFunction,
    selected: &SelectedFunction,
    operation: &AbstractOperation,
) -> bool {
    let (identity, place, scalar, role) = match operation {
        AbstractOperation::EstablishPrimitiveLocal {
            psi_operation,
            result,
            ..
        } => {
            return establishment(function, selected, result.place)
                .is_some_and(|(actual, _)| actual == *psi_operation);
        }
        AbstractOperation::PrimitiveLocalStore {
            psi_operation,
            destination,
            value,
        } => {
            if establishment(function, selected, *destination).is_none()
                || producer(function, *destination)
                    .is_none_or(|(_, _, initializer)| initializer.scalar_type != value.scalar_type)
            {
                return false;
            }
            (
                *psi_operation,
                *destination,
                value.scalar_type,
                SelectedMemoryAccessRole::WritePlace,
            )
        }
        _ => return false,
    };
    let Some(size) = primitive_size(scalar) else {
        return false;
    };
    let mut accesses = selected
        .memory_accesses
        .iter()
        .filter(|row| row.origin == SelectedMemoryAccessOrigin::Operation(identity));
    let Some(access) = accesses.next() else {
        return false;
    };
    let memory_matches = accesses.next().is_none()
        && access.place == place
        && access.byte_offset == 0
        && access.byte_count == u32::from(size)
        && access.role == role;
    if !memory_matches {
        return false;
    }
    let mut instructions = selected
        .instructions()
        .filter(|row| row.id == access.instruction);
    let Some(instruction) = instructions.next() else {
        return false;
    };
    if instructions.next().is_some() || instruction.provenance.operations.last() != Some(&identity)
    {
        return false;
    }
    match operation {
        AbstractOperation::PrimitiveLocalStore { value, .. } => {
            instruction.kind
                == (SelectedInstructionKind::Store {
                    byte_offset: 0,
                    byte_size: size,
                })
                && instruction.provenance.values == [value.value]
        }
        _ => false,
    }
}
