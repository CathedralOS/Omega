//! Optimizer module role: stage group. calls in abstract operations.
//!
//! These modules own the related program facts; lowering algorithms live in
//! pipeline stages and consume these data types.

mod access_inputs;
pub use access_inputs::AbstractOperationPlanWithAccessViewInputs;
mod results;
pub use results::{AbstractBoundaryResult, AbstractStructuralCallResult};
mod targets;
pub use targets::AbstractCallTarget;
mod signatures;
pub use signatures::{
    AbstractCallSignature, StructuralCallFormal, StructuralCallResult, call_source,
};
