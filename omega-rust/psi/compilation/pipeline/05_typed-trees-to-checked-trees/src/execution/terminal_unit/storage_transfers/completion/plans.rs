//! Follow each selected branch in source order. Cleanup for one branch must
//! not include operands belonging to another.
use super::{CompletionPlan, ComposedCompletion, PreparedCompletion};
use checked_trees::{
    CheckFacts, CheckedCallScalarArgument, CheckedComposedUnitControlTerminatorPlan as Terminator,
    CheckedConditionalReturnArm, CheckedGuardedJumpArmPlan, CheckedGuardedJumpFallbackPlan,
    CheckedScalarBranchDestination, CheckedScalarReturnPlan, CheckedStructuralControlSuccessorPlan,
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralReturnPlan, FlowClaimJoinExit, FlowClaimJoinExitKind,
};
use symbols::SymbolHandle;

#[derive(Clone, Copy)]
pub(super) enum Guard<'a> {
    Argument(&'a CheckedCallScalarArgument),
    Role(u32),
    Subject(&'a CheckedUnitStructuralArgumentPlan),
}

#[derive(Clone, Copy)]
pub(super) enum CompletionOutcome<'a> {
    Prefix,
    Scalar,
    Structural(&'a CheckedUnitStructuralReturnPlan),
    Operation(&'a CheckedUnitEffectOperationPlan),
    Arm(&'a CheckedConditionalReturnArm),
    Successor(&'a CheckedStructuralControlSuccessorPlan),
    NoNormal,
}

impl CompletionOutcome<'_> {
    pub(super) fn has_normal_completion(self) -> bool {
        match self {
            Self::NoNormal => false,
            Self::Arm(arm) => arm.has_normal_continuation(),
            Self::Successor(successor) => successor.has_normal_caller_completion(),
            Self::Operation(operation) => operation.has_normal_continuation(),
            _ => true,
        }
    }
}

pub(super) struct CompletionPath<'a> {
    pub exit: Option<FlowClaimJoinExit>,
    pub guards: Vec<Guard<'a>>,
    pub outcome: CompletionOutcome<'a>,
}

fn edge(state: SymbolHandle, ordinal: u32, continuation: bool) -> FlowClaimJoinExit {
    FlowClaimJoinExit {
        state_symbol: state,
        statement_index: ordinal as usize,
        kind: if continuation {
            FlowClaimJoinExitKind::TransitionContinuation
        } else {
            FlowClaimJoinExitKind::TransitionTarget
        },
    }
}

fn successor_path<'a>(
    state: SymbolHandle,
    successor: &'a CheckedStructuralControlSuccessorPlan,
    guards: &[Guard<'a>],
) -> CompletionPath<'a> {
    CompletionPath {
        exit: Some(edge(
            state,
            successor.statement_ordinal,
            successor.is_continuation,
        )),
        guards: guards.to_vec(),
        outcome: CompletionOutcome::Successor(successor),
    }
}

fn return_path<'a>(
    state: SymbolHandle,
    arm: &'a CheckedConditionalReturnArm,
    continuation: bool,
    guards: &[Guard<'a>],
) -> Result<CompletionPath<'a>, &'static str> {
    let ordinal = arm
        .statement_ordinal()
        .ok_or("completion cleanup: a return arm has no source coordinate")?;
    Ok(CompletionPath {
        exit: Some(edge(state, ordinal, continuation)),
        guards: guards.to_vec(),
        outcome: CompletionOutcome::Arm(arm),
    })
}

impl<'a> CompletionPlan<'a> {
    pub(in crate::execution::terminal_unit) fn prepare<'context>(
        self,
        program: &dyn validation::CheckingInput,
        facts: &'context CheckFacts,
        machine: SymbolHandle,
        state: &typed_trees::state::State,
    ) -> Result<PreparedCompletion<'context>, &'static str>
    where
        'a: 'context,
    {
        let plan: CompletionPlan<'context> = self;
        Ok(PreparedCompletion {
            plan,
            paths: plan.paths(program, facts, machine, state)?,
        })
    }

    pub(super) fn paths(
        self,
        program: &dyn validation::CheckingInput,
        facts: &'a CheckFacts,
        machine: SymbolHandle,
        state: &typed_trees::state::State,
    ) -> Result<Vec<CompletionPath<'a>>, &'static str> {
        match self {
            Self::Body {
                scalar_control: Some(control),
                ..
            } => scalar_paths(program, facts, machine, state, &control.terminator),
            Self::Body { returned, .. } => direct_paths(
                program,
                facts,
                machine,
                state,
                returned.map_or(CompletionOutcome::Prefix, CompletionOutcome::Structural),
            ),
            Self::Composed(completion) => {
                let mut paths = completion.paths(program, facts, machine, state)?;
                if let Some(selected) = completion.selected_exit {
                    paths.retain(|path| path.exit == Some(selected));
                    if paths.is_empty() {
                        return Err(
                            "completion cleanup: the selected exit is absent from its completion plan",
                        );
                    }
                }
                Ok(paths)
            }
        }
    }
}

