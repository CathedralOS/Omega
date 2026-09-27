//! `Optimization::CopyPropagation` evidence matrix legs.

use super::super::{VerifiedPsiOptimizationUnit, verified_merge_parameter_unit};
use super::{SelectionMatrix, assert_measured_budget_leg};
use crate::rules::RedundantBlockParameterRule;
use optimization_core::{Optimization, OptimizationRuleIdentity};

fn expected_rule() -> OptimizationRuleIdentity {
    RedundantBlockParameterRule::contract().identity()
}

fn positive() -> VerifiedPsiOptimizationUnit {
    verified_merge_parameter_unit(true)
}

const CASE: SelectionMatrix = SelectionMatrix {
    selection: Optimization::CopyPropagation,
    expected_rule,
    expected_commits: 1,
    positive,
    sibling: Optimization::DeadPureScalarElimination,
};

#[test]
fn measured_budget_admits_exact_usage_and_refuses_one_less() {
    assert_measured_budget_leg(&CASE);
}
