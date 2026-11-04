//! Keep referenced data alive through the last return or call argument using it.
//! Read the body's prepared completion plan so cleanup follows its selected
//! branch and argument order.

use super::{Destination, Query, Results, Transfers, ValueReceiver, query_for_storage};
use checked_trees::{
    CheckFacts, CheckedCallScalarArgument, CheckedComposedUnitControlTerminatorPlan,
    CheckedScalarExpressionRole, CheckedStructuralScalarParameterPlan,
    CheckedUnitEffectOperationPlan, CheckedUnitScalarControlPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralReturnPlan, FlowClaimJoinExit,
};
use symbols::SymbolHandle;

mod plans;
mod successors;
use plans::{CompletionOutcome, CompletionPath, Guard};

/// Borrow the signatures already prepared for this exact graph. Structural
/// parameter positions preserve their order among the scalar arguments.
#[derive(Clone, Copy)]
pub(in crate::execution::terminal_unit) struct CompletionSignature<'a> {
    pub state: SymbolHandle,
    pub structural: &'a [CheckedUnitStructuralParameterPlan],
    pub scalar: &'a [CheckedStructuralScalarParameterPlan],
}

#[derive(Clone, Copy)]
pub(in crate::execution::terminal_unit) struct ComposedCompletion<'a> {
    pub state: SymbolHandle,
    pub terminator: &'a CheckedComposedUnitControlTerminatorPlan,
    pub selected_exit: Option<FlowClaimJoinExit>,
    pub signatures: &'a [CompletionSignature<'a>],
}

#[derive(Clone, Copy)]
pub(in crate::execution::terminal_unit) enum CompletionPlan<'a> {
    /// Ordinary bodies have already evaluated their result in the operation
    /// prefix. Any scalar control still selects its actual returning edge.
    Body {
        state: SymbolHandle,
        scalar_control: Option<&'a CheckedUnitScalarControlPlan>,
        returned: Option<&'a CheckedUnitStructuralReturnPlan>,
    },
    Composed(ComposedCompletion<'a>),
}

impl CompletionPlan<'_> {
    pub(super) fn state(self) -> SymbolHandle {
        match self {
            Self::Body { state, .. } => state,
            Self::Composed(completion) => completion.state,
        }
    }
}

/// Selected paths are prepared once for this scheduler invocation. Each holder
/// then follows the same immutable completion and its exact source exits.
pub(in crate::execution::terminal_unit) struct PreparedCompletion<'a> {
    plan: CompletionPlan<'a>,
    paths: Vec<CompletionPath<'a>>,
}

impl PreparedCompletion<'_> {
    pub(in crate::execution::terminal_unit) fn has_normal_completion(&self) -> bool {
        self.paths
            .iter()
            .any(|path| path.outcome.has_normal_completion())
    }
}

pub(in crate::execution::terminal_unit) struct CompletionTransfers {
    pub prefix: Transfers,
    pub exits: Vec<(FlowClaimJoinExit, Transfers)>,
}

pub(in crate::execution::terminal_unit) fn at_completion(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    prepared_calls: &crate::execution::terminal_unit::structural_value_calls::PreparedCalls,
    results: &Results,
    source: CheckedUnitStructuralArgumentSourcePlan,
    formation: u32,
    selection: &[CheckedUnitStructuralPathSegment],
    operations: &[CheckedUnitEffectOperationPlan],
    completion: &PreparedCompletion<'_>,
) -> Result<CompletionTransfers, &'static str> {
    let query = query_for_storage(
        program,
        facts,
        prepared_calls,
        results,
        source,
        formation,
        operations,
    );
    let (prefix, still_occupies_storage) = query
        .through_prefix(selection)
        .ok_or("completion cleanup: cannot follow this value through the body")?;
    if !still_occupies_storage {
        return Err("completion cleanup: this storage contains a later value");
    }
    let mut exits = Vec::new();
    for path in &completion.paths {
        if !path.outcome.has_normal_completion() {
            continue;
        }
        let exit = path
            .exit
            .ok_or("completion cleanup: a normal path has no exact source exit")?;
        let mut transfers = Transfers::default();
        for guard in &path.guards {
            transfers.append(match guard {
                Guard::Argument(value) => query
                    .scalar(value)
                    .ok_or("completion cleanup: a guard has no complete operand transfers")?,
                Guard::Role(ordinal) => query.scalar_role(
                    completion.plan.state(),
                    *ordinal,
                    CheckedScalarExpressionRole::Guard,
                )?,
                Guard::Subject(subject) => query.argument_computations(subject).ok_or(
                    "completion cleanup: the selected subject has no complete operand transfers",
                )?,
            });
        }
        transfers.append(query.completion_outcome(completion.plan, exit, path)?);
        exits.push((exit, transfers));
    }
    Ok(CompletionTransfers { prefix, exits })
}

