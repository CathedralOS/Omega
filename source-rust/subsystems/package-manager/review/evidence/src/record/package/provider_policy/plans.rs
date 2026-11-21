// Selected provider plan and family containers for a package and target profile.

use crate::record::accessors::field_projection_accessors;
use crate::record::{
    PackagePolicyProviderFamily, PackagePolicyProviderRow, PackagePolicyServiceMethod,
    PackageReviewNominalIdentity, PackageReviewProviderGrantSelectorKind,
};
use semantic_vocabulary::PackageKeyIdentity;
use target::TargetProfile;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicySelectedProviders {
    pub package: PackageKeyIdentity,
    pub target: TargetProfile,
    pub plans: Vec<PackagePolicyProviderPlan>,
    pub families: Vec<PackagePolicyProviderFamily>,
}

impl PackagePolicySelectedProviders {
    pub(crate) fn has_application_grants(&self) -> bool {
        self.plans
            .iter()
            .flat_map(|plan| &plan.grants)
            .any(|grant| {
                matches!(
                    grant,
                    PackageReviewProviderGrantSelectorKind::ProviderApplication(_)
                )
            })
    }

    field_projection_accessors!(copied package: PackageKeyIdentity, target: TargetProfile);
    field_projection_accessors!(sliced plans: &[PackagePolicyProviderPlan]);
    field_projection_accessors!(sliced families: &[PackagePolicyProviderFamily]);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyProviderPlan {
    pub plan_name: String,
    pub realizing_package: Option<PackageKeyIdentity>,
    pub schema_declaration: PackageReviewNominalIdentity,
    pub schema_application: Option<crate::record::PackageReviewTypeIdentity>,
    pub provider_type: String,
    pub provider_type_declaration: Option<PackageReviewNominalIdentity>,
    pub target: String,
    pub methods: Vec<PackagePolicyServiceMethod>,
    pub rows: Vec<PackagePolicyProviderRow>,
    pub grants: Vec<PackageReviewProviderGrantSelectorKind>,
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
