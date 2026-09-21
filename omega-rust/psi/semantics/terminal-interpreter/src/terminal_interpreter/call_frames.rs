use super::{
    TerminalExecution, TerminalInterpretError, TerminalScalarValue, TerminalStructuralValue,
    scalar_case_arguments,
};
use crate::terminal_interpreter::byte_sequence_binding::ByteSequenceBinding;
use crate::terminal_interpreter::byte_sequence_binding::StructuralCallArguments;
use crate::terminal_interpreter::custody::bind_affine_frontier;
use crate::terminal_interpreter::custody::bind_arguments;
use crate::terminal_interpreter::custody::bind_structural_arguments;
use crate::terminal_interpreter::custody::resolve_structural_arguments;
use crate::terminal_interpreter::custody::transfer_claims;
use crate::terminal_interpreter::execution::RuntimeDynamicDescriptor;
use crate::terminal_interpreter::execution::SuspendedCall;
use crate::terminal_interpreter::execution::SuspendedCallResult;
use semantic_vocabulary::{MachineId, OperationId, PlaceId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    ClaimTransfer, StructuralAccess, StructuralArgument, StructuralMultiplicity,
    StructuralOperationResult, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralResultClaimTransfer, StructuralTypeShape, TerminalMachineResult,
};

impl TerminalExecution {
    pub(super) fn resolve_dynamic_call_arguments(
        &self,
        operation: OperationId,
    ) -> Result<BTreeMap<u32, RuntimeDynamicDescriptor>, TerminalInterpretError> {
        let mut resolved = BTreeMap::new();
        for argument in self
            .dynamic_descriptor_arguments
            .get(&(self.current_machine, operation))
            .into_iter()
            .flatten()
        {
            let descriptor = match argument.source {
                terminal_psi::TerminalDynamicDescriptorSource::Selection { ordinal } => {
                    let template = self
                        .dynamic_selection_templates
                        .get(&(self.current_machine, ordinal))
                        .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                    let sources = resolve_structural_arguments(
                        &self.structural_types,
                        &self.structural_values,
                        std::slice::from_ref(&template.source),
                    )?;
                    let [source] = sources.as_slice() else {
                        return Err(TerminalInterpretError::VerifiedOperationMalformed);
                    };
                    RuntimeDynamicDescriptor {
                        source: source.clone(),
                        callables: template.callables.clone(),
                    }
                }
                terminal_psi::TerminalDynamicDescriptorSource::ReboundDescriptor { ordinal } => {
                    let template = self
                        .dynamic_descriptor_templates
                        .get(&(self.current_machine, ordinal))
                        .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
                    let sources = resolve_structural_arguments(
                        &self.structural_types,
                        &self.structural_values,
                        std::slice::from_ref(&template.source),
                    )?;
                    let [source] = sources.as_slice() else {
                        return Err(TerminalInterpretError::VerifiedOperationMalformed);
                    };
                    RuntimeDynamicDescriptor {
                        source: source.clone(),
                        callables: template.callables.clone(),
                    }
                }
                terminal_psi::TerminalDynamicDescriptorSource::Parameter { ordinal } => self
                    .dynamic_parameters
                    .get(&ordinal)
                    .cloned()
                    .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?,
            };
            if resolved
                .insert(argument.parameter_ordinal, descriptor)
                .is_some()
            {
                return Err(TerminalInterpretError::VerifiedOperationMalformed);
            }
        }
        Ok(resolved)
    }

