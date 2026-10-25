//! Call operations of the interpreter loop: Unit, structural and runtime
//! scalar calls that push a frame and redispatch, plus boundary
//! calls and port writes handed to the effect handler.
//!
//! `arguments.rs` prepares and validates the packed/opaque carrier partition;
//! `frames.rs` preserves that partition while binding and pushing the frame;

pub(crate) mod arguments;
pub(crate) mod frames;
pub(crate) mod target_families;

mod component_arguments;
mod component_calls;

use crate::custody::{
    bind_active_structural_arguments, bind_arguments, bind_boundary_arguments, complete_claims,
    remove_affine_root, validate_boundary_requirements,
};
use crate::effects::results::TerminalEffectResult;
use crate::effects::{TerminalEffect, TerminalEffectHandler};
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::{OperationFlow, SuspendedCall, SuspendedCallResult, TerminalExecution};
use crate::results::TerminalExecutionStatus;
use crate::runtime_rows::RuntimeRows;
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    ClaimTransfer, OperationKind, StructuralAccess, StructuralMultiplicity, TerminalMachineResult,
};

/// The rows at `positions`, in that order: one declared-position subset.
fn select_positions<T: Clone>(
    rows: &[T],
    positions: &[usize],
) -> Result<Vec<T>, TerminalInterpretError> {
    positions
        .iter()
        .map(|position| rows.get(*position).cloned().ok_or_else(malformed))
        .collect()
}

