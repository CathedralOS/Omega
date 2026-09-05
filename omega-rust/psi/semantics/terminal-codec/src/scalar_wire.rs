//! Canonical scalar type and value wire primitives.
//!
//! This module owns only exact scalar tags and little-endian payloads. Module
//! framing, semantic validation, and recursive term structure remain in the
//! parent codec.

use semantic_vocabulary::{
    IeeeFloatFormat, IeeeFloatValue, IntegerCarrier, IntegerSign, IntegerType, IntegerValue,
    ScalarType,
};

use super::CodecError;
use super::wire::{Reader, Writer};

pub(super) fn encode_scalar_type(writer: &mut Writer, scalar_type: ScalarType) {
    match scalar_type {
        ScalarType::Boolean => writer.u8(1),
        ScalarType::Integer(integer_type) => {
            writer.u8(2);
            encode_integer_type(writer, integer_type);
        }
        ScalarType::IeeeFloat(format) => {
            writer.u8(3);
            encode_ieee_float_format(writer, format);
        }
    }
}

pub(super) fn encode_ieee_float_value(writer: &mut Writer, value: IeeeFloatValue) {
    match value {
        IeeeFloatValue::Binary32(bits) => {
            writer.u8(1);
            writer.u32(bits);
        }
        IeeeFloatValue::Binary64(bits) => {
            writer.u8(2);
            writer.u64(bits);
        }
    }
}

fn encode_ieee_float_format(writer: &mut Writer, format: IeeeFloatFormat) {
    writer.u8(match format {
        IeeeFloatFormat::Binary32 => 1,
        IeeeFloatFormat::Binary64 => 2,
    });
}

pub(super) fn encode_integer_type(writer: &mut Writer, integer_type: IntegerType) {
    writer.u8(match (integer_type.carrier(), integer_type.sign()) {
        (IntegerCarrier::Fixed, IntegerSign::Signed) => 1,
        (IntegerCarrier::Fixed, IntegerSign::Unsigned) => 2,
        (IntegerCarrier::Address, IntegerSign::Unsigned) => 3,
        (IntegerCarrier::Address, IntegerSign::Signed) => {
            unreachable!("address carriers are unsigned")
        }
    });
    writer.u16(integer_type.bits());
}

pub(super) fn encode_integer_value(writer: &mut Writer, value: IntegerValue) {
    match value {
        IntegerValue::Signed(value) => {
            writer.u8(1);
            writer.bytes(&value.to_le_bytes());
        }
        IntegerValue::Unsigned(value) => {
            writer.u8(2);
            writer.bytes(&value.to_le_bytes());
        }
    }
}

pub(super) fn decode_scalar_type(reader: &mut Reader<'_>) -> Result<ScalarType, CodecError> {
    Ok(match reader.u8()? {
        1 => ScalarType::Boolean,
        2 => ScalarType::Integer(decode_integer_type(reader)?),
        3 => ScalarType::IeeeFloat(decode_ieee_float_format(reader)?),
        tag => return Err(CodecError::InvalidTag("ScalarType", tag)),
    })
}

pub(super) fn decode_ieee_float_value(
    reader: &mut Reader<'_>,
) -> Result<IeeeFloatValue, CodecError> {
    Ok(match reader.u8()? {
        1 => IeeeFloatValue::Binary32(reader.u32()?),
        2 => IeeeFloatValue::Binary64(reader.u64()?),
        tag => return Err(CodecError::InvalidTag("IeeeFloatValue", tag)),
    })
}

fn decode_ieee_float_format(reader: &mut Reader<'_>) -> Result<IeeeFloatFormat, CodecError> {
    Ok(match reader.u8()? {
        1 => IeeeFloatFormat::Binary32,
        2 => IeeeFloatFormat::Binary64,
        tag => return Err(CodecError::InvalidTag("IeeeFloatFormat", tag)),
    })
}

pub(super) fn decode_integer_type(reader: &mut Reader<'_>) -> Result<IntegerType, CodecError> {
    let tag = reader.u8()?;
    let bits = reader.u16()?;
    match tag {
        1 => IntegerType::new(IntegerSign::Signed, bits),
        2 => IntegerType::new(IntegerSign::Unsigned, bits),
        3 => IntegerType::address(bits),
        tag => return Err(CodecError::InvalidTag("IntegerSign", tag)),
    }
    .map_err(CodecError::MalformedProposition)
}

pub(super) fn decode_integer_value(reader: &mut Reader<'_>) -> Result<IntegerValue, CodecError> {
    Ok(match reader.u8()? {
        1 => IntegerValue::Signed(i128::from_le_bytes(reader.array()?)),
        2 => IntegerValue::Unsigned(u128::from_le_bytes(reader.array()?)),
        tag => return Err(CodecError::InvalidTag("IntegerValue", tag)),
    })
}
