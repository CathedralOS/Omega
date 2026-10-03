use super::{CheckedOperatorOccurrence, CheckedTrees, replace_executions, selected_executions};
use build_time_evaluation::{
    BuildMachineExecutionMode, BuildMachineInvocation, BuildTimeValue, PreparedBuildMachineProgram,
    evaluate_build_machine_measured,
};
use effects::provider_plan::ProviderBinding;
use provider_planning::{
    ProviderPlanDerivation, ProviderPlanningContext, ProviderSelectionProvenance,
    SelectedProviderReviewProvenance,
};
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
    data Choice { selected: u64; }
    machine capture(choice: &mut Choice, value: f32, first: f32, second: f32) {
        choice.selected = choose(value, first, second);
    }
"#;

fn fixture() -> (CheckedTrees, SelectedProviderReviewProvenance) {
    let tokens = source_files_to_tokens::Lexer::new(SOURCE)
        .tokenize()
        .expect("tokenize floating Match fixture");
    let syntax =
        tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse floating Match fixture");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax),
    )
    .expect("resolve floating Match fixture");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type floating Match fixture");
    let plans = provider_planning::derive_satisfies_plans(ProviderPlanDerivation::unevaluated(
        ProviderPlanningContext::new(&typed, source::DependencyScope::Product),
        None,
    ))
    .into_iter()
    .map(|derived| SelectedProviderReviewProvenance {
        plan: derived.plan,
        provider: derived.provenance,
        selected_by: ProviderSelectionProvenance::UniqueCoveringCandidate,
        row_compiler_intrinsic_executions: vec![],
    })
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
    let (checked, provider) = fixture();
    let selected = effects::SelectedProviderPlanFacts::from_selection(
        std::slice::from_ref(&provider.plan),
        std::slice::from_ref(&provider.plan.name),
    )
    .expect("select floating equality plan");
    let source_before = checked.typed.clone();
    let checked = Arc::new(checked);
    let checked = super::super::settle_selected_execution_dispatch(
        checked,
        &selected,
        std::slice::from_ref(&provider),
    )
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
    let checked = super::super::settle_selected_execution_dispatch(
        checked,
        &selected,
        std::slice::from_ref(&provider),
    )
    .expect("settlement remains idempotent");
    assert_eq!(checked.as_ref(), &before);
}

#[test]
fn match_comparison_execution_rejects_plan_identity_drift() {
    let (checked, provider) = fixture();
    let mut wrong_intrinsic = provider;
    wrong_intrinsic.plan.rows[0].binding = ProviderBinding::CompilerIntrinsic {
        machine: "FloatProvider::wrong_format".into(),
    };
    assert!(
        selected_executions(
            &checked,
            source::DependencyScope::Product,
            &[wrong_intrinsic]
        )
        .is_err()
    );
}

#[test]
fn build_match_requires_exact_selected_declaration_before_execution() {
    let (mut checked, provider) = fixture();
    let source_before = checked.clone();
    let missing = super::super::settle_selected_comparison_execution(
        &mut checked,
        source::DependencyScope::Build,
        &[],
    )
    .expect_err("Build comparisons must close before any effects");
    assert!(
        missing
            .iter()
            .any(|diagnostic| diagnostic.message.contains("no provider was selected"))
    );
    assert_eq!(checked, source_before);

    let mut substituted = provider.clone();
    substituted.provider.schema = provider_planning::ProviderSchemaDeclaration::BoundaryOperator(
        symbols::SymbolHandle::invalid(),
    );
    assert!(
        super::super::settle_selected_comparison_execution(
            &mut checked,
            source::DependencyScope::Build,
            &[substituted],
        )
        .is_err(),
        "an equal portable plan cannot replace its declaration custody"
    );
    assert_eq!(checked, source_before);

    super::super::settle_selected_comparison_execution(
        &mut checked,
        source::DependencyScope::Build,
        &[provider],
    )
    .expect("the exact selected provider closes the source-free fixture");
    assert!(
        !checked
            .facts
            .operators
            .selected_float_comparisons
            .is_empty()
    );
    assert_eq!(checked.typed, source_before.typed);
    let prepared = PreparedBuildMachineProgram::from_checked(checked);
    let symbol = prepared
        .typed()
        .realized_machine_named("capture")
        .expect("exact capture machine")
        .symbol;
    let entry = prepared.entry(symbol).expect("admit capture entry");
    let choice = |selected| BuildTimeValue::Struct {
        type_name: "Choice".to_owned(),
        fields: vec![("selected".to_owned(), BuildTimeValue::Int(selected))],
    };
    let evaluation = evaluate_build_machine_measured(
        &prepared,
        BuildMachineInvocation {
            entry: &entry,
            arguments: vec![
                choice(0),
                BuildTimeValue::Float(-0.0),
                BuildTimeValue::Float(0.0),
                BuildTimeValue::Float(1.0),
            ],
            mode: BuildMachineExecutionMode::Pure,
            sponsor: None,
            product_entry_compatibility: None,
        },
    )
    .expect("prepared Build execution retains selected comparison custody into its helper");
    assert_eq!(
        evaluation.value()[0],
        choice(7),
        "IEEE equality selects positive zero for negative zero"
    );
}

#[test]
fn unselected_match_does_not_retain_previous_execution() {
    let (mut checked, provider) = fixture();
    let executions = selected_executions(&checked, source::DependencyScope::Product, &[provider])
        .expect("derive selected executions");
    assert!(!executions.is_empty());
    replace_executions(&mut checked, executions);
    let checked = Arc::new(checked);
    let checked = super::super::settle_selected_execution_dispatch(
        checked,
        &effects::SelectedProviderPlanFacts::default(),
        &[],
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
