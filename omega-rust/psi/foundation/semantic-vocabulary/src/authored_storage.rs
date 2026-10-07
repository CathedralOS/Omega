//! Closed source-authored storage geometry, independent of a target ABI.
//! These values retain an admitted layout plan; they grant neither access nor
//! write-fit evidence. Fragmented fields have no direct scalar byte address.

use crate::ScalarType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AuthoredRecordStorage {
    pub byte_size: u64,
    pub byte_alignment: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AuthoredFieldStorage {
    Scalar {
        byte_offset: u64,
        stored_scalar: Option<ScalarType>,
    },
    Repeated {
        byte_offset: u64,
        element_stride: u64,
    },
    Direct {
        byte_offset: u64,
    },
    Fragmented,
}

impl AuthoredFieldStorage {
    pub const fn byte_offset(self) -> Option<u64> {
        match self {
            Self::Scalar { byte_offset, .. }
            | Self::Repeated { byte_offset, .. }
            | Self::Direct { byte_offset } => Some(byte_offset),
            Self::Fragmented => None,
        }
    }

    pub const fn stored_scalar(self) -> Option<ScalarType> {
        match self {
            Self::Scalar { stored_scalar, .. } => stored_scalar,
            _ => None,
        }
    }

    pub const fn element_stride(self) -> Option<u64> {
        match self {
            Self::Repeated { element_stride, .. } => Some(element_stride),
            _ => None,
        }
    }
}
