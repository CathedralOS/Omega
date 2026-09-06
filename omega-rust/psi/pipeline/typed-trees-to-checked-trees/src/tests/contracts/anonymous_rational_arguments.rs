use super::{lower_typed_trees, parse_typed_trees};
use checked_trees::{CheckedScalarComputationKind, CheckedScalarExpressionRole, CheckedTrees};

mod call_results;

#[derive(Clone, Copy, Debug)]
enum CallForm {
    Statement,
    Returned,
    LocalBinding,
}

const FORMS: [CallForm; 3] = [
    CallForm::Statement,
    CallForm::Returned,
    CallForm::LocalBinding,
];

fn source(form: CallForm, argument: &str, parameter_type: &str, guarantee: &str) -> String {
    match form {
        CallForm::Statement => format!(
            "machine accept(delivered: {parameter_type}) {{}}
             machine run() {{ accept({argument}); }}"
        ),
        CallForm::Returned | CallForm::LocalBinding => {
            let body = match form {
                CallForm::Returned => format!("accept({argument})"),
                CallForm::LocalBinding => format!("let saved: i32 = accept({argument}); saved"),
                CallForm::Statement => unreachable!(),
            };
            format!(
                "machine accept(delivered: {parameter_type}) -> i32
                 ensures result == delivered {{ delivered }}
                 machine run() -> i32 {guarantee} {{ {body} }}"
            )
        }
    }
}

fn accepts(source: &str) -> CheckedTrees {
    lower_typed_trees(parse_typed_trees(source))
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"))
}

fn rejects(source: &str) {
    match lower_typed_trees(parse_typed_trees(source)) {
        Ok(_) => panic!("invalid anonymous call argument or guarantee was accepted: {source}"),
        Err(diagnostics) => assert!(!diagnostics.is_empty(), "{source}"),
    }
}

fn assert_delivered_value(checked: &CheckedTrees, expected: i64) {
    let caller = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "run")
        .unwrap();
    let callee = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "accept")
        .unwrap();
    let state = checked.machine_states(caller)[0].symbol;
    let plans = &checked.facts.values.scalar_expressions;
    let mut arguments = Vec::new();
    for (_, binding) in plans.source_bindings.iter() {
        if binding.state == state
            && matches!(
                binding.role,
                CheckedScalarExpressionRole::CallArgument { .. }
                    | CheckedScalarExpressionRole::UnitCallArgument { .. }
            )
        {
            arguments.push(
                plans
                    .expression_at(binding.state, binding.statement_ordinal, binding.role)
                    .expect("the source argument binding has its retained value"),
            );
        }
    }
    let computations = &checked.facts.values.scalar_computations;
    for (_, node) in computations.nodes.iter() {
        if let CheckedScalarComputationKind::Call {
            target_machine,
            arguments: operands,
            ..
        } = &node.kind
            && *target_machine == callee.symbol
        {
            for operand in computations.operands.span_or_empty(*operands) {
                let CheckedScalarComputationKind::Value(value) =
                    &computations.nodes.get(*operand).kind
                else {
                    panic!("closed scalar argument must retain a pure value");
                };
                arguments.push(value);
            }
        }
    }
    assert!(
        !arguments.is_empty(),
        "the call must retain the delivered scalar operand"
    );
    for argument in arguments {
        assert_eq!(
            crate::values::evaluate_checked_scalar(argument, &mut |_| None),
            Some(facts::ScalarValue::Integer(
                numerics::bignum::BigInt::from_i64(expected)
            )),
            "the argument's checked operations must deliver {expected}: {argument:#?}"
        );
    }
}

#[test]
fn anonymous_rational_arguments_deliver_seven_at_every_call_form() {
    for form in FORMS {
        let checked = accepts(&source(form, "7 / 2 * 2", "i32", ""));
        assert_delivered_value(&checked, 7);
    }
}

#[test]
fn final_fractional_arguments_reject_at_every_call_form() {
    for form in FORMS {
        rejects(&source(form, "7 / 2", "i32", ""));
    }
}

#[test]
fn explicitly_typed_quotients_deliver_six_at_every_call_form() {
    for form in FORMS {
        let checked = accepts(&source(form, "7i32 / 2 * 2", "i32", ""));
        assert_delivered_value(&checked, 6);
    }
}

#[test]
fn exact_rational_arguments_establish_the_actual_singleton_parameter() {
    for form in FORMS {
        let checked = accepts(&source(form, "7 / 2 * 2", "i32 [7..=7]", ""));
        assert_delivered_value(&checked, 7);
    }
}

#[test]
fn rational_arguments_cannot_claim_the_truncated_singleton_parameter() {
    for form in FORMS {
        rejects(&source(form, "7 / 2 * 2", "i32 [6..=6]", ""));
    }
}