impl<'artifact> TerminalExecution<'artifact> {
    pub(crate) fn execute_call_unit(
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
            return Err(malformed());
        }
        let scalar_arguments = scalar_argument_ids
            .iter()
            .map(|argument| self.scalar_value(*argument))
            .collect::<Result<Vec<_>, _>>()?;
        let prepared_arguments =
            self.prepare_structural_call_arguments(callee, structural_arguments)?;
        self.begin_unit_call(
            callee,
            &scalar_arguments,
            structural_arguments,
            prepared_arguments,
            claim_transfers,
        )?;
        Ok(OperationFlow::Redispatch)
    }

    pub(crate) fn execute_call_structural_scalar(
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
        let result = operation.result.scalar().ok_or(malformed())?;
        let (scalar_arguments, prepared_arguments) =
            self.resolve_call_arguments(callee, arguments, structural_arguments)?;
        self.begin_structural_scalar_call(
            callee,
            result,
            &scalar_arguments,
            structural_arguments,
            prepared_arguments,
            claim_transfers,
        )?;
        Ok(OperationFlow::Redispatch)
    }

    /// A selector is the selected table's catalog index, carried as the
    /// operation's declared unsigned integer. No other operation reads it.
    pub(crate) fn execute_select_implementation(
        &mut self,
        operation: &terminal_psi::Operation,
        table: u32,
        program: &std::sync::Arc<crate::program::AdmittedTerminalProgram>,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let result = operation.result.scalar().ok_or(malformed())?;
        let Some(scalar_type) = result.scalar_type.as_integer() else {
            return Err(malformed());
        };
        if program.catalogs.dynamic_dispatch.table(table).is_none() {
            return Err(malformed());
        }
        self.values.insert(
            result.id,
            crate::values::TerminalScalarValue::Integer {
                scalar_type,
                value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(table)),
            },
        )?;
        Ok(OperationFlow::Advance)
    }

    /// Resolve the selector's table row for the interface slot, then enter
    /// that machine exactly as a direct call with the same arguments would.
    pub(crate) fn execute_call_dynamic(
        &mut self,
        operation: &terminal_psi::Operation,
        program: &std::sync::Arc<crate::program::AdmittedTerminalProgram>,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::CallDynamic {
            selector,
            interface,
            slot,
            ref arguments,
            ref family_actuals,
            ref structural_arguments,
            ref claim_transfers,
            ..
        } = operation.kind
        else {
            unreachable!("dispatched execute_call_dynamic")
        };
        let Some(crate::values::TerminalScalarValue::Integer {
            value: semantic_vocabulary::IntegerValue::Unsigned(table),
            ..
        }) = self.values.get(&selector).copied()
        else {
            return Err(TerminalInterpretError::VerifiedValueMissing(selector));
        };
        let catalog = &program.catalogs.dynamic_dispatch;
        let table = u32::try_from(table).map_err(|_| malformed())?;
        if catalog.table(table).map(|table| table.interface) != Some(interface) {
            return Err(malformed());
        }
        // A family call's `slot` names the roster's first tuple slot: scan
        // the contiguous run sharing its requirement identity and select the
        // row whose declared tuple equals the evaluated actuals. A proved
        // call always matches exactly one row; a missing row is malformed
        // input, never a fallback.
        let slot = if family_actuals.is_empty() {
            slot
        } else {
            let interface_slots = catalog
                .interface(interface)
                .map(|interface| interface.slots.as_slice())
                .ok_or(malformed())?;
            let base = interface_slots
                .get(usize::try_from(slot).map_err(|_| malformed())?)
                .ok_or(malformed())?;
            let actuals = family_actuals
                .iter()
                .map(|actual| self.family_actual_integer(*actual))
                .collect::<Result<Vec<_>, _>>()?;
            let mut matched = None;
            for (index, candidate) in interface_slots
                .iter()
                .enumerate()
                .skip(usize::try_from(slot).map_err(|_| malformed())?)
            {
                if candidate.family_tuple.is_empty() {
                    break;
                }
                if candidate.requirement_identity != base.requirement_identity {
                    break;
                }
                if candidate.family_tuple.len() == actuals.len()
                    && candidate
                        .family_tuple
                        .iter()
                        .zip(&actuals)
                        .all(|(identity, actual)| {
                            family_tuple_member_value(identity) == Some(*actual)
                        })
                {
                    matched = Some(u32::try_from(index).map_err(|_| malformed())?);
                    break;
                }
            }
            matched.ok_or(malformed())?
        };
        let callee = catalog.row(table, slot).ok_or(malformed())?;
        let (scalar_arguments, prepared_arguments) =
            self.resolve_call_arguments(callee, arguments, structural_arguments)?;
        match operation.result {
            terminal_psi::OperationResult::Unit => self.begin_unit_call(
                callee,
                &scalar_arguments,
                structural_arguments,
                prepared_arguments,
                claim_transfers,
            )?,
            terminal_psi::OperationResult::Scalar(result) => self.begin_structural_scalar_call(
                callee,
                result,
                &scalar_arguments,
                structural_arguments,
                prepared_arguments,
                claim_transfers,
            )?,
            terminal_psi::OperationResult::Structural(ref result) => self
                .begin_structural_result_call(
                    callee,
                    result.clone(),
                    &scalar_arguments,
                    structural_arguments,
                    prepared_arguments,
                    claim_transfers,
                    crate::execution::ResultClaimOrigin::CalleeReturn,
                )?,
        }
        Ok(OperationFlow::Redispatch)
    }

    pub(crate) fn execute_call_structural(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::CallStructural {
            callee,
            ref structural_arguments,
            ref claim_transfers,
            ..
        } = operation.kind
        else {
            unreachable!("dispatched execute_call_structural")
        };
        let result = operation.result.structural().cloned().ok_or(malformed())?;
        let prepared_arguments =
            self.prepare_structural_call_arguments(callee, structural_arguments)?;
        self.begin_structural_result_call(
            callee,
            result,
            &[],
            structural_arguments,
            prepared_arguments,
            claim_transfers,
            crate::execution::ResultClaimOrigin::CalleeReturn,
        )?;
        Ok(OperationFlow::Redispatch)
    }

    pub(crate) fn execute_call_structural_with_scalar_arguments(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::CallStructuralWithScalarArguments {
            callee,
            ref arguments,
            ref structural_arguments,
            ref claim_transfers,
            ..
        } = operation.kind
        else {
            unreachable!("dispatched execute_call_structural_with_scalar_arguments")
        };
        let result = operation.result.structural().cloned().ok_or(malformed())?;
        let (scalar_arguments, prepared_arguments) =
            self.resolve_call_arguments(callee, arguments, structural_arguments)?;
        self.begin_structural_result_call(
            callee,
            result,
            &scalar_arguments,
            structural_arguments,
            prepared_arguments,
            claim_transfers,
            crate::execution::ResultClaimOrigin::CalleeReturn,
        )?;
        Ok(OperationFlow::Redispatch)
    }

    pub(crate) fn execute_boundary_call(
        &mut self,
        operation: &terminal_psi::Operation,
        handler: &mut impl TerminalEffectHandler,
        meter: &mut terminal_fuel::TerminalFuelMeter,
        components: &mut crate::instance::ComponentBindings<'_, 'artifact>,
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
        // Keep the admitted declaration owner independent of mutable result
        // storage; no declaration or program contents are copied for a call.
        let program = std::sync::Arc::clone(&self.program);
        let boundary_declaration = program.verified_boundary_machine(&boundary)?;
        // A compiler primitive's completion is not a selectable assertion.
        // Until its execution binds the live extent, completed profile call
        // and actual backing, neither a checked adapter nor a host handler may
        // fulfill it. Keep this before both dispatch routes.
        if let Some(placement_operation) = boundary_declaration
            .declaration
            .as_ref()
            .and_then(|declaration| declaration.execution.placement_operation())
        {
            let plan = program
                .catalogs
                .placement_lifecycle_plans
                .iter()
                .find(|plan| plan.boundary == boundary)
                .ok_or_else(malformed)?;
            match placement_operation {
                terminal_psi::TerminalPlacementLifecycleOperation::View => {
                    self.establish_placement_view(operation, plan)?;
                    return Ok(OperationFlow::Advance);
                }
                terminal_psi::TerminalPlacementLifecycleOperation::Initialize => {
                    let [storage_argument, value, profile_argument] =
                        structural_arguments.as_slice()
                    else {
                        return Err(malformed());
                    };
                    let storage = self.resolve_live_projection(
                        storage_argument.place,
                        &storage_argument.path,
                        storage_argument.access,
                    )?;
                    let profile = self.resolve_live_projection(
                        profile_argument.place,
                        &profile_argument.path,
                        profile_argument.access,
                    )?;
                    // Resolve and normalize the exact completed supply before any
                    // write. The result alone does not establish resident contents.
                    let (binding, supplied) = self.lifecycle_profile(plan, &storage, &profile)?;
                    // Terminal byte regions use the same little-endian scalar
                    // codec as ordinary captured byte storage. Native realization
                    // supplies its target byte order independently.
                    let image = crate::provider_storage::value_image::prepare(
                        self,
                        plan,
                        value,
                        layout_plans::ByteOrder::LittleEndian,
                    )?;
                    // Dynamic rejection still owes the ordinary result carrying
                    // both original inputs. Neither outcome may publish ahead of
                    // the complete value/extent custody transaction.
                    let prepared = self.provider_storage.prepare_initial_image(
                        binding,
                        supplied,
                        &plan.placement,
                        image,
                    )?;
                    match prepared {
                        Err(rejection) => self.reject_placement_initialization(
                            operation,
                            boundary_declaration,
                            plan,
                            rejection,
                        )?,
                        Ok(prepared) => self.complete_placement_initialization(
                            operation,
                            boundary_declaration,
                            plan,
                            prepared,
                        )?,
                    }
                    return Ok(OperationFlow::Advance);
                }
                terminal_psi::TerminalPlacementLifecycleOperation::MoveOut => {
                    self.extract_placement_resident(operation, boundary_declaration, plan)?;
                    return Ok(OperationFlow::Advance);
                }
            }
        }
        let scalar_arguments = scalar_argument_ids
            .iter()
            .map(|argument| self.scalar_value(*argument))
            .collect::<Result<Vec<_>, _>>()?;
        bind_boundary_arguments(&boundary_declaration.scalar_parameters, &scalar_arguments)?;
        if program.is_target_family_head(boundary) {
            match target_families::selected_member(
                &program,
                self.target_machines.as_ref(),
                boundary,
            )? {
                terminal_psi::TargetMachineCallable::Machine(member) => {
                    return self.execute_target_family_call(
                        operation,
                        boundary_declaration,
                        member,
                        &scalar_arguments,
                    );
                }
                terminal_psi::TargetMachineCallable::Boundary(_) => {
                    // Selection identifies the opaque implementation; its
                    // execution still needs the ordinary admitted supply.
                    // Keep the portable head's contract and occurrence ID.
                }
            }
        }
        let supply = self.boundary_supplies.get(&boundary).copied();
        if let Some(crate::effects::AdmittedBoundarySupply::ComponentProvider { .. }) = supply {
            return self.execute_component_call(operation, &scalar_arguments, meter, components);
        }
        if matches!(
            supply,
            Some(crate::effects::AdmittedBoundarySupply::CheckedProvider(_))
        ) || (supply.is_none() && self.program.has_provider(&boundary))
        {
            // An owned array copies its payload into the provider's own
            // formal, exactly as an ordinary call does. The boundary admits
            // every other argument through its own declaration.
            let owned_arrays = boundary_declaration
                .structural_parameters
                .iter()
                .enumerate()
                .filter(|(_, parameter)| {
                    parameter.access == StructuralAccess::Owned
                        && terminal_semantics::scalar_array_leaf_shape(
                            self.program.structural_types().values(),
                            parameter.structural_type,
                        )
                        .is_some()
                })
                .map(|(position, _)| position)
                .collect::<Vec<_>>();
            let opaque_positions = (0..boundary_declaration.structural_parameters.len())
                .filter(|position| !owned_arrays.contains(position))
                .collect::<Vec<_>>();
            let opaque_parameters = select_positions(
                &boundary_declaration.structural_parameters,
                &opaque_positions,
            )?;
            let boundary_arguments = self.prepare_boundary_arguments(
                &opaque_parameters,
                &select_positions(structural_arguments, &opaque_positions)?,
            )?;
            bind_active_structural_arguments(
                &opaque_parameters,
                &boundary_arguments.values,
                &self.program.structural_types(),
                self.structural_cases.as_map(),
                self.account.as_ref(),
            )?;
            validate_boundary_requirements(boundary_declaration, |index| {
                opaque_positions
                    .iter()
                    .position(|position| *position == index)
                    .and_then(|position| boundary_arguments.values.get(position))
            })?;
            // Checked providers use ordinary call frames and return custody.
            // The admitted route may introduce whole-value qualifications;
            // projected qualifications must match the actual callee signature.
            // The shared frame binder checks both, including result type and
            // multiplicity. No result capability is inferred from a host value.
            let Some(crate::effects::AdmittedBoundarySupply::CheckedProvider(callee_id)) = supply
            else {
                return Err(TerminalInterpretError::ProviderInstallationMissing(
                    boundary,
                ));
            };
            let callee = self.program.verified_machine(callee_id)?;
            let mut prepared_arguments = boundary_arguments.into_call_arguments(
                &select_positions(&callee.structural_parameters, &opaque_positions)?,
                &self.program.structural_types(),
                self.structural_cases.as_map(),
                self.account.as_ref(),
            )?;
            for position in owned_arrays {
                let formal = callee
                    .structural_parameters
                    .get(position)
                    .ok_or_else(malformed)?;
                let argument = structural_arguments.get(position).ok_or_else(malformed)?;
                if formal.structural_type
                    != boundary_declaration.structural_parameters[position].structural_type
                    || prepared_arguments
                        .scalar_arrays
                        .insert(
                            formal.place,
                            self.prepare_scalar_array_argument(callee, formal, argument)?,
                        )?
                        .is_some()
                {
                    return Err(malformed());
                }
            }
            let claim_transfers = completion_receipts
                .iter()
                .map(|receipt| ClaimTransfer {
                    claim: receipt.claim,
                    argument_index: receipt.argument_index,
                    path: receipt.path.clone(),
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
                )?,
                terminal_psi::OperationResult::Structural(result) => {
                    let profile_completion = self.capture_resource_profile_call(
                        boundary,
                        callee_id,
                        structural_arguments,
                    )?;
                    self.begin_structural_result_call(
                        callee_id,
                        result.clone(),
                        &scalar_arguments,
                        structural_arguments,
                        prepared_arguments,
                        &claim_transfers,
                        crate::execution::ResultClaimOrigin::CheckedProvider,
                    )?;
                    let Some(SuspendedCall {
                        result:
                            SuspendedCallResult::Structural {
                                profile_completion: pending,
                                ..
                            },
                        ..
                    }) = self.call_stack.last_mut()
                    else {
                        return Err(malformed());
                    };
                    *pending = profile_completion;
                }
                terminal_psi::OperationResult::Scalar(result) => {
                    self.begin_structural_scalar_call(
                        callee_id,
                        *result,
                        &scalar_arguments,
                        structural_arguments,
                        prepared_arguments,
                        &claim_transfers,
                    )?;
                }
            }
            return Ok(OperationFlow::Redispatch);
        }
        if self.execute_placed_boundary(
            operation,
            boundary_declaration,
            &scalar_arguments,
            structural_arguments,
        )? {
            return Ok(OperationFlow::Advance);
        }
        let mut boundary_arguments = self.resolve_boundary_arguments(
            &boundary_declaration.structural_parameters,
            structural_arguments,
            handler.storage_arguments(boundary),
        )?;
        bind_active_structural_arguments(
            &boundary_declaration.structural_parameters,
            &boundary_arguments.values,
            &self.program.structural_types(),
            self.structural_cases.as_map(),
            self.account.as_ref(),
        )?;
        validate_boundary_requirements(boundary_declaration, |index| {
            boundary_arguments.values.get(index)
        })?;
        self.preflight_boundary_result(boundary_declaration, &operation.result)?;
        let remaining_claims = complete_claims(
            &self.live_claims,
            structural_arguments,
            completion_receipts,
            &boundary_declaration.structural_parameters,
        )?;
        // Complete ordinary storage/custody checking above before presenting
        // exact admitted abstract payloads to the host. Borrowed arguments keep
        // their original storage/descriptor route; an atom grants no loan.
        let mut exported_values = std::mem::take(&mut boundary_arguments.values);
        for (parameter, value) in boundary_declaration
            .structural_parameters
            .iter()
            .zip(&mut exported_values)
        {
            if parameter.access == StructuralAccess::Owned {
                self.export_host_atom(value)?;
            }
        }
        let effect = TerminalEffect::BoundaryCall {
            operation: operation.id,
            boundary,
            arguments: scalar_arguments,
            structural_arguments: exported_values,
            byte_sequence_arguments: std::mem::take(&mut boundary_arguments.bytes),
            completion_receipts: completion_receipts.clone(),
            result: boundary_declaration.result.clone(),
        };
        let returned = handler
            .handle_effect_with_buffers(
                &effect,
                &mut boundary_arguments.buffers,
                &mut boundary_arguments.storage_inputs.buffers,
            )
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
        boundary_arguments.validate_writeback(self)?;
        if let TerminalEffectResult::Structural(value) = &returned {
            boundary_arguments.reserve_host_result(&mut self.local_structural_identities, value)?;
        }
        boundary_arguments.commit_identity_cursor(self);
        self.commit_boundary_result(&operation.result, &boundary_declaration.result, returned)?;
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
                crate::custody::bind_result_claim(
                    &mut self.live_claims,
                    binding,
                    result.place,
                    result.multiplicity,
                )?;
            }
        }
        boundary_arguments.commit(self)?;
        let loan_completions = self.loan_completions_for_operation(operation.id)?;
        self.complete_loan_custody(&loan_completions)?;
        self.effects.push(effect);
        Ok(OperationFlow::Advance)
    }

    pub(crate) fn execute_port_write(
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
            return Err(malformed());
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

    pub(crate) fn execute_call(
        &mut self,
        operation: &terminal_psi::Operation,
        program: &std::sync::Arc<crate::program::AdmittedTerminalProgram>,
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
            .map(|argument| self.scalar_value(*argument))
            .collect::<Result<Vec<_>, _>>()?;
        let callee_id = callee;
        let callee = program.verified_machine(callee_id)?;
        if !callee.structural_parameters.is_empty()
            || !callee.entry_claims.is_empty()
            || !callee.content_entry_claims.is_empty()
            || !matches!(callee.result, TerminalMachineResult::Scalar(_))
        {
            return Err(malformed());
        }
        let values = bind_arguments(&callee.parameters, &arguments, self.account.as_ref())?;
        let caller = self.suspend_with(
            SuspendedCallResult::Scalar(operation.result.expect_scalar().id),
            Vec::new(),
        )?;
        caller.next_operation += 1;
        self.values = values;
        self.structural_values = RuntimeRows::new(self.account.as_ref());
        self.live_affine_frontier = BTreeSet::new();
        self.live_claims = BTreeMap::new();
        self.current_machine = callee_id;
        self.current = callee.entry;
        self.next_operation = 0;
        Ok(OperationFlow::Redispatch)
    }
}

