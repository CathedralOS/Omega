use super::*;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;

fn aggregate_result_program(body: &str, extra: &str, scalar: &str) -> typed_trees::TypedTrees {
    let source = format!(
        r#"
        data Cell {{ value: {scalar}; }}
        data View {{ body: &mut {scalar}; tag: u64; }}
        data OtherView {{ body: &mut {scalar}; tag: u64; }}
        data Outer {{ inner: View; }}
        data Choice {{ case Selected(view: View); case Empty; }}
        data Main {{ value: {scalar}; other: {scalar}; audit: {scalar}; cells: [{scalar}; 2]; }}
        machine make_view(value: &mut {scalar}) -> View {{ View {{ body: value }} }}
        machine make_local(value: &mut {scalar}) -> View {{ let result: View = View {{ body: value }}; result }}
        machine make_moved(value: &mut {scalar}) -> View {{ let first: View = View {{ body: value }}; let result: View = first; result }}
        machine make_nested(value: &mut {scalar}) -> View {{ make_view(value) }}
        machine make_reused(value: &mut {scalar}) -> View {{ write_view(View {{ body: value }}); View {{ body: value }} }}
        machine make_transition(value: &mut {scalar}) -> View {{ let result: View = View {{ body: value }}; transition {{ _ -> result }} }}
        machine forward_view(value: View) -> View {{ value }}
        machine make_outer(value: &mut {scalar}) -> Outer {{ Outer {{ inner: make_view(value) }} }}
        machine make_choice(value: &mut {scalar}) -> Choice {{ Choice::Selected {{ view: View {{ body: value }} }} }}
        machine make_empty() -> Choice {{ Choice::Empty {{}} }}
        machine make_array(values: &mut [{scalar}; 2]) -> [View; 2] {{ [View {{ body: &mut values[0] }}, View {{ body: &mut values[1] }}] }}
        machine foreign_view(value: &mut {scalar}) -> OtherView {{ OtherView {{ body: value }} }}
        machine write_view(mut view: View) {{ view.body = 255; }}
        machine write_outer(mut outer: Outer) {{ outer.inner.body = 255; }}
        machine write_choice(mut choice: Choice) {{ choice.view.body = 255; }}
        machine write_array(mut values: [View; 2]) {{ values[0].body = 255; }}
        machine Main::run(&mut self) {{ {body} }}
        {extra}
    "#
    );
    let syntax =
        parse_syntax_trees(&Lexer::new(&source).tokenize().expect("tokenize")).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

#[test]
fn aggregate_helper_results_transport_complete_reference_origins() {
    let cases = [
        (
            "stored_result",
            "let local: View = make_view(&mut self.value); write_view(local);",
            "",
            Some(vec!["self.value"]),
        ),
        (
            "direct_result",
            "write_view(make_view(&mut self.value));",
            "",
            Some(vec!["self.value"]),
        ),
        (
            "finite_consumer_reuse_in_producer",
            "write_view(make_reused(&mut self.value));",
            "",
            Some(vec!["self.value"]),
        ),
        (
            "local_result",
            "let local: View = make_local(&mut self.value); write_view(local);",
            "",
            Some(vec!["self.value"]),
        ),
        (
            "moved_result",
            "let local: View = make_moved(&mut self.value); write_view(local);",
            "",
            Some(vec!["self.value"]),
        ),
        (
            "nested_result",
            "let local: View = make_nested(&mut self.value); write_view(local);",
            "",
            Some(vec!["self.value"]),
        ),
        (
            "owned_carrier_formal_result",
            "let first: View = make_view(&mut self.value); let local: View = forward_view(first); write_view(local);",
            "",
            Some(vec!["self.value"]),
        ),
        (
            "terminal_value_transition",
            "let local: View = make_transition(&mut self.value); write_view(local);",
            "",
            Some(vec!["self.value"]),
        ),
        (
            "result_moved_again",
            "let first: View = make_view(&mut self.value); let local: View = first; write_view(local);",
            "",
            Some(vec!["self.value"]),
        ),
        (
            "nested_literal",
            "let local: Outer = Outer { inner: make_view(&mut self.value) }; write_outer(local);",
            "",
            Some(vec!["self.value"]),
        ),
        (
            "immediate_literal",
            "write_outer(Outer { inner: make_view(&mut self.value) });",
            "",
            Some(vec!["self.value"]),
        ),
        (
            "outer_result",
            "let local: Outer = make_outer(&mut self.value); write_outer(local);",
            "",
            Some(vec!["self.value"]),
        ),
        (
            "outer_projection",
            "let first: Outer = make_outer(&mut self.value); let local: View = first.inner; write_view(local);",
            "",
            Some(vec!["self.value"]),
        ),
        (
            "selected_result",
            "let local: Choice = make_choice(&mut self.value); write_choice(local);",
            "",
            Some(vec!["self.value"]),
        ),
        (
            "selected_projection",
            "let first: Choice = make_choice(&mut self.value); let local: View = first.view; write_view(local);",
            "",
            Some(vec!["self.value"]),
        ),
        (
            "empty_result",
            "let local: Choice = make_empty(); write_choice(local);",
            "",
            Some(vec![]),
        ),
        (
            "array_result",
            "let local: [View; 2] = make_array(&mut self.cells); write_array(local);",
            "",
            Some(vec!["self.cells"]),
        ),
        (
            "array_projection",
            "let first: [View; 2] = make_array(&mut self.cells); let local: View = first[0]; write_view(local);",
            "",
            Some(vec!["self.cells"]),
        ),
        (
            "frozen_caller_alias",
            "let mut alias: &mut u64 = &mut self.value; let local: View = make_view(alias); alias = &mut self.other; write_view(local);",
            "",
            Some(vec!["self.value"]),
        ),
        (
            "frozen_helper_alias",
            "let local: View = make_frozen(&mut self.value, &mut self.other); write_view(local);",
            "machine make_frozen(first: &mut u64, second: &mut u64) -> View { let mut alias: &mut u64 = first; let held: View = View { body: alias }; alias = second; held }",
            Some(vec!["self.value"]),
        ),
        (
            "missing_producer",
            "let local: View = absent(&mut self.value); write_view(local);",
            "",
            None,
        ),
        (
            "private_owned_parameter",
            "let local: View = private_owned(Cell { value: 0 }); write_view(local);",
            "machine private_owned(mut cell: Cell) -> View { View { body: &mut cell.value } }",
            None,
        ),
        (
            "mixed_array_private_parameter_field",
            "let local: View = mixed_private([Mixed { body: &mut self.value, value: 0 }, Mixed { body: &mut self.other, value: 0 }]); write_view(local);",
            "data Mixed { body: &mut u64; value: u64; } machine mixed_private(mut values: [Mixed; 2]) -> View { View { body: &mut values[0].value } }",
            None,
        ),
        (
            "private_local",
            "let local: View = private_local(); write_view(local);",
            "machine private_local() -> View { let mut scratch: u64 = 0; View { body: &mut scratch } }",
            None,
        ),
        (
            "recursive_result",
            "let local: View = recursive_view(&mut self.value); write_view(local);",
            "machine recursive_view(value: &mut u64) -> View { recursive_view(value) }",
            None,
        ),
        (
            "reference_slot_replacement",
            "let local: View = replace_slot(&mut self.value, &mut self.other); write_view(local);",
            "machine replace_slot(value: &mut u64, other: &mut u64) -> View { let mut local: View = View { body: value }; local.body = other; local }",
            None,
        ),
        (
            "whole_carrier_replacement",
            "let local: View = replace_carrier(&mut self.value, &mut self.other); write_view(local);",
            "machine replace_carrier(value: &mut u64, other: &mut u64) -> View { let mut local: View = View { body: value }; local = View { body: other }; local }",
            None,
        ),
        (
            "wrong_result_nominal",
            "let local: View = wrong_view(&mut self.value); write_view(local);",
            "machine wrong_view(value: &mut u64) -> View { OtherView { body: value } }",
            None,
        ),
        (
            "empty_payload_projection",
            "let first: Choice = make_empty(); let local: View = first.view; write_view(local);",
            "",
            None,
        ),
        (
            "terminal_reference_binding_exposure",
            "let local: View = terminal_slot(&mut self.value); write_view(local);",
            "machine touch_slot(slot: &mut u64) -> u64 { 0 } machine terminal_slot(mut value: &mut u64) -> View { View { body: value, tag: touch_slot(&mut value) } }",
            None,
        ),
        (
            "prefix_reference_binding_exposure",
            "let local: View = prefix_slot(&mut self.value); write_view(local);",
            "machine touch_slot(slot: &mut u64) -> u64 { 0 } machine prefix_slot(mut value: &mut u64) -> View { let ignored: u64 = touch_slot(&mut value); View { body: value } }",
            None,
        ),
        (
            "named_state_result",
            "let local: View = routed_view(&mut self.value); write_view(local);",
            "machine routed_view(value: &mut u64) -> View { transition { _ -> finish(value) } state finish(source: &mut u64) { View { body: source } } }",
            None,
        ),
    ];
    let mut failures = Vec::new();
    for (name, body, extra, expected) in cases {
        let program = aggregate_result_program(body, extra, "u64");
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::run")
            .expect("caller");
        let state = &program.machine_states(machine)[0];
        let StatementNode::Call(call) = program
            .statement_table
            .statements(state.statement_nodes)
            .last()
            .expect("call")
        else {
            panic!("write demand");
        };
        let resolver = validation::CallFrameResolver::new(&program).expect("resolver");
        for (query, paths) in [
            (
                "state",
                resolver
                    .inferred_state_write_frame(machine, state)
                    .into_complete_paths(),
            ),
            ("public call", resolver.may_write_paths(machine, call)),
        ] {
            let actual = paths.map(|paths| {
                let mut paths: Vec<_> = paths
                    .into_iter()
                    .filter(|path| path == "self" || path.starts_with("self."))
                    .collect();
                paths.sort();
                paths.dedup();
                paths
            });
            let expected = expected
                .as_ref()
                .map(|paths| paths.iter().map(|path| (*path).to_owned()).collect());
            if actual != expected {
                failures.push(format!(
                    "{name} {query}: expected {expected:?}, got {actual:?}"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}

#[test]
fn aggregate_helper_result_producer_writes_do_not_become_later_call_writes() {
    let source = r#"
        data View<'source> { body: &'source mut u64; }
        machine make_audited<'value, 'audit>(value: &'value mut u64, audit: &'audit mut u64) -> View<'value> {
            audit = 1;
            View { body: value }
        }
        machine write_view<'source>(mut view: View<'source>) { view.body = 255; }
        machine exercise<'value, 'audit>(value: &'value mut u64, audit: &'audit mut u64) {
            let local: View<'value> = make_audited(value, audit);
            write_view(local);
        }
    "#;
    let syntax =
        parse_syntax_trees(&Lexer::new(source).tokenize().expect("tokenize")).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let program = lower_symbol_resolved_trees(&resolved).expect("type");
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "exercise")
        .expect("caller");
    let state = &program.machine_states(machine)[0];
    let StatementNode::Call(call) = program
        .statement_table
        .statements(state.statement_nodes)
        .last()
        .expect("call")
    else {
        panic!("call");
    };
    let resolver = validation::CallFrameResolver::new(&program).expect("resolver");
    for (query, paths, expected) in [
        (
            "state",
            resolver
                .inferred_state_write_frame(machine, state)
                .into_complete_paths(),
            vec!["$P0", "$P1"],
        ),
        (
            "later call",
            resolver.may_write_paths(machine, call),
            vec!["value"],
        ),
    ] {
        let mut paths: Vec<_> = paths
            .expect("complete frame")
            .into_iter()
            .filter(|path| path.starts_with("$P") || path == "value" || path == "audit")
            .collect();
        paths.sort();
        paths.dedup();
        assert_eq!(paths, expected, "{query}");
    }
    lower_typed_trees(program).expect("audited result has an explicit retained input lifetime");
}

#[test]
fn aggregate_helper_results_reach_checked_trees() {
    for body in [
        "write_view(make_view(&mut self.value));",
        "write_view(make_reused(&mut self.value));",
        "let local: View = make_view(&mut self.value); write_view(local);",
        "let local: View = make_local(&mut self.value); write_view(local);",
        "let local: View = make_moved(&mut self.value); write_view(local);",
        "let local: View = make_nested(&mut self.value); write_view(local);",
        "let local: View = make_transition(&mut self.value); write_view(local);",
        "let local: Outer = Outer { inner: make_view(&mut self.value) }; write_outer(local);",
    ] {
        lower_typed_trees(aggregate_result_program(body, "", "u64"))
            .expect("aggregate result reaches checked trees");
    }
}

#[test]
fn aggregate_helper_result_target_identity_is_live_and_nominally_compatible() {
    let original = aggregate_result_program(
        "let local: View = make_view(&mut self.value); write_view(local);",
        "",
        "u64",
    );
    let machine = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::run")
        .expect("caller");
    let state = &original.machine_states(machine)[0];
    let StatementNode::LocalData(local) =
        &original.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("result local");
    };
    let expression = local.initial_value;
    let ExpressionNode::Call(call) = original.expression_table.expression(expression) else {
        panic!("producer");
    };
    let exact = call.target_symbol;
    let foreign = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "foreign_view")
        .expect("wrong nominal producer");
    let foreign = original.machine_states(foreign)[0].symbol;
    for (name, target, complete) in [
        ("exact", exact, true),
        ("wrong_nominal", foreign, false),
        (
            "stale",
            symbols::SymbolHandle::from_parts(exact.arena_index(), exact.generation() + 1),
            false,
        ),
    ] {
        let mut program = original.clone();
        let ExpressionNode::Call(call) = program.expression_table.expression_mut(expression) else {
            panic!("producer");
        };
        call.target_symbol = target;
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::run")
            .expect("caller");
        let state = &program.machine_states(machine)[0];
        let statement = program
            .statement_table
            .statements(state.statement_nodes)
            .last()
            .expect("call");
        let StatementNode::Call(call) = statement else {
            panic!("call");
        };
        let resolver = validation::CallFrameResolver::new(&program).expect("resolver");
        assert_eq!(
            resolver
                .inferred_state_write_frame(machine, state)
                .is_complete(),
            complete,
            "{name}: state"
        );
        assert_eq!(
            resolver.may_write_frame(machine, call).is_complete(),
            complete,
            "{name}: demand"
        );
        assert_eq!(
            resolver
                .local_write_origins_before_statement(machine, statement)
                .is_some(),
            complete,
            "{name}: metadata"
        );
    }
}

