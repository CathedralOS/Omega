//! Independent object replay for fixed-width immediate stores into staged
//! attached-Unit structural parameter homes.

use calling_conventions::{CallSignature, CallingPolicy, ValueLocation, evaluate_call_plan};
use machine_code::{
    InternalUnitScalarArgumentSourceRecord, MachineCodeFunction, SemanticCodeSite,
    UnitStructuralScalarFieldStoreRecord,
};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use target::{Architecture, NativeTarget};
use terminal_psi::StructuralAccess;

use super::instruction_loads::{
    aarch64_terminal_register, expected_aarch64_stack_load, expected_x86_stack_load,
    x86_terminal_register,
};
use super::{ObjectError, ObjectUnitStack};

pub(super) fn validate_unit_structural_scalar_field_stores(
    target: NativeTarget,
    function: &MachineCodeFunction,
    validated_function_stack: Option<&ObjectUnitStack>,
) -> Result<(), ObjectError> {
    let invalid = || ObjectError::InvalidUnitStructuralScalarFieldStoreEvidence(function.machine);
    let mut previous = None;
    for store in &function.unit_structural_scalar_field_stores {
        let key = (store.operation_ordinal, store.code_offset);
        if previous.is_some_and(|previous| previous >= key) {
            return Err(invalid());
        }
        validate_store(target, function, validated_function_stack, store).ok_or_else(invalid)?;
        previous = Some(key);
    }
    Ok(())
}

