use super::*;

#[test]
fn direct_frames_close_over_current_aliases_without_redirecting_prior_aliases() {
    use psi_typed_trees::statement::StatementNode;
    let source = r#"
    data Cell { value: u64; }
    data Main { first: Cell; second: Cell; }
    machine Cell::write(&mut self) { self.value = 9; }
    machine Cell::read_after_write(&mut self) -> u64 { self.value = 9; 0 }
    machine Main::run(&mut self) {
        let mut receiver: &mut Cell = &mut self.first;
        let prior: &mut Cell = receiver;
        receiver = &mut self.second;
        prior.write();
        receiver.write();
        self.first.write();
        let result: u64 = prior.read_after_write();
    }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("lower typed trees");
    let resolver = psi_validation::CallFrameResolver::new(&typed).expect("symbol cache");
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::run")
        .expect("caller");
    let state = typed.machine_states(machine).first().expect("entry");
    let statements = typed.statement_table.statements(state.statement_nodes);
    let calls: Vec<_> = statements
        .iter()
        .filter_map(|statement| match statement {
            StatementNode::Call(call) => Some(call),
            _ => None,
        })
        .collect();
    assert_eq!(calls.len(), 3);
    for (call, expected) in calls.into_iter().zip([
        ["prior.value", "self.first.value"],
        ["receiver.value", "self.second.value"],
        ["prior.value", "self.first.value"],
    ]) {
        let mut actual = resolver
            .may_write_paths(machine, call)
            .expect("complete direct frame");
        actual.sort();
        assert_eq!(actual, expected);
    }
    let statement = statements.last().expect("value initializer");
    let StatementNode::LocalData(local) = statement else {
        panic!("local initializer")
    };
    for mut paths in [
        resolver
            .statement_value_may_write_paths(machine, statement)
            .expect("statement value frame"),
        resolver
            .expression_may_write_paths(machine, local.initial_value)
            .expect("expression frame"),
    ] {
        paths.sort();
        assert_eq!(paths, ["prior.value", "self.first.value"]);
    }
}

