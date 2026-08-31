//! Optimizer module role: executable entrance. Removal of unused scalar computations, grouped by their semantic safety proof.
//!
//! The ordered pass roster is visible here. Each row descends to an exact
//! named rule leaf; `family` owns the common safety contract, while `proposal`,
//! `shapes`, and `accounting` are shared mechanics.

use crate::rules::catalog::BuiltInRuleRegistration;

mod accounting;
mod family;
mod literal;
mod proof_certified;
mod proposal;
mod shapes;
mod unconditionally_total;

pub use literal::DeadScalarLiteralEliminationRule;
pub use proof_certified::ProofCertifiedDeadScalarEliminationRule;
pub use unconditionally_total::DeadUnconditionallyTotalScalarEliminationRule;

/// The exact local rule order for this pass.
pub(in crate::rules) fn built_in_registrations() -> Vec<BuiltInRuleRegistration> {
    vec![
        BuiltInRuleRegistration::new(0, DeadScalarLiteralEliminationRule),
        BuiltInRuleRegistration::new(1, DeadUnconditionallyTotalScalarEliminationRule),
    ]
}
