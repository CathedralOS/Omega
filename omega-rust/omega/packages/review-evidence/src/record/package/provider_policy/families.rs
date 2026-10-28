use crate::record::accessors::field_projection_accessors;
use crate::record::{
    PackageReviewNominalIdentity, PackageReviewProviderFamilyCoverage,
    PackageReviewProviderSelectionAuthority,
};
use target::TargetProfile;

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
    field_projection_accessors!(borrowed family_identity: &PackageReviewNominalIdentity);
    field_projection_accessors!(borrowed provider_type_declaration: &PackageReviewNominalIdentity);

    field_projection_accessors!(copied target: TargetProfile);
    field_projection_accessors!(copied authority: PackageReviewProviderSelectionAuthority);
    field_projection_accessors!(copied coverage: PackageReviewProviderFamilyCoverage);
    field_projection_accessors!(sliced coordinates: &[PackagePolicyProviderFamilyCoordinate]);
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

    field_projection_accessors!(borrowed operator_declaration: &PackageReviewNominalIdentity);

    field_projection_accessors!(copied plan_index: u32);
}
