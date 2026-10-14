//! A conditional return lowers into a private completion block. Value producers
//! execute before the selected exit's exact cleanup, while sibling exits keep
//! their own ownership partition. A state-wide remainder cannot describe a
//! local consumed by only one returning arm.
//! Case selection likewise leaves its private observation home live until
//! operand production finishes. Scalar edge snapshots still bind immediately;
//! nested observations and calls keep their authored order instead of being
//! hoisted across later case decisions or effects.

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
    /// These admitted arms each own a transition statement's target; a false
    /// decision does not turn that target into a source continuation. Ordered
    /// structural exits carry actual continuation identity through `guarded`.
    pub(super) fn conditional_return_arm(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        position: usize,
        state_parameters: &[StructuralParameterDeclaration],
        arm_namespace: Vec<ValueDeclaration>,
        established: &[crate::expression_preparation::bindings::structural_fields::EstablishedCasePayload],
        payload_parameters: &[ValueDeclaration],
        dispatch_discards: &[semantic_vocabulary::PlaceId],
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
            .length_observations
            .bytes
            .retain(|(_, value)| decided.contains(value));
        operations
            .length_observations
            .elements
            .retain(|(_, value)| decided.contains(value));
        operations.length_observations.field_bytes.clear();
        let mut terminator = match return_arm {
            checked_trees::CheckedConditionalReturnArm::Crash { statement_ordinal } => {
                self.crash_completion(identities, position, *statement_ordinal, next_edge)?
            }
            checked_trees::CheckedConditionalReturnArm::Unit { .. } => Terminator::ReturnUnit {
                edge: edge_id(allocate_dense(next_edge)?),
                cleanup_actions: Vec::new(),
            },
            checked_trees::CheckedConditionalReturnArm::Structural(operation) => {
                super::super::guarded::emit_return(
                    identities,
                    checked,
                    plan,
                    state,
                    super::super::cleanup::coordinate(
                        self.catalogs.cleanup_control,
                        state,
                        return_arm
                            .statement_ordinal()
                            .ok_or(LoweringError::Unsupported(
                                "return arm lost its source coordinate",
                            ))? as usize,
                        false,
                    )?,
                    operation,
                    self.catalogs,
                    state_parameters,
                    self.claims.published(),
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
                    identities,
                    checked,
                    plan.machine,
                    state,
                    &planned,
                    self.catalogs,
                    state_parameters,
                    self.claims.published(),
                    &mut arm_evaluation,
                    &mut arm_values,
                    &self.state_erased[position],
                    next_value,
                    next_block,
                    next_edge,
                    operations,
                )?;
                arm_evaluation.remap_transported_call_operands(operations);
                if let Some(terminator) = operations.take_external_completion(next_edge)? {
                    terminator
                } else {
                    let result = operation
                        .call()
                        .ok_or(LoweringError::Unsupported(
                            "Unit graph call arm lost its call result",
                        ))?
                        .result();
                    match result {
                        checked_trees::CheckedUnitCallResult::Structural { .. } => {
                            super::super::guarded::complete_structural_return(
                                identities,
                                self.claims.published(),
                                self.checked,
                                state,
                                super::super::cleanup::coordinate(
                                    self.catalogs.cleanup_control,
                                    state,
                                    return_arm.statement_ordinal().ok_or(
                                        LoweringError::Unsupported(
                                            "return arm lost its source coordinate",
                                        ),
                                    )? as usize,
                                    false,
                                )?,
                                operation,
                                self.catalogs,
                                state_parameters,
                                &mut arm_evaluation,
                                &mut arm_values,
                                next_value,
                                next_block,
                                next_edge,
                                operations,
                            )?
                        }
                        checked_trees::CheckedUnitCallResult::Unit => Terminator::ReturnUnit {
                            edge: edge_id(allocate_dense(next_edge)?),
                            cleanup_actions: Vec::new(),
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
                                cleanup_actions: Vec::new(),
                            }
                        }
                    }
                }
            }
            // The arm alone evaluates the value checking retained under its
            // `Return` role, then disposes the roots a scalar return does.
            checked_trees::CheckedConditionalReturnArm::Scalar {
                statement_ordinal, ..
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
                    identities,
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
                Terminator::Return {
                    edge: edge_id(allocate_dense(next_edge)?),
                    value: value.id,
                    cleanup_actions: Vec::new(),
                }
            }
        };
        // Resolve the selected exit only after its producer has materialized
        // the result and moved any operands. This uses the same checked
        // disposition schedule as structural returns; neither result type nor
        // a sibling arm's consumption changes the current edge's remainder.
        if matches!(
            terminator,
            terminal_psi::terminator_return_pair_pattern!(..)
        ) {
            let statement_ordinal =
                return_arm
                    .statement_ordinal()
                    .ok_or(LoweringError::Unsupported(
                        "return arm lost its source coordinate",
                    ))?;
            let exit = super::super::cleanup::coordinate(
                self.catalogs.cleanup_control,
                state,
                statement_ordinal as usize,
                false,
            )?;
            let mut regions = super::super::cleanup::for_exit_regions(
                identities,
                self.claims.published(),
                checked,
                state,
                exit,
                self.catalogs,
                state_parameters,
                &arm_evaluation,
                operations,
            )?;
            super::super::cleanup::selection(&arm_evaluation, operations, &mut regions)?;
            let temporary_discards = dispatch_discards
                .iter()
                .copied()
                .filter(|place| !regions.iter().any(|region| region.action.place() == *place))
                .map(|place| {
                    crate::unit::unit_cleanup::CleanupRegion::unconditional(
                        terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place),
                    )
                })
                .collect::<Vec<_>>();
            regions.splice(0..0, temporary_discards);
            let selected_cleanup = arm_evaluation.emit_cleanup_regions(
                identities,
                self.checked,
                regions,
                &mut arm_values,
                next_value,
                next_block,
                next_edge,
                operations,
            )?;
            match &mut terminator {
                terminal_psi::terminator_return_pair_pattern!(cleanup_actions, ..) => {
                    *cleanup_actions = selected_cleanup;
                }
                _ => {}
            }
        }
        // Case selection consumes source authority, not the storage its arm
        // still observes. Named subjects already have their exact disposition
        // in the selected exit schedule. Anonymous computed subjects have no
        // source owner row: carry their checked dispatch discard to completion
        // instead. Never replace an existing root or residual disposition;
        // operand production may have transferred part of its custody.
        match &mut terminator {
            Terminator::ReturnStructural {
                cleanup_actions, ..
            }
            | Terminator::Jump {
                cleanup_actions, ..
            } => {
                let temporary_discards = dispatch_discards
                    .iter()
                    .copied()
                    .filter(|place| {
                        !cleanup_actions
                            .iter()
                            .any(|action| action.place() == *place)
                    })
                    .map(terminal_psi::TerminalAffineCleanupAction::DiscardRoot)
                    .collect::<Vec<_>>();
                // Computed subjects are established after the source-owned
                // roots, so their disposal precedes that ordered remainder.
                cleanup_actions.splice(0..0, temporary_discards);
            }
            // A crash or an external completion abandons the activation; it
            // runs no return cleanup, so the dispatch's temporary dispositions
            // do not become normal-exit cleanup here.
            Terminator::Crash { .. } | Terminator::ExternalCompletion { .. } => {}
            terminal_psi::terminator_return_pair_pattern!(..) => {}
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
        operations.length_observations.clear();
        Ok(SuccessorEdge {
            edge: edge_id(allocate_dense(next_edge)?),
            target,
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            cleanup_actions: Vec::new(),
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
    let position = evaluation.scalar_bindings.immutable_position(slot)?;
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