    /// Rebind whole existing byte arguments, without converting inline storage.
    pub(super) fn bind_byte_sequence_arguments(
        &self,
        parameters: &[StructuralParameterDeclaration],
        arguments: &[StructuralArgument],
        resolved_arguments: &[TerminalStructuralValue],
    ) -> Result<BTreeMap<PlaceId, ByteSequenceBinding>, TerminalInterpretError> {
        if parameters.len() != arguments.len() || parameters.len() != resolved_arguments.len() {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let mut values = BTreeMap::new();
        for ((parameter, argument), resolved) in
            parameters.iter().zip(arguments).zip(resolved_arguments)
        {
            let declaration = self
                .structural_types
                .get(&parameter.structural_type)
                .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
            let StructuralTypeShape::ByteSequence(carrier) = &declaration.shape else {
                continue;
            };
            if *carrier != terminal_psi::ByteSequenceCarrier::BorrowedView
                || parameter.structural_type != resolved.structural_type
                || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                || parameter.access != argument.access
                || !matches!(
                    parameter.access,
                    StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                )
                || (parameter.access == StructuralAccess::SharedBorrow && !resolved.path.is_empty())
                || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
                || !resolved.qualifications.is_empty()
                || !argument.path.is_empty()
            {
                return Err(TerminalInterpretError::VerifiedOperationMalformed);
            }
            let bytes = self.byte_sequence_values.get(&argument.place).ok_or(
                TerminalInterpretError::VerifiedStructuralPlaceMissing(argument.place),
            )?;
            match bytes {
                ByteSequenceBinding::Immutable(_) => {
                    if parameter.access != StructuralAccess::SharedBorrow {
                        return Err(TerminalInterpretError::VerifiedOperationMalformed);
                    }
                }
                ByteSequenceBinding::MutableField { .. }
                | ByteSequenceBinding::MutableArray { .. } => {
                    if parameter.access != StructuralAccess::MutableBorrow {
                        return Err(TerminalInterpretError::VerifiedOperationMalformed);
                    }
                    bytes.validate_mutable_referent(&self.structural_types, resolved)?;
                }
            }
            if values.insert(parameter.place, bytes.clone()).is_some() {
                return Err(TerminalInterpretError::VerifiedOperationMalformed);
            }
        }
        Ok(values)
    }

    /// Enter one structural Unit callee after the operation-specific argument
    /// checks have succeeded. Ordinary calls and admitted provider dispatch
    /// share this exact ownership and continuation transition.
    pub(super) fn begin_unit_call(
        &mut self,
        callee_id: MachineId,
        scalar_arguments: &[TerminalScalarValue],
        structural_arguments: &[StructuralArgument],
        prepared_arguments: StructuralCallArguments,
        claim_transfers: &[ClaimTransfer],
        dynamic_parameters: BTreeMap<u32, RuntimeDynamicDescriptor>,
    ) -> Result<(), TerminalInterpretError> {
        let machines = std::sync::Arc::clone(&self.machines);
        let callee = machines
            .get(&callee_id)
            .ok_or(TerminalInterpretError::VerifiedCallTargetMissing(callee_id))?;
        if callee.result != TerminalMachineResult::Unit {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let values = bind_arguments(&callee.parameters, scalar_arguments)?;
        let mut structural_values = prepared_arguments.values;
        self.copy_owned_record_arguments(&callee.structural_parameters, &mut structural_values)?;
        let byte_sequence_values = prepared_arguments.byte_sequences;
        let callee_affine_frontier = scalar_case_arguments::bind_call_affine_frontier(
            &callee.structural_parameters,
            &structural_values,
            &prepared_arguments.scalar_cases,
        )?;
        let (remaining_claims, live_claims) = transfer_claims(
            &self.live_claims,
            &self.structural_values,
            structural_arguments,
            claim_transfers,
            &callee.structural_parameters,
            &callee.entry_claims,
            &callee.content_entry_claims,
            &structural_values,
        )?;
        self.next_operation += 1;
        self.live_claims = remaining_claims;
        let mut caller_affine_frontier = std::mem::take(&mut self.live_affine_frontier);
        for (argument, parameter) in structural_arguments
            .iter()
            .zip(&callee.structural_parameters)
        {
            if parameter.multiplicity == StructuralMultiplicity::Affine
                && argument.access == StructuralAccess::Owned
            {
                self.consume_affine_call_argument(&mut caller_affine_frontier, argument)?;
            }
        }
        let mut caller_structural_values = std::mem::take(&mut self.structural_values);
        let mut caller_scalar_case_values = std::mem::take(&mut self.scalar_case_values);
        for (argument, _parameter) in structural_arguments
            .iter()
            .zip(&callee.structural_parameters)
            .filter(|(argument, parameter)| {
                argument.path.is_empty()
                    && parameter.multiplicity != StructuralMultiplicity::Unrestricted
            })
        {
            if caller_structural_values.remove(&argument.place).is_none()
                && caller_scalar_case_values.remove(&argument.place).is_none()
            {
                return Err(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                    argument.place,
                ));
            }
        }
        self.call_stack.push(SuspendedCall {
            values: std::mem::take(&mut self.values),
            structural_values: caller_structural_values,
            byte_sequence_values: std::mem::take(&mut self.byte_sequence_values),
            scalar_case_values: caller_scalar_case_values,
            scalar_array_values: std::mem::take(&mut self.scalar_array_values),
            live_affine_frontier: caller_affine_frontier,
            live_claims: std::mem::take(&mut self.live_claims),
            dynamic_parameters: std::mem::take(&mut self.dynamic_parameters),
            current_machine: self.current_machine,
            current: self.current,
            next_operation: self.next_operation,
            result: SuspendedCallResult::Unit,
        });
        self.values = values;
        self.structural_values = structural_values;
        self.byte_sequence_values = byte_sequence_values;
        self.scalar_array_values = prepared_arguments.scalar_arrays;
        self.scalar_case_values = prepared_arguments.scalar_cases;
        self.live_affine_frontier = callee_affine_frontier;
        self.live_claims = live_claims;
        self.dynamic_parameters = dynamic_parameters;
        self.current_machine = callee_id;
        self.current = callee.entry;
        self.next_operation = 0;
        Ok(())
    }

    /// Enter one scalar-result callee after the operation-specific argument
    /// checks have succeeded. Ordinary and dynamic calls prepare their
    /// arguments against the callee signature directly; admitted provider
    /// dispatch passes the boundary-prepared arguments converted by
    /// `BoundaryArguments::into_call_arguments`, the same custody conversion
    /// the Unit and structural provider results use.
    pub(super) fn begin_structural_scalar_call(
        &mut self,
        callee_id: MachineId,
        result: terminal_psi::ValueDeclaration,
        scalar_arguments: &[TerminalScalarValue],
        structural_arguments: &[StructuralArgument],
        prepared_arguments: StructuralCallArguments,
        claim_transfers: &[ClaimTransfer],
        dynamic_parameters: BTreeMap<u32, RuntimeDynamicDescriptor>,
    ) -> Result<(), TerminalInterpretError> {
        let machines = std::sync::Arc::clone(&self.machines);
        let callee = machines
            .get(&callee_id)
            .ok_or(TerminalInterpretError::VerifiedCallTargetMissing(callee_id))?;
        if callee.result.scalar().map(|result| result.scalar_type) != Some(result.scalar_type) {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let values = bind_arguments(&callee.parameters, scalar_arguments)?;
        let mut structural_values = prepared_arguments.values;
        self.copy_owned_record_arguments(&callee.structural_parameters, &mut structural_values)?;
        let byte_sequence_values = prepared_arguments.byte_sequences;
        let callee_affine_frontier = scalar_case_arguments::bind_call_affine_frontier(
            &callee.structural_parameters,
            &structural_values,
            &prepared_arguments.scalar_cases,
        )?;
        let (remaining_claims, live_claims) = transfer_claims(
            &self.live_claims,
            &self.structural_values,
            structural_arguments,
            claim_transfers,
            &callee.structural_parameters,
            &callee.entry_claims,
            &callee.content_entry_claims,
            &structural_values,
        )?;
        self.next_operation += 1;
        self.live_claims = remaining_claims;
        let mut caller_affine_frontier = std::mem::take(&mut self.live_affine_frontier);
        for (argument, parameter) in structural_arguments
            .iter()
            .zip(&callee.structural_parameters)
        {
            if parameter.multiplicity == StructuralMultiplicity::Affine
                && argument.access == StructuralAccess::Owned
            {
                self.consume_affine_call_argument(&mut caller_affine_frontier, argument)?;
            }
        }
        let mut caller_structural_values = std::mem::take(&mut self.structural_values);
        let mut caller_scalar_case_values = std::mem::take(&mut self.scalar_case_values);
        for (argument, _parameter) in structural_arguments
            .iter()
            .zip(&callee.structural_parameters)
            .filter(|(argument, parameter)| {
                argument.path.is_empty()
                    && parameter.multiplicity != StructuralMultiplicity::Unrestricted
            })
        {
            if caller_structural_values.remove(&argument.place).is_none()
                && caller_scalar_case_values.remove(&argument.place).is_none()
            {
                return Err(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                    argument.place,
                ));
            }
        }
        self.call_stack.push(SuspendedCall {
            values: std::mem::take(&mut self.values),
            structural_values: caller_structural_values,
            byte_sequence_values: std::mem::take(&mut self.byte_sequence_values),
            scalar_case_values: caller_scalar_case_values,
            scalar_array_values: std::mem::take(&mut self.scalar_array_values),
            live_affine_frontier: caller_affine_frontier,
            live_claims: std::mem::take(&mut self.live_claims),
            dynamic_parameters: std::mem::take(&mut self.dynamic_parameters),
            current_machine: self.current_machine,
            current: self.current,
            next_operation: self.next_operation,
            result: SuspendedCallResult::Scalar(result.id),
        });
        self.values = values;
        self.structural_values = structural_values;
        self.byte_sequence_values = byte_sequence_values;
        self.scalar_array_values = prepared_arguments.scalar_arrays;
        self.scalar_case_values = prepared_arguments.scalar_cases;
        self.live_affine_frontier = callee_affine_frontier;
        self.live_claims = live_claims;
        self.dynamic_parameters = dynamic_parameters;
        self.current_machine = callee_id;
        self.current = callee.entry;
        self.next_operation = 0;
        Ok(())
    }

    pub(super) fn begin_structural_result_call(
        &mut self,
        callee_id: MachineId,
        result: StructuralOperationResult,
        scalar_arguments: &[TerminalScalarValue],
        structural_arguments: &[StructuralArgument],
        prepared_arguments: StructuralCallArguments,
        claim_transfers: &[ClaimTransfer],
        returned_claim_transfers: Vec<StructuralResultClaimTransfer>,
    ) -> Result<(), TerminalInterpretError> {
        let machines = std::sync::Arc::clone(&self.machines);
        let callee = machines
            .get(&callee_id)
            .ok_or(TerminalInterpretError::VerifiedCallTargetMissing(callee_id))?;
        let Some(callee_result) = callee.result.structural() else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        let exact_primitive_structural_call = callee.entry_claims.is_empty()
            && callee.content_entry_claims.is_empty()
            && claim_transfers.is_empty()
            && returned_claim_transfers.is_empty()
            && matches!(
                result.multiplicity,
                StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
            )
            && result.qualifications.is_empty()
            && result.projected_qualifications.is_empty()
            && result.claims.is_empty()
            && (self
                .structural_types
                .get(&result.structural_type)
                .is_some_and(|declaration| match &declaration.shape {
                    StructuralTypeShape::Sum { cases } => cases.iter().all(|case| {
                        case.fields.iter().all(|field| {
                            !field.relevance.is_erased() && field.field_type.scalar_type().is_some()
                        })
                    }),
                    StructuralTypeShape::Record { .. } => {
                        self.plain_record_type(result.structural_type)
                    }
                    _ => false,
                })
                || (result.multiplicity == StructuralMultiplicity::Unrestricted
                    && terminal_semantics::scalar_array_leaf_shape(
                        self.structural_types.values(),
                        result.structural_type,
                    )
                    .is_some()));
        if structural_arguments
            .iter()
            .zip(&callee.structural_parameters)
            .any(|(argument, parameter)| {
                !argument.path.is_empty()
                    // Preparing the argument already resolved its exact path,
                    // type, access and alias custody. A structural return does
                    // not change where a static borrowed input is backed.
                    && !(argument.access != StructuralAccess::Owned
                        && parameter.access == argument.access
                        && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                        && argument.path.iter().all(|segment| matches!(segment,
                            StructuralPathSegment::Field(_) | StructuralPathSegment::FixedIndex(_)))
                        && prepared_arguments.values.get(&parameter.place).is_some_and(|value|
                            value.structural_type == parameter.structural_type))
                    && !(argument.path.last() == Some(&StructuralPathSegment::Referent)
                        && argument.access != StructuralAccess::Owned
                        && parameter.access == argument.access
                        && prepared_arguments
                            .values
                            .get(&parameter.place)
                            .is_some_and(|value| {
                                value.structural_type == parameter.structural_type
                                    && self
                                        .structural_types
                                        .get(&value.structural_type)
                                        .is_some_and(|declaration| {
                                            matches!(
                                                declaration.shape,
                                                StructuralTypeShape::PrimitiveScalar(_)
                                            )
                                        })
                            }))
                    // An owned operand may also name a declared-field subtree
                    // when the callee returns reference custody: the result's
                    // published source roster pins each moved leaf, and the
                    // affine projection split already removed it from the
                    // caller's frontier.
                    && !(argument.access == StructuralAccess::Owned
                        && parameter.access == StructuralAccess::Owned
                        && parameter.multiplicity == StructuralMultiplicity::Affine
                        && !callee_result.reference_sources.is_empty()
                        && argument.path.iter().all(|segment| matches!(segment,
                            StructuralPathSegment::Field(_)))
                        && prepared_arguments.values.get(&parameter.place).is_some_and(|value|
                            value.structural_type == parameter.structural_type))
                    && !matches!(
                        prepared_arguments.byte_sequences.get(&parameter.place),
                        Some(
                            ByteSequenceBinding::MutableField { .. }
                                | ByteSequenceBinding::MutableArray { .. }
                        )
                    )
            })
            || result.structural_type != callee_result.structural_type
            || result.multiplicity != callee_result.multiplicity
            || result.qualifications != callee_result.qualifications
            || (result.multiplicity == StructuralMultiplicity::Unrestricted
                && !exact_primitive_structural_call)
            || self.structural_values.contains_key(&result.place)
            || self.scalar_case_values.contains_key(&result.place)
            || self.scalar_array_values.contains_key(&result.place)
        {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let values = bind_arguments(&callee.parameters, scalar_arguments)?;
        let mut structural_values = prepared_arguments.values;
        self.copy_owned_record_arguments(&callee.structural_parameters, &mut structural_values)?;
        let expected_reference_backings = self.bind_reference_return_backings(
            &callee.structural_parameters,
            callee_result,
            &structural_values,
        )?;
        let byte_sequence_values = prepared_arguments.byte_sequences;
        let callee_affine_frontier = scalar_case_arguments::bind_call_affine_frontier(
            &callee.structural_parameters,
            &structural_values,
            &prepared_arguments.scalar_cases,
        )?;
        let (remaining_claims, live_claims) = transfer_claims(
            &self.live_claims,
            &self.structural_values,
            structural_arguments,
            claim_transfers,
            &callee.structural_parameters,
            &callee.entry_claims,
            &callee.content_entry_claims,
            &structural_values,
        )?;

        let mut caller_affine_frontier = self.live_affine_frontier.clone();
        let mut retired_carriers = BTreeSet::new();
        for (argument, parameter) in structural_arguments
            .iter()
            .zip(&callee.structural_parameters)
        {
            if parameter.multiplicity == StructuralMultiplicity::Affine
                && argument.access == StructuralAccess::Owned
            {
                self.consume_affine_call_argument(&mut caller_affine_frontier, argument)?;
                // A projected operand that consumed every affine subtree
                // retires its carrier: no residual frontier entry survives
                // to answer for the root, so its value cannot remain either.
                if !argument.path.is_empty()
                    && !caller_affine_frontier
                        .iter()
                        .any(|entry| entry.place == argument.place)
                {
                    retired_carriers.insert(argument.place);
                }
            }
        }
        let mut caller_structural_values = self.structural_values.clone();
        for (argument, _parameter) in structural_arguments
            .iter()
            .zip(&callee.structural_parameters)
            .filter(|(argument, parameter)| {
                (argument.path.is_empty()
                    && parameter.multiplicity != StructuralMultiplicity::Unrestricted)
                    || retired_carriers.contains(&argument.place)
            })
        {
            if caller_structural_values.remove(&argument.place).is_none()
                && !self.scalar_case_values.contains_key(&argument.place)
            {
                return Err(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                    argument.place,
                ));
            }
        }

        let mut caller_scalar_case_values = std::mem::take(&mut self.scalar_case_values);
        for (argument, parameter) in structural_arguments
            .iter()
            .zip(&callee.structural_parameters)
        {
            if argument.path.is_empty()
                && parameter.multiplicity != StructuralMultiplicity::Unrestricted
            {
                caller_scalar_case_values.remove(&argument.place);
            }
        }
        self.next_operation += 1;
        self.call_stack.push(SuspendedCall {
            values: std::mem::take(&mut self.values),
            structural_values: caller_structural_values,
            byte_sequence_values: std::mem::take(&mut self.byte_sequence_values),
            scalar_case_values: caller_scalar_case_values,
            scalar_array_values: std::mem::take(&mut self.scalar_array_values),
            live_affine_frontier: caller_affine_frontier,
            live_claims: remaining_claims,
            dynamic_parameters: std::mem::take(&mut self.dynamic_parameters),
            current_machine: self.current_machine,
            current: self.current,
            next_operation: self.next_operation,
            result: SuspendedCallResult::Structural {
                result,
                returned_claim_transfers,
                expected_reference_backings,
            },
        });
        self.values = values;
        self.structural_values = structural_values;
        self.byte_sequence_values = byte_sequence_values;
        self.scalar_array_values = prepared_arguments.scalar_arrays;
        self.scalar_case_values = prepared_arguments.scalar_cases;
        self.live_affine_frontier = callee_affine_frontier;
        self.live_claims = live_claims;
        self.dynamic_parameters = BTreeMap::new();
        self.current_machine = callee_id;
        self.current = callee.entry;
        self.next_operation = 0;
        Ok(())
    }

    pub(super) fn begin_runtime_dynamic_scalar_call(
        &mut self,
        callee_id: MachineId,
        result: terminal_psi::ValueDeclaration,
        source: TerminalStructuralValue,
    ) -> Result<(), TerminalInterpretError> {
        let machines = std::sync::Arc::clone(&self.machines);
        let callee = machines
            .get(&callee_id)
            .ok_or(TerminalInterpretError::VerifiedCallTargetMissing(callee_id))?;
        if !callee.parameters.is_empty()
            || callee.structural_parameters.len() != 1
            || callee.result.scalar().map(|result| result.scalar_type) != Some(result.scalar_type)
        {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let structural_values =
            bind_structural_arguments(&callee.structural_parameters, &[source])?;
        let callee_affine_frontier =
            bind_affine_frontier(&callee.structural_parameters, &structural_values)?;
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
            result: SuspendedCallResult::Scalar(result.id),
        });
        self.values = BTreeMap::new();
        self.structural_values = structural_values;
        self.live_affine_frontier = callee_affine_frontier;
        self.live_claims = BTreeMap::new();
        self.dynamic_parameters = BTreeMap::new();
        self.current_machine = callee_id;
        self.current = callee.entry;
        self.next_operation = 0;
        Ok(())
    }

    pub(super) fn begin_runtime_dynamic_unit_call(
        &mut self,
        callee_id: MachineId,
        source: TerminalStructuralValue,
    ) -> Result<(), TerminalInterpretError> {
        let machines = std::sync::Arc::clone(&self.machines);
        let callee = machines
            .get(&callee_id)
            .ok_or(TerminalInterpretError::VerifiedCallTargetMissing(callee_id))?;
        if !callee.parameters.is_empty()
            || callee.structural_parameters.len() != 1
            || callee.result != TerminalMachineResult::Unit
        {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let structural_values =
            bind_structural_arguments(&callee.structural_parameters, &[source])?;
        let callee_affine_frontier =
            bind_affine_frontier(&callee.structural_parameters, &structural_values)?;
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
            result: SuspendedCallResult::Unit,
        });
        self.values = BTreeMap::new();
        self.structural_values = structural_values;
        self.live_affine_frontier = callee_affine_frontier;
        self.live_claims = BTreeMap::new();
        self.dynamic_parameters = BTreeMap::new();
        self.current_machine = callee_id;
        self.current = callee.entry;
        self.next_operation = 0;
        Ok(())
    }
}
