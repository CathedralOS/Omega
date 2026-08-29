//! Explicit, independently validated machine-lowering transformations.

pub(crate) mod fixed_view_copy;
pub(crate) mod literal_fold;
pub(crate) mod pressure_rematerialization;

pub use fixed_view_copy::*;
pub use literal_fold::*;
pub use pressure_rematerialization::*;
