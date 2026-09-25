//! The authored `(expression)` arm of a conditional return: its value
//! producer lowers into a private block closed by the return, and the edge
//! into that block joins the state's conditional arms.

use super::super::super::super::super::{StructuralParameterDeclaration, SuccessorEdge, block_id};
use super::super::super::super::{
    Block, CheckedScalarExpressionRole, Terminator, ValueDeclaration, allocate_dense, edge_id,
    terminal_scalar_type, unsupported,
};
use super::super::super::LoweringError;
use super::StateGraphEmission;
use crate::emission::operation_emission::buffer::OperationBuffer;

impl StateGraphEmission<'_, '_> {
    /// Lower the return arm of a `ConditionalReturn` terminator and stage
    /// the edge the state's decision takes into it.
    pub(super) fn conditional_return_arm(
        &mut self,
        position: usize,
        state_parameters: &[StructuralParameterDeclaration],
        arm_namespace: Vec<ValueDeclaration>,
        return_arm: &checked_trees::CheckedConditionalReturnArm,
        evaluation: &mut crate::unit::attached_unit::argument_evaluation::Evaluation,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
    ) -> Result<SuccessorEdge, LoweringError> {
        let checked = self.checked;
        let plan = self.plan;
        let state = &plan.states[position];
        let target = block_id(allocate_dense(next_block)?);
        let retained_bindings = operations.structural_values.len();
        let mut arm_evaluation = evaluation.branch(target, operations.len());
        let mut arm_values = arm_namespace;
        operations.byte_lengths.clear();
        operations.field_byte_lengths.clear();
        let terminator = match return_arm {
            checked_trees::CheckedConditionalReturnArm::Structural(operation) => {
                super::super::guarded::emit_return(
                    checked,
                    plan,
                    state,
                    operation,
                    self.catalogs,
                    state_parameters,
                    &self.claims.source_claims,
                    &mut arm_evaluation,
                    &mut arm_values,
                    &self.state_erased[position],
                    next_value,
                    next_block,
                    next_edge,
                    operations,
                )?
            }
            // The arm alone evaluates the value checking retained under its
            // `Return` role, then disposes the roots a scalar return does.
            checked_trees::CheckedConditionalReturnArm::Scalar {
                statement_ordinal,
                primitive_type,
            } => {
                let role = CheckedScalarExpressionRole::Return;
                let retained = match checked.facts.values.scalar_expressions.expression_at(
                    state.state,
                    *statement_ordinal,
                    role,
                ) {
                    Some(expression) => {
                        checked_trees::CheckedCallScalarArgument::Pure(expression.clone())
                    }
                    None => checked_trees::CheckedCallScalarArgument::Computation(
                        checked
                            .facts
                            .values
                            .scalar_computations
                            .root_at(state.state, *statement_ordinal, role)
                            .ok_or(LoweringError::Unsupported(
                                "Unit graph scalar return arm lost its retained value",
                            ))?
                            .root,
                    ),
                };
                let mut calls = self.catalogs.scalar_calls.emission_context();
                let value = arm_evaluation.source_value(
                    checked,
                    plan.machine,
                    state.state,
                    *statement_ordinal,
                    role,
                    &retained,
                    arm_values.len(),
                    &mut arm_values,
                    next_value,
                    next_block,
                    next_edge,
                    operations,
                    &mut calls,
                )?;
                self.catalogs.scalar_calls.next_call_obligation = calls.next_obligation_identity;
                if value.scalar_type != terminal_scalar_type(*primitive_type)? {
                    return unsupported("Unit graph scalar return arm changed its carrier");
                }
                Terminator::Return {
                    edge: edge_id(allocate_dense(next_edge)?),
                    value: value.id,
                    cleanup_actions: super::state::return_root_discards(
                        checked,
                        plan,
                        state,
                        operations,
                        &arm_evaluation,
                        state_parameters,
                    )?
                    .into_iter()
                    .map(terminal_psi::TerminalAffineCleanupAction::DiscardRoot)
                    .collect(),
                }
            }
        };
        arm_evaluation.remap_transported_call_operands(operations);
        arm_evaluation.blocks.push(Block {
            id: arm_evaluation.current,
            parameters: arm_evaluation.parameters,
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: arm_evaluation.block_structural_parameters,
            operations: operations[arm_evaluation.operation_start..].to_vec(),
            terminator,
        });
        evaluation.blocks.extend(arm_evaluation.blocks);
        operations.structural_values.truncate(retained_bindings);
        operations.byte_lengths.clear();
        operations.field_byte_lengths.clear();
        Ok(SuccessorEdge {
            edge: edge_id(allocate_dense(next_edge)?),
            target,
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        })
    }
}
