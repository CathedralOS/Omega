use super::{
    BOOLEAN_HELPERS, INTEGER_HELPERS, ObserveSettlement, artifact, assert_unsettled_helper_crash,
    pause_before_crashing_helper, source, start, unsigned,
};
use crate::structural_return_source::{
    TerminalExecutionResult, TerminalExecutionStatus, TerminalFuelMeter, TerminalInterpretError,
    TerminalScalarValue,
};
use checked_trees::{
    CheckedScalarComputationKind, CheckedScalarExpressionRole, CheckedUnitEffectOperationPlan,
};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;

#[test]
fn returned_boolean_and_integer_boundary_values_preserve_computed_and_pure_operands() {
    for (source, expected) in [
        (
            source(
                "bool",
                "identity(false), Scalar::identity(identity(true))",
                BOOLEAN_HELPERS,
                false,
            ),
            vec![
                TerminalScalarValue::Boolean(false),
                TerminalScalarValue::Boolean(true),
            ],
        ),
        (
            source(
                "u16",
                "(Scalar::identity(identity(255u8)) as u16) + 1u16, identity(7u8) as u16",
                INTEGER_HELPERS,
                false,
            ),
            vec![unsigned(256), unsigned(7)],
        ),
        (
            source("bool", "false, true", "", false),
            vec![
                TerminalScalarValue::Boolean(false),
                TerminalScalarValue::Boolean(true),
            ],
        ),
    ] {
        let artifact = artifact(&crate::front_end::checked_program(&source));
        let mut execution = start(&artifact);
        let mut observer = ObserveSettlement::default();
        assert_eq!(
            execution
                .resume(&mut TerminalFuelMeter::unbounded(), &mut observer)
                .unwrap(),
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
                *expected.last().unwrap()
            ))
        );
        assert_eq!(observer.calls, vec![expected]);
        assert_eq!(execution.effects().len(), 1);
        assert_eq!(execution.live_claim_frontier().count(), 0);
    }
}

#[test]
fn returned_boundary_provider_rejection_preserves_receipt_until_successful_retry() {
    for (source, expected) in [
        (
            source(
                "bool",
                "identity(false), Scalar::identity(identity(true))",
                BOOLEAN_HELPERS,
                false,
            ),
            vec![
                TerminalScalarValue::Boolean(false),
                TerminalScalarValue::Boolean(true),
            ],
        ),
        (
            source(
                "u16",
                "identity(17u8) as u16, Scalar::identity(identity(23u8)) as u16",
                INTEGER_HELPERS,
                false,
            ),
            vec![unsigned(17), unsigned(23)],
        ),
    ] {
        let artifact = artifact(&crate::front_end::checked_program(&source));
        let mut execution = start(&artifact);
        let claims = execution.live_claim_frontier().collect::<Vec<_>>();
        assert_eq!(claims.len(), 1);
        let mut observer = ObserveSettlement {
            reject: true,
            ..ObserveSettlement::default()
        };
        assert!(
            matches!(execution.resume(&mut TerminalFuelMeter::unbounded(), &mut observer), Err(TerminalInterpretError::EffectRejected { rejection, .. }) if rejection.reason == "settlement refused")
        );
        assert_eq!(execution.live_claim_frontier().collect::<Vec<_>>(), claims);
        assert!(execution.effects().is_empty());
        assert_eq!(observer.calls, vec![expected.clone()]);
        observer.reject = false;
        assert_eq!(
            execution
                .resume(&mut TerminalFuelMeter::unbounded(), &mut observer)
                .unwrap(),
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
                *expected.last().unwrap()
            ))
        );
        assert_eq!(observer.calls, vec![expected.clone(), expected]);
        assert_eq!(observer.receipts[0], observer.receipts[1]);
        assert_eq!(execution.effects().len(), 1);
        assert_eq!(execution.live_claim_frontier().count(), 0);
    }
}

