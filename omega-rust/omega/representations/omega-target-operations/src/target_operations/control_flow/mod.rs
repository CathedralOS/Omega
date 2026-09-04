//! Optimizer module role: stage group. control flow in target operations.
//!
//! These modules own the related program facts; lowering algorithms live in
//! pipeline stages and consume these data types.

mod functions;
pub use functions::*;
mod edges;
pub use edges::*;
mod ranking;
pub use ranking::*;
mod boolean;
pub use boolean::*;
mod integer;
pub use integer::*;
