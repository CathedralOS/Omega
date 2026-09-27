#![forbid(unsafe_code)]

//! Policy-filesystem-free trust obligations and derived trust-report evidence.
//!
//! This crate reconstructs what compilation requires and compares that exact
//! set with admissions supplied by the build owner. Policy discovery and
//! mutation belong to a coordinator-facing ledger crate. Derived report
//! evidence is filesystem-free; an observation coordinator may render it but
//! rendering carries no admission authority. Start at `admission_settlement.rs`:
//! `reconstruct_trust_settlement` owns obligation reconstruction, admission
//! comparison, and report validation for one checked program. `root_grants.rs`
//! holds invocation-local grant meanings and package-grant policy;
//! `trust_report` independently reconstructs reports for other callers.

mod accepted_machine_grants;
mod accepted_templates;
mod admission_settlement;
mod provider_grants;
mod root_grants;
mod trust_admission;
mod trust_report;

pub use accepted_machine_grants::{NonProviderTrustGrant, resolve_non_provider_trust_grant};
pub use accepted_templates::{AcceptedTemplateClassifications, AcceptedTemplateIdentity};
pub use admission_settlement::{
    TrustAdmissionSettlement, reconstruct_trust_obligations, reconstruct_trust_settlement,
    settle_trust_admissions,
};
pub use provider_grants::{
    AuthoredRootGrant, ProviderGrantSelectorKind, ResolvedAuthoredSelectedProviderGrant,
    ResolvedSelectedProviderGrant, resolve_authored_selected_provider_grants,
    resolve_selected_provider_grants,
};
pub use root_grants::reject_package_non_provider_grants;
pub use trust_admission::{TrustAdmission, TrustAdmissionDigest};
pub use trust_report::reconstruct_trust_report;
