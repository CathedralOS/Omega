//! Optimizer module role: stage group. legalized operations control flow.
//!
//! These modules own related program facts at this representation boundary.
//! Transformation and independent source-to-target replay remain in pipeline.

mod unit;
pub use unit::*;
mod scalar;
pub use scalar::*;
