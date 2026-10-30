#![forbid(unsafe_code)]

//! Lowered Psi to the canonical Terminal Psi artifact.
//!
//! The stage operation is `finalize_terminal_program` (`publish_artifact`).
//! It seals source-free semantics with their proof and debug companions, and
//! accepts only the output of the explicit pre-Terminal optimization stage.
//! `entry_receipt` checks source-entry correspondence
//! separately from canonical sealing.

mod entry_receipt;
mod publish_artifact;
mod result_policy_verification;

pub use publish_artifact::finalize_terminal_program;
pub use result_policy_verification::verify_boundary_result_policies;

pub use entry_receipt::{
    ProgramEntryTerminalReceiptError, prepare_program_entry_receipt, rejoin_program_entry_receipt,
    retained_source_machine,
};
