//! Optimizer module role: stage group. Whole-engine custody for every control-flow cleanup row.
//!
//! [`matrix`] names the exact roster. [`custody`] owns the common operational
//! contract, while [`fixtures`] isolates each graph transformation.

mod custody;
mod fixtures;
mod matrix;
