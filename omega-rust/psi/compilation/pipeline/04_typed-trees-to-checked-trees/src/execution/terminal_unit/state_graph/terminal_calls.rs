//! Terminal machine calls in transition arms
//! ([state contracts](../../../../../../../../../wiki/spec/language/state_contracts.md#terminal-calls-in-transition-arms)).
//!
//! An arm is classified by its resolved target: a local state is an internal
//! jump (`successor` plans its edge), while another machine is an ordinary
//! invocation whose normal result completes this machine. Two placements
//! exist, and neither bypasses the ordinary call plan:
//!
//! - An unconditional final arm (`transition { _ -> self.finish() }`) is the
//!   state's last operation. The shared statement sequence plans it through
//!   `call_occurrences::terminal_named_call`, exactly as a single-state body
//!   does, so its occurrence counts against the state's call roster; the
//!   terminator then completes after it.
//! - A guarded arm's call runs only when its arm is selected, so it cannot
//!   join the state's linear operations. `arm_call` plans it with the same
//!   `build_call_operation` as the arm's own operation
//!   (`CheckedConditionalReturnArm::Call`); emission places it in the arm's
//!   private completion block.
//!
//! Its subslice operands replay at the call-argument site the checker records
//! for an invocation (`values::scalar::expression_plans::transition`), like an
//! expression call's.
//!
//! The callee's normal result completes this machine, so Unit and primitive
//! scalar results are planned. A scalar result is the call's ordinary scalar
//! binding: the unconditional arm binds it as the state's returned value
//! (`CheckedScalarReturnPlan::Binding` at the transition), and a guarded arm
//! binds it at the next ordinal of the namespace its arm inherits and returns
//! it there. The two signatures must agree exactly; checking rejects a
//! terminal call whose result differs from the caller's declared result.
//!
//! Scalar operand computations retain their nested calls. Structural operands
//! retain the ordinary preparation sequence; the shared argument schedule
//! evaluates both inside the selected arm in authored order.
//! Honest boundaries: structural results are not planned here; an arm call
//! transfers no claims.
use super::{
    CheckFacts, CheckedUnitStructuralParameterPlan, ScalarCalleePlans, TransitionExit,
    TransitionTargetNode, TypedTrees, control,
};
use crate::execution::terminal_unit::calls::{ExpectedCallValueResult, build_call_operation};
use crate::execution::terminal_unit::control::bind_scalar_call_result;
use crate::execution::terminal_unit::types::{ShapeCollector, is_unit, state_flow};

/// The exact callable destination an arm invokes. Local state transfers and
/// entry re-entry remain state-graph edges; requirements keep their declaration
/// identity and ordinary machines keep their executable state identity.
pub(super) fn invoked_callable(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    transition: &typed_trees::statement::TableTransition,
) -> Option<symbols::SymbolHandle> {
    if transition.exit != TransitionExit::Ordinary
        || transition.continuation.is_valid()
        || !program
            .statement_table
            .transition_target_is_valid(transition.target)
    {
        return None;
    }
    let TransitionTargetNode::Named {
        path,
        static_machine_parameter,
        ..
    } = program.statement_table.transition_target(transition.target)
    else {
        return None;
    };
    if static_machine_parameter.is_valid() {
        return None;
    }
    control::terminal_callable_target(program, machine, path.symbol).map(|(target, _)| target)
}

