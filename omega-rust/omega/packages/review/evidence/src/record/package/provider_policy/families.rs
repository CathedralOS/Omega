use crate::record::*;
use omega_target::TargetProfile;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyProviderFamily {
    pub(crate) family_identity: PackageReviewNominalIdentity,
    pub(crate) provider_type_declaration: PackageReviewNominalIdentity,
    pub(crate) target: TargetProfile,
    pub(crate) authority: PackageReviewProviderSelectionAuthority,
    pub(crate) coverage: PackageReviewProviderFamilyCoverage,
    pub(crate) coordinates: Vec<PackagePolicyProviderFamilyCoordinate>,
}

impl PackagePolicyProviderFamily {
    pub fn family_identity(&self) -> &PackageReviewNominalIdentity {
        &self.family_identity
    }

    pub fn provider_type_declaration(&self) -> &PackageReviewNominalIdentity {
        &self.provider_type_declaration
    }

    pub fn target(&self) -> TargetProfile {
        self.target
    }

    pub fn authority(&self) -> PackageReviewProviderSelectionAuthority {
        self.authority
    }

    pub fn coverage(&self) -> PackageReviewProviderFamilyCoverage {
        self.coverage
    }

    pub fn coordinates(&self) -> &[PackagePolicyProviderFamilyCoordinate] {
        &self.coordinates
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyProviderFamilyCoordinate {
    pub(crate) requirement_identity: String,
    pub(crate) operator_declaration: PackageReviewNominalIdentity,
    pub(crate) plan_index: u32,
}

impl PackagePolicyProviderFamilyCoordinate {
    pub fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }

    pub fn operator_declaration(&self) -> &PackageReviewNominalIdentity {
        &self.operator_declaration
    }

    pub fn plan_index(&self) -> u32 {
        self.plan_index
    }
}
