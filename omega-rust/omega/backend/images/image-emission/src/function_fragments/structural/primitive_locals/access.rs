//! Primitive read and replacement publication over established or borrowed referents.
use super::establishment::{establishment, primitive_size, producer};
use abstract_operations::{AbstractFunction, AbstractOperation};
use selected_instructions::{
    SelectedFunction, SelectedInstructionKind, SelectedMemoryAccessOrigin,
    SelectedMemoryAccessRole, VirtualRegisterOrigin,
};
use semantic_vocabulary::{PlaceId, ScalarType};
use terminal_psi::{StructuralAccess, StructuralMultiplicity, StructuralTypeShape};

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
        AbstractOperation::PrimitiveScalarRead {
            psi_operation,
            result,
            source,
        } => {
            if !readable(function, selected, *source, result.scalar_type) {
                return false;
            }
            (
                *psi_operation,
                *source,
                result.scalar_type,
                SelectedMemoryAccessRole::ReadPlace,
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
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter(|row| row.id == access.instruction);
    let Some(instruction) = instructions.next() else {
        return false;
    };
    if instructions.next().is_some() || instruction.provenance.operations != [identity] {
        return false;
    }
    match operation {
        AbstractOperation::PrimitiveScalarRead { result, .. } => {
            let load = match size {
                1 => SelectedInstructionKind::Load8 { byte_offset: 0 },
                2 => SelectedInstructionKind::Load16 { byte_offset: 0 },
                4 => SelectedInstructionKind::Load32 { byte_offset: 0 },
                8 => SelectedInstructionKind::Load64 { byte_offset: 0 },
                _ => return false,
            };
            instruction.kind == load
                && instruction.operands.len() == 2
                && instruction.provenance.values == [result.value]
                && selected.virtual_registers.iter().any(|register| {
                    register.id == instruction.operands[1].virtual_register
                        && register.scalar_type == result.scalar_type
                        && matches!(register.origin, VirtualRegisterOrigin::InstructionResult { instruction: defining, source_value }
                            if defining == instruction.id && source_value == result.value)
                })
        }
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

fn readable(
    function: &AbstractFunction,
    selected: &SelectedFunction,
    place: PlaceId,
    scalar: ScalarType,
) -> bool {
    if let Some((_, _, value)) = producer(function, place) {
        return value.scalar_type == scalar && establishment(function, selected, place).is_some();
    }
    let Some(contract) = &selected.structural else {
        return false;
    };
    let mut parameters = contract
        .parameters
        .iter()
        .filter(|parameter| parameter.semantic.place == place);
    let Some(parameter) = parameters.next() else {
        return false;
    };
    let declaration = &parameter.semantic;
    parameters.next().is_none()
        && contract.entry_claims.is_empty()
        && matches!(
            declaration.access,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
        )
        && declaration.multiplicity == StructuralMultiplicity::Unrestricted
        && declaration.qualifications.is_empty()
        && declaration.projected_qualifications.is_empty()
        && contract
            .structural_types
            .iter()
            .filter(|declaration_type| {
                declaration_type.id == declaration.structural_type
                    && declaration_type.shape == StructuralTypeShape::PrimitiveScalar(scalar)
            })
            .count()
            == 1
}
