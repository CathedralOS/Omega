//! Optimizer module role: stage group. legalized operations control flow.
//!
//! These modules own related program facts at this representation boundary.
//! Transformation and independent source-to-target replay remain in pipeline.

mod scalar;
pub use scalar::*;

mod scalar_graph;
pub use scalar_graph::*;