#[test]
fn aggregate_helper_results_keep_independent_input_lifetimes_and_origins() {
    let source = r#"
        data Pair<'left, 'right> { left: &'left mut u64; right: &'right mut u64; }
        machine make_pair<'left, 'right>(left: &'left mut u64, right: &'right mut u64) -> Pair<'left, 'right> {
            Pair { left: left, right: right }
        }
        machine write_pair<'left, 'right>(mut pair: Pair<'left, 'right>) { pair.left = 1; pair.right = 2; }
        machine exercise<'left, 'right>(first: &'left mut u64, second: &'right mut u64) {
            let local: Pair<'left, 'right> = make_pair(first, second);
            write_pair(local);
        }
    "#;
    let syntax =
        parse_syntax_trees(&Lexer::new(source).tokenize().expect("tokenize")).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let program = lower_symbol_resolved_trees(&resolved).expect("type");
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "exercise")
        .expect("caller");
    let state = &program.machine_states(machine)[0];
    let StatementNode::Call(call) = program
        .statement_table
        .statements(state.statement_nodes)
        .last()
        .expect("call")
    else {
        panic!("call");
    };
    let resolver = validation::CallFrameResolver::new(&program).expect("resolver");
    for (paths, expected) in [
        (
            resolver
                .inferred_state_write_frame(machine, state)
                .into_complete_paths(),
            vec!["$P0", "$P1"],
        ),
        (
            resolver.may_write_paths(machine, call),
            vec!["first", "second"],
        ),
    ] {
        let mut paths: Vec<_> = paths
            .expect("complete independent origins")
            .into_iter()
            .filter(|path| path.starts_with("$P") || path == "first" || path == "second")
            .collect();
        paths.sort();
        paths.dedup();
        assert_eq!(paths, expected);
    }
    lower_typed_trees(program).expect("independent result lifetimes reach checked trees");
}

