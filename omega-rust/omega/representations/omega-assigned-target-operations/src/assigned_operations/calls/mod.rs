//! Optimizer module role: stage group. assigned operations calls.
//!
//! These modules own related program facts at this representation boundary.
//! Transformation and independent source-to-target replay remain in pipeline.

mod native_inputs;
pub use native_inputs::*;
mod arguments;
pub use arguments::*;
