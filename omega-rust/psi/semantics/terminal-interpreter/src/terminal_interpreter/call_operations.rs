//! Call operations of the interpreter loop: Unit, structural, dynamic and
//! runtime scalar calls that push a frame and redispatch, plus boundary
//! calls and port writes handed to the effect handler.

use crate::TerminalEffectResult;
use crate::terminal_interpreter::custody::{
    bind_arguments, bind_boundary_arguments, bind_structural_arguments, complete_claims,
    remove_affine_root, resolve_structural_arguments, validate_boundary_requirements,
};
use crate::terminal_interpreter::effect_results;
use crate::terminal_interpreter::execution::{
    ExecutableMachine, LiveClaim, OperationFlow, SuspendedCall, SuspendedCallResult,
    TerminalExecution,
};
use crate::terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalExecutionStatus, TerminalInterpretError,
};
use semantic_vocabulary::MachineId;
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    ClaimTransfer, OperationKind, StructuralAccess, StructuralMultiplicity, TerminalMachineResult,
};

impl TerminalExecution {
    pub(super) fn execute_call_unit(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::CallUnit {
            callee,
            arguments: ref scalar_argument_ids,
            ref structural_arguments,
            ref claim_transfers,
            ..
        } = operation.kind
        else {
            unreachable!("dispatched execute_call_unit")
        };
        if !matches!(operation.result, terminal_psi::OperationResult::Unit) {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let scalar_arguments = scalar_argument_ids
            .iter()
            .map(|argument| {
                self.values
                    .get(argument)
                    .copied()
                    .ok_or(TerminalInterpretError::VerifiedValueMissing(*argument))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let dynamic_parameters = self.resolve_dynamic_call_arguments(operation.id)?;
        let prepared_arguments =
            self.prepare_structural_call_arguments(callee, structural_arguments)?;
        self.begin_unit_call(
            callee,
            &scalar_arguments,
            structural_arguments,
            prepared_arguments,
            claim_transfers,
            dynamic_parameters,
        )?;
        Ok(OperationFlow::Redispatch)
    }

    pub(super) fn execute_call_structural_scalar(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::CallStructuralScalar {
            callee,
            ref arguments,
            ref structural_arguments,
            ref claim_transfers,
            ..
        } = operation.kind
        else {
            unreachable!("dispatched execute_call_structural_scalar")
        };
        let result = operation
            .result
            .scalar()
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        let dynamic_parameters = self.resolve_dynamic_call_arguments(operation.id)?;
        let scalar_arguments = arguments
            .iter()
            .map(|argument| {
                self.values
                    .get(argument)
                    .copied()
                    .ok_or(TerminalInterpretError::VerifiedValueMissing(*argument))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let prepared_arguments =
            self.prepare_structural_call_arguments(callee, structural_arguments)?;
        self.begin_structural_scalar_call(
            callee,
            result,
            &scalar_arguments,
            structural_arguments,
            prepared_arguments,
            claim_transfers,
            dynamic_parameters,
        )?;
        Ok(OperationFlow::Redispatch)
    }

    pub(super) fn execute_call_dynamic_scalar(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::CallDynamicScalar {
            descriptor_ordinal, ..
        } = operation.kind
        else {
            unreachable!("dispatched execute_call_dynamic_scalar")
        };
        let result = operation
            .result
            .scalar()
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        let (callee, source) = self
            .dynamic_scalar_calls
            .get(&(self.current_machine, descriptor_ordinal))
            .cloned()
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        let structural_arguments = std::slice::from_ref(&source);
        let prepared_arguments =
            self.prepare_structural_call_arguments(callee, structural_arguments)?;
        self.begin_structural_scalar_call(
            callee,
            result,
            &[],
            structural_arguments,
            prepared_arguments,
            &[],
            BTreeMap::new(),
        )?;
        Ok(OperationFlow::Redispatch)
    }

    pub(super) fn execute_call_dynamic_parameter_scalar(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::CallDynamicParameterScalar {
            parameter_ordinal,
            requirement_slot,
            ..
        } = operation.kind
        else {
            unreachable!("dispatched execute_call_dynamic_parameter_scalar")
        };
        let result = operation
            .result
            .scalar()
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        let descriptor = self
            .dynamic_parameters
            .get(&parameter_ordinal)
            .cloned()
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        let slot = usize::try_from(requirement_slot)
            .map_err(|_| TerminalInterpretError::VerifiedOperationMalformed)?;
        let callee = descriptor
            .callables
            .get(slot)
            .copied()
            .flatten()
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        self.begin_runtime_dynamic_scalar_call(callee, result, descriptor.source)?;
        Ok(OperationFlow::Redispatch)
    }

    pub(super) fn execute_call_dynamic_unit(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::CallDynamicUnit {
            descriptor_ordinal, ..
        } = operation.kind
        else {
            unreachable!("dispatched execute_call_dynamic_unit")
        };
        if operation.result != terminal_psi::OperationResult::Unit {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let (callee, source) = self
            .dynamic_scalar_calls
            .get(&(self.current_machine, descriptor_ordinal))
            .cloned()
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        let arguments = resolve_structural_arguments(
            &self.structural_types,
            &self.structural_values,
            std::slice::from_ref(&source),
        )?;
        let [source] = arguments.as_slice() else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        self.begin_runtime_dynamic_unit_call(callee, source.clone())?;
        Ok(OperationFlow::Redispatch)
    }

    pub(super) fn execute_call_dynamic_parameter_unit(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::CallDynamicParameterUnit {
            parameter_ordinal,
            requirement_slot,
            ..
        } = operation.kind
        else {
            unreachable!("dispatched execute_call_dynamic_parameter_unit")
        };
        if operation.result != terminal_psi::OperationResult::Unit {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let descriptor = self
            .dynamic_parameters
            .get(&parameter_ordinal)
            .cloned()
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        let slot = usize::try_from(requirement_slot)
            .map_err(|_| TerminalInterpretError::VerifiedOperationMalformed)?;
        let callee = descriptor
            .callables
            .get(slot)
            .copied()
            .flatten()
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        self.begin_runtime_dynamic_unit_call(callee, descriptor.source)?;
        Ok(OperationFlow::Redispatch)
    }

    pub(super) fn execute_call_structural(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::CallStructural {
            callee,
            ref structural_arguments,
            ref claim_transfers,
            ref returned_claim_transfers,
            ..
        } = operation.kind
        else {
            unreachable!("dispatched execute_call_structural")
        };
        let result = operation
            .result
            .structural()
            .cloned()
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        let prepared_arguments =
            self.prepare_structural_call_arguments(callee, structural_arguments)?;
        self.begin_structural_result_call(
            callee,
            result,
            &[],
            structural_arguments,
            prepared_arguments,
            claim_transfers,
            returned_claim_transfers.clone(),
        )?;
        Ok(OperationFlow::Redispatch)
    }

    pub(super) fn execute_call_structural_with_scalar_arguments(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::CallStructuralWithScalarArguments {
            callee,
            ref arguments,
            ref structural_arguments,
            ref claim_transfers,
            ref returned_claim_transfers,
            ..
        } = operation.kind
        else {
            unreachable!("dispatched execute_call_structural_with_scalar_arguments")
        };
        let result = operation
            .result
            .structural()
            .cloned()
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        let scalar_arguments = arguments
            .iter()
            .map(|argument| {
                self.values
                    .get(argument)
                    .copied()
                    .ok_or(TerminalInterpretError::VerifiedValueMissing(*argument))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let prepared_arguments =
            self.prepare_structural_call_arguments(callee, structural_arguments)?;
        self.begin_structural_result_call(
            callee,
            result,
            &scalar_arguments,
            structural_arguments,
            prepared_arguments,
            claim_transfers,
            returned_claim_transfers.clone(),
        )?;
        Ok(OperationFlow::Redispatch)
    }

    pub(super) fn execute_boundary_call(
        &mut self,
        operation: &terminal_psi::Operation,
        handler: &mut impl TerminalEffectHandler,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::BoundaryCall {
            boundary,
            arguments: ref scalar_argument_ids,
            ref structural_arguments,
            ref completion_receipts,
            ..
        } = operation.kind
        else {
            unreachable!("dispatched execute_boundary_call")
        };
        let boundary_declaration = self.boundary_machines.get(&boundary).ok_or(
            TerminalInterpretError::VerifiedBoundaryMachineMissing(boundary),
        )?;
        let scalar_arguments = scalar_argument_ids
            .iter()
            .map(|argument| {
                self.values
                    .get(argument)
                    .copied()
                    .ok_or(TerminalInterpretError::VerifiedValueMissing(*argument))
            })
            .collect::<Result<Vec<_>, _>>()?;
        bind_boundary_arguments(&boundary_declaration.scalar_parameters, &scalar_arguments)?;
        if self.provider_candidates.contains(&boundary) {
            let boundary_arguments = self.prepare_boundary_arguments(
                &boundary_declaration.structural_parameters,
                structural_arguments,
            )?;
            bind_structural_arguments(
                &boundary_declaration.structural_parameters,
                &boundary_arguments.values,
            )?;
            validate_boundary_requirements(boundary_declaration, &boundary_arguments.values)?;
            // One installed provider call shares the exact frame each result
            // form already uses for an ordinary callee. Scalar results carry
            // no structural custody to restrict: the callee's declared scalar
            // result type is checked against the operation result at entry.
            let supported_result = match &operation.result {
                terminal_psi::OperationResult::Unit | terminal_psi::OperationResult::Scalar(_) => {
                    true
                }
                terminal_psi::OperationResult::Structural(result) => {
                    result.multiplicity == StructuralMultiplicity::Affine
                        && result.qualifications.is_empty()
                        && result.projected_qualifications.is_empty()
                        && result.claims.is_empty()
                }
            };
            if !supported_result {
                return Err(TerminalInterpretError::VerifiedOperationMalformed);
            }
            let callee_id = self.provider_installation.get(&boundary).copied().ok_or(
                TerminalInterpretError::ProviderInstallationMissing(boundary),
            )?;
            let callee = self
                .machines
                .get(&callee_id)
                .ok_or(TerminalInterpretError::VerifiedCallTargetMissing(callee_id))?;
            let prepared_arguments =
                boundary_arguments.into_call_arguments(&callee.structural_parameters)?;
            let claim_transfers = completion_receipts
                .iter()
                .map(|receipt| ClaimTransfer {
                    claim: receipt.claim,
                    argument_index: receipt.argument_index,
                })
                .collect::<Vec<_>>();
            match &operation.result {
                terminal_psi::OperationResult::Unit => self.begin_unit_call(
                    callee_id,
                    // Boundary binding and installed conformance preserve
                    // this ordered scalar lane; the ordinary call binder
                    // validates it again against the selected callee.
                    &scalar_arguments,
                    structural_arguments,
                    prepared_arguments,
                    &claim_transfers,
                    BTreeMap::new(),
                )?,
                terminal_psi::OperationResult::Structural(result) => {
                    self.begin_structural_result_call(
                        callee_id,
                        result.clone(),
                        &scalar_arguments,
                        structural_arguments,
                        prepared_arguments,
                        &claim_transfers,
                        Vec::new(),
                    )?;
                }
                terminal_psi::OperationResult::Scalar(result) => {
                    self.begin_structural_scalar_call(
                        callee_id,
                        *result,
                        &scalar_arguments,
                        structural_arguments,
                        prepared_arguments,
                        &claim_transfers,
                        BTreeMap::new(),
                    )?;
                }
            }
            return Ok(OperationFlow::Redispatch);
        }
        let mut boundary_arguments = self.resolve_boundary_arguments(
            &boundary_declaration.structural_parameters,
            structural_arguments,
        )?;
        bind_structural_arguments(
            &boundary_declaration.structural_parameters,
            &boundary_arguments.values,
        )?;
        validate_boundary_requirements(boundary_declaration, &boundary_arguments.values)?;
        self.preflight_boundary_result(&operation.result)?;
        let remaining_claims = complete_claims(
            &self.live_claims,
            structural_arguments,
            completion_receipts,
            &boundary_declaration.structural_parameters,
        )?;
        let effect = TerminalEffect::BoundaryCall {
            operation: operation.id,
            boundary,
            arguments: scalar_arguments,
            structural_arguments: std::mem::take(&mut boundary_arguments.values),
            byte_sequence_arguments: std::mem::take(&mut boundary_arguments.bytes),
            completion_receipts: completion_receipts.clone(),
            result: boundary_declaration.result.clone(),
        };
        let returned = handler
            .handle_effect_with_byte_buffers(&effect, &mut boundary_arguments.buffers)
            .map_err(|rejection| TerminalInterpretError::EffectRejected {
                operation: operation.id,
                rejection,
            })?;
        if let TerminalEffectResult::Crash(cause) = returned {
            // A boundary crash belongs to the invocation, not a
            // fabricated CFG edge. Validate before publishing a
            // result, writeback, disposal, or completion receipt.
            let crash = self.admit_boundary_crash(boundary_declaration, &effect, cause)?;
            self.effects.push(effect);
            self.crash = Some(crash.clone());
            return Ok(OperationFlow::Yield(TerminalExecutionStatus::Crashed(
                crash,
            )));
        }
        boundary_arguments.validate_writeback()?;
        if let TerminalEffectResult::Structural(value) = &returned {
            self.local_structural_identities.reserve_host(value)?;
        }
        effect_results::commit_boundary_result(
            &mut self.values,
            &mut self.structural_values,
            &mut self.live_affine_frontier,
            &operation.result,
            &boundary_declaration.result,
            returned,
        )?;
        for (argument, parameter) in structural_arguments
            .iter()
            .zip(&boundary_declaration.structural_parameters)
            .filter(|(argument, parameter)| {
                argument.path.is_empty()
                    && parameter.access == StructuralAccess::Owned
                    && parameter.multiplicity != StructuralMultiplicity::Unrestricted
            })
        {
            if self.structural_values.remove(&argument.place).is_none() {
                return Err(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                    argument.place,
                ));
            }
            if parameter.multiplicity == StructuralMultiplicity::Affine {
                remove_affine_root(&mut self.live_affine_frontier, argument.place);
            }
        }
        self.live_claims = remaining_claims;
        // The boundary's claimed result re-enters the caller's claim frontier
        // at its own result place, matching the static frontier's ordering:
        // receipted claims retire before the returned bindings install.
        if let terminal_psi::OperationResult::Structural(result) = &operation.result {
            for binding in &result.claims {
                if self
                    .live_claims
                    .insert(
                        binding.claim,
                        LiveClaim {
                            place: Some(result.place),
                            path: binding.path.clone(),
                            multiplicity: Some(if binding.path.is_empty() {
                                result.multiplicity
                            } else {
                                StructuralMultiplicity::Linear
                            }),
                        },
                    )
                    .is_some()
                {
                    return Err(TerminalInterpretError::VerifiedOperationMalformed);
                }
            }
        }
        boundary_arguments.commit(self);
        self.effects.push(effect);
        Ok(OperationFlow::Advance)
    }

    pub(super) fn execute_port_write(
        &mut self,
        operation: &terminal_psi::Operation,
        handler: &mut impl TerminalEffectHandler,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::PortWrite {
            service,
            port,
            value,
        } = operation.kind
        else {
            unreachable!("dispatched execute_port_write")
        };
        if !matches!(operation.result, terminal_psi::OperationResult::Unit) {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let effect = TerminalEffect::PortWrite {
            operation: operation.id,
            service,
            port,
            value,
        };
        handler.handle_effect(&effect).map_err(|rejection| {
            TerminalInterpretError::EffectRejected {
                operation: operation.id,
                rejection,
            }
        })?;
        self.effects.push(effect);
        Ok(OperationFlow::Advance)
    }

    pub(super) fn execute_call(
        &mut self,
        operation: &terminal_psi::Operation,
        machines: &std::sync::Arc<BTreeMap<MachineId, ExecutableMachine>>,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::Call {
            callee,
            ref arguments,
            ..
        } = operation.kind
        else {
            unreachable!("dispatched execute_call")
        };
        let arguments = arguments
            .iter()
            .map(|argument| {
                self.values
                    .get(argument)
                    .copied()
                    .ok_or(TerminalInterpretError::VerifiedValueMissing(*argument))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let callee_id = callee;
        let callee = machines
            .get(&callee_id)
            .ok_or(TerminalInterpretError::VerifiedCallTargetMissing(callee_id))?;
        if !callee.structural_parameters.is_empty()
            || !callee.entry_claims.is_empty()
            || !callee.content_entry_claims.is_empty()
            || !matches!(callee.result, TerminalMachineResult::Scalar(_))
        {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let values = bind_arguments(&callee.parameters, &arguments)?;
        self.next_operation += 1;
        self.call_stack.push(SuspendedCall {
            values: std::mem::take(&mut self.values),
            structural_values: std::mem::take(&mut self.structural_values),
            byte_sequence_values: std::mem::take(&mut self.byte_sequence_values),
            scalar_case_values: std::mem::take(&mut self.scalar_case_values),
            scalar_array_values: std::mem::take(&mut self.scalar_array_values),
            live_affine_frontier: std::mem::take(&mut self.live_affine_frontier),
            live_claims: std::mem::take(&mut self.live_claims),
            dynamic_parameters: std::mem::take(&mut self.dynamic_parameters),
            current_machine: self.current_machine,
            current: self.current,
            next_operation: self.next_operation,
            result: SuspendedCallResult::Scalar(operation.result.expect_scalar().id),
        });
        self.values = values;
        self.structural_values = BTreeMap::new();
        self.live_affine_frontier = BTreeSet::new();
        self.live_claims = BTreeMap::new();
        self.dynamic_parameters = BTreeMap::new();
        self.current_machine = callee_id;
        self.current = callee.entry;
        self.next_operation = 0;
        Ok(OperationFlow::Redispatch)
    }
}
