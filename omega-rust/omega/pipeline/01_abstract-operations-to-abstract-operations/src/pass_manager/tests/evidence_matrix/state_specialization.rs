//! `Optimization::StateSpecialization` evidence matrix legs.

use super::super::verified_dispatch_specialization_unit;
use super::{
    SelectionMatrix, assert_disabled_leg, assert_empty_selection_leg,
    assert_full_entrance_leg, assert_measured_budget_leg, assert_negative_leg,
};
use crate::rules::StateArgumentSpecializationRule;
use optimization_core::{Optimization, OptimizationRuleIdentity};

fn expected_rule() -> OptimizationRuleIdentity {
    StateArgumentSpecializationRule::contract().identity()
}

const CASE: SelectionMatrix = SelectionMatrix {
    selection: Optimization::StateSpecialization,
    expected_rule,
    // Three dispatch sites commit independently: the Boolean state argument
    // fuses its constant-supplied jump and conditional-arm edges, the
    // integer state argument's `istate < 4` dispatch fuses its one
    // constant-supplied jump edge, and the result specialization fuses
    // `ccall_pred`'s edge whose bound argument is `Call{leaf}`'s scalar
    // result — proven constant by `leaf`'s own single-Return lattice — while
    // each parameter-supplied variable edge keeps its dispatch reachable.
    expected_commits: 3,
    positive: verified_dispatch_specialization_unit,
    sibling: Optimization::GlobalValueNumbering,
};

#[test]
fn full_entrance_optimizes_and_publishes() {
    assert_full_entrance_leg(&CASE);
}

#[test]
fn disabled_sibling_selection_leaves_the_unit_untouched() {
    assert_disabled_leg(&CASE);
}

#[test]
fn empty_selection_leaves_the_unit_untouched() {
    assert_empty_selection_leg(&CASE);
}

#[test]
fn negative_leaves_an_empty_unit_untouched() {
    assert_negative_leg(&CASE);
}

#[test]
fn measured_budget_admits_exact_usage_and_refuses_one_less() {
    assert_measured_budget_leg(&CASE);
}
