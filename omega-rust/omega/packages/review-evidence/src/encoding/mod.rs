//! Canonical byte production and bounded recovery for package-review evidence.
//!
//! Encoding owns stable framing only. It consumes inert evidence and does not
//! inspect compiler state or decide whether evidence should be admitted.

mod encode;
mod recovery;
pub use encode::membership::{PackagePolicyMembershipError, PackagePolicyMembershipLimits};
pub use encode::restricted_build_request_acceptance_text;

pub use recovery::PackagePolicyRecoveryLimits;
pub(crate) use recovery::{PackagePolicyRecoveryError, PackagePolicyRecoveryUsage};

pub(crate) const PACKAGE_POLICY_BASELINE_VERSION: u16 = 13;
pub(crate) const PACKAGE_POLICY_BASELINE_MAGIC: &[u8] = b"OMEGA-PACKAGE-POLICY\0";

pub(crate) const PACKAGE_EXTERNAL_SUPPLY_POLICY_VERSION: u16 = 7;
pub(crate) const EXTERNAL_SUPPLY_POLICY_MAGIC: &[u8] = b"OMEGA-EXTERNAL-SUPPLY-POLICY\0";

pub(crate) const PACKAGE_CALLING_POLICY_VERSION: u16 = 9;
pub(crate) const CALLING_POLICY_MAGIC: &[u8] = b"OMEGA-CALLING-POLICY\0";

pub(crate) const PACKAGE_REPRESENTATION_POLICY_VERSION: u16 = 8;
pub(crate) const REPRESENTATION_POLICY_MAGIC: &[u8] = b"OMEGA-REPRESENTATION-POLICY\0";

pub(crate) const PACKAGE_SELECTED_PROVIDER_POLICY_VERSION: u16 = 10;
pub(crate) const SELECTED_PROVIDER_POLICY_MAGIC: &[u8] = b"OMEGA-SELECTED-PROVIDER-POLICY\0";

pub(crate) const PACKAGE_TERMINAL_PERMISSION_POLICY_VERSION: u16 = 8;
pub(crate) const TERMINAL_PERMISSION_POLICY_MAGIC: &[u8] = b"OMEGA-TERMINAL-PERMISSION-POLICY\0";

pub(crate) const PACKAGE_CALLABLE_POLICY_VERSION: u16 = 11;
pub(crate) const CALLABLE_POLICY_MAGIC: &[u8] = b"OMEGA-CALLABLE-POLICY\0";

pub use encode::PackageReviewEncodingError;