#[test]
fn aggregate_helper_result_writes_invalidate_prior_arithmetic_facts() {
    for (name, body) in [
        (
            "owner",
            "self.value = 0; let local: View = make_view(&mut self.value); write_view(local); self.value = self.value + 1;",
        ),
        (
            "alias",
            "let mut local: View = make_view(&mut self.value); local.body = 0; self.value = 255; self.value = local.body + 1;",
        ),
    ] {
        match validation::validate_program(&aggregate_result_program(body, "", "u8")) {
            Err(diagnostics)
                if diagnostics.iter().any(|diagnostic| {
                    let message = diagnostic.to_string();
                    message.contains("Main::run") && message.contains("may overflow")
                }) => {}
            result => panic!("{name}: aggregate result retained stale fact: {result:?}"),
        }
    }
}

#[test]
fn aggregate_helper_result_from_exclusive_self_retains_owned_field_origin() {
    let program = aggregate_result_program(
        "",
        r#"
        data Holder { cell: Cell; }
        machine Cell::make_view(&mut self) -> View { View { body: &mut self.value } }
        machine Holder::run(&mut self) {
            let local: View = self.cell.make_view();
            write_view(local);
        }
    "#,
        "u64",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Holder::run")
        .expect("caller");
    let state = &program.machine_states(machine)[0];
    let StatementNode::Call(call) = program
        .statement_table
        .statements(state.statement_nodes)
        .last()
        .expect("call")
    else {
        panic!("call");
    };
    let resolver = validation::CallFrameResolver::new(&program).expect("resolver");
    for paths in [
        resolver
            .inferred_state_write_frame(machine, state)
            .into_complete_paths(),
        resolver.may_write_paths(machine, call),
    ] {
        let paths: Vec<_> = paths
            .expect("attached result origin")
            .into_iter()
            .filter(|path| path == "self" || path.starts_with("self."))
            .collect();
        assert_eq!(paths, vec!["self.cell.value"]);
    }
    lower_typed_trees(program).expect("attached aggregate result reaches checked trees");
}

#[test]
fn aggregate_helper_result_reborrow_fence_peels_constrained_formals() {
    for (name, helper_body) in [
        (
            "terminal",
            "View { body: value, tag: touch_slot(&mut value) }",
        ),
        (
            "prefix",
            "let ignored: u64 = touch_slot(&mut value); View { body: value }",
        ),
    ] {
        let extra = format!(
            "machine touch_slot(slot: &mut u64) -> u64 {{ 0 }} machine exposed(mut value: &mut u64) -> View {{ {helper_body} }}"
        );
        let mut program = aggregate_result_program(
            "let local: View = exposed(&mut self.value); write_view(local);",
            &extra,
            "u64",
        );
        let helper = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "exposed")
            .expect("helper");
        let reference =
            program.state_parameters(&program.machine_states(helper)[0])[0].type_reference;
        let node = program
            .type_reference_table
            .type_reference(reference)
            .clone();
        assert!(matches!(
            node,
            typed_trees::types::TypeReferenceNode::Reference { .. }
        ));
        let base_type = program.type_reference_table.insert(node);
        program.type_reference_table.substitute_node(
            reference,
            typed_trees::types::TypeReferenceNode::Constrained {
                base_type,
                constraints: Default::default(),
            },
        );
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::run")
            .expect("caller");
        let state = &program.machine_states(machine)[0];
        let StatementNode::Call(call) = program
            .statement_table
            .statements(state.statement_nodes)
            .last()
            .expect("call")
        else {
            panic!("call");
        };
        let resolver = validation::CallFrameResolver::new(&program).expect("resolver");
        assert!(
            !resolver
                .inferred_state_write_frame(machine, state)
                .is_complete(),
            "{name}: state"
        );
        assert!(
            !resolver.may_write_frame(machine, call).is_complete(),
            "{name}: demand"
        );
    }
}

