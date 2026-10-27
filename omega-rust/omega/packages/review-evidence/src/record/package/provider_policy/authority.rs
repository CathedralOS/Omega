//! Source-qualified operational authority, independently of readable schemas.

use crate::record::package::callable_policy::validation::nominal_set;
use crate::record::validation::ordered;
use crate::record::{PackageReviewNominalIdentity, PackageReviewSynchronousInvocation};
use effects::provider_plan::{ServiceProgressEstablishmentRouteKind, ServiceProgressSubject};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyServiceAuthority {
    pub(crate) service_reach: Vec<PackageReviewNominalIdentity>,
    pub(crate) synchronous_invocations: Vec<PackageReviewSynchronousInvocation>,
    pub(crate) progress_premises: Vec<PackagePolicyServiceProgressPremise>,
}

impl PackagePolicyServiceAuthority {
    pub fn service_reach(&self) -> &[PackageReviewNominalIdentity] {
        &self.service_reach
    }
    pub fn synchronous_invocations(&self) -> &[PackageReviewSynchronousInvocation] {
        &self.synchronous_invocations
    }
    pub fn progress_premises(&self) -> &[PackagePolicyServiceProgressPremise] {
        &self.progress_premises
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyServiceProgressPremise {
    pub(crate) profile: PackageReviewNominalIdentity,
    pub(crate) subject: ServiceProgressSubject,
    pub(crate) subject_projections: Vec<PackageReviewNominalIdentity>,
    pub(crate) establishment_routes: Vec<PackagePolicyServiceProgressRoute>,
}

impl PackagePolicyServiceProgressPremise {
    pub fn profile(&self) -> &PackageReviewNominalIdentity {
        &self.profile
    }
    pub fn subject(&self) -> ServiceProgressSubject {
        self.subject
    }
    pub fn subject_projections(&self) -> &[PackageReviewNominalIdentity] {
        &self.subject_projections
    }
    pub fn establishment_routes(&self) -> &[PackagePolicyServiceProgressRoute] {
        &self.establishment_routes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyServiceProgressRoute {
    pub(crate) kind: ServiceProgressEstablishmentRouteKind,
    pub(crate) requirement_owner: PackageReviewNominalIdentity,
    pub(crate) requirement: PackageReviewNominalIdentity,
}

impl PackagePolicyServiceProgressRoute {}

impl super::PackagePolicyServiceMethod {
    pub(super) fn validate_authority(&self) -> Result<(), &'static str> {
        let authority = &self.authority;
        ordered(&authority.service_reach)?;
        ordered(&authority.synchronous_invocations)?;
        ordered(&authority.progress_premises)?;
        if (!self.terminates_guarantee && !authority.progress_premises.is_empty())
            || authority.progress_premises.len() != self.termination_premises.len()
        {
            return Err("provider authority detaches its progress premises");
        }
        nominal_set(&authority.service_reach)?;
        for invocation in &authority.synchronous_invocations {
            match invocation {
                PackageReviewSynchronousInvocation::Parameter(ordinal) => {
                    if *ordinal as usize >= self.parameter_count {
                        return Err("provider invocation parameter is outside its telescope");
                    }
                }
                PackageReviewSynchronousInvocation::Service(service) => {
                    service.validate_resolved()?
                }
            }
        }
        for premise in &authority.progress_premises {
            premise.profile.validate_resolved()?;
            if matches!(premise.subject, ServiceProgressSubject::Parameter(ordinal) if ordinal >= self.parameter_count)
            {
                return Err("provider progress premise has an invalid subject");
            }
            ordered(&premise.establishment_routes)?;
            for projection in &premise.subject_projections {
                projection.validate_resolved()?;
            }
            for route in &premise.establishment_routes {
                route.requirement_owner.validate_resolved()?;
                route.requirement.validate_resolved()?;
                if route.requirement_owner.owner != route.requirement.owner {
                    return Err("provider progress route changes its declaring owner");
                }
            }
        }
        Ok(())
    }
}
