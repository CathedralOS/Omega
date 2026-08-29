#![forbid(unsafe_code)]

//! Policy-filesystem-free trust obligations and derived trust-report evidence.
//!
//! This crate reconstructs what compilation requires and compares that exact
//! set with admissions supplied by the build owner. Policy discovery and
//! mutation belong to a coordinator-facing ledger crate. Optional report
//! emission writes diagnostics only and carries no admission authority.

mod accepted_templates;
mod admissions;
mod grants;
mod provider_grants;
mod report;

pub use accepted_templates::AcceptedTemplateClassifications;
pub use admissions::{
    TrustAdmission, TrustAdmissionSettlement, reconstruct_trust_obligations,
    settle_trust_admissions,
};
pub use grants::{
    NonProviderTrustGrant, reject_package_non_provider_grants, resolve_non_provider_trust_grant,
};
pub use provider_grants::{
    ProviderGrantSelectorKind, ResolvedSelectedProviderGrant, resolve_selected_provider_grants,
};
pub use report::write_trust_report;
