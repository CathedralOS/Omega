//! A conditional return lowers into a private completion block. Unit uses
//! ordinary return disposal; value producers execute only on their return
//! edge. The same branch machinery preserves either arm's position.

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
        established: &[crate::expression_preparation::bindings::structural_fields::EstablishedCasePayload],
        payload_parameters: &[ValueDeclaration],
        predecessor_discards: &[semantic_vocabulary::PlaceId],
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
        crate::expression_preparation::bindings::structural_fields::establish_case_payloads(
            &mut arm_evaluation.structural_fields,
            established,
        );
        arm_evaluation.parameters = payload_parameters.to_vec();
        let mut arm_values = arm_namespace;
        // The arm's block is dominated by the state's current decision block,
        // not by the private blocks a planned short-circuit guard observes
        // lengths in. Keep only the view lengths the decision block itself
        // observed: an immutable view's extent cannot change, and reusing the
        // guard's own observation is what lets a range inside the arm
        // discharge its bound from that guard (`self.k < path.len` before
        // `path[0..self.k]`). Every other length is observed afresh; field
        // lengths always are, since a call may replace the field.
        let decided = operations[evaluation.operation_start..]
            .iter()
            .filter_map(|operation| operation.result.scalar().map(|value| value.id))
            .collect::<std::collections::BTreeSet<_>>();
        operations
            .byte_lengths
            .retain(|(_, value)| decided.contains(value));
        operations
            .element_lengths
            .retain(|(_, value)| decided.contains(value));
        operations.field_byte_lengths.clear();
        let mut terminator = match return_arm {
            checked_trees::CheckedConditionalReturnArm::Unit { .. } => Terminator::ReturnUnit {
                edge: edge_id(allocate_dense(next_edge)?),
                trivial_affine_discards: super::state::return_root_discards(
                    checked,
                    state,
                    operations,
                    &arm_evaluation,
                    state_parameters,
                )?,
            },
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
            // The selected arm evaluates its operands and invokes its call
            // once. Ordinary completion then forwards the exact result the
            // call bound, with cleanup appropriate to that result's custody.
            checked_trees::CheckedConditionalReturnArm::Call {
                operands,
                operation,
            } => {
                let planned = operands
                    .iter()
                    .chain(std::iter::once(operation))
                    .cloned()
                    .collect::<Vec<_>>();
                super::super::super::emission::emit_call_operations(
                    checked,
                    plan.machine,
                    state,
                    &planned,
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
                )?;
                let result = operation
                    .call()
                    .ok_or(LoweringError::Unsupported(
                        "Unit graph call arm lost its call result",
                    ))?
                    .result();
                match result {
                    checked_trees::CheckedUnitCallResult::Structural { .. } => {
                        super::super::guarded::complete_structural_return(
                            checked,
                            plan,
                            state,
                            operation,
                            &planned[..planned.len() - 1],
                            state_parameters,
                            &mut arm_evaluation,
                            next_edge,
                            operations,
                        )?
                    }
                    checked_trees::CheckedUnitCallResult::Unit => Terminator::ReturnUnit {
                        edge: edge_id(allocate_dense(next_edge)?),
                        trivial_affine_discards: super::state::return_root_discards(
                            checked,
                            state,
                            operations,
                            &arm_evaluation,
                            state_parameters,
                        )?,
                    },
                    checked_trees::CheckedUnitCallResult::Scalar(binding) => {
                        let value = arm_call_result(
                            binding,
                            state.scalar_parameters.len(),
                            &arm_evaluation,
                            &arm_values,
                        )?;
                        Terminator::Return {
                            edge: edge_id(allocate_dense(next_edge)?),
                            value: value.id,
                            cleanup_actions: super::state::return_root_discards(
                                checked,
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
                }
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
        // Return cleanup starts at the arriving frontier, not the dispatch's
        // pre-edge frontier. A case edge already disposed its saved subject
        // after extracting payloads. Retain the other roots in their original
        // order; Terminal independently replays the edge and exact remainder.
        match &mut terminator {
            Terminator::Return {
                cleanup_actions, ..
            } => {
                cleanup_actions.retain(|action| {
                    !matches!(
                        action,
                        terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place)
                            if predecessor_discards.contains(place)
                    )
                });
            }
            Terminator::ReturnUnit {
                trivial_affine_discards,
                ..
            }
            | Terminator::ReturnStructural {
                trivial_affine_discards,
                ..
            } => {
                trivial_affine_discards.retain(|place| !predecessor_discards.contains(place));
            }
            _ => return unsupported("return arm did not produce a completion terminator"),
        }
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
        operations.element_lengths.clear();
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

/// The value a guarded arm's terminal scalar call bound. Its checked binding
/// ordinal names the next slot of the namespace the arm inherited (the
/// state's scalar formals, then its retained bindings), and the call
/// published its result there as the arm's last value. Any other ordinal or
/// carrier is a plan that disagrees with the arm it completes.
fn arm_call_result(
    binding: &checked_trees::CheckedUnitScalarResultBindingPlan,
    scalar_parameter_count: usize,
    evaluation: &crate::unit::attached_unit::argument_evaluation::Evaluation,
    values: &[ValueDeclaration],
) -> Result<ValueDeclaration, LoweringError> {
    let slot = usize::try_from(binding.binding_ordinal)
        .ok()
        .and_then(|ordinal| ordinal.checked_add(scalar_parameter_count))
        .ok_or(LoweringError::Unsupported(
            "Unit graph call arm result ordinal overflows",
        ))?;
    let position = match &evaluation.scalar_bindings {
        Some(bindings) => bindings.immutable_position(slot)?,
        None => slot,
    };
    let value = values
        .get(position)
        .copied()
        .ok_or(LoweringError::Unsupported(
            "Unit graph call arm result binding is absent",
        ))?;
    if position + 1 != values.len()
        || value.scalar_type != terminal_scalar_type(binding.primitive_type)?
    {
        return unsupported("Unit graph call arm result is not its call's binding");
    }
    Ok(value)
}
