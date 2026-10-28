//! All completed native Build choices, including private and unused supplies.
use crate::record::{
    PackagePolicyProviderBinding, PackagePolicyServiceMethod, PackageReviewNominalOwner,
};
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackagePolicyNativeBinding {
    pub method: PackagePolicyServiceMethod,
    pub producer: crate::record::PackagePolicyEvaluatedBindingProducer,
    pub binding: PackagePolicyProviderBinding,
}
impl PackagePolicyNativeBinding {
    pub fn key(&self) -> (&crate::record::PackageReviewNominalIdentity, &str) {
        (
            &self.method.requirement,
            self.producer.callable_identity.as_str(),
        )
    }
    pub fn producer(&self) -> &crate::record::PackagePolicyEvaluatedBindingProducer {
        &self.producer
    }

    pub fn validate(
        &self,
        package: semantic_vocabulary::PackageKeyIdentity,
        target: target::TargetProfile,
    ) -> Result<(), &'static str> {
        let producer = self.producer();
        if producer.package != Some(package)
            || producer.declaration.owner() != PackageReviewNominalOwner::Package(package)
        {
            return Err("native inventory substituted its supplying package");
        }
        crate::record::package::validate_native_service_method(&self.method, target)?;
        self.binding.validate_native(target)
    }
}
