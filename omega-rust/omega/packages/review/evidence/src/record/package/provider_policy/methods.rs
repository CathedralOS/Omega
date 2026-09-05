use crate::record::*;

use effects::provider_plan::{ServiceEntryClaim, ServiceProgressPremise, ServiceResultClaim};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyServiceMethod {
    pub(crate) name: String,
    pub(crate) requirement_owner: PackageReviewNominalIdentity,
    pub(crate) requirement: PackageReviewNominalIdentity,
    pub(crate) signature: PackagePolicyServiceSignature,
    pub(crate) authority: PackagePolicyServiceAuthority,
    pub(crate) parameter_count: usize,
    pub(crate) parameter_type_identities: Vec<String>,
    pub(crate) entry_claims: Vec<ServiceEntryClaim>,
    pub(crate) has_result: bool,
    pub(crate) result_type_identity: Option<String>,
    pub(crate) result_claims: Vec<ServiceResultClaim>,
    pub(crate) service_reach: Vec<String>,
    pub(crate) synchronous_invocations: Vec<String>,
    pub(crate) may_suspend: bool,
    pub(crate) may_block: bool,
    pub(crate) terminates_guarantee: bool,
    pub(crate) termination_premises: Vec<ServiceProgressPremise>,
    pub(crate) calling: Option<PackagePolicyCallingPlan>,
}

impl PackagePolicyServiceMethod {
    pub fn authority(&self) -> &PackagePolicyServiceAuthority {
        &self.authority
    }
    pub fn signature(&self) -> &PackagePolicyServiceSignature {
        &self.signature
    }
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn requirement_owner(&self) -> &PackageReviewNominalIdentity {
        &self.requirement_owner
    }

    pub fn requirement(&self) -> &PackageReviewNominalIdentity {
        &self.requirement
    }

    pub fn parameter_count(&self) -> usize {
        self.parameter_count
    }

    pub fn parameter_type_identities(&self) -> &[String] {
        &self.parameter_type_identities
    }

    pub fn entry_claims(&self) -> &[ServiceEntryClaim] {
        &self.entry_claims
    }

    pub fn has_result(&self) -> bool {
        self.has_result
    }

    pub fn result_type_identity(&self) -> Option<&str> {
        self.result_type_identity.as_deref()
    }

    pub fn result_claims(&self) -> &[ServiceResultClaim] {
        &self.result_claims
    }

    pub fn service_reach(&self) -> &[String] {
        &self.service_reach
    }

    pub fn synchronous_invocations(&self) -> &[String] {
        &self.synchronous_invocations
    }

    pub fn may_suspend(&self) -> bool {
        self.may_suspend
    }

    pub fn may_block(&self) -> bool {
        self.may_block
    }

    pub fn terminates_guarantee(&self) -> bool {
        self.terminates_guarantee
    }

    pub fn termination_premises(&self) -> &[ServiceProgressPremise] {
        &self.termination_premises
    }

    pub fn calling(&self) -> Option<&PackagePolicyCallingPlan> {
        self.calling.as_ref()
    }
}
