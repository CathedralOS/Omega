use super::{constant, evaluate, literal_encoding, parse};

#[test]
fn computed_nominal_leaves_reject_invalid_unused_fields_and_constructor_shapes() {
    for initializer in [
        "Config { count: 200 + 100, enabled: true }",
        "Config { count: 7 / 2, enabled: true }",
        "Config { count: 2 + 1, enabled: 2 }",
        "Config { count: 2 + 1 }",
        "Config { count: 2 + 1, count: 2, enabled: true }",
        "Other { count: 2 + 1, enabled: true }",
    ] {
        assert!(
            evaluate(&format!(
                "data Config [copy] {{ count: u8; enabled: bool; }}
            data Other [copy] {{ count: u8; enabled: bool; }}
            const UNUSED: Config = {initializer};"
            ))
            .is_err(),
            "{initializer}"
        );
    }
}

#[test]
fn computed_record_leaves_preserve_payloadless_case_siblings() {
    let declarations = "data Mode [copy] { case On; case Off; }
        data Config [copy] { mode: Mode; count: u64; }";
    let evaluated = evaluate(&format!(
        "{declarations}
        const UNUSED: Mode = Mode::Off;
        const CONFIG: Config = Config {{ mode: Mode::On, count: 7 / 2 * 2 }};"
    ))
    .expect("case literal is not a constant dependency");
    assert_eq!(
        constant(&evaluated, "CONFIG")
            .normalization
            .as_ref()
            .unwrap()
            .canonical_result_encoding,
        literal_encoding(
            &format!(
                "{declarations} const CONFIG: Config = Config {{ mode: Mode::On, count: 7 }};"
            ),
            "CONFIG"
        )
    );
    syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&evaluated),
    )
    .expect("case identity survives evaluation and receipt replay");
}

#[test]
fn retained_invocation_replay_rechecks_concrete_crash_discharge() {
    // Both actuals would return 7 if interpretation were used as admission.
    // Only false discharges the published ceiling; keep the folded value fixed.
    let (syntax, sources) = parse(
        "machine gate(flag: bool) -> u64 crashes Trap flag { 7 }
        const SIZE: u64 = gate(false);",
    );
    let evaluated = super::super::evaluate(syntax, Some(sources.clone()), &[], None)
        .expect("concrete safe invocation");
    let mut typed =
        crate::front_end::typed_program_from_evaluated_syntax(&evaluated, Some(sources));
    super::super::validate_retained_invocations(&typed, None).expect("unchanged discharge");
    let original = typed.const_declarations()[0].authored_initializer;
    let typed_trees::expression::ExpressionNode::Call(call) =
        typed.expression_table.expression(original)
    else {
        panic!("retained call");
    };
    let argument = typed.expression_table.expression_handles(call.arguments)[0];
    *typed.expression_table.expression_mut(argument) =
        typed_trees::expression::ExpressionNode::Boolean(true);
    let diagnostics = super::super::validate_retained_invocations(&typed, None)
        .expect_err("same result cannot preserve changed invocation admission");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("retains unhandled")),
        "{diagnostics:?}"
    );
}

#[test]
fn retained_invocation_replay_rejects_changed_or_erased_computation_and_results() {
    let (syntax, sources) = parse(
        "machine size() -> u64 { 7 }
        machine unrelated() -> u64 { 7 }
        const SIZE: u64 = size();",
    );
    let evaluated =
        super::super::evaluate(syntax, Some(sources.clone()), &[], None).expect("evaluated call");
    let typed = crate::front_end::typed_program_from_evaluated_syntax(&evaluated, Some(sources));
    super::super::validate_retained_invocations(&typed, None).expect("unchanged invocation replay");
    let declaration = typed.const_declarations()[0].clone();
    for mutation in 0..6 {
        let mut changed = typed.clone();
        match mutation {
            0 => {
                *changed
                    .expression_table
                    .expression_mut(declaration.authored_initializer) =
                    typed_trees::expression::ExpressionNode::Integer(
                        numerics::literals::IntegerLiteral::from_value(7),
                    )
            }
            1 => {
                *changed
                    .expression_table
                    .expression_mut(declaration.materialized_initializer) =
                    typed_trees::expression::ExpressionNode::Integer(
                        numerics::literals::IntegerLiteral::from_value(8),
                    )
            }
            2 => {
                let other = changed
                    .machines()
                    .iter()
                    .find(|machine| machine.name.as_str() == "unrelated")
                    .unwrap();
                let target = changed.machine_states(other)[0].symbol;
                let typed_trees::expression::ExpressionNode::Call(call) = changed
                    .expression_table
                    .expression_mut(declaration.authored_initializer)
                else {
                    panic!("retained original call")
                };
                call.target_symbol = target;
            }
            3 => {
                let span = changed.roots.const_declarations;
                let retained = &mut changed.tables.const_declarations.span_mut_or_empty(span)[0];
                retained.authored_initializer =
                    typed_trees::expression::ExpressionHandle::invalid();
                retained.materialized_initializer =
                    typed_trees::expression::ExpressionHandle::invalid();
            }
            4 => {
                let machine = changed
                    .machines()
                    .iter()
                    .find(|machine| machine.name.as_str() == "size")
                    .unwrap();
                let state = &changed.machine_states(machine)[0];
                let value = match &changed.statement_table.statements(state.statement_nodes)[0] {
                    typed_trees::statement::StatementNode::Expression(value) => *value,
                    typed_trees::statement::StatementNode::Transition(transition) => {
                        let typed_trees::statement::TransitionTargetNode::Value(value) =
                            changed.statement_table.transition_target(transition.target)
                        else {
                            panic!("value return")
                        };
                        *value
                    }
                    _ => panic!("value body"),
                };
                *changed.expression_table.expression_mut(value) =
                    typed_trees::expression::ExpressionNode::Integer(
                        numerics::literals::IntegerLiteral::from_value(8),
                    );
            }
            5 => {
                let value = changed.expression_table.insert(
                    typed_trees::expression::ExpressionNode::Integer(
                        numerics::literals::IntegerLiteral::from_value(7),
                    ),
                );
                changed
                    .expression_table
                    .set_source_span(value, declaration.initializer_source_span);
                let span = changed.roots.const_declarations;
                changed.tables.const_declarations.span_mut_or_empty(span)[0]
                    .materialized_initializer = value;
            }
            _ => unreachable!(),
        }
        assert!(
            super::super::validate_retained_invocations(&changed, None).is_err(),
            "mutation {mutation} bypassed replay"
        );
    }
}
