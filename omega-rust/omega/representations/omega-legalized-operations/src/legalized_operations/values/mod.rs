//! Optimizer module role: stage group. legalized operations values.
//!
//! These modules own related program facts at this representation boundary.
//! Transformation and independent source-to-target replay remain in pipeline.

mod conditions;
pub use conditions::*;
mod leaves;
pub use leaves::*;
