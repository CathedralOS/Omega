#![forbid(unsafe_code)]

//! Canonical component descriptions and their source-free verification.
//!
//! A description is the bounded, byte-replayed carrier that publishes a
//! checked component's exact semantic subject, entry roster, outgoing
//! authority, custody constraints, retained providers, and installation
//! obligations. It embeds the canonical Terminal artifact and nothing from
//! native realization: the crate depends on the Terminal codec, the Terminal
//! verifier and its admission profile, the selected provider-plan facts, and
//! the shared semantic vocabulary only, so build planning may consume
//! verified components without importing image emission, native artifacts,
//! or any installation owner.
//!
//! Start at `component_description.rs` for the carrier, its producer
//! `describe_component_facts`, and the canonical codec. The independent
//! consumer `verify_component` in `component_verification.rs` re-decodes the
//! embedded artifact, verifies the module under the caller's admission
//! profile, and re-derives every module-evident row, returning the
//! evidence-only `VerifiedComponent` whose `realizes_selected_plan` joins a
//! build-selected provider plan to the component's exported realizations.
//! The native-realization producer that fills these facts from a
//! `ComponentCandidate` lives in `component-candidate`. `test_support`
//! (feature `test-support`) builds canonical provider-component modules for
//! consumer tests that need a real described-and-verified component.

pub mod component_description;
pub mod component_verification;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;

pub use component_description::{
    COMPONENT_DESCRIPTION_SCHEMA_V1, ComponentDescription, ComponentDescriptionFacts,
    ComponentEntry, ComponentEntryKind, CustodyConstraint, CustodyEvidence, CustodyKind,
    DescribeError, DescriptionDecodeRejection, DescriptionFrontier, EntryEvidence, ExportSurface,
    ImportSlot, InstallationObligation, ObligationKind, OutgoingAuthority, OutgoingAuthorityClass,
    OutgoingEvidence, RetainedProvider, StackDemandFacts, component_description_identity,
    decode_component_description, describe_component_facts, description_subject,
    encode_component_description, port_mechanism_assumption, requirement_contract_identity,
    requirement_export_identity,
};
pub use component_verification::{
    AdmissionProfile, ComponentVerificationRejection, ComponentVerificationRequest,
    IndependentRealizationMismatch, VerifiedComponent, verify_component,
};
