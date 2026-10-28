//! Lossless policy signatures cannot be reconstructed from legacy review rows.

use crate::record::accessors::field_projection_accessors;
use crate::record::{
    PackagePolicyCallableConformance, PackagePolicyTypeParameter, PackageReviewConformanceBound,
    PackageReviewExternalCallableParameter, PackageReviewNominalIdentity,
    PackageReviewOperatorCoordinate, PackageReviewTypeIdentity,
};
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyExternalCallableSignature {
    pub(crate) lifetime_parameter_count: usize,
    pub(crate) static_parameters: Vec<PackagePolicyTypeParameter>,
    pub(crate) conformance_bounds: Vec<PackageReviewConformanceBound>,
    pub(crate) parameters: Vec<PackageReviewExternalCallableParameter>,
    pub(crate) return_type: Option<PackageReviewTypeIdentity>,
}

impl PackagePolicyExternalCallableSignature {
    field_projection_accessors!(copied lifetime_parameter_count: usize);
    field_projection_accessors!(sliced static_parameters: &[PackagePolicyTypeParameter]);
    field_projection_accessors!(sliced conformance_bounds: &[PackageReviewConformanceBound]);
    field_projection_accessors!(optional return_type: Option<&PackageReviewTypeIdentity>);
    field_projection_accessors!(sliced parameters: &[PackageReviewExternalCallableParameter]);
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackagePolicyExternalRequirement {
    Trait(PackagePolicyCallableConformance),
    Operator {
        coordinate: PackageReviewOperatorCoordinate,
        alias: Option<String>,
    },
    TopLevelRequirement {
        identity: PackageReviewNominalIdentity,
        signature: PackagePolicyExternalCallableSignature,
        alias: Option<String>,
    },
}

impl PackagePolicyExternalRequirement {
    pub fn alias(&self) -> Option<&str> {
        match self {
            Self::Trait(conformance) => conformance.alias(),
            Self::Operator { alias, .. } | Self::TopLevelRequirement { alias, .. } => {
                alias.as_deref()
            }
        }
    }
}
