//! Optimizer module role: stage group. boundary in target operations.
//!
//! These modules own the related program facts; lowering algorithms live in
//! pipeline stages and consume these data types.

mod execution;
pub use execution::*;
mod realizations;
pub use realizations::*;
mod arguments;
pub use arguments::*;
mod fma;
pub use fma::*;
