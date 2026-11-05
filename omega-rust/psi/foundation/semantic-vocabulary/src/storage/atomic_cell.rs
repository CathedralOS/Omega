//! The resident contract of one atomic storage location, independent of a
//! containing record. The contract describes storage; it supplies neither a
//! resident value nor permission to perform an ordinary scalar read or write.

use crate::{BoundedIntegerType, IntegerType, ScalarType};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum AtomicCellType {
    Boolean,
    Integer(IntegerType),
    BoundedInteger(BoundedIntegerType),
}

impl AtomicCellType {
    pub fn from_scalar(scalar: ScalarType) -> Option<Self> {
        match scalar {
            ScalarType::Boolean => Some(Self::Boolean),
            ScalarType::Integer(integer) => Self::Integer(integer)
                .is_supported()
                .then_some(Self::Integer(integer)),
            _ => None,
        }
    }

    pub const fn scalar_type(self) -> ScalarType {
        match self {
            Self::Boolean => ScalarType::Boolean,
            Self::Integer(integer) => ScalarType::Integer(integer),
            Self::BoundedInteger(bounds) => ScalarType::Integer(bounds.integer_type()),
        }
    }

    pub const fn bounds(self) -> Option<BoundedIntegerType> {
        match self {
            Self::BoundedInteger(bounds) => Some(bounds),
            _ => None,
        }
    }

    /// Independent consumers check the admitted width even for decoded input.
    pub fn is_supported(self) -> bool {
        match self.scalar_type() {
            ScalarType::Boolean => true,
            ScalarType::Integer(integer) => {
                !integer.is_address() && matches!(integer.bits(), 8 | 16 | 32 | 64)
            }
            _ => false,
        }
    }
}
