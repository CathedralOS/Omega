/*
Canonical byte production and bounded recovery for package-review evidence.

We own stable binary and text framing across the composed package policy baseline and its individual policy components (`external-supply`, `calling`, `representation`, `selected-provider`, `terminal-permission`, and `callable`). Our encoders and recovery readers consume and produce inert evidence under explicit byte, sequence-element, and nesting budgets without inspecting live compiler state or deciding admission authority.
*/
/*
Versioned magic headers and schema version constants live at this boundary so both canonical emission (`encode`) and fail-closed deserialization (`recovery`) agree on one wire identity per component.
*/

mod encode;
mod intrinsic_tags;
mod recovery;
pub use encode::membership::{
    MalformedIdentityExcerpt, PackagePolicyMembershipError, PackagePolicyMembershipFailure,
    PackagePolicyMembershipLimits,
};
pub(crate) use encode::runtime_permission_rows;
pub use encode::{restricted_build_request_acceptance_text, restricted_build_request_payload};

pub use recovery::PackagePolicyRecoveryLimits;
pub(crate) use recovery::{PackagePolicyRecoveryError, PackagePolicyRecoveryUsage};

// Calling plans distinguish Darwin integer composites from scalar values.
// Every component embedding a physical calling application advertises the
// expanded class vocabulary even though existing class tags are unchanged.
// Ordinary public requirements preserve open reach/crash separately from
// authored empty ceilings; predecessor payloads are not reinterpreted.
// Domain records distinguish unrestricted issuance from an empty permitted issuer set.
// Service method records in each of these layouts carry the foreign caller-stack bound.
// Native callers also retain their selected function field and formal mapping.
// These baseline layouts omit the retired assumed-binding roster.
pub(crate) const BASELINE_WITHOUT_STRUCTURAL_PARAMETERS: u16 = 61;
pub(crate) const BASELINE_WITH_STRUCTURAL_PARAMETERS: u16 = 62;
pub(crate) const BASELINE_WITH_PROVIDER_APPLICATION_GRANTS: u16 = 63;
pub(crate) const BASELINE_WITH_TYPE_ALIASES: u16 = 64;
pub(crate) const BASELINE_WITH_CARRIED_TYPES: u16 = 65;
pub(crate) const BASELINE_WITH_APPLIED_SERVICES: u16 = 66;
pub(crate) const BASELINE_WITH_UNUSED_GENERIC_IMPORTS: u16 = 67;
pub const PACKAGE_POLICY_BASELINE_VERSION: u16 = BASELINE_WITH_UNUSED_GENERIC_IMPORTS;
pub(crate) const PACKAGE_POLICY_BASELINE_MAGIC: &[u8] = b"OMEGA-PACKAGE-POLICY\0";

pub(crate) const PACKAGE_EXTERNAL_SUPPLY_POLICY_VERSION: u16 = 11;
pub(crate) const EXTERNAL_SUPPLY_POLICY_MAGIC: &[u8] = b"OMEGA-EXTERNAL-SUPPLY-POLICY\0";

pub(crate) const PACKAGE_CALLING_POLICY_VERSION: u16 = 18;
pub(crate) const CALLING_POLICY_MAGIC: &[u8] = b"OMEGA-CALLING-POLICY\0";

pub(crate) const PACKAGE_REPRESENTATION_POLICY_VERSION: u16 = 15;
pub(crate) const REPRESENTATION_POLICY_MAGIC: &[u8] = b"OMEGA-REPRESENTATION-POLICY\0";

pub(crate) const PROVIDER_POLICY_WITHOUT_APPLICATION_GRANTS: u16 = 33;
pub(crate) const PROVIDER_POLICY_WITH_APPLICATION_GRANTS: u16 = 34;
pub(crate) const PACKAGE_SELECTED_PROVIDER_POLICY_VERSION: u16 = 35;
pub(crate) const SELECTED_PROVIDER_POLICY_MAGIC: &[u8] = b"OMEGA-SELECTED-PROVIDER-POLICY\0";

pub(crate) const TERMINAL_POLICY_WITH_NOMINAL_SERVICES: u16 = 19;
pub(crate) const PACKAGE_TERMINAL_PERMISSION_POLICY_VERSION: u16 = 20;
pub(crate) const TERMINAL_PERMISSION_POLICY_MAGIC: &[u8] = b"OMEGA-TERMINAL-PERMISSION-POLICY\0";

pub(crate) const CALLABLE_POLICY_WITH_NOMINAL_SERVICES: u16 = 21;
pub(crate) const PACKAGE_CALLABLE_POLICY_VERSION: u16 = 22;
pub(crate) const CALLABLE_POLICY_MAGIC: &[u8] = b"OMEGA-CALLABLE-POLICY\0";

pub use encode::PackageReviewEncodingError;
