/*
We define the two-word native scalar-reference descriptor layout
(`ScalarReferenceLayout`) and the second-word storage codec
(`ScalarStorageCodec`) used by internal scalar loans.

A C pointer output overwrites only the address word at offset 0 while leaving
the codec word at offset 8 untouched; consumers authenticate that codec against
the new endpoint before reading or writing backing storage.
*/

use semantic_vocabulary::{
    ClosedScalarType, IeeeFloatFormat, IntegerCarrier, IntegerSign, ScalarType,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScalarReferenceLayout {
    pub address_byte_offset: u32,
    pub address_byte_size: u32,
    pub codec_byte_offset: u32,
    pub codec_byte_size: u32,
    pub byte_size: u32,
    pub alignment: u16,
}

impl ScalarReferenceLayout {
    pub const NATIVE: Self = Self {
        address_byte_offset: 0,
        address_byte_size: 8,
        codec_byte_offset: 8,
        codec_byte_size: 8,
        byte_size: 16,
        alignment: 8,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarStorageCodec {
    Integer { byte_size: u8, sign: IntegerSign },
    Boolean,
    Address, // Canonical 64-bit address storage; grants no integer arithmetic authority.
    IeeeFloat(IeeeFloatFormat),
}

impl ScalarStorageCodec {
    pub fn from_scalar(scalar: ScalarType) -> Option<Self> {
        Self::from_closed_scalar(scalar.try_into().ok()?)
    }

    /// A codec describes concrete backing storage. Checking types cross the
    /// fallible boundary above; a rigid width never supplies byte geometry.
    pub fn from_closed_scalar(scalar: ClosedScalarType) -> Option<Self> {
        match scalar {
            ClosedScalarType::Boolean => Some(Self::Boolean),
            ClosedScalarType::Integer(integer) if integer.is_address() && integer.bits() == 64 => {
                Some(Self::Address)
            }
            ClosedScalarType::Integer(integer)
                if integer.carrier() == IntegerCarrier::Fixed
                    && integer.bits() != 0
                    && integer.bits() <= 64
                    && integer.bits() % 8 == 0 =>
            {
                Some(Self::Integer {
                    byte_size: u8::try_from(integer.bits() / 8).ok()?,
                    sign: integer.sign(),
                })
            }
            ClosedScalarType::IeeeFloat(format) => Some(Self::IeeeFloat(format)),
            _ => None,
        }
    }

    pub const fn byte_size(self) -> u8 {
        match self {
            Self::Integer { byte_size, .. } => byte_size,
            Self::Boolean => 1,
            Self::Address | Self::IeeeFloat(IeeeFloatFormat::Binary64) => 8,
            Self::IeeeFloat(IeeeFloatFormat::Binary32) => 4,
        }
    }

    pub const fn encode(self) -> Option<u64> {
        match self {
            Self::Integer {
                byte_size: 1..=8,
                sign: IntegerSign::Unsigned,
            } => Some(self.byte_size() as u64),
            Self::Integer {
                byte_size: 1..=8,
                sign: IntegerSign::Signed,
            } => Some(self.byte_size() as u64 | 256),
            Self::Boolean => Some(1025),
            Self::Address => Some(2056),
            Self::IeeeFloat(IeeeFloatFormat::Binary32) => Some(516),
            Self::IeeeFloat(IeeeFloatFormat::Binary64) => Some(520),
            _ => None,
        }
    }

    pub const fn decode(word: u64) -> Option<Self> {
        match word {
            1..=8 => Some(Self::Integer {
                byte_size: word as u8,
                sign: IntegerSign::Unsigned,
            }),
            257..=264 => Some(Self::Integer {
                byte_size: (word - 256) as u8,
                sign: IntegerSign::Signed,
            }),
            1025 => Some(Self::Boolean),
            2056 => Some(Self::Address),
            516 => Some(Self::IeeeFloat(IeeeFloatFormat::Binary32)),
            520 => Some(Self::IeeeFloat(IeeeFloatFormat::Binary64)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ScalarStorageCodec;
    use semantic_vocabulary::{
        ClosedIntegerType, ClosedScalarType, IeeeFloatFormat, IntegerSign, IntegerType, MachineId,
        ScalarType, ValueId, WidthParameter,
    };

    #[test]
    fn backing_codecs_close_widths_without_extending_storage_support() {
        let parameter = WidthParameter {
            owner: MachineId::new(1).unwrap(),
            value: ValueId::new(2).unwrap(),
        };
        for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
            assert_eq!(
                ScalarStorageCodec::from_scalar(ScalarType::Integer(IntegerType::parameterized(
                    sign, parameter,
                ))),
                None
            );
            for bits in [1, 8, 13, 16, 24, 32, 40, 48, 56, 64, 128] {
                let integer = ClosedIntegerType::new(sign, bits).unwrap();
                let scalar = ClosedScalarType::Integer(integer);
                let expected = (bits % 8 == 0 && bits <= 64).then(|| ScalarStorageCodec::Integer {
                    byte_size: u8::try_from(bits / 8).unwrap(),
                    sign,
                });
                assert_eq!(ScalarStorageCodec::from_closed_scalar(scalar), expected);
                assert_eq!(
                    ScalarStorageCodec::from_scalar(scalar.as_checking()),
                    expected
                );
            }
        }
        for bits in [24, 32, 64, 128] {
            let address = ClosedScalarType::Integer(ClosedIntegerType::address(bits).unwrap());
            assert_eq!(
                ScalarStorageCodec::from_closed_scalar(address),
                (bits == 64).then_some(ScalarStorageCodec::Address)
            );
        }
        for (scalar, expected) in [
            (ClosedScalarType::Boolean, ScalarStorageCodec::Boolean),
            (
                ClosedScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
                ScalarStorageCodec::IeeeFloat(IeeeFloatFormat::Binary32),
            ),
            (
                ClosedScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
                ScalarStorageCodec::IeeeFloat(IeeeFloatFormat::Binary64),
            ),
        ] {
            assert_eq!(
                ScalarStorageCodec::from_closed_scalar(scalar),
                Some(expected)
            );
            assert_eq!(
                ScalarStorageCodec::from_scalar(scalar.as_checking()),
                Some(expected)
            );
        }
    }

    #[test]
    fn codec_preserves_every_byte_aligned_integer_width_and_sign() {
        for byte_size in 1..=8 {
            for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
                let codec = ScalarStorageCodec::Integer { byte_size, sign };
                assert_eq!(
                    ScalarStorageCodec::decode(codec.encode().expect("valid codec")),
                    Some(codec)
                );
            }
        }
        assert_eq!(ScalarStorageCodec::decode(0), None);
        assert_eq!(ScalarStorageCodec::decode(9), None);
        assert_eq!(ScalarStorageCodec::decode(768), None);
        assert_ne!(ScalarStorageCodec::Boolean.encode(), Some(1));
    }

    #[test]
    fn codec_domain_rejects_reserved_bits_and_invalid_geometry() {
        let mut admitted = 0;
        for word in 0..=4096 {
            if let Some(codec) = ScalarStorageCodec::decode(word) {
                admitted += 1;
                assert_eq!(codec.encode(), Some(word));
                assert!((1..=8).contains(&codec.byte_size()));
            }
        }
        assert_eq!(admitted, 20);
        assert_eq!(ScalarStorageCodec::decode(u64::MAX), None);
    }
}
