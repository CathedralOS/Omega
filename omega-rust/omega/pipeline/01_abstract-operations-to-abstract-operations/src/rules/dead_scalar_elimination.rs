//! Optimizer module role: executable entrance. Removal of unused scalar computations, grouped by their semantic safety proof.
//!
//! The ordered pass roster is visible here. Each row descends to an exact
//! named rule leaf that owns its contract and closed operation admission.

use crate::rules::catalog::BuiltInRuleRegistration;

mod dead_scalar_literal_elimination;
mod dead_unconditionally_total_scalar_elimination;

pub use dead_scalar_literal_elimination::DeadScalarLiteralEliminationRule;
pub use dead_unconditionally_total_scalar_elimination::DeadUnconditionallyTotalScalarEliminationRule;

/// The exact local rule order for this pass.
pub(super) fn built_in_registrations() -> Vec<BuiltInRuleRegistration> {
    vec![
        BuiltInRuleRegistration::new(0, DeadScalarLiteralEliminationRule),
        BuiltInRuleRegistration::new(1, DeadUnconditionallyTotalScalarEliminationRule),
    ]
}
