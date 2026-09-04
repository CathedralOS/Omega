//! Canonical installation transport for an attached Unit function's scalar ABI.

use omega_machine_code::UnitScalarFunctionAbiRecord;
use omega_target_operations::UnitScalarAbiValue;
use psi_core::ValueId;

use super::{
    InstallationError, Reader, push_u32, push_u64,
    scalar_call_plan_codec::{decode_scalar_call_plan, encode_scalar_call_plan},
    unit_scalar_codec::{decode_scalar_type, encode_scalar_type},
    value_placement_codec::{decode_direct_placement, encode_direct_placement},
};

pub(super) fn encode_unit_scalar_abi(
    bytes: &mut Vec<u8>,
    abi: Option<&UnitScalarFunctionAbiRecord>,
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
    Ok(())
}

pub(super) fn decode_unit_scalar_abi(
    reader: &mut Reader<'_>,
) -> Result<Option<UnitScalarFunctionAbiRecord>, InstallationError> {
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
                parameters.push(UnitScalarAbiValue {
                    value: ValueId::new(reader.u64()?)
                        .ok_or(InstallationError::ZeroInstalledScalarIdentity)?,
                    scalar_type: decode_scalar_type(reader)?,
                    placement: decode_direct_placement(reader)?,
                });
            }
            Ok(Some(UnitScalarFunctionAbiRecord {
                call_plan,
                parameters,
            }))
        }
        tag => Err(InstallationError::InvalidPresenceFlag(tag)),
    }
}
