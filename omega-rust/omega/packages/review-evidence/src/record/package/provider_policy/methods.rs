use crate::record::{
    PackagePolicyCallingPlan, PackagePolicyServiceAuthority, PackagePolicyServiceSignature,
    PackageReviewNominalIdentity,
};

use crate::record::accessors::field_projection_accessors;
use effects::provider_plan::{
    ProviderIdentityTable, ServiceEntryClaim, ServiceProgressPremise, ServiceResultClaim,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyServiceMethod {
    pub(crate) completion: semantic_vocabulary::BoundaryCompletion,
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
    /// The interned identities this method's embedded plan-side fields point
    /// into; capture clones the schema's table, decoding rebuilds an
    /// equivalent one from the same canonical texts.
    pub(crate) identities: ProviderIdentityTable,
    pub(crate) calling: Option<PackagePolicyCallingPlan>,
}

impl PackagePolicyServiceMethod {
    crate::record::accessors::record_accessors!(requirement);
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

    field_projection_accessors!(sliced result_claims: &[ServiceResultClaim]);
    field_projection_accessors!(sliced service_reach: &[String]);
    field_projection_accessors!(sliced synchronous_invocations: &[String]);
    field_projection_accessors!(copied may_suspend: bool, may_block: bool);
    field_projection_accessors!(copied terminates_guarantee: bool);
    field_projection_accessors!(sliced termination_premises: &[ServiceProgressPremise]);

    pub fn calling(&self) -> Option<&PackagePolicyCallingPlan> {
        self.calling.as_ref()
    }
}