/// Plan a guarded arm's terminal machine call as the arm's own operation.
/// The outer flow call names this exact transition target after the guard's
/// earlier occurrences. Operand computations and structural preparation retain
/// their own calls and execute inside this arm's completion block.
///
/// A scalar-result machine forwards the callee's result: the call binds it
/// at `next_scalar_binding`, the ordinal the arm's namespace (the state's
/// retained bindings) gives its next result, and the arm returns that value.
#[allow(clippy::too_many_arguments)]
pub(super) fn arm_call(
    program: &TypedTrees,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleePlans<'_>,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural: &[CheckedUnitStructuralParameterPlan],
    entry_claims: &[checked_trees::CheckedUnitEntryClaimPlan],
    local_results: &[(
        checked_trees::CheckedUnitStructuralResultBindingPlan,
        facts::PlaceRoot,
    )],
    preamble: &[checked_trees::CheckedUnitEffectOperationPlan],
    result: &checked_trees::CheckedControlResultPlan,
    next_scalar_binding: usize,
    transition: &typed_trees::statement::TableTransition,
    statement_ordinal: u32,
    trace: &control::LocalConstructionTrace,
) -> Option<checked_trees::CheckedConditionalReturnArm> {
    let target = invoked_callable(program, machine, transition)?;
    trace.phase("state graph: terminator: terminal call arm: completion");
    trace.statement(Some(statement_ordinal));
    let return_type = crate::flow::call_target_return_type(program, target)?;
    // The callee's normal result is this machine's result, so the two
    // signatures must agree exactly; checking rejects any other pairing.
    let scalar_result = match result {
        checked_trees::CheckedControlResultPlan::Unit if is_unit(program, return_type) => None,
        checked_trees::CheckedControlResultPlan::Scalar { primitive_type }
            if program.primitive_type_reference(return_type) == Some(*primitive_type)
                && program.normalized_type_identity(return_type)
                    == program.normalized_type_identity(state.return_type) =>
        {
            Some(checked_trees::CheckedUnitScalarResultBindingPlan {
                statement_index: statement_ordinal,
                binding_ordinal: u32::try_from(next_scalar_binding).ok()?,
                primitive_type: *primitive_type,
            })
        }
        _ => return None,
    };
    trace.phase("state graph: terminator: terminal call arm: occurrence");
    trace.statement(Some(statement_ordinal));
    let flow = state_flow(facts, machine.symbol, state.symbol)?;
    let calls = facts.flow.control.calls.span_or_empty(flow.calls);
    let mut occurrences = calls.iter().filter(|call| {
        call.statement_index == statement_ordinal as usize
            && !facts.flow.control.is_retired(state.symbol, call)
            && matches!(
                crate::semantic::calls::find_call_site(
                    program,
                    machine.symbol,
                    state.symbol,
                    call.statement_index,
                    call.call_ordinal,
                ),
                Some(crate::semantic::calls::CallSite::TransitionNamed { .. })
            )
            && crate::semantic::calls::transition_call_target(
                program,
                machine,
                state,
                call.statement_index,
                call.call_ordinal,
            ) == Some(transition.target)
    });
    let call = occurrences.next()?;
    // Flow custody retains the resolved authored symbol, which can name the
    // machine rather than its entry state. Compare executable destinations
    // without replacing that source identity in the ordinary call fact.
    if occurrences.next().is_some()
        || control::terminal_callable_target(program, machine, call.target_symbol)
            .is_none_or(|(selected, _)| selected != target)
    {
        return None;
    }
    trace.phase("state graph: terminator: terminal call arm: operands");
    let mut structural_results = local_results.to_vec();
    let mut preparation = preamble.to_vec();
    let predecessor_count = preparation.len();
    let mut structural_count = super::returns::next_result_ordinal(preamble)?;
    control::structural_operands::plan_call_operands(
        program,
        facts,
        scalar_callees,
        shapes,
        machine,
        state,
        structural,
        &[],
        entry_claims,
        &mut structural_results,
        &mut preparation,
        &mut structural_count,
        call,
        trace,
    )?;
    trace.phase("state graph: terminator: terminal call arm: operand occurrences");
    trace.statement(Some(statement_ordinal));
    control::call_owns_statement_computations(
        program,
        facts,
        machine.symbol,
        state.symbol,
        call,
        calls,
    )?;
    trace.phase("state graph: terminator: terminal call arm: call operation");
    trace.statement(Some(statement_ordinal));
    let operation = build_call_operation(
        program,
        facts,
        Some(scalar_callees),
        machine,
        state,
        structural,
        &[],
        entry_claims,
        call,
        false,
        scalar_result.map(|result| ExpectedCallValueResult::Scalar(result.primitive_type)),
        &structural_results,
        trace,
    )?;
    let planned = operation.call()?;
    if *planned.result() != checked_trees::CheckedUnitCallResult::Unit
        || !planned.claim_transfers().is_empty()
        || planned.coordinate.statement_index != statement_ordinal
    {
        return None;
    }
    let operation = match scalar_result {
        Some(result) => bind_scalar_call_result(facts, operation, result, true)?,
        None => operation,
    };
    control::statement_sequence::consume_results(&mut preparation, &operation)?;
    preparation.drain(..predecessor_count);
    Some(checked_trees::CheckedConditionalReturnArm::Call {
        operands: checked_trees::CheckedOperandSequence::new(preparation),
        operation,
    })
}

