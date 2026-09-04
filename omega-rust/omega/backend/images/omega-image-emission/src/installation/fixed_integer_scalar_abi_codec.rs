//! Canonical installation transport for a function's exact fixed-integer ABI.

use omega_target_operations::{FixedIntegerScalarAbiValue, FixedIntegerScalarFunctionAbi};
use psi_core::ValueId;

use super::{
    InstallationError, Reader, push_u32, push_u64,
    scalar_call_plan_codec::{decode_scalar_call_plan, encode_scalar_call_plan},
    unit_scalar_codec::{decode_integer_type, encode_integer_type},
    value_placement_codec::{decode_direct_placement, encode_direct_placement},
};

pub(super) fn encode_fixed_integer_scalar_abi(
    bytes: &mut Vec<u8>,
    abi: Option<&FixedIntegerScalarFunctionAbi>,
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
        encode_abi_value(bytes, parameter)?;
    }
    encode_abi_value(bytes, &abi.result)?;
    Ok(())
}

pub(super) fn decode_fixed_integer_scalar_abi(
    reader: &mut Reader<'_>,
) -> Result<Option<FixedIntegerScalarFunctionAbi>, InstallationError> {
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
            let count = usize::try_from(reader.u32()?)
                .map_err(|_| InstallationError::TooManyScalarCallPlanValues)?;
            if count > reader.remaining() / 28 {
                return Err(InstallationError::UnexpectedEnd);
            }
            let mut parameters = Vec::with_capacity(count);
            for _ in 0..count {
                parameters.push(decode_abi_value(reader)?);
            }
            let result = decode_abi_value(reader)?;
            Ok(Some(FixedIntegerScalarFunctionAbi {
                call_plan,
                parameters,
                result,
            }))
        }
        tag => Err(InstallationError::InvalidPresenceFlag(tag)),
    }
}

pub(super) fn encode_abi_value(
    bytes: &mut Vec<u8>,
    value: &FixedIntegerScalarAbiValue,
) -> Result<(), InstallationError> {
    push_u64(bytes, value.value.get());
    encode_integer_type(bytes, value.scalar_type)?;
    encode_direct_placement(bytes, &value.placement)?;
    Ok(())
}

pub(super) fn decode_abi_value(
    reader: &mut Reader<'_>,
) -> Result<FixedIntegerScalarAbiValue, InstallationError> {
    Ok(FixedIntegerScalarAbiValue {
        value: ValueId::new(reader.u64()?).ok_or(InstallationError::ZeroInstalledScalarIdentity)?,
        scalar_type: decode_integer_type(reader)?,
        placement: decode_direct_placement(reader)?,
    })
}
