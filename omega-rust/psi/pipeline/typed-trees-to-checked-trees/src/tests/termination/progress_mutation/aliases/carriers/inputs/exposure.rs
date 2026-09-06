use super::*;

pub(super) fn typed_source(source: &str) -> typed_trees::TypedTrees {
    let tokens = Lexer::new(source).tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).unwrap();
    let resolved = lower_syntax_trees(&syntax).unwrap();
    lower_symbol_resolved_trees(&resolved).unwrap()
}

fn reject_exposed_input(access: &str, preceding: &str, operand: &str, extra: &str) {
    let source = fixture_source(
        access,
        &format!(
            "let borrowed: &{access}Context = carrier.context;
             {preceding}
             transition {{ _ -> waiting({operand}, borrowed) }}
             state waiting(ignored: u64, selected: &Context) -> u64
             requires selected.scheduler in WeakFair
             {{ wait_context(selected) }}"
        ),
        extra,
    );
    let Err(diagnostics) = lower_typed_trees(typed_source(&source)) else {
        panic!("exposure cannot preserve an input reference's requires evidence");
    };
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove requires contract for call waiting")),
        "{preceding} / {operand}: {diagnostics:#?}"
    );
}

#[test]
fn preceding_explicit_readonly_slot_exposure_rejects_requires() {
    reject_exposed_input(
        "",
        "_ = inspect_context(&mut carrier.context);",
        "0",
        "machine inspect_context(context: &mut Context) -> u64 { 0 }",
    );
}

#[test]
fn earlier_operand_explicit_readonly_slot_exposure_rejects_requires() {
    // An empty frame is not evidence that an exposed reference slot is frozen.
    reject_exposed_input(
        "",
        "",
        "inspect_context(&mut carrier.context)",
        "machine inspect_context(context: &mut Context) -> u64 { 0 }",
    );
}

#[test]
fn preceding_empty_mutable_ancestor_method_rejects_requires() {
    for access in ["", "mut "] {
        reject_exposed_input(
            access,
            "carrier.touch();",
            "0",
            "machine Carrier::touch(&mut self) -> u64 { 0 }",
        );
    }
}

#[test]
fn earlier_operand_empty_mutable_ancestor_method_rejects_requires() {
    for access in ["", "mut "] {
        reject_exposed_input(
            access,
            "",
            "carrier.touch()",
            "machine Carrier::touch(&mut self) -> u64 { 0 }",
        );
    }
}

#[test]
fn a_disjoint_referent_method_preserves_the_input_subject() {
    assert_disjoint_referent_method("carrier.context.increment_counter();", "0");
}

#[test]
fn an_operand_referent_method_preserves_the_input_subject() {
    assert_disjoint_referent_method("", "carrier.context.increment_counter()");
}

fn assert_disjoint_referent_method(preceding: &str, operand: &str) {
    let source = referent_method_source(preceding, operand, "self.counter = 1;");
    // The endpoint receiver borrows the referent. Its counter write neither
    // replaces the containing reference slot nor overlaps the scheduler.
    let checked = check_source(&source);
    assert_input_subject(&checked);
}

fn referent_method_source(preceding: &str, operand: &str, mutation: &str) -> String {
    fixture_source(
        "mut ",
        &format!(
            "{preceding}
                 transition {{ _ -> waiting({operand}, carrier.context) }}
                 state waiting(ignored: u64, selected: &Context) -> u64
                 requires selected.scheduler in WeakFair
                 {{ wait_context(selected) }}"
        ),
        &format!(
            "machine Context::increment_counter(&mut self) terminates; -> u64 {{
                 {mutation}
                 0
             }}"
        ),
    )
}

#[test]
fn a_referent_scheduler_write_retires_the_input_qualification() {
    for (preceding, operand) in [
        ("carrier.context.increment_counter();", "0"),
        ("", "carrier.context.increment_counter()"),
    ] {
        let source =
            referent_method_source(preceding, operand, "self.scheduler = SchedulerHandle {};");
        let Err(diagnostics) = lower_typed_trees(typed_source(&source)) else {
            panic!("{preceding} / {operand}: a replaced scheduler has no qualification");
        };
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("cannot prove requires contract for call waiting")),
            "{diagnostics:#?}"
        );
    }
}

