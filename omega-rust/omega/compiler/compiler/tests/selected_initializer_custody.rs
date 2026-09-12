//! Selected initializer ownership survives settlement without an ordinary call row.

use checked_trees::{
    CheckedProviderPlanCommitment, CheckedUnitEffectOperationPlan, CheckedValueOrigin,
    CheckedValueStatementRole,
};
use std::sync::Arc;

const SOURCE: &str = r#"
    data Math {}
    boundary operator Math::identity(value: i32) -> i32;

    data Provider {}
    machine Provider::identity(input: i32) -> i32
    satisfies Math::identity
    { input }

    data Root {}
    machine Root::enter() {
        let unused: i32 = Math::identity(70);
    }
"#;

#[test]
fn settled_selected_initializer_lowers_and_cannot_be_deleted() {
    let tokens = source_files_to_tokens::Lexer::new(SOURCE)
        .tokenize()
        .expect("tokenize selected initializer");
    let syntax =
        tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse selected initializer");
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
        .expect("resolve selected initializer");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type selected initializer");
    let plans = provider_planning::plans::derive_satisfies_plans(&typed, None);
    let [provider] = plans.as_slice() else {
        panic!("one authored operator realization");
    };
    let mut checked =
        typed_trees_to_checked_trees::lower_typed_trees(typed).expect("check selected initializer");
    let caller = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Root::enter")
        .expect("initializer caller");
    let caller_symbol = caller.symbol;
    let caller_state = checked.machine_states(caller)[0].symbol;

    // Follow selected-dispatch's source fixture protocol: the authored use
    // retains the exact ProviderPlan identity before dispatch is settled.
    let uses = checked
        .facts
        .operators
        .named_uses
        .iter()
        .map(|(handle, operator_use)| (handle, *operator_use))
        .collect::<Vec<_>>();
    let [(use_handle, operator_use)] = uses.as_slice() else {
        panic!("one authored named operator use");
    };
    let mut operator_use = *operator_use;
    assert_eq!(
        operator_use.origin,
        CheckedValueOrigin::StateStatement {
            machine_symbol: caller_symbol,
            state_symbol: caller_state,
            statement_index: 0,
            role: CheckedValueStatementRole::LocalInitializer,
        }
    );
    operator_use.provider_plan_report_fingerprint = provider.report_fingerprint();
    operator_use.provider_plan_commitment =
        CheckedProviderPlanCommitment::from_digest(*provider.identity_digest().as_bytes());
    *checked.facts.operators.named_uses.get_mut(*use_handle) = operator_use;
    let selected = effects::SelectedProviderPlanFacts::from_selection(
        std::slice::from_ref(provider),
        std::slice::from_ref(&provider.name),
    )
    .expect("select exact authored provider");
    let mut settled = Arc::new(checked);
    selected_dispatch::settle_selected_execution_dispatch(&mut settled, &selected)
        .expect("settle selected initializer");
    selected_dispatch::validate_selected_operator_terminal_custody(&settled, &selected)
        .expect("selected initializer retains its exact application");

    let caller = settled
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(caller_symbol)
        .expect("settled Unit caller");
    let [
        CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall {
            coordinate,
            result,
            realization_state,
            ..
        },
        CheckedUnitEffectOperationPlan::Complete { .. },
    ] = caller.operations.as_slice()
    else {
        panic!("unused initializer retains one selected call and one return");
    };
    assert_eq!(
        (coordinate.statement_index, coordinate.call_ordinal),
        (0, 0)
    );
    assert_eq!((result.statement_index, result.binding_ordinal), (0, 0));
    let typed_trees::expression::ExpressionNode::Call(rewritten) =
        settled.expression_table.expression(operator_use.expression)
    else {
        panic!("settlement rewrites the initializer to its realization call");
    };
    assert_eq!(rewritten.target_symbol, *realization_state);

    let artifact = terminal_production::TerminalProductionRequest::new(&settled, "Root::enter")
        .produce_artifact()
        .expect("selected initializer lowers without ordinary occurrence custody");
    let module = terminal_codec::decode_module(artifact.semantic_bytes())
        .expect("decode selected initializer artifact");
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("Terminal entry");
    assert_eq!(
        entry
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| matches!(operation.kind, terminal_psi::OperationKind::Call { .. }))
            .count(),
        1,
        "the unused result must not erase its selected invocation"
    );

    let mut deleted = settled.as_ref().clone();
    let caller = deleted
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|machine| machine.machine == caller_symbol)
        .expect("retained caller");
    caller.operations.remove(0);
    assert!(
        terminal_production::TerminalProductionRequest::new(&deleted, "Root::enter")
            .produce_artifact()
            .is_err(),
        "source-derived initializer coverage must reject a deleted selected operation"
    );
}
