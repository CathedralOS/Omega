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
//! `describe_component_facts`, and the canonical codec. The
//! serialized-publication entrance in `publication.rs` derives the description
//! and its producer-observed subject from one decoded module. It produces no
//! verification evidence; consuming policy still selects its expected subject.
//! Independent consumer `verify_component` in `component_verification.rs` re-decodes the
//! embedded artifact, verifies the module under the caller's admission
//! profile, and re-derives every module-evident row, returning the
//! evidence-only `VerifiedComponent` whose `realizes_selected_plan` joins a
//! build-selected provider plan to the component's exported realizations.
//! Native compilation may supply the same explicit facts directly when it
//! publishes a component product.
//! builds canonical provider-component modules for consumer tests that need a
//! real described-and-verified component.

pub mod component_description;
pub(crate) use optimization_core::digest_encoder;
pub(crate) mod component_verification;
mod publication;
pub use component_description::{
    COMPONENT_DESCRIPTION_SCHEMA_V2, ComponentDescriptionFacts, DescribeError,
    decode_component_description, description_subject, encode_component_description,
};
pub use component_verification::{
    ComponentVerificationRejection, ComponentVerificationRequest, IndependentRealizationMismatch,
    VerifiedComponent, verify_component,
};
pub use publication::publish_component_description;
