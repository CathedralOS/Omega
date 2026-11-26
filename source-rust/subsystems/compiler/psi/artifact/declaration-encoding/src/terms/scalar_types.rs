use semantic_vocabulary::{
    IeeeFloatValue, IntegerCarrier, IntegerSign, IntegerType, IntegerValue, ScalarType,
};

use crate::CodecError;
use crate::structural_fields::{decode_ieee_float_format, encode_ieee_float_format};
use crate::wire::{Reader, Writer};

pub fn encode_scalar_type(writer: &mut Writer, scalar_type: ScalarType) -> Result<(), CodecError> {
    match scalar_type {
        ScalarType::Boolean => writer.u8(1),
        ScalarType::Integer(integer_type) => {
            writer.u8(2);
            encode_integer_type(writer, integer_type)?;
        }
        ScalarType::IeeeFloat(format) => {
            writer.u8(3);
            encode_ieee_float_format(writer, format);
        }
        ScalarType::Callable(contract) => {
            writer.u8(4);
            writer.u64(contract.get());
        }
    }
    Ok(())
}

pub fn encode_ieee_float_value(writer: &mut Writer, value: IeeeFloatValue) {
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

pub fn encode_integer_type(
    writer: &mut Writer,
    integer_type: IntegerType,
) -> Result<(), CodecError> {
    writer.require_integer_width(integer_type)?;
    writer.u8(match (integer_type.carrier(), integer_type.sign()) {
        (IntegerCarrier::Fixed, IntegerSign::Signed) => 1,
        (IntegerCarrier::Fixed, IntegerSign::Unsigned) => 2,
        (IntegerCarrier::Address, IntegerSign::Unsigned) => 3,
        (IntegerCarrier::Address, IntegerSign::Signed) => {
            return Err(CodecError::IntegerWidthClosure(
                semantic_vocabulary::IntegerWidthClosureError::SignedAddress,
            ));
        }
    });
    integer_type
        .width()
        .write_identity(|bytes| writer.bytes(bytes));
    Ok(())
}

pub fn encode_integer_value(writer: &mut Writer, value: IntegerValue) {
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

pub fn decode_scalar_type(reader: &mut Reader<'_>) -> Result<ScalarType, CodecError> {
    Ok(match reader.u8()? {
        1 => ScalarType::Boolean,
        2 => ScalarType::Integer(decode_integer_type(reader)?),
        3 => ScalarType::IeeeFloat(decode_ieee_float_format(reader)?),
        4 => ScalarType::Callable(
            semantic_vocabulary::CallableContractId::new(reader.u64()?)
                .ok_or(CodecError::ZeroIdentity("CallableContractId"))?,
        ),
        tag => return Err(CodecError::InvalidTag("ScalarType", tag)),
    })
}

pub fn decode_ieee_float_value(reader: &mut Reader<'_>) -> Result<IeeeFloatValue, CodecError> {
    Ok(match reader.u8()? {
        1 => IeeeFloatValue::Binary32(reader.u32()?),
        2 => IeeeFloatValue::Binary64(reader.u64()?),
        tag => return Err(CodecError::InvalidTag("IeeeFloatValue", tag)),
    })
}

pub fn decode_integer_type(reader: &mut Reader<'_>) -> Result<IntegerType, CodecError> {
    let tag = reader.u8()?;
    let bits = reader.u16()?;
    let integer = match tag {
        1 => IntegerType::new(IntegerSign::Signed, bits),
        2 => IntegerType::new(IntegerSign::Unsigned, bits),
        3 => IntegerType::address(bits),
        tag => return Err(CodecError::InvalidTag("IntegerSign", tag)),
    }
    .map_err(CodecError::MalformedProposition)?;
    reader.record_integer_type(integer)?;
    Ok(integer)
}

pub fn decode_integer_value(reader: &mut Reader<'_>) -> Result<IntegerValue, CodecError> {
    Ok(match reader.u8()? {
        1 => IntegerValue::Signed(i128::from_le_bytes(reader.array()?)),
        2 => IntegerValue::Unsigned(u128::from_le_bytes(reader.array()?)),
        tag => return Err(CodecError::InvalidTag("IntegerValue", tag)),
    })
}

pub struct Scalar;
impl crate::fields::Field<semantic_vocabulary::ScalarType> for Scalar {
    fn write(
        writer: &mut Writer,
        value: &semantic_vocabulary::ScalarType,
    ) -> Result<(), CodecError> {
        encode_scalar_type(writer, *value)
    }
    fn read(reader: &mut Reader<'_>) -> Result<semantic_vocabulary::ScalarType, CodecError> {
        decode_scalar_type(reader)
    }
}
