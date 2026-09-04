//! Native emission for bounded ordered mutable-self stores then a scalar return.

use omega_assigned_target_operations::{
    AssignedBooleanExpression, AssignedFunction, AssignedIntegerExpression, AssignedOperation,
};
use omega_calling_conventions::{IndirectPointerLocation, ValueClass, ValueLocation};
use omega_machine_code::{
    ScalarStructuralScalarFieldStoreRecord, SemanticCodeAttribution, SemanticCodeSite,
};
use omega_target::{Architecture, NativeTarget};
use omega_target_operations::{TargetScalarStructuralFieldStore, TargetStructuralParameter};

use crate::scalar::{
    emit_aarch64_boolean_expression, emit_aarch64_integer_expression,
    emit_x86_64_boolean_expression, emit_x86_64_integer_expression, integer_bits,
    require_native_integer_width,
};
use crate::unit::structural_scalar::{emit_aarch64_unit_immediate, emit_x86_64_memory_store_width};
use crate::{
    EmissionError, aarch64_load_base, aarch64_store_base, aarch64_unit_memory_access,
    aarch64_unit_register, aarch64_unit_stack_access, append_aarch64_instructions,
    emit_x86_64_stack_load_width, x86_unit_register,
};

pub(super) struct EmittedScalarStore {
    pub(super) bytes: Vec<u8>,
    pub(super) stores: Vec<ScalarStructuralScalarFieldStoreRecord>,
    pub(super) semantic_code_attribution: Vec<SemanticCodeAttribution>,
}

