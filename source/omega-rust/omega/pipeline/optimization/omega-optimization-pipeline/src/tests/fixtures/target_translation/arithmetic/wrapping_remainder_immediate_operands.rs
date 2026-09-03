//! Tiny policy wrapper for proof-bearing wrapping remainder over constant operands.

use super::super::*;
use super::nonzero_divisor_immediate_operands::{
    NonzeroDivisorIntegerOperation, nonzero_divisor_integer_immediate_operands_return_artifact,
};

pub(crate) fn wrapping_integer_remainder_immediate_operands_return_artifact(
    scalar_type: IntegerType,
    left: IntegerValue,
    right: IntegerValue,
) -> (Vec<u8>, Vec<u8>) {
    nonzero_divisor_integer_immediate_operands_return_artifact(
        scalar_type,
        left,
        right,
        NonzeroDivisorIntegerOperation::WrappingRemainder,
    )
}
