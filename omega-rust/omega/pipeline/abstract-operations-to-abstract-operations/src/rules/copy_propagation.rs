//! Optimizer module role: executable entrance. Copy-propagation pass entrance.
//!
//! This entrance owns exact rule order; `redundant_block_parameter` owns the
//! rule, its contract and its proposal traversal.

mod redundant_block_parameter;

pub use redundant_block_parameter::RedundantBlockParameterRule;

use crate::rules::catalog::BuiltInRuleRegistration;

/// The exact local rule order for this pass.
pub(super) fn built_in_registrations() -> Vec<BuiltInRuleRegistration> {
    vec![BuiltInRuleRegistration::new(0, RedundantBlockParameterRule)]
}
