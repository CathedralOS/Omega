//! Optimizer module role: stage group. Boolean-result constant evaluation by exact rule identity.
//!
//! Each exact rule owns its contract and proposal join. Traversal and typed
//! evaluation are shared here because all five rules emit the same Boolean
//! rewrite shape from scalar-constant evidence.

mod boolean_equal_constants;
mod boolean_not_constants;
mod evaluation;
mod integer_equal_constants;
mod integer_less_or_equal_constants;
mod integer_less_than_constants;
mod model;
mod proposal;

pub use boolean_equal_constants::BooleanEqualConstantsRule;
pub use boolean_not_constants::BooleanNotConstantsRule;
pub use integer_equal_constants::IntegerEqualConstantsRule;
pub use integer_less_or_equal_constants::IntegerLessOrEqualConstantsRule;
pub use integer_less_than_constants::IntegerLessThanConstantsRule;

use optimization_core::{OptimizationRuleContract, OptimizationSafetyClass};

pub(super) fn contract(rule_name: &[u8]) -> OptimizationRuleContract {
    super::constant_evaluation_contract(rule_name, OptimizationSafetyClass::ExactOperationSemantics)
}
