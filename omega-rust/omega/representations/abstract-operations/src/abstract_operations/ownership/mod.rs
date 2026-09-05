//! Optimizer module role: stage group. ownership in abstract operations.
//!
//! These modules own the related program facts; lowering algorithms live in
//! pipeline stages and consume these data types.

mod completion;
pub use completion::*;