pub(super) fn emit(
    function: &AssignedFunction,
    stores: &[TargetScalarStructuralFieldStore],
    scalar: &AssignedOperation,
    structural_parameters: &[TargetStructuralParameter],
    target: NativeTarget,
) -> Result<EmittedScalarStore, EmissionError> {
    let Some(store) = stores.first() else {
        return Err(EmissionError::EmptyStructuralScalarFieldStores(
            function.machine,
        ));
    };
    let invalid = || EmissionError::InvalidStructuralScalarFieldStoreCustody(store.psi_operation);
    let parameter_index = usize::try_from(store.destination.position).map_err(|_| invalid())?;
    let parameter = structural_parameters
        .get(parameter_index)
        .filter(|parameter| {
            parameter.place == store.destination.place
                && parameter.structural_type == store.destination.structural_type
                && parameter.multiplicity == store.destination.multiplicity
                && parameter.access == store.destination.access
                && parameter.projected_qualifications == store.destination.projected_qualifications
                && parameter.placement == store.destination_placement
        })
        .ok_or_else(invalid)?;
    let (
        psi_edge,
        frame,
        read_operation,
        return_source_value,
        return_field,
        return_field_byte_offset,
        return_scalar_type,
        exact_return,
    ) = match scalar {
        AssignedOperation::ReturnBooleanExpression {
            psi_edge,
            source_value,
            frame,
            expression:
                AssignedBooleanExpression::StructuralField {
                    psi_operation,
                    source_value: expression_value,
                    source,
                    field,
                    source_placement,
                    field_byte_offset,
                },
        } => (
            psi_edge,
            frame,
            psi_operation,
            source_value,
            field,
            field_byte_offset,
            psi_core::ScalarType::Boolean,
            source_value == expression_value
                && source == &store.destination.place
                && source_placement == &store.destination_placement,
        ),
        AssignedOperation::ReturnIntegerExpression {
            psi_edge,
            source_value,
            scalar_type: return_type,
            frame,
            expression:
                AssignedIntegerExpression::StructuralField {
                    psi_operation,
                    source_value: expression_value,
                    source,
                    field,
                    source_placement,
                    field_byte_offset,
                    integer_type,
                },
        } => (
            psi_edge,
            frame,
            psi_operation,
            source_value,
            field,
            field_byte_offset,
            psi_core::ScalarType::Integer(*return_type),
            source_value == expression_value
                && return_type == integer_type
                && source == &store.destination.place
                && source_placement == &store.destination_placement,
        ),
        _ => return Err(invalid()),
    };
    if stores.len() > 3 {
        return Err(invalid());
    }
    let prepared_stores = stores
        .iter()
        .map(|store| match store.immediate {
            omega_target_operations::TargetScalarImmediate::Boolean(value) => {
                Ok((store, 1, u64::from(value)))
            }
            omega_target_operations::TargetScalarImmediate::Integer { scalar_type, value } => Ok((
                store,
                require_native_integer_width(store.source_value, scalar_type)? / 8,
                integer_bits(store.source_value, scalar_type, value)?,
            )),
        })
        .collect::<Result<Vec<_>, EmissionError>>()?;
    let valid_stores = prepared_stores
        .iter()
        .enumerate()
        .all(|(index, (candidate, width, _))| {
            candidate.destination == store.destination
                && candidate.destination_placement == store.destination_placement
                && valid_store_path(&candidate.path)
                && candidate
                    .field_byte_offset
                    .checked_add(u32::from(*width))
                    .is_some_and(|end| end <= u32::from(parameter.shape.byte_size))
                && function
                    .provenance
                    .operations
                    .contains(&candidate.defining_operation)
                && function
                    .provenance
                    .operations
                    .contains(&candidate.psi_operation)
                && candidate.psi_operation != *read_operation
                && !stores[..index].iter().any(|earlier| {
                    earlier.psi_operation == candidate.psi_operation
                        || earlier.defining_operation == candidate.defining_operation
                        || earlier.source_value == candidate.source_value
                        || (earlier.path == candidate.path && earlier.field == candidate.field)
                })
        });
    if !store.destination.is_self
        || function.attachment != Some(store.destination.structural_type)
        || !matches!(
            store.destination.multiplicity,
            psi_terminal::StructuralMultiplicity::Unrestricted
                | psi_terminal::StructuralMultiplicity::Affine
        )
        || store.destination.access != psi_terminal::StructuralAccess::MutableBorrow
        || !store.destination.qualifications.is_empty()
        || !store.destination.projected_qualifications.is_empty()
        || !valid_stores
        || parameter.shape.class != ValueClass::BorrowedReference
        || !matches!(
            parameter.placement.locations.as_slice(),
            [ValueLocation::Indirect {
                copy_stack_byte_offset: None,
                ..
            }]
        )
        || !exact_return
        || !frame.register_spills.is_empty()
        || frame.byte_size != 0
        || !function.provenance.operations.contains(read_operation)
        || !function.provenance.edges.contains(psi_edge)
    {
        return Err(invalid());
    }
    let mut bytes = Vec::new();
    let mut store_records = Vec::with_capacity(stores.len());
    let mut semantic_code_attribution = Vec::with_capacity(stores.len() * 2 + 2);
    for (index, (store, width, bits)) in prepared_stores.into_iter().enumerate() {
        let code_offset = bytes.len();
        match target.architecture {
            Architecture::X86_64 => emit_x86_store(&mut bytes, parameter, store, width, bits)?,
            Architecture::Aarch64 => emit_aarch64_store(&mut bytes, parameter, store, width, bits)?,
        }
        let store_bytes = bytes[code_offset..].to_vec();
        let store_byte_count = store_bytes.len();
        let defining_ordinal = index.checked_mul(2).ok_or_else(invalid)?;
        let store_ordinal = defining_ordinal.checked_add(1).ok_or_else(invalid)?;
        semantic_code_attribution.push(SemanticCodeAttribution {
            site: SemanticCodeSite::Operation(store.defining_operation),
            operation_ordinal: defining_ordinal,
            code_offset,
            byte_count: 0,
        });
        semantic_code_attribution.push(SemanticCodeAttribution {
            site: SemanticCodeSite::Operation(store.psi_operation),
            operation_ordinal: store_ordinal,
            code_offset,
            byte_count: store_byte_count,
        });
        store_records.push(ScalarStructuralScalarFieldStoreRecord {
            psi_operation: store.psi_operation,
            destination: store.destination.clone(),
            path: store.path.clone(),
            field: store.field,
            destination_placement: store.destination_placement.clone(),
            field_byte_offset: store.field_byte_offset,
            defining_operation: store.defining_operation,
            source_value: store.source_value,
            immediate: store.immediate,
            return_operation: *read_operation,
            return_source_value: *return_source_value,
            return_field: *return_field,
            return_field_byte_offset: *return_field_byte_offset,
            return_scalar_type,
            operation_ordinal: store_ordinal,
            code_offset,
            byte_count: store_byte_count,
            bytes: store_bytes,
        });
    }
    let stores_byte_count = bytes.len();
    let mut internal_calls = Vec::new();
    let scalar_bytes = match (target.architecture, scalar) {
        (Architecture::X86_64, AssignedOperation::ReturnBooleanExpression { expression, .. }) => {
            emit_x86_64_boolean_expression(frame, expression, Some((&mut internal_calls, target)))?
        }
        (Architecture::Aarch64, AssignedOperation::ReturnBooleanExpression { expression, .. }) => {
            emit_aarch64_boolean_expression(frame, expression, Some((&mut internal_calls, target)))?
        }
        (
            Architecture::X86_64,
            AssignedOperation::ReturnIntegerExpression {
                scalar_type,
                expression,
                ..
            },
        ) => emit_x86_64_integer_expression(
            *scalar_type,
            frame,
            expression,
            Some((&mut internal_calls, target)),
        )?,
        (
            Architecture::Aarch64,
            AssignedOperation::ReturnIntegerExpression {
                scalar_type,
                expression,
                ..
            },
        ) => emit_aarch64_integer_expression(
            *scalar_type,
            frame,
            expression,
            Some((&mut internal_calls, target)),
        )?,
        _ => unreachable!("scalar-store result shape was checked above"),
    };
    if !internal_calls.is_empty() {
        return Err(invalid());
    }
    let return_byte_count = match target.architecture {
        Architecture::X86_64 => 1,
        Architecture::Aarch64 => 4,
    };
    let read_byte_count = scalar_bytes
        .len()
        .checked_sub(return_byte_count)
        .ok_or_else(invalid)?;
    bytes.extend_from_slice(&scalar_bytes);
    let read_ordinal = stores.len().checked_mul(2).ok_or_else(invalid)?;
    let return_ordinal = read_ordinal.checked_add(1).ok_or_else(invalid)?;
    semantic_code_attribution.extend([
        SemanticCodeAttribution {
            site: SemanticCodeSite::Operation(*read_operation),
            operation_ordinal: read_ordinal,
            code_offset: stores_byte_count,
            byte_count: read_byte_count,
        },
        SemanticCodeAttribution {
            site: SemanticCodeSite::Edge(*psi_edge),
            operation_ordinal: return_ordinal,
            code_offset: stores_byte_count + read_byte_count,
            byte_count: return_byte_count,
        },
    ]);
    Ok(EmittedScalarStore {
        bytes,
        stores: store_records,
        semantic_code_attribution,
    })
}

