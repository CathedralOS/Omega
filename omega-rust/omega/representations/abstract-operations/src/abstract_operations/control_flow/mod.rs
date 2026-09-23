//! Optimizer module role: stage group. control flow in abstract operations.
//!
//! These modules own the related program facts; lowering algorithms live in
//! pipeline stages and consume these data types.

pub mod address_joins;
mod functions;
pub use functions::{AbstractBlockEntry, AbstractFunction};
mod edges;
pub use edges::{
    AbstractStructuralBinding, AbstractStructuralCasePayloadBinding,
    AbstractStructuralCaseSuccessor, AbstractSuccessor,
};
