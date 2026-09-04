//! Optimizer module role: stage group. assigned operations operations.
//!
//! These modules own related program facts at this representation boundary.
//! Transformation and independent source-to-target replay remain in pipeline.

mod operation;
pub use operation::*;
mod unit;
pub use unit::*;
