//! Optimizer module role: stage group. operations in target operations.
//!
//! These modules own the related program facts; lowering algorithms live in
//! pipeline stages and consume these data types.

mod unit;
pub use unit::*;
mod operation;
pub use operation::*;
