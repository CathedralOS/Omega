use super::*;
use psi_typed_trees::expression::ExpressionNode;
use psi_typed_trees::statement::StatementNode;

fn contextual_case_program(body: &str) -> psi_typed_trees::TypedTrees {
    let source = format!(
        r#"
        data View {{ body: &mut u64; }}
        data Outer {{ inner: View; }}
        data Choice {{ case Selected(view: View); case Empty; }}
        data Nested {{ inner: Choice; }}
        data Plain {{ tag: u64; }}
        data PlainChoice {{ case Selected(view: Plain); case Empty; }}
        data PlainOuter {{ inner: Plain; }}
        data Shared {{ body: &u64; tag: u64; }}
        data SharedChoice {{ case Selected(view: Shared); case Empty; }}
        data SharedOuter {{ inner: Shared; }}
        data Main {{ value: u64; other: u64; }}
        machine write_outer(mut outer: Outer) {{ outer.inner.body = 1; }}
        machine write_value(mut outer: Outer) -> u64 {{ outer.inner.body = 1; 0 }}
        machine identity_outer(input: Outer) -> Outer {{ input }}
        machine make_choice(value: &mut u64) -> Choice {{ Choice::Selected {{ view: View {{ body: value }} }} }}
        machine write_plain(mut outer: PlainOuter) {{ outer.inner.tag = 1; }}
        machine write_shared(mut outer: SharedOuter) {{ outer.inner.tag = 1; }}
        machine clear_plain(value: &mut PlainChoice) -> u64 {{ value = PlainChoice::Empty {{}}; 0 }}
        machine clear_shared(value: &mut SharedChoice) -> u64 {{ value = SharedChoice::Empty {{}}; 0 }}
        machine set_tag(value: &mut u64) {{ value = 1; }}
        machine write_plain_after(ignored: u64, mut outer: PlainOuter) {{ outer.inner.tag = 1; }}
        machine write_plain_value(mut outer: PlainOuter) -> u64 {{ outer.inner.tag = 1; 0 }}
        machine PlainChoice::clear(&mut self) {{ self = PlainChoice::Empty {{}}; }}
        machine PlainChoice::identity(&mut self) -> &mut PlainChoice {{ self }}
        machine Main::run(&mut self, input: Choice, index: u64) {{ {body} }}
        machine foreign(value: &mut u64) {{ let local: Choice = Choice::Selected {{ view: View {{ body: value }} }}; }}
        "#
    );
    let syntax =
        parse_syntax_trees(&Lexer::new(&source).tokenize().expect("tokenize")).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

fn visible_paths(paths: Option<Vec<String>>) -> Option<Vec<String>> {
    paths.map(|paths| {
        let mut visible = paths
            .into_iter()
            .filter(|path| path == "self" || path.starts_with("self."))
            .collect::<Vec<_>>();
        visible.sort();
        visible.dedup();
        visible
    })
}

fn statement_frames(program: &psi_typed_trees::TypedTrees) -> [Option<Vec<String>>; 2] {
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
    let resolver = psi_validation::CallFrameResolver::new(program).expect("resolver");
    [
        resolver
            .inferred_state_write_frame(machine, state)
            .into_complete_paths(),
        resolver.may_write_paths(machine, call),
    ]
    .map(visible_paths)
}

#[test]
fn immediate_payload_moves_use_the_owning_local_case_context() {
    let mut failures = Vec::new();
    for (name, body, expected) in [
        (
            "selected",
            "let local: Choice = Choice::Selected { view: View { body: &mut self.value } }; write_outer(Outer { inner: local.view });",
            Some(vec!["self.value"]),
        ),
        (
            "copied",
            "let first: Choice = Choice::Selected { view: View { body: &mut self.value } }; let local: Choice = first; write_outer(Outer { inner: local.view });",
            Some(vec!["self.value"]),
        ),
        (
            "nested",
            "let local: Nested = Nested { inner: Choice::Selected { view: View { body: &mut self.value } } }; write_outer(Outer { inner: local.inner.view });",
            Some(vec!["self.value"]),
        ),
        (
            "helper_result",
            "let local: Choice = make_choice(&mut self.value); write_outer(Outer { inner: local.view });",
            Some(vec!["self.value"]),
        ),
        (
            "frozen_alias",
            "let mut alias: &mut u64 = &mut self.value; let local: Choice = Choice::Selected { view: View { body: alias } }; alias = &mut self.other; write_outer(Outer { inner: local.view });",
            Some(vec!["self.value"]),
        ),
        (
            "fixed_selected",
            "let local: [Choice; 2] = [Choice::Selected { view: View { body: &mut self.value } }, Choice::Empty {}]; write_outer(Outer { inner: local[0].view });",
            Some(vec!["self.value"]),
        ),
        (
            "runtime_all_selected",
            "let local: [Choice; 2] = [Choice::Selected { view: View { body: &mut self.value } }, Choice::Selected { view: View { body: &mut self.other } }]; write_outer(Outer { inner: local[index].view });",
            Some(vec!["self.other", "self.value"]),
        ),
        (
            "owned_payload",
            "let local: PlainChoice = PlainChoice::Selected { view: Plain { tag: 0 } }; write_plain(PlainOuter { inner: local.view });",
            Some(vec![]),
        ),
        (
            "shared_payload",
            "let local: SharedChoice = SharedChoice::Selected { view: Shared { body: &self.value, tag: 0 } }; write_shared(SharedOuter { inner: local.view });",
            Some(vec![]),
        ),
        (
            "absent_payload",
            "let local: Choice = Choice::Empty {}; write_outer(Outer { inner: local.view });",
            None,
        ),
        (
            "fixed_empty",
            "let local: [Choice; 2] = [Choice::Selected { view: View { body: &mut self.value } }, Choice::Empty {}]; write_outer(Outer { inner: local[1].view });",
            None,
        ),
        (
            "runtime_mixed",
            "let local: [Choice; 2] = [Choice::Selected { view: View { body: &mut self.value } }, Choice::Empty {}]; write_outer(Outer { inner: local[index].view });",
            None,
        ),
        (
            "absent_owned_payload",
            "let local: PlainChoice = PlainChoice::Empty {}; write_plain(PlainOuter { inner: local.view });",
            None,
        ),
        (
            "absent_shared_payload",
            "let local: SharedChoice = SharedChoice::Empty {}; write_shared(SharedOuter { inner: local.view });",
            None,
        ),
        (
            "unknown_parameter_case",
            "write_outer(Outer { inner: input.view });",
            None,
        ),
        (
            "unknown_moved_parameter_case",
            "let local: Choice = input; write_outer(Outer { inner: local.view });",
            None,
        ),
        (
            "replaced_carrier",
            "let mut local: Choice = Choice::Selected { view: View { body: &mut self.value } }; local = Choice::Empty {}; write_outer(Outer { inner: local.view });",
            None,
        ),
        (
            "replaced_plain_case",
            "let mut local: PlainChoice = PlainChoice::Selected { view: Plain { tag: 0 } }; local = PlainChoice::Empty {}; write_plain(PlainOuter { inner: local.view });",
            None,
        ),
        (
            "replaced_shared_case",
            "let mut local: SharedChoice = SharedChoice::Selected { view: Shared { body: &self.value, tag: 0 } }; local = SharedChoice::Empty {}; write_shared(SharedOuter { inner: local.view });",
            None,
        ),
        (
            "exposed_plain_case",
            "let mut local: PlainChoice = PlainChoice::Selected { view: Plain { tag: 0 } }; clear_plain(&mut local); write_plain(PlainOuter { inner: local.view });",
            None,
        ),
        (
            "exposed_shared_case",
            "let mut local: SharedChoice = SharedChoice::Selected { view: Shared { body: &self.value, tag: 0 } }; clear_shared(&mut local); write_shared(SharedOuter { inner: local.view });",
            None,
        ),
        (
            "alias_to_plain_case",
            "let mut local: PlainChoice = PlainChoice::Selected { view: Plain { tag: 0 } }; let alias: &mut PlainChoice = &mut local; clear_plain(alias); write_plain(PlainOuter { inner: local.view });",
            None,
        ),
        (
            "same_expression_exposure",
            "let mut local: PlainChoice = PlainChoice::Selected { view: Plain { tag: 0 } }; write_plain_after(clear_plain(&mut local), PlainOuter { inner: local.view });",
            None,
        ),
        (
            "payload_scalar_store",
            "let mut local: PlainChoice = PlainChoice::Selected { view: Plain { tag: 0 } }; local.view.tag = 2; write_plain(PlainOuter { inner: local.view });",
            Some(vec![]),
        ),
        (
            "payload_scalar_borrow",
            "let mut local: PlainChoice = PlainChoice::Selected { view: Plain { tag: 0 } }; set_tag(&mut local.view.tag); write_plain(PlainOuter { inner: local.view });",
            Some(vec![]),
        ),
        (
            "payload_value_replacement",
            "let mut local: PlainChoice = PlainChoice::Selected { view: Plain { tag: 0 } }; local.view = Plain { tag: 2 }; write_plain(PlainOuter { inner: local.view });",
            Some(vec![]),
        ),
        (
            "implicit_case_receiver",
            "let mut local: PlainChoice = PlainChoice::Selected { view: Plain { tag: 0 } }; local.clear(); write_plain(PlainOuter { inner: local.view });",
            None,
        ),
        (
            "returned_case_alias",
            "let mut local: PlainChoice = PlainChoice::Selected { view: Plain { tag: 0 } }; let alias: &mut PlainChoice = local.identity(); clear_plain(alias); write_plain(PlainOuter { inner: local.view });",
            None,
        ),
        (
            "unknown_source",
            "write_outer(Outer { inner: absent.view });",
            None,
        ),
    ] {
        let expected = expected.map(|paths| paths.into_iter().map(str::to_owned).collect());
        for (query, actual) in statement_frames(&contextual_case_program(body))
            .into_iter()
            .enumerate()
        {
            if actual != expected {
                failures.push(format!(
                    "{name} query {query}: expected {expected:?}, got {actual:?}"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}

#[test]
fn expression_call_payload_moves_use_the_same_case_context() {
    for (name, prefix, expected) in [
        (
            "selected",
            "let local: Choice = Choice::Selected { view: View { body: &mut self.value } };",
            Some(vec!["self.value".to_owned()]),
        ),
        ("absent", "let local: Choice = Choice::Empty {};", None),
        ("unknown", "let local: Choice = input;", None),
    ] {
        let program = contextual_case_program(&format!(
            "{prefix} let result: u64 = write_value(Outer {{ inner: local.view }});"
        ));
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::run")
            .expect("caller");
        let state = &program.machine_states(machine)[0];
        let StatementNode::LocalData(result) = program
            .statement_table
            .statements(state.statement_nodes)
            .last()
            .expect("result")
        else {
            panic!("result");
        };
        let resolver = psi_validation::CallFrameResolver::new(&program).expect("resolver");
        for (query, actual) in [
            resolver
                .inferred_state_write_frame(machine, state)
                .into_complete_paths(),
            resolver.expression_may_write_paths(machine, result.initial_value),
        ]
        .into_iter()
        .map(visible_paths)
        .enumerate()
        {
            assert_eq!(actual, expected, "{name} query {query}");
        }
    }
}

#[test]
fn immediate_payload_moves_match_stored_declaration_and_result_transfer() {
    for (name, body) in [
        (
            "intermediate_declaration",
            "let local: Choice = Choice::Selected { view: View { body: &mut self.value } }; let moved: View = local.view; write_outer(Outer { inner: moved });",
        ),
        (
            "stored_literal",
            "let local: Choice = Choice::Selected { view: View { body: &mut self.value } }; let moved: Outer = Outer { inner: local.view }; write_outer(moved);",
        ),
        (
            "helper_actual_literal",
            "let local: Choice = Choice::Selected { view: View { body: &mut self.value } }; let moved: Outer = identity_outer(Outer { inner: local.view }); write_outer(moved);",
        ),
    ] {
        for (query, paths) in statement_frames(&contextual_case_program(body))
            .into_iter()
            .enumerate()
        {
            assert_eq!(
                paths,
                Some(vec!["self.value".to_owned()]),
                "{name} query {query}"
            );
        }
    }
}

#[test]
fn named_state_payload_calls_retain_state_scoped_case_evidence() {
    for (name, body) in [
        (
            "acyclic_literal_edge",
            r#"
            let local: Choice = Choice::Selected { view: View { body: value } };
            transition { _ -> finish(Outer { inner: local.view }) }
            state finish(mut outer: Outer) { outer.inner.body = 1; }
        "#,
        ),
        (
            "call_inside_cycle",
            r#"
            transition { _ -> cycle(value) }
            state cycle(item: &mut u64) {
                let local: Choice = Choice::Selected { view: View { body: item } };
                write_outer(Outer { inner: local.view });
                transition { _ -> cycle(item) }
            }
        "#,
        ),
    ] {
        let source = format!(
            r#"
            data View {{ body: &mut u64; }}
            data Outer {{ inner: View; }}
            data Choice {{ case Selected(view: View); case Empty; }}
            machine write_outer(mut outer: Outer) {{ outer.inner.body = 1; }}
            machine probe(value: &mut u64) {{ {body} }}
        "#
        );
        let syntax =
            parse_syntax_trees(&Lexer::new(&source).tokenize().expect("tokenize")).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let program = lower_symbol_resolved_trees(&resolved).expect("type");
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "probe")
            .expect("caller");
        let resolver = psi_validation::CallFrameResolver::new(&program).expect("resolver");
        for _ in 0..2 {
            assert_eq!(
                resolver
                    .inferred_state_write_frame(machine, &program.machine_states(machine)[0])
                    .into_complete_paths(),
                Some(vec!["$P0".to_owned()]),
                "{name}"
            );
        }
        if name == "call_inside_cycle" {
            let state = program
                .machine_states(machine)
                .iter()
                .find(|state| state.name.as_str() == "cycle")
                .expect("cycle");
            let call = program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .find_map(|statement| match statement {
                    StatementNode::Call(call) => Some(call),
                    _ => None,
                })
                .expect("cycle call");
            let paths = resolver
                .may_write_paths(machine, call)
                .expect("public cycle call");
            assert!(paths.iter().any(|path| path == "item"), "{name}: {paths:?}");
        }
    }
}

#[test]
fn immediate_payload_case_evidence_cannot_cross_local_symbol_identity() {
    let original = contextual_case_program(
        "let local: Choice = Choice::Selected { view: View { body: &mut self.value } }; write_outer(Outer { inner: local.view });",
    );
    let machine = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::run")
        .expect("caller");
    let state = &original.machine_states(machine)[0];
    let StatementNode::Call(call) = original
        .statement_table
        .statements(state.statement_nodes)
        .last()
        .expect("call")
    else {
        panic!("call");
    };
    let argument = original.statement_table.expression_handles(call.arguments)[0];
    let ExpressionNode::StructLiteral(literal) = original.expression_table.expression(argument)
    else {
        panic!("literal");
    };
    let mut expression = original.expression_table.struct_fields(literal.fields)[0].value;
    while let ExpressionNode::Member(member) = original.expression_table.expression(expression) {
        expression = member.receiver;
    }
    let ExpressionNode::Name(path) = original.expression_table.expression(expression) else {
        panic!("source");
    };
    let exact = path.head_symbol;
    let foreign_machine = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "foreign")
        .expect("foreign");
    let foreign_state = &original.machine_states(foreign_machine)[0];
    let StatementNode::LocalData(foreign) = &original
        .statement_table
        .statements(foreign_state.statement_nodes)[0]
    else {
        panic!("foreign local");
    };
    for (name, symbol, complete) in [
        ("exact", exact, true),
        ("foreign", foreign.symbol, false),
        (
            "stale",
            psi_symbols::SymbolHandle::from_parts(exact.arena_index(), exact.generation() + 1),
            false,
        ),
    ] {
        let mut program = original.clone();
        let ExpressionNode::Name(path) = program.expression_table.expression_mut(expression) else {
            panic!("source");
        };
        path.head_symbol = symbol;
        if path.symbol == exact {
            path.symbol = symbol;
        }
        let member_symbols = path.member_symbols;
        program
            .expression_table
            .set_name_path_member_symbol_at_offset(member_symbols, 0, symbol);
        for (query, paths) in statement_frames(&program).into_iter().enumerate() {
            assert_eq!(paths.is_some(), complete, "{name} query {query}");
        }
    }
}
