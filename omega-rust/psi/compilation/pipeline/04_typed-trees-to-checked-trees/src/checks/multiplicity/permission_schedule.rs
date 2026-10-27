//! One source evaluation schedule for permission production and replay.
//! Custody forks after guard evaluation, and only the missed guard state
//! continues to the next statement. No expression traversal is duplicated.
use checked_trees::{FlowClaimJoinExit, FlowClaimJoinExitKind, FlowPermissionEvaluation};
use typed_trees::statement::{StatementNode, TransitionExit, TransitionGuardNode};

#[derive(Clone, Copy)]
pub(super) enum PermissionStep {
    Evaluate {
        statement: usize,
        evaluation: FlowPermissionEvaluation,
    },
    MissedGuard,
    Exit {
        coordinate: FlowClaimJoinExit,
        crash: bool,
    },
}

pub(super) fn walk<S: Clone>(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    calls: &[checked_trees::FlowCallFact],
    mut custody: S,
    mut visit: impl FnMut(PermissionStep, &mut S),
) {
    let statements = program.statement_table.statements(state.statement_nodes);
    let mut transitions = Vec::new();
    for (statement, node) in program
        .statement_table
        .admitted_execution_statements(state.statement_nodes)
    {
        let Some(transition) = node.as_transition() else {
            visit(
                PermissionStep::Evaluate {
                    statement,
                    evaluation: FlowPermissionEvaluation::Statement,
                },
                &mut custody,
            );
            if evaluation_has_no_normal_completion(
                program,
                state,
                calls,
                statement,
                FlowPermissionEvaluation::Statement,
            ) {
                return;
            }
            continue;
        };
        transitions.push(statement);
        visit(
            PermissionStep::Evaluate {
                statement,
                evaluation: FlowPermissionEvaluation::TransitionGuard,
            },
            &mut custody,
        );
        if evaluation_has_no_normal_completion(
            program,
            state,
            calls,
            statement,
            FlowPermissionEvaluation::TransitionGuard,
        ) {
            return;
        }
        for (target, kind) in [
            (transition.target, FlowClaimJoinExitKind::TransitionTarget),
            (
                transition.continuation,
                FlowClaimJoinExitKind::TransitionContinuation,
            ),
        ] {
            if !target.is_valid() {
                continue;
            }
            let mut branch = custody.clone();
            visit(
                PermissionStep::Evaluate {
                    statement,
                    evaluation: FlowPermissionEvaluation::TransitionTarget(target),
                },
                &mut branch,
            );
            if target_has_no_normal_completion(program, state, target) {
                continue;
            }
            visit(
                PermissionStep::Exit {
                    coordinate: FlowClaimJoinExit {
                        state_symbol: state.symbol,
                        statement_index: statement,
                        kind,
                    },
                    crash: matches!(transition.exit, TransitionExit::Crash(_)),
                },
                &mut branch,
            );
        }
        if transition.continuation.is_valid()
            || matches!(transition.guard, TransitionGuardNode::Always)
        {
            return;
        }
        visit(PermissionStep::MissedGuard, &mut custody);
    }
    if !transitions.is_empty()
        && super::permission_events::case_transition_run_is_exhaustive(
            program,
            state.symbol,
            statements,
            &transitions,
        )
    {
        return;
    }
    visit(
        PermissionStep::Exit {
            coordinate: FlowClaimJoinExit {
                state_symbol: state.symbol,
                statement_index: if matches!(statements.last(), Some(StatementNode::Expression(_)))
                {
                    statements.len() - 1
                } else {
                    statements.len()
                },
                kind: FlowClaimJoinExitKind::FinalExpression,
            },
            crash: false,
        },
        &mut custody,
    );
}

/// An invocation without normal completion has no normal caller successor. Its
/// argument evaluation still runs above, but neither cleanup nor a fabricated
/// consumption event may discharge the remaining frontier. Use the producer's
/// unconditional call occurrences: a call in only one selected expression arm
/// cannot erase the other arm's normal-exit obligations.
fn evaluation_has_no_normal_completion(
    program: &dyn validation::CheckingInput,
    state: &typed_trees::state::State,
    calls: &[checked_trees::FlowCallFact],
    statement: usize,
    evaluation: FlowPermissionEvaluation,
) -> bool {
    let Some(machine) = program.machine_holding_state(state.symbol) else {
        return false;
    };
    calls.iter().any(|call| {
        call.statement_index == statement
            && !call.authored_selected
            && call.authored_dispatch_role != checked_trees::FlowCallDispatchRole::SavedSubjectRead
            && crate::flow::call_permission_evaluation(
                program,
                machine,
                state,
                statement,
                call.call_ordinal,
            ) == evaluation
            && call_has_no_normal_completion(program, call.target_symbol)
    })
}

