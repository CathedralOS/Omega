//!
//! `linear_obligations.rs` establishes claims and checks moves,
//! `consumed_owners.rs` rejects uses of an owner after its transfer,
//! `permission_events.rs` records the events, `claim_outcomes.rs` settles
//! which claim each outcome reaches, `linear_validation.rs` walks the
//! statements, and `type_multiplicity.rs` answers what a type carries;
//! `owned_selection`, `projected_affine` and `temporary_results` stay as they
//! were, with the nested-ownership and generic-substitution tests beside
//! them.

mod borrowed_windows;
mod claim_outcomes;
pub(crate) use claim_outcomes::claim_paths_are_case_alternatives;
mod cleanup_dispositions;
mod construction_frontier;
mod consumed_owners;
mod linear_obligations;
mod linear_validation;
mod owned_cleanup;
mod owned_selection;
pub(super) use owned_selection::reachable_arms;
pub(super) use projected_affine::transfers_owned_storage;
mod instruction_dispositions;
mod permission_events;
mod permission_schedule;
mod returned_claims;
pub(crate) use returned_claims::{ReturnedClaim, ReturnedClaimSource, current_returned_claims};
mod selected_cleanup;
mod selected_evaluation;
pub(crate) use permission_events::prepare_permission_events;
mod projected_affine;
mod temporary_results;
mod type_multiplicity;

pub(crate) use linear_obligations::{
    check_linear_obligations, nominal_drop_machine_symbol, type_reference_is_reference,
};
pub(crate) use linear_validation::linear_claim_frontier;
pub(crate) use temporary_results::owned_temporary_type;
pub(crate) use type_multiplicity::{
    data_field_name, type_carries_linear_obligation, type_multiplicity,
};
