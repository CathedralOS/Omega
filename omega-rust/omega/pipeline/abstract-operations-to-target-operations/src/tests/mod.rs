//! Optimizer module role: stage group.
use super::*;

mod dynamic_dispatch;
mod native_boundaries;
mod native_callback_arguments;
mod prelude;
mod projected_result_cleanup;
mod projected_result_qualifications;
mod ranked_countdown;
mod scalar;
mod scalar_abi;
mod scalar_primitive_stores;
mod shared_view_calls;
mod structural_and_cleanup;
mod structural_borrows;
mod support;
mod translation_validation_byte_sequence_literal_unit_return;
mod translation_validation_ieee_float_literal_sequence_unit_return;
mod translation_validation_ieee_float_literal_unit_return;
mod translation_validation_integer_ieee_float_literal_sequence_unit_return;
mod translation_validation_integer_literal_sequence_unit_return;
mod translation_validation_integer_literal_unit_return;
mod translation_validation_nearest_ieee_float_fused_multiply_add_unit_return;
mod translation_validation_port_write_unit_return;
mod translation_validation_trivial_affine_local_unit_return;
mod translation_validation_unit_call_return;
mod translation_validation_unit_return;
mod unit_and_settlements;
mod unit_ieee_stores;
mod unit_scalar_calls;
mod unit_structural_calls;
mod unit_structural_scalar;

use prelude::*;
pub(super) use support::identity;
