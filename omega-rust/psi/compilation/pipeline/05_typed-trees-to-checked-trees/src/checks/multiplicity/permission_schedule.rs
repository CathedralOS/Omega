//! One source evaluation schedule for permission production and replay.
//! Custody forks after guard evaluation, and only the missed guard state
//! continues to the next statement. No expression traversal is duplicated.
use super::cleanup_dispositions::RemainingCustody;
use super::linear_obligations::LinearPlace;
use super::permission_events::case_transition_run_is_exhaustive;
use crate::flow::CaseEvidence;
use crate::flow::call_permission_evaluation;
use checked_trees::{FlowClaimJoinExit, FlowClaimJoinExitKind, FlowPermissionEvaluation};
use std::sync::Arc;
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
        && case_transition_run_is_exhaustive(program, state.symbol, statements, &transitions)
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
            && call_permission_evaluation(program, machine, state, statement, call.call_ordinal)
                == evaluation
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
pub(super) struct SelectedCases<'program> {
    evidence: CaseEvidence<'program>,
}

impl<'program> SelectedCases<'program> {
    pub(super) fn at_step(
        program: &'program dyn validation::CheckingInput,
        facts: &'program checked_trees::CheckFacts,
        flow: &checked_trees::FlowStateFact,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        step: PermissionStep,
    ) -> Option<Self> {
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
        if constraints.is_empty() {
            return None;
        }
        let contexts = facts
            .flow
            .semantic_constraint_contexts(constraints)
            .collect::<Vec<_>>();
        if contexts.is_empty() {
            return None;
        }
        let evidence = CaseEvidence::prepare(
            program,
            &facts.semantic,
            &contexts,
            machine.symbol,
            state.symbol,
            statement,
        )?;
        // An empty emission is an inert prepared batch. All three private
        // callers only apply it and continue; no producer failure follows
        // consumer effects, and no materialized negative roster is retained.
        Some(Self { evidence })
    }

    pub(super) fn apply_claims(self, places: &mut [LinearPlace]) {
        self.apply(places, None);
    }

    pub(super) fn apply(
        self,
        places: &mut [LinearPlace],
        mut remaining: Option<&mut RemainingCustody>,
    ) {
        // No consumer exists for this pure prepared batch. Cleanup, when
        // present, must still receive every exclusion even with no claims.
        if places.is_empty() && remaining.is_none() {
            return;
        }
        self.evidence.for_each_inactive_owned_case(|excluded| {
            exclude_case(excluded, places, remaining.as_deref_mut());
        });
    }

    /// This step's exclusions, carried forward from the previous step's list.
    /// The evidence reads no custody, so the parallel preparation of the
    /// permission walk computes it ahead of the sequential custody replay that
    /// applies it. Consecutive steps of a large state usually exclude the same
    /// cases, so a list equal to the `carried` one is shared instead of copying
    /// every place again; only a step that changes the list allocates a new
    /// one, copying the unchanged prefix and then its own exclusions.
    pub(super) fn inactive_owned_cases_after(
        self,
        carried: Option<&Arc<[validation::CanonicalPlace]>>,
    ) -> Arc<[validation::CanonicalPlace]> {
        let previous = carried.map_or(&[][..], |previous| &previous[..]);
        // Exclusions matched so far against `previous`, until the first change.
        let mut matched = 0;
        let mut changed: Option<Vec<validation::CanonicalPlace>> = None;
        self.evidence.for_each_inactive_owned_case(|excluded| {
            if let Some(cases) = &mut changed {
                cases.push(excluded.clone());
            } else if previous.get(matched) == Some(excluded) {
                matched += 1;
            } else {
                let mut cases = Vec::with_capacity(previous.len().max(matched + 1));
                cases.extend_from_slice(&previous[..matched]);
                cases.push(excluded.clone());
                changed = Some(cases);
            }
        });
        match (changed, carried) {
            (Some(cases), _) => cases.into(),
            (None, Some(carried)) if matched == carried.len() => Arc::clone(carried),
            // The step excludes a strict prefix of the previous list.
            (None, _) => previous[..matched].into(),
        }
    }
}

/// Retire one inactive owned case namespace from claim and cleanup custody.
pub(super) fn exclude_case(
    excluded: &validation::CanonicalPlace,
    places: &mut [LinearPlace],
    remaining: Option<&mut RemainingCustody>,
) {
    for place in places
        .iter_mut()
        .filter(|place| place.root == excluded.root && place.path.starts_with(&excluded.segments))
    {
        place.live = false;
        place.case_excluded = true;
    }
    if let Some(remaining) = remaining {
        remaining.exclude_absent_case(excluded);
    }
}
