//! Checked scalar state facts that every Unit body shares: a state's
//! scalar binding prefix and terminator, every state's guarded exits, the
//! record shapes scalar computations construct inline, and each machine's
//! signature eligibility in `CheckedTerminalMachineSelections`.
//!
//! - While checked facts are built, `facts::build_check_facts` calls
//!   `build_checked_guarded_exit_plans`, which collects the ordered guarded
//!   exits and fallback of every state of every machine (`guarded_exits`),
//!   and `build_checked_terminal_machine_selections`.
//! - `execution::terminal_unit` plans each state's bindings and exit with
//!   `checked_binding_prefix` and `checked_terminator`.
//! - `execution::finalize_execution` adopts the shapes a computation
//!   constructs inline (`constructions::retain_shapes`) into the Unit type
//!   roster, so a record built only inside an expression still has its
//!   declaration.
//!
//! There is no separate scalar graph: Unit bodies are the one lowering route
//! for scalar machines, and these facts are what they read.

use checked_trees::{
    CheckedScalarBinding, CheckedScalarBindingValue, CheckedScalarBranchDestination,
    CheckedScalarStateTerminator, CheckedScalarSuccessor, CheckedTerminalMachineSelection,
    CheckedTerminalMachineSelections, CheckedTerminalSignatureEligibility,
};

