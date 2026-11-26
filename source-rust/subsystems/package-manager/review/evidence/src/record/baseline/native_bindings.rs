// Completed native `Build` choices (`PackagePolicyNativeBinding`), including
// private and unused supplies.

use crate::record::{
    PackagePolicyProviderBinding, PackagePolicyServiceMethod, PackageReviewNominalOwner,
};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyNativeBinding {
    pub application: Option<String>,
    pub method: PackagePolicyServiceMethod,
    pub binding: PackagePolicyProviderBinding,
}
impl PackagePolicyNativeBinding {
    /// The complete closed callable selected by Build, when the binding has
    /// static arguments. Its original method remains in the calling ABI key.
    pub fn application(&self) -> Option<&str> {
        self.application.as_deref()
    }

    pub fn method(&self) -> &PackagePolicyServiceMethod {
        &self.method
    }

    pub fn locator(&self) -> Option<&crate::record::PackageReviewForeignLocator> {
        match &self.binding {
            PackagePolicyProviderBinding::Import { locator, .. } => Some(locator),
            _ => None,
        }
    }

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
            PackagePolicyProviderBinding::Import { producer, .. }
            | PackagePolicyProviderBinding::FunctionField { producer, .. } => Some(producer),
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
        match &self.application {
            Some(application)
                if !self
                    .method
                    .requirement
                    .matches_closed_calling_application(application, &calling.abi)? =>
            {
                return Err(
                    "native inventory application differs from its complete calling identity",
                );
            }
            None if !calling.abi.operands.is_empty() => {
                return Err("native inventory lost its closed calling application");
            }
            _ => {}
        }
        if calling.abi.requirement_identity == calling.requirement.path {
            return Err("native inventory has no complete declaration ABI key");
        }
        if let PackagePolicyProviderBinding::FunctionField { selector, .. } = &self.binding {
            let expected = self
                .application
                .as_deref()
                .unwrap_or(&calling.abi.requirement_identity);
            if selector.table.requirement_application != expected.as_bytes()
                || selector.table.declaration_position >= self.method.parameter_count as u64
                || calling
                    .dispatch_only_formal
                    .is_some_and(|formal| u64::from(formal) != selector.table.declaration_position)
            {
                return Err("function table selects a formal outside its reviewed requirement");
            }
        } else if calling.dispatch_only_formal.is_some() {
            return Err("native argument is only omitted for table dispatch");
        }
        crate::record::package::validate_native_service_method(&self.method, target)?;
        self.binding.validate_native(target)
    }
}
