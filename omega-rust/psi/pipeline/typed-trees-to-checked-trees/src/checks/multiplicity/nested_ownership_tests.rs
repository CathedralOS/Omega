//! Source ownership of whole ordinary affine call operands. These assertions
//! inspect permissions independently of the Terminal Unit planner.

use super::*;

fn check(source: &str) -> Result<checked_trees::CheckedTrees, Vec<Diagnostic>> {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type");
    crate::lower_typed_trees(typed)
}

fn caller_events(checked: &checked_trees::CheckedTrees) -> Vec<&FlowPermissionEventFact> {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::caller")
        .expect("caller");
    checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter_map(|(_, event)| (event.machine_symbol == machine.symbol).then_some(event))
        .collect()
}

fn assert_claim_free_transfer(event: &FlowPermissionEventFact) {
    assert_eq!(event.kind, PermissionEventKind::Transfer);
    assert_eq!(event.multiplicity, Multiplicity::Affine);
    assert_eq!(event.access, PermissionAccess::Owned);
    assert_eq!(event.claim_identity, PermissionClaimIdentity::Unknown);
    assert!(!event.obligation_live);
    assert!(event.segments.is_empty());
}

#[test]
fn nested_owned_affine_operands_transfer_once_in_captured_call_order() {
    for (operand, expected_ordinals) in [
        ("value", vec![0]),
        ("forward(value)", vec![1, 0]),
        ("forward(forward(value))", vec![2, 1, 0]),
    ] {
        let checked = check(&format!(
            "data Value {{ number: u64; }}
             data Main {{}}
             machine forward(value: Value) -> Value {{ value }}
             machine Main::consume(value: Value) {{}}
             machine Main::caller(value: Value) {{ Main::consume({operand}); }}"
        ))
        .expect("nested owned call checks");
        let events = caller_events(&checked);
        assert_eq!(events.len(), expected_ordinals.len(), "{events:#?}");
        let state = checked
            .facts
            .flow
            .control
            .states
            .iter()
            .find_map(|(_, state)| (state.state_symbol == events[0].state_symbol).then_some(state))
            .expect("caller flow");
        let calls = checked.facts.flow.control.calls.span_or_empty(state.calls);
        assert_eq!(calls.len(), events.len());
        for ((event, call), ordinal) in events.iter().zip(calls).zip(expected_ordinals) {
            assert_claim_free_transfer(event);
            assert_eq!(call.call_ordinal, ordinal);
            assert_eq!(
                event.source,
                PermissionEventSource::Call {
                    statement_index: 0,
                    call_ordinal: ordinal,
                    target_symbol: call.target_symbol,
                }
            );
        }
        let caller = crate::find_state(&checked, state.state_symbol).expect("caller state");
        let value = checked
            .state_parameters(caller)
            .iter()
            .find(|parameter| !parameter.is_self)
            .expect("owned caller parameter");
        assert_eq!(events[0].root, facts::PlaceRoot::Symbol(value.symbol));
        for (event, producer) in events.iter().skip(1).zip(calls) {
            assert!(producer.authored_expression.is_valid());
            assert_eq!(
                event.root,
                facts::PlaceRoot::Expression(producer.authored_expression)
            );
        }
        assert!(
            events
                .iter()
                .all(|event| event.kind != PermissionEventKind::AffineDrop)
        );
    }
}

#[test]
fn nested_owned_affine_arguments_cannot_transfer_a_source_twice() {
    for body in [
        "Main::consume_pair(forward(value), forward(value));",
        "Main::consume(forward(value)); Main::consume(value);",
    ] {
        let diagnostics = check(&format!(
            "data Value {{ number: u64; }}
             data Main {{}}
             machine forward(value: Value) -> Value {{ value }}
             machine Main::consume(value: Value) {{}}
             machine Main::consume_pair(first: Value, second: Value) {{}}
             machine Main::caller(value: Value) {{ {body} }}"
        ))
        .expect_err("a nested call consumes its owned argument");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("already transferred or consumed")),
            "{diagnostics:#?}"
        );
    }
}

#[test]
fn call_initialized_affine_local_establishes_before_nested_transfer() {
    let checked = check(
        "data Value { number: u64; }
         data Main {}
         machine forward(value: Value) -> Value { value }
         machine Main::consume(value: Value) {}
         machine Main::caller(value: Value) {
             let result: Value = forward(value);
             Main::consume(forward(result));
         }",
    )
    .expect("local result feeds a nested call");
    let events = caller_events(&checked);
    assert_eq!(events.len(), 4, "{events:#?}");
    assert_claim_free_transfer(events[0]);
    assert_eq!(events[1].kind, PermissionEventKind::Establish);
    assert_eq!(
        events[1].source,
        PermissionEventSource::Statement { statement_index: 0 }
    );
    assert_eq!(events[1].claim_identity, PermissionClaimIdentity::Unknown);
    assert!(!events[1].obligation_live);
    assert_eq!(events[2].root, events[1].root);
    assert_claim_free_transfer(events[2]);
    assert_claim_free_transfer(events[3]);
    assert!(matches!(events[3].root, facts::PlaceRoot::Expression(_)));
}

#[test]
fn nested_borrowed_results_do_not_create_owned_transfers() {
    let checked = check(
        "data Value { number: u64; }
         data Main {}
         machine forward(value: &Value) -> &Value { value }
         machine Main::consume(value: &Value) {}
         machine Main::caller(value: &Value) { Main::consume(forward(value)); }",
    )
    .expect("nested borrowed result checks");
    assert!(caller_events(&checked).iter().all(|event| {
        event.access != PermissionAccess::Owned
            || !matches!(
                event.kind,
                PermissionEventKind::Transfer | PermissionEventKind::Consume
            )
    }));
}

