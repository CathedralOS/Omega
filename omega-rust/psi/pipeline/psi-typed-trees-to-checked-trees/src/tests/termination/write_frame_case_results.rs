use super::*;
use psi_typed_trees::expression::ExpressionNode;
use psi_typed_trees::statement::StatementNode;

fn case_result_program(body: &str) -> psi_typed_trees::TypedTrees {
    case_result_program_with_helpers(body, "")
}

fn case_result_program_with_helpers(body: &str, helpers: &str) -> psi_typed_trees::TypedTrees {
    let source = format!(
        r#"
        data View {{ body: &mut u64; }}
        data Outer {{ inner: View; }}
        data Choice {{ case Selected(view: View); case Empty; }}
        data Nested {{ inner: Choice; }}
        data Pair {{ left: Choice; right: Choice; }}
        data Plain {{ tag: u64; }}
        data PlainChoice {{ case Selected(view: Plain); case Empty; }}
        data PlainOuter {{ inner: Plain; }}
        data Shared {{ body: &u64; tag: u64; }}
        data SharedChoice {{ case Selected(view: Shared); case Empty; }}
        data SharedOuter {{ inner: Shared; }}
        data Main {{ value: u64; other: u64; audit: u64; }}
        machine identity(input: Choice) -> Choice {{ input }}
        machine identity_nested(input: Nested) -> Nested {{ input }}
        machine identity_array(input: [Choice; 2]) -> [Choice; 2] {{ input }}
        machine identity_pair(input: Pair) -> Pair {{ input }}
        machine identity_plain(input: PlainChoice) -> PlainChoice {{ input }}
        machine identity_shared(input: SharedChoice) -> SharedChoice {{ input }}
        machine copy_input(input: Choice) -> Choice {{ let local: Choice = input; local }}
        machine wrap_input(input: Choice) -> Nested {{ Nested {{ inner: input }} }}
        machine project_nested(input: Nested) -> Choice {{ input.inner }}
        machine project_array(input: [Choice; 2]) -> Choice {{ input[0] }}
        machine project_runtime(input: [Choice; 2], index: u64) -> Choice {{ input[index] }}
        machine project_unproved(input: Choice) -> View {{ input.view }}
        machine forward(input: Choice) -> Choice {{ identity(input) }}
        machine make_choice(value: &mut u64) -> Choice {{ Choice::Selected {{ view: View {{ body: value }} }} }}
        machine replace_empty(input: Choice) -> Choice {{ Choice::Empty {{}} }}
        machine replace_selected(input: Choice, value: &mut u64) -> Choice {{ Choice::Selected {{ view: View {{ body: value }} }} }}
        machine private_result(input: Choice) -> Choice {{ let mut local: u64 = 0; Choice::Selected {{ view: View {{ body: &mut local }} }} }}
        machine private_pair(input: Choice) -> Pair {{ let mut local: u64 = 0; Pair {{ left: input, right: Choice::Selected {{ view: View {{ body: &mut local }} }} }} }}
        machine effectful(input: Choice, audit: &mut u64) -> Choice {{ audit = 1; input }}
        machine write_outer(mut outer: Outer) {{ outer.inner.body = 1; }}
        machine write_value(mut outer: Outer) -> u64 {{ outer.inner.body = 1; 0 }}
        machine write_plain(mut outer: PlainOuter) {{ outer.inner.tag = 1; }}
        machine write_shared(mut outer: SharedOuter) {{ outer.inner.tag = 1; }}
        machine Main::run(&mut self, input: Choice, index: u64) {{ {body} }}
        {helpers}
        "#
    );
    let syntax =
        parse_syntax_trees(&Lexer::new(&source).tokenize().expect("tokenize")).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

fn visible_paths(paths: Option<Vec<String>>) -> Option<Vec<String>> {
    paths.map(|paths| {
        let mut paths = paths
            .into_iter()
            .filter(|path| path == "self" || path.starts_with("self."))
            .collect::<Vec<_>>();
        paths.sort();
        paths.dedup();
        paths
    })
}

fn caller_frames(program: &psi_typed_trees::TypedTrees) -> [Option<Vec<String>>; 2] {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::run")
        .expect("caller");
    let state = &program.machine_states(machine)[0];
    let resolver = psi_validation::CallFrameResolver::new(program).expect("resolver");
    let public = match program
        .statement_table
        .statements(state.statement_nodes)
        .last()
        .expect("consumer")
    {
        StatementNode::Call(call) => resolver.may_write_paths(machine, call),
        StatementNode::LocalData(local) => {
            resolver.expression_may_write_paths(machine, local.initial_value)
        }
        _ => panic!("consumer"),
    };
    [
        resolver
            .inferred_state_write_frame(machine, state)
            .into_complete_paths(),
        public,
    ]
    .map(visible_paths)
}

#[test]
fn helper_result_cases_follow_whole_owned_input_moves() {
    for (name, body) in [
        (
            "literal",
            "let local: Choice = identity(Choice::Selected { view: View { body: &mut self.value } }); write_outer(Outer { inner: local.view });",
        ),
        (
            "local",
            "let source: Choice = Choice::Selected { view: View { body: &mut self.value } }; let local: Choice = identity(source); write_outer(Outer { inner: local.view });",
        ),
        (
            "copied_body",
            "let local: Choice = copy_input(Choice::Selected { view: View { body: &mut self.value } }); write_outer(Outer { inner: local.view });",
        ),
        (
            "nested_calls",
            "let local: Choice = identity(identity(Choice::Selected { view: View { body: &mut self.value } })); write_outer(Outer { inner: local.view });",
        ),
        (
            "forwarded_body",
            "let local: Choice = forward(Choice::Selected { view: View { body: &mut self.value } }); write_outer(Outer { inner: local.view });",
        ),
        (
            "helper_actual",
            "let local: Choice = identity(make_choice(&mut self.value)); write_outer(Outer { inner: local.view });",
        ),
        (
            "nested_move",
            "let local: Nested = identity_nested(Nested { inner: Choice::Selected { view: View { body: &mut self.value } } }); write_outer(Outer { inner: local.inner.view });",
        ),
        (
            "wrapped_move",
            "let local: Nested = wrap_input(Choice::Selected { view: View { body: &mut self.value } }); write_outer(Outer { inner: local.inner.view });",
        ),
        (
            "projected_move",
            "let local: Choice = project_nested(Nested { inner: Choice::Selected { view: View { body: &mut self.value } } }); write_outer(Outer { inner: local.view });",
        ),
        (
            "frozen_alias",
            "let mut alias: &mut u64 = &mut self.value; let source: Choice = Choice::Selected { view: View { body: alias } }; alias = &mut self.other; let local: Choice = identity(source); write_outer(Outer { inner: local.view });",
        ),
        (
            "stored_projection",
            "let local: Choice = identity(Choice::Selected { view: View { body: &mut self.value } }); let outer: Outer = Outer { inner: local.view }; write_outer(outer);",
        ),
        (
            "expression_consumer",
            "let local: Choice = identity(Choice::Selected { view: View { body: &mut self.value } }); let result: u64 = write_value(Outer { inner: local.view });",
        ),
    ] {
        let expected = Some(vec!["self.value".to_owned()]);
        assert_eq!(
            caller_frames(&case_result_program(body)),
            [expected.clone(), expected],
            "{name}"
        );
    }
}

#[test]
fn helper_result_cases_preserve_array_selectors_and_runtime_unions() {
    for (name, body, expected) in [
        (
            "fixed_selected",
            "let local: [Choice; 2] = identity_array([Choice::Selected { view: View { body: &mut self.value } }, Choice::Empty {}]); write_outer(Outer { inner: local[0].view });",
            Some(vec!["self.value"]),
        ),
        (
            "fixed_second",
            "let local: [Choice; 2] = identity_array([Choice::Empty {}, Choice::Selected { view: View { body: &mut self.other } }]); write_outer(Outer { inner: local[1].view });",
            Some(vec!["self.other"]),
        ),
        (
            "projected_selected",
            "let local: Choice = project_array([Choice::Selected { view: View { body: &mut self.value } }, Choice::Empty {}]); write_outer(Outer { inner: local.view });",
            Some(vec!["self.value"]),
        ),
        (
            "runtime_selected",
            "let local: [Choice; 2] = identity_array([Choice::Selected { view: View { body: &mut self.value } }, Choice::Selected { view: View { body: &mut self.other } }]); write_outer(Outer { inner: local[index].view });",
            Some(vec!["self.other", "self.value"]),
        ),
        (
            "projected_runtime_selected",
            "let local: Choice = project_runtime([Choice::Selected { view: View { body: &mut self.value } }, Choice::Selected { view: View { body: &mut self.other } }], index); write_outer(Outer { inner: local.view });",
            Some(vec!["self.other", "self.value"]),
        ),
        (
            "fixed_absent",
            "let local: [Choice; 2] = identity_array([Choice::Selected { view: View { body: &mut self.value } }, Choice::Empty {}]); write_outer(Outer { inner: local[1].view });",
            None,
        ),
        (
            "runtime_mixed",
            "let local: [Choice; 2] = identity_array([Choice::Selected { view: View { body: &mut self.value } }, Choice::Empty {}]); write_outer(Outer { inner: local[index].view });",
            None,
        ),
        (
            "projected_runtime_mixed",
            "let local: Choice = project_runtime([Choice::Selected { view: View { body: &mut self.value } }, Choice::Empty {}], index); write_outer(Outer { inner: local.view });",
            None,
        ),
    ] {
        let expected = expected.map(|paths| paths.into_iter().map(str::to_owned).collect());
        assert_eq!(
            caller_frames(&case_result_program(body)),
            [expected.clone(), expected],
            "{name}"
        );
    }
}

#[test]
fn helper_result_cases_do_not_cross_wire_sibling_or_prior_actuals() {
    for (name, body, expected) in [
        (
            "left_selected",
            "let local: Pair = identity_pair(Pair { left: Choice::Selected { view: View { body: &mut self.value } }, right: Choice::Empty {} }); write_outer(Outer { inner: local.left.view });",
            Some(vec!["self.value"]),
        ),
        (
            "right_selected",
            "let local: Pair = identity_pair(Pair { left: Choice::Empty {}, right: Choice::Selected { view: View { body: &mut self.other } } }); write_outer(Outer { inner: local.right.view });",
            Some(vec!["self.other"]),
        ),
        (
            "sibling_absent",
            "let local: Pair = identity_pair(Pair { left: Choice::Selected { view: View { body: &mut self.value } }, right: Choice::Empty {} }); write_outer(Outer { inner: local.right.view });",
            None,
        ),
        (
            "selected_after_empty",
            "let first: Choice = identity(Choice::Empty {}); let local: Choice = identity(Choice::Selected { view: View { body: &mut self.value } }); write_outer(Outer { inner: local.view });",
            Some(vec!["self.value"]),
        ),
        (
            "empty_after_selected",
            "let first: Choice = identity(Choice::Selected { view: View { body: &mut self.value } }); let local: Choice = identity(Choice::Empty {}); write_outer(Outer { inner: local.view });",
            None,
        ),
        (
            "unknown",
            "let local: Choice = identity(input); write_outer(Outer { inner: local.view });",
            None,
        ),
        (
            "unproved_body_projection",
            "let local: View = project_unproved(Choice::Selected { view: View { body: &mut self.value } }); write_outer(Outer { inner: local });",
            None,
        ),
    ] {
        let expected = expected.map(|paths| paths.into_iter().map(str::to_owned).collect());
        assert_eq!(
            caller_frames(&case_result_program(body)),
            [expected.clone(), expected],
            "{name}"
        );
    }
}

#[test]
fn helper_result_cases_exist_without_exclusive_reference_leaves() {
    for (name, body, complete) in [
        (
            "owned_selected",
            "let local: PlainChoice = identity_plain(PlainChoice::Selected { view: Plain { tag: 0 } }); write_plain(PlainOuter { inner: local.view });",
            true,
        ),
        (
            "owned_absent",
            "let local: PlainChoice = identity_plain(PlainChoice::Empty {}); write_plain(PlainOuter { inner: local.view });",
            false,
        ),
        (
            "shared_selected",
            "let local: SharedChoice = identity_shared(SharedChoice::Selected { view: Shared { body: &self.value, tag: 0 } }); write_shared(SharedOuter { inner: local.view });",
            true,
        ),
        (
            "shared_absent",
            "let local: SharedChoice = identity_shared(SharedChoice::Empty {}); write_shared(SharedOuter { inner: local.view });",
            false,
        ),
    ] {
        let expected = complete.then(Vec::new);
        assert_eq!(
            caller_frames(&case_result_program(body)),
            [expected.clone(), expected],
            "{name}"
        );
    }
}

#[test]
fn helper_result_constructors_do_not_inherit_unrelated_input_cases() {
    for (name, body, expected) in [
        (
            "constructor_empty",
            "let local: Choice = replace_empty(Choice::Selected { view: View { body: &mut self.value } }); write_outer(Outer { inner: local.view });",
            None,
        ),
        (
            "constructor_selected",
            "let local: Choice = replace_selected(Choice::Empty {}, &mut self.other); write_outer(Outer { inner: local.view });",
            Some(vec!["self.other"]),
        ),
        (
            "private_result",
            "let local: Choice = private_result(Choice::Selected { view: View { body: &mut self.value } }); write_outer(Outer { inner: local.view });",
            None,
        ),
        (
            "private_sibling",
            "let local: Pair = private_pair(Choice::Selected { view: View { body: &mut self.value } }); write_outer(Outer { inner: local.left.view });",
            None,
        ),
    ] {
        let expected = expected.map(|paths| paths.into_iter().map(str::to_owned).collect());
        assert_eq!(
            caller_frames(&case_result_program(body)),
            [expected.clone(), expected],
            "{name}"
        );
    }
}

#[test]
fn helper_case_substitution_keeps_producer_writes_separate() {
    let program = case_result_program(
        "let local: Choice = effectful(Choice::Selected { view: View { body: &mut self.value } }, &mut self.audit); write_outer(Outer { inner: local.view });",
    );
    assert_eq!(
        caller_frames(&program),
        [
            Some(vec!["self.audit".to_owned(), "self.value".to_owned()]),
            Some(vec!["self.value".to_owned()]),
        ],
    );
}

#[test]
fn helper_case_substitution_requires_frozen_owned_and_shared_inputs() {
    let helpers = r#"
        machine clear_plain(value: &mut PlainChoice) { value = PlainChoice::Empty {}; }
        machine clear_shared(value: &mut SharedChoice) { value = SharedChoice::Empty {}; }
        machine PlainChoice::clear(&mut self) { self = PlainChoice::Empty {}; }
        machine SharedChoice::clear(&mut self) { self = SharedChoice::Empty {}; }
        machine replace_plain(mut input: PlainChoice) -> PlainChoice { input = PlainChoice::Empty {}; input }
        machine replace_shared(mut input: SharedChoice) -> SharedChoice { input = SharedChoice::Empty {}; input }
        machine expose_plain(mut input: PlainChoice) -> PlainChoice { clear_plain(&mut input); input }
        machine expose_shared(mut input: SharedChoice) -> SharedChoice { clear_shared(&mut input); input }
        machine receive_plain(mut input: PlainChoice) -> PlainChoice { input.clear(); input }
        machine receive_shared(mut input: SharedChoice) -> SharedChoice { input.clear(); input }
    "#;
    for (name, body) in [
        (
            "owned_replacement",
            "let local: PlainChoice = replace_plain(PlainChoice::Selected { view: Plain { tag: 0 } }); write_plain(PlainOuter { inner: local.view });",
        ),
        (
            "shared_replacement",
            "let local: SharedChoice = replace_shared(SharedChoice::Selected { view: Shared { body: &self.value, tag: 0 } }); write_shared(SharedOuter { inner: local.view });",
        ),
        (
            "owned_exposure",
            "let local: PlainChoice = expose_plain(PlainChoice::Selected { view: Plain { tag: 0 } }); write_plain(PlainOuter { inner: local.view });",
        ),
        (
            "shared_exposure",
            "let local: SharedChoice = expose_shared(SharedChoice::Selected { view: Shared { body: &self.value, tag: 0 } }); write_shared(SharedOuter { inner: local.view });",
        ),
        (
            "owned_receiver",
            "let local: PlainChoice = receive_plain(PlainChoice::Selected { view: Plain { tag: 0 } }); write_plain(PlainOuter { inner: local.view });",
        ),
        (
            "shared_receiver",
            "let local: SharedChoice = receive_shared(SharedChoice::Selected { view: Shared { body: &self.value, tag: 0 } }); write_shared(SharedOuter { inner: local.view });",
        ),
    ] {
        assert_eq!(
            caller_frames(&case_result_program_with_helpers(body, helpers)),
            [None, None],
            "{name}",
        );
    }
}

#[test]
fn helper_case_substitution_allows_writes_that_preserve_case_storage() {
    let helpers = r#"
        data Envelope { choice: PlainChoice; tag: u64; }
        machine change_tag(mut input: Envelope) -> Envelope { input.tag = 1; input }
        machine change_selected() -> PlainChoice {
            let mut local: PlainChoice = PlainChoice::Selected { view: Plain { tag: 0 } };
            local.view.tag = 1;
            local
        }
        machine change_plain(mut input: PlainOuter) -> PlainOuter { input.inner.tag = 1; input }
    "#;
    for (name, body) in [
        (
            "case_free_sibling",
            "let local: Envelope = change_tag(Envelope { choice: PlainChoice::Selected { view: Plain { tag: 0 } }, tag: 0 }); write_plain(PlainOuter { inner: local.choice.view });",
        ),
        (
            "selected_owned_payload",
            "let local: PlainChoice = change_selected(); write_plain(PlainOuter { inner: local.view });",
        ),
        (
            "case_free_owned_payload",
            "let local: PlainOuter = change_plain(PlainOuter { inner: Plain { tag: 0 } }); write_plain(local);",
        ),
    ] {
        assert_eq!(
            caller_frames(&case_result_program_with_helpers(body, helpers)),
            [Some(Vec::new()), Some(Vec::new())],
            "{name}",
        );
    }
}

#[test]
fn helper_case_moves_require_exact_input_source_identity() {
    let original = case_result_program_with_helpers(
        "let local: Choice = identity(Choice::Selected { view: View { body: &mut self.value } }); write_outer(Outer { inner: local.view });",
        "machine foreign(input: Choice) -> Choice { input }",
    );
    let helper = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "identity")
        .expect("helper");
    let state = &original.machine_states(helper)[0];
    let StatementNode::Expression(expression) = original
        .statement_table
        .statements(state.statement_nodes)
        .last()
        .expect("result")
    else {
        panic!("result");
    };
    let expression = *expression;
    let exact = original.state_parameters(state)[0].symbol;
    let foreign = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "foreign")
        .expect("foreign");
    let foreign = original.state_parameters(&original.machine_states(foreign)[0])[0].symbol;
    for (name, symbol, complete) in [
        ("exact", exact, true),
        ("foreign", foreign, false),
        ("missing", psi_symbols::SymbolHandle::invalid(), false),
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
        path.symbol = symbol;
        path.head_symbol = symbol;
        let member_symbols = path.member_symbols;
        program
            .expression_table
            .set_name_path_member_symbol_at_offset(member_symbols, 0, symbol);
        for (query, paths) in caller_frames(&program).into_iter().enumerate() {
            assert_eq!(paths.is_some(), complete, "{name} query {query}");
        }
    }
}

