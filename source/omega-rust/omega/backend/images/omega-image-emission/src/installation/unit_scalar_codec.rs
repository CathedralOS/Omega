//! Canonical transport for fixed-width integer identities, durable Unit homes,
//! and zero-code Unit integer definitions.

use omega_machine_code::{UnitIntegerConstantRecord, UnitScalarHomeRecord};
use psi_core::{IntegerSign, IntegerType, IntegerValue, OperationId, ValueId};

use super::{
    InstallationError, Reader, push_u16, push_u32, push_u64, push_u128,
    value_placement_codec::{decode_shape, encode_shape},
};

pub(super) fn encode_integer_type(
    bytes: &mut Vec<u8>,
    scalar_type: IntegerType,
) -> Result<(), InstallationError> {
    if scalar_type.is_address() {
        return Err(InstallationError::UnsupportedInstalledFixedIntegerType);
    }
    bytes.push(match scalar_type.sign() {
        IntegerSign::Signed => 1,
        IntegerSign::Unsigned => 2,
    });
    bytes.push(0);
    push_u16(bytes, scalar_type.bits());
    Ok(())
}

pub(super) fn decode_integer_type(
    reader: &mut Reader<'_>,
) -> Result<IntegerType, InstallationError> {
    let sign = match reader.u8()? {
        1 => IntegerSign::Signed,
        2 => IntegerSign::Unsigned,
        tag => return Err(InstallationError::InvalidInstalledIntegerSignTag(tag)),
    };
    if reader.u8()? != 0 {
        return Err(InstallationError::NonzeroReservedField);
    }
    let bits = reader.u16()?;
    if !matches!(bits, 8 | 16 | 32 | 64) {
        return Err(InstallationError::UnsupportedInstalledFixedIntegerType);
    }
    IntegerType::new(sign, bits)
        .map_err(|_| InstallationError::UnsupportedInstalledFixedIntegerType)
}

pub(super) fn encode_integer_value(bytes: &mut Vec<u8>, value: IntegerValue) {
    match value {
        IntegerValue::Signed(value) => {
            bytes.extend_from_slice(&[1, 0, 0, 0]);
            push_u128(bytes, value as u128);
        }
        IntegerValue::Unsigned(value) => {
            bytes.extend_from_slice(&[2, 0, 0, 0]);
            push_u128(bytes, value);
        }
    }
}

pub(super) fn decode_integer_value(
    reader: &mut Reader<'_>,
) -> Result<IntegerValue, InstallationError> {
    let tag = reader.u8()?;
    if reader.take(3)? != [0; 3] {
        return Err(InstallationError::NonzeroReservedField);
    }
    let bits = reader.u128()?;
    match tag {
        1 => Ok(IntegerValue::Signed(bits as i128)),
        2 => Ok(IntegerValue::Unsigned(bits)),
        tag => Err(InstallationError::InvalidInstalledIntegerValueTag(tag)),
    }
}

pub(super) fn encode_unit_scalar_home(
    bytes: &mut Vec<u8>,
    home: UnitScalarHomeRecord,
) -> Result<(), InstallationError> {
    push_u64(bytes, home.defining_operation.get());
    push_u64(bytes, home.source_value.get());
    encode_integer_type(bytes, home.scalar_type)?;
    encode_shape(bytes, home.shape)?;
    push_u32(bytes, home.byte_offset);
    Ok(())
}

pub(super) fn decode_unit_scalar_home(
    reader: &mut Reader<'_>,
) -> Result<UnitScalarHomeRecord, InstallationError> {
    Ok(UnitScalarHomeRecord {
        defining_operation: OperationId::new(reader.u64()?)
            .ok_or(InstallationError::ZeroInstalledScalarIdentity)?,
        source_value: ValueId::new(reader.u64()?)
            .ok_or(InstallationError::ZeroInstalledScalarIdentity)?,
        scalar_type: decode_integer_type(reader)?,
        shape: decode_shape(reader)?,
        byte_offset: reader.u32()?,
    })
}

pub(super) fn encode_unit_scalar_homes(
    bytes: &mut Vec<u8>,
    homes: &[UnitScalarHomeRecord],
) -> Result<(), InstallationError> {
    push_u32(
        bytes,
        u32::try_from(homes.len()).map_err(|_| InstallationError::TooManyUnitScalarHomes)?,
    );
    for home in homes {
        encode_unit_scalar_home(bytes, *home)?;
    }
    Ok(())
}

pub(super) fn decode_unit_scalar_homes(
    reader: &mut Reader<'_>,
) -> Result<Vec<UnitScalarHomeRecord>, InstallationError> {
    let count =
        usize::try_from(reader.u32()?).map_err(|_| InstallationError::TooManyUnitScalarHomes)?;
    if count > reader.remaining() / 32 {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut homes = Vec::with_capacity(count);
    for _ in 0..count {
        homes.push(decode_unit_scalar_home(reader)?);
    }
    Ok(homes)
}

pub(super) fn encode_unit_integer_constants(
    bytes: &mut Vec<u8>,
    constants: &[UnitIntegerConstantRecord],
) -> Result<(), InstallationError> {
    push_u32(
        bytes,
        u32::try_from(constants.len())
            .map_err(|_| InstallationError::TooManyUnitIntegerConstants)?,
    );
    for constant in constants {
        push_u64(bytes, constant.defining_operation.get());
        push_u64(bytes, constant.source_value.get());
        encode_integer_type(bytes, constant.scalar_type)?;
        encode_integer_value(bytes, constant.value);
        push_u64(
            bytes,
            u64::try_from(constant.operation_ordinal)
                .map_err(|_| InstallationError::InstalledScalarOffsetNotRepresentable)?,
        );
    }
    Ok(())
}

pub(super) fn decode_unit_integer_constants(
    reader: &mut Reader<'_>,
) -> Result<Vec<UnitIntegerConstantRecord>, InstallationError> {
    let count = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::TooManyUnitIntegerConstants)?;
    if count > reader.remaining() / 48 {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut constants = Vec::with_capacity(count);
    for _ in 0..count {
        constants.push(UnitIntegerConstantRecord {
            defining_operation: OperationId::new(reader.u64()?)
                .ok_or(InstallationError::ZeroInstalledScalarIdentity)?,
            source_value: ValueId::new(reader.u64()?)
                .ok_or(InstallationError::ZeroInstalledScalarIdentity)?,
            scalar_type: decode_integer_type(reader)?,
            value: decode_integer_value(reader)?,
            operation_ordinal: usize::try_from(reader.u64()?)
                .map_err(|_| InstallationError::InstalledScalarOffsetNotRepresentable)?,
        });
    }
    Ok(constants)
}