fn direct_paths<'a>(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    outcome: CompletionOutcome<'a>,
) -> Result<Vec<CompletionPath<'a>>, &'static str> {
    let exits = crate::execution::terminal_unit::cleanup::normal_edges::state_exits(
        program, facts, machine, state,
    )?;
    if exits.is_empty() {
        return Err("completion cleanup: the body has no checked normal completion");
    }
    Ok(exits
        .into_iter()
        .map(|exit| CompletionPath {
            exit: Some(exit),
            guards: Vec::new(),
            outcome,
        })
        .collect())
}

fn scalar_paths<'a>(
    program: &dyn validation::CheckingInput,
    facts: &'a CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    terminator: &checked_trees::CheckedScalarStateTerminator,
) -> Result<Vec<CompletionPath<'a>>, &'static str> {
    if let checked_trees::CheckedScalarStateTerminator::Crash { statement_ordinal } = terminator {
        return Ok(vec![CompletionPath {
            exit: Some(edge(state.symbol, *statement_ordinal, false)),
            guards: Vec::new(),
            outcome: CompletionOutcome::NoNormal,
        }]);
    }
    let paths = terminator.returning_paths(&facts.flow.guarded_exits)?;
    let exits = crate::execution::terminal_unit::cleanup::normal_edges::state_exits(
        program, facts, machine, state,
    )?;
    paths
        .iter()
        .map(|path| {
            let CheckedScalarBranchDestination::Return {
                statement_ordinal,
                is_continuation,
            } = path.destination
            else {
                return Err(
                    "completion cleanup: a scalar return has no exact returning destination",
                );
            };
            let mut matching = exits.iter().filter(|exit| {
                exit.statement_index == statement_ordinal as usize
                    && if is_continuation {
                        exit.kind == FlowClaimJoinExitKind::TransitionContinuation
                    } else {
                        matches!(
                            exit.kind,
                            FlowClaimJoinExitKind::TransitionTarget
                                | FlowClaimJoinExitKind::FinalExpression
                        )
                    }
            });
            let exit = *matching
                .next()
                .ok_or("completion cleanup: the scalar return has no checked source exit")?;
            if matching.next().is_some() {
                return Err("completion cleanup: the scalar return has ambiguous source exits");
            }
            Ok(CompletionPath {
                exit: Some(exit),
                guards: path.guard_ordinals().map(Guard::Role).collect(),
                outcome: CompletionOutcome::Scalar,
            })
        })
        .collect()
}

