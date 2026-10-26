//! A term-variant destructure for the shared checker and denotation bodies.
//! Every arithmetic `ScalarTerm` carries the same `{scalar_type, left, right}`
//! operand shape, so a matcher is the only per-variant surface a shared body
//! needs.

use semantic_vocabulary::{IntegerMathTerm, IntegerType, ScalarTerm};

pub(crate) type TermMatcher = fn(&ScalarTerm) -> Option<(&IntegerType, &ScalarTerm, &ScalarTerm)>;

/// One row per arithmetic variant: the row names the variant whose
/// `{scalar_type, left, right}` operands the matcher projects, so the
/// destructure is one row per variant rather than one body per variant.
/// Each row keeps its own function name, and that name is its identity.
macro_rules! scalar_term_matcher {
    ($($matcher:ident => $variant:ident),+ $(,)?) => {
        $(
            pub(crate) fn $matcher(
                term: &ScalarTerm,
            ) -> Option<(&IntegerType, &ScalarTerm, &ScalarTerm)> {
                match term {
                    ScalarTerm::$variant { scalar_type, left, right } => {
                        Some((scalar_type, left, right))
                    }
                    _ => None,
                }
            }
        )+
    };
}

scalar_term_matcher! {
    exact_integer_add => ExactIntegerAdd,
    wrapping_integer_add => WrappingIntegerAdd,
    exact_integer_subtract => ExactIntegerSubtract,
    exact_integer_multiply => ExactIntegerMultiply,
    exact_integer_divide => ExactIntegerDivide,
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