/// Whether this state's authored tail holds a guarded arm that invokes
/// another machine. Such a body needs the state graph even with one state:
/// the linear single-state sequence cannot select which arm's call runs.
pub(super) fn has_guarded_machine_arm(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
) -> bool {
    let statements = program.statement_table.statements(state.statement_nodes);
    let Some(start) = statements.iter().position(|statement| {
        matches!(
            statement,
            typed_trees::statement::StatementNode::Transition(_)
        )
    }) else {
        return false;
    };
    let tail = &statements[start..];
    tail.len() > 1
        && tail.iter().all(|statement| {
            matches!(
                statement,
                typed_trees::statement::StatementNode::Transition(_)
            )
        })
        && tail.iter().any(|statement| {
            matches!(statement, typed_trees::statement::StatementNode::Transition(transition)
                if invoked_callable(program, machine, transition).is_some())
        })
}

#[cfg(test)]
mod tests {
    #[test]
    fn guarded_terminal_operands_retain_nested_call_custody() {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../tests/omega/pass/calls/runtime_guarded_terminal_operand_order_exit/main.omg"
        ));
        assert_guarded_terminal_call_custody(source, 0, 0);
    }

    #[test]
    fn guarded_terminal_guard_and_operands_retain_separate_call_custody() {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../tests/omega/pass/calls/runtime_guarded_terminal_guard_order_exit/main.omg"
        ));
        assert_guarded_terminal_call_custody(source, 1, 0);
    }

    #[test]
    fn guarded_terminal_structural_operands_retain_selected_preparation() {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../tests/omega/pass/calls/runtime_guarded_terminal_array_operands_exit/main.omg"
        ));
        assert_guarded_terminal_call_custody(source, 0, 1);
    }

    fn assert_guarded_terminal_call_custody(
        source: &str,
        call_ordinal: u32,
        operand_operations: usize,
    ) {
        let checked = crate::tests::front_end::checked_program(source);
        let program = &checked.typed;
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "choose")
            .expect("caller");
        if operand_operations != 0 {
            let state = &program.machine_states(machine)[0];
            let site = crate::semantic::calls::find_call_site(
                program,
                machine.symbol,
                state.symbol,
                0,
                call_ordinal as usize,
            )
            .expect("first selected arm has its authored call coordinate");
            let crate::semantic::calls::CallSite::TransitionNamed { path, .. } = site else {
                panic!("first selected arm is the terminal invocation");
            };
            let (_, return_type) =
                super::control::terminal_callable_target(program, machine, path.symbol)
                    .expect("terminal invocation resolves its result signature");
            assert_eq!(
                program.primitive_type_reference(return_type),
                Some(typed_trees::types::PrimitiveType::U64)
            );
            let arrays = crate::values::call_array_constructions(
                program,
                &checked.facts.flow,
                machine,
                state,
                0,
            );
            assert_eq!(
                arrays.len(),
                1,
                "terminal invocation {:?} at statement0/call{call_ordinal} retains its array actual",
                program.symbols.get(path.symbol),
            );
            assert_eq!(
                arrays[0].source,
                checked_trees::CheckedArrayConstructionSource::CallArgument {
                    call_ordinal,
                    parameter_position: 1,
                },
                "the structural actual keeps its full authored formal position"
            );
        }
        assert!(
            checked
                .facts
                .flow
                .terminal_scalar_graphs
                .for_machine(machine.symbol)
                .is_none(),
            "an incomplete fused graph must yield to the common invocation plan"
        );
        let trace = super::control::LocalConstructionTrace::default();
        let plan = super::super::build_traced(
            program,
            &checked.facts,
            super::ScalarCalleePlans {
                boundary_returns: &checked.facts.flow.terminal_boundary_scalar_returns,
                structural_returns: &checked.facts.flow.terminal_structural_scalar_returns,
            },
            &mut crate::execution::terminal_unit::types::ShapeCollector::new(program),
            machine,
            None,
            &trace,
        )
        .unwrap_or_else(|| panic!("nested terminal operand plan: {:?}", trace.stage()));
        for arm in plan.states[0].terminator.return_arms() {
            let checked_trees::CheckedConditionalReturnArm::Call { operands, .. } = arm else {
                panic!("terminal invocation arm");
            };
            assert_eq!(operands.iter().count(), operand_operations);
        }
        let calls = plan.states[0]
            .operation_dependencies()
            .filter_map(|operation| operation.call())
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 2, "each arm owns one outer invocation");
        assert_eq!(calls[0].coordinate.call_ordinal, call_ordinal);
        assert_eq!(calls[0].scalar_arguments.len(), 2);
        assert!(calls[0].scalar_arguments.iter().all(|argument| matches!(
            argument,
            checked_trees::CheckedCallScalarArgument::Computation(_)
        )));
    }

    #[test]
    fn conditional_requirement_calls_retain_callable_targets_and_plans() {
        for source in [
            r#"
            boundary trait Host { machine send(value: i32); }
            machine choose(value: bool) reaches Host {
                transition value {
                    true -> Host::send(70)
                    _ -> Host::send(74)
                }
            }
            "#,
            r#"
            data Value {}
            boundary requirement Value::read(value: i32) -> i32;
            machine choose(value: i32) -> i32 {
                transition value == 70 {
                    true -> Value::read(value)
                    _ -> Value::read(71)
                }
            }
            "#,
        ] {
            let checked = crate::tests::front_end::checked_program(source);
            let program = &checked.typed;
            let machine = program
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == "choose")
                .expect("caller");
            let state = &program.machine_states(machine)[0];
            for statement in program.statement_table.statements(state.statement_nodes) {
                let typed_trees::statement::StatementNode::Transition(transition) = statement
                else {
                    continue;
                };
                let typed_trees::statement::TransitionTargetNode::Named { path, .. } =
                    program.statement_table.transition_target(transition.target)
                else {
                    panic!("named requirement call");
                };
                assert!(
                    super::control::terminal_callable_target(program, machine, path.symbol)
                        .is_some(),
                    "missing callable target {:?}: {:?}",
                    path,
                    program.symbols.get(path.symbol),
                );
                assert!(super::invoked_callable(program, machine, transition).is_some());
            }
            assert!(super::has_guarded_machine_arm(program, machine, state));
            for statement_ordinal in 0..2 {
                let role = checked_trees::CheckedScalarExpressionRole::BoundaryCallArgument {
                    call_ordinal: 0,
                    argument_ordinal: 0,
                };
                assert!(
                    checked
                        .facts
                        .values
                        .scalar_computations
                        .root_at(state.symbol, statement_ordinal, role,)
                        .is_some()
                        || checked
                            .facts
                            .values
                            .scalar_expressions
                            .bound_expression_at(state.symbol, statement_ordinal, role,)
                            .is_some(),
                    "missing boundary argument at {statement_ordinal}; source: {source}",
                );
            }
            let trace = super::control::LocalConstructionTrace::default();
            let plan = super::super::build_traced(
                program,
                &checked.facts,
                super::ScalarCalleePlans {
                    boundary_returns: &checked.facts.flow.terminal_boundary_scalar_returns,
                    structural_returns: &checked.facts.flow.terminal_structural_scalar_returns,
                },
                &mut crate::execution::terminal_unit::types::ShapeCollector::new(program),
                machine,
                None,
                &trace,
            )
            .unwrap_or_else(|| {
                panic!(
                    "requirement terminal call plan: {:?}; source: {source}",
                    trace.stage()
                )
            });
            assert_eq!(
                plan.states.len(),
                1,
                "bodyless requirements own no graph states"
            );
            assert_eq!(
                plan.states[0]
                    .operation_dependencies()
                    .filter(|operation| operation.call().is_some())
                    .count(),
                2,
            );
        }
    }

    #[test]
    fn field_precondition_program_keeps_terminal_requirement_plan() {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../tests/omega/pass/recast/runtime_field_precondition_capture_exit/main.omg"
        ))
        .replace(
            "use omega_language_std::console;",
            "boundary trait Console { machine exit_process(return_code: i32) reaches Console; }",
        );
        let checked = crate::tests::front_end::checked_program(&source);
        let program = &checked.typed;
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "main")
            .expect("caller");
        let trace = super::control::LocalConstructionTrace::default();
        let plan = super::super::build_traced(
            program,
            &checked.facts,
            super::ScalarCalleePlans {
                boundary_returns: &checked.facts.flow.terminal_boundary_scalar_returns,
                structural_returns: &checked.facts.flow.terminal_structural_scalar_returns,
            },
            &mut crate::execution::terminal_unit::types::ShapeCollector::new(program),
            machine,
            None,
            &trace,
        )
        .unwrap_or_else(|| panic!("full recast terminal call plan: {:?}", trace.stage()));
        assert_eq!(plan.states.len(), 1);
        assert_eq!(
            plan.states[0]
                .operation_dependencies()
                .filter(|operation| operation.call().is_some())
                .count(),
            3,
        );
    }
}
