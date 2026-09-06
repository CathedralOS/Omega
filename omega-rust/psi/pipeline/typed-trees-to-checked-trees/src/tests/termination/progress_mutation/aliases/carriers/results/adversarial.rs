use super::*;

fn assert_no_subject(program: &checked_trees::CheckedTrees, operation: &str) {
    let plan = program
        .facts
        .termination
        .for_machine(symbol_of_checked(program, "replace"))
        .unwrap();
    assert_eq!(
        plan.checked_summary,
        TerminationGuarantee::NoGuarantee,
        "a shared result cannot export a stale input subject: {operation}"
    );
}

fn assert_incoming_carrier_has_no_subject(operation: &str) {
    let program = fixture_with_body(
        "let original: Carrier = Carrier { context: &context, other: &replacement };
         let returned: Carrier = forward(original);
         let borrowed: &Context = returned.context;
         transition { _ -> wait_context(borrowed) }",
        true,
        false,
        &format!(
            "data Carrier<'source> {{
                 context: &'source Context;
                 other: &'source Context;
             }}
             machine inspect_context(context: &mut Context) -> u64 {{ 0 }}
             machine Carrier::touch(&mut self) -> u64 {{ 0 }}
             machine forward<'source>(mut input: Carrier<'source>) -> Carrier<'source> {{
                 {operation}
                 input
             }}"
        ),
    );
    assert_no_subject(&program, operation);
}

#[test]
fn replacing_an_incoming_shared_slot_cannot_export_its_original_subject() {
    assert_incoming_carrier_has_no_subject("input.context = input.other;");
}

#[test]
fn replacing_an_incoming_shared_record_cannot_export_its_original_subject() {
    assert_incoming_carrier_has_no_subject(
        "input = Carrier { context: input.other, other: input.other };",
    );
}

#[test]
fn explicit_mutable_exposure_of_an_incoming_readonly_slot_retires_result_identity() {
    assert_incoming_carrier_has_no_subject("_ = inspect_context(&mut input.context);");
}

#[test]
fn an_incoming_mutable_ancestor_receiver_retires_shared_result_identity() {
    assert_incoming_carrier_has_no_subject("input.touch();");
}

#[test]
fn a_terminal_unused_operand_cannot_expose_a_shared_parameter_binding() {
    let program = fixture_with_body(
        "let returned: Carrier = forward(context);
         let borrowed: &Context = returned.context;
         transition { _ -> wait_context(borrowed) }",
        true,
        false,
        "data Carrier { context: &Context; tag: u64; }
         machine inspect_binding(binding: &mut Context) -> u64 { 0 }
         machine forward(mut context: &Context) -> Carrier {
             Carrier { context: context, tag: inspect_binding(&mut context) }
         }",
    );
    // The unused scalar operand has an empty write frame, but exposes the
    // binding that anchors the other field's returned-reference relation.
    assert_no_subject(&program, "terminal sibling exposes the shared parameter");
}

#[test]
fn a_helper_local_carrier_copy_keeps_its_source_after_alias_rebinding() {
    let program = fixture_with_body(
        "let returned: Carrier = forward(context, replacement);
         let borrowed: &Context = returned.context;
         transition { _ -> wait_context(borrowed) }",
        true,
        false,
        "data Carrier<'context> { context: &'context Context; }
         machine forward<'context, 'replacement>(
             context: &'context Context,
             replacement: &'replacement Context
         ) -> Carrier<'context> {
             let mut borrowed: &Context = context;
             let carrier: Carrier<'context> = Carrier { context: borrowed };
             let saved: Carrier<'context> = carrier;
             borrowed = replacement;
             saved
         }",
    );
    assert_subjects(&program, &["context"]);
}

#[test]
fn a_shared_aggregate_result_keeps_its_subject_after_disjoint_producer_writes() {
    let program = fixture_with_body(
        "let returned: Carrier = forward(context);
         let borrowed: &Context = returned.context;
         transition { _ -> wait_context(borrowed) }",
        true,
        false,
        "data Carrier { context: &Context; }
         machine forward(context: &mut Context) -> Carrier {
             context.counter = 1;
             Carrier { context: &context }
         }",
    );
    assert_subjects(&program, &["context"]);
}

#[test]
fn a_shared_aggregate_result_cannot_restore_a_subject_after_overlapping_producer_writes() {
    let program = fixture_with_body(
        "let returned: Carrier = forward(context, replacement);
         let borrowed: &Context = returned.context;
         transition { _ -> wait_context(borrowed) }",
        true,
        false,
        "data Carrier<'context> { context: &'context Context; }
         machine forward<'context, 'replacement>(
             context: &'context mut Context,
             replacement: &'replacement Context
         ) -> Carrier<'context>
         requires replacement.scheduler in WeakFair
         ensures context.scheduler in WeakFair
         terminates;
         {
             context.scheduler = replacement.scheduler;
             Carrier { context: &context }
         }",
    );
    // The returned reference and restored qualification do not identify the
    // scheduler value installed by the producer's may-write frame.
    assert_no_subject(&program, "producer replaces the scheduler");
}

#[test]
fn a_helper_local_mutable_value_receiver_retires_shared_result_identity() {
    use typed_trees::statement::StatementNode;

    for (operation, exact) in [("", true), ("let ignored: u64 = carrier.touch();", false)] {
        // LET-bound value receivers have a separate native-realization fence.
        // Query typed identity directly, with an unexposed positive control.
        let source = fixture_source(
            "let returned: Carrier = forward(context);
             let borrowed: &Context = returned.context;
             transition { _ -> 0 }",
            true,
            false,
            &format!(
                "data Carrier {{ context: &Context; }}
                 machine Carrier::touch(&mut self) -> u64 {{ 0 }}
                 machine forward(context: &Context) -> Carrier {{
                     let mut carrier: Carrier = Carrier {{ context: context }};
                     {operation}
                     carrier
                 }}"
            ),
        );
        let tokens = Lexer::new(&source).tokenize().unwrap();
        let syntax = parse_syntax_trees(&tokens).unwrap();
        let resolved = lower_syntax_trees(&syntax).unwrap();
        let program = lower_symbol_resolved_trees(&resolved).unwrap();
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "replace")
            .unwrap();
        let state = &program.machine_states(machine)[0];
        let statements = program.statement_table.statements(state.statement_nodes);
        let borrowed = statements
            .iter()
            .find_map(|statement| match statement {
                StatementNode::LocalData(local) if local.name.as_str() == "borrowed" => {
                    Some(local.symbol)
                }
                _ => None,
            })
            .unwrap();
        let context = program.state_parameters(state)[0].symbol;
        let resolver = validation::CallFrameResolver::new(&program).unwrap();
        let frame = resolver.inferred_state_write_frame(machine, state);
        let origin = resolver.local_reference_origin_before_statement(
            machine,
            statements.last().unwrap(),
            borrowed,
        );
        assert_eq!(origin, exact.then_some((context, vec![])), "{operation}");
        assert_eq!(
            resolver.inferred_state_write_frame(machine, state),
            frame,
            "shared result discovery cannot change ordinary write permissions"
        );
    }
}
