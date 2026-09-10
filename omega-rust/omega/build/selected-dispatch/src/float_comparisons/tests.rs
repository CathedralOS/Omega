use super::*;
use checked_trees::CheckedProviderPlanCommitment;
use effects::provider_plan::{ProviderBinding, ProviderPlan};
use std::sync::Arc;

const SOURCE: &str = r#"
    data Float {}
    boundary operator == Float::equal(left: f32, right: f32) -> bool;
    data FloatProvider {}
    machine FloatProvider::equal(left: f32, right: f32) -> bool
    satisfies Float::equal
    via Binding::CompilerIntrinsic;

    machine choose(value: f32, first: f32, second: f32) -> u64 {
        match value { first -> 7, second -> 9, _ -> 11 }
    }
"#;

fn fixture() -> (CheckedTrees, ProviderPlan) {
    let tokens = source_files_to_tokens::Lexer::new(SOURCE)
        .tokenize()
        .expect("tokenize floating Match fixture");
    let syntax =
        tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse floating Match fixture");
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
        .expect("resolve floating Match fixture");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type floating Match fixture");
    let plans = provider_planning::plans::derive_satisfies_plans(&typed, None);
    let [plan] = plans.as_slice() else {
        panic!("floating Match fixture must derive exactly one plan")
    };
    let mut checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("check floating Match fixture");
    bind_plan(&mut checked, plan);
    (checked, plan.clone())
}

fn bind_plan(checked: &mut CheckedTrees, plan: &ProviderPlan) {
    let handles = checked
        .facts
        .operators
        .uses
        .iter()
        .filter(|(_, operator_use)| {
            matches!(
                operator_use.occurrence,
                CheckedOperatorOccurrence::MatchEquality { .. }
            )
        })
        .map(|(handle, _)| handle)
        .collect::<Vec<_>>();
    assert!(!handles.is_empty(), "fixture retains arm occurrences");
    for handle in handles {
        let operator_use = checked.facts.operators.uses.get_mut(handle);
        operator_use.provider_plan_report_fingerprint = plan.report_fingerprint();
        operator_use.provider_plan_commitment =
            CheckedProviderPlanCommitment::from_digest(*plan.identity_digest().as_bytes());
    }
}

#[test]
fn selected_match_comparisons_settle_without_replacing_source() {
    let (checked, plan) = fixture();
    let selected = effects::SelectedProviderPlanFacts::from_selection(
        std::slice::from_ref(&plan),
        std::slice::from_ref(&plan.name),
    )
    .expect("select floating equality plan");
    let source_before = checked.typed.clone();
    let mut checked = Arc::new(checked);
    super::super::settle_selected_execution_dispatch(&mut checked, &selected)
        .expect("settle floating Match comparisons");
    assert_eq!(checked.typed, source_before);
    let expected_count = checked
        .facts
        .operators
        .uses
        .iter()
        .filter(|(_, operator_use)| {
            matches!(
                operator_use.occurrence,
                CheckedOperatorOccurrence::MatchEquality { .. }
            )
        })
        .count();
    assert_eq!(
        checked
            .facts
            .operators
            .selected_float_comparisons
            .iter()
            .count(),
        expected_count,
    );
    let settled = Arc::clone(&checked);
    super::super::settle_selected_execution_dispatch(&mut checked, &selected)
        .expect("settlement remains idempotent");
    assert!(Arc::ptr_eq(&settled, &checked));
}

#[test]
fn match_comparison_execution_rejects_plan_identity_drift() {
    let (checked, plan) = fixture();
    assert!(selected_executions(&checked, &[]).is_err());
    assert!(selected_executions(&checked, &[plan.clone(), plan.clone()]).is_err());
    let mut wrong_commitment = checked.clone();
    let handle = wrong_commitment
        .facts
        .operators
        .uses
        .iter()
        .find(|(_, operator_use)| {
            matches!(
                operator_use.occurrence,
                CheckedOperatorOccurrence::MatchEquality { .. }
            )
        })
        .unwrap()
        .0;
    wrong_commitment
        .facts
        .operators
        .uses
        .get_mut(handle)
        .provider_plan_commitment = CheckedProviderPlanCommitment::from_digest([7; 32]);
    assert!(selected_executions(&wrong_commitment, std::slice::from_ref(&plan)).is_err());

    let mut wrong_intrinsic = plan.clone();
    wrong_intrinsic.rows[0].binding = ProviderBinding::CompilerIntrinsic {
        machine: "FloatProvider::wrong_format".into(),
    };
    let mut checked = checked;
    bind_plan(&mut checked, &wrong_intrinsic);
    assert!(selected_executions(&checked, &[wrong_intrinsic]).is_err());
}

#[test]
fn rejected_match_settlement_preserves_shared_program() {
    let (checked, plan) = fixture();
    let selected = effects::SelectedProviderPlanFacts::from_selection(
        std::slice::from_ref(&plan),
        std::slice::from_ref(&plan.name),
    )
    .expect("select floating equality plan");
    let mut checked = checked;
    let handle = checked
        .facts
        .operators
        .uses
        .iter()
        .filter(|(_, operator_use)| {
            matches!(
                operator_use.occurrence,
                CheckedOperatorOccurrence::MatchEquality { .. }
            )
        })
        .last()
        .unwrap()
        .0;
    checked
        .facts
        .operators
        .uses
        .get_mut(handle)
        .provider_plan_commitment = CheckedProviderPlanCommitment::from_digest([9; 32]);
    let original = Arc::new(checked);
    let mut rejected = Arc::clone(&original);
    super::super::settle_selected_execution_dispatch(&mut rejected, &selected)
        .expect_err("invalid later occurrence rejects the complete roster");
    assert!(Arc::ptr_eq(&original, &rejected));
    assert_eq!(original.as_ref(), rejected.as_ref());
}

#[test]
fn unselected_match_does_not_retain_previous_execution() {
    let (mut checked, plan) = fixture();
    let executions = selected_executions(&checked, &[plan]).expect("derive selected executions");
    assert!(!executions.is_empty());
    replace_executions(&mut checked, executions);
    let handles = checked
        .facts
        .operators
        .uses
        .iter()
        .map(|(handle, _)| handle)
        .collect::<Vec<_>>();
    for handle in handles {
        let operator_use = checked.facts.operators.uses.get_mut(handle);
        operator_use.provider_plan_report_fingerprint = 0;
        operator_use.provider_plan_commitment = CheckedProviderPlanCommitment::default();
    }
    let mut checked = Arc::new(checked);
    super::super::settle_selected_execution_dispatch(
        &mut checked,
        &effects::SelectedProviderPlanFacts::default(),
    )
    .expect("remove no-longer-selected execution custody");
    assert_eq!(
        checked
            .facts
            .operators
            .selected_float_comparisons
            .iter()
            .count(),
        0
    );
}
