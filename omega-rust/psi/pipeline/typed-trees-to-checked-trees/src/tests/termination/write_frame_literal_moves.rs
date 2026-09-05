use super::*;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;

fn literal_move_program(body: &str, scalar: &str) -> typed_trees::TypedTrees {
    let unknown_machine = if body.contains("write_unknown") {
        "machine write_unknown(mut outer: Outer) { outer.inner.absent = 1; }"
    } else {
        ""
    };
    let source = format!(
        r#"
        data View {{ body: &mut {scalar}; tag: u64; }}
        data OtherView {{ body: &mut {scalar}; tag: u64; }}
        data Outer {{ inner: View; }}
        data Pair {{ left: View; right: View; }}
        data Choice {{ case Selected(view: View); case Empty; }}
        data OuterChoice {{ inner: Choice; }}
        data Main {{ value: {scalar}; other: {scalar}; audit: {scalar}; }}
        machine write_outer(mut outer: Outer) {{ outer.inner.body = 255; }}
        machine write_pair(mut pair: Pair) {{ pair.left.body = 255; pair.right.body = 255; }}
        machine write_array(mut views: [View; 2]) {{ views[0].body = 255; }}
        machine write_choice(mut choice: Choice) {{ choice.view.body = 255; }}
        machine write_outer_choice(mut outer: OuterChoice) {{ outer.inner.view.body = 255; }}
        machine write_owned(mut outer: Outer) {{ outer.inner.tag = 1; }}
        machine write_outer_value(mut outer: Outer) -> u64 {{ outer.inner.body = 255; 0 }}
        {unknown_machine}
        machine audited<'value, 'audit>(value: &'value mut {scalar}, audit: &'audit mut {scalar}) -> &'value mut {scalar} {{ audit = 1; value }}
        machine Main::run(&mut self, mut input: View, outer: Outer, values: [View; 2], choice: Choice, index: u64) {{ {body} }}
        machine foreign(input: View) {{ let local: View = input; }}
    "#
    );
    let syntax =
        parse_syntax_trees(&Lexer::new(&source).tokenize().expect("tokenize")).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

#[test]
fn immediate_literal_moves_preserve_complete_caller_reference_frames() {
    let cases = [
        (
            "record",
            "let local: View = View { body: &mut self.value }; write_outer(Outer { inner: local });",
            Some(vec!["self.value"]),
        ),
        (
            "explicit_move",
            "let local: View = View { body: &mut self.value }; write_outer(Outer { inner: move local });",
            Some(vec!["self.value"]),
        ),
        (
            "array",
            "let local: View = View { body: &mut self.value }; let second: View = View { body: &mut self.other }; write_array([local, second]);",
            Some(vec!["self.other", "self.value"]),
        ),
        (
            "selected_case",
            "let local: View = View { body: &mut self.value }; write_choice(Choice::Selected { view: local });",
            Some(vec!["self.value"]),
        ),
        (
            "local_field",
            "let local: Outer = Outer { inner: View { body: &mut self.value } }; write_outer(Outer { inner: local.inner });",
            Some(vec!["self.value"]),
        ),
        (
            "local_fixed_element",
            "let local: [View; 2] = [View { body: &mut self.value }, View { body: &mut self.other }]; write_outer(Outer { inner: local[0] });",
            Some(vec!["self.other", "self.value"]),
        ),
        (
            "local_runtime_element",
            "let local: [View; 2] = [View { body: &mut self.value }, View { body: &mut self.other }]; write_outer(Outer { inner: local[index] });",
            Some(vec!["self.other", "self.value"]),
        ),
        (
            "frozen_alias",
            "let mut alias: &mut u64 = &mut self.value; let local: View = View { body: alias }; alias = &mut self.other; write_outer(Outer { inner: local });",
            Some(vec!["self.value"]),
        ),
        (
            "owned_field",
            "let local: View = View { body: &mut self.value }; write_owned(Outer { inner: local });",
            Some(vec![]),
        ),
        (
            "empty_whole_case",
            "let local: Choice = Choice::Empty {}; write_outer_choice(OuterChoice { inner: local });",
            Some(vec![]),
        ),
        (
            "unknown_field_demand",
            "let local: View = View { body: &mut self.value }; write_unknown(Outer { inner: local });",
            None,
        ),
        (
            "parameter",
            "write_outer(Outer { inner: input });",
            Some(vec!["input.body"]),
        ),
        (
            "parameter_field",
            "write_outer(Outer { inner: outer.inner });",
            Some(vec!["outer.inner.body"]),
        ),
        (
            "parameter_fixed_element",
            "write_outer(Outer { inner: values[0] });",
            Some(vec!["values"]),
        ),
        (
            "parameter_runtime_element",
            "write_outer(Outer { inner: values[index] });",
            Some(vec!["values"]),
        ),
        (
            "parameter_whole_case",
            "write_outer_choice(OuterChoice { inner: choice });",
            Some(vec!["choice.view.body"]),
        ),
        (
            "missing_source",
            "write_outer(Outer { inner: absent });",
            None,
        ),
        (
            "missing_leaf",
            "let local: View = View {}; write_outer(Outer { inner: local });",
            None,
        ),
        (
            "opaque_leaf",
            "let local: View = View { body: unknown(&mut self.value) }; write_outer(Outer { inner: local });",
            None,
        ),
        (
            "wrong_nominal",
            "let local: OtherView = OtherView { body: &mut self.value }; write_outer(Outer { inner: local });",
            None,
        ),
        (
            "absent_payload",
            "let local: Choice = Choice::Empty {}; write_outer(Outer { inner: local.view });",
            None,
        ),
        (
            "selected_payload_with_prefix_evidence",
            "let local: Choice = Choice::Selected { view: View { body: &mut self.value } }; write_outer(Outer { inner: local.view });",
            Some(vec!["self.value"]),
        ),
        (
            "unknown_parameter_payload",
            "write_outer(Outer { inner: choice.view });",
            None,
        ),
        (
            "parameter_slot_replacement",
            "input.body = &mut self.other; write_outer(Outer { inner: input });",
            None,
        ),
        (
            "parameter_carrier_borrow",
            "unknown(&mut input); write_outer(Outer { inner: input });",
            None,
        ),
        (
            "local_slot_replacement",
            "let mut local: View = View { body: &mut self.value }; local.body = &mut self.other; write_outer(Outer { inner: local });",
            None,
        ),
        (
            "unknown_sibling",
            "let local: View = View { body: &mut self.value }; write_array([local, absent]);",
            None,
        ),
        (
            "out_of_bounds",
            "write_outer(Outer { inner: values[2] });",
            None,
        ),
        (
            "negative_index",
            "write_outer(Outer { inner: values[-1] });",
            None,
        ),
    ];
    let mut failures = Vec::new();
    for (name, body, expected) in cases {
        let program = literal_move_program(body, "u64");
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
                let mut visible = Vec::new();
                for mut path in paths {
                    for (index, parameter) in program
                        .state_parameters(state)
                        .iter()
                        .filter(|parameter| !parameter.is_self)
                        .enumerate()
                    {
                        if let Some(suffix) = path.strip_prefix(&format!("$P{index}")) {
                            path = format!("{}{suffix}", parameter.name.as_str());
                            break;
                        }
                    }
                    let root = path.split('.').next().expect("root");
                    if root == "self"
                        || program
                            .state_parameters(state)
                            .iter()
                            .any(|parameter| parameter.name.as_str() == root)
                    {
                        visible.push(path);
                    }
                }
                visible.sort();
                visible.dedup();
                visible
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
fn immediate_literal_moves_keep_sibling_producer_writes_separate() {
    let program = literal_move_program(
        "let local: View = View { body: &mut self.value }; write_pair(Pair { left: local, right: View { body: audited(&mut self.other, &mut self.audit) } });",
        "u64",
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
    for (query, paths, expected) in [
        (
            "state",
            resolver
                .inferred_state_write_frame(machine, state)
                .into_complete_paths(),
            vec!["self.audit", "self.other", "self.value"],
        ),
        (
            "callee",
            resolver.may_write_paths(machine, call),
            vec!["self.other", "self.value"],
        ),
    ] {
        let mut actual: Vec<_> = paths
            .expect("complete frame")
            .into_iter()
            .filter(|path| path == "self" || path.starts_with("self."))
            .collect();
        actual.sort();
        actual.dedup();
        assert_eq!(actual, expected, "{query}");
    }
}

#[test]
fn immediate_literal_moves_preserve_expression_call_frames() {
    let program = literal_move_program(
        "let local: View = View { body: &mut self.value }; let result: u64 = write_outer_value(Outer { inner: local });",
        "u64",
    );
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
    let resolver = validation::CallFrameResolver::new(&program).expect("resolver");
    for paths in [
        resolver
            .inferred_state_write_frame(machine, state)
            .into_complete_paths(),
        resolver.expression_may_write_paths(machine, result.initial_value),
    ] {
        let mut visible = paths
            .expect("complete expression frame")
            .into_iter()
            .filter(|path| path == "self" || path.starts_with("self."))
            .collect::<Vec<_>>();
        visible.sort();
        visible.dedup();
        assert_eq!(visible, ["self.value"]);
    }
    lower_typed_trees(program).expect("expression literal move preserves its access route");
}

#[test]
fn immediate_literal_moves_reach_checked_trees() {
    for body in [
        "let local: View = View { body: &mut self.value }; write_outer(Outer { inner: local });",
        "let local: View = View { body: &mut self.value }; let second: View = View { body: &mut self.other }; write_array([local, second]);",
        "write_outer(Outer { inner: input });",
        "write_outer(Outer { inner: outer.inner });",
    ] {
        lower_typed_trees(literal_move_program(body, "u64"))
            .expect("literal move reaches checked trees");
    }
}

#[test]
fn immediate_literal_move_sources_require_exact_declaration_identity() {
    for (body, parameter_source) in [
        (
            "let local: View = View { body: &mut self.value }; write_outer(Outer { inner: local });",
            false,
        ),
        ("write_outer(Outer { inner: input });", true),
    ] {
        let original = literal_move_program(body, "u64");
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
        let expression = original.expression_table.struct_fields(literal.fields)[0].value;
        let ExpressionNode::Name(name) = original.expression_table.expression(expression) else {
            panic!("source");
        };
        let exact = name.symbol;
        let foreign = original
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "foreign")
            .expect("foreign");
        let foreign_state = &original.machine_states(foreign)[0];
        let foreign = if parameter_source {
            original.state_parameters(foreign_state)[0].symbol
        } else {
            let StatementNode::LocalData(local) = &original
                .statement_table
                .statements(foreign_state.statement_nodes)[0]
            else {
                panic!("foreign local");
            };
            local.symbol
        };
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
            let ExpressionNode::Name(path) = program.expression_table.expression_mut(expression)
            else {
                panic!("source");
            };
            path.symbol = symbol;
            path.head_symbol = symbol;
            let member_symbols = path.member_symbols;
            program
                .expression_table
                .set_name_path_member_symbol_at_offset(member_symbols, 0, symbol);
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
            assert_eq!(
                resolver
                    .inferred_state_write_frame(machine, state)
                    .is_complete(),
                complete,
                "{parameter_source} {name}: state"
            );
            assert_eq!(
                resolver.may_write_frame(machine, call).is_complete(),
                complete,
                "{parameter_source} {name}: public"
            );
        }
    }
}

#[test]
fn immediate_literal_moves_invalidate_owner_and_alias_arithmetic_facts() {
    for (name, body) in [
        (
            "owner",
            "self.value = 0; let local: View = View { body: &mut self.value }; write_outer(Outer { inner: local }); self.value = self.value + 1;",
        ),
        (
            "alias",
            "let mut local: View = View { body: &mut self.value }; local.body = 0; write_outer(Outer { inner: local }); self.value = local.body + 1;",
        ),
        (
            "parameter",
            "input.body = 0; write_outer(Outer { inner: input }); input.body = input.body + 1;",
        ),
    ] {
        let program = literal_move_program(body, "u8");
        match validation::validate_program(&program) {
            Err(diagnostics)
                if diagnostics.iter().any(|diagnostic| {
                    let message = diagnostic.to_string();
                    message.contains("Main::run") && message.contains("may overflow")
                }) => {}
            result => panic!("{name}: literal move retained stale fact: {result:?}"),
        }
    }
}

#[test]
fn immediate_literal_parameter_move_preserves_named_state_cycle_frame() {
    let source = r#"
        data View { body: &mut u64; }
        data Outer { inner: View; }
        machine write_outer(mut outer: Outer) { outer.inner.body = 1; }
        machine literal_cycle(value: View) {
            transition { _ -> cycle(value) }
            state cycle(item: View) {
                write_outer(Outer { inner: item });
                transition { _ -> cycle(item) }
            }
        }
    "#;
    let syntax =
        parse_syntax_trees(&Lexer::new(source).tokenize().expect("tokenize")).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let program = lower_symbol_resolved_trees(&resolved).expect("type");
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "literal_cycle")
        .expect("machine");
    let entry = &program.machine_states(machine)[0];
    let resolver = validation::CallFrameResolver::new(&program).expect("resolver");
    for _ in 0..2 {
        assert_eq!(
            resolver
                .inferred_state_write_frame(machine, entry)
                .complete_paths(),
            Some(["$P0.body".to_owned()].as_slice())
        );
    }
}

#[test]
fn immediate_literal_move_composes_owned_suffix_below_reference_leaf() {
    let source = r#"
        data Cell { value: u64; other: u64; }
        data View { body: &mut Cell; }
        data Outer { inner: View; }
        data Main { cell: Cell; }
        machine write_outer(mut outer: Outer) { outer.inner.body.value = 1; }
        machine Main::run(&mut self) {
            let local: View = View { body: &mut self.cell };
            write_outer(Outer { inner: local });
        }
    "#;
    let syntax =
        parse_syntax_trees(&Lexer::new(source).tokenize().expect("tokenize")).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let program = lower_symbol_resolved_trees(&resolved).expect("type");
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
    for paths in [
        resolver
            .inferred_state_write_frame(machine, state)
            .into_complete_paths(),
        resolver.may_write_paths(machine, call),
    ] {
        let paths: Vec<_> = paths
            .expect("complete suffix transport")
            .into_iter()
            .filter(|path| path == "self" || path.starts_with("self."))
            .collect();
        assert_eq!(paths, vec!["self.cell.value"]);
    }
    lower_typed_trees(program).expect("owned reference suffix reaches checked trees");
}

#[test]
fn immediate_literal_moves_preserve_complete_empty_owned_and_shared_frames() {
    for (name, source) in [
        (
            "zero_length_array",
            r#"
            data Outer { inner: [u64; 0]; }
            machine write_outer(mut outer: Outer) { outer.inner = []; }
            machine probe() {
                let local: [u64; 0] = [];
                write_outer(Outer { inner: local });
            }
            "#,
        ),
        (
            "shared_only_subrecord",
            r#"
            data SharedView { body: &u64; tag: u64; }
            data SharedOuter { inner: SharedView; }
            data Outer { inner: SharedView; }
            data Main { value: u64; }
            machine write_outer(mut outer: Outer) { outer.inner.tag = 1; }
            machine Main::probe(&self) {
                let local: SharedOuter = SharedOuter {
                    inner: SharedView { body: &self.value, tag: 0 }
                };
                write_outer(Outer { inner: local.inner });
            }
            "#,
        ),
    ] {
        let syntax =
            parse_syntax_trees(&Lexer::new(source).tokenize().expect("tokenize")).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let program = lower_symbol_resolved_trees(&resolved).expect("type");
        let machine = program
            .machines()
            .iter()
            .find(|machine| matches!(machine.name.as_str(), "probe" | "Main::probe"))
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
        for (query, paths) in [
            (
                "state",
                resolver
                    .inferred_state_write_frame(machine, state)
                    .into_complete_paths(),
            ),
            ("public call", resolver.may_write_paths(machine, call)),
        ] {
            assert_eq!(paths, Some(Vec::new()), "{name} {query}");
        }
        lower_typed_trees(program).expect("zero-leaf literal move reaches checked trees");
    }
}

#[test]
fn immediate_literal_moves_reject_unknown_suffix_below_reference_leaf() {
    let source = r#"
        data Cell { value: u64; }
        data View { body: &mut Cell; }
        data Outer { inner: View; }
        data Main { cell: Cell; }
        machine write_outer(mut outer: Outer) { outer.inner.body.absent = 1; }
        machine Main::run(&mut self) {
            let local: View = View { body: &mut self.cell };
            write_outer(Outer { inner: local });
        }
    "#;
    let syntax =
        parse_syntax_trees(&Lexer::new(source).tokenize().expect("tokenize")).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let program = lower_symbol_resolved_trees(&resolved).expect("type");
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
        "unknown reference suffix must not supply a complete state frame"
    );
    assert!(
        !resolver.may_write_frame(machine, call).is_complete(),
        "unknown reference suffix must not supply a complete public frame"
    );
}

#[test]
fn immediate_literal_moves_preserve_acyclic_named_transition_origins() {
    let source = r#"
        data View { body: &mut u64; }
        data Outer { inner: View; }
        data Main { value: u64; }
        machine Main::run(&mut self) {
            let local: View = View { body: &mut self.value };
            transition { _ -> finish(Outer { inner: local }) }
            state finish(&mut self, mut outer: Outer) {
                outer.inner.body = 1;
            }
        }
    "#;
    let syntax =
        parse_syntax_trees(&Lexer::new(source).tokenize().expect("tokenize")).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let program = lower_symbol_resolved_trees(&resolved).expect("type");
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::run")
        .expect("caller");
    let state = &program.machine_states(machine)[0];
    let resolver = validation::CallFrameResolver::new(&program).expect("resolver");
    for _ in 0..2 {
        assert_eq!(
            resolver
                .inferred_state_write_frame(machine, state)
                .complete_paths(),
            Some(["self.value".to_owned()].as_slice()),
            "transition literal must expand its moved local before private-root filtering"
        );
    }
    lower_typed_trees(program).expect("transition literal move reaches checked trees");
}
