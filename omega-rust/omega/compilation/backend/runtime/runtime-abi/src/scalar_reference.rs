//! Internal scalar loans retain backing geometry independently of semantic type.
//! The second descriptor word is a codec, not a length or a grant of access.

use semantic_vocabulary::{IeeeFloatFormat, IntegerCarrier, IntegerSign, ScalarType};

/// The two independent lanes of a native scalar-reference descriptor. A C
/// pointer output writes the address lane only; its retained storage codec is
/// unchanged. Consumers must authenticate that codec against the new endpoint.
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

/// Closed native interpretation of one contiguous scalar storage region.
/// Integer byte widths include packed, byte-aligned widths such as 24 bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarStorageCodec {
    Integer {
        byte_size: u8,
        sign: IntegerSign,
    },
    Boolean,
    /// Canonical address storage only; this codec grants no integer authority.
    Address,
    IeeeFloat(IeeeFloatFormat),
}

impl ScalarStorageCodec {
    /// Convert admitted scalar geometry, excluding non-fixed integer carriers.
    pub fn from_scalar(scalar: ScalarType) -> Option<Self> {
        match scalar {
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
            Self::Address | Self::IeeeFloat(IeeeFloatFormat::Binary64) => 8,
            Self::IeeeFloat(IeeeFloatFormat::Binary32) => 4,
        }
    }

    /// Low eight bits give byte width; bits 8/9/10 select signed/float/Boolean.
    /// Invalid integer widths have no ABI encoding.
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
    use semantic_vocabulary::IntegerSign;

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
