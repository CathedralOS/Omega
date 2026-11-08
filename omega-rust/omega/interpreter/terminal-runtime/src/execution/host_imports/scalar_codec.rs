//! Encode logical scalar values with the target producer's exact storage codec.
//! Staged I/O and original native homes share this conversion; neither may
//! recover signedness or qualifications from an equal byte width.

use semantic_vocabulary::{IeeeFloatFormat, IeeeFloatValue, IntegerSign, IntegerValue, ScalarType};
use terminal_interpreter::{TerminalEffectRejection, TerminalScalarValue};

fn invalid_operands() -> TerminalEffectRejection {
    TerminalEffectRejection::new("native scalar differs from its admitted logical/storage codec")
}

pub(super) fn scalar_extent(scalar: ScalarType) -> Result<usize, TerminalEffectRejection> {
    match scalar {
        ScalarType::Boolean => Ok(1),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary32) => Ok(4),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary64) => Ok(8),
        ScalarType::Integer(integer)
            if !integer.is_address()
                && integer.bits() != 0
                && integer.bits() <= 128
                && integer.bits() % 8 == 0 =>
        {
            Ok(usize::from(integer.bits() / 8))
        }
        _ => Err(invalid_operands()),
    }
}

pub(super) fn encode(
    value: TerminalScalarValue,
    storage: ScalarType,
) -> Result<[u8; 16], TerminalEffectRejection> {
    let mut bytes = [0; 16];
    match (value, storage) {
        (TerminalScalarValue::Boolean(value), ScalarType::Boolean) => bytes[0] = u8::from(value),
        (
            TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary32(bits)),
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
        ) => bytes[..4].copy_from_slice(&bits.to_le_bytes()),
        (
            TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary64(bits)),
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
        ) => bytes[..8].copy_from_slice(&bits.to_le_bytes()),
        (TerminalScalarValue::Integer { scalar_type, value }, ScalarType::Integer(stored))
            if !scalar_type.is_address()
                && scalar_type.bits() >= stored.bits()
                && scalar_type.admits(value) =>
        {
            let value = integer_value_with_sign(value, stored.sign())?;
            if !stored.admits(value) {
                return Err(invalid_operands());
            }
            bytes = match value {
                IntegerValue::Signed(value) => value.to_le_bytes(),
                IntegerValue::Unsigned(value) => value.to_le_bytes(),
            };
        }
        _ => return Err(invalid_operands()),
    }
    Ok(bytes)
}

pub(super) fn decode(
    bytes: &[u8],
    storage: ScalarType,
    logical: ScalarType,
) -> Result<TerminalScalarValue, TerminalEffectRejection> {
    if bytes.len() != scalar_extent(storage)? {
        return Err(invalid_operands());
    }
    match (storage, logical) {
        (ScalarType::Boolean, ScalarType::Boolean) => match bytes {
            [0] => Ok(TerminalScalarValue::Boolean(false)),
            [1] => Ok(TerminalScalarValue::Boolean(true)),
            _ => Err(invalid_operands()),
        },
        (
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
        ) => Ok(TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary32(
            u32::from_le_bytes(bytes.try_into().map_err(|_| invalid_operands())?),
        ))),
        (
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
        ) => Ok(TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary64(
            u64::from_le_bytes(bytes.try_into().map_err(|_| invalid_operands())?),
        ))),
        (ScalarType::Integer(stored), ScalarType::Integer(integer))
            if integer.bits() >= stored.bits() && !integer.is_address() =>
        {
            let mut padded = [0; 16];
            padded[..bytes.len()].copy_from_slice(bytes);
            let raw = u128::from_le_bytes(padded);
            let value = match stored.sign() {
                IntegerSign::Unsigned => IntegerValue::Unsigned(raw),
                IntegerSign::Signed => IntegerValue::Signed(
                    ((raw << (128 - stored.bits())) as i128) >> (128 - stored.bits()),
                ),
            };
            let value = integer_value_with_sign(value, integer.sign())?;
            if !integer.admits(value) {
                return Err(invalid_operands());
            }
            Ok(TerminalScalarValue::Integer {
                scalar_type: integer,
                value,
            })
        }
        _ => Err(invalid_operands()),
    }
}

fn integer_value_with_sign(
    value: IntegerValue,
    sign: IntegerSign,
) -> Result<IntegerValue, TerminalEffectRejection> {
    match (sign, value) {
        (IntegerSign::Unsigned, IntegerValue::Signed(value)) => Ok(IntegerValue::Unsigned(
            u128::try_from(value).map_err(|_| invalid_operands())?,
        )),
        (IntegerSign::Signed, IntegerValue::Unsigned(value)) => Ok(IntegerValue::Signed(
            i128::try_from(value).map_err(|_| invalid_operands())?,
        )),
        (_, value) => Ok(value),
    }
}
