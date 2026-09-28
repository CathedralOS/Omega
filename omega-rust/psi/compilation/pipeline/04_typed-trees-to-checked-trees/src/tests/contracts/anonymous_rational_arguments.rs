use super::super::lower_typed_trees;
use super::parse_typed_trees;
use crate::CheckingRequest;
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
    lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"))
}

fn rejects(source: &str) {
    match lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled()) {
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
fn final_fractional_arguments_reject_at_every_call_form() {
    for argument in ["7 / 2", "7 / 2.0", "7.5", "0.1 * 71", "7.0f64"] {
        for form in FORMS {
            rejects(&source(form, argument, "i32", ""));
        }
    }
}

#[test]
fn large_anonymous_intermediates_land_only_at_the_parameter() {
    for argument in [
        "(2147483647 + 1) - 2147483641",
        "(18446744073709551615 + 1) - 18446744073709551609",
        "(18446744073709551615.0 + 1) - 18446744073709551609",
    ] {
        for form in FORMS {
            let checked = accepts(&source(form, argument, "i32 [7..=7]", ""));
            assert_delivered_value(&checked, 7);
        }
    }
}

#[test]
fn decimal_argument_proofs_establish_only_the_exact_integer_value() {
    for argument in ["7.0", "7 / 2.0 * 2", "0.1 * 70"] {
        for (parameter_type, accepted) in [("i32 [7..=7]", true), ("i32 [6..=6]", false)] {
            let source = source(CallForm::Statement, argument, parameter_type, "");
            let program = parse_typed_trees(&source);
            let plan = proof::obligations::build_proof_plan(&program);
            assert_eq!(
                proof::checker::check_proof_plan(&plan).is_ok(),
                accepted,
                "{source}"
            );
        }
    }
}
