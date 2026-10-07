//! A term-variant destructure for the shared checker and denotation bodies.
//! Every arithmetic `ScalarTerm` carries the same `{scalar_type, left, right}`
//! operand shape, so a matcher is the only per-variant surface a shared body
//! needs.

use semantic_vocabulary::{IntegerMathTerm, IntegerType, ScalarTerm};

pub(crate) type TermMatcher = fn(&ScalarTerm) -> Option<(&IntegerType, &ScalarTerm, &ScalarTerm)>;

pub(crate) fn exact_integer_add(
    term: &ScalarTerm,
) -> Option<(&IntegerType, &ScalarTerm, &ScalarTerm)> {
    match term {
        ScalarTerm::ExactIntegerAdd {
            scalar_type,
            left,
            right,
        } => Some((scalar_type, left, right)),
        _ => None,
    }
}

pub(crate) fn wrapping_integer_add(
    term: &ScalarTerm,
) -> Option<(&IntegerType, &ScalarTerm, &ScalarTerm)> {
    match term {
        ScalarTerm::WrappingIntegerAdd {
            scalar_type,
            left,
            right,
        } => Some((scalar_type, left, right)),
        _ => None,
    }
}

pub(crate) fn exact_integer_subtract(
    term: &ScalarTerm,
) -> Option<(&IntegerType, &ScalarTerm, &ScalarTerm)> {
    match term {
        ScalarTerm::ExactIntegerSubtract {
            scalar_type,
            left,
            right,
        } => Some((scalar_type, left, right)),
        _ => None,
    }
}

pub(crate) fn exact_integer_multiply(
    term: &ScalarTerm,
) -> Option<(&IntegerType, &ScalarTerm, &ScalarTerm)> {
    match term {
        ScalarTerm::ExactIntegerMultiply {
            scalar_type,
            left,
            right,
        } => Some((scalar_type, left, right)),
        _ => None,
    }
}

pub(crate) fn exact_integer_divide(
    term: &ScalarTerm,
) -> Option<(&IntegerType, &ScalarTerm, &ScalarTerm)> {
    match term {
        ScalarTerm::ExactIntegerDivide {
            scalar_type,
            left,
            right,
        } => Some((scalar_type, left, right)),
        _ => None,
    }
}

pub(crate) fn is_math_add(term: &IntegerMathTerm) -> bool {
    matches!(term, IntegerMathTerm::Add(..))
}

pub(crate) fn is_math_subtract(term: &IntegerMathTerm) -> bool {
    matches!(term, IntegerMathTerm::Subtract(..))
}

pub(crate) fn is_math_multiply(term: &IntegerMathTerm) -> bool {
    matches!(term, IntegerMathTerm::Multiply(..))
}
