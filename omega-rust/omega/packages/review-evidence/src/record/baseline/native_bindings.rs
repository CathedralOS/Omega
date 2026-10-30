//! All completed native Build choices, including private and unused supplies.
use crate::record::{
    PackagePolicyProviderBinding, PackagePolicyServiceMethod, PackageReviewNominalOwner,
};
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackagePolicyNativeBinding {
    pub method: PackagePolicyServiceMethod,
    pub producer: crate::record::PackagePolicyEvaluatedBindingProducer,
    pub binding: PackagePolicyProviderBinding,
    pub application: PackagePolicyNativeApplication,
}
impl PackagePolicyNativeBinding {
    pub fn key(
        &self,
    ) -> (
        &crate::record::PackageReviewNominalIdentity,
        &str,
        &PackagePolicyNativeApplication,
    ) {
        (
            &self.method.requirement,
            self.producer.callable_identity.as_str(),
            &self.application,
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
        self.application.validate()?;
        self.binding.validate_native(target)
    }
}

/// Complete static identity participates even when no physical parameter uses
/// an actual. Universal is distinct from a supplied outer or member tuple.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum PackagePolicyNativeApplication {
    Universal,
    Applied {
        outer: Option<String>,
        local: Option<PackagePolicyNativeLocalApplication>,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PackagePolicyNativeLocalApplication {
    pub arguments: Vec<String>,
    pub lifetimes: Vec<String>,
}
impl From<build_time_evaluation::native_bindings::NativeApplicationIdentity>
    for PackagePolicyNativeApplication
{
    fn from(value: build_time_evaluation::native_bindings::NativeApplicationIdentity) -> Self {
        if value.outer.is_none() && value.local.is_none() {
            Self::Universal
        } else {
            Self::Applied {
                outer: value.outer,
                local: value
                    .local
                    .map(|local| PackagePolicyNativeLocalApplication {
                        arguments: local.arguments,
                        lifetimes: local.lifetimes,
                    }),
            }
        }
    }
}
impl PackagePolicyNativeApplication {
    fn validate(&self) -> Result<(), &'static str> {
        if let Self::Applied { outer, local } = self {
            if (outer.is_none() && local.is_none())
                || outer.as_ref().is_some_and(String::is_empty)
                || local.as_ref().is_some_and(|local| {
                    local
                        .arguments
                        .iter()
                        .chain(&local.lifetimes)
                        .any(String::is_empty)
                })
            {
                return Err("native inventory lost its complete static application");
            }
        }
        Ok(())
    }
}
