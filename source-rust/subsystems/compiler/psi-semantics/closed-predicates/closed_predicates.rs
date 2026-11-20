//! Evaluate published predicates over concrete operands under the same resource
//! limits during compilation and interpretation.
#![forbid(unsafe_code)]

mod budget;
mod guard;
mod ieee;
mod integer;

pub use budget::{PredicateBudget, PredicateSizeLimit, check_predicate_evaluation_size};
pub use guard::{closed_guard, closed_guard_with};
pub use ieee::closed_ieee_relation;
pub use integer::{
    ClosedIntegerEvaluationError, ClosedIntegerEvaluator, big_integer_literal,
    check_integer_math_term_size,
};
