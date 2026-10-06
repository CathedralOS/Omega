//! Independent checks of target-neutral rewrites before Terminal publication.

mod copy_propagation;
mod dead_scalar_elimination;
mod proof_check_elision;
mod proof_context_transport;

pub use copy_propagation::{CopyPropagationRewriteError, transport_copy_propagation_evidence};
pub use dead_scalar_elimination::{
    DeadScalarRewriteError, transport_dead_scalar_elimination_evidence,
    validate_dead_scalar_elimination,
};
pub use proof_check_elision::{ProofCheckElisionRewriteError, validate_proof_check_elision};
