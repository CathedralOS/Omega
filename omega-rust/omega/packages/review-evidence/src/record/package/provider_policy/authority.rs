//! Source-qualified operational authority, independently of readable schemas.

use crate::record::accessors::field_projection_accessors;
use crate::record::package::callable_policy::validation::nominal_set;
use crate::record::package::callable_policy::validation::signature::owned_pair;
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
    field_projection_accessors!(sliced service_reach: &[PackageReviewNominalIdentity]);
    field_projection_accessors!(sliced synchronous_invocations: &[PackageReviewSynchronousInvocation]);
    field_projection_accessors!(sliced progress_premises: &[PackagePolicyServiceProgressPremise]);
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyServiceProgressPremise {
    pub(crate) profile: PackageReviewNominalIdentity,
    pub(crate) subject: ServiceProgressSubject,
    pub(crate) subject_projections: Vec<PackageReviewNominalIdentity>,
    pub(crate) establishment_routes: Vec<PackagePolicyServiceProgressRoute>,
}

impl PackagePolicyServiceProgressPremise {
    field_projection_accessors!(borrowed profile: &PackageReviewNominalIdentity);
    field_projection_accessors!(copied subject: ServiceProgressSubject);
    field_projection_accessors!(sliced subject_projections: &[PackageReviewNominalIdentity]);
    field_projection_accessors!(sliced establishment_routes: &[PackagePolicyServiceProgressRoute]);
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
                owned_pair(&route.requirement_owner, &route.requirement)?;
            }
        }
        Ok(())
    }
}
