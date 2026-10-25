//! Content coordinate of an explicitly selected function-relative rewrite.
//!
//! The optimizer owns construction and replay; this identity grants neither.

register_homes::identity_newtype! { pub struct X86BranchRelaxationIdentity([u8; 32]); }

pub mod relaxation;
pub mod relaxation_identity;
pub use relaxation::{
    X86BranchRelaxationAction, X86BranchRelaxationAttempt, X86BranchRelaxationAttemptOutcome,
    X86BranchRelaxationPolicy, X86BranchRelaxationRevisionIdentity,
};
