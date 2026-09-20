//! Optimizer module role: stage group.

mod dynamic_parameters;
mod indexed_field_reads;
mod native_boundaries;
mod native_callback_arguments;
mod normalized_foreign_calls;
mod partial_affine;
mod prelude;
mod scalar;
mod scalar_abi;
mod scalar_primitive_stores;
mod shared_view_calls;
mod structural_borrows;
mod support;
mod unit_ieee_stores;
mod unit_scalar_calls;

use prelude::*;
pub(super) use support::identity;
