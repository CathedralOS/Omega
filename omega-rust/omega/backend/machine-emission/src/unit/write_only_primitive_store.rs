//! Exact machine emission for a whole-root non-observing primitive store.

use std::collections::BTreeMap;

use assigned_target_operations::{
    AssignedScalarLocation, AssignedUnitBody, AssignedUnitOperation,
    AssignedUnitWriteOnlyPrimitiveStoreSource,
};
use calling_conventions::{
    CallSignature, CallingPolicy, ValueLocation, ValueShape, evaluate_call_plan,
};
use machine_code::{UnitWriteOnlyPrimitiveStoreRecord, UnitWriteOnlyPrimitiveStoreSourceRecord};
use semantic_vocabulary::{IntegerType, IntegerValue, OperationId, ScalarType, ValueId};
use target::{Architecture, NativeTarget};
use terminal_psi::{StructuralAccess, StructuralMultiplicity, StructuralTypeShape};

use super::structural_scalar::{
    emit_aarch64_unit_store_immediate, emit_aarch64_unit_store_register,
    emit_x86_64_unit_store_immediate, emit_x86_64_unit_store_register,
};
use super::{
    Aarch64UnitStructuralHome, X86UnitStructuralHome, aarch64_load_base, aarch64_unit_stack_access,
    append_aarch64_instructions, emit_x86_64_stack_load_width, unit_scalar_home_record,
    unit_scalar_shape,
};
use crate::{EmissionError, integer_bits, require_native_integer_width};

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_write_only_primitive_store(
    operation: &AssignedUnitOperation,
    body: &AssignedUnitBody,
    target: NativeTarget,
    x86_homes: &[X86UnitStructuralHome],
    aarch64_homes: &[Aarch64UnitStructuralHome],
    x86_frame_bytes: u32,
    aarch64_frame_bytes: u32,
    established_integer_constants: &BTreeMap<ValueId, (OperationId, IntegerType, IntegerValue)>,
    established_boolean_constants: &BTreeMap<ValueId, (OperationId, bool, usize)>,
    established_ieee_float_constants: &BTreeMap<
        ValueId,
        (OperationId, semantic_vocabulary::IeeeFloatValue, usize),
    >,
    bytes: &mut Vec<u8>,
    operation_ordinal: usize,
    code_offset: usize,
) -> Result<UnitWriteOnlyPrimitiveStoreRecord, EmissionError> {
    let AssignedUnitOperation::WriteOnlyPrimitiveStore {
        psi_operation,
        destination,
        destination_type,
        destination_placement,
        source,
    } = operation
    else {
        unreachable!("whole-root store router supplied another operation")
    };
    let invalid = || EmissionError::InvalidWriteOnlyPrimitiveStoreCustody(*psi_operation);
    let (source_record, destination_scalar_type, byte_size, immediate_bits) = match *source {
        AssignedUnitWriteOnlyPrimitiveStoreSource::Parameter {
            parameter_index,
            source_value,
            scalar_type,
            location,
        } => {
            let scalar_parameter_index = usize::try_from(parameter_index).map_err(|_| invalid())?;
            let scalar_parameter = body
                .scalar_parameters
                .get(scalar_parameter_index)
                .ok_or_else(invalid)?;
            let (scalar_shape, byte_size) = match scalar_type {
                ScalarType::Boolean => (ValueShape::integer(1, 1), 1),
                ScalarType::Integer(integer) => {
                    let byte_size = require_native_integer_width(source_value, integer)? / 8;
                    (ValueShape::integer(byte_size, byte_size.min(8)), byte_size)
                }
                ScalarType::IeeeFloat(_) => return Err(invalid()),
            };
            let mut parameter_shapes = body
                .scalar_parameters
                .iter()
                .map(|parameter| {
                    let shape = unit_scalar_shape(parameter.value, parameter.scalar_type)?;
                    if parameter.placement.shape != shape {
                        return Err(invalid());
                    }
                    Ok(shape)
                })
                .collect::<Result<Vec<_>, _>>()?;
            parameter_shapes.push(destination_placement.shape);
            let expected_plan = evaluate_call_plan(
                CallingPolicy::native_for_target(target),
                &CallSignature {
                    parameters: parameter_shapes,
                    result: None,
                },
            )
            .map_err(|_| invalid())?;
            let (expected_location, placed_byte_size) =
                match scalar_parameter.placement.locations.as_slice() {
                    [
                        ValueLocation::Register {
                            register,
                            value_byte_offset: 0,
                            byte_size,
                        },
                    ] => (AssignedScalarLocation::Register(*register), *byte_size),
                    [
                        ValueLocation::Stack {
                            stack_byte_offset,
                            value_byte_offset: 0,
                            byte_size,
                            ..
                        },
                    ] => (
                        AssignedScalarLocation::IncomingStack {
                            byte_offset: *stack_byte_offset,
                        },
                        *byte_size,
                    ),
                    _ => return Err(invalid()),
                };
            if body.parameters.len() != 1
                || scalar_parameter.value != source_value
                || scalar_parameter.scalar_type != scalar_type
                || scalar_parameter.placement.shape != scalar_shape
                || placed_byte_size != byte_size
                || location != expected_location
                || body.call_plan != expected_plan
                || body.call_plan.parameters.get(scalar_parameter_index)
                    != Some(&scalar_parameter.placement)
                || body.call_plan.parameters.get(body.scalar_parameters.len())
                    != Some(destination_placement)
            {
                return Err(invalid());
            }
            (
                UnitWriteOnlyPrimitiveStoreSourceRecord::Parameter {
                    parameter_index,
                    source_value,
                    scalar_type,
                    location: match expected_location {
                        AssignedScalarLocation::Register(register) => {
                            machine_code::UnitScalarParameterLocationRecord::Register(register)
                        }
                        AssignedScalarLocation::IncomingStack { byte_offset } => {
                            machine_code::UnitScalarParameterLocationRecord::IncomingStack {
                                byte_offset,
                            }
                        }
                        AssignedScalarLocation::FrameSpill { .. } => unreachable!(),
                    },
                },
                scalar_type,
                byte_size,
                None,
            )
        }
        AssignedUnitWriteOnlyPrimitiveStoreSource::IntegerImmediate {
            defining_operation,
            source_value,
            scalar_type,
            value,
        } => {
            if established_integer_constants.get(&source_value)
                != Some(&(defining_operation, scalar_type, value))
            {
                return Err(invalid());
            }
            let byte_size = require_native_integer_width(source_value, scalar_type)? / 8;
            (
                UnitWriteOnlyPrimitiveStoreSourceRecord::IntegerImmediate {
                    defining_operation,
                    source_value,
                    scalar_type,
                    value,
                },
                ScalarType::Integer(scalar_type),
                byte_size,
                Some(integer_bits(source_value, scalar_type, value)?),
            )
        }
        AssignedUnitWriteOnlyPrimitiveStoreSource::BooleanImmediate {
            defining_operation,
            source_value,
            value,
        } => {
            let Some((retained_operation, retained_value, definition_ordinal)) =
                established_boolean_constants.get(&source_value).copied()
            else {
                return Err(invalid());
            };
            if retained_operation != defining_operation || retained_value != value {
                return Err(invalid());
            }
            (
                UnitWriteOnlyPrimitiveStoreSourceRecord::BooleanImmediate {
                    defining_operation,
                    source_value,
                    value,
                    definition_ordinal,
                },
                ScalarType::Boolean,
                1,
                Some(u64::from(value)),
            )
        }
        AssignedUnitWriteOnlyPrimitiveStoreSource::IeeeFloatImmediate {
            defining_operation,
            source_value,
            value,
        } => {
            let Some((retained_operation, retained_value, definition_ordinal)) =
                established_ieee_float_constants.get(&source_value).copied()
            else {
                return Err(invalid());
            };
            if retained_operation != defining_operation || retained_value != value {
                return Err(invalid());
            }
            let (byte_size, bits) = match value {
                semantic_vocabulary::IeeeFloatValue::Binary32(bits) => (4, u64::from(bits)),
                semantic_vocabulary::IeeeFloatValue::Binary64(bits) => (8, bits),
            };
            (
                UnitWriteOnlyPrimitiveStoreSourceRecord::IeeeFloatImmediate {
                    defining_operation,
                    source_value,
                    value,
                    definition_ordinal,
                },
                ScalarType::IeeeFloat(value.format()),
                byte_size,
                Some(bits),
            )
        }
        AssignedUnitWriteOnlyPrimitiveStoreSource::Home(home) => {
            let exact_source_count = body.operations[..operation_ordinal]
                .iter()
                .filter(|operation| {
                    matches!(
                        operation,
                        AssignedUnitOperation::ScalarCall { result_home, .. }
                            if *result_home == home
                    )
                })
                .count();
            let ScalarType::Integer(integer) = home.scalar_type else {
                return Err(invalid());
            };
            if exact_source_count != 1
                || home.shape != unit_scalar_shape(home.source_value, home.scalar_type)?
            {
                return Err(invalid());
            }
            let byte_size = require_native_integer_width(home.source_value, integer)? / 8;
            (
                UnitWriteOnlyPrimitiveStoreSourceRecord::Home(unit_scalar_home_record(home)),
                home.scalar_type,
                byte_size,
                None,
            )
        }
    };
    let parameter_index = usize::try_from(destination.position).map_err(|_| invalid())?;
    let parameter = body.parameters.get(parameter_index).ok_or_else(invalid)?;
    let expected_shape = ValueShape::borrowed_reference(byte_size, byte_size.min(8));
    if destination.is_self
        || destination.multiplicity != StructuralMultiplicity::Unrestricted
        || !matches!(
            destination.access,
            StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
        )
        || !destination.qualifications.is_empty()
        || !destination.projected_qualifications.is_empty()
        || destination_type.id != destination.structural_type
        || destination_type.shape != StructuralTypeShape::PrimitiveScalar(destination_scalar_type)
        || !body
            .structural_types
            .iter()
            .any(|candidate| candidate == destination_type)
        || parameter.place != destination.place
        || parameter.structural_type != destination.structural_type
        || parameter.multiplicity != destination.multiplicity
        || parameter.access != destination.access
        || parameter.projected_qualifications != destination.projected_qualifications
        || parameter.shape != expected_shape
        || &parameter.placement != destination_placement
    {
        return Err(invalid());
    }
    let (parameter_home_byte_offset, parameter_home_indirect) = match target.architecture {
        Architecture::X86_64 => {
            let home = x86_homes
                .iter()
                .find(|home| home.place == destination.place)
                .ok_or(EmissionError::MissingUnitParameterHome(destination.place))?;
            if home.source != *destination_placement || home.shape != parameter.shape {
                return Err(EmissionError::UnitParameterHomeMismatch(destination.place));
            }
            match source_record {
                UnitWriteOnlyPrimitiveStoreSourceRecord::Parameter {
                    location: machine_code::UnitScalarParameterLocationRecord::Register(register),
                    ..
                } => emit_x86_64_unit_store_register(bytes, home, 0, byte_size, register)?,
                UnitWriteOnlyPrimitiveStoreSourceRecord::Parameter {
                    location:
                        machine_code::UnitScalarParameterLocationRecord::IncomingStack { byte_offset },
                    ..
                } => {
                    let source_offset = x86_frame_bytes
                        .checked_add(8)
                        .and_then(|offset| offset.checked_add(byte_offset))
                        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
                    emit_x86_64_stack_load_width(bytes, 11, source_offset, byte_size)?;
                    emit_x86_64_unit_store_register(
                        bytes,
                        home,
                        0,
                        byte_size,
                        target_operations::MachineRegister::X86R11,
                    )?;
                }
                UnitWriteOnlyPrimitiveStoreSourceRecord::Home(source_home) => {
                    emit_x86_64_stack_load_width(bytes, 11, source_home.byte_offset, 8)?;
                    emit_x86_64_unit_store_register(
                        bytes,
                        home,
                        0,
                        byte_size,
                        target_operations::MachineRegister::X86R11,
                    )?;
                }
                _ => emit_x86_64_unit_store_immediate(
                    bytes,
                    home,
                    0,
                    byte_size,
                    immediate_bits.ok_or_else(invalid)?,
                )?,
            }
            (home.byte_offset, home.indirect)
        }
        Architecture::Aarch64 => {
            let home = aarch64_homes
                .iter()
                .find(|home| home.place == destination.place)
                .ok_or(EmissionError::MissingUnitParameterHome(destination.place))?;
            if home.source != *destination_placement || home.shape != parameter.shape {
                return Err(EmissionError::UnitParameterHomeMismatch(destination.place));
            }
            match source_record {
                UnitWriteOnlyPrimitiveStoreSourceRecord::Parameter {
                    location: machine_code::UnitScalarParameterLocationRecord::Register(register),
                    ..
                } => emit_aarch64_unit_store_register(bytes, home, 0, byte_size, register)?,
                UnitWriteOnlyPrimitiveStoreSourceRecord::Parameter {
                    location:
                        machine_code::UnitScalarParameterLocationRecord::IncomingStack { byte_offset },
                    ..
                } => {
                    let source_offset = aarch64_frame_bytes
                        .checked_add(byte_offset)
                        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
                    let instruction = aarch64_unit_stack_access(
                        aarch64_load_base(byte_size)?,
                        16,
                        source_offset,
                        byte_size,
                    )?;
                    append_aarch64_instructions(bytes, vec![instruction]);
                    emit_aarch64_unit_store_register(
                        bytes,
                        home,
                        0,
                        byte_size,
                        target_operations::MachineRegister::Aarch64X(16),
                    )?;
                }
                UnitWriteOnlyPrimitiveStoreSourceRecord::Home(source_home) => {
                    let instruction = aarch64_unit_stack_access(
                        aarch64_load_base(8)?,
                        16,
                        source_home.byte_offset,
                        8,
                    )?;
                    append_aarch64_instructions(bytes, vec![instruction]);
                    emit_aarch64_unit_store_register(
                        bytes,
                        home,
                        0,
                        byte_size,
                        target_operations::MachineRegister::Aarch64X(16),
                    )?;
                }
                _ => emit_aarch64_unit_store_immediate(
                    bytes,
                    home,
                    0,
                    byte_size,
                    immediate_bits.ok_or_else(invalid)?,
                )?,
            }
            (home.byte_offset, home.indirect)
        }
    };
    Ok(UnitWriteOnlyPrimitiveStoreRecord {
        psi_operation: *psi_operation,
        destination: destination.clone(),
        destination_type: destination_type.clone(),
        destination_placement: destination_placement.clone(),
        source: source_record,
        parameter_home_byte_offset,
        parameter_home_indirect,
        operation_ordinal,
        code_offset,
        byte_count: bytes.len() - code_offset,
        bytes: bytes[code_offset..].to_vec(),
    })
}
