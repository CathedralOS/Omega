//! The authored operand of a selected byte comparison or byte predicate,
//! which the computation node does not yet carry.
use super::{CheckedTrees, ExpressionHandle, ExpressionNode, LoweringError, unsupported};

pub(crate) mod byte_comparisons;
pub(crate) mod byte_predicates;