#[test]
fn proof_calls_read_claim_free_affine_operands_without_transferring_them() {
    let checked = check(
        "data Nat { case Zero; case Succ(previous: Nat); }
         data Value { number: u64; }
         machine proof_value(natural: Nat, value: Value) -> i32 [0..=7] { 7 }
         machine proof_twice(natural: Nat, value: Value) -> i32 {
             proof_value(natural, value) + proof_value(natural, value)
         }",
    )
    .expect("proof applications can read the same affine operands twice");
    assert!(
        checked
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .all(|(_, event)| {
                !matches!(event.source, PermissionEventSource::Call { .. })
                    || event.access != PermissionAccess::Owned
                    || !matches!(
                        event.kind,
                        PermissionEventKind::Transfer | PermissionEventKind::Consume
                    )
            })
    );
}

#[test]
fn affine_parameter_replacement_settles_only_the_displaced_value() {
    for (replacement, first_kind) in [
        ("Value { number: 2 }", PermissionEventKind::AffineDrop),
        ("forward(value)", PermissionEventKind::Transfer),
    ] {
        let checked = check(&format!(
            "data Value {{ number: u64; }}
             data Main {{}}
             machine forward(value: Value) -> Value {{ value }}
             machine Main::consume(value: Value) {{}}
             machine Main::caller(mut value: Value) {{
                 value = {replacement};
                 Main::consume(value);
             }}"
        ))
        .expect("an affine parameter can be replaced and moved again");
        let events = caller_events(&checked);
        assert_eq!(events.len(), 3, "{events:#?}");
        assert_eq!(events[0].kind, first_kind);
        assert_eq!(events[1].kind, PermissionEventKind::Establish);
        assert_eq!(
            events[1].source,
            PermissionEventSource::Statement { statement_index: 0 }
        );
        assert_claim_free_transfer(events[2]);
        assert!(events.iter().all(|event| {
            event.root == events[0].root
                && event.multiplicity == Multiplicity::Affine
                && event.claim_identity == PermissionClaimIdentity::Unknown
                && !event.obligation_live
        }));
    }
}

#[test]
fn initialized_affine_destinations_cannot_be_consumed_twice() {
    for (declaration, carrier, initializer) in [
        (
            "data Category { case Other; case Missing; }",
            "Category",
            "Category::Missing",
        ),
        (
            "data Value { number: u64; }",
            "Value",
            "Value { number: code }",
        ),
        (
            "data Category { case Other; case Missing; }",
            "[Category; 1]",
            "[Category::Missing]",
        ),
        (
            "data Category { case Other; case Missing; }",
            "Category",
            "match code { 2 -> Category::Missing, _ -> Category::Other }",
        ),
    ] {
        for mutable in ["", "mut "] {
            let source = format!(
                "{declaration} data Main {{}}
                 machine Main::consume(value: {carrier}) {{}}
                 machine Main::caller(code: u64) {{
                     let {mutable}value: {carrier} = {initializer};
                     Main::consume(value);
                     Main::consume(value);
                 }}"
            );
            let diagnostics = match check(&source) {
                Ok(_) => panic!("one initialized value cannot transfer twice: {source}"),
                Err(diagnostics) => diagnostics,
            };
            assert!(
                diagnostics.iter().any(|diagnostic| diagnostic
                    .message
                    .contains("already transferred or consumed")),
                "{source}: {diagnostics:#?}"
            );
            let single = source.replacen("Main::consume(value);", "", 1);
            check(&single).unwrap_or_else(|diagnostics| panic!("{single}: {diagnostics:#?}"));
        }
    }
}

#[test]
fn initialized_mutable_affine_local_replacement_has_one_ordered_lifetime() {
    for (before_replace, expected) in [
        (
            "",
            vec![
                PermissionEventKind::Establish,
                PermissionEventKind::AffineDrop,
                PermissionEventKind::Establish,
                PermissionEventKind::Transfer,
            ],
        ),
        (
            "Main::consume(value);",
            vec![
                PermissionEventKind::Establish,
                PermissionEventKind::Transfer,
                PermissionEventKind::Establish,
                PermissionEventKind::Transfer,
            ],
        ),
    ] {
        let source = format!(
            "data Category {{ case Other; case Missing; }} data Main {{}}
             machine Main::consume(value: Category) {{}}
             machine Main::caller() {{
                 let mut value: Category = Category::Missing;
                 {before_replace}
                 value = Category::Other;
                 Main::consume(value);
             }}"
        );
        let checked =
            check(&source).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
        let events = caller_events(&checked);
        assert_eq!(
            events.iter().map(|event| event.kind).collect::<Vec<_>>(),
            expected,
            "{events:#?}"
        );
        assert!(events.iter().all(|event| event.root == events[0].root
            && event.segments.is_empty()
            && event.multiplicity == Multiplicity::Affine
            && event.access == PermissionAccess::Owned
            && event.claim_identity == PermissionClaimIdentity::Unknown
            && !event.obligation_live));
        let replacement_statement = if before_replace.is_empty() { 1 } else { 2 };
        assert_eq!(
            events[2].source,
            PermissionEventSource::Statement {
                statement_index: replacement_statement,
            }
        );
        assert_claim_free_transfer(events[3]);
    }
}
