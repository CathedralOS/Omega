//! A Unit expression selects ordinary invocation completions. The subject is
//! evaluated once; only selected predicates and the selected call execute.
//! Unit never acquires a scalar slot or structural result home.

use super::StateGraphEmission;
use crate::emission::operation_emission::buffer::{OperationBuffer, scalar_result_declaration};
use crate::emission::operation_emission::integer::LoweredIntegerComparisonKind;
use crate::emission::value_successor;
use crate::shared::lowering_error::LoweringError;
use crate::shared::terminal_identities::{allocate_dense, block_id, edge_id, value_id};
use crate::unit::attached_unit::argument_evaluation::Evaluation;
use crate::unit::attached_unit::composed_control::emission::{
    FrameValues, OperationSequence, StateFrame,
};
use checked_trees::{
    CheckedScalarExpressionRole as Role, CheckedUnitCompletionExpression,
    CheckedUnitCompletionKind as Kind, CheckedUnitCompletionSelection as Selection,
    CheckedUnitScalarCompletionCondition as Condition,
};
use terminal_psi::{
    Block, StructuralParameterDeclaration, SuccessorEdge, Terminator, ValueDeclaration,
};

#[derive(Clone, Copy)]
enum Arm<'a> {
    Scalar(&'a checked_trees::CheckedUnitScalarCompletionArm, usize),
    Structural(&'a checked_trees::CheckedUnitStructuralCompletionArm),
}

impl Arm<'_> {
    fn computation(&self) -> Option<checked_trees::CheckedScalarComputationHandle> {
        match self {
            Self::Scalar(arm, _) => arm.condition.computation(),
            Self::Structural(arm) => arm.predicate,
        }
    }
    fn completion(&self) -> &CheckedUnitCompletionExpression {
        match self {
            Self::Scalar(arm, _) => &arm.completion,
            Self::Structural(arm) => &arm.completion,
        }
    }
}

