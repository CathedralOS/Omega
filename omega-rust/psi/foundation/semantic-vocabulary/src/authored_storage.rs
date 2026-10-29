//! Closed source-authored storage geometry, independent of a target ABI.
//! These values retain an admitted layout plan; they grant neither access nor
//! write-fit evidence. Fragmented fields have no direct scalar byte address.

use crate::ScalarType;
use arena::{Arena, HandleSpan};
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};

/// One normalized transfer-container fragment. Every coordinate is in bits
/// except the container's byte offset; semantic width comes from the field,
/// not from whichever fragment happens to have the highest source bit.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
)]
pub struct AuthoredBitFragment {
    pub container_byte_offset: u64,
    pub container_width_bits: u64,
    pub destination_lsb: u64,
    pub source_lsb: u64,
    pub width: u64,
}

/// Immutable, arena-owned placement recipe. Canonical identity compares rows,
/// never allocation handles or arena generations. Sorting follows the layout
/// normalizer's complete coordinate tuple; duplicate rows remain observable
/// so an invalid recipe cannot become valid through silent deduplication.
#[derive(Debug, Clone, compiler_storage::Record)]
pub struct AuthoredBitFragments {
    storage: Arena<AuthoredBitFragment>,
    fragments: HandleSpan<AuthoredBitFragment>,
}

impl AuthoredBitFragments {
    pub fn new(fragments: impl IntoIterator<Item = AuthoredBitFragment>) -> Self {
        let mut rows: Vec<_> = fragments.into_iter().collect();
        rows.sort_unstable();
        let mut storage = Arena::new();
        let fragments = storage.insert_many(rows);
        Self { storage, fragments }
    }

    pub fn fragments(&self) -> &[AuthoredBitFragment] {
        // The private arena never frees or mutates this constructor-owned span.
        self.storage.span_or_empty(self.fragments)
    }

    /// The envelope bounds full container accesses, including RMW sibling
    /// bits. It is not permission to read gaps or a contiguous scalar address.
    pub fn byte_envelope(&self) -> Option<(u64, u64)> {
        let mut start = u64::MAX;
        let mut end = 0;
        for fragment in self.fragments() {
            if fragment.container_width_bits == 0
                || !fragment.container_width_bits.is_multiple_of(8)
            {
                return None;
            }
            start = start.min(fragment.container_byte_offset);
            end = end.max(
                fragment
                    .container_byte_offset
                    .checked_add(fragment.container_width_bits / 8)?,
            );
        }
        Some((start, end.checked_sub(start)?))
    }
}

impl PartialEq for AuthoredBitFragments {
    fn eq(&self, other: &Self) -> bool {
        self.fragments() == other.fragments()
    }
}
impl Eq for AuthoredBitFragments {}
impl PartialOrd for AuthoredBitFragments {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for AuthoredBitFragments {
    fn cmp(&self, other: &Self) -> Ordering {
        self.fragments().cmp(other.fragments())
    }
}
impl Hash for AuthoredBitFragments {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.fragments().hash(state);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub struct AuthoredRecordStorage {
    pub byte_size: u64,
    pub byte_alignment: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
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
    Fragmented(AuthoredBitFragments),
}

impl AuthoredFieldStorage {
    pub const fn byte_offset(&self) -> Option<u64> {
        match self {
            Self::Scalar { byte_offset, .. }
            | Self::Repeated { byte_offset, .. }
            | Self::Direct { byte_offset } => Some(*byte_offset),
            Self::Fragmented(_) => None,
        }
    }

    pub const fn stored_scalar(&self) -> Option<ScalarType> {
        match self {
            Self::Scalar { stored_scalar, .. } => *stored_scalar,
            _ => None,
        }
    }

    pub const fn element_stride(&self) -> Option<u64> {
        match self {
            Self::Repeated { element_stride, .. } => Some(*element_stride),
            _ => None,
        }
    }
}
