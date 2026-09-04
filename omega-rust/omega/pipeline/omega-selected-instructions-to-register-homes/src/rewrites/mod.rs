//! Optimizer module role: stage group. Selected-instruction rewrites and retained replay evidence.

mod fixed_view;
mod literal_folds;
mod rematerialization;

pub use fixed_view::*;
pub use literal_folds::*;
pub use rematerialization::*;