fn call_has_no_normal_completion(
    program: &dyn validation::CheckingInput,
    target: symbols::SymbolHandle,
) -> bool {
    matches!(
        validation::boundary_requirement_completion(program, target),
        semantic_vocabulary::BoundaryCompletion::External(_)
    ) || validation::call_target_state(program, target)
        .is_some_and(|state| validation::normal_completion_possible(program, state) == Some(false))
}

fn target_has_no_normal_completion(
    program: &dyn validation::CheckingInput,
    state: &typed_trees::state::State,
    target: typed_trees::statement::TransitionTargetHandle,
) -> bool {
    match program.statement_table.transition_target(target) {
        typed_trees::statement::TransitionTargetNode::Named { path, .. } => {
            let Some(machine) = program.machine_holding_state(state.symbol) else {
                return false;
            };
            // A same-activation state jump still owns its arrival transfer,
            // even if the destination can never return from the machine.
            validation::terminal_callable_target(program, machine, path.symbol).is_some()
                && call_has_no_normal_completion(program, path.symbol)
        }
        typed_trees::statement::TransitionTargetNode::Value(value) => {
            validation::expression_normal_completion_possible(program, *value) == Some(false)
        }
        typed_trees::statement::TransitionTargetNode::Terminal => false,
    }
}

/// Case exclusions at one evaluation cut. Flow already authenticated the saved
/// guard receiver, selected polarity and intervening writes. Custody consumes
/// those exact observations instead of resolving the guard's descriptor again.
pub(super) struct SelectedCases {
    inactive: Vec<validation::CanonicalPlace>,
}

impl SelectedCases {
    pub(super) fn at_step(
        program: &dyn validation::CheckingInput,
        facts: &checked_trees::CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        step: PermissionStep,
    ) -> Option<Self> {
        let flow = facts
            .flow
            .control
            .unique_state(machine.symbol, state.symbol)?;
        let (statement, constraints) = match step {
            PermissionStep::Evaluate {
                statement,
                evaluation:
                    FlowPermissionEvaluation::Statement | FlowPermissionEvaluation::TransitionGuard,
            } => {
                let mut rows = facts
                    .flow
                    .control
                    .statements
                    .span(flow.statements)?
                    .iter()
                    .filter(|row| row.statement_index == statement);
                let row = rows.next()?;
                if rows.next().is_some() {
                    return None;
                }
                (statement, row.entry_constraints)
            }
            PermissionStep::Evaluate {
                statement,
                evaluation: FlowPermissionEvaluation::TransitionTarget(target),
            } => {
                let mut rows = facts
                    .flow
                    .control
                    .outgoing
                    .span(flow.outgoing)?
                    .iter()
                    .filter(|row| {
                        row.statement_index == statement
                            && row.transition_target == target
                            && row.machine_symbol == machine.symbol
                            && row.state_symbol == state.symbol
                    });
                let row = rows.next()?;
                if rows.next().is_some() {
                    return None;
                }
                (statement, row.operand_entry_constraints)
            }
            PermissionStep::Exit { coordinate, .. }
                if coordinate.kind == FlowClaimJoinExitKind::FinalExpression =>
            {
                let mut rows = facts
                    .flow
                    .control
                    .outgoing
                    .span(flow.outgoing)?
                    .iter()
                    .filter(|row| {
                        row.statement_index == coordinate.statement_index
                            && !row.transition_target.is_valid()
                            && row.machine_symbol == machine.symbol
                            && row.state_symbol == state.symbol
                    });
                let row = rows.next()?;
                if rows.next().is_some() {
                    return None;
                }
                (coordinate.statement_index, row.constraints)
            }
            // Rejected guards live in the next statement's entry cut or the
            // implicit completion's outgoing cut. A selected target's exit
            // keeps the exclusions selected before its operands.
            _ => return None,
        };
        let contexts = facts
            .flow
            .semantic_constraint_contexts(constraints)
            .collect::<Vec<_>>();
        let evidence = crate::flow::CaseEvidence::prepare(
            program,
            &facts.semantic,
            &contexts,
            machine.symbol,
            state.symbol,
            statement,
        )?;
        Some(Self {
            inactive: evidence.inactive_owned_cases(),
        })
    }

    pub(super) fn apply_claims(&self, places: &mut [super::linear_obligations::LinearPlace]) {
        for excluded in &self.inactive {
            for place in places.iter_mut().filter(|place| {
                place.root == excluded.root && place.path.starts_with(&excluded.segments)
            }) {
                place.live = false;
                place.case_excluded = true;
            }
        }
    }

    pub(super) fn apply_cleanup(
        &self,
        remaining: &mut super::cleanup_dispositions::RemainingCustody,
    ) {
        for excluded in &self.inactive {
            remaining.exclude_absent_case(excluded);
        }
    }
}
