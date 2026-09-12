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
    let original = program.clone();
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
                unresolved_machine_parameters: false,
                unresolved_evidence_parameters: false,
                unresolved_const_parameters: false,
                type_bindings: vec![Some(program.state_parameters(state)[0].type_reference)],
                const_bindings: Vec::new(),
                machine_bindings: Vec::new(),
                evidence_bindings: Vec::new(),
                conflicted: false,
            }
        })
        .collect::<Vec<_>>();
    apply_call_specializations(&mut program, &template, &selections, 0)
        .expect("three recursive graph instances");
    assert_eq!(program.machine_specializations.len(), 3);
    assert!(
        !program
            .machine_type_parameters(&program.machines()[template_index])
            .is_empty()
    );
    for receipt in &program.machine_specializations {
        assert_ne!(receipt.instance, template.template_symbol);
        assert_eq!(receipt.canonical_template_contract_bytes, contract);
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == receipt.instance)
            .expect("instance");
        assert!(!machine.is_public);
        assert!(program.machine_type_parameters(machine).is_empty());
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
    assert_template_unchanged(&original, &program, "repeat");
}

fn assert_template_unchanged(before: &TypedTrees, after: &TypedTrees, name: &str) {
    let original = before
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .expect("original template");
    let retained = after
        .machines()
        .iter()
        .find(|machine| machine.symbol == original.symbol)
        .expect("retained template");
    assert_eq!(retained, original);
    assert_eq!(
        after.machine_type_parameters(retained),
        before.machine_type_parameters(original)
    );
    assert!(!after.machine_type_parameters(retained).is_empty());
    assert_eq!(
        after.machine_states(retained),
        before.machine_states(original)
    );
    for state in before.machine_states(original) {
        let statements = before.statement_table.statements(state.statement_nodes);
        assert_eq!(
            after.statement_table.statements(state.statement_nodes),
            statements
        );
        let mut expressions = Vec::new();
        for statement in statements {
            collect_statement_expression_trees(before, statement, &mut expressions);
        }
        for expression in expressions {
            assert_eq!(
                after.expression_table.expression(expression),
                before.expression_table.expression(expression)
            );
        }
    }
}

fn call_targets(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> Vec<SymbolHandle> {
    let mut targets = Vec::new();
    for state in program.machine_states(machine) {
        for statement in program.statement_table.statements(state.statement_nodes) {
            if let StatementNode::Call(call) = statement {
                targets.push(call.target_symbol);
            }
            let mut expressions = Vec::new();
            collect_statement_expression_trees(program, statement, &mut expressions);
            for expression in expressions {
                if let ExpressionNode::Call(call) = program.expression_table.expression(expression)
                {
                    targets.push(call.target_symbol);
                }
            }
        }
    }
    targets
}

#[test]
fn mutually_recursive_generic_instances_reuse_exact_tuples_without_changing_templates() {
    let mut program = typed(
        r#"
        pub machine ping<T>(value: &T) { pong(value); }
        machine pong<T>(value: &T) { ping(value); }
        machine first(value: &u8) { ping(value); }
        machine second(value: &u16) { ping(value); }
    "#,
    );
    let original = program.clone();
    let mut nominal_uses = Vec::new();
    monomorphize_generic_machine_value_calls_with_nominal_uses(&mut program, &mut nominal_uses)
        .expect("mutual recursion closes both concrete tuples");
    assert_eq!(program.machine_specializations.len(), 4);
    for name in ["ping", "pong"] {
        assert_template_unchanged(&original, &program, name);
    }
    for receipt in &program.machine_specializations {
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == receipt.instance)
            .expect("private instance");
        assert!(!machine.is_public);
        assert!(program.machine_type_parameters(machine).is_empty());
        assert_ne!(receipt.template, receipt.instance);
        let targets = call_targets(&program, machine);
        let [target] = targets.as_slice() else {
            panic!("one exact recursive target: {targets:?}");
        };
        let destination = program
            .machines()
            .iter()
            .find(|machine| {
                program
                    .machine_states(machine)
                    .iter()
                    .any(|state| state.symbol == *target)
            })
            .expect("selected recursive callee");
        let peer = program
            .machine_specializations
            .iter()
            .find(|peer| peer.instance == destination.symbol)
            .expect("peer receipt");
        assert_ne!(peer.template, receipt.template);
        assert_eq!(
            peer.type_argument_identities,
            receipt.type_argument_identities
        );
    }
    let instances = program
        .machine_specializations
        .iter()
        .map(|receipt| receipt.instance)
        .collect::<Vec<_>>();
    monomorphize_generic_machine_value_calls_with_nominal_uses(&mut program, &mut nominal_uses)
        .expect("repeated specialization is a fixed point");
    assert_eq!(
        program
            .machine_specializations
            .iter()
            .map(|receipt| receipt.instance)
            .collect::<Vec<_>>(),
        instances
    );
}

#[test]
fn nested_generic_reference_forwarding_only_specializes_the_closed_caller() {
    let mut program = typed(
        r#"
        data Buffer<T> { value: T; }
        machine inspect<T>(value: &T) {}
        pub machine relay<U>(value: &Buffer<U>) { inspect(value); }
        machine first(value: &Buffer<u8>) { relay(value); }
        machine second(value: &Buffer<u8>) { relay(value); }
    "#,
    );
    let original = program.clone();
    monomorphize_generic_machine_value_calls_with_nominal_uses(&mut program, &mut Vec::new())
        .expect("nested reference argument closes in the cloned caller");
    assert_template_unchanged(&original, &program, "relay");
    assert_template_unchanged(&original, &program, "inspect");
    assert_eq!(program.machine_specializations.len(), 2);
    let relay = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "relay")
        .expect("relay");
    let relay_receipt = program
        .machine_specializations
        .iter()
        .find(|receipt| receipt.template == relay.symbol)
        .expect("one relay instance");
    let relay_instance = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == relay_receipt.instance)
        .expect("relay instance");
    let entry = program.machine_states(relay_instance)[0].symbol;
    for name in ["first", "second"] {
        let caller = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .expect("caller");
        assert_eq!(call_targets(&program, caller), [entry]);
    }
    let inspect_receipt = program
        .machine_specializations
        .iter()
        .find(|receipt| receipt.template != relay.symbol)
        .expect("one inspect instance");
    let inspect_instance = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == inspect_receipt.instance)
        .expect("inspect instance");
    assert_eq!(
        call_targets(&program, relay_instance),
        [program.machine_states(inspect_instance)[0].symbol]
    );
    assert!(!relay_instance.is_public);
    assert!(!inspect_instance.is_public);
}
