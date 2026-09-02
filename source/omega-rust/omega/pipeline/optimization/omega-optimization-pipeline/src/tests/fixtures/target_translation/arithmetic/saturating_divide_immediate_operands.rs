//! Tiny policy wrapper for proof-bearing saturating divide over constant operands.

use super::super::*;
use super::nonzero_divisor_immediate_operands::{
    nonzero_divisor_integer_immediate_operands_return_artifact, NonzeroDivisorIntegerOperation,
};

pub(crate) fn saturating_integer_divide_immediate_operands_return_artifact(
    scalar_type: IntegerType,
    left: IntegerValue,
    right: IntegerValue,
) -> (Vec<u8>, Vec<u8>) {
    nonzero_divisor_integer_immediate_operands_return_artifact(
        scalar_type,
        left,
        right,
        NonzeroDivisorIntegerOperation::SaturatingDivide,
    )
}
