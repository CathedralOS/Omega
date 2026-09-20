//! Proof surface collection and proof-obligation building.
//!
//! This root collects every proposition, domain, and capability-contract surface
//! a program declares; `obligations` turns them into the obligations checking
//! must discharge, `lemmas` holds the reusable lemma library, `checker` decides
//! obligations, `derivation_store` retains decided derivations under semantic
//! obligation identity for independent re-decision, and `boundary` owns the
//! boundary-facing proof contract.
//!
//! `proof_surface.rs` is the root: the report and its collection from syntax
//! trees. `obligations` turns the surface into obligations, `checker` checks
//! them, `derivation_store` indexes retained derivations by obligation key,
//! `lemmas` supplies the lemma vocabulary and `boundary` the boundary
//! facts.

pub mod boundary;
pub mod checker;
pub mod derivation_store;
pub mod lemmas;
pub mod obligations;
mod proof_surface;
#[cfg(test)]
mod tests;

pub use proof_surface::{
    BoundedTypeSite, ContractKindSurface, ContractSurface, DomainSurface, ProofSurfaceReport,
    PropositionBodySurface, PropositionSurface, build_proof_surface_report,
};
