/*
We define the two-word native scalar-reference descriptor layout
(`ScalarReferenceLayout`) and the second-word storage codec
(`ScalarStorageCodec`) used by internal scalar loans.

A C pointer output overwrites only the address word at offset 0 while leaving
the codec word at offset 8 untouched; consumers authenticate that codec against
the new endpoint before reading or writing backing storage.
*/

use semantic_vocabulary::{IeeeFloatFormat, IntegerCarrier, IntegerSign, ScalarType};

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
    Integer {
        byte_size: u8,
        sign: IntegerSign,
    },
    Boolean,
    Address, // Canonical 64-bit address storage; grants no integer arithmetic authority.
    IeeeFloat(IeeeFloatFormat),
    Callable {
        contract: semantic_vocabulary::CallableContractId,
    },
}

impl ScalarStorageCodec {
    pub fn from_scalar(scalar: ScalarType) -> Option<Self> {
        match scalar {
            ScalarType::Callable(contract) => Some(Self::Callable { contract }),
            ScalarType::Boolean => Some(Self::Boolean),
            ScalarType::Integer(integer) if integer.is_address() && integer.bits() == 64 => {
                Some(Self::Address)
            }
            ScalarType::Integer(integer)
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
            ScalarType::IeeeFloat(format) => Some(Self::IeeeFloat(format)),
            _ => None,
        }
    }

    pub const fn byte_size(self) -> u8 {
        match self {
            Self::Integer { byte_size, .. } => byte_size,
            Self::Boolean => 1,
            Self::Address | Self::IeeeFloat(IeeeFloatFormat::Binary64) | Self::Callable { .. } => 8,
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
            Self::Callable { contract } => match contract.get().checked_mul(4096) {
                Some(identity) => Some(identity | 3080),
                None => None,
            },
            _ => None,
        }
    }

    pub fn decode(word: u64) -> Option<Self> {
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
            word if word & 4095 == 3080 => semantic_vocabulary::CallableContractId::new(word >> 12)
                .map(|contract| Self::Callable { contract }),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ScalarStorageCodec;
    use semantic_vocabulary::IntegerSign;

    #[test]
    fn callable_storage_codec_retains_the_contract_and_rejects_overflow() {
        let first = ScalarStorageCodec::Callable {
            contract: semantic_vocabulary::CallableContractId::new(1).unwrap(),
        };
        let second = ScalarStorageCodec::Callable {
            contract: semantic_vocabulary::CallableContractId::new(73).unwrap(),
        };
        assert_ne!(first.encode(), second.encode());
        for codec in [first, second] {
            assert_eq!(
                ScalarStorageCodec::decode(codec.encode().unwrap()),
                Some(codec)
            );
            assert_eq!(codec.byte_size(), 8);
        }
        assert_eq!(ScalarStorageCodec::decode(3080), None);
        assert_eq!(
            ScalarStorageCodec::Callable {
                contract: semantic_vocabulary::CallableContractId::new(u64::MAX).unwrap(),
            }
            .encode(),
            None
        );
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
