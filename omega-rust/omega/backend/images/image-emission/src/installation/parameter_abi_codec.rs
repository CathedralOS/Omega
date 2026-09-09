//! Canonical incoming scalar parameters and their complete function call plan.

use machine_code::{ParameterFunctionAbiRecord, UnitEntryRegisterSpillRecord};
use semantic_vocabulary::ValueId;
use target_operations::ScalarAbiValue;

use super::{
    InstallationError, Reader, push_u32, push_u64,
    scalar_call_plan_codec::{decode_scalar_call_plan, encode_scalar_call_plan},
    unit_scalar_codec::{decode_scalar_type, encode_scalar_type},
    value_placement_codec::{decode_direct_placement, encode_direct_placement},
    value_placement_codec::{decode_register, register_tag},
};

pub(super) fn encode_parameter_abi(
    bytes: &mut Vec<u8>,
    abi: Option<&ParameterFunctionAbiRecord>,
) -> Result<(), InstallationError> {
    let Some(abi) = abi else {
        bytes.extend_from_slice(&[0; 4]);
        return Ok(());
    };
    bytes.extend_from_slice(&[1, 0, 0, 0]);
    encode_scalar_call_plan(bytes, &abi.call_plan)?;
    push_u32(
        bytes,
        u32::try_from(abi.parameters.len())
            .map_err(|_| InstallationError::TooManyScalarCallPlanValues)?,
    );
    for parameter in &abi.parameters {
        push_u64(bytes, parameter.value.get());
        encode_scalar_type(bytes, parameter.scalar_type)?;
        encode_direct_placement(bytes, &parameter.placement)?;
    }
    push_u32(
        bytes,
        u32::try_from(abi.entry_register_spills.len())
            .map_err(|_| InstallationError::TooManyScalarCallPlanValues)?,
    );
    for spill in &abi.entry_register_spills {
        push_u64(bytes, spill.source_value.get());
        push_u32(
            bytes,
            u32::try_from(spill.parameter_index)
                .map_err(|_| InstallationError::TooManyScalarCallPlanValues)?,
        );
        bytes.push(register_tag(spill.register)?);
        bytes.extend_from_slice(&[0; 3]);
        push_u32(bytes, spill.byte_offset);
        push_u64(
            bytes,
            u64::try_from(spill.code_offset)
                .map_err(|_| InstallationError::InstalledScalarOffsetNotRepresentable)?,
        );
        push_u64(
            bytes,
            u64::try_from(spill.byte_count)
                .map_err(|_| InstallationError::InstalledScalarOffsetNotRepresentable)?,
        );
    }
    Ok(())
}

pub(super) fn decode_parameter_abi(
    reader: &mut Reader<'_>,
) -> Result<Option<ParameterFunctionAbiRecord>, InstallationError> {
    match reader.u8()? {
        0 => {
            if reader.take(3)? != [0; 3] {
                return Err(InstallationError::NonzeroReservedField);
            }
            Ok(None)
        }
        1 => {
            if reader.take(3)? != [0; 3] {
                return Err(InstallationError::NonzeroReservedField);
            }
            let call_plan = decode_scalar_call_plan(reader)?;
            let parameter_count = usize::try_from(reader.u32()?)
                .map_err(|_| InstallationError::TooManyScalarCallPlanValues)?;
            if parameter_count > reader.remaining() / 20 {
                return Err(InstallationError::UnexpectedEnd);
            }
            let mut parameters = Vec::with_capacity(parameter_count);
            for _ in 0..parameter_count {
                parameters.push(ScalarAbiValue {
                    value: ValueId::new(reader.u64()?)
                        .ok_or(InstallationError::ZeroInstalledScalarIdentity)?,
                    scalar_type: decode_scalar_type(reader)?,
                    placement: decode_direct_placement(reader)?,
                });
            }
            Ok(Some(ParameterFunctionAbiRecord {
                call_plan,
                parameters,
                entry_register_spills: decode_entry_spills(reader)?,
            }))
        }
        tag => Err(InstallationError::InvalidPresenceFlag(tag)),
    }
}

fn decode_entry_spills(
    reader: &mut Reader<'_>,
) -> Result<Vec<UnitEntryRegisterSpillRecord>, InstallationError> {
    let count = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::TooManyScalarCallPlanValues)?;
    if count > reader.remaining() / 36 {
        return Err(InstallationError::UnexpectedEnd);
    }
    (0..count)
        .map(|_| {
            let source_value = ValueId::new(reader.u64()?)
                .ok_or(InstallationError::ZeroInstalledScalarIdentity)?;
            let parameter_index = usize::try_from(reader.u32()?)
                .map_err(|_| InstallationError::TooManyScalarCallPlanValues)?;
            let register = decode_register(reader.u8()?)?;
            if reader.take(3)? != [0; 3] {
                return Err(InstallationError::NonzeroReservedField);
            }
            Ok(UnitEntryRegisterSpillRecord {
                source_value,
                parameter_index,
                register,
                byte_offset: reader.u32()?,
                code_offset: usize::try_from(reader.u64()?)
                    .map_err(|_| InstallationError::InstalledScalarOffsetNotRepresentable)?,
                byte_count: usize::try_from(reader.u64()?)
                    .map_err(|_| InstallationError::InstalledScalarOffsetNotRepresentable)?,
            })
        })
        .collect()
}
