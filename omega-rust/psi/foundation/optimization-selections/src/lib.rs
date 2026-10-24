#![forbid(unsafe_code)]

//! Optimizer module role: crate map.
//!
//! Target-neutral Psi optimization vocabulary and canonical selections. Psi
//! owns this closed set because it is exactly what a build selection can
//! project onto Psi; target and physical optimization identities remain
//! Omega-owned.

pub mod codec_bytes;
pub mod cursor;

pub mod digest_encoder;
pub mod optimization_selections;
pub mod variant_accessors;

pub use optimization_selections::{
    DuplicatePsiOptimization, PsiOptimization, PsiOptimizationSelectionDecodeError,
    PsiOptimizationSelectionIdentity, PsiOptimizationSelections,
};
