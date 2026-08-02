#![forbid(unsafe_code)]

//! Self-contained, target-neutral terminal-Psi semantics.
//!
//! The frozen v1 vocabulary contains integer constants and a straight-line
//! chain of explicit jump/return edges; current v2 also contains Boolean
//! constants. This is the smallest executable slice that exercises values,
//! control, and bodyful contracts without pretending that branching or
//! arithmetic policy has already been specified. Every later operation extends
//! this vocabulary together with its execution transition, generated facts,
//! proof rule, and lowering contract.

mod identity;
mod module;

pub use identity::*;
pub use module::*;
