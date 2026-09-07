//! Endpoint custody is retained before executable range admission.

use super::*;
use checked_trees::{CheckedScalarExpression, CheckedScalarExpressionRole};
use typed_trees::{expression::ExpressionNode, types::PrimitiveType};

#[test]
fn byte_subslice_endpoints_bind_dense_structural_roles_and_prior_locals() {
    // This checks retention, not a bounds theorem: unknown bounds need a
    // separately checked guard or contract before the body can execute.
    let program = typed_trees(
        "boundary trait Host {
            machine write(first: u8, whole: &[u8], enabled: bool, tail: &[u8], last: u8);
         }
         machine forward(bytes: &[u8], start: u64, end: u64) reaches Host {
            let saved: u64 = start;
            Host::write(7, bytes, true, bytes[saved..end], 9);
         }",
    );
    let state = &program.machine_states(&program.machines()[0])[0];
    let parameters = program.state_parameters(state);
    let statements = program.statement_table.statements(state.statement_nodes);
    let [StatementNode::LocalData(local), StatementNode::Call(call)] = statements else {
        panic!("one local precedes the boundary call")
    };
    let arguments = program.statement_table.expression_handles(call.arguments);
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(arguments[3]) else {
        panic!("fourth authored argument is the second structural operand")
    };
    let ExpressionNode::Range(range) = program.expression_table.expression(indexed.index) else {
        panic!("exclusive source range")
    };
    let plans = crate::values::build_checked_scalar_expression_plans(
        &program,
        &checked_trees::CheckedOperatorFacts::default(),
        &[],
    );
    for (role, authored, expected) in [
        (
            CheckedScalarExpressionRole::ByteSequenceSubsliceStart {
                call_ordinal: 0,
                argument_ordinal: 1,
            },
            range.start,
            CheckedScalarExpression::Local {
                position: 2,
                primitive_type: PrimitiveType::U64,
            },
        ),
        (
            CheckedScalarExpressionRole::ByteSequenceSubsliceEnd {
                call_ordinal: 0,
                argument_ordinal: 1,
            },
            range.end,
            CheckedScalarExpression::Parameter {
                position: 1,
                primitive_type: PrimitiveType::U64,
            },
        ),
    ] {
        let (binding, expression) = plans
            .bound_expression_at(state.symbol, 1, role)
            .expect("unique source-bound endpoint");
        assert_eq!(binding.expression, authored);
        assert!(!binding.destination.is_valid());
        assert_eq!(expression, &expected);
        assert_eq!(
            plans.binding_symbols.span_or_empty(binding.symbols),
            &[parameters[1].symbol, parameters[2].symbol, local.symbol]
        );
    }
    assert!(
        plans
            .bound_expression_at(
                state.symbol,
                1,
                CheckedScalarExpressionRole::ByteSequenceSubsliceStart {
                    call_ordinal: 0,
                    argument_ordinal: 3,
                }
            )
            .is_none(),
        "mixed authored position must not alias structural ordinal"
    );
}

#[test]
fn byte_subslice_endpoint_retention_lands_only_exact_u64_and_keeps_omissions() {
    for (range, start, end) in [
        ("..", false, false),
        ("0..", true, false),
        ("..1", false, true),
        ("(1 + 2)..4u64", true, true),
        ("0i64..1i64", false, false),
        ("0u32..1u32", false, false),
        ("0..=1", false, false),
    ] {
        let program = typed_trees(&format!(
            "data Relay {{}}
             machine Relay::write(bytes: &[u8]) {{}}
             machine forward(bytes: &[u8]) {{ Relay::write(bytes[{range}]); }}"
        ));
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "forward")
            .expect("ordinary caller");
        let state = &program.machine_states(machine)[0];
        let plans = crate::values::build_checked_scalar_expression_plans(
            &program,
            &checked_trees::CheckedOperatorFacts::default(),
            &[],
        );
        for (role, retained) in [
            (
                CheckedScalarExpressionRole::ByteSequenceSubsliceStart {
                    call_ordinal: 0,
                    argument_ordinal: 0,
                },
                start,
            ),
            (
                CheckedScalarExpressionRole::ByteSequenceSubsliceEnd {
                    call_ordinal: 0,
                    argument_ordinal: 0,
                },
                end,
            ),
        ] {
            let endpoint = plans.bound_expression_at(state.symbol, 0, role);
            assert_eq!(endpoint.is_some(), retained, "{range}: {role:?}");
            if let Some((_, expression)) = endpoint {
                assert_eq!(expression.primitive_type(), Some(PrimitiveType::U64));
            }
        }
    }
}

#[test]
fn byte_subslice_full_view_retains_an_ordinary_checked_call_plan() {
    let checked = lower_typed_trees(typed_trees(
        "boundary trait Host { machine write(bytes: &[u8]); }
         data Relay {}
         machine Relay::write(bytes: &[u8]) reaches Host { Host::write(bytes); }
         data Helper {}
         machine Helper::write(bytes: &[u8]) reaches Host {
            Relay::write(bytes[..]);
            Host::write(bytes);
         }",
    ))
    .expect("full exclusive byte view checks without extra bounds");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| {
            machine
                .attached_data
                .as_ref()
                .is_some_and(|name| name.as_str() == "Helper")
        })
        .expect("helper");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine.symbol)
        .expect("full view retains the callable helper");
    let checked_trees::CheckedUnitEffectOperationPlan::CallUnit {
        structural_arguments,
        ..
    } = &plan.operations[0]
    else {
        panic!("ordinary Relay call")
    };
    assert!(matches!(
        structural_arguments[0].source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
            parameter_index: 0,
            start: None,
            end: None,
            ..
        }
    ));
    assert!(
        checked
            .facts
            .values
            .scalar_expressions
            .source_bindings
            .iter()
            .all(|(_, binding)| binding.state != plan.state
                || !matches!(
                    binding.role,
                    CheckedScalarExpressionRole::ByteSequenceSubsliceStart { .. }
                        | CheckedScalarExpressionRole::ByteSequenceSubsliceEnd { .. }
                ))
    );
}
