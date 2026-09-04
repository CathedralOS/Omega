//! Optimizer module role: stage group. Unary integer folds by exact rule identity.
//!
//! Each exact rule owns its contract and proposal join. Traversal and rewrite
//! construction stay shared because widening and bitwise-not have the same
//! single-operand scalar-evaluation shape.

mod integer_bitwise_not_constants;
mod integer_widen_constants;
mod model;
mod proposal;

pub use integer_bitwise_not_constants::IntegerBitwiseNotConstantsRule;
pub use integer_widen_constants::IntegerWidenConstantsRule;

use omega_optimization_core::{OptimizationRuleContract, OptimizationSafetyClass};

pub(super) fn contract(rule_name: &[u8]) -> OptimizationRuleContract {
    super::super::constant_evaluation_contract(
        rule_name,
        OptimizationSafetyClass::ExactOperationSemantics,
    )
}