fn validate_store(
    target: NativeTarget,
    function: &MachineCodeFunction,
    validated_function_stack: Option<&ObjectUnitStack>,
    store: &UnitStructuralScalarFieldStoreRecord,
) -> Option<()> {
    let parameter_index = usize::try_from(store.destination.position).ok()?;
    let parameter = function.unit_parameters.get(parameter_index)?;
    let home = function.unit_parameter_homes.get(parameter_index)?;
    if (store.destination.is_self && function.attachment != Some(store.destination.structural_type))
        || !matches!(
            store.destination.access,
            StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
        )
        || parameter.place != store.destination.place
        || parameter.structural_type != store.destination.structural_type
        || parameter.multiplicity != store.destination.multiplicity
        || parameter.access != store.destination.access
        || home.place != parameter.place
        || home.structural_type != parameter.structural_type
        || home.multiplicity != parameter.multiplicity
        || home.access != parameter.access
        || home.shape != parameter.shape
        || store.destination_placement != home.source
        || Some(store.parameter_home_byte_offset) != home.location.stack_byte_offset()
        || store.parameter_home_indirect != home.indirect
        || !terminal_psi::is_bounded_structural_scalar_store_path(&store.path)
        || !function
            .provenance
            .operations
            .contains(&store.psi_operation)
        || exact_attribution_count(function, store) != 1
    {
        return None;
    }
    let (source_is_exact, byte_size, bits) = match store.source {
        InternalUnitScalarArgumentSourceRecord::Parameter {
            parameter_index,
            source_value,
            scalar_type,
            location,
        } => {
            let abi = function.unit_scalar_abi.as_ref()?;
            let scalar_parameter_index = usize::try_from(parameter_index).ok()?;
            let scalar_parameter = abi.parameters.get(scalar_parameter_index)?;
            let (scalar_shape, byte_size) =
                super::unit_write_only_primitive_store::native_scalar_shape(scalar_type)?;
            let (expected_location, placed_byte_size) =
                match scalar_parameter.placement.locations.as_slice() {
                    [
                        ValueLocation::Register {
                            register,
                            value_byte_offset: 0,
                            byte_size,
                        },
                    ] => (
                        machine_code::UnitScalarParameterLocationRecord::Register(*register),
                        *byte_size,
                    ),
                    [
                        ValueLocation::Stack {
                            stack_byte_offset,
                            value_byte_offset: 0,
                            byte_size,
                            ..
                        },
                    ] => (
                        machine_code::UnitScalarParameterLocationRecord::IncomingStack {
                            byte_offset: *stack_byte_offset,
                        },
                        *byte_size,
                    ),
                    _ => return None,
                };
            let mut parameter_shapes = abi
                .parameters
                .iter()
                .map(|parameter| {
                    let (shape, _) = super::unit_write_only_primitive_store::native_scalar_shape(
                        parameter.scalar_type,
                    )?;
                    (parameter.placement.shape == shape).then_some(shape)
                })
                .collect::<Option<Vec<_>>>()?;
            parameter_shapes.push(parameter.shape);
            let expected_plan = evaluate_call_plan(
                CallingPolicy::native_for_target(target),
                &CallSignature {
                    parameters: parameter_shapes,
                    result: None,
                },
            )
            .ok()?;
            (
                function.unit_parameters.len() == 1
                    && scalar_parameter.value == source_value
                    && scalar_parameter.scalar_type == scalar_type
                    && scalar_parameter.placement.shape == scalar_shape
                    && placed_byte_size == byte_size
                    && location == expected_location
                    && abi.call_plan == expected_plan
                    && abi.call_plan.parameters.get(scalar_parameter_index)
                        == Some(&scalar_parameter.placement)
                    && abi.call_plan.parameters.get(abi.parameters.len()) == Some(&home.source),
                byte_size,
                None,
            )
        }
        InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
            defining_operation,
            source_value,
            scalar_type,
            value,
        } => {
            let source_count = function
                .unit_integer_constants
                .iter()
                .filter(|constant| {
                    constant.defining_operation == defining_operation
                        && constant.source_value == source_value
                        && constant.scalar_type == scalar_type
                        && constant.value == value
                        && constant.operation_ordinal < store.operation_ordinal
                })
                .count();
            let byte_size = scalar_type.bits().checked_div(8)?;
            (
                source_count == 1
                    && matches!(scalar_type.bits(), 8 | 16 | 32 | 64)
                    && !scalar_type.is_address()
                    && scalar_type.admits(value),
                byte_size,
                Some(integer_bits(scalar_type, value)?),
            )
        }
        InternalUnitScalarArgumentSourceRecord::BooleanImmediate {
            defining_operation,
            source_value,
            value,
            definition_ordinal,
        } => {
            let definition_count = function
                .semantic_code_attribution
                .iter()
                .filter(|row| {
                    row.site == SemanticCodeSite::Operation(defining_operation)
                        && row.operation_ordinal == definition_ordinal
                        && row.code_offset <= store.code_offset
                        && row.byte_count == 0
                })
                .count();
            (
                definition_ordinal < store.operation_ordinal
                    && definition_count == 1
                    && function
                        .provenance
                        .operations
                        .iter()
                        .filter(|operation| **operation == defining_operation)
                        .count()
                        == 1
                    && function.unit_integer_constants.iter().all(|constant| {
                        constant.defining_operation != defining_operation
                            && constant.source_value != source_value
                    })
                    && function.unit_scalar_homes.iter().all(|home| {
                        home.defining_operation != defining_operation
                            && home.source_value != source_value
                    })
                    && zero_code_source_is_consistent(function, store.source),
                1,
                Some(u64::from(value)),
            )
        }
        InternalUnitScalarArgumentSourceRecord::Home(source_home) => {
            if !matches!(
                source_home.scalar_type,
                semantic_vocabulary::ScalarType::Integer(_)
            ) {
                return None;
            }
            let source_count = function
                .internal_unit_scalar_calls
                .iter()
                .filter(|call| {
                    call.result.home == source_home
                        && call.operation_ordinal < store.operation_ordinal
                })
                .count();
            let home_count = function
                .unit_scalar_homes
                .iter()
                .filter(|home| **home == source_home)
                .count();
            let (shape, byte_size) = super::unit_write_only_primitive_store::native_scalar_shape(
                source_home.scalar_type,
            )?;
            (
                source_count == 1 && home_count == 1 && source_home.shape == shape,
                byte_size,
                None,
            )
        }
    };
    if !source_is_exact
        || store
            .field_byte_offset
            .checked_add(u32::from(byte_size))
            .is_none_or(|end| end > u32::from(parameter.shape.byte_size))
    {
        return None;
    }
    let expected = match store.source {
        InternalUnitScalarArgumentSourceRecord::Parameter {
            location: machine_code::UnitScalarParameterLocationRecord::Register(register),
            ..
        } => expected_parameter_store_bytes(
            target,
            home,
            store.field_byte_offset,
            byte_size,
            register,
        )?,
        InternalUnitScalarArgumentSourceRecord::Parameter {
            location: machine_code::UnitScalarParameterLocationRecord::IncomingStack { byte_offset },
            ..
        } => expected_incoming_parameter_store_bytes(
            target,
            home,
            store.field_byte_offset,
            byte_size,
            byte_offset,
            validated_function_stack?.frame_bytes,
        )?,
        InternalUnitScalarArgumentSourceRecord::Home(source_home) => expected_home_store_bytes(
            target,
            home,
            store.field_byte_offset,
            byte_size,
            source_home,
        )?,
        _ => expected_store_bytes(target, home, store.field_byte_offset, byte_size, bits?)?,
    };
    let end = store.code_offset.checked_add(store.byte_count)?;
    if store.byte_count == 0
        || store.byte_count != expected.len()
        || store.bytes != expected
        || function.bytes.get(store.code_offset..end) != Some(expected.as_slice())
    {
        return None;
    }
    Some(())
}