impl<'a> ComposedCompletion<'a> {
    fn paths(
        self,
        program: &dyn validation::CheckingInput,
        facts: &'a CheckFacts,
        machine: SymbolHandle,
        state: &typed_trees::state::State,
    ) -> Result<Vec<CompletionPath<'a>>, &'static str> {
        let mut output = Vec::new();
        match self.terminator {
            Terminator::InvokeNoNormal { .. }
            | Terminator::EvaluateNoNormal
            | Terminator::Crash { .. } => {
                output.push(CompletionPath {
                    exit: self.selected_exit,
                    guards: Vec::new(),
                    outcome: CompletionOutcome::NoNormal,
                });
            }
            Terminator::ReturnUnit
            | Terminator::ReturnScalar {
                completion: CheckedScalarReturnPlan::Binding(_),
            } => {
                output = direct_paths(program, facts, machine, state, CompletionOutcome::Prefix)?;
            }
            Terminator::ReturnStructural { result } => {
                output = direct_paths(
                    program,
                    facts,
                    machine,
                    state,
                    CompletionOutcome::Structural(result),
                )?;
            }
            Terminator::ReturnScalar {
                completion: CheckedScalarReturnPlan::Exits(control),
            } => {
                output = scalar_paths(program, facts, machine, state, &control.terminator)?;
            }
            Terminator::Jump { successor } => {
                output.push(successor_path(self.state, successor, &[]))
            }
            Terminator::Conditional {
                guard,
                when_true,
                when_false,
            } => {
                let guards = [Guard::Argument(guard)];
                output.push(successor_path(self.state, when_true, &guards));
                output.push(successor_path(self.state, when_false, &guards));
            }
            Terminator::ConditionalReturn {
                guard,
                jump,
                return_arm,
                ..
            } => {
                let guards = [Guard::Argument(guard)];
                output.push(successor_path(self.state, jump, &guards));
                output.push(return_path(self.state, return_arm, false, &guards)?);
            }
            Terminator::ConditionalCompletion {
                guard,
                when_true,
                when_false,
            } => {
                let guards = [Guard::Argument(guard)];
                output.push(return_path(self.state, when_true, false, &guards)?);
                output.push(return_path(self.state, when_false, false, &guards)?);
            }
            Terminator::GuardedJumps { arms, fallback } => {
                let mut guards = Vec::new();
                for arm in arms {
                    guards.push(Guard::Argument(arm.guard()));
                    output.push(match arm {
                        CheckedGuardedJumpArmPlan::Jump(plan) => {
                            successor_path(self.state, &plan.successor, &guards)
                        }
                        CheckedGuardedJumpArmPlan::Return(plan) => {
                            return_path(self.state, &plan.return_arm, false, &guards)?
                        }
                    });
                }
                output.push(match fallback {
                    CheckedGuardedJumpFallbackPlan::Successor(successor) => {
                        successor_path(self.state, successor, &guards)
                    }
                    CheckedGuardedJumpFallbackPlan::Return(arm) => {
                        return_path(self.state, arm, false, &guards)?
                    }
                });
            }
            Terminator::Guarded {
                arms,
                fallback,
                return_values,
            } => {
                let arms = facts
                    .flow
                    .guarded_exits
                    .exits
                    .span(*arms)
                    .ok_or("completion cleanup: the ordered guard roster is unavailable")?;
                if arms.is_empty() {
                    return Err("completion cleanup: the ordered guard roster is empty");
                }
                let mut guards = Vec::new();
                if return_values.len() != arms.len() + usize::from(fallback.is_some()) {
                    return Err(
                        "completion cleanup: the ordered return values do not match their selected arms",
                    );
                }
                for (index, arm) in arms.iter().enumerate() {
                    guards.push(Guard::Role(arm.guard_statement_ordinal));
                    output.push(self.scalar_destination(
                        &arm.destination,
                        &guards,
                        &return_values[index],
                    )?);
                }
                if let Some(fallback) = fallback {
                    output.push(self.scalar_destination(
                        fallback,
                        &guards,
                        &return_values[arms.len()],
                    )?);
                }
            }
            Terminator::ClosedSum { subject, cases, .. } => {
                if cases.is_empty() {
                    return Err("completion cleanup: the case dispatch has no arms");
                }
                let guards = [Guard::Subject(subject)];
                for case in cases {
                    output.push(match &case.outcome {
                        checked_trees::CheckedClosedSumCaseOutcomePlan::Successor(successor) => {
                            successor_path(self.state, successor, &guards)
                        }
                        checked_trees::CheckedClosedSumCaseOutcomePlan::Return(arm) => {
                            return_path(self.state, arm, false, &guards)?
                        }
                    });
                }
            }
        }
        Ok(output)
    }

    fn scalar_destination(
        self,
        destination: &'a CheckedScalarBranchDestination,
        guards: &[Guard<'a>],
        operation: &'a CheckedUnitEffectOperationPlan,
    ) -> Result<CompletionPath<'a>, &'static str> {
        match destination {
            CheckedScalarBranchDestination::Jump(successor) => {
                Ok(successor_path(self.state, successor, guards))
            }
            CheckedScalarBranchDestination::Return {
                statement_ordinal,
                is_continuation,
            } => {
                let outcome = CompletionOutcome::Operation(operation);
                Ok(CompletionPath {
                    exit: Some(edge(self.state, *statement_ordinal, *is_continuation)),
                    guards: guards.to_vec(),
                    outcome,
                })
            }
            CheckedScalarBranchDestination::Crash { statement_ordinal } => Ok(CompletionPath {
                exit: Some(edge(self.state, *statement_ordinal, false)),
                guards: guards.to_vec(),
                outcome: CompletionOutcome::NoNormal,
            }),
            CheckedScalarBranchDestination::InvokeNoNormal {
                statement_ordinal,
                is_continuation,
            } => Ok(CompletionPath {
                exit: Some(edge(self.state, *statement_ordinal, *is_continuation)),
                guards: guards.to_vec(),
                outcome: CompletionOutcome::NoNormal,
            }),
        }
    }
}
