//! Optimizer module role: stage group. values in target operations.
//!
//! These modules own the related program facts; lowering algorithms live in
//! pipeline stages and consume these data types.

mod structural;
pub use structural::*;
mod byte_view;
pub use byte_view::*;
mod scalar;
pub use scalar::*;
mod boolean;
pub use boolean::*;
mod integer;
pub use integer::*;
