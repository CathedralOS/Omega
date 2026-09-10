//! Canonical installation transport for a function's exact scalar ABI.

use semantic_vocabulary::ValueId;
use target_operations::{ScalarAbiValue, ScalarFunctionAbi};

use super::{
    InstallationError, Reader, push_u32, push_u64,
    scalar_call_plan_codec::{decode_scalar_call_plan, encode_scalar_call_plan},
    unit_scalar_codec::{decode_scalar_type, encode_scalar_type},
    value_placement_codec::{decode_direct_placement, encode_direct_placement},
};

pub(super) fn encode_scalar_abi(
    bytes: &mut Vec<u8>,
    abi: Option<&ScalarFunctionAbi>,
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

pub(super) fn decode_scalar_abi(
    reader: &mut Reader<'_>,
) -> Result<Option<ScalarFunctionAbi>, InstallationError> {
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
            Ok(Some(ScalarFunctionAbi {
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
    value: &ScalarAbiValue,
) -> Result<(), InstallationError> {
    push_u64(bytes, value.value.get());
    encode_scalar_type(bytes, value.scalar_type)?;
    encode_direct_placement(bytes, &value.placement)?;
    Ok(())
}

pub(super) fn decode_abi_value(
    reader: &mut Reader<'_>,
) -> Result<ScalarAbiValue, InstallationError> {
    Ok(ScalarAbiValue {
        value: ValueId::new(reader.u64()?).ok_or(InstallationError::ZeroInstalledScalarIdentity)?,
        scalar_type: decode_scalar_type(reader)?,
        placement: decode_direct_placement(reader)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
    use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};

    #[test]
    fn scalar_abi_transport_preserves_boolean_kind_and_integer_bytes() {
        let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let call_plan = evaluate_call_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: vec![ValueShape::integer(1, 1)],
                result: Some(ValueShape::integer(8, 8)),
            },
        )
        .unwrap();
        let abi = ScalarFunctionAbi {
            parameters: vec![ScalarAbiValue {
                value: ValueId::new(1).unwrap(),
                scalar_type: ScalarType::Boolean,
                placement: call_plan.parameters[0].clone(),
            }],
            result: ScalarAbiValue {
                value: ValueId::new(2).unwrap(),
                scalar_type: ScalarType::Integer(integer),
                placement: call_plan.result.clone().unwrap(),
            },
            call_plan,
        };
        let mut bytes = Vec::new();
        encode_scalar_abi(&mut bytes, Some(&abi)).unwrap();
        let mut reader = Reader::new(&bytes);
        assert_eq!(decode_scalar_abi(&mut reader).unwrap(), Some(abi.clone()));
        assert_eq!(reader.remaining(), 0);
        assert!(
            super::super::installed_unit_scalar_transport::installed_scalar_abi_is_canonical(
                &abi,
                target::NativeTarget::linux_x64(),
            )
        );

        let mut integer_bytes = Vec::new();
        encode_abi_value(&mut integer_bytes, &abi.result).unwrap();
        assert_eq!(&integer_bytes[8..12], &[2, 0, 64, 0]);
        let mut boolean_bytes = Vec::new();
        encode_abi_value(&mut boolean_bytes, &abi.parameters[0]).unwrap();
        assert_eq!(&boolean_bytes[8..12], &[0; 4]);
        boolean_bytes[10] = 1;
        assert!(decode_abi_value(&mut Reader::new(&boolean_bytes)).is_err());

        // Boolean results retain their exact carrier and canonical one-byte ABI.
        let mut boolean_result = abi;
        boolean_result.call_plan = evaluate_call_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: vec![ValueShape::integer(1, 1)],
                result: Some(ValueShape::integer(1, 1)),
            },
        )
        .unwrap();
        boolean_result.result.scalar_type = ScalarType::Boolean;
        boolean_result.result.placement = boolean_result.call_plan.result.clone().unwrap();
        assert!(
            super::super::installed_unit_scalar_transport::installed_scalar_abi_is_canonical(
                &boolean_result,
                target::NativeTarget::linux_x64(),
            )
        );
    }
}
