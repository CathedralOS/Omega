use checked_trees::{
    CheckedScalarBinding, CheckedScalarBindingValue, CheckedScalarBranchDestination,
    CheckedScalarGraphPlans, CheckedScalarMachineGraph, CheckedScalarStateGraph,
    CheckedScalarStateTerminator, CheckedScalarSuccessor, CheckedTerminalMachineSelection,
    CheckedTerminalMachineSelections, CheckedTerminalSignatureEligibility,
};

pub(crate) fn build_checked_terminal_machine_selections(
    program: &TypedTrees,
) -> CheckedTerminalMachineSelections {
    CheckedTerminalMachineSelections {
        machines: program
            .machines()
            .iter()
            .map(|machine| CheckedTerminalMachineSelection {
                machine: machine.symbol,
                name: machine.name.as_str().to_owned(),
                signature: if machine.attached_data.is_some() {
                    CheckedTerminalSignatureEligibility::Attached
                } else if !machine.type_parameters.is_empty()
                    || !machine.owned_data.is_empty()
                    || !machine.satisfies.is_empty()
                    || machine.termination_plan.implementation_witness.is_some()
                    || machine.suspends
                    || machine.blocks
                    || !machine.supply_mode.is_checked_body()
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
use typed_trees::{
    TypedTrees,
    statement::{StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetNode},
};

mod guards;

#[cfg(test)]
mod tests;

pub(crate) fn build_checked_scalar_graph_plans(
    program: &TypedTrees,
    expressions: &checked_trees::CheckedScalarExpressionPlans,
    computations: &checked_trees::CheckedScalarComputationPlans,
) -> CheckedScalarGraphPlans {
    CheckedScalarGraphPlans {
        machines: program
            .machines()
            .iter()
            .filter_map(|machine| build_machine_graph(program, machine, expressions, computations))
            .collect(),
    }
}

fn build_machine_graph(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    expressions: &checked_trees::CheckedScalarExpressionPlans,
    computations: &checked_trees::CheckedScalarComputationPlans,
) -> Option<CheckedScalarMachineGraph> {
    let source_states = program.machine_states(machine);
    if source_states.is_empty() {
        return None;
    }
    let states = source_states
        .iter()
        .map(|state| {
            if !program.state_contracts(state).is_empty() {
                return None;
            }
            let parameters = program.state_parameters(state);
            if parameters
                .iter()
                .any(|parameter| parameter.is_self || parameter.is_const || parameter.is_mutable)
            {
                return None;
            }
            let parameter_types = parameters
                .iter()
                .map(|parameter| program.primitive_type_reference(parameter.type_reference))
                .collect::<Option<Vec<_>>>()?;
            let result_type = program.primitive_type_reference(state.return_type)?;
            let statements = program.statement_table.statements(state.statement_nodes);
            let binding_count = statements
                .iter()
                .take_while(|statement| {
                    matches!(
                        statement,
                        StatementNode::LocalData(_) | StatementNode::Assignment(_)
                    )
                })
                .count();
            let bindings = statements[..binding_count]
                .iter()
                .enumerate()
                .map(|(statement_index, statement)| {
                    use checked_trees::CheckedScalarBindingDestination;
                    match statement {
                        StatementNode::LocalData(local) => {
                            if !program
                                .expression_table
                                .expression_is_valid(local.initial_value)
                            {
                                return None;
                            }
                            let statement_ordinal = u32::try_from(statement_index).ok()?;
                            let role = if local.is_mutable {
                                checked_trees::CheckedScalarExpressionRole::StorageInitializer
                            } else {
                                checked_trees::CheckedScalarExpressionRole::LocalInitializer {
                                    binding_ordinal: u32::try_from(statements[..statement_index]
                                        .iter().filter(|statement| matches!(statement,
                                            StatementNode::LocalData(local) if !local.is_mutable
                                        )).count()).ok()?,
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
                            let local =
                                statements[..statement_index].iter().find_map(|statement| {
                                    match statement {
                                        StatementNode::LocalData(local)
                                            if local.symbol == name.symbol && local.is_mutable =>
                                        {
                                            Some(local)
                                        }
                                        _ => None,
                                    }
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
                                    symbol: local.symbol,
                                },
                                primitive_type: program
                                    .primitive_type_reference(local.type_reference)?,
                                value,
                            })
                        }
                        _ => None,
                    }
                })
                .collect::<Option<Vec<_>>>()?;
            let terminator_ordinal = u32::try_from(binding_count).ok()?;
            let terminator = match &statements[binding_count..] {
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
                ] if matches!(when_true.guard, TransitionGuardNode::When(_))
                    && (when_false.guard == TransitionGuardNode::Always
                        || guards::complementary(
                            expressions,
                            state.symbol,
                            terminator_ordinal,
                        ))
                    && !when_true.continuation.is_valid()
                    && !when_false.continuation.is_valid() =>
                {
                    CheckedScalarStateTerminator::Conditional {
                        guard_statement_ordinal: terminator_ordinal,
                        when_true: checked_branch_destination(
                            program,
                            source_states,
                            terminator_ordinal,
                            when_true,
                            false,
                        )?,
                        when_false: checked_branch_destination(
                            program,
                            source_states,
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
                            source_states,
                            terminator_ordinal,
                            transition,
                            false,
                        )?,
                        when_false: checked_branch_destination(
                            program,
                            source_states,
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
                            source_states,
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
                        source_states,
                        terminator_ordinal,
                        transition,
                        false,
                    )?)
                }
                _ => return None,
            };
            Some(CheckedScalarStateGraph {
                state: state.symbol,
                parameter_types,
                bindings,
                result_type,
                terminator,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(CheckedScalarMachineGraph {
        machine: machine.symbol,
        states,
    })
}

fn checked_binding_value(
    program: &TypedTrees,
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
    let target_machine = program.machines().iter().find(|machine| {
        program
            .machine_states(machine)
            .first()
            .is_some_and(|entry| entry.symbol == call.target_symbol)
    })?;
    Some(CheckedScalarBindingValue::DirectCall {
        target_machine: target_machine.symbol,
        target_state: call.target_symbol,
        // A supported call is the root of its local initializer. Nested calls
        // cannot acquire scalar argument plans and therefore fail closed.
        call_ordinal: 0,
        argument_count: u32::try_from(
            program
                .expression_table
                .expression_handles(call.arguments)
                .len(),
        )
        .ok()?,
    })
}

fn checked_successor(
    program: &TypedTrees,
    states: &[typed_trees::state::State],
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
    states
        .iter()
        .any(|candidate| candidate.symbol == path.symbol)
        .then_some(())?;
    Some(CheckedScalarSuccessor {
        statement_ordinal,
        is_continuation,
        target: path.symbol,
        argument_count: u32::try_from(program.statement_table.expression_handles(*arguments).len())
            .ok()?,
    })
}

fn checked_branch_destination(
    program: &TypedTrees,
    states: &[typed_trees::state::State],
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
            states,
            statement_ordinal,
            transition,
            is_continuation,
        )
        .map(CheckedScalarBranchDestination::Jump),
        _ => None,
    }
}
