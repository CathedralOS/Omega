//! Optimizer module role: stage group. assigned operations control flow.
//!
//! These modules own related program facts at this representation boundary.
//! Transformation and independent source-to-target replay remain in pipeline.

mod functions;
pub use functions::*;
mod boolean;
pub use boolean::*;
mod integer;
pub use integer::*;
