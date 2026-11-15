// Completed native `Build` choices (`PackagePolicyNativeBinding`), including
// private and unused supplies.

use crate::record::{
    PackagePolicyProviderBinding, PackagePolicyServiceMethod, PackageReviewNominalOwner,
};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyNativeBinding {
    pub(crate) application: Option<String>,
    pub(crate) method: PackagePolicyServiceMethod,
    pub(crate) binding: PackagePolicyProviderBinding,
}
impl PackagePolicyNativeBinding {
    pub fn method(&self) -> &PackagePolicyServiceMethod {
        &self.method
    }

    pub fn locator(&self) -> Option<&crate::record::PackageReviewForeignLocator> {
        match &self.binding {
            PackagePolicyProviderBinding::Import { locator, .. } => Some(locator),
            _ => None,
        }
    }

    pub(crate) fn key(
        &self,
    ) -> (
        &crate::record::PackageReviewNominalIdentity,
        Option<&str>,
        &str,
    ) {
        (
            &self.method.requirement,
            self.application.as_deref(),
            self.producer().map_or("", |p| p.callable_identity.as_str()),
        )
    }
    pub fn producer(&self) -> Option<&crate::record::PackagePolicyEvaluatedBindingProducer> {
        match &self.binding {
            PackagePolicyProviderBinding::Import { producer, .. } => Some(producer),
            PackagePolicyProviderBinding::Syscall {
                evaluated: Some(evaluated),
                ..
            } => Some(&evaluated.producer),
            _ => None,
        }
    }
    pub(crate) fn validate(
        &self,
        package: semantic_vocabulary::PackageKeyIdentity,
        target: target::TargetProfile,
    ) -> Result<(), &'static str> {
        if self.application.is_some() {
            return Err("native inventory has an unsupported applied requirement");
        }
        let producer = self
            .producer()
            .ok_or("native inventory has no evaluated Build producer")?;
        if producer.package != Some(package)
            || producer.declaration.owner() != PackageReviewNominalOwner::Package(package)
        {
            return Err("native inventory substituted its supplying package");
        }
        let [calling] = self.method.calling.as_slice() else {
            return Err("native inventory has no unique pinned calling application");
        };
        if calling.abi.requirement_identity == calling.requirement.path {
            return Err("native inventory has no complete declaration ABI key");
        }
        crate::record::package::validate_native_service_method(&self.method, target)?;
        self.binding.validate_native(target)
    }
}
