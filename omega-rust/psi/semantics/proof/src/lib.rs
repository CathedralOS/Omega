//! Proof surface collection and proof-obligation building.
//!
//! This root collects every proposition, domain, and capability-contract surface
//! a program declares; `obligations` turns them into the obligations checking
//! must discharge, `lemmas` holds the reusable lemma library, `checker` decides
//! obligations, and `boundary` owns the boundary-facing proof contract.
//!
//! `proof_surface.rs` is the root: the report and its collection from syntax
//! trees. `obligations` turns the surface into obligations, `checker` checks
//! them, `lemmas` supplies the lemma vocabulary and `boundary` the boundary
//! facts.

pub mod boundary;
pub mod checker;
pub mod lemmas;
pub mod obligations;
mod proof_surface;
#[cfg(test)]
mod tests;

pub use proof_surface::{
    BoundedTypeSite, ContractKindSurface, ContractSurface, DomainSurface, ProofSurfaceReport,
    PropositionBodySurface, PropositionSurface, build_proof_surface_report,
};
