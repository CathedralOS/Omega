//! Canonical installation transport for the exact mixed scalar/structural ABI.

use semantic_vocabulary::{PlaceId, StructuralTypeId, ValueId};
use target_operations::{
    MixedStructuralScalarAbiResult, MixedStructuralScalarFunctionAbi, TargetStructuralParameter,
};

use super::{
    InstallationError, Reader,
    fixed_integer_scalar_abi_codec::{decode_abi_value, encode_abi_value},
    push_u32, push_u64,
    scalar_call_plan_codec::{decode_scalar_call_plan, encode_scalar_call_plan},
    structural_scalar_codec::{access_tag, decode_access, decode_multiplicity, multiplicity_tag},
    unit_scalar_codec::{decode_scalar_type, encode_scalar_type},
    unit_structural_scalar_field_store_codec::{
        decode_projected_qualifications, encode_projected_qualifications,
    },
    value_placement_codec::{
        decode_direct_placement, decode_shape, encode_direct_placement, encode_shape,
    },
};

pub(super) fn encode_mixed_structural_scalar_abi(
    bytes: &mut Vec<u8>,
    abi: Option<&MixedStructuralScalarFunctionAbi>,
) -> Result<(), InstallationError> {
    let Some(abi) = abi else {
        bytes.extend_from_slice(&[0; 4]);
        return Ok(());
    };
    bytes.extend_from_slice(&[1, 0, 0, 0]);
    encode_scalar_call_plan(bytes, &abi.call_plan)?;
    push_u32(
        bytes,
        u32::try_from(abi.scalar_parameters.len())
            .map_err(|_| InstallationError::TooManyScalarCallPlanValues)?,
    );
    for parameter in &abi.scalar_parameters {
        encode_abi_value(bytes, parameter)?;
    }
    push_u32(
        bytes,
        u32::try_from(abi.structural_parameters.len())
            .map_err(|_| InstallationError::TooManyStructuralReturnParameters)?,
    );
    for parameter in &abi.structural_parameters {
        push_u64(bytes, parameter.place.get());
        push_u64(bytes, parameter.structural_type.get());
        bytes.push(multiplicity_tag(parameter.multiplicity));
        bytes.push(access_tag(parameter.access));
        bytes.extend_from_slice(&[0; 2]);
        encode_projected_qualifications(bytes, &parameter.projected_qualifications)?;
        encode_shape(bytes, parameter.shape)?;
        encode_direct_placement(bytes, &parameter.placement)?;
    }
    push_u64(bytes, abi.result.value.get());
    encode_scalar_type(bytes, abi.result.scalar_type)?;
    encode_direct_placement(bytes, &abi.result.placement)
}

pub(super) fn decode_mixed_structural_scalar_abi(
    reader: &mut Reader<'_>,
) -> Result<Option<MixedStructuralScalarFunctionAbi>, InstallationError> {
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
            let scalar_count = usize::try_from(reader.u32()?)
                .map_err(|_| InstallationError::TooManyScalarCallPlanValues)?;
            if scalar_count > reader.remaining() / 28 {
                return Err(InstallationError::UnexpectedEnd);
            }
            let mut scalar_parameters = Vec::with_capacity(scalar_count);
            for _ in 0..scalar_count {
                scalar_parameters.push(decode_abi_value(reader)?);
            }
            let structural_count = usize::try_from(reader.u32()?)
                .map_err(|_| InstallationError::TooManyStructuralReturnParameters)?;
            if structural_count > reader.remaining() / 40 {
                return Err(InstallationError::UnexpectedEnd);
            }
            let mut structural_parameters = Vec::with_capacity(structural_count);
            for _ in 0..structural_count {
                let place = PlaceId::new(reader.u64()?).ok_or(
                    InstallationError::ZeroStructuralReturnIdentity("mixed ABI place"),
                )?;
                let structural_type = StructuralTypeId::new(reader.u64()?).ok_or(
                    InstallationError::ZeroStructuralReturnIdentity("mixed ABI type"),
                )?;
                let multiplicity = decode_multiplicity(reader.u8()?)?;
                let access = decode_access(reader.u8()?)?;
                if reader.take(2)? != [0; 2] {
                    return Err(InstallationError::NonzeroReservedField);
                }
                structural_parameters.push(TargetStructuralParameter {
                    place,
                    structural_type,
                    multiplicity,
                    access,
                    projected_qualifications: decode_projected_qualifications(reader)?,
                    shape: decode_shape(reader)?,
                    placement: decode_direct_placement(reader)?,
                });
            }
            Ok(Some(MixedStructuralScalarFunctionAbi {
                call_plan,
                scalar_parameters,
                structural_parameters,
                result: MixedStructuralScalarAbiResult {
                    value: ValueId::new(reader.u64()?)
                        .ok_or(InstallationError::ZeroInstalledScalarIdentity)?,
                    scalar_type: decode_scalar_type(reader)?,
                    placement: decode_direct_placement(reader)?,
                },
            }))
        }
        tag => Err(InstallationError::InvalidPresenceFlag(tag)),
    }
}
