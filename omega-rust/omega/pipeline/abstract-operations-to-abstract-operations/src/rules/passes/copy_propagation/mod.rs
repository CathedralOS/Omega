//! Optimizer module role: executable entrance. Copy-propagation pass entrance.
//!
//! This entrance owns exact rule order; `redundant_block_parameter` owns the
//! named proposal leaf.

mod redundant_block_parameter;

pub use redundant_block_parameter::RedundantBlockParameterRule;

use crate::rules::catalog::BuiltInRuleRegistration;

/// The exact local rule order for this pass.
pub(in crate::rules) fn built_in_registrations() -> Vec<BuiltInRuleRegistration> {
    vec![BuiltInRuleRegistration::new(0, RedundantBlockParameterRule)]
}