#[test]
fn aggregate_helper_borrowed_source_requires_exact_live_parameter_identity() {
    let original = aggregate_result_program(
        "",
        r#"
        data Holder { cell: Cell; }
        machine projected(value: &mut Cell) -> View { View { body: &mut value.value } }
        machine foreign_projected(value: &mut Cell) -> View { View { body: &mut value.value } }
        machine Holder::run(&mut self) {
            let local: View = projected(&mut self.cell);
            write_view(local);
        }
    "#,
        "u64",
    );
    let helper = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "projected")
        .expect("helper");
    let helper_state = &original.machine_states(helper)[0];
    let exact = original.state_parameters(helper_state)[0].symbol;
    let StatementNode::Expression(result) = original
        .statement_table
        .statements(helper_state.statement_nodes)
        .last()
        .expect("result")
    else {
        panic!("terminal result");
    };
    let ExpressionNode::StructLiteral(literal) = original.expression_table.expression(*result)
    else {
        panic!("result literal");
    };
    let initializer = original.expression_table.struct_fields(literal.fields)[0].value;
    let ExpressionNode::Borrow(borrow) = original.expression_table.expression(initializer) else {
        panic!("borrowed field");
    };
    let mut root_expression = borrow.target;
    loop {
        match original.expression_table.expression(root_expression) {
            ExpressionNode::Member(member) => root_expression = member.receiver,
            ExpressionNode::Name(_) => break,
            unexpected => panic!("borrowed root: {unexpected:?}"),
        }
    }
    let foreign = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "foreign_projected")
        .expect("foreign helper");
    let foreign = original.state_parameters(&original.machine_states(foreign)[0])[0].symbol;
    for (name, symbol, complete) in [
        ("exact", exact, true),
        ("foreign", foreign, false),
        (
            "stale",
            symbols::SymbolHandle::from_parts(exact.arena_index(), exact.generation() + 1),
            false,
        ),
    ] {
        let mut program = original.clone();
        let ExpressionNode::Name(path) = program.expression_table.expression_mut(root_expression)
        else {
            panic!("root");
        };
        path.head_symbol = symbol;
        if path.members.len() == 1 {
            path.symbol = symbol;
        }
        let member_symbols = path.member_symbols;
        program
            .expression_table
            .set_name_path_member_symbol_at_offset(member_symbols, 0, symbol);
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Holder::run")
            .expect("caller");
        let state = &program.machine_states(machine)[0];
        let statement = program
            .statement_table
            .statements(state.statement_nodes)
            .last()
            .expect("call");
        let StatementNode::Call(call) = statement else {
            panic!("call");
        };
        let resolver = validation::CallFrameResolver::new(&program).expect("resolver");
        assert_eq!(
            resolver
                .inferred_state_write_frame(machine, state)
                .is_complete(),
            complete,
            "{name}: state"
        );
        assert_eq!(
            resolver.may_write_frame(machine, call).is_complete(),
            complete,
            "{name}: demand"
        );
        assert_eq!(
            resolver
                .local_write_origins_before_statement(machine, statement)
                .is_some(),
            complete,
            "{name}: metadata"
        );
    }
}