impl Query<'_> {
    fn completion_outcome(
        &self,
        completion: CompletionPlan<'_>,
        exit: FlowClaimJoinExit,
        path: &CompletionPath<'_>,
    ) -> Result<Transfers, &'static str> {
        match path.outcome {
            CompletionOutcome::Prefix => Ok(Transfers::default()),
            CompletionOutcome::Structural(result) => {
                let argument = checked_trees::CheckedUnitStructuralArgumentPlan {
                    projection_evaluations: Default::default(),
                    source: result.source.clone(),
                    path: Vec::new(),
                    type_identity: result.type_identity,
                    access: checked_trees::CheckedStructuralAccess::Owned,
                };
                self.outgoing_argument(
                    &argument,
                    result.multiplicity == language_semantics::Multiplicity::Unrestricted,
                )
            }
            CompletionOutcome::Scalar => self.scalar_role(
                completion.plan.state(),
                u32::try_from(exit.statement_index)
                    .map_err(|_| "completion cleanup: return statement is out of range")?,
                if exit.kind == checked_trees::FlowClaimJoinExitKind::TransitionContinuation {
                    CheckedScalarExpressionRole::ContinuationReturn
                } else {
                    CheckedScalarExpressionRole::Return
                },
            ),
            CompletionOutcome::Operation(operation) => self.outgoing_operation(operation),
            CompletionOutcome::Arm(arm) => self.return_arm(completion.plan.state(), exit, arm),
            CompletionOutcome::Successor(successor) => {
                let CompletionPlan::Composed(graph) = completion else {
                    return Err("completion cleanup: a successor has no prepared graph signature");
                };
                self.successor_transfers(graph, successor)
            }
            CompletionOutcome::NoNormal => {
                Err("completion cleanup: an abnormal path has no cleanup successor")
            }
        }
    }

    fn scalar_role(
        &self,
        state: SymbolHandle,
        statement: u32,
        role: CheckedScalarExpressionRole,
    ) -> Result<Transfers, &'static str> {
        let mut roots = self
            .facts
            .values
            .scalar_computations
            .roots_for_statement_role(state, statement, role);
        let pure = self
            .facts
            .values
            .scalar_expressions
            .bound_expression_at(state, statement, role);
        match (roots.next(), pure) {
            (Some(root), None) if roots.next().is_none() => self
                .computation(root.root)
                .ok_or("completion cleanup: cannot follow the final expression's owned arguments"),
            (None, Some(_)) => Ok(Transfers::default()),
            (None, None) => Err("completion cleanup: final expression has no retained value"),
            _ => Err("completion cleanup: final expression has conflicting retained values"),
        }
    }

    fn outgoing_argument(
        &self,
        argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
        copied: bool,
    ) -> Result<Transfers, &'static str> {
        let mut query = self.clone();
        query.source_is_unrestricted = false;
        let mut output = query
            .argument_transfers(argument, ValueReceiver::Outgoing, &mut Vec::new())
            .ok_or("completion cleanup: the outgoing value has incomplete custody transfers")?;
        if copied || self.source_is_unrestricted {
            output.definite.clear();
            output.after_production = None;
        }
        Ok(output)
    }

    fn outgoing_operation(
        &self,
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<Transfers, &'static str> {
        // Keep copied descriptor destinations too. They do not move the
        // original slot, but they still prevent release of outgoing backing.
        let mut query = self.clone();
        query.source_is_unrestricted = false;
        let mut transfers = query
            .operation(operation, &mut Vec::new())
            .ok_or("completion cleanup: the selected value producer has incomplete transfers")?;
        if self.source_is_unrestricted {
            transfers.definite.clear();
            transfers.after_production = None;
        }
        if let Some(result) = operation.structural_result_binding() {
            for transfer in &mut transfers.possible {
                if matches!(&transfer.destination, Destination::Holder(holder, _) if holder == result)
                {
                    transfer.destination = Destination::Outgoing;
                }
            }
            if self.source
                == (CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    binding_ordinal: result.binding_ordinal,
                })
            {
                transfers.possible.push(super::Transfer {
                    source: Vec::new(),
                    destination: Destination::Outgoing,
                });
                if !self.source_is_unrestricted
                    && result.multiplicity != language_semantics::Multiplicity::Unrestricted
                {
                    transfers.definite.push(Vec::new());
                }
            }
        }
        Ok(transfers)
    }

    fn return_arm(
        &self,
        state: SymbolHandle,
        exit: FlowClaimJoinExit,
        arm: &checked_trees::CheckedConditionalReturnArm,
    ) -> Result<Transfers, &'static str> {
        use checked_trees::CheckedConditionalReturnArm as Arm;
        match arm {
            Arm::Unit { .. } => Ok(Transfers::default()),
            Arm::Scalar {
                statement_ordinal, ..
            } => self.scalar_role(
                state,
                *statement_ordinal,
                if exit.kind == checked_trees::FlowClaimJoinExitKind::TransitionContinuation {
                    CheckedScalarExpressionRole::ContinuationReturn
                } else {
                    CheckedScalarExpressionRole::Return
                },
            ),
            Arm::Structural(operation) => self.outgoing_operation(operation),
            Arm::Call {
                operands,
                operation,
                completions,
            } => {
                let mut output = self.operation_sequence(operands.iter())?;
                output.append(self.outgoing_operation(operation)?);
                output.append(self.operation_sequence(completions.iter())?);
                Ok(output)
            }
            Arm::Crash { .. } | Arm::EvaluateNoNormal { .. } => {
                Err("completion cleanup: an abnormal return arm has no cleanup successor")
            }
        }
    }

    fn operation_sequence<'a>(
        &self,
        operations: impl Iterator<Item = &'a CheckedUnitEffectOperationPlan>,
    ) -> Result<Transfers, &'static str> {
        let mut output = Transfers::default();
        for operation in operations {
            output.append(
                self.operation(operation, &mut Vec::new())
                    .ok_or("completion cleanup: an operand sequence has incomplete transfers")?,
            );
            if !operation.has_normal_continuation() {
                return Err("completion cleanup: an operand does not complete normally");
            }
        }
        Ok(output)
    }
}
