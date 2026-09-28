//! Source-qualified operational authority, independently of readable schemas.

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

impl PackagePolicyServiceProgressRoute {
    pub fn kind(&self) -> ServiceProgressEstablishmentRouteKind {
        self.kind
    }
    pub fn requirement_owner(&self) -> &PackageReviewNominalIdentity {
        &self.requirement_owner
    }
    pub fn requirement(&self) -> &PackageReviewNominalIdentity {
        &self.requirement
    }
}

impl super::PackagePolicyServiceMethod {
    pub(super) fn validate_authority(&self) -> Result<(), &'static str> {
        use super::validation::nominal;
        let authority = &self.authority;
        if authority
            .service_reach
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
            || authority
                .synchronous_invocations
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || authority
                .progress_premises
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || (!self.terminates_guarantee && !authority.progress_premises.is_empty())
            || authority.progress_premises.len() != self.termination_premises.len()
        {
            return Err("provider authority repeats, reorders, or detaches its progress premises");
        }
        for service in &authority.service_reach {
            nominal(service)?;
        }
        for invocation in &authority.synchronous_invocations {
            match invocation {
                PackageReviewSynchronousInvocation::Parameter(ordinal) => {
                    if *ordinal as usize >= self.parameter_count {
                        return Err("provider invocation parameter is outside its telescope");
                    }
                }
                PackageReviewSynchronousInvocation::Service(service) => nominal(service)?,
            }
        }
        for premise in &authority.progress_premises {
            nominal(&premise.profile)?;
            if matches!(premise.subject, ServiceProgressSubject::Parameter(ordinal) if ordinal >= self.parameter_count)
                || premise
                    .establishment_routes
                    .windows(2)
                    .any(|pair| pair[0] >= pair[1])
            {
                return Err("provider progress premise has invalid subject or route ordering");
            }
            for projection in &premise.subject_projections {
                nominal(projection)?;
            }
            for route in &premise.establishment_routes {
                nominal(&route.requirement_owner)?;
                nominal(&route.requirement)?;
                if route.requirement_owner.owner != route.requirement.owner {
                    return Err("provider progress route changes its declaring owner");
                }
            }
        }
        Ok(())
    }
}
