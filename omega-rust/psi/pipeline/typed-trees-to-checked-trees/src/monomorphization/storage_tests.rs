use super::*;

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolution");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("typing")
}

#[test]
fn recursive_instances_keep_each_tuples_own_state_and_template_commitment() {
    let mut program = typed(
        "machine repeat<T [copy]>(value: T) -> T { repeat(value) }
        machine first(value: u8) -> u8 { repeat(value) }
        machine second(value: u16) -> u16 { repeat(value) }
        machine third(value: u32) -> u32 { repeat(value) }",
    );
    let template_index = program
        .machines()
        .iter()
        .position(|machine| machine.name.as_str() == "repeat")
        .expect("template");
    let template = candidate::from_machine(&program, template_index);
    let contract = canonical_template_contract_bytes(&program, template_index);
    // Exercise graph publication independently of discovery's recursive-call
    // admission. These are the three exact external selections from the input.
    let selections = program
        .machines()
        .iter()
        .filter(|machine| matches!(machine.name.as_str(), "first" | "second" | "third"))
        .map(|machine| {
            let state = &program.machine_states(machine)[0];
            let expression = program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .find_map(|statement| {
                    let StatementNode::LocalData(local) = statement else {
                        return None;
                    };
                    Some(local.initial_value)
                })
                .expect("hoisted external value call");
            let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
                panic!("external value call");
            };
            CallSelection {
                site: CallSite::Expression(expression),
                callee_symbol: call.target_symbol,
                candidate_index: 0,
                caller_is_generic: false,
                self_forwarded_machine_parameters: false,
                self_forwarded_evidence_parameters: false,
                unresolved_const_parameters: false,
                type_bindings: vec![Some(program.state_parameters(state)[0].type_reference)],
                const_bindings: Vec::new(),
                machine_bindings: Vec::new(),
                evidence_bindings: Vec::new(),
                conflicted: false,
            }
        })
        .collect::<Vec<_>>();
    apply_multiple_specializations(&mut program, &template, &selections, 0)
        .expect("three recursive graph instances");
    assert_eq!(program.machine_specializations.len(), 3);
    assert_eq!(
        program.machine_specializations[0].instance,
        template.template_symbol
    );
    for receipt in &program.machine_specializations {
        assert_eq!(receipt.canonical_template_contract_bytes, contract);
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == receipt.instance)
            .expect("instance");
        let state = &program.machine_states(machine)[0];
        let calls = program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .filter_map(|statement| {
                let StatementNode::LocalData(local) = statement else {
                    return None;
                };
                let ExpressionNode::Call(call) =
                    program.expression_table.expression(local.initial_value)
                else {
                    return None;
                };
                Some(call.target_symbol)
            })
            .collect::<Vec<_>>();
        assert_eq!(calls, [state.symbol]);
    }
}
