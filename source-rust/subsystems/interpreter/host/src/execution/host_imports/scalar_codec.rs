// We encode and decode logical scalar values using the target producer's exact
// storage codec. Staged I/O and original native homes share this conversion and
// never recover signedness or qualifications from an equal byte width. Raw
// import metadata is closed before byte geometry; symbolic checking widths
// cannot choose a buffer extent or a sign-extension shift.

use semantic_vocabulary::{
    ClosedIntegerType, ClosedScalarType, IeeeFloatFormat, IeeeFloatValue, IntegerSign,
    IntegerValue, ScalarType,
};
use terminal_interpreter::{TerminalEffectRejection, TerminalScalarValue};

fn invalid_operands() -> TerminalEffectRejection {
    TerminalEffectRejection::new("native scalar differs from its admitted logical/storage codec")
}

// The interpreter carries a host-returned `addr` only as an opaque value at a
// native import's bare scalar argument or result; the codecs below never
// admit it, so it cannot reach interpreter storage or a record layout.
pub(super) fn is_host_address_carrier(scalar: ScalarType) -> bool {
    matches!(scalar, ScalarType::Integer(integer) if integer.is_address() && integer.bits() == Some(64))
}

pub(super) fn boundary_scalar_extent(scalar: ScalarType) -> Result<usize, TerminalEffectRejection> {
    if is_host_address_carrier(scalar) {
        return Ok(8);
    }
    scalar_extent(scalar)
}

pub(super) fn scalar_extent(scalar: ScalarType) -> Result<usize, TerminalEffectRejection> {
    closed_scalar_extent(ClosedScalarType::try_from(scalar).map_err(|_| invalid_operands())?)
}

fn closed_scalar_extent(scalar: ClosedScalarType) -> Result<usize, TerminalEffectRejection> {
    match scalar {
        ClosedScalarType::Boolean => Ok(1),
        ClosedScalarType::IeeeFloat(IeeeFloatFormat::Binary32) => Ok(4),
        ClosedScalarType::IeeeFloat(IeeeFloatFormat::Binary64) => Ok(8),
        ClosedScalarType::Integer(integer)
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
    let closed_storage = ClosedScalarType::try_from(storage).map_err(|_| invalid_operands())?;
    closed_scalar_extent(closed_storage)?;
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
        (TerminalScalarValue::Integer { scalar_type, value }, ScalarType::Integer(stored)) => {
            let logical =
                ClosedIntegerType::try_from(scalar_type).map_err(|_| invalid_operands())?;
            let closed_stored = closed_storage.as_integer().ok_or_else(invalid_operands)?;
            if logical.is_address()
                || logical.bits() < closed_stored.bits()
                || !scalar_type.admits(value)
            {
                return Err(invalid_operands());
            }
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
    let closed_storage = ClosedScalarType::try_from(storage).map_err(|_| invalid_operands())?;
    let closed_logical = ClosedScalarType::try_from(logical).map_err(|_| invalid_operands())?;
    if bytes.len() != closed_scalar_extent(closed_storage)? {
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
        (ScalarType::Integer(stored), ScalarType::Integer(integer)) => {
            let closed_stored = closed_storage.as_integer().ok_or_else(invalid_operands)?;
            let closed_integer = closed_logical.as_integer().ok_or_else(invalid_operands)?;
            if closed_integer.bits() < closed_stored.bits() || closed_integer.is_address() {
                return Err(invalid_operands());
            }
            let mut padded = [0; 16];
            padded[..bytes.len()].copy_from_slice(bytes);
            let raw = u128::from_le_bytes(padded);
            let value = match stored.sign() {
                IntegerSign::Unsigned => IntegerValue::Unsigned(raw),
                IntegerSign::Signed => IntegerValue::Signed(
                    ((raw << (128 - closed_stored.bits())) as i128) >> (128 - closed_stored.bits()),
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

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{IntegerType, MachineId, ValueId, WidthParameter};

    fn integer(bits: u16) -> IntegerType {
        IntegerType::new(IntegerSign::Signed, bits).unwrap()
    }

    #[test]
    fn symbolic_logical_and_storage_widths_are_refused_before_byte_conversion() {
        let parameter = IntegerType::parameterized(
            IntegerSign::Signed,
            WidthParameter {
                owner: MachineId::new(7).unwrap(),
                value: ValueId::new(11).unwrap(),
            },
        );
        for (logical, stored) in [
            (parameter, integer(8)),
            (integer(16), parameter),
            (parameter, parameter),
        ] {
            assert!(
                encode(
                    TerminalScalarValue::Integer {
                        scalar_type: logical,
                        value: IntegerValue::Signed(-1),
                    },
                    ScalarType::Integer(stored),
                )
                .is_err()
            );
            assert!(
                decode(
                    &[0xff],
                    ScalarType::Integer(stored),
                    ScalarType::Integer(logical),
                )
                .is_err()
            );
        }
        assert!(scalar_extent(ScalarType::Integer(parameter)).is_err());
    }

    #[test]
    fn signed_storage_roundtrip_retains_the_logical_carrier_and_range() {
        let logical = integer(16);
        let storage = ScalarType::Integer(integer(8));
        for value in [-128, -1, 0, 127] {
            let original = TerminalScalarValue::Integer {
                scalar_type: logical,
                value: IntegerValue::Signed(value),
            };
            let bytes = encode(original, storage).unwrap();
            assert_eq!(bytes[0], value as u8);
            assert_eq!(
                decode(&bytes[..1], storage, ScalarType::Integer(logical)),
                Ok(original)
            );
        }
        assert!(
            encode(
                TerminalScalarValue::Integer {
                    scalar_type: logical,
                    value: IntegerValue::Signed(128),
                },
                storage,
            )
            .is_err()
        );
        assert!(decode(&[0xff, 0xff], ScalarType::Integer(logical), storage).is_err());
        assert!(decode(&[], storage, ScalarType::Integer(logical)).is_err());
    }

    #[test]
    fn scalar_storage_still_requires_whole_bytes_and_refuses_addresses() {
        let seven = ScalarType::Integer(integer(7));
        let address = ScalarType::Integer(IntegerType::address(64).unwrap());
        for storage in [seven, address] {
            assert!(scalar_extent(storage).is_err());
            assert!(
                encode(
                    TerminalScalarValue::Integer {
                        scalar_type: integer(64),
                        value: IntegerValue::Signed(0),
                    },
                    storage,
                )
                .is_err()
            );
            assert!(decode(&[0], storage, ScalarType::Integer(integer(64))).is_err());
        }
        assert_eq!(boundary_scalar_extent(address), Ok(8));
        assert!(decode(&[2], ScalarType::Boolean, ScalarType::Boolean).is_err());
    }
}
