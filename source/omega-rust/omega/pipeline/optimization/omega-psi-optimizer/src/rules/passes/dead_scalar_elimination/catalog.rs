//! Exact dead-pure-scalar-elimination rule order.

use crate::rules::catalog::BuiltInRuleRegistration;

use super::{DeadScalarLiteralEliminationRule, DeadUnconditionallyTotalScalarEliminationRule};

pub(in crate::rules) fn built_in_registrations() -> Vec<BuiltInRuleRegistration> {
    vec![
        BuiltInRuleRegistration::new(0, DeadScalarLiteralEliminationRule),
        BuiltInRuleRegistration::new(1, DeadUnconditionallyTotalScalarEliminationRule),
    ]
}
