//! Shared complete service-schema validation for inert policy components.

use super::{
    PackagePolicyServiceMethod,
    validation::{nominal, same_target},
};
use crate::record::PackageReviewNominalIdentity;
use omega_target::TargetProfile;

impl PackagePolicyServiceMethod {
    pub(crate) fn validate_service_structure(
        &self,
        schema: &PackageReviewNominalIdentity,
        target: TargetProfile,
    ) -> Result<(), &'static str> {
        self.validate_signature()?;
        self.validate_authority()?;
        nominal(&self.requirement_owner)?;
        nominal(&self.requirement)?;
        if self.requirement.owner != self.requirement_owner.owner
            || self.name.is_empty()
            || self.parameter_count != self.parameter_type_identities.len()
            || self.parameter_type_identities.iter().any(String::is_empty)
            || self.has_result != self.result_type_identity.is_some()
            || self
                .result_type_identity
                .as_ref()
                .is_some_and(String::is_empty)
            || (!self.has_result && !self.result_claims.is_empty())
            || self.entry_claims.iter().any(|claim| {
                claim.parameter_index >= self.parameter_count
                    || claim.carrier_identity.is_empty()
                    || claim.domain.is_empty()
            })
        {
            return Err("provider method has inconsistent identity, signature, or claims");
        }
        if let Some(calling) = &self.calling {
            calling.validate_canonical_structure()?;
            if !same_target(calling.target.profile, target)
                || calling.boundary_trait != *schema
                || calling.requirement != self.requirement
                || calling.requirement_trait != self.requirement_owner
                || calling.semantic_parameters.len() != self.parameter_count
                || calling.semantic_result.is_some() != self.has_result
            {
                return Err(
                    "provider method calling policy is detached from its schema or requirement",
                );
            }
        }
        for premise in &self.termination_premises {
            if premise.profile.is_empty()
                || matches!(premise.subject, omega_effects::provider_plan::ServiceProgressSubject::Parameter(ordinal) if ordinal >= self.parameter_count)
                || premise
                    .establishment_routes
                    .iter()
                    .any(|route| route.requirement_identity.is_empty())
            {
                return Err("provider progress premise has an invalid subject or route");
            }
        }
        Ok(())
    }
}

pub(crate) fn validate_service_methods(
    methods: &[PackagePolicyServiceMethod],
    schema: &PackageReviewNominalIdentity,
    target: TargetProfile,
) -> Result<(), &'static str> {
    nominal(schema)?;
    if methods.is_empty() {
        return Err("service schema has no declared methods");
    }
    for (index, method) in methods.iter().enumerate() {
        method.validate_service_structure(schema, target)?;
        if methods[..index]
            .iter()
            .any(|prior| prior.requirement == method.requirement)
        {
            return Err("service schema repeats an exact requirement");
        }
    }
    Ok(())
}