fn zero_code_source_is_consistent(
    function: &MachineCodeFunction,
    source: InternalUnitScalarArgumentSourceRecord,
) -> bool {
    let InternalUnitScalarArgumentSourceRecord::BooleanImmediate {
        defining_operation,
        source_value,
        ..
    } = source
    else {
        return false;
    };
    function
        .unit_structural_scalar_field_stores
        .iter()
        .all(|candidate| {
            let candidate_source = candidate.source;
            if matches!(
                candidate_source,
                InternalUnitScalarArgumentSourceRecord::BooleanImmediate {
                    defining_operation: candidate_operation,
                    source_value: candidate_value,
                    ..
                } if candidate_operation == defining_operation || candidate_value == source_value
            ) {
                candidate_source == source
            } else {
                true
            }
        })
}

fn exact_attribution_count(
    function: &MachineCodeFunction,
    store: &UnitStructuralScalarFieldStoreRecord,
) -> usize {
    function
        .semantic_code_attribution
        .iter()
        .filter(|row| {
            row.site == SemanticCodeSite::Operation(store.psi_operation)
                && row.operation_ordinal == store.operation_ordinal
                && row.code_offset == store.code_offset
                && row.byte_count == store.byte_count
        })
        .count()
}

pub(crate) fn integer_bits(scalar_type: IntegerType, value: IntegerValue) -> Option<u64> {
    let width = scalar_type.bits();
    let mask = if width == 64 {
        u64::MAX
    } else {
        (1_u64 << width) - 1
    };
    let bits = match (scalar_type.sign(), value) {
        (IntegerSign::Signed, IntegerValue::Signed(value)) => value as u128 as u64,
        (IntegerSign::Unsigned, IntegerValue::Unsigned(value)) => value as u64,
        _ => return None,
    };
    Some(bits & mask)
}

pub(crate) fn expected_store_bytes(
    target: NativeTarget,
    home: &machine_code::UnitParameterHomeRecord,
    field_byte_offset: u32,
    byte_size: u16,
    bits: u64,
) -> Option<Vec<u8>> {
    match target.architecture {
        Architecture::X86_64 => expected_x86_store(home, field_byte_offset, byte_size, bits),
        Architecture::Aarch64 => expected_aarch64_store(home, field_byte_offset, byte_size, bits),
    }
}

pub(crate) fn expected_parameter_store_bytes(
    target: NativeTarget,
    home: &machine_code::UnitParameterHomeRecord,
    field_byte_offset: u32,
    byte_size: u16,
    source: target_operations::MachineRegister,
) -> Option<Vec<u8>> {
    match target.architecture {
        Architecture::X86_64 => {
            const ADDRESS_REGISTER: u8 = 10;
            let source_register = x86_terminal_register(source)?;
            let mut bytes = Vec::new();
            if home.indirect {
                emit_x86_stack_load(
                    &mut bytes,
                    ADDRESS_REGISTER,
                    home.location.stack_byte_offset()?,
                    8,
                )?;
                emit_x86_memory_store(
                    &mut bytes,
                    source_register,
                    ADDRESS_REGISTER,
                    field_byte_offset,
                    byte_size,
                )?;
            } else {
                emit_x86_stack_store(
                    &mut bytes,
                    source_register,
                    home.location
                        .stack_byte_offset()?
                        .checked_add(field_byte_offset)?,
                    byte_size,
                )?;
            }
            Some(bytes)
        }
        Architecture::Aarch64 => {
            const ADDRESS_REGISTER: u8 = 17;
            let source_register = aarch64_terminal_register(source)?;
            let mut instructions = Vec::new();
            if home.indirect {
                instructions.push(aarch64_access(
                    aarch64_load_base(8)?,
                    ADDRESS_REGISTER,
                    31,
                    home.location.stack_byte_offset()?,
                    8,
                )?);
                instructions.push(aarch64_access(
                    aarch64_store_base(byte_size)?,
                    source_register,
                    ADDRESS_REGISTER,
                    field_byte_offset,
                    byte_size,
                )?);
            } else {
                instructions.push(aarch64_access(
                    aarch64_store_base(byte_size)?,
                    source_register,
                    31,
                    home.location
                        .stack_byte_offset()?
                        .checked_add(field_byte_offset)?,
                    byte_size,
                )?);
            }
            Some(
                instructions
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect(),
            )
        }
    }
}

