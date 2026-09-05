//! Optimizer module role: stage group. Exhaustive binary operation-shape classification map.
//!
//! Classification retains the prior wildcard-free semantic order: shifts,
//! add/subtract/multiply, quotient/remainder, then bitwise operations.

mod arithmetic;
mod bitwise;
mod quotient;
mod shifts;

use abstract_operations::AbstractOperation;

use super::model::IntegerBinaryShape;

pub(super) fn classify(operation: &AbstractOperation) -> Option<IntegerBinaryShape> {
    shifts::classify(operation)
        .or_else(|| arithmetic::classify(operation))
        .or_else(|| quotient::classify(operation))
        .or_else(|| bitwise::classify(operation))
}
