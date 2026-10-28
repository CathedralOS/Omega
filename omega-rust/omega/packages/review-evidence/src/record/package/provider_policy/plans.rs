use crate::record::accessors::field_projection_accessors;
use crate::record::{
    PackagePolicyProviderFamily, PackagePolicyProviderRow, PackagePolicyServiceMethod,
    PackageReviewNominalIdentity, PackageReviewProviderGrantSelectorKind,
};
use semantic_vocabulary::PackageKeyIdentity;
use target::TargetProfile;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicySelectedProviders {
    pub(crate) package: PackageKeyIdentity,
    pub(crate) target: TargetProfile,
    pub(crate) plans: Vec<PackagePolicyProviderPlan>,
    pub(crate) families: Vec<PackagePolicyProviderFamily>,
}

impl PackagePolicySelectedProviders {
    field_projection_accessors!(copied package: PackageKeyIdentity, target: TargetProfile);
    field_projection_accessors!(sliced plans: &[PackagePolicyProviderPlan]);
    field_projection_accessors!(sliced families: &[PackagePolicyProviderFamily]);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyProviderPlan {
    pub(crate) plan_name: String,
    pub(crate) realizing_package: Option<PackageKeyIdentity>,
    pub(crate) schema_declaration: PackageReviewNominalIdentity,
    pub(crate) provider_type: String,
    pub(crate) provider_type_declaration: Option<PackageReviewNominalIdentity>,
    pub(crate) target: String,
    pub(crate) methods: Vec<PackagePolicyServiceMethod>,
    pub(crate) rows: Vec<PackagePolicyProviderRow>,
    pub(crate) grants: Vec<PackageReviewProviderGrantSelectorKind>,
}

impl PackagePolicyProviderPlan {
    pub fn plan_name(&self) -> &str {
        &self.plan_name
    }

    field_projection_accessors!(borrowed schema_declaration: &PackageReviewNominalIdentity);

    pub fn provider_type(&self) -> &str {
        &self.provider_type
    }

    pub fn provider_type_declaration(&self) -> Option<&PackageReviewNominalIdentity> {
        self.provider_type_declaration.as_ref()
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    field_projection_accessors!(sliced methods: &[PackagePolicyServiceMethod]);
    field_projection_accessors!(sliced rows: &[PackagePolicyProviderRow]);
    field_projection_accessors!(sliced grants: &[PackageReviewProviderGrantSelectorKind]);
}
