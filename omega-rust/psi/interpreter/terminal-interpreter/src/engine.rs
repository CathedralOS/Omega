//! The interpreter loop and what it dispatches. `execution` is the live state
//! and the loop itself; `calls`, `terminators`, `scalar_operations`,
//! `structural_operations` and `atomic_operations` are the operation families
//! it steps; `custody` owns claims, the affine frontier and argument binding;
//! `capabilities` decides what the admitted projection may do before any
//! effect; `content_transformations` stages explicit content transactions.

pub(crate) mod atomic_operations;
pub(crate) mod calls;
pub(crate) mod capabilities;
pub(crate) mod content_transformations;
pub(crate) mod custody;
pub(crate) mod execution;
pub(crate) mod native_code_layouts;
pub(crate) mod scalar_operations;
pub(crate) mod structural_operations;
pub(crate) mod terminators;
