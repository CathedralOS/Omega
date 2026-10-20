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
    MissedGuard {
        statement: usize,
    },
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
            if ends_domain(
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
        if ends_domain(
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
            if target_ends_domain(program, target) {
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
        visit(PermissionStep::MissedGuard { statement }, &mut custody);
    }
    if !transitions.is_empty()
        && super::permission_events::case_transition_run_is_exhaustive(
            program,
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

/// An admitted external completion has no normal ownership successor. Its
/// argument evaluation still runs above, but neither cleanup nor a fabricated
/// consumption event may discharge the remaining frontier. Use the producer's
/// unconditional call occurrences: a call in only one selected expression arm
/// cannot erase the other arm's normal-exit obligations.
fn ends_domain(
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
            && external_completion(program, call.target_symbol)
    })
}

fn external_completion(
    program: &dyn validation::CheckingInput,
    target: symbols::SymbolHandle,
) -> bool {
    matches!(
        validation::boundary_requirement_completion(program, target),
        semantic_vocabulary::BoundaryCompletion::External(_)
    )
}

fn target_ends_domain(
    program: &dyn validation::CheckingInput,
    target: typed_trees::statement::TransitionTargetHandle,
) -> bool {
    match program.statement_table.transition_target(target) {
        typed_trees::statement::TransitionTargetNode::Named { path, .. } => {
            external_completion(program, path.symbol)
        }
        typed_trees::statement::TransitionTargetNode::Value(value) => program
            .expression_table
            .call_node(*value)
            .is_some_and(|call| external_completion(program, call.target_symbol)),
        typed_trees::statement::TransitionTargetNode::Terminal => false,
    }
}

/// Case alternatives narrow custody on the same chosen edge as its moves.
pub(super) fn select_cases(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    step: PermissionStep,
    places: &mut [super::linear_obligations::LinearPlace],
) {
    let (statement, target) = match step {
        PermissionStep::Evaluate {
            statement,
            evaluation: FlowPermissionEvaluation::TransitionTarget(target),
        } => (statement, Some(target)),
        PermissionStep::MissedGuard { statement } => (statement, None),
        _ => return,
    };
    let StatementNode::Transition(transition) =
        &program.statement_table.statements(state.statement_nodes)[statement]
    else {
        return;
    };
    let TransitionGuardNode::When(guard) = transition.guard else {
        return;
    };
    if target == Some(transition.target) {
        let mut positive = Vec::new();
        super::permission_events::collect_positive_case_tests(program, guard, &mut positive);
        for (subject, variant) in positive {
            super::permission_events::select_case_alternative_from_guard(
                program,
                state.symbol,
                statement,
                subject,
                variant,
                places,
            );
        }
    } else if let Some((subject, variant)) =
        super::permission_events::exact_positive_case_test(program, guard)
    {
        super::permission_events::exclude_case_alternative(
            program,
            state.symbol,
            statement,
            subject,
            variant,
            places,
        );
    }
}
