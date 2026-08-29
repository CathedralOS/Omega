//! Exact copy-propagation rule order.

use crate::rules::catalog::BuiltInRuleRegistration;

use super::RedundantBlockParameterRule;

pub(in crate::rules) fn built_in_registrations() -> Vec<BuiltInRuleRegistration> {
    vec![BuiltInRuleRegistration::new(
        0,
        RedundantBlockParameterRule,
    )]
}
