//! Canonical byte-content predicates shared by source declaration custody and
//! lowered semantic operations. A declaration name alone grants no meaning:
//! ordinary selection and the sealed catalog authenticate compiler supply.

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum ByteSequencePredicate {
    ValidUtf8,
    NoNul,
    AsciiOnly,
    NonEmpty,
}

impl ByteSequencePredicate {
    pub const ALL: [Self; 4] = [
        Self::ValidUtf8,
        Self::NoNul,
        Self::AsciiOnly,
        Self::NonEmpty,
    ];

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "valid_utf8" => Some(Self::ValidUtf8),
            "no_nul" => Some(Self::NoNul),
            "ascii_only" => Some(Self::AsciiOnly),
            "non_empty" => Some(Self::NonEmpty),
            _ => None,
        }
    }

    pub fn holds_for(self, bytes: &[u8]) -> bool {
        match self {
            Self::ValidUtf8 => std::str::from_utf8(bytes).is_ok(),
            Self::NoNul => !bytes.contains(&0),
            Self::AsciiOnly => bytes.iter().all(|byte| *byte < 128),
            Self::NonEmpty => !bytes.is_empty(),
        }
    }

    pub fn is_concat_preserving(self) -> bool {
        match self {
            Self::ValidUtf8 | Self::NoNul | Self::AsciiOnly | Self::NonEmpty => true,
        }
    }

    pub fn is_subslice_preserving(self) -> bool {
        match self {
            Self::NoNul | Self::AsciiOnly => true,
            Self::ValidUtf8 | Self::NonEmpty => false,
        }
    }

    pub fn implies(self, other: Self) -> bool {
        self == other || (self == Self::AsciiOnly && other == Self::ValidUtf8)
    }

    pub fn mask_bit(self) -> u8 {
        match self {
            Self::ValidUtf8 => 1 << 0,
            Self::NoNul => 1 << 1,
            Self::AsciiOnly => 1 << 2,
            Self::NonEmpty => 1 << 3,
        }
    }

    pub fn in_mask(mask: u8) -> impl Iterator<Item = Self> {
        Self::ALL
            .into_iter()
            .filter(move |predicate| mask & predicate.mask_bit() != 0)
    }
}