pub(crate) fn build_checked_terminal_machine_selections(
    program: &dyn ::validation::CheckingInput,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> CheckedTerminalMachineSelections {
    // The shared runtime rank judgment, including weak edges across
    // machines, is what every body route ranks a cycle by: the state graph
    // projects it (`runtime_component_ranks`).
    let runtime_components = validation::validated_runtime_natural_components(program);
    CheckedTerminalMachineSelections {
        machines: program
            .machines()
            .iter()
            .map(|machine| CheckedTerminalMachineSelection {
                machine: machine.symbol,
                name: program.symbols.display_path(machine.symbol, "::"),
                // `satisfies` is checked requirement refinement, not a runtime
                // receiver or an alternate body. A closed checked machine keeps
                // its ordinary signature and its own published contract.
                signature: if !machine.type_parameters.is_empty()
                    || !machine.owned_data.is_empty()
                    || (machine.termination_plan.implementation_witness.is_some()
                        // A machine whose states the shared runtime rank
                        // judgment covers, or whose composed body ranks a
                        // cycle through sibling machines by the fused
                        // countdown, is ranked by that proof. Legacy
                        // machine-local rank recognizers cannot veto it;
                        // body planning and Terminal replay still validate
                        // every cyclic edge.
                        && !runtime_components.iter().any(|component| {
                            component.states.iter().any(|rank| {
                                program
                                    .machine_states(machine)
                                    .iter()
                                    .any(|state| state.symbol == rank.state)
                            })
                        })
                        && crate::checks::termination::proven_slice_length_ranks_with_call_frames(
                            program,
                            machine,
                            call_frames,
                        )
                        .is_none()
                        && crate::checks::termination::proven_nat_countdown_sccs_with_call_frames(
                            program,
                            machine,
                            call_frames,
                        )
                        .is_none_or(|components| components.is_empty())
                        && crate::checks::termination::proven_state_natural_ranks_with_call_frames(
                            program,
                            machine,
                            call_frames,
                        )
                        .is_none()
                        && !crate::execution::terminal_unit::fused_countdown_ranks(
                            program,
                            machine,
                            call_frames,
                        ))
                    // Blocking retains the ordinary stack and call ABI.
                    // Its ceiling and acknowledgements are checked separately;
                    // unlike suspension it needs no continuation representation.
                    || machine.suspends
                    // A bodied `boundary machine` adapter is an ordinary
                    // checked root: the Unit roster already builds its plan,
                    // so a narrower `is_checked_body()` here left its callers
                    // with a selection they could not lower.
                    || !machine.supplies_checked_body()
                {
                    CheckedTerminalSignatureEligibility::Unsupported
                } else if !program
                    .service_reach_rows
                    .services(machine.service_reach_row)
                    .is_empty()
                    || !machine.invokes.is_empty()
                    || program.machine_states(machine).iter().any(|state| {
                        program.state_parameters(state).iter().any(|parameter| {
                            typed_trees::service::exact_bound_service_requirement(
                                program,
                                parameter.type_reference,
                            )
                            .is_some()
                        })
                    })
                {
                    CheckedTerminalSignatureEligibility::FreeUnitEffect
                } else {
                    CheckedTerminalSignatureEligibility::Eligible
                },
            })
            .collect(),
    }
}
use typed_trees::statement::{
    StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetNode,
};

pub(crate) mod constructions;
mod guarded_exits;

/// Every state's ordered guarded exits and fallback, for every machine.
pub(crate) fn build_checked_guarded_exit_plans(
    program: &dyn ::validation::CheckingInput,
    expressions: &checked_trees::CheckedScalarExpressionPlans,
    computations: &checked_trees::CheckedScalarComputationPlans,
) -> checked_trees::CheckedGuardedExitPlans {
    let (exits, tails) = guarded_exits::build(program, expressions, computations);
    checked_trees::CheckedGuardedExitPlans { exits, tails }
}

/// Retain the contiguous authored declaration/assignment prefix in the shared
/// scalar binding representation. Consumers choose the supported value forms.
pub(super) fn checked_binding_prefix(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    computations: &checked_trees::CheckedScalarComputationPlans,
) -> Option<Vec<CheckedScalarBinding>> {
    checked_statement_bindings(program, state, computations, false)
}

/// A projected assignment belongs to the structural effect inventory, not
/// the scalar binding inventory. Both collectors must use the same rule:
/// otherwise a store can disappear between them while its graph stays eligible.
fn assignment_has_projected_target(
    program: &dyn ::validation::CheckingInput,
    assignment: &typed_trees::statement::TableAssignment,
) -> bool {
    matches!(
        program.expression_table.expression(assignment.target),
        typed_trees::expression::ExpressionNode::Member(_)
            | typed_trees::expression::ExpressionNode::Indexed(_)
    )
}

fn checked_statement_bindings(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    computations: &checked_trees::CheckedScalarComputationPlans,
    unit_calls: bool,
) -> Option<Vec<CheckedScalarBinding>> {
    let parameters = program.state_parameters(state);
    let statements = program.statement_table.statements(state.statement_nodes);
    let binding_count = statements
        .iter()
        .take_while(|statement| {
            matches!(
                statement,
                StatementNode::LocalData(_) | StatementNode::Assignment(_)
            ) || (unit_calls && matches!(statement, StatementNode::Call(_)))
        })
        .count();
    let bindings =
        statements[..binding_count]
            .iter()
            .enumerate()
            .filter(|(_, statement)| !matches!(statement, StatementNode::Call(_))
                && !(unit_calls && matches!(statement, StatementNode::Assignment(assignment)
                    if assignment_has_projected_target(program, assignment)))
                && !(unit_calls && matches!(statement, StatementNode::LocalData(local)
                    if program.primitive_type_reference(local.type_reference).is_none())))
            .map(|(statement_index, statement)| {
                use checked_trees::CheckedScalarBindingDestination;
                match statement {
                    StatementNode::LocalData(local) => {
                        if !program
                            .expression_table
                            .expression_is_valid(local.initial_value)
                            && crate::values::zeroed_scalar_local_initializer(program, local).is_none()
                        {
                            return None;
                        }
                        let statement_ordinal = u32::try_from(statement_index).ok()?;
                        let role = if local.is_mutable {
                            checked_trees::CheckedScalarExpressionRole::StorageInitializer
                        } else {
                            let preceding_immutable_count = statements[..statement_index].iter().filter(|statement| {
                                matches!(statement, StatementNode::LocalData(local) if !local.is_mutable
                                    && program.primitive_type_reference(local.type_reference).is_some())
                            }).count();
                            checked_trees::CheckedScalarExpressionRole::LocalInitializer {
                                binding_ordinal: u32::try_from(preceding_immutable_count).ok()?,
                            }
                        };
                        let value = if computations
                            .root_at(state.symbol, statement_ordinal, role)
                            .is_some()
                        {
                            CheckedScalarBindingValue::Computation
                        } else {
                            checked_binding_value(program, local.initial_value)?
                        };
                        if local.is_mutable
                            && matches!(value, CheckedScalarBindingValue::DirectCall { .. })
                        {
                            return None;
                        }
                        Some(CheckedScalarBinding {
                            statement_ordinal,
                            destination: if local.is_mutable {
                                CheckedScalarBindingDestination::StorageInitialize {
                                    symbol: local.symbol,
                                }
                            } else {
                                CheckedScalarBindingDestination::Immutable
                            },
                            primitive_type: program
                                .primitive_type_reference(local.type_reference)?,
                            value,
                        })
                    }
                    StatementNode::Assignment(assignment) => {
                        let typed_trees::expression::ExpressionNode::Name(name) =
                            program.expression_table.expression(assignment.target)
                        else {
                            return None;
                        };
                        if !name.symbol.is_valid() || name.head_symbol != name.symbol {
                            return None;
                        }
                        let destination = program
                            .state_local_declaration(state, name.symbol)
                            .filter(|&(ordinal, local)| {
                                ordinal < statement_index && local.is_mutable
                            })
                            .and_then(|(_, local)| {
                                Some((
                                    local.symbol,
                                    program.primitive_type_reference(local.type_reference)?,
                                ))
                            })
                            .or_else(|| {
                                parameters.iter().find_map(|parameter| {
                                    (parameter.symbol == name.symbol)
                                        .then(|| {
                                            Some((
                                                parameter.symbol,
                                                crate::values::mutable_scalar_parameter_type(
                                                    program, parameter,
                                                )?,
                                            ))
                                        })
                                        .flatten()
                                })
                            })?;
                        let statement_ordinal = u32::try_from(statement_index).ok()?;
                        let role = checked_trees::CheckedScalarExpressionRole::AssignmentValue;
                        let value = if computations
                            .root_at(state.symbol, statement_ordinal, role)
                            .is_some()
                        {
                            CheckedScalarBindingValue::Computation
                        } else {
                            CheckedScalarBindingValue::Expression
                        };
                        Some(CheckedScalarBinding {
                            statement_ordinal,
                            destination: CheckedScalarBindingDestination::StorageAssign {
                                symbol: destination.0,
                            },
                            primitive_type: destination.1,
                            value,
                        })
                    }
                    _ => None,
                }
            })
            .collect::<Option<Vec<_>>>()?;
    Some(bindings)
}

fn checked_binding_value(
    program: &dyn ::validation::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<CheckedScalarBindingValue> {
    let typed_trees::expression::ExpressionNode::Call(call) =
        program.expression_table.expression(expression)
    else {
        return Some(CheckedScalarBindingValue::Expression);
    };
    if call.receiver.is_valid() || !call.machine_arguments.is_empty() {
        return None;
    }
    let (target_machine, target_state) =
        validation::find_machine_by_entry_state(program, call.target_symbol)?;
    // DirectCall retains a checked body, not a requirement to be installed.
    // Whole-value boundary calls belong to the ordinary operation sequence,
    // which owns their result binding and exact provider-call custody. Giving
    // them a direct-call binding sends closure discovery looking for a body
    // that a boundary declaration deliberately does not contain.
    if !target_machine.supply_mode.is_checked_body() || !target_machine.body_is_present {
        return None;
    }
    let parameters = program.state_parameters(target_state);
    let authored_arguments = program
        .expression_table
        .expression_handles(call.arguments)
        .len();
    if authored_arguments != parameters.len() {
        return None;
    }
    // The retained argument count. An erased position has no caller argument
    // ordinal (`values::scalar::call_lowering` numbers `CallArgument` roles
    // over the retained positions only), matching the callee's stripped
    // scalar signature.
    let mut retained_arguments = 0usize;
    for parameter in parameters {
        if !super::terminal_unit::strips_erased_parameter(parameter)? {
            retained_arguments = retained_arguments.checked_add(1)?;
        }
    }
    Some(CheckedScalarBindingValue::DirectCall {
        target_machine: target_machine.symbol,
        target_state: call.target_symbol,
        // A supported call is the root of its local initializer. Nested calls
        // cannot acquire scalar argument plans and therefore fail closed.
        call_ordinal: 0,
        argument_count: u32::try_from(retained_arguments).ok()?,
    })
}

fn checked_successor(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    statement_ordinal: u32,
    transition: &typed_trees::statement::TableTransition,
    is_continuation: bool,
) -> Option<CheckedScalarSuccessor> {
    if transition.exit != TransitionExit::Ordinary {
        return None;
    }
    let TransitionTargetNode::Named {
        path, arguments, ..
    } = program
        .statement_table
        .transition_target(if is_continuation {
            transition.continuation
        } else {
            transition.target
        })
    else {
        return None;
    };
    // An authored machine-name backedge denotes its entry child. Reuse the
    // same identity normalization as ranking and structural state forwarding.
    let target = if let Some(target_index) =
        crate::checks::termination::named_transition_target_state_index(
            program,
            machine,
            path.symbol,
        ) {
        program.machine_states(machine).get(target_index)?.symbol
    } else {
        // A free name may retain the machine head while a qualified call
        // names its entry child. Both denote the same exact first state;
        // neither grants access to a foreign machine's interior states.
        validation::find_machine_head(program, path.symbol)?
            .1
            .symbol
    };
    Some(CheckedScalarSuccessor {
        statement_ordinal,
        is_continuation,
        target,
        argument_count: u32::try_from(program.statement_table.expression_handles(*arguments).len())
            .ok()?,
    })
}

/// Retain authored exit coordinates independently of the preceding operation values.
pub(super) fn checked_terminator(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expressions: &checked_trees::CheckedScalarExpressionPlans,
    binding_count: usize,
) -> Option<CheckedScalarStateTerminator> {
    let statements = program.statement_table.statements(state.statement_nodes);
    let terminator_ordinal = u32::try_from(binding_count).ok()?;
    Some(match &statements[binding_count..] {
        [StatementNode::Expression(_)] => CheckedScalarStateTerminator::Return {
            statement_ordinal: terminator_ordinal,
        },
        [StatementNode::Transition(transition)]
            if transition.exit == TransitionExit::Ordinary
                && transition.guard == TransitionGuardNode::Always
                && !transition.continuation.is_valid()
                && matches!(
                    program.statement_table.transition_target(transition.target),
                    TransitionTargetNode::Value(_)
                ) =>
        {
            CheckedScalarStateTerminator::Return {
                statement_ordinal: terminator_ordinal,
            }
        }
        [StatementNode::Transition(transition)]
            if matches!(transition.exit, TransitionExit::Crash(_))
                && transition.guard == TransitionGuardNode::Always
                && !transition.continuation.is_valid()
                && program
                    .statement_table
                    .transition_target_is_valid(transition.target)
                && matches!(
                    program.statement_table.transition_target(transition.target),
                    TransitionTargetNode::Terminal
                ) =>
        {
            CheckedScalarStateTerminator::Crash {
                statement_ordinal: terminator_ordinal,
            }
        }
        [
            StatementNode::Transition(when_true),
            StatementNode::Transition(when_false),
            rest @ ..,
        ] if matches!(when_true.guard, TransitionGuardNode::When(_))
            && (when_false.guard == TransitionGuardNode::Always
                || crate::execution::guard_complement::complementary(
                    program,
                    expressions,
                    state,
                    terminator_ordinal,
                ))
            && !when_true.continuation.is_valid()
            && !when_false.continuation.is_valid()
            && rest.iter().all(|statement| {
                matches!(
                    statement,
                    StatementNode::Transition(transition)
                        if transition.guard == TransitionGuardNode::Always
                            && transition.exit == TransitionExit::Ordinary
                            && !transition.continuation.is_valid()
                )
            }) =>
        {
            CheckedScalarStateTerminator::Conditional {
                guard_statement_ordinal: terminator_ordinal,
                when_true: checked_branch_destination(
                    program,
                    machine,
                    terminator_ordinal,
                    when_true,
                    false,
                )?,
                when_false: checked_branch_destination(
                    program,
                    machine,
                    terminator_ordinal.checked_add(1)?,
                    when_false,
                    false,
                )?,
            }
        }
        [StatementNode::Transition(transition)]
            if matches!(transition.guard, TransitionGuardNode::When(_))
                && transition.continuation.is_valid() =>
        {
            CheckedScalarStateTerminator::Conditional {
                guard_statement_ordinal: terminator_ordinal,
                when_true: checked_branch_destination(
                    program,
                    machine,
                    terminator_ordinal,
                    transition,
                    false,
                )?,
                when_false: checked_branch_destination(
                    program,
                    machine,
                    terminator_ordinal,
                    transition,
                    true,
                )?,
            }
        }
        [
            StatementNode::Transition(transition),
            StatementNode::Expression(_),
        ] if matches!(transition.guard, TransitionGuardNode::When(_))
            && !transition.continuation.is_valid() =>
        {
            CheckedScalarStateTerminator::Conditional {
                guard_statement_ordinal: terminator_ordinal,
                when_true: checked_branch_destination(
                    program,
                    machine,
                    terminator_ordinal,
                    transition,
                    false,
                )?,
                when_false: CheckedScalarBranchDestination::Return {
                    statement_ordinal: terminator_ordinal.checked_add(1)?,
                    is_continuation: false,
                },
            }
        }
        [StatementNode::Transition(transition)]
            if transition.guard == TransitionGuardNode::Always
                && !transition.continuation.is_valid() =>
        {
            CheckedScalarStateTerminator::Jump(checked_successor(
                program,
                machine,
                terminator_ordinal,
                transition,
                false,
            )?)
        }
        _ => return None,
    })
}

fn checked_branch_destination(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    statement_ordinal: u32,
    transition: &typed_trees::statement::TableTransition,
    is_continuation: bool,
) -> Option<CheckedScalarBranchDestination> {
    if matches!(transition.exit, TransitionExit::Crash(_)) {
        return (!is_continuation
            && !transition.continuation.is_valid()
            && program
                .statement_table
                .transition_target_is_valid(transition.target)
            && matches!(
                program.statement_table.transition_target(transition.target),
                TransitionTargetNode::Terminal
            ))
        .then_some(CheckedScalarBranchDestination::Crash { statement_ordinal });
    }
    let target = if is_continuation {
        transition.continuation
    } else {
        transition.target
    };
    match program.statement_table.transition_target(target) {
        // This shared roster retains completion coordinates, not a promise
        // of a scalar value. Unit consumers replay ordinary Terminal exits;
        // scalar consumers still require a return expression at this position.
        TransitionTargetNode::Terminal
            if !is_continuation
                && transition.exit == TransitionExit::Ordinary
                && !transition.continuation.is_valid() =>
        {
            Some(CheckedScalarBranchDestination::Return {
                statement_ordinal,
                is_continuation: false,
            })
        }
        TransitionTargetNode::Value(expression)
            if program.expression_table.expression_is_valid(*expression) =>
        {
            Some(CheckedScalarBranchDestination::Return {
                statement_ordinal,
                is_continuation,
            })
        }
        TransitionTargetNode::Named { .. } => checked_successor(
            program,
            machine,
            statement_ordinal,
            transition,
            is_continuation,
        )
        .map(CheckedScalarBranchDestination::Jump),
        _ => None,
    }
}
