//! Optimizer module role: stage group. control flow in target operations.
//!
//! These modules own the related program facts; lowering algorithms live in
//! pipeline stages and consume these data types.

mod functions;
pub use functions::*;
mod graph;
pub use graph::*;
