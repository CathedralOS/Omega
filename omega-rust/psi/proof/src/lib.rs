//! Typed-program proof obligations and their checking.
//!
//! The checking stage calls `obligations::build_proof_plan` to collect obligations
//! from typed trees, then `checker::check_proof_plan` to discharge them or report
//! diagnostics. `obligations` owns construction and identity; `checker` owns
//! the checking sequence, range reasoning and certificate decisions. Both read
//! the shared type, place and dehoisting queries in `typed_expressions`.
//!
//! `derivation_store` retains certificates by obligation identity; the checker's
//! `derivation_cache` rechecks them before reuse. `lemmas` supplies reusable
//! lemma vocabulary and `boundary` names boundary-facing proof contracts.
//! `proof_surface` separately inventories declared syntax for reporting; it is
//! not the input to typed-program obligation construction.

pub mod boundary;
pub mod checker;
pub mod derivation_store;
pub mod lemmas;
pub mod obligations;
mod proof_surface;
#[cfg(test)]
mod tests;
mod typed_expressions;

pub use proof_surface::{
    BoundedTypeSite, ContractKindSurface, ContractSurface, DomainSurface, ProofSurfaceReport,
    PropositionBodySurface, PropositionSurface, build_proof_surface_report,
};