pub(crate) fn expected_incoming_parameter_store_bytes(
    target: NativeTarget,
    home: &machine_code::UnitParameterHomeRecord,
    field_byte_offset: u32,
    byte_size: u16,
    source_stack_byte_offset: u32,
    frame_bytes: u32,
) -> Option<Vec<u8>> {
    match target.architecture {
        Architecture::X86_64 => {
            let incoming = frame_bytes
                .checked_add(8)?
                .checked_add(source_stack_byte_offset)?;
            let mut bytes = Vec::new();
            expected_x86_stack_load(&mut bytes, 11, incoming, byte_size)?;
            bytes.extend(expected_parameter_store_bytes(
                target,
                home,
                field_byte_offset,
                byte_size,
                target_operations::MachineRegister::X86R11,
            )?);
            Some(bytes)
        }
        Architecture::Aarch64 => {
            let incoming = frame_bytes.checked_add(source_stack_byte_offset)?;
            let mut bytes = expected_aarch64_stack_load(16, incoming, byte_size)?
                .to_le_bytes()
                .to_vec();
            bytes.extend(expected_parameter_store_bytes(
                target,
                home,
                field_byte_offset,
                byte_size,
                target_operations::MachineRegister::Aarch64X(16),
            )?);
            Some(bytes)
        }
    }
}

pub(crate) fn expected_home_store_bytes(
    target: NativeTarget,
    destination: &machine_code::UnitParameterHomeRecord,
    field_byte_offset: u32,
    byte_size: u16,
    source: machine_code::UnitScalarHomeRecord,
) -> Option<Vec<u8>> {
    match target.architecture {
        Architecture::X86_64 => {
            let mut bytes = Vec::new();
            expected_x86_stack_load(&mut bytes, 11, source.byte_offset, 8)?;
            bytes.extend(expected_parameter_store_bytes(
                target,
                destination,
                field_byte_offset,
                byte_size,
                target_operations::MachineRegister::X86R11,
            )?);
            Some(bytes)
        }
        Architecture::Aarch64 => {
            let mut bytes = expected_aarch64_stack_load(16, source.byte_offset, 8)?
                .to_le_bytes()
                .to_vec();
            bytes.extend(expected_parameter_store_bytes(
                target,
                destination,
                field_byte_offset,
                byte_size,
                target_operations::MachineRegister::Aarch64X(16),
            )?);
            Some(bytes)
        }
    }
}

fn expected_x86_store(
    home: &machine_code::UnitParameterHomeRecord,
    field_byte_offset: u32,
    byte_size: u16,
    bits: u64,
) -> Option<Vec<u8>> {
    const ADDRESS_REGISTER: u8 = 10;
    const VALUE_REGISTER: u8 = 11;
    let mut bytes = vec![0x49, 0xb8 | (VALUE_REGISTER & 7)];
    bytes.extend_from_slice(&bits.to_le_bytes());
    if home.indirect {
        emit_x86_stack_load(
            &mut bytes,
            ADDRESS_REGISTER,
            home.location.stack_byte_offset()?,
            8,
        )?;
        emit_x86_memory_store(
            &mut bytes,
            VALUE_REGISTER,
            ADDRESS_REGISTER,
            field_byte_offset,
            byte_size,
        )?;
    } else {
        let destination = home
            .location
            .stack_byte_offset()?
            .checked_add(field_byte_offset)?;
        emit_x86_stack_store(&mut bytes, VALUE_REGISTER, destination, byte_size)?;
    }
    Some(bytes)
}

fn emit_x86_stack_load(
    bytes: &mut Vec<u8>,
    register: u8,
    byte_offset: u32,
    byte_size: u16,
) -> Option<()> {
    match byte_size {
        8 => {
            bytes.push(0x48 | (((register >> 3) & 1) << 2));
            bytes.push(0x8b);
        }
        _ => return None,
    }
    emit_x86_rsp_modrm(bytes, register, byte_offset);
    Some(())
}

