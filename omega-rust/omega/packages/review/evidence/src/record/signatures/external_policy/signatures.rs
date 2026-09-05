//! Lossless policy signatures cannot be reconstructed from legacy review rows.
use crate::record::*;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyExternalCallableSignature {
    pub(crate) lifetime_parameter_count: usize,
    pub(crate) static_parameters: Vec<PackagePolicyTypeParameter>,
    pub(crate) conformance_bounds: Vec<PackageReviewConformanceBound>,
    pub(crate) parameters: Vec<PackageReviewExternalCallableParameter>,
    pub(crate) return_type: Option<PackageReviewTypeIdentity>,
}

impl PackagePolicyExternalCallableSignature {
    pub const fn lifetime_parameter_count(&self) -> usize {
        self.lifetime_parameter_count
    }
    pub fn static_parameters(&self) -> &[PackagePolicyTypeParameter] {
        &self.static_parameters
    }
    pub fn conformance_bounds(&self) -> &[PackageReviewConformanceBound] {
        &self.conformance_bounds
    }
    pub fn parameters(&self) -> &[PackageReviewExternalCallableParameter] {
        &self.parameters
    }
    pub const fn return_type(&self) -> Option<&PackageReviewTypeIdentity> {
        self.return_type.as_ref()
    }
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
