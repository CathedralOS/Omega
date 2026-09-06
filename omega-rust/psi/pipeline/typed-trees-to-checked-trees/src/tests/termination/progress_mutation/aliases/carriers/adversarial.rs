use super::*;

fn assert_readonly_carrier_has_no_subject(operation: &str) {
    let program = fixture_with_body(
        &format!(
            "let mut carrier: Carrier = Carrier {{ context: &context }};
             {operation}
             let borrowed: &Context = carrier.context;
             transition {{ _ -> wait_context(borrowed) }}"
        ),
        true,
        false,
        "data Carrier { context: &Context; }
         machine identity(context: &Context) -> &Context { context }
         machine inspect_carrier(carrier: &mut Carrier) {}
         machine inspect_context(context: &mut Context) {}
         machine Carrier::touch(&mut self) -> u64 { 0 }
         machine forward<'selected, 'binding>(
             selected: &'selected Context,
             binding: &'binding mut Context
         ) -> &'selected Context { selected }",
    );
    let plan = program
        .facts
        .termination
        .for_machine(symbol_of_checked(&program, "replace"))
        .unwrap();
    assert_eq!(
        plan.checked_summary,
        TerminationGuarantee::NoGuarantee,
        "a frozen reference origin cannot survive carrier replacement or exposure: {operation}"
    );
}

#[test]
fn replacing_a_readonly_carrier_or_reference_leaf_retires_its_frozen_origin() {
    for operation in [
        "carrier = Carrier { context: &replacement };",
        "carrier.context = &replacement;",
        "carrier.context = identity(replacement);",
    ] {
        assert_readonly_carrier_has_no_subject(operation);
    }
}

#[test]
fn explicit_mutable_exposure_retires_readonly_carrier_origins() {
    for operation in [
        "inspect_carrier(&mut carrier);",
        "inspect_context(&mut carrier.context);",
        "let ignored: &Context = forward(replacement, &mut carrier.context);",
    ] {
        // Empty helper frames and an independent selected return cannot
        // establish that an exclusively exposed reference slot stayed frozen.
        assert_readonly_carrier_has_no_subject(operation);
    }
}

#[test]
fn an_implicit_mutable_statement_receiver_retires_readonly_carrier_origins() {
    assert_readonly_carrier_has_no_subject("carrier.touch();");
}

#[test]
fn an_implicit_mutable_value_receiver_retires_readonly_carrier_origins() {
    assert_receiver_origin(
        "",
        "let ignored: u64 = carrier.touch();",
        "machine Carrier::touch(&mut self) -> u64 { 0 }",
        false,
    );
}

#[test]
fn an_implicit_mutable_reference_receiver_preserves_a_disjoint_subject() {
    for operation in [
        "carrier.context.increment_counter();",
        "let ignored: u64 = carrier.context.increment_counter();",
    ] {
        assert_receiver_origin(
            "mut ",
            operation,
            "machine Context::increment_counter(&mut self) -> u64 {
                 self.counter = 1;
                 0
             }",
            true,
        );
    }
}

// Value calls on LET-bound receivers have a separate native-realization fence.
// Exercise the prefix query before that fence; an implicit receiver at the
// reference leaf borrows its referent, whereas an ancestor exposes the slot.
fn assert_receiver_origin(access: &str, operation: &str, helper: &str, exact: bool) {
    use typed_trees::statement::StatementNode;

    let source = fixture_source(
        &format!(
            "let mut carrier: Carrier = Carrier {{ context: &{access}context }};
             {operation}
             let borrowed: &{access}Context = carrier.context;
             transition {{ _ -> 0 }}"
        ),
        true,
        false,
        &format!("data Carrier {{ context: &{access}Context; }} {helper}"),
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
    let origin = validation::CallFrameResolver::new(&program)
        .unwrap()
        .local_reference_origin_before_statement(machine, statements.last().unwrap(), borrowed);
    assert_eq!(origin, exact.then_some((context, vec![])), "{operation}");
}

#[test]
fn a_projected_nested_carrier_copy_keeps_its_reference_before_alias_rebinding() {
    let program = fixture_with_body(
        "let mut selected: &Context = &context;
         let carrier: Carrier = Carrier { context: selected };
         let outer: Outer = Outer { inner: carrier };
         let saved: Carrier = outer.inner;
         selected = &replacement;
         let borrowed: &Context = saved.context;
         transition { _ -> wait_context(borrowed) }",
        true,
        false,
        "data Carrier { context: &Context; }
         data Outer { inner: Carrier; }",
    );
    assert_subjects(&program, &["context"]);
}

#[test]
fn a_disjoint_store_through_a_loaded_mutable_carrier_reference_keeps_its_subject() {
    let program = fixture_with_body(
        "let carrier: Carrier = Carrier { context: &mut context };
         let borrowed: &mut Context = carrier.context;
         borrowed.counter = 1;
         transition { _ -> wait_context(borrowed) }",
        true,
        false,
        "data Carrier { context: &mut Context; }",
    );
    assert_subjects(&program, &["context"]);
}

#[test]
fn a_loaded_mutable_carrier_reference_tracks_its_live_replacement_subject() {
    let program = fixture_with_body(
        "let carrier: Carrier = Carrier { context: &mut context };
         let borrowed: &mut Context = carrier.context;
         borrowed.scheduler = replacement.scheduler;
         transition { _ -> wait_context(borrowed) }",
        false,
        false,
        "data Carrier { context: &mut Context; }",
    );
    assert_subjects(&program, &["replacement"]);
}
