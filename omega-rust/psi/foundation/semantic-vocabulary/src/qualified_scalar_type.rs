use crate::{ScalarQualificationSetId, ScalarType};

/// Scalar payload shape and immutable canonical membership set coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QualifiedScalarType {
    pub scalar_type: ScalarType,
    pub qualifications: ScalarQualificationSetId,
}

impl From<ScalarType> for QualifiedScalarType {
    fn from(scalar_type: ScalarType) -> Self {
        Self {
            scalar_type,
            qualifications: ScalarQualificationSetId::ZERO,
        }
    }
}
