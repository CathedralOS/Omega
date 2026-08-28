#![forbid(unsafe_code)]

//! Trust-grant resolution, receipt custody, and disclosed trust reporting.
//!
//! The compiler supplies checked semantic facts and selected provider plans;
//! this crate owns the trust ledger derived from those facts. It neither
//! coordinates compilation nor grants itself authority.

mod accepted_templates;
mod lockfile;
mod provider_grants;
mod report;

pub use accepted_templates::AcceptedTemplateClassifications;
pub use lockfile::{
    PreparedTrustLock, enforce_trust_lockfile, prepare_trust_lockfile,
    reject_package_non_provider_grants,
};
pub use provider_grants::{
    ProviderGrantSelectorKind, ResolvedSelectedProviderGrant, resolve_selected_provider_grants,
};
pub use report::write_trust_report;
