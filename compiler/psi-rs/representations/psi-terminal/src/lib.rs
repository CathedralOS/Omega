#![forbid(unsafe_code)]

//! Self-contained, target-neutral terminal-Psi semantics.
//!
//! The initial vocabulary deliberately contains only integer constants and a
//! straight-line chain of explicit jump/return edges. This is the smallest
//! executable slice that exercises values, control, and bodyful contracts
//! without pretending that branching or arithmetic policy has already been
//! specified. Every later operation extends this vocabulary together with its
//! execution transition, generated facts, proof rule, and lowering contract.

mod module;

pub use module::*;
