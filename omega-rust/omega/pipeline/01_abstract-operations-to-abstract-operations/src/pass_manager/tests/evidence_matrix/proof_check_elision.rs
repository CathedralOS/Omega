//! `Optimization::ProofCheckElision` evidence matrix legs.

use super::super::verified_exact_self_divide_unit;
use super::{
    SelectionMatrix, assert_disabled_leg, assert_empty_selection_leg,
    assert_full_entrance_leg, assert_malformed_carrier_legs, assert_negative_leg,
};
use crate::rules::LiveProofCertifiedIntegerSelfDivideEliminationRule;
use optimization_core::{Optimization, OptimizationRuleIdentity};

fn expected_rule() -> OptimizationRuleIdentity {
    LiveProofCertifiedIntegerSelfDivideEliminationRule::contract().identity()
}

const CASE: SelectionMatrix = SelectionMatrix {
    selection: Optimization::ProofCheckElision,
    expected_rule,
    expected_commits: 1,
    positive: verified_exact_self_divide_unit,
    sibling: Optimization::CopyPropagation,
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
fn malformed_carriers_fail_admission() {
    assert_malformed_carrier_legs(&CASE);
}
