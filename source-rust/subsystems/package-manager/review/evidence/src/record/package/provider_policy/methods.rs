// Service method records and interned identity tables for selected provider plans.

use crate::record::{
    PackagePolicyCallingPlan, PackagePolicyServiceAuthority, PackagePolicyServiceSignature,
    PackageReviewNominalIdentity,
};

use crate::record::accessors::field_projection_accessors;
use service_surface::ServiceSynchronousInvocation;
use service_surface::identities::ProviderIdentityTable;
use service_surface::{ServiceEntryClaim, ServiceProgressPremise, ServiceResultClaim};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyServiceMethod {
    pub completion: semantic_vocabulary::BoundaryCompletion,
    pub name: String,
    pub requirement_owner: PackageReviewNominalIdentity,
    pub requirement: PackageReviewNominalIdentity,
    pub signature: PackagePolicyServiceSignature,
    pub authority: PackagePolicyServiceAuthority,
    pub parameter_count: usize,
    pub parameter_type_identities: Vec<String>,
    pub entry_claims: Vec<ServiceEntryClaim>,
    pub has_result: bool,
    pub result_type_identity: Option<String>,
    pub normal_result_memberships: Vec<service_surface::ServiceNormalResultMembership>,
    pub result_claims: Vec<ServiceResultClaim>,
    pub service_reach: Vec<String>,
    pub synchronous_invocations: Vec<ServiceSynchronousInvocation>,
    pub may_suspend: bool,
    pub may_block: bool,
    /// Foreign stack demand claimed for this requirement. Zero means the source supplies no bound.
    pub admitted_same_stack_bytes: u64,
    pub terminates_guarantee: bool,
    pub termination_premises: Vec<ServiceProgressPremise>,
    pub identities: ProviderIdentityTable, // Interned identities referenced by embedded plan-side fields.
    pub calling: Vec<PackagePolicyCallingPlan>,
}

impl PackagePolicyServiceMethod {
    field_projection_accessors!(borrowed requirement: &PackageReviewNominalIdentity);
    field_projection_accessors!(copied completion: semantic_vocabulary::BoundaryCompletion);
    field_projection_accessors!(borrowed authority: &PackagePolicyServiceAuthority);
    field_projection_accessors!(borrowed signature: &PackagePolicyServiceSignature);
    pub fn name(&self) -> &str {
        &self.name
    }

    field_projection_accessors!(borrowed requirement_owner: &PackageReviewNominalIdentity);

    field_projection_accessors!(copied parameter_count: usize);
    field_projection_accessors!(sliced parameter_type_identities: &[String]);
    field_projection_accessors!(sliced entry_claims: &[ServiceEntryClaim]);
    field_projection_accessors!(copied has_result: bool);

    pub fn result_type_identity(&self) -> Option<&str> {
        self.result_type_identity.as_deref()
    }

    field_projection_accessors!(sliced normal_result_memberships: &[service_surface::ServiceNormalResultMembership]);
    field_projection_accessors!(sliced result_claims: &[ServiceResultClaim]);
    field_projection_accessors!(sliced service_reach: &[String]);
    field_projection_accessors!(sliced synchronous_invocations: &[ServiceSynchronousInvocation]);
    field_projection_accessors!(copied may_suspend: bool, may_block: bool);
    field_projection_accessors!(copied terminates_guarantee: bool);
    field_projection_accessors!(sliced termination_premises: &[ServiceProgressPremise]);

    pub fn calling(&self) -> &[PackagePolicyCallingPlan] {
        &self.calling
    }
}
