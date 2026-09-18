//!
//! `linear_obligations.rs` establishes claims and checks moves,
//! `permission_events.rs` records the events, `claim_outcomes.rs` settles
//! which claim each outcome reaches, `linear_validation.rs` walks the
//! statements, and `type_multiplicity.rs` answers what a type carries;
//! `owned_selection`, `projected_affine` and `temporary_results` stay as they
//! were, with the nested-ownership and generic-substitution tests beside
//! them.

mod borrowed_windows;
mod claim_outcomes;
#[cfg(test)]
mod generic_substitution_tests;
mod linear_obligations;
mod linear_validation;
#[cfg(test)]
mod nested_ownership_tests;
mod owned_selection;
mod permission_events;
mod projected_affine;
mod temporary_results;
mod type_multiplicity;

pub(crate) use linear_obligations::{check_linear_obligations, nominal_drop_machine_symbol};
pub(crate) use linear_validation::linear_claim_frontier;
#[cfg(test)]
pub(crate) use linear_validation::validate_linear_permission_events;
#[cfg(test)]
pub(crate) use permission_events::record_permission_events;
pub(crate) use type_multiplicity::{type_carries_linear_obligation, type_multiplicity};
