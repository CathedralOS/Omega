//! Optimizer module role: stage group. Scalar operation semantics used by the common control graph.

pub(in crate::lowering) mod byte_views;
mod expressions;
mod integer_binary;
mod integer_operation;
mod shift;

use super::shared::*;
use expressions::*;
pub(in crate::lowering) use expressions::{
    KnownInteger, KnownScalar, equal_boolean, equal_integer, negate_boolean, order_integer,
    scalar_parameter_location, scalar_shape,
};
pub(in crate::lowering) use integer_binary::{IntegerBinaryKind, lower_integer_binary};
pub(in crate::lowering) use integer_operation::try_lower_integer_operation;
use shift::{
    WrappingShiftKind, lower_exact_shift_left, lower_exact_shift_right, lower_wrapping_shift,
};