fn valid_store_path(path: &[psi_terminal::StructuralPathSegment]) -> bool {
    path.iter().all(
        |segment| matches!(segment, psi_terminal::StructuralPathSegment::Field(identity) if !identity.is_empty()),
    )
}

fn emit_x86_store(
    bytes: &mut Vec<u8>,
    parameter: &TargetStructuralParameter,
    store: &TargetScalarStructuralFieldStore,
    width: u16,
    bits: u64,
) -> Result<(), EmissionError> {
    const ADDRESS_REGISTER: u8 = 10;
    const VALUE_REGISTER: u8 = 11;
    bytes.extend_from_slice(&[0x49, 0xb8 | (VALUE_REGISTER & 7)]);
    bytes.extend_from_slice(&bits.to_le_bytes());
    let [ValueLocation::Indirect { pointer, .. }] = parameter.placement.locations.as_slice() else {
        return Err(EmissionError::InvalidStructuralScalarFieldStoreCustody(
            store.psi_operation,
        ));
    };
    let base = match *pointer {
        IndirectPointerLocation::Register(register) => x86_unit_register(register)?,
        IndirectPointerLocation::Stack {
            stack_byte_offset, ..
        } => {
            let incoming = stack_byte_offset.checked_add(8).ok_or(
                EmissionError::IncomingStackOffsetNotEncodable {
                    value: store.source_value,
                    byte_offset: stack_byte_offset,
                },
            )?;
            emit_x86_64_stack_load_width(bytes, ADDRESS_REGISTER, incoming, 8)?;
            ADDRESS_REGISTER
        }
    };
    emit_x86_64_memory_store_width(bytes, VALUE_REGISTER, base, store.field_byte_offset, width)
}

fn emit_aarch64_store(
    bytes: &mut Vec<u8>,
    parameter: &TargetStructuralParameter,
    store: &TargetScalarStructuralFieldStore,
    width: u16,
    bits: u64,
) -> Result<(), EmissionError> {
    const ADDRESS_REGISTER: u8 = 17;
    const VALUE_REGISTER: u8 = 16;
    let [ValueLocation::Indirect { pointer, .. }] = parameter.placement.locations.as_slice() else {
        return Err(EmissionError::InvalidStructuralScalarFieldStoreCustody(
            store.psi_operation,
        ));
    };
    let mut instructions = Vec::new();
    emit_aarch64_unit_immediate(&mut instructions, VALUE_REGISTER, bits);
    let base = match *pointer {
        IndirectPointerLocation::Register(register) => aarch64_unit_register(register)?,
        IndirectPointerLocation::Stack {
            stack_byte_offset, ..
        } => {
            instructions.push(aarch64_unit_stack_access(
                aarch64_load_base(8)?,
                ADDRESS_REGISTER,
                stack_byte_offset,
                8,
            )?);
            ADDRESS_REGISTER
        }
    };
    instructions.push(aarch64_unit_memory_access(
        aarch64_store_base(width)?,
        VALUE_REGISTER,
        base,
        store.field_byte_offset,
        width,
    )?);
    append_aarch64_instructions(bytes, instructions);
    Ok(())
}
