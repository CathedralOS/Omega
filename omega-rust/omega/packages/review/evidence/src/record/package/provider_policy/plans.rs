use crate::record::*;
use omega_target::TargetProfile;
use psi_core::PackageKeyIdentity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicySelectedProviders {
    pub(crate) package: PackageKeyIdentity,
    pub(crate) target: TargetProfile,
    pub(crate) plans: Vec<PackagePolicyProviderPlan>,
    pub(crate) families: Vec<PackagePolicyProviderFamily>,
}

impl PackagePolicySelectedProviders {
    pub fn package(&self) -> PackageKeyIdentity {
        self.package
    }

    pub fn target(&self) -> TargetProfile {
        self.target
    }

    pub fn plans(&self) -> &[PackagePolicyProviderPlan] {
        &self.plans
    }

    pub fn families(&self) -> &[PackagePolicyProviderFamily] {
        &self.families
    }
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

    pub fn realizing_package(&self) -> Option<PackageKeyIdentity> {
        self.realizing_package
    }

    pub fn schema_declaration(&self) -> &PackageReviewNominalIdentity {
        &self.schema_declaration
    }

    pub fn provider_type(&self) -> &str {
        &self.provider_type
    }

    pub fn provider_type_declaration(&self) -> Option<&PackageReviewNominalIdentity> {
        self.provider_type_declaration.as_ref()
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn methods(&self) -> &[PackagePolicyServiceMethod] {
        &self.methods
    }

    pub fn rows(&self) -> &[PackagePolicyProviderRow] {
        &self.rows
    }

    pub fn grants(&self) -> &[PackageReviewProviderGrantSelectorKind] {
        &self.grants
    }
}
