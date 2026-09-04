//! Optimizer module role: stage group. control flow in abstract operations.
//!
//! These modules own the related program facts; lowering algorithms live in
//! pipeline stages and consume these data types.

mod ranking;
pub use ranking::*;
mod functions;
pub use functions::*;
mod edges;
pub use edges::*;