#[test]
fn direct_alias_stores_invalidate_arithmetic_facts_in_both_spellings() {
    let cases = [
        (
            "scalar alias to owner",
            "self.value = 0;
             let alias: &mut u8 = &mut self.value;
             alias = 255;
             self.value = self.value + 1;",
        ),
        (
            "owner to scalar alias",
            "let alias: &mut u8 = &mut self.value;
             alias = 0;
             self.value = 255;
             self.value = alias + 1;",
        ),
        (
            "record alias to owner",
            "self.value = 0;
             let receiver: &mut Main = &mut self;
             receiver.value = 255;
             self.value = self.value + 1;",
        ),
        (
            "owner to record alias",
            "let receiver: &mut Main = &mut self;
             receiver.value = 0;
             self.value = 255;
             self.value = receiver.value + 1;",
        ),
    ];
    let mut missing_overflow = Vec::new();
    for (name, body) in cases {
        let source = format!(
            "data Main {{ value: u8; }}
             machine Main::run(&mut self) {{ {body} }}"
        );
        let tokens = Lexer::new(&source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("lower typed trees");
        match psi_validation::validate_program(&typed) {
            Err(diagnostics)
                if diagnostics.iter().any(|diagnostic| {
                    let message = diagnostic.to_string();
                    message.contains("Main::run") && message.contains("may overflow")
                }) => {}
            result => missing_overflow.push(format!("{name}: {result:?}")),
        }
    }
    assert!(
        missing_overflow.is_empty(),
        "stale alias facts must not prove u8 overflow safety: {missing_overflow:?}"
    );
}

#[test]
fn local_receiver_calls_invalidate_arithmetic_facts_on_caller_storage() {
    for call in [
        "receiver.write();",
        "let result: u64 = receiver.write_value();",
    ] {
        let source = format!(
            "data Main {{ value: u8; }}
            machine Main::write(&mut self) {{ self.value = 255; }}
            machine Main::write_value(&mut self) -> u64 {{ self.value = 255; 0 }}
            machine Main::run(&mut self) {{
                self.value = 0;
                let receiver: &mut Main = &mut self;
                {call}
                self.value = self.value + 1;
            }}"
        );
        let tokens = Lexer::new(&source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("lower typed trees");
        let diagnostics = psi_validation::validate_program(&typed)
            .expect_err("stale zero cannot prove overflow safety");
        assert!(
            diagnostics.iter().any(|diagnostic| {
                let message = diagnostic.to_string();
                message.contains("Main::run") && message.contains("may overflow")
            }),
            "{call}: {diagnostics:?}"
        );
    }
}

#[test]
fn local_receiver_references_retain_caller_writes_and_returned_places() {
    let cases = [
        (
            "statement",
            "let receiver: &mut Cell = &mut self.first; receiver.write(compute(&mut self.audit));",
            vec!["self.audit", "self.first.value"],
        ),
        (
            "returned_place",
            "let receiver: &mut Cell = &mut self.first; let result: &mut u64 = receiver.select(compute(&mut self.audit)); result = 2;",
            vec!["self.audit", "self.first.value"],
        ),
        (
            "replaced_receiver",
            "let mut receiver: &mut Cell = &mut self.first; let prior: &mut Cell = receiver; receiver = &mut self.second; prior.write(compute(&mut self.audit)); receiver.write(0);",
            vec!["self.audit", "self.first.value", "self.second.value"],
        ),
        (
            "indexed_receiver",
            "let receiver: &mut Cell = &mut self.cells[0]; receiver.write(compute(&mut self.audit));",
            vec!["self.audit", "self.cells"],
        ),
        (
            "indexed_returned_place",
            "let receiver: &mut Cell = &mut self.cells[0]; let result: &mut u64 = receiver.select(compute(&mut self.audit)); result = 2;",
            vec!["self.audit", "self.cells"],
        ),
    ];
    for (name, body, expected) in cases {
        let source = format!(
            "data Cell {{ value: u64; }}
            data Main {{ first: Cell; second: Cell; cells: [Cell; 2]; audit: u64; }}
            machine compute(value: &mut u64) -> u64 {{ value = 1; 1 }}
            machine Cell::write(&mut self, value: u64) {{ self.value = value; }}
            machine Cell::select(&mut self, value: u64) -> &mut u64 {{ self.value = value; &mut self.value }}
            machine Main::run(&mut self) {{ {body} }}"
        );
        let tokens = Lexer::new(&source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("lower typed trees");
        let resolver = psi_validation::CallFrameResolver::new(&typed).expect("symbol cache");
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::run")
            .expect("caller");
        let state = typed.machine_states(machine).first().expect("entry");
        let frame = resolver.inferred_state_write_frame(machine, state);
        let mut actual = frame
            .complete_paths()
            .unwrap_or_else(|| panic!("{name} must be complete"))
            .to_vec();
        actual.sort();
        assert_eq!(actual, expected, "{name}");
    }
}

#[test]
fn computed_attached_arguments_exclude_the_receiver_parameter() {
    let source = r#"
    data Main { value: u64; audit: u64; }
    machine compute(value: &mut u64) -> u64 { value = 1; 1 }
    machine Main::consume(&self, count: u64) {}
    machine Main::identity(&self, count: u64) -> u64 { count }
    machine Main::after_statement(&mut self) -> &mut u64 {
        self.consume(compute(&mut self.audit) + 1);
        &mut self.value
    }
    machine Main::after_expression(&mut self) -> &mut u64 {
        self.value = self.identity(compute(&mut self.audit) + 1);
        &mut self.value
    }
    machine Main::statement(&mut self) {
        let alias: &mut u64 = self.after_statement();
        alias = 2;
    }
    machine Main::expression(&mut self) {
        let alias: &mut u64 = self.after_expression();
        alias = 2;
    }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("lower typed trees");
    let resolver = psi_validation::CallFrameResolver::new(&typed).expect("symbol cache");
    for name in ["Main::statement", "Main::expression"] {
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .expect("caller");
        let state = typed.machine_states(machine).first().expect("entry");
        let frame = resolver.inferred_state_write_frame(machine, state);
        let mut actual = frame
            .complete_paths()
            .unwrap_or_else(|| panic!("{name} must be complete"))
            .to_vec();
        actual.sort();
        assert_eq!(actual, ["self.audit", "self.value"]);
    }
}

#[test]
fn computed_argument_siblings_keep_indexed_borrow_origins() {
    let template = r#"
    data Main { value: u64; cells: [u64; 2]; audit: u64; other: u64; }
    machine index(value: &mut u64) -> u64 [0..=1] { value = 1; 0 }
    machine compute(value: &mut u64) -> u64 { value = 1; 1 }
    machine recursive(value: &mut u64) -> u64 { recursive(value) }
    machine consume(value: &mut u64, count: u64) { value = count; }
    machine after<'value, 'cells, 'audit, 'other>(
        value: &'value mut u64, cells: &'cells mut [u64; 2],
        audit: &'audit mut u64, other: &'other mut u64
    ) -> &'value mut u64 {
        consume(&mut cells[$INDEX], $VALUE);
        value
    }
    machine Main::run(&mut self) {
        let alias: &mut u64 = after(&mut self.value, &mut self.cells, &mut self.audit, &mut self.other);
        alias = 2;
    }
    "#;
    for (index, value, complete) in [
        ("index(audit)", "compute(other) + 1", true),
        ("index(&mut audit)", "compute(other) + 1", false),
        ("index(audit)", "recursive(other) + 1", false),
    ] {
        let source = template.replace("$INDEX", index).replace("$VALUE", value);
        let tokens = Lexer::new(&source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("lower typed trees");
        let resolver = psi_validation::CallFrameResolver::new(&typed).expect("symbol cache");
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::run")
            .expect("caller");
        let state = typed.machine_states(machine).first().expect("entry");
        let frame = resolver.inferred_state_write_frame(machine, state);
        if complete {
            let mut actual = frame
                .complete_paths()
                .expect("complete mixed arguments")
                .to_vec();
            actual.sort();
            assert_eq!(
                actual,
                ["self.audit", "self.cells", "self.other", "self.value"],
                "the indexed argument must retain its collection and both sibling writes"
            );
        } else {
            assert!(
                !frame.is_complete(),
                "hostile argument must remain opaque: {index}, {value}"
            );
        }
    }
}

#[test]
fn computed_call_arguments_preserve_every_write_and_reject_hostile_siblings() {
    let alternating = (0..16).fold("compute(audit)".to_owned(), |expression, _| {
        format!("identity({expression} + 1)")
    });
    let cases = [
        (
            "scalar",
            "consume_value(~(compute(audit) + 1));",
            true,
            false,
        ),
        (
            "record",
            "consume_record(Pair { first: compute(audit) + 1, second: compute(other) });",
            true,
            true,
        ),
        (
            "array",
            "consume_array([compute(audit) + 1, compute(other) + 1]);",
            true,
            true,
        ),
        (
            "selected_case",
            "consume_choice(Choice::Filled { pair: Pair { first: compute(audit) + 1, second: compute(other) } });",
            true,
            true,
        ),
        (
            "reference_sibling",
            "consume_reference(return_value(audit), identity(compute(other) + 1));",
            true,
            true,
        ),
        (
            "external_initializer",
            "let scratch: u64 = identity(compute(audit) + 1);",
            false,
            false,
        ),
        (
            "local_attached_statement",
            "let pair: Pair = Pair { first: 0, second: 0 }; pair.consume(compute(audit) + 1);",
            true,
            false,
        ),
        (
            "local_attached_expression",
            "let pair: Pair = Pair { first: 0, second: 0 }; value = pair.identity(compute(audit) + 1);",
            true,
            false,
        ),
        (
            "local_mutating_receiver",
            "let mut pair: Pair = Pair { first: 0, second: 0 }; pair.overwrite(compute(audit) + 1);",
            true,
            false,
        ),
        (
            "local_reference_receiver",
            "let mut pair: Pair = Pair { first: 0, second: 0 }; let receiver: &mut Pair = &mut pair; receiver.overwrite(compute(audit) + 1);",
            true,
            false,
        ),
        (
            "assignment_call",
            "value = identity(compute(audit) + 1);",
            true,
            false,
        ),
        (
            "private_initializer",
            "let mut prior: u64 = 0; let scratch: u64 = identity(compute(&mut prior) + 1); consume_value(compute(audit) + 1);",
            true,
            false,
        ),
        (
            "projected_array",
            "consume_value([compute(audit), compute(other)][0]);",
            true,
            true,
        ),
        (
            "effectful_projection",
            "consume_value([compute(audit), compute(other)][index(audit)]);",
            true,
            true,
        ),
        (
            "projected_record",
            "consume_value(Pair { first: compute(audit), second: compute(other) }.first);",
            true,
            true,
        ),
        (
            "recursive",
            "consume_record(Pair { first: compute(audit) + 1, second: recursive(other) + 1 });",
            false,
            false,
        ),
        (
            "reborrow",
            "consume_array([compute(audit) + 1, compute(&mut other) + 1]);",
            false,
            false,
        ),
        (
            "reference_computation",
            "consume_value(return_value(audit) + 1);",
            false,
            false,
        ),
        (
            "reference_record",
            "consume_reference_record(ReferenceRecord { value: return_value(audit), count: compute(other) + 1 });",
            false,
            false,
        ),
        (
            "reference_projection",
            "consume_value(make_reference_record(audit).count + 1);",
            false,
            false,
        ),
        (
            "generic_record",
            "consume_generic(GenericPair { first: compute(audit) + 1, second: compute(other) });",
            false,
            false,
        ),
        (
            "wrong_nominal",
            "consume_record(OtherPair { first: compute(audit) + 1, second: compute(other) });",
            false,
            false,
        ),
        (
            "wrong_length",
            "consume_array([compute(audit) + 1]);",
            false,
            false,
        ),
    ];
    let mut source = r#"
    data Main { value: u64; audit: u64; other: u64; }
    data Pair { first: u64; second: u64; }
    data OtherPair { first: u64; second: u64; }
    data GenericPair<T> { first: T; second: u64; }
    data Choice { case Filled(pair: Pair); case Empty; }
    data ReferenceRecord { value: &mut u64; count: u64; }
    machine compute(value: &mut u64) -> u64 { value = 1; 1 }
    machine index(value: &mut u64) -> u64 [0..=1] { value = 1; 0 }
    machine recursive(value: &mut u64) -> u64 { recursive(value) }
    machine identity(value: u64) -> u64 { value }
    machine return_value(value: &mut u64) -> &mut u64 { value }
    machine consume_value(value: u64) {}
    machine consume_record(value: Pair) {}
    machine consume_choice(value: Choice) {}
    machine consume_array(value: [u64; 2]) {}
    machine consume_reference(value: &mut u64, count: u64) { value = count; }
    machine consume_reference_record(value: ReferenceRecord) {}
    machine consume_generic(value: GenericPair<u64>) {}
    machine Pair::consume(&self, value: u64) {}
    machine Pair::identity(&self, value: u64) -> u64 { value }
    machine Pair::overwrite(&mut self, value: u64) { self.first = value; }
    machine make_reference_record(value: &mut u64) -> ReferenceRecord {
        ReferenceRecord { value: value, count: 0 }
    }
    "#
    .to_owned();
    let nested_statement = format!("consume_value({alternating});");
    let all_cases =
        cases
            .into_iter()
            .chain([("alternating", nested_statement.as_str(), true, false)]);
    for (name, statement, _, _) in all_cases.clone() {
        source.push_str(&format!(
            "machine after_{name}<'value, 'audit, 'other>(
                value: &'value mut u64, audit: &'audit mut u64, other: &'other mut u64
            ) -> &'value mut u64 {{
                {statement}
                value
            }}
            machine Main::{name}(&mut self) {{
                let alias: &mut u64 = after_{name}(&mut self.value, &mut self.audit, &mut self.other);
                alias = 2;
            }}"
        ));
    }
    let tokens = Lexer::new(&source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    // The negative cases deliberately include malformed value contexts. Frame
    // inference must not claim completeness before validation rejects them.
    let typed = lower_symbol_resolved_trees(&resolved).expect("lower typed trees");
    let resolver = psi_validation::CallFrameResolver::new(&typed).expect("symbol cache");
    for (name, _, complete, writes_other) in all_cases {
        let qualified_name = format!("Main::{name}");
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == qualified_name)
            .expect("caller");
        let state = typed.machine_states(machine).first().expect("entry");
        let frame = resolver.inferred_state_write_frame(machine, state);
        if complete {
            let mut expected = vec!["self.audit".to_owned(), "self.value".to_owned()];
            if writes_other {
                expected.insert(1, "self.other".to_owned());
            }
            let mut actual = frame
                .complete_paths()
                .unwrap_or_else(|| panic!("{name} must be complete"))
                .to_vec();
            actual.sort();
            assert_eq!(actual, expected, "{name} must retain all argument writes");
        } else {
            assert!(!frame.is_complete(), "{name} must remain opaque");
        }
    }
}
