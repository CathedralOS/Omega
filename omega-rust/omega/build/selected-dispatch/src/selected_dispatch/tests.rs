use crate::{
    settle_selected_execution_dispatch, settle_selected_execution_dispatch_with_source_edits,
};
use checked_trees::{CheckedProviderPlanCommitment, CheckedTrees, CheckedUnitEffectOperationPlan};
use effects::SelectedProviderPlanFacts;
use provider_planning::ProviderPlanDerivation;
use std::sync::Arc;

fn mixed_fixture() -> (CheckedTrees, SelectedProviderPlanFacts) {
    let source = r#"
        data Math {}
        boundary operator Math::identity(value: i32) -> i32;
        data MathProvider {}
        machine MathProvider::identity(value: i32) -> i32 satisfies Math::identity { value }

        data F32 {}
        boundary operator F32::fused_multiply_add(left: f32, right: f32, addend: f32) -> f32;
        data FloatProvider {}
        machine FloatProvider::fused_multiply_add(left: f32, right: f32, addend: f32) -> f32
        satisfies F32::fused_multiply_add via ForeignBinding::CompilerIntrinsic;

        data Root {}
        machine Root::enter(&mut self) {
            let before: i32 = Math::identity(7);
            let after: i32 = before + 0;
        }
        machine Root::float_entry(&mut self) {
            let fused: f32 = F32::fused_multiply_add(2.0f32, 3.0f32, 4.0f32);
        }
        machine probe(value: f32) -> f32 { F32::fused_multiply_add(value, 3.0f32, 4.0f32) }
    "#;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let plans = provider_planning::derive_satisfies_plans(
        &typed,
        ProviderPlanDerivation::unevaluated(None),
    )
    .into_iter()
    .map(|derived| derived.plan)
    .collect::<Vec<_>>();
    assert_eq!(plans.len(), 2);
    let mut checked = typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .unwrap();
    let uses = checked
        .facts
        .operators
        .named_uses
        .iter()
        .map(|(handle, operator_use)| (handle, *operator_use))
        .collect::<Vec<_>>();
    for (handle, mut operator_use) in uses {
        let operator = checked
            .operators()
            .iter()
            .find(|operator| operator.symbol == operator_use.selected_operator_symbol)
            .unwrap();
        let name = checked
            .operator_path_members(operator.name)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>()
            .join("::");
        let plan = plans
            .iter()
            .find(|plan| plan.schema.trait_name.contains(&name))
            .unwrap();
        operator_use.provider_plan_report_fingerprint = plan.report_fingerprint();
        operator_use.provider_plan_commitment =
            CheckedProviderPlanCommitment::from_digest(*plan.identity_digest().as_bytes());
        *checked.facts.operators.named_uses.get_mut(handle) = operator_use;
    }
    let selected = SelectedProviderPlanFacts::from_selection(
        &plans,
        &plans
            .iter()
            .map(|plan| plan.name.clone())
            .collect::<Vec<_>>(),
    )
    .unwrap();
    (checked, selected)
}

#[test]
fn shared_mixed_settlement_keeps_both_plans_and_source_custody() {
    let (checked, selected) = mixed_fixture();
    let original = checked.clone();
    let (settled, journal) =
        settle_selected_execution_dispatch_with_source_edits(Arc::new(checked), &selected)
            .expect("one transaction settles both operator and FMA applications");
    let caller = settled
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Root::enter")
        .unwrap();
    let operations = &settled
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(caller.symbol)
        .unwrap()
        .operations;
    assert_eq!(
        operations
            .iter()
            .filter(|operation| matches!(
                operation,
                CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall { .. }
            ))
            .count(),
        1
    );
    assert_eq!(
        settled
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter()
            .flat_map(|machine| &machine.operations)
            .filter(|operation| matches!(
                operation,
                CheckedUnitEffectOperationPlan::SelectedIeeeFloatFusedMultiplyAdd { .. }
            ))
            .count(),
        1
    );
    crate::validate_selected_operator_terminal_custody(&settled, &selected).unwrap();
    let source = journal
        .source_trees(&settled.typed)
        .expect("both rewrites retain source custody");
    for (_, operator_use) in original.facts.operators.named_uses.iter() {
        assert_eq!(
            source.expression_table.expression(operator_use.expression),
            original
                .expression_table
                .expression(operator_use.expression)
        );
    }
}

#[test]
fn shared_mixed_source_guard_failure_publishes_neither_rewrites_nor_plans() {
    let (mut checked, selected) = mixed_fixture();
    let caller = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "probe")
        .unwrap();
    let parameter = checked.state_parameters(&checked.machine_states(caller)[0])[0].clone();
    // An orphan duplicate is outside the declaration span used by plan rebuilding,
    // but prevents the journal from certifying one exact operand binding.
    checked.typed.state_parameters.append(parameter);
    let original = Arc::new(checked);
    settle_selected_execution_dispatch(Arc::new(original.as_ref().clone()), &selected)
        .expect("the selected execution plans and rewrites themselves remain valid");
    let diagnostics = settle_selected_execution_dispatch_with_source_edits(original, &selected)
        .expect_err("late source guard failure must abort the complete transaction");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("duplicate operand parameter binding")),
        "{diagnostics:?}"
    );
}
