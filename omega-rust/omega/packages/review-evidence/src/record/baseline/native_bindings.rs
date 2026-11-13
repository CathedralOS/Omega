// Completed native `Build` choices (`PackagePolicyNativeBinding`), including
// private and unused supplies.

use crate::record::{
    PackagePolicyProviderBinding, PackagePolicyServiceMethod, PackageReviewNominalOwner,
};
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackagePolicyNativeBinding {
    pub application: Option<String>,
    pub method: PackagePolicyServiceMethod,
    pub binding: PackagePolicyProviderBinding,
}
impl PackagePolicyNativeBinding {
    pub fn key(
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
    pub fn validate(
        &self,
        package: semantic_vocabulary::PackageKeyIdentity,
        target: target::TargetProfile,
    ) -> Result<(), &'static str> {
        if self.application.as_ref().is_some_and(String::is_empty) {
            return Err("native inventory has an empty application identity");
        }
        let producer = self
            .producer()
            .ok_or("native inventory has no evaluated Build producer")?;
        if producer.package != Some(package)
            || producer.declaration.owner() != PackageReviewNominalOwner::Package(package)
        {
            return Err("native inventory substituted its supplying package");
        }
        crate::record::package::validate_native_service_method(&self.method, target)?;
        self.binding.validate_native(target)
    }
}