impl StateGraphEmission<'_, '_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn completion_expression(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        position: usize,
        parameters: &[StructuralParameterDeclaration],
        values: Vec<ValueDeclaration>,
        established: &[crate::expression_preparation::bindings::structural_fields::EstablishedCasePayload],
        payload_parameters: &[ValueDeclaration],
        discards: &[semantic_vocabulary::PlaceId],
        plan: &checked_trees::CheckedUnitCompletionPlan,
        evaluation: &mut Evaluation,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
        sequence: &OperationSequence,
    ) -> Result<SuccessorEdge, LoweringError> {
        self.completion_node(
            identities,
            position,
            parameters,
            values,
            established,
            payload_parameters,
            discards,
            plan,
            &plan.expression,
            evaluation,
            next_value,
            next_block,
            next_edge,
            operations,
            sequence,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn completion_node(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        position: usize,
        parameters: &[StructuralParameterDeclaration],
        mut values: Vec<ValueDeclaration>,
        established: &[crate::expression_preparation::bindings::structural_fields::EstablishedCasePayload],
        payload_parameters: &[ValueDeclaration],
        discards: &[semantic_vocabulary::PlaceId],
        plan: &checked_trees::CheckedUnitCompletionPlan,
        node: &CheckedUnitCompletionExpression,
        evaluation: &mut Evaluation,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
        sequence: &OperationSequence,
    ) -> Result<SuccessorEdge, LoweringError> {
        let checked = self.checked;
        let dispatch = match &node.kind {
            Kind::Invocation(invocation) => {
                return self.conditional_return_arm(
                    identities,
                    position,
                    parameters,
                    values,
                    established,
                    payload_parameters,
                    discards,
                    &checked_trees::CheckedConditionalReturnArm::EvaluateUnit {
                        statement_ordinal: plan.statement_ordinal,
                        operations: invocation.clone(),
                    },
                    evaluation,
                    next_value,
                    next_block,
                    next_edge,
                    operations,
                    sequence,
                );
            }
            Kind::Selection(selection) => selection,
        };
        let state = &self.plan.states[position];
        let owner = self.admitted.authored[position].0.symbol;
        let entry = block_id(allocate_dense(next_block)?);
        let mut current = evaluation.branch(entry, operations.len());
        crate::expression_preparation::bindings::structural_fields::establish_case_payloads(
            &mut current.structural_fields,
            established,
        );
        current.parameters = payload_parameters.to_vec();
        let mut sequence = sequence.branch();
        let source_count = values.len();
        let (scalar, structural) = match dispatch {
            Selection::Scalar { subject, arms } => {
                let completed = self.completion_scalar(
                    identities,
                    position,
                    parameters,
                    plan.statement_ordinal,
                    Role::StructuralValueSubject {
                        expression: node.expression,
                    },
                    *subject,
                    &mut current,
                    &mut values,
                    next_value,
                    next_block,
                    next_edge,
                    operations,
                    &mut sequence,
                )?;
                values.push(completed);
                (Some(arms.as_slice()), None)
            }
            Selection::Structural {
                value, alias, arms, ..
            } => {
                let request = crate::scalar_lowering::branch_states::structural_values::ValueRequest::prepare(
                    identities, checked, *value, &self.catalogs.structural_types,
                )?;
                StateFrame::new(checked, owner, state, &self.state_erased[position]).evaluate(
                    self.catalogs,
                    parameters,
                    &mut sequence,
                    FrameValues {
                        source_value_count: values.len(),
                        evaluation: &mut current,
                        values: &mut values,
                        next_value,
                        next_block,
                        next_edge,
                        operations,
                    },
                    |frame| {
                        let (mut environment, emission) = frame.split();
                        environment.produce_value(
                            identities,
                            *value,
                            request.structural_type,
                            emission.evaluation,
                            emission.values,
                            emission.next_value,
                            emission.next_block,
                            emission.next_edge,
                            emission.calls,
                            emission.operations,
                        )
                    },
                )?;
                if operations.invocation_completion.is_none() {
                    current.bind_completed_value_alias(*value, *alias, operations)?;
                    current.completed_operand_scope.push(*value);
                }
                (None, Some(arms.as_slice()))
            }
        };
        if let Some(terminator) = operations.take_invocation_completion(next_edge)? {
            finish(&mut current, operations, terminator);
            evaluation.blocks.extend(current.blocks);
            return Ok(value_successor(edge_id(allocate_dense(next_edge)?), entry));
        }
        let arms = scalar
            .into_iter()
            .flatten()
            .map(|arm| Arm::Scalar(arm, source_count))
            .chain(structural.into_iter().flatten().map(Arm::Structural));
        let arm_count = scalar.map_or(0, <[_]>::len) + structural.map_or(0, <[_]>::len);
        for (ordinal, arm) in arms.enumerate() {
            let pattern = arm.computation();
            let condition = if let Some(pattern) = pattern {
                let pattern = self.completion_scalar(
                    identities,
                    position,
                    parameters,
                    plan.statement_ordinal,
                    Role::StructuralValuePattern {
                        source_arm: match arm {
                            Arm::Scalar(arm, _) => arm.source_arm,
                            Arm::Structural(arm) => arm.source_arm,
                        },
                    },
                    pattern,
                    &mut current,
                    &mut values,
                    next_value,
                    next_block,
                    next_edge,
                    operations,
                    &mut sequence,
                )?;
                let kind = match arm {
                    Arm::Scalar(arm, subject) => match arm.condition {
                        Condition::Boolean(_) => Some(terminal_psi::OperationKind::BooleanEqual {
                            left: values[subject].id,
                            right: pattern.id,
                        }),
                        Condition::Integer(_) => Some(
                            LoweredIntegerComparisonKind::Equal
                                .operation(values[subject].id, pattern.id),
                        ),
                        Condition::Predicate(_) | Condition::Default => None,
                    },
                    Arm::Structural(_) => None,
                };
                if let Some(kind) = kind {
                    let condition = value_id(allocate_dense(next_value)?);
                    let id = operations.allocate();
                    operations.push(terminal_psi::Operation::unsuspended(
                        id,
                        scalar_result_declaration(condition, terminal_psi::ScalarType::Boolean),
                        kind,
                    ));
                    Some(condition)
                } else {
                    Some(pattern.id)
                }
            } else {
                None
            };
            if let Some(terminator) = operations.take_invocation_completion(next_edge)? {
                finish(&mut current, operations, terminator);
                break;
            }
            let observations = operations.storage_observations.clone();
            let decision_end = operations.len();
            let mut selected_values = values.clone();
            selected_values.truncate(source_count);
            let selected = self.completion_node(
                identities,
                position,
                parameters,
                selected_values,
                &[],
                &[],
                discards,
                plan,
                arm.completion(),
                &mut current,
                next_value,
                next_block,
                next_edge,
                operations,
                &sequence,
            )?;
            operations.storage_observations = observations;
            let condition = match condition {
                Some(condition) if ordinal + 1 < arm_count => condition,
                _ => {
                    finish_at(&mut current, operations, decision_end, jump(selected));
                    break;
                }
            };
            let fallback = block_id(allocate_dense(next_block)?);
            finish_at(
                &mut current,
                operations,
                decision_end,
                Terminator::Conditional {
                    condition,
                    when_true: selected,
                    when_false: value_successor(edge_id(allocate_dense(next_edge)?), fallback),
                },
            );
            current.current = fallback;
            current.operation_start = operations.len();
            current.parameters.clear();
            current.block_structural_parameters.clear();
            operations.storage_observations.clear();
        }
        evaluation.blocks.extend(current.blocks);
        Ok(value_successor(edge_id(allocate_dense(next_edge)?), entry))
    }

    #[allow(clippy::too_many_arguments)]
    fn completion_scalar(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        position: usize,
        parameters: &[StructuralParameterDeclaration],
        statement: u32,
        role: Role,
        computation: checked_trees::CheckedScalarComputationHandle,
        evaluation: &mut Evaluation,
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
        sequence: &mut OperationSequence,
    ) -> Result<ValueDeclaration, LoweringError> {
        StateFrame::new(
            self.checked,
            self.admitted.authored[position].0.symbol,
            &self.plan.states[position],
            &self.state_erased[position],
        )
        .evaluate(
            self.catalogs,
            parameters,
            sequence,
            FrameValues {
                source_value_count: values.len(),
                evaluation,
                values,
                next_value,
                next_block,
                next_edge,
                operations,
            },
            |mut frame| {
                frame.source_value(
                    identities,
                    statement,
                    role,
                    &checked_trees::CheckedCallScalarArgument::Computation(computation),
                )
            },
        )
    }
}

fn finish(evaluation: &mut Evaluation, operations: &OperationBuffer, terminator: Terminator) {
    finish_at(evaluation, operations, operations.len(), terminator)
}

fn finish_at(
    evaluation: &mut Evaluation,
    operations: &OperationBuffer,
    end: usize,
    terminator: Terminator,
) {
    evaluation.blocks.push(Block {
        id: evaluation.current,
        parameters: std::mem::take(&mut evaluation.parameters),
        structural_parameters: std::mem::take(&mut evaluation.block_structural_parameters),
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        operations: operations[evaluation.operation_start..end].to_vec(),
        terminator,
    });
}

fn jump(edge: SuccessorEdge) -> Terminator {
    Terminator::Jump {
        edge: edge.edge,
        target: edge.target,
        arguments: edge.arguments,
        structural_arguments: edge.structural_arguments,
        erased_arguments: edge.erased_arguments,
        erased_proof_arguments: edge.erased_proof_arguments,
        cleanup_actions: edge.cleanup_actions,
    }
}
