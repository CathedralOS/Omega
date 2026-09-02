//! Canonical installation transport for attached-Unit fixed-integer calls.
//!
//! These rows contain semantic identities, physical placements, and code
//! intervals only. Expected instruction bytes are deliberately not accepted
//! from the producer.

use omega_machine_code::{
    InternalUnitScalarArgumentSourceRecord, InternalUnitScalarCallArgumentRecord,
    InternalUnitScalarCallRecord, InternalUnitScalarCallResultRecord,
};
use psi_core::{MachineId, OperationId, ValueId};

use super::{
    InstallationError, InstalledInternalUnitScalarCall, Reader,
    call_site_owner_codec::{decode_call_site_owner, encode_call_site_owner},
    push_u32, push_u64,
    scalar_call_plan_codec::{decode_scalar_call_plan, encode_scalar_call_plan},
    unit_scalar_codec::{
        decode_integer_type, decode_integer_value, decode_unit_scalar_home, encode_integer_type,
        encode_integer_value, encode_unit_scalar_home,
    },
    value_placement_codec::{decode_direct_placement, encode_direct_placement},
};

pub(super) fn encode_internal_unit_scalar_calls(
    bytes: &mut Vec<u8>,
    count: u32,
    calls: &[InstalledInternalUnitScalarCall],
) -> Result<(), InstallationError> {
    push_u32(bytes, count);
    for installed in calls {
        push_u64(bytes, installed.machine.get());
        push_u64(
            bytes,
            u64::try_from(installed.text_offset)
                .map_err(|_| InstallationError::InstalledScalarOffsetNotRepresentable)?,
        );
        encode_call_site_owner(bytes, installed.custody.owner);
        push_u64(bytes, installed.custody.target.get());
        encode_scalar_call_plan(bytes, &installed.custody.call_plan)?;
        encode_unit_scalar_home(bytes, installed.custody.result.home)?;
        encode_direct_placement(bytes, &installed.custody.result.source)?;
        encode_offset(bytes, installed.custody.result.code_offset)?;
        encode_offset(bytes, installed.custody.result.byte_count)?;
        push_u32(
            bytes,
            u32::try_from(installed.custody.arguments.len())
                .map_err(|_| InstallationError::TooManyInternalUnitScalarCallArguments)?,
        );
        for argument in &installed.custody.arguments {
            push_u32(bytes, argument.parameter_index);
            encode_argument_source(bytes, argument.source)?;
            encode_direct_placement(bytes, &argument.destination)?;
            encode_offset(bytes, argument.code_offset)?;
            encode_offset(bytes, argument.byte_count)?;
        }
        encode_offset(bytes, installed.custody.operation_ordinal)?;
        encode_offset(bytes, installed.custody.code_offset)?;
        encode_offset(bytes, installed.custody.byte_count)?;
    }
    Ok(())
}

pub(super) fn decode_internal_unit_scalar_calls(
    reader: &mut Reader<'_>,
) -> Result<Vec<InstalledInternalUnitScalarCall>, InstallationError> {
    let count = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::TooManyInternalUnitScalarCalls)?;
    if count > reader.remaining() / 96 {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut calls = Vec::with_capacity(count);
    for _ in 0..count {
        calls.push(decode_internal_unit_scalar_call(reader)?);
    }
    Ok(calls)
}