impl TerminalExecution<'_> {
    /// The integer one evaluated family actual carries, for tuple matching.
    /// A roster member is a landed integer literal; any other value shape is
    /// malformed admission input, not a miss.
    fn family_actual_integer(
        &self,
        actual: semantic_vocabulary::ValueId,
    ) -> Result<i128, TerminalInterpretError> {
        match self.values.get(&actual).copied() {
            Some(crate::values::TerminalScalarValue::Integer {
                value: semantic_vocabulary::IntegerValue::Signed(value),
                ..
            }) => Ok(value),
            Some(crate::values::TerminalScalarValue::Integer {
                value: semantic_vocabulary::IntegerValue::Unsigned(value),
                ..
            }) => i128::try_from(value).map_err(|_| malformed()),
            Some(_) => Err(malformed()),
            None => Err(TerminalInterpretError::VerifiedValueMissing(actual)),
        }
    }
}

/// The integer one canonical tuple identity declares:
/// `named(integer-const(v))`. Any other canonical member shape does not
/// participate in runtime row matching yet.
fn family_tuple_member_value(identity: &str) -> Option<i128> {
    identity
        .strip_prefix("named(integer-const(")
        .and_then(|inner| inner.strip_suffix("))"))
        .and_then(|inner| inner.parse::<i128>().ok())
}
