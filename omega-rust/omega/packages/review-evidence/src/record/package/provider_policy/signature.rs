//! Target-independent source signature of a selected service application.

use crate::record::{
    PackagePolicyTypeParameter, PackageReviewSignatureParameter, PackageReviewTypeIdentity,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyServiceSignature {
    pub(crate) schema_arguments: Vec<PackageReviewTypeIdentity>,
    pub(crate) schema_lifetime_parameter_count: u32,
    pub(crate) schema_lifetime_arguments: Vec<u32>,
    pub(crate) application_lifetime_parameter_count: u32,
    pub(crate) requirement_arguments: Vec<PackageReviewTypeIdentity>,
    pub(crate) requirement_lifetime_arguments: Vec<u32>,
    pub(crate) requirement_lifetime_parameter_count: u32,
    pub(crate) static_parameters: Vec<PackagePolicyTypeParameter>,
    pub(crate) parameters: Vec<PackageReviewSignatureParameter>,
    pub(crate) result: Option<PackageReviewTypeIdentity>,
}

impl PackagePolicyServiceSignature {
    crate::record::accessors::record_accessors!(static_parameters, parameters);
    pub fn result(&self) -> Option<&PackageReviewTypeIdentity> {
        self.result.as_ref()
    }
}

impl super::PackagePolicyServiceMethod {
    pub(super) fn validate_signature(&self) -> Result<(), &'static str> {
        let signature = &self.signature;
        let application_scope = signature
            .schema_lifetime_parameter_count
            .checked_add(signature.application_lifetime_parameter_count)
            .ok_or("provider signature lifetime telescope overflows")?;
        let lifetime_count = application_scope
            .checked_add(signature.requirement_lifetime_parameter_count)
            .ok_or("provider signature lifetime telescope overflows")?;
        if signature.parameters.len() != self.parameter_count
            || signature.result.is_some() != self.has_result
            || signature.schema_lifetime_arguments.len()
                != signature.schema_lifetime_parameter_count as usize
            || signature
                .schema_lifetime_arguments
                .iter()
                .any(|ordinal| *ordinal >= application_scope)
            || signature
                .requirement_lifetime_arguments
                .iter()
                .any(|ordinal| *ordinal >= application_scope)
            || signature.parameters.iter().any(|parameter| {
                parameter.name.is_empty() || parameter.type_identity.canonical.is_empty()
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
        let scope = crate::record::package::callable_policy::validation::signature::Scope {
            outer: None,
            statics: &[],
            policy_statics: &signature.static_parameters,
            proposition_binders: &[],
            static_offset: 0,
            lifetimes: lifetime_count as usize,
            parameters: signature.parameters.len(),
            result: signature.result.is_some(),
            domain_subject: false,
        };
        crate::record::public_policy::validation::signatures::parameters(&scope, 0)?;
        if let Some(calling) = &self.calling
            && (signature.schema_arguments != calling.boundary_arguments
                || signature.schema_lifetime_parameter_count
                    != calling.boundary_lifetime_parameter_count
                || signature.schema_lifetime_arguments != calling.boundary_lifetime_arguments
                || signature.application_lifetime_parameter_count
                    != calling.application_lifetime_parameter_count
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
