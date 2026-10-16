//! Checked scalar state facts that every Unit body shares: a state's
//! scalar terminator, every state's guarded exits, the
//! record shapes scalar computations construct inline, and each machine's
//! signature eligibility in `CheckedTerminalMachineSelections`.
//!
//! - While checked facts are built, `facts::build_check_facts` calls
//!   `build_checked_guarded_exit_plans`, which collects the ordered guarded
//!   exits and fallback of every state of every machine (`guarded_exits`),
//!   and `build_checked_terminal_machine_selections`.
//! - `execution::terminal_unit` plans each state's exit with
//!   `checked_terminator`; its statements, locals included, belong to the
//!   state's statement sequence.
//! - `execution::finalize_execution` adopts the shapes a computation
//!   constructs inline (`constructions::retain_shapes`) into the Unit type
//!   roster, so a record built only inside an expression still has its
//!   declaration.
//!
//! There is no separate scalar graph: Unit bodies are the one lowering route
//! for scalar machines, and these facts are what they read.

use checked_trees::{
    CheckedScalarBranchDestination, CheckedScalarStateTerminator,
    CheckedStructuralControlSuccessorPlan, CheckedTerminalMachineSelection,
    CheckedTerminalMachineSelections, CheckedTerminalSignatureEligibility,
};

pub(crate) fn build_checked_terminal_machine_selections(
    program: &dyn ::validation::CheckingInput,
    rank_arrivals: &checked_trees::CheckedRankArrivalPlan,
) -> CheckedTerminalMachineSelections {
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
                        && crate::execution::terminal_unit::state_graph::natural_rank_plans(
                            program, machine, rank_arrivals,
                        ).is_none())
                    // Operational ceilings do not change the value ABI.
                    // Checked suspension crossings retain their exact call,
                    // live storage and preservation demands during emission;
                    // every selected callee still needs a complete body plan.
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
                            typed_trees::service::service_requirement_declaration(
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

fn checked_successor(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    statement_ordinal: u32,
    transition: &typed_trees::statement::TableTransition,
    is_continuation: bool,
) -> Option<CheckedStructuralControlSuccessorPlan> {
    if transition.exit != TransitionExit::Ordinary {
        return None;
    }
    let TransitionTargetNode::Named { path, .. } =
        program
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
    // execution storage locator as structural forwarding. Source ranking keeps
    // recursive invocation semantics separate from this constant-stack layout.
    let target = if let Some(target_index) =
        validation::activation_target_state_index(program, machine, path.symbol)
    {
        program.machine_states(machine).get(target_index)?.symbol
    } else {
        // A free name may retain the machine head while a qualified call
        // names its entry child. Both denote the same exact first state;
        // neither grants access to a foreign machine's interior states.
        validation::find_machine_head(program, path.symbol)?
            .1
            .symbol
    };
    // Scalar terminators and the composed route share the one successor
    // plan; a scalar edge carries only the edge identity and continuation
    // selection, leaving transfer rosters to the structural path.
    Some(CheckedStructuralControlSuccessorPlan::new(
        statement_ordinal,
        target,
        is_continuation,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    ))
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
    let reachable =
        typed_trees::statement::reachable_transition_prefix(&statements[binding_count..]);
    Some(match reachable {
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