#[test]
fn returned_boundary_boolean_arguments_short_circuit_without_settling_on_crash() {
    let helpers = format!(
        "{BOOLEAN_HELPERS}\nmachine abort() -> bool crashes Abort {{ crash Abort; }}\nmachine trap() -> bool crashes Trap {{ crash Trap; }}"
    );
    for (first, second, cause) in [
        (false, true, None),
        (true, false, Some(terminal_psi::CrashCause::Abort)),
        (false, false, Some(terminal_psi::CrashCause::Trap)),
    ] {
        let source = source(
            "bool",
            &format!("identity({first}) && abort(), Scalar::identity({second}) || trap()"),
            &helpers,
            true,
        );
        let artifact = artifact(&crate::front_end::checked_program(&source));
        let mut execution = start(&artifact);
        let claims = execution.live_claim_frontier().collect::<Vec<_>>();
        let mut observer = ObserveSettlement::default();
        if let Some(cause) = cause {
            pause_before_crashing_helper(&artifact, &mut execution, &mut observer, &claims, cause);
        }
        let status = execution
            .resume(&mut TerminalFuelMeter::unbounded(), &mut observer)
            .unwrap();
        if let Some(cause) = cause {
            assert_unsettled_helper_crash(
                &artifact,
                &mut execution,
                &mut observer,
                &claims,
                cause,
                status,
            );
        } else {
            assert_eq!(
                status,
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
                    TerminalScalarValue::Boolean(true)
                ))
            );
            assert_eq!(
                observer.calls,
                vec![vec![
                    TerminalScalarValue::Boolean(false),
                    TerminalScalarValue::Boolean(true)
                ]]
            );
            assert_eq!(execution.live_claim_frontier().count(), 0);
        }
    }
}

#[test]
fn returned_boundary_first_argument_crash_precedes_later_cast_and_retains_linear_claim() {
    for (first, second, cause) in [
        ("Abort", "Trap", terminal_psi::CrashCause::Abort),
        ("Trap", "Abort", terminal_psi::CrashCause::Trap),
    ] {
        let helpers = format!(
            "machine first() -> u8 crashes {first} {{ crash {first}; }}\nmachine second() -> u8 crashes {second} {{ crash {second}; }}"
        );
        let artifact = artifact(&crate::front_end::checked_program(&source(
            "u16",
            "first() as u16, second() as u16",
            &helpers,
            true,
        )));
        let mut execution = start(&artifact);
        let claims = execution.live_claim_frontier().collect::<Vec<_>>();
        assert_eq!(claims.len(), 1);
        let mut observer = ObserveSettlement::default();
        pause_before_crashing_helper(&artifact, &mut execution, &mut observer, &claims, cause);
        let status = execution
            .resume(&mut TerminalFuelMeter::unbounded(), &mut observer)
            .unwrap();
        assert_unsettled_helper_crash(
            &artifact,
            &mut execution,
            &mut observer,
            &claims,
            cause,
            status,
        );
    }
}

