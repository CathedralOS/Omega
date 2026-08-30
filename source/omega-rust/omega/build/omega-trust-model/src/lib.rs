#![forbid(unsafe_code)]

//! Policy-filesystem-free trust obligations and derived trust-report evidence.
//!
//! This crate reconstructs what compilation requires and compares that exact
//! set with admissions supplied by the build owner. Policy discovery and
//! mutation belong to a coordinator-facing ledger crate. Derived report
//! evidence is filesystem-free; an observation coordinator may render it but
//! rendering carries no admission authority.

mod accepted_templates;
mod admissions;
mod grants;
mod provider_grants;
mod report;

pub use accepted_templates::{AcceptedTemplateClassifications, AcceptedTemplateIdentity};
pub use admissions::{
    TrustAdmission, TrustAdmissionDigest, TrustAdmissionSettlement, reconstruct_trust_obligations,
    settle_trust_admissions,
};
pub use grants::{
    NonProviderTrustGrant, reject_package_non_provider_grants, resolve_non_provider_trust_grant,
};
pub use provider_grants::{
    AuthoredRootGrant, ProviderGrantSelectorKind, ResolvedAuthoredSelectedProviderGrant,
    ResolvedSelectedProviderGrant, resolve_authored_selected_provider_grants,
    resolve_selected_provider_grants,
};
pub use report::reconstruct_trust_report;
