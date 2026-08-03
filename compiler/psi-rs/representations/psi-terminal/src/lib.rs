#![forbid(unsafe_code)]

//! Self-contained, target-neutral terminal-Psi semantics.
//!
//! The frozen v1 vocabulary contains integer constants and a straight-line
//! chain of explicit jump/return edges; v2 adds Boolean constants, v3 adds
//! explicit wrapping integer addition, v4 adds saturating integer addition,
//! v5 adds wrapping integer subtraction, v6 adds saturating integer
//! subtraction, v7 adds wrapping integer multiplication, v8 adds saturating
//! integer multiplication, v9 adds proof-only structural-place content
//! conservation, v10 adds identity-preserving claim reshuffles, v11 adds
//! stable sum-case structural paths, and current v12 adds exact authored
//! partition substitutions.
//! This small executable slice
//! exercises values, control, bodyful contracts, and one width-relative
//! arithmetic policy without pretending that branching or other arithmetic
//! policies have already been specified. Every later operation extends this
//! vocabulary together with its execution transition, generated facts, proof
//! rule, and lowering contract.

mod identity;
mod module;

pub use identity::*;
pub use module::*;