#[test]
fn a_referent_method_does_not_replace_a_saved_reference_binding() {
    for (preceding, operand) in [
        ("carrier.context.increment_counter();", "0"),
        ("", "carrier.context.increment_counter()"),
    ] {
        let source = fixture_source(
            "mut ",
            &format!(
                "let borrowed: &mut Context = carrier.context;
                 {preceding}
                 transition {{ _ -> waiting({operand}, borrowed) }}
                 state waiting(ignored: u64, selected: &Context) -> u64
                 requires selected.scheduler in WeakFair
                 {{ wait_context(selected) }}"
            ),
            "machine Context::increment_counter(&mut self) terminates; -> u64 {
                 self.counter = 1; 0
             }",
        );
        let mut program = typed_source(&source);
        crate::lookup::resolve_projected_receiver_calls(&mut program).unwrap();
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "replace")
            .unwrap();
        let state = &program.machine_states(machine)[0];
        let statements = program.statement_table.statements(state.statement_nodes);
        let typed_trees::statement::StatementNode::LocalData(borrowed) = &statements[0] else {
            panic!("reference local")
        };
        let resolver = validation::CallFrameResolver::new(&program).unwrap();
        let (root, segments) = resolver
            .local_reference_origin_before_statement(
                machine,
                statements.last().unwrap(),
                borrowed.symbol,
            )
            .expect("referent writes do not replace the reference slot");
        assert_eq!(root, program.state_parameters(state)[0].symbol);
        let [facts::PlaceSegment::Field { symbol }] = segments.as_slice() else {
            panic!("input reference field: {segments:?}")
        };
        assert_eq!(
            program.symbols.display_path(*symbol, "::"),
            "Carrier::context"
        );
        // Reference identity is not permission to mutate through the parent
        // while the saved exclusive loan remains live.
        let Err(diagnostics) = lower_typed_trees(program) else {
            panic!("{preceding} / {operand}: a live exclusive loan must reject mutation");
        };
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains("while local borrow `borrowed` is still active")
            }),
            "{diagnostics:#?}"
        );
    }
}

#[test]
fn later_exposure_does_not_change_an_earlier_input_origin_query() {
    use typed_trees::statement::StatementNode;

    for exposure in [
        "_ = inspect_context(&mut carrier.context);",
        "carrier.touch();",
    ] {
        let program = typed_source(&fixture_source(
            "",
            &format!(
                "let borrowed: &Context = carrier.context;
                 let observed: u64 = 0;
                 {exposure}
                 transition {{ _ -> 0 }}"
            ),
            "machine inspect_context(context: &mut Context) -> u64 { 0 }
             machine Carrier::touch(&mut self) -> u64 { 0 }",
        ));
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "replace")
            .unwrap();
        let state = &program.machine_states(machine)[0];
        let statements = program.statement_table.statements(state.statement_nodes);
        let StatementNode::LocalData(borrowed) = &statements[0] else {
            panic!("reference declaration")
        };
        let carrier = program.state_parameters(state)[0].symbol;
        let definition = program
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "Carrier")
            .unwrap();
        let context = program
            .data_members(definition)
            .iter()
            .find_map(|member| match member {
                typed_trees::data::DataMember::Field(field) if field.name.as_str() == "context" => {
                    Some(field.symbol)
                }
                _ => None,
            })
            .unwrap();
        let expected = Some((
            carrier,
            vec![facts::PlaceSegment::Field { symbol: context }],
        ));
        let resolver = validation::CallFrameResolver::new(&program).unwrap();
        let origin_before = |statement| {
            resolver.local_reference_origin_before_statement(machine, statement, borrowed.symbol)
        };
        assert_eq!(origin_before(&statements[1]), expected, "{exposure}");
        assert_eq!(
            origin_before(statements.last().unwrap()),
            None,
            "{exposure}"
        );
        assert_eq!(origin_before(&statements[1]), expected, "{exposure}");
    }
}
