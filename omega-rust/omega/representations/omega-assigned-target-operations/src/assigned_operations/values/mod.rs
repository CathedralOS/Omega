//! Optimizer module role: stage group. assigned operations values.
//!
//! These modules own related program facts at this representation boundary.
//! Transformation and independent source-to-target replay remain in pipeline.

mod scalar;
pub use scalar::*;
mod boolean;
pub use boolean::*;
mod integer;
pub use integer::*;
