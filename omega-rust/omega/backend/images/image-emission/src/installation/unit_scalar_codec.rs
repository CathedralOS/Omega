//! Canonical transport for fixed-width integer identities, durable Unit homes,
//! and zero-code Unit integer definitions.

use machine_code::{
    UnitAffineScalarRecordEstablishmentRecord, UnitIntegerConstantRecord, UnitScalarHomeRecord,
};
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, OperationId, PlaceId, ScalarType, StructuralFieldId,
    StructuralTypeId, ValueId,
};

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

pub(super) fn encode_scalar_type(
    bytes: &mut Vec<u8>,
    scalar_type: ScalarType,
) -> Result<(), InstallationError> {
    match scalar_type {
        ScalarType::Boolean => {
            bytes.extend_from_slice(&[0; 4]);
            Ok(())
        }
        ScalarType::Integer(integer) => encode_integer_type(bytes, integer),
        ScalarType::IeeeFloat(_) => Err(InstallationError::UnsupportedInstalledFixedIntegerType),
    }
}

pub(super) fn decode_scalar_type(reader: &mut Reader<'_>) -> Result<ScalarType, InstallationError> {
    let tag = reader.u8()?;
    let reserved = reader.u8()?;
    let bits = reader.u16()?;
    if tag == 0 {
        return (reserved == 0 && bits == 0)
            .then_some(ScalarType::Boolean)
            .ok_or(InstallationError::NonzeroReservedField);
    }
    let sign = match tag {
        1 => IntegerSign::Signed,
        2 => IntegerSign::Unsigned,
        tag => return Err(InstallationError::InvalidInstalledIntegerSignTag(tag)),
    };
    if reserved != 0 {
        return Err(InstallationError::NonzeroReservedField);
    }
    if !matches!(bits, 8 | 16 | 32 | 64) {
        return Err(InstallationError::UnsupportedInstalledFixedIntegerType);
    }
    IntegerType::new(sign, bits)
        .map(ScalarType::Integer)
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
    encode_scalar_type(bytes, home.scalar_type)?;
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
        scalar_type: decode_scalar_type(reader)?,
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

pub(super) fn encode_unit_affine_scalar_records(
    bytes: &mut Vec<u8>,
    records: &[UnitAffineScalarRecordEstablishmentRecord],
) -> Result<(), InstallationError> {
    push_u32(
        bytes,
        u32::try_from(records.len())
            .map_err(|_| InstallationError::TooManyUnitAffineScalarRecords)?,
    );
    for record in records {
        if record.result.multiplicity != terminal_psi::StructuralMultiplicity::Affine
            || !record.result.qualifications.is_empty()
            || !record.result.projected_qualifications.is_empty()
            || !record.result.claims.is_empty()
            || record.shape != calling_conventions::ValueShape::integer(8, 8)
            || !matches!(record.value, IntegerValue::Signed(value) if i64::try_from(value).is_ok())
        {
            return Err(InstallationError::InvalidUnitAffineScalarRecord);
        }
        push_u64(bytes, record.psi_operation.get());
        push_u64(bytes, record.result.place.get());
        push_u64(bytes, record.result.structural_type.get());
        push_u64(bytes, record.field.get());
        encode_integer_value(bytes, record.value);
        encode_shape(bytes, record.shape)?;
        push_u64(
            bytes,
            u64::try_from(record.operation_ordinal)
                .map_err(|_| InstallationError::InstalledScalarOffsetNotRepresentable)?,
        );
    }
    Ok(())
}

pub(super) fn decode_unit_affine_scalar_records(
    reader: &mut Reader<'_>,
) -> Result<Vec<UnitAffineScalarRecordEstablishmentRecord>, InstallationError> {
    let count = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::TooManyUnitAffineScalarRecords)?;
    if count > reader.remaining() / 64 {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut records = Vec::with_capacity(count);
    for _ in 0..count {
        let record = UnitAffineScalarRecordEstablishmentRecord {
            psi_operation: OperationId::new(reader.u64()?)
                .ok_or(InstallationError::InvalidUnitAffineScalarRecord)?,
            result: terminal_psi::StructuralOperationResult {
                place: PlaceId::new(reader.u64()?)
                    .ok_or(InstallationError::InvalidUnitAffineScalarRecord)?,
                structural_type: StructuralTypeId::new(reader.u64()?)
                    .ok_or(InstallationError::InvalidUnitAffineScalarRecord)?,
                multiplicity: terminal_psi::StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            },
            field: StructuralFieldId::new(reader.u64()?)
                .ok_or(InstallationError::InvalidUnitAffineScalarRecord)?,
            value: decode_integer_value(reader)?,
            shape: decode_shape(reader)?,
            operation_ordinal: usize::try_from(reader.u64()?)
                .map_err(|_| InstallationError::InstalledScalarOffsetNotRepresentable)?,
        };
        if record.shape != calling_conventions::ValueShape::integer(8, 8)
            || !matches!(record.value, IntegerValue::Signed(value) if i64::try_from(value).is_ok())
        {
            return Err(InstallationError::InvalidUnitAffineScalarRecord);
        }
        records.push(record);
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use calling_conventions::ValueShape;

    use super::*;

    fn record() -> UnitAffineScalarRecordEstablishmentRecord {
        UnitAffineScalarRecordEstablishmentRecord {
            psi_operation: OperationId::new(1).unwrap(),
            result: terminal_psi::StructuralOperationResult {
                place: PlaceId::new(2).unwrap(),
                structural_type: StructuralTypeId::new(3).unwrap(),
                multiplicity: terminal_psi::StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            },
            field: StructuralFieldId::new(4).unwrap(),
            value: IntegerValue::Signed(7),
            shape: calling_conventions::ValueShape::integer(8, 8),
            operation_ordinal: 5,
        }
    }

    #[test]
    fn affine_scalar_record_round_trips_and_rejects_unsigned_forgery() {
        let expected = vec![record()];
        let mut bytes = Vec::new();
        encode_unit_affine_scalar_records(&mut bytes, &expected).unwrap();
        let decoded = decode_unit_affine_scalar_records(&mut Reader::new(&bytes)).unwrap();
        assert_eq!(decoded, expected);

        bytes[36] = 2;
        assert_eq!(
            decode_unit_affine_scalar_records(&mut Reader::new(&bytes)),
            Err(InstallationError::InvalidUnitAffineScalarRecord)
        );
    }

    #[test]
    fn boolean_home_round_trips_and_noncanonical_tags_reject() {
        let home = UnitScalarHomeRecord {
            defining_operation: OperationId::new(7).unwrap(),
            source_value: ValueId::new(8).unwrap(),
            scalar_type: ScalarType::Boolean,
            shape: ValueShape::integer(1, 1),
            byte_offset: 16,
        };
        let mut bytes = Vec::new();
        encode_unit_scalar_home(&mut bytes, home).expect("encode Boolean home");
        let mut reader = Reader::new(&bytes);
        assert_eq!(
            decode_unit_scalar_home(&mut reader).expect("decode Boolean home"),
            home
        );
        assert_eq!(reader.remaining(), 0);

        let mut noncanonical = bytes;
        noncanonical[17] = 1;
        assert!(decode_unit_scalar_home(&mut Reader::new(&noncanonical)).is_err());
    }
}