#[test]
fn returned_boundary_computations_reject_outer_and_nested_source_custody_drift() {
    let checked = crate::front_end::checked_program(&source(
        "bool",
        "identity(false), Scalar::identity(identity(true))",
        BOOLEAN_HELPERS,
        false,
    ));
    artifact(&checked);
    let root = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Root::enter")
        .unwrap();
    let state = &checked.typed.machine_states(root)[0];
    let StatementNode::LocalData(local) = &checked
        .typed
        .statement_table
        .statements(state.statement_nodes)[0]
    else {
        unreachable!();
    };
    let ExpressionNode::Call(call) = checked
        .typed
        .expression_table
        .expression(local.initial_value)
    else {
        unreachable!();
    };
    let arguments = checked
        .typed
        .expression_table
        .expression_handles(call.arguments);
    let receipt = &checked.typed.state_parameters(state)[0];
    let (outer_flow, _) = checked
        .facts
        .flow
        .control
        .calls
        .iter()
        .find(|(_, flow)| flow.authored_expression == local.initial_value && flow.call_ordinal == 0)
        .unwrap();
    for mutation in 0..11 {
        let mut changed = checked.clone();
        match mutation {
            0 => {
                let ExpressionNode::Call(call) = changed
                    .typed
                    .expression_table
                    .expression_mut(local.initial_value)
                else {
                    unreachable!();
                };
                call.target_symbol = symbols::SymbolHandle::invalid();
            }
            1 => changed
                .typed
                .expression_table
                .set_expression_handle_at_offset(call.arguments, 0, arguments[1]),
            2 => {
                let plan = changed
                    .facts
                    .flow
                    .terminal_boundary_scalar_returns
                    .machines
                    .iter_mut()
                    .find(|plan| plan.machine == root.symbol)
                    .unwrap();
                let CheckedUnitEffectOperationPlan::BoundaryCall { coordinate, .. } =
                    &mut plan.boundary_call
                else {
                    unreachable!();
                };
                coordinate.call_ordinal += 1;
            }
            3 => {
                changed
                    .facts
                    .flow
                    .terminal_boundary_scalar_returns
                    .machines
                    .iter_mut()
                    .find(|plan| plan.machine == root.symbol)
                    .unwrap()
                    .return_statement_ordinal += 1
            }
            4 => {
                let plan = changed
                    .facts
                    .flow
                    .terminal_boundary_scalar_returns
                    .machines
                    .iter_mut()
                    .find(|plan| plan.machine == root.symbol)
                    .unwrap();
                let CheckedUnitEffectOperationPlan::BoundaryCall {
                    scalar_arguments, ..
                } = &mut plan.boundary_call
                else {
                    unreachable!();
                };
                scalar_arguments.swap(0, 1);
            }
            5 | 10 => {
                let StatementNode::LocalData(changed_local) = &mut changed
                    .typed
                    .statement_table
                    .statements_mut(state.statement_nodes)[0]
                else {
                    unreachable!();
                };
                if mutation == 5 {
                    changed_local.type_reference = receipt.type_reference;
                } else {
                    changed_local.symbol = symbols::SymbolHandle::invalid();
                }
            }
            6 => {
                changed
                    .facts
                    .flow
                    .terminal_boundary_scalar_returns
                    .machines
                    .iter_mut()
                    .find(|plan| plan.machine == root.symbol)
                    .unwrap()
                    .result_type = typed_trees::types::PrimitiveType::U16;
            }
            7 => {
                let StatementNode::Expression(returned) = checked
                    .typed
                    .statement_table
                    .statements(state.statement_nodes)[1]
                else {
                    unreachable!();
                };
                let ExpressionNode::Name(path) =
                    changed.typed.expression_table.expression_mut(returned)
                else {
                    unreachable!();
                };
                path.symbol = receipt.symbol;
            }
            8 | 9 => {
                changed
                    .facts
                    .flow
                    .control
                    .calls
                    .get_mut(outer_flow)
                    .authored_expression = if mutation == 8 {
                    arena::Handle::invalid()
                } else {
                    arena::Handle::from_parts(
                        local.initial_value.arena_index(),
                        local.initial_value.generation() + 1,
                    )
                };
            }
            _ => unreachable!(),
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &changed,
                TerminalMachineSelection::Name("Root::enter")
            )
            .is_err(),
            "outer returned-boundary mutation={mutation}"
        );
    }
    let computations = &checked.facts.values.scalar_computations;
    let roots = computations
        .roots
        .iter()
        .filter(|(_, root_plan)| {
            root_plan.machine == root.symbol
                && matches!(
                    root_plan.role,
                    CheckedScalarExpressionRole::BoundaryCallArgument { .. }
                )
        })
        .collect::<Vec<_>>();
    assert_eq!(roots.len(), 2);
    for (handle, plan) in roots {
        let mut changed = checked.clone();
        changed
            .facts
            .values
            .scalar_computations
            .roots
            .get_mut(handle)
            .statement_ordinal += 1;
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &changed,
                TerminalMachineSelection::Name("Root::enter")
            )
            .is_err()
        );
        let mut changed = checked.clone();
        changed
            .facts
            .values
            .scalar_computations
            .nodes
            .get_mut(plan.root)
            .authored_root = local.initial_value;
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &changed,
                TerminalMachineSelection::Name("Root::enter")
            )
            .is_err()
        );
    }
    for (_, node) in computations.nodes.iter() {
        let CheckedScalarComputationKind::Call { source_call, .. } = node.kind else {
            continue;
        };
        let authored = checked
            .facts
            .flow
            .control
            .calls
            .get(source_call)
            .authored_expression;
        let mut changed = checked.clone();
        changed
            .facts
            .flow
            .control
            .calls
            .get_mut(source_call)
            .authored_expression = arena::Handle::invalid();
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &changed,
                TerminalMachineSelection::Name("Root::enter")
            )
            .is_err()
        );
        let ExpressionNode::Call(call) = checked.typed.expression_table.expression(authored) else {
            unreachable!();
        };
        if call.receiver.is_valid() {
            let mut changed = checked.clone();
            let ExpressionNode::Name(path) =
                changed.typed.expression_table.expression_mut(call.receiver)
            else {
                unreachable!();
            };
            path.symbol = symbols::SymbolHandle::invalid();
            assert!(
                checked_trees_to_lowered_psi::lower_machine(
                    &changed,
                    TerminalMachineSelection::Name("Root::enter")
                )
                .is_err()
            );
        }
    }
}
