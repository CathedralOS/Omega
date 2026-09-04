//! Optimizer module role: stage group. calls in target operations.
//!
//! These modules own the related program facts; lowering algorithms live in
//! pipeline stages and consume these data types.

mod inputs;
pub use inputs::*;
mod abi;
pub use abi::*;
mod arguments;
pub use arguments::*;