#[test]
fn helper_case_moves_require_live_and_compatible_selected_target() {
    let original = case_result_program(
        "let local: Choice = identity(Choice::Selected { view: View { body: &mut self.value } }); write_outer(Outer { inner: local.view });",
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
        .find(|machine| machine.name.as_str() == "identity_plain")
        .expect("wrong nominal producer");
    let foreign = original.machine_states(foreign)[0].symbol;
    for (name, target, complete) in [
        ("exact", exact, true),
        ("wrong_nominal", foreign, false),
        ("missing", psi_symbols::SymbolHandle::invalid(), false),
        (
            "stale",
            psi_symbols::SymbolHandle::from_parts(exact.arena_index(), exact.generation() + 1),
            false,
        ),
    ] {
        let mut program = original.clone();
        let ExpressionNode::Call(call) = program.expression_table.expression_mut(expression) else {
            panic!("producer");
        };
        call.target_symbol = target;
        for (query, paths) in caller_frames(&program).into_iter().enumerate() {
            assert_eq!(paths.is_some(), complete, "{name} query {query}");
        }
    }
}

#[test]
fn named_actual_cases_remove_absent_declared_reference_rows() {
    let helpers = r#"
        machine consume(input: Choice) {}
        machine consume_array(input: [Choice; 2]) {}
    "#;
    for (name, body, expected) in [
        (
            "named_empty_whole",
            "let source: Choice = Choice::Empty {}; let local: Choice = identity(source); consume(local);",
            Some(vec![]),
        ),
        (
            "named_empty_direct_consumer",
            "let source: Choice = Choice::Empty {}; consume(identity(source));",
            Some(vec![]),
        ),
        (
            "named_mixed_selected",
            "let source: [Choice; 2] = [Choice::Selected { view: View { body: &mut self.value } }, Choice::Empty {}]; let local: [Choice; 2] = identity_array(source); write_outer(Outer { inner: local[0].view });",
            Some(vec!["self.value"]),
        ),
        (
            "named_mixed_absent",
            "let source: [Choice; 2] = [Choice::Selected { view: View { body: &mut self.value } }, Choice::Empty {}]; let local: [Choice; 2] = identity_array(source); write_outer(Outer { inner: local[1].view });",
            None,
        ),
        (
            "named_mixed_runtime",
            "let source: [Choice; 2] = [Choice::Selected { view: View { body: &mut self.value } }, Choice::Empty {}]; let local: [Choice; 2] = identity_array(source); write_outer(Outer { inner: local[index].view });",
            None,
        ),
        (
            "named_all_empty_whole",
            "let source: [Choice; 2] = [Choice::Empty {}, Choice::Empty {}]; let local: [Choice; 2] = identity_array(source); consume_array(local);",
            Some(vec![]),
        ),
        (
            "named_all_empty_absent",
            "let source: [Choice; 2] = [Choice::Empty {}, Choice::Empty {}]; let local: [Choice; 2] = identity_array(source); write_outer(Outer { inner: local[0].view });",
            None,
        ),
    ] {
        let expected = expected.map(|paths| paths.into_iter().map(str::to_owned).collect());
        assert_eq!(
            caller_frames(&case_result_program_with_helpers(body, helpers)),
            [expected.clone(), expected],
            "{name}",
        );
    }
}
