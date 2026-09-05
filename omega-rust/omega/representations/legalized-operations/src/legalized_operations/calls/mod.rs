//! Optimizer module role: stage group. legalized operations calls.
//!
//! These modules own related program facts at this representation boundary.
//! Transformation and independent source-to-target replay remain in pipeline.

mod projected_returns;
pub use projected_returns::*;
mod scalar;
pub use scalar::*;
mod structural;
pub use structural::*;