fn emit_x86_stack_store(
    bytes: &mut Vec<u8>,
    register: u8,
    byte_offset: u32,
    byte_size: u16,
) -> Option<()> {
    emit_x86_width_prefix(bytes, register, 0, byte_size)?;
    bytes.push(if byte_size == 1 { 0x88 } else { 0x89 });
    emit_x86_rsp_modrm(bytes, register, byte_offset);
    Some(())
}

fn emit_x86_memory_store(
    bytes: &mut Vec<u8>,
    source: u8,
    base: u8,
    byte_offset: u32,
    byte_size: u16,
) -> Option<()> {
    emit_x86_width_prefix(bytes, source, base, byte_size)?;
    bytes.push(if byte_size == 1 { 0x88 } else { 0x89 });
    if byte_offset == 0 && (base & 7) != 5 {
        bytes.push(((source & 7) << 3) | (base & 7));
    } else if byte_offset <= i8::MAX as u32 {
        bytes.push(0x40 | ((source & 7) << 3) | (base & 7));
        bytes.push(byte_offset as u8);
    } else {
        bytes.push(0x80 | ((source & 7) << 3) | (base & 7));
        bytes.extend_from_slice(&byte_offset.to_le_bytes());
    }
    Some(())
}

fn emit_x86_width_prefix(
    bytes: &mut Vec<u8>,
    register: u8,
    base: u8,
    byte_size: u16,
) -> Option<()> {
    let extension = (((register >> 3) & 1) << 2) | ((base >> 3) & 1);
    match byte_size {
        1 | 4 => bytes.push(0x40 | extension),
        2 => {
            bytes.push(0x66);
            bytes.push(0x40 | extension);
        }
        8 => bytes.push(0x48 | extension),
        _ => return None,
    }
    Some(())
}

fn emit_x86_rsp_modrm(bytes: &mut Vec<u8>, register: u8, byte_offset: u32) {
    if byte_offset <= i8::MAX as u32 {
        bytes.extend_from_slice(&[0x44 | ((register & 7) << 3), 0x24, byte_offset as u8]);
    } else {
        bytes.extend_from_slice(&[0x84 | ((register & 7) << 3), 0x24]);
        bytes.extend_from_slice(&byte_offset.to_le_bytes());
    }
}

fn expected_aarch64_store(
    home: &machine_code::UnitParameterHomeRecord,
    field_byte_offset: u32,
    byte_size: u16,
    bits: u64,
) -> Option<Vec<u8>> {
    const ADDRESS_REGISTER: u8 = 17;
    const VALUE_REGISTER: u8 = 16;
    let mut instructions = Vec::new();
    for chunk in 0..4 {
        let immediate = ((bits >> (chunk * 16)) & 0xffff) as u32;
        if chunk == 0 || immediate != 0 {
            let base = if chunk == 0 { 0xd280_0000 } else { 0xf280_0000 };
            instructions
                .push(base | ((chunk as u32) << 21) | (immediate << 5) | u32::from(VALUE_REGISTER));
        }
    }
    if home.indirect {
        instructions.push(aarch64_access(
            aarch64_load_base(8)?,
            ADDRESS_REGISTER,
            31,
            home.location.stack_byte_offset()?,
            8,
        )?);
        instructions.push(aarch64_access(
            aarch64_store_base(byte_size)?,
            VALUE_REGISTER,
            ADDRESS_REGISTER,
            field_byte_offset,
            byte_size,
        )?);
    } else {
        instructions.push(aarch64_access(
            aarch64_store_base(byte_size)?,
            VALUE_REGISTER,
            31,
            home.location
                .stack_byte_offset()?
                .checked_add(field_byte_offset)?,
            byte_size,
        )?);
    }
    Some(
        instructions
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect(),
    )
}

fn aarch64_load_base(byte_size: u16) -> Option<u32> {
    match byte_size {
        8 => Some(0xf940_0000),
        _ => None,
    }
}

fn aarch64_store_base(byte_size: u16) -> Option<u32> {
    match byte_size {
        1 => Some(0x3900_0000),
        2 => Some(0x7900_0000),
        4 => Some(0xb900_0000),
        8 => Some(0xf900_0000),
        _ => None,
    }
}

fn aarch64_access(
    base: u32,
    register: u8,
    address_register: u8,
    byte_offset: u32,
    byte_size: u16,
) -> Option<u32> {
    let scale = u32::from(byte_size);
    if scale == 0 || !byte_offset.is_multiple_of(scale) || byte_offset / scale > 0xfff {
        return None;
    }
    Some(
        base | ((byte_offset / scale) << 10)
            | (u32::from(address_register) << 5)
            | u32::from(register),
    )
}
