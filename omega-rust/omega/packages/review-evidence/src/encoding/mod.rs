/*
Canonical byte production and bounded recovery for package-review evidence.

We own stable binary and text framing across the composed package policy baseline and its individual policy components (`external-supply`, `calling`, `representation`, `selected-provider`, `terminal-permission`, and `callable`). Our encoders and recovery readers consume and produce inert evidence under explicit byte, sequence-element, and nesting budgets without inspecting live compiler state or deciding admission authority.
*/
/*
Versioned magic headers and schema version constants live at this boundary so both canonical emission (`encode`) and fail-closed deserialization (`recovery`) agree on one wire identity per component.
*/

mod encode;
mod recovery;
pub use encode::membership::{
    MalformedIdentityExcerpt, PackagePolicyMembershipError, PackagePolicyMembershipFailure,
    PackagePolicyMembershipLimits,
};
pub use encode::restricted_build_request_acceptance_text;
pub(crate) use encode::runtime_permission_rows;

pub use recovery::PackagePolicyRecoveryLimits;
pub(crate) use recovery::{PackagePolicyRecoveryError, PackagePolicyRecoveryUsage};

pub(crate) const PACKAGE_POLICY_BASELINE_VERSION: u16 = 29;
pub(crate) const PACKAGE_POLICY_BASELINE_MAGIC: &[u8] = b"OMEGA-PACKAGE-POLICY\0";

pub(crate) const PACKAGE_EXTERNAL_SUPPLY_POLICY_VERSION: u16 = 11;
pub(crate) const EXTERNAL_SUPPLY_POLICY_MAGIC: &[u8] = b"OMEGA-EXTERNAL-SUPPLY-POLICY\0";

pub(crate) const PACKAGE_CALLING_POLICY_VERSION: u16 = 17;
pub(crate) const CALLING_POLICY_MAGIC: &[u8] = b"OMEGA-CALLING-POLICY\0";

pub(crate) const PACKAGE_REPRESENTATION_POLICY_VERSION: u16 = 15;
pub(crate) const REPRESENTATION_POLICY_MAGIC: &[u8] = b"OMEGA-REPRESENTATION-POLICY\0";

pub(crate) const PACKAGE_SELECTED_PROVIDER_POLICY_VERSION: u16 = 22;
pub(crate) const SELECTED_PROVIDER_POLICY_MAGIC: &[u8] = b"OMEGA-SELECTED-PROVIDER-POLICY\0";

pub(crate) const PACKAGE_TERMINAL_PERMISSION_POLICY_VERSION: u16 = 14;
pub(crate) const TERMINAL_PERMISSION_POLICY_MAGIC: &[u8] = b"OMEGA-TERMINAL-PERMISSION-POLICY\0";

pub(crate) const PACKAGE_CALLABLE_POLICY_VERSION: u16 = 17;
pub(crate) const CALLABLE_POLICY_MAGIC: &[u8] = b"OMEGA-CALLABLE-POLICY\0";

pub use encode::PackageReviewEncodingError;
