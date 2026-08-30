//! Copy-propagation pass entrance.
//!
//! This entrance owns exact rule order; `rule` owns proposal mechanics.

mod rule;

pub use rule::RedundantBlockParameterRule;

use crate::rules::catalog::BuiltInRuleRegistration;

/// The exact local rule order for this pass.
pub(in crate::rules) fn built_in_registrations() -> Vec<BuiltInRuleRegistration> {
    vec![BuiltInRuleRegistration::new(0, RedundantBlockParameterRule)]
}
