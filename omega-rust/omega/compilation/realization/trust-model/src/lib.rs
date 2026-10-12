#![forbid(unsafe_code)]

//! Realization trust obligations established by the root's grants.
//!
//! A grant in the root's build declaration is the root's acceptance; dependency
//! claims are accepted through the package lock. Start at
//! `admission_settlement.rs`: `reconstruct_trust_obligations` resolves each
//! grant to its exact claim or provider plan for one checked program. `root_grants.rs` holds invocation-local
//! grant meanings and package-grant policy. Admission does not depend on host
//! artifact writing or compiler orchestration. Source judgments still
//! arrive as checked trees; this is not a Terminal-only consumer yet.
//!
//! `AssumedBindingRelationship` is a separate runtime assumption obligation.
//! Its complete content commitment and receiving-consent receipt never grant
//! provider rights or create a checked conformance. The settlement producer
//! supplies independent compatibility evidence; package review authenticates
//! exact receiving consent. No descriptor is inferred from Build intent here.

pub(crate) use optimization_core::digest_encoder;
mod accepted_machine_grants;
mod accepted_templates;
mod admission_settlement;
mod assumed_bindings;
mod behavior_exclusions;
mod component_assumptions;
use effects::provider_grants;
mod root_grants;
mod trust_admission;

pub(crate) use accepted_machine_grants::{NonProviderTrustGrant, resolve_non_provider_trust_grant};
pub use accepted_templates::AcceptedTemplateClassifications;
pub use admission_settlement::reconstruct_trust_obligations;
pub use assumed_bindings::{
    AcceptedAssumedBindingRelationship, AssumedBindingRelationship,
    AssumedBindingRelationshipCommitments, AssumedBindingRelationshipDigest,
    AssumedBindingRequirementCommitments, accept_assumed_binding_relationship,
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
pub use trust_admission::TrustAdmission;
