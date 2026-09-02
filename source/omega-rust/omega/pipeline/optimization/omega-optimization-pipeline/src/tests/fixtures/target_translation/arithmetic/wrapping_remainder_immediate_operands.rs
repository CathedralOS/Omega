//! Tiny policy wrapper for proof-bearing wrapping remainder over constant operands.

use super::super::*;
use super::wrapping_nonzero_immediate_operands::{
    wrapping_nonzero_integer_immediate_operands_return_artifact, WrappingNonzeroIntegerOperation,
};

pub(crate) fn wrapping_integer_remainder_immediate_operands_return_artifact(
    scalar_type: IntegerType,
    left: IntegerValue,
    right: IntegerValue,
) -> (Vec<u8>, Vec<u8>) {
    wrapping_nonzero_integer_immediate_operands_return_artifact(
        scalar_type,
        left,
        right,
        WrappingNonzeroIntegerOperation::Remainder,
    )
}
