//! Optimizer module role: executable entrance. Exact constant-evaluation rule-family entrance.
//!
//! Boolean rewrites and integer rewrites are separate semantic families. The
//! integer entrance descends again into binary arithmetic, exact casts, unary
//! operations, and fact lookup. This module owns their shared SCCP rule
//! contract; the pass entrance remains the only local rule-order point.

mod boolean;
mod integer;

pub use boolean::*;
pub(super) use integer::integer_value_type;
pub use integer::*;

use omega_optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
};

use crate::rules::passes::SCCP_PASS_NAME;

fn integer_evaluation_contract(
    rule_name: &[u8],
    safety_class: OptimizationSafetyClass,
) -> OptimizationRuleContract {
    OptimizationRuleContract::new(
        OptimizationRuleIdentity::from_canonical_bytes(rule_name),
        OptimizationPassIdentity::from_canonical_bytes(SCCP_PASS_NAME),
        1,
        AnalysisSet::new([AnalysisKind::ScalarConstants]),
        AnalysisInvalidationSet::new([AnalysisKind::UseDefinition]),
        safety_class,
    )
    .expect("built-in rule has nonzero version")
}