fn decode_internal_unit_scalar_call(
    reader: &mut Reader<'_>,
) -> Result<InstalledInternalUnitScalarCall, InstallationError> {
    let machine =
        MachineId::new(reader.u64()?).ok_or(InstallationError::ZeroInstalledScalarIdentity)?;
    let text_offset = decode_offset(reader)?;
    let owner = decode_call_site_owner(reader)?;
    let target =
        MachineId::new(reader.u64()?).ok_or(InstallationError::ZeroInstalledScalarIdentity)?;
    let call_plan = decode_scalar_call_plan(reader)?;
    let result = InternalUnitScalarCallResultRecord {
        home: decode_unit_scalar_home(reader)?,
        source: decode_direct_placement(reader)?,
        code_offset: decode_offset(reader)?,
        byte_count: decode_offset(reader)?,
    };
    let argument_count = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::TooManyInternalUnitScalarCallArguments)?;
    if argument_count > reader.remaining() / 48 {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut arguments = Vec::with_capacity(argument_count);
    for _ in 0..argument_count {
        arguments.push(InternalUnitScalarCallArgumentRecord {
            parameter_index: reader.u32()?,
            source: decode_argument_source(reader)?,
            destination: decode_direct_placement(reader)?,
            code_offset: decode_offset(reader)?,
            byte_count: decode_offset(reader)?,
        });
    }
    Ok(InstalledInternalUnitScalarCall {
        machine,
        text_offset,
        custody: InternalUnitScalarCallRecord {
            owner,
            target,
            call_plan,
            result,
            arguments,
            operation_ordinal: decode_offset(reader)?,
            code_offset: decode_offset(reader)?,
            byte_count: decode_offset(reader)?,
        },
    })
}

pub(super) fn encode_argument_source(
    bytes: &mut Vec<u8>,
    source: InternalUnitScalarArgumentSourceRecord,
) -> Result<(), InstallationError> {
    match source {
        InternalUnitScalarArgumentSourceRecord::Parameter { .. } => {
            return Err(InstallationError::UnsupportedInstalledScalarSource);
        }
        InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
            defining_operation,
            source_value,
            scalar_type,
            value,
        } => {
            bytes.extend_from_slice(&[1, 0, 0, 0]);
            push_u64(bytes, defining_operation.get());
            push_u64(bytes, source_value.get());
            encode_integer_type(bytes, scalar_type)?;
            encode_integer_value(bytes, value);
        }
        InternalUnitScalarArgumentSourceRecord::Home(home) => {
            bytes.extend_from_slice(&[2, 0, 0, 0]);
            encode_unit_scalar_home(bytes, home)?;
        }
    }
    Ok(())
}

pub(super) fn decode_argument_source(
    reader: &mut Reader<'_>,
) -> Result<InternalUnitScalarArgumentSourceRecord, InstallationError> {
    let tag = reader.u8()?;
    if reader.take(3)? != [0; 3] {
        return Err(InstallationError::NonzeroReservedField);
    }
    match tag {
        1 => Ok(InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
            defining_operation: OperationId::new(reader.u64()?)
                .ok_or(InstallationError::ZeroInstalledScalarIdentity)?,
            source_value: ValueId::new(reader.u64()?)
                .ok_or(InstallationError::ZeroInstalledScalarIdentity)?,
            scalar_type: decode_integer_type(reader)?,
            value: decode_integer_value(reader)?,
        }),
        2 => Ok(InternalUnitScalarArgumentSourceRecord::Home(
            decode_unit_scalar_home(reader)?,
        )),
        tag => Err(InstallationError::InvalidInstalledScalarSourceTag(tag)),
    }
}

pub(super) fn encode_offset(bytes: &mut Vec<u8>, value: usize) -> Result<(), InstallationError> {
    push_u64(
        bytes,
        u64::try_from(value)
            .map_err(|_| InstallationError::InstalledScalarOffsetNotRepresentable)?,
    );
    Ok(())
}

pub(super) fn decode_offset(reader: &mut Reader<'_>) -> Result<usize, InstallationError> {
    usize::try_from(reader.u64()?)
        .map_err(|_| InstallationError::InstalledScalarOffsetNotRepresentable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_machine_code::UnitScalarParameterLocationRecord;
    use omega_target_operations::MachineRegister;
    use psi_core::{IntegerSign, IntegerType};

    #[test]
    fn ordinary_installation_codec_rejects_parameter_sources() {
        let mut bytes = Vec::new();
        let source = InternalUnitScalarArgumentSourceRecord::Parameter {
            parameter_index: 0,
            source_value: ValueId::new(1).unwrap(),
            scalar_type: IntegerType::new(IntegerSign::Signed, 32).unwrap(),
            location: UnitScalarParameterLocationRecord::Register(MachineRegister::X86Rdi),
        };
        assert_eq!(
            encode_argument_source(&mut bytes, source),
            Err(InstallationError::UnsupportedInstalledScalarSource)
        );
        assert!(bytes.is_empty());
    }
}
