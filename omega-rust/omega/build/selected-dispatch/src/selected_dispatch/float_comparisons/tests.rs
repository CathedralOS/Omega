use super::{CheckedOperatorOccurrence, CheckedTrees, replace_executions, selected_executions};
use effects::provider_plan::{ProviderBinding, ProviderPlan};
use provider_planning::ProviderPlanDerivation;
use std::sync::Arc;

const SOURCE: &str = r#"
    data Float {}
    boundary operator == Float::equal(left: f32, right: f32) -> bool;
    data FloatProvider {}
    machine FloatProvider::equal(left: f32, right: f32) -> bool
    satisfies Float::equal
    via ForeignBinding::CompilerIntrinsic;

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
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve floating Match fixture");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type floating Match fixture");
    let plans = provider_planning::derive_satisfies_plans(
        &typed,
        ProviderPlanDerivation::unevaluated(None),
    )
    .into_iter()
    .map(|derived| derived.plan)
    .collect::<Vec<_>>();
    let [plan] = plans.as_slice() else {
        panic!("floating Match fixture must derive exactly one plan")
    };
    let checked = typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .expect("check floating Match fixture");
    (checked, plan.clone())
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
    let checked = Arc::new(checked);
    let checked = super::super::settle_selected_execution_dispatch(checked, &selected)
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
    let before = checked.as_ref().clone();
    let checked = super::super::settle_selected_execution_dispatch(checked, &selected)
        .expect("settlement remains idempotent");
    assert_eq!(checked.as_ref(), &before);
}

#[test]
fn match_comparison_execution_rejects_plan_identity_drift() {
    let (checked, plan) = fixture();
    let mut wrong_intrinsic = plan.clone();
    wrong_intrinsic.rows[0].binding = ProviderBinding::CompilerIntrinsic {
        machine: "FloatProvider::wrong_format".into(),
    };
    assert!(selected_executions(&checked, &[wrong_intrinsic]).is_err());
}

#[test]
fn unselected_match_does_not_retain_previous_execution() {
    let (mut checked, plan) = fixture();
    let executions = selected_executions(&checked, &[plan]).expect("derive selected executions");
    assert!(!executions.is_empty());
    replace_executions(&mut checked, executions);
    let checked = Arc::new(checked);
    let checked = super::super::settle_selected_execution_dispatch(
        checked,
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
