use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;

#[test]
fn saved_provider_call_keeps_its_unresolved_tuple_after_live_specialization() {
    let source = r#"
        machine helper<Element>(value: Element) -> u64 { 0 }

        machine warmup(value: u64) -> u64 { helper(value) }

        data GenericProvider {}
        machine GenericProvider::measure<Value>(value: Value) -> u64 {
            helper(value)
        }

        machine direct(value: i32) -> u64 {
            GenericProvider::measure(value)
        }
        "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let mut source =
        lower_symbol_resolved_trees(&resolved).expect("type the unresolved provider call");
    let provider = source
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "GenericProvider::measure")
        .expect("saved generic provider")
        .clone();
    let helper = source
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "helper")
        .expect("saved generic helper");
    let helper_symbol = helper.symbol;
    let helper_state = source.machine_states(helper)[0].symbol;
    let occurrences = source
        .machine_states(&provider)
        .iter()
        .flat_map(|state| source.statement_table.statements(state.statement_nodes))
        .filter_map(|statement| {
            let StatementNode::LocalData(local) = statement else {
                return None;
            };
            let ExpressionNode::Call(call) =
                source.expression_table.expression(local.initial_value)
            else {
                return None;
            };
            (call.target_symbol == helper_state).then(|| (local.initial_value, call.clone()))
        })
        .collect::<Vec<_>>();
    assert_eq!(occurrences.len(), 1, "one inferred tail-call occurrence");
    assert_eq!(source.machine_type_parameters(&provider).len(), 1);

    let mut program = source.clone();
    crate::specialize_static_machine_calls(&mut program)
        .expect("ordinary specialization closes the live provider and helper");
    let live_provider = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == provider.symbol)
        .expect("the ordinary provider retains its declaration");
    assert_eq!(program.machine_type_parameters(live_provider).len(), 1);
    assert!(
        program
            .machine_specializations
            .iter()
            .any(|specialization| {
                specialization.template == provider.symbol
                    && specialization.instance != provider.symbol
                    && specialization.type_arguments == ["i32"]
            })
    );
    let warmup = program
        .machine_specializations
        .iter()
        .find(|specialization| {
            specialization.template == helper_symbol && specialization.type_arguments == ["u64"]
        })
        .expect("the warmup selects the first helper tuple");
    assert_ne!(warmup.instance, helper_symbol);
    let selected = program
        .machine_specializations
        .iter()
        .find(|specialization| {
            specialization.template == helper_symbol && specialization.type_arguments == ["i32"]
        })
        .expect("the live provider selects a distinct helper tuple");
    assert_ne!(selected.instance, helper_symbol);
    let selected_helper = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == selected.instance)
        .expect("the cloned i32 helper");
    let selected_state = program.machine_states(selected_helper)[0].symbol;
    assert_ne!(selected_state, helper_state);
    for (handle, original) in &occurrences {
        let ExpressionNode::Call(live) = program.expression_table.expression(*handle) else {
            panic!("the live provider retains its call occurrence");
        };
        assert_eq!(live, original, "the authored generic body stays unchanged");
    }
    let provider_instance = program
        .machine_specializations
        .iter()
        .find(|specialization| specialization.template == provider.symbol)
        .expect("closed provider receipt");
    let provider_instance = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == provider_instance.instance)
        .expect("closed provider");
    assert!(
        program
            .machine_type_parameters(provider_instance)
            .is_empty()
    );
    assert!(
        program
            .machine_states(provider_instance)
            .iter()
            .flat_map(|state| program.statement_table.statements(state.statement_nodes))
            .any(|statement| {
                let StatementNode::LocalData(local) = statement else {
                    return false;
                };
                matches!(program.expression_table.expression(local.initial_value),
                ExpressionNode::Call(call) if call.target_symbol == selected_state)
            })
    );

    // The live call is concrete only because its provider bound Value to i32.
    // That binding supplies no evidence for the immutable saved occurrence.
    super::replay(&mut source, &program, &provider);

    assert_eq!(source.machine_type_parameters(&provider).len(), 1);
    for (handle, original) in occurrences {
        let ExpressionNode::Call(retained) = source.expression_table.expression(handle) else {
            panic!("saved replay preserves the expression kind");
        };
        assert_eq!(
            retained, &original,
            "the unresolved saved call stays unchanged"
        );
    }
}
