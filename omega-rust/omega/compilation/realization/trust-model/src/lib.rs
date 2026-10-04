#![forbid(unsafe_code)]

//! Realization trust obligations and their admission against the build
//! owner's accepted set.
//!
//! This crate reconstructs what compilation requires and compares that exact
//! set with admissions supplied by the build owner. Policy discovery and
//! mutation belong to the CLI ledger crate. Start at `admission_settlement.rs`:
//! `reconstruct_trust_settlement` owns obligation reconstruction and admission
//! comparison for one checked program. `root_grants.rs` holds invocation-local
//! grant meanings and package-grant policy. Admission does not depend on host
//! artifact writing or compiler orchestration. Source judgments still
//! arrive as checked trees; this is not a Terminal-only consumer yet.

mod accepted_machine_grants;
mod accepted_templates;
mod admission_settlement;
mod behavior_exclusions;
mod component_assumptions;
use effects::provider_grants;
mod root_grants;
mod trust_admission;

pub use accepted_machine_grants::{NonProviderTrustGrant, resolve_non_provider_trust_grant};
pub use accepted_templates::{AcceptedTemplateClassifications, AcceptedTemplateIdentity};
pub use admission_settlement::{
    TrustAdmissionSettlement, reconstruct_trust_obligations, reconstruct_trust_settlement,
    settle_trust_admissions,
};
pub use behavior_exclusions::{harvest_behavior_exclusions, verify_entry_behavior_exclusions};
pub use component_assumptions::{
    AuthoredComponentAssumptionAcceptance, harvest_component_assumption_acceptances,
};
pub use provider_grants::{
    AuthoredRootGrant, ProviderGrantSelectorKind, ResolvedAuthoredSelectedProviderGrant,
    ResolvedSelectedProviderGrant, resolve_authored_selected_provider_grants,
    resolve_selected_provider_grants,
};
pub use root_grants::{harvest_root_grants, reject_package_non_provider_grants};
pub use trust_admission::{TrustAdmission, TrustAdmissionDigest};