#[test]
fn large_anonymous_intermediates_land_only_at_the_parameter() {
    for argument in [
        "(2147483647 + 1) - 2147483641",
        "(18446744073709551615 + 1) - 18446744073709551609",
    ] {
        for form in FORMS {
            let checked = accepts(&source(form, argument, "i32 [7..=7]", ""));
            assert_delivered_value(&checked, 7);
        }
    }
}

#[test]
fn call_guarantees_transport_exact_rational_arguments() {
    for form in [CallForm::Returned, CallForm::LocalBinding] {
        accepts(&source(form, "7 / 2 * 2", "i32", "ensures result == 7"));
        rejects(&source(form, "7 / 2 * 2", "i32", "ensures result == 6"));
    }
}

#[test]
fn call_guarantees_preserve_explicitly_typed_integer_division() {
    for form in [CallForm::Returned, CallForm::LocalBinding] {
        accepts(&source(form, "7i32 / 2 * 2", "i32", "ensures result == 6"));
        rejects(&source(form, "7i32 / 2 * 2", "i32", "ensures result == 7"));
    }
}

#[test]
fn bounded_statement_argument_proof_consumes_the_exact_anonymous_value() {
    // Bypass ordinary checking so its rejection cannot conceal a proof consumer
    // that still truncates the quotient before establishing the parameter range.
    for (parameter_type, accepted) in [("i32 [7..=7]", true), ("i32 [6..=6]", false)] {
        let source = source(CallForm::Statement, "7 / 2 * 2", parameter_type, "");
        let program = parse_typed_trees(&source);
        let plan = proof::obligations::build_proof_plan(&program);
        match proof::checker::check_proof_plan(&plan) {
            Ok(()) => assert!(
                accepted,
                "wrong singleton argument proof accepted: {source}"
            ),
            Err(diagnostics) => {
                assert!(!accepted, "{source}: {diagnostics:#?}");
                assert!(!diagnostics.is_empty(), "{source}");
            }
        }
    }
}

#[test]
fn parameter_arithmetic_policy_does_not_truncate_anonymous_fractions() {
    for policy in ["Wrapping", "Saturating", "Trapping"] {
        let parameter_type = format!("i32 in {policy}");
        rejects(&source(CallForm::Statement, "7 / 2", &parameter_type, ""));
        // The same declaration admits an integral argument. Its policy changes
        // subsequent typed operations, not anonymous integrality at delivery.
        accepts(&source(CallForm::Statement, "7", &parameter_type, ""));
    }
}

#[test]
fn named_state_rational_argument_checks_and_proves_the_same_singleton() {
    for (bound, accepted) in [(7, true), (6, false)] {
        let source = format!(
            "machine run() -> i32 {{
                transition {{ _ -> finish(7 / 2 * 2) }}
                state finish(delivered: i32 [{bound}..={bound}]) -> i32 {{ delivered }}
            }}"
        );
        let program = parse_typed_trees(&source);
        let plan = proof::obligations::build_proof_plan(&program);
        match proof::checker::check_proof_plan(&plan) {
            Ok(()) => assert!(
                accepted,
                "wrong named-state singleton proof accepted: {source}"
            ),
            Err(diagnostics) => assert!(!accepted, "{source}: {diagnostics:#?}"),
        }
        if accepted {
            let checked = accepts(&source);
            let plans = &checked.facts.values.scalar_expressions;
            let binding = plans
                .source_bindings
                .iter()
                .map(|(_, binding)| binding)
                .find(|binding| {
                    matches!(
                        binding.role,
                        CheckedScalarExpressionRole::TransitionArgument {
                            argument_ordinal: 0
                        }
                    )
                })
                .expect("retained transition argument");
            let argument = plans
                .expression_at(binding.state, binding.statement_ordinal, binding.role)
                .expect("transition argument value");
            assert_eq!(
                crate::values::evaluate_checked_scalar(argument, &mut |_| None),
                Some(facts::ScalarValue::Integer(
                    numerics::bignum::BigInt::from_i64(7)
                ))
            );
        } else {
            rejects(&source);
        }
    }
}

#[test]
fn parameter_policy_cannot_wrap_or_saturate_anonymous_argument_landing() {
    for policy in ["", " in Wrapping", " in Saturating", " in Trapping"] {
        let parameter_type = format!("u8{policy}");
        for argument in ["513 / 2 * 2", "256", "-1"] {
            rejects(&source(CallForm::Statement, argument, &parameter_type, ""));
        }
        accepts(&source(
            CallForm::Statement,
            "255 / 2 * 2",
            &parameter_type,
            "",
        ));
    }
}
