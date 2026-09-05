//! Target-independent source signature of a selected service application.

use crate::record::{
    PackageReviewTraitRequirementParameter, PackageReviewTypeIdentity, PackageReviewTypeParameter,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyServiceSignature {
    pub(crate) schema_arguments: Vec<PackageReviewTypeIdentity>,
    pub(crate) schema_lifetime_parameter_count: u32,
    pub(crate) requirement_arguments: Vec<PackageReviewTypeIdentity>,
    pub(crate) requirement_lifetime_arguments: Vec<u32>,
    pub(crate) requirement_lifetime_parameter_count: u32,
    pub(crate) static_parameters: Vec<PackageReviewTypeParameter>,
    pub(crate) parameters: Vec<PackageReviewTraitRequirementParameter>,
    pub(crate) result: Option<PackageReviewTypeIdentity>,
}

impl PackagePolicyServiceSignature {
    pub fn schema_arguments(&self) -> &[PackageReviewTypeIdentity] {
        &self.schema_arguments
    }
    pub fn schema_lifetime_parameter_count(&self) -> u32 {
        self.schema_lifetime_parameter_count
    }
    pub fn requirement_arguments(&self) -> &[PackageReviewTypeIdentity] {
        &self.requirement_arguments
    }
    pub fn requirement_lifetime_arguments(&self) -> &[u32] {
        &self.requirement_lifetime_arguments
    }
    pub fn requirement_lifetime_parameter_count(&self) -> u32 {
        self.requirement_lifetime_parameter_count
    }
    pub fn static_parameters(&self) -> &[PackageReviewTypeParameter] {
        &self.static_parameters
    }
    pub fn parameters(&self) -> &[PackageReviewTraitRequirementParameter] {
        &self.parameters
    }
    pub fn result(&self) -> Option<&PackageReviewTypeIdentity> {
        self.result.as_ref()
    }
}

impl super::PackagePolicyServiceMethod {
    pub(super) fn validate_signature(&self) -> Result<(), &'static str> {
        let signature = &self.signature;
        if signature.parameters.len() != self.parameter_count
            || signature.result.is_some() != self.has_result
            || signature
                .schema_lifetime_parameter_count
                .checked_add(signature.requirement_lifetime_parameter_count)
                .is_none()
            || signature
                .requirement_lifetime_arguments
                .iter()
                .any(|ordinal| *ordinal >= signature.schema_lifetime_parameter_count)
            || signature.parameters.iter().any(|parameter| {
                parameter.is_self
                    || parameter.name.is_empty()
                    || parameter.type_identity.canonical.is_empty()
            })
            || signature
                .result
                .as_ref()
                .is_some_and(|result| result.canonical.is_empty())
        {
            return Err(
                "provider typed service signature has inconsistent parameters or lifetimes",
            );
        }
        if let Some(calling) = &self.calling
            && (signature.schema_arguments != calling.boundary_arguments
                || signature.schema_lifetime_parameter_count
                    != calling.boundary_lifetime_parameter_count
                || signature.requirement_arguments != calling.requirement_arguments
                || signature.requirement_lifetime_arguments
                    != calling.requirement_lifetime_arguments
                || signature.requirement_lifetime_parameter_count
                    != calling.requirement_lifetime_parameter_count
                || signature.static_parameters != calling.static_parameters
                || signature.result != calling.semantic_result
                || signature.parameters.len() != calling.semantic_parameters.len()
                || signature
                    .parameters
                    .iter()
                    .zip(&calling.semantic_parameters)
                    .any(|(source, physical)| {
                        source.name != physical.name
                            || source.type_identity != physical.value_type
                            || source.is_const != physical.is_const
                            || source.is_mutable != physical.is_mutable
                    }))
        {
            return Err(
                "provider service signature is detached from its complete calling application",
            );
        }
        Ok(())
    }
}
