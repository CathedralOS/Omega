//! Optimizer module role: stage group.
use super::*;

mod native_boundaries;
mod native_callback_arguments;
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
