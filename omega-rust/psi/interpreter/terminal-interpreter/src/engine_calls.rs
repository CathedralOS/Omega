/*
    Machine and component call preparation, argument staging, saved caller
    frames, and target-family resolution for the Terminal Psi engine.

    We prepare scalar, structural, and component arguments, reserve and capture
    suspended caller frames, and restore caller state upon callee completion.
*/

pub(crate) mod arguments {
    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::values::{TerminalScalarValue, TerminalStructuralValue};
    use crate::engine::custody::StructuralArgumentSources;
    use crate::engine::execution::TerminalExecution;
    use crate::storage::byte_sequences::binding::ByteSequenceBinding;
    use crate::storage::runtime_rows::RuntimeRows;
    use crate::storage::scalar_array::ScalarArrayBindings;
    use semantic_vocabulary::{MachineId, PlaceId, ValueId};
    use terminal_psi::{StructuralAccess, StructuralArgument, StructuralParameterDeclaration};
    use terminal_semantics::scalar_array_leaf_shape;

    pub(crate) struct StructuralCallArguments {
        pub(crate) local_identity_cursor: Option<u64>,
        pub(crate) descriptor_parameters: Vec<StructuralParameterDeclaration>,
        pub(crate) values: RuntimeRows<PlaceId, TerminalStructuralValue>,
        pub(crate) aggregate_backings:
            RuntimeRows<PlaceId, crate::storage::reference::AggregateBacking>,
        pub(crate) byte_sequences: RuntimeRows<PlaceId, ByteSequenceBinding>,
        pub(crate) primitive_loans:
            RuntimeRows<PlaceId, crate::storage::primitive_storage::PrimitiveLoan>,
        pub(crate) scalar_arrays: ScalarArrayBindings,
        pub(crate) element_views: RuntimeRows<PlaceId, crate::storage::element_views::ElementView>,
    }

    impl StructuralCallArguments {
        pub(crate) fn validate_carrier_partition(
            &self,
            parameters: &[StructuralParameterDeclaration],
        ) -> Result<(), TerminalInterpretError> {
            if self.values.len() + self.scalar_arrays.len() != parameters.len()
                || self.descriptor_parameters.len() != self.values.len()
            {
                return Err(malformed());
            }
            let mut descriptors = self.descriptor_parameters.iter();
            for parameter in parameters {
                let opaque = self.values.contains_key(&parameter.place);
                let arrays = self.scalar_arrays.contains_key(&parameter.place);
                if usize::from(opaque) + usize::from(arrays) != 1
                    || (opaque && descriptors.next() != Some(parameter))
                {
                    return Err(malformed());
                }
            }
            if descriptors.next().is_some() {
                return Err(malformed());
            }
            Ok(())
        }
    }

    impl TerminalExecution<'_> {
        pub(crate) fn resolve_call_arguments(
            &self,
            callee: MachineId,
            arguments: &[ValueId],
            structural_arguments: &[StructuralArgument],
        ) -> Result<(Vec<TerminalScalarValue>, StructuralCallArguments), TerminalInterpretError>
        {
            let scalar_arguments = self
                .scalar_values_at(arguments)
                .map_err(|error| error.in_argument_preparation("read scalar call arguments"))?;
            let prepared_arguments =
                self.prepare_structural_call_arguments(callee, structural_arguments)?;
            Ok((scalar_arguments, prepared_arguments))
        }

        pub(crate) fn prepare_structural_call_arguments(
            &self,
            callee: MachineId,
            arguments: &[StructuralArgument],
        ) -> Result<StructuralCallArguments, TerminalInterpretError> {
            let machine = self.program.verified_machine(callee)?;
            if machine.structural_parameters.len() != arguments.len() {
                return Err(TerminalInterpretError::StructuralArgumentCount {
                    expected: machine.structural_parameters.len(),
                    actual: arguments.len(),
                });
            }
            let mut scalar_arrays = ScalarArrayBindings::new(self.account.as_ref());
            let mut opaque_parameters = Vec::new();
            let mut opaque_arguments = Vec::new();
            for (parameter, argument) in machine.structural_parameters.iter().zip(arguments) {
                if scalar_array_leaf_shape(
                    self.structural_types().values(),
                    parameter.structural_type,
                )
                .is_some()
                    && parameter.access == StructuralAccess::Owned
                    && parameter.projected_qualifications.is_empty()
                {
                    let value = self.prepare_scalar_array_argument(machine, parameter, argument)?;
                    scalar_arrays.publish(parameter.place, value)?;
                } else {
                    opaque_parameters.push(parameter.clone());
                    opaque_arguments.push(argument.clone());
                }
            }
            let provenance = self.structural_argument_provenance(&opaque_arguments)?;
            let mut prepared = self
                .prepare_boundary_arguments(&opaque_parameters, &opaque_arguments)
                .map_err(|error| error.in_argument_preparation("resolve argument carriers"))?
                .into_call_arguments(
                    &opaque_parameters,
                    &self.structural_types(),
                    self.structural_cases.as_map(),
                    Some(StructuralArgumentSources {
                        arguments: &opaque_arguments,
                        provenance: &provenance,
                    }),
                    self.account.as_ref(),
                )
                .map_err(|error| error.in_argument_preparation("admit argument carriers"))?;
            prepared.scalar_arrays = scalar_arrays;
            Ok(prepared)
        }
    }
}
pub(crate) mod frames {
    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::values::{TerminalScalarValue, TerminalStructuralValue};
    use crate::engine::calls::arguments::StructuralCallArguments;
    use crate::engine::custody::bind_arguments;
    use crate::engine::custody::transfer_claims;
    use crate::engine::execution::SuspendedCall;
    use crate::engine::execution::SuspendedCallResult;
    use crate::engine::execution::TerminalExecution;
    use crate::storage::byte_sequences::binding::ByteSequenceBinding;
    use crate::storage::runtime_rows::RuntimeRows;
    use semantic_vocabulary::{MachineId, PlaceId};
    use std::collections::BTreeSet;
    use terminal_psi::{
        ClaimTransfer, StructuralAccess, StructuralArgument, StructuralMultiplicity,
        StructuralOperationResult, StructuralParameterDeclaration, StructuralTypeShape,
        TerminalMachineResult,
    };

    fn is_direct_borrowed_rebinding(
        parameter: &StructuralParameterDeclaration,
        argument: &StructuralArgument,
        resolved: &TerminalStructuralValue,
        declarations: &crate::embedding::program::ProgramDeclarations<
            '_,
            terminal_psi::StructuralTypeDeclaration,
        >,
    ) -> bool {
        parameter.structural_type == resolved.structural_type
            && terminal_semantics::view_descriptor_parameter(parameter, declarations.values())
            && parameter.access == argument.access
            && matches!(
                parameter.access,
                terminal_psi::reading_borrow_pair_pattern!()
            )
            && parameter.qualifications.is_empty()
            && parameter.projected_qualifications.iter().all(|row| {
                row.is_element_membership() && resolved.projected_qualifications.contains(row)
            })
            && resolved.qualifications.is_empty()
    }

    pub(crate) fn transferred_call_sources(
        arguments: &[StructuralArgument],
        parameters: &[StructuralParameterDeclaration],
        values: &RuntimeRows<PlaceId, TerminalStructuralValue>,
    ) -> Result<Vec<PlaceId>, TerminalInterpretError> {
        let sources = arguments
            .iter()
            .zip(parameters)
            .filter(|(argument, parameter)| {
                argument.access == StructuralAccess::Owned
                    && parameter.multiplicity != StructuralMultiplicity::Unrestricted
                    && argument.path.is_empty()
            })
            .map(|(argument, _)| argument.place)
            .collect::<BTreeSet<_>>();
        for place in &sources {
            if !values.contains_key(place) {
                return Err(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                    *place,
                ));
            }
        }
        Ok(sources.into_iter().collect())
    }

    fn retire_transferred_call_sources(
        arguments: &[StructuralArgument],
        parameters: &[StructuralParameterDeclaration],
        values: &mut RuntimeRows<PlaceId, TerminalStructuralValue>,
    ) -> Result<(), TerminalInterpretError> {
        for place in transferred_call_sources(arguments, parameters, values)? {
            values.remove(&place);
        }
        Ok(())
    }

    impl TerminalExecution<'_> {
        pub(crate) fn suspend_with(
            &mut self,
            result: SuspendedCallResult,
            loan_completions: Vec<terminal_psi::LoanCompletion>,
        ) -> Result<&mut SuspendedCall, TerminalInterpretError> {
            let frame_storage = match self
                .account
                .as_ref()
                .map(|account| account.reserve_storage(1, 0))
                .transpose()
            {
                Ok(storage) => storage,
                Err(error) => {
                    self.failed_call_result = Some(result);
                    return Err(TerminalInterpretError::Storage(error));
                }
            };
            self.call_stack.push(SuspendedCall {
                access_view_occurrences: std::mem::take(&mut self.access_view_occurrences),
                loan_completions,
                values: self.values.take(),
                structural_values: self.structural_values.take(),
                linear_homes: self.linear_homes.take(),
                aggregate_argument_backings: self.aggregate_argument_backings.take(),
                byte_sequence_values: self.byte_sequence_values.take(),
                primitive_loan_values: self.primitive_loan_values.take(),
                scalar_array_values: self.scalar_array_values.take(),
                element_view_values: self.element_view_values.take(),
                live_affine_frontier: std::mem::take(&mut self.live_affine_frontier),
                live_claims: std::mem::take(&mut self.live_claims),
                current_machine: self.current_machine,
                current: self.current,
                next_operation: self.next_operation,
                result,
                _frame_storage: frame_storage,
            });
            self.call_stack.last_mut().ok_or_else(malformed)
        }

        pub(crate) fn consume_affine_call_argument(
            &self,
            frontier: &mut BTreeSet<terminal_psi::StructuralAffineDiscard>,
            argument: &StructuralArgument,
        ) -> Result<(), TerminalInterpretError> {
            if !self.stage_constructor_operand(argument, frontier, &mut Vec::new())? {
                return Err(TerminalInterpretError::AffineFrontierMismatch);
            }
            Ok(())
        }

        pub(crate) fn bind_byte_sequence_arguments(
            &self,
            parameters: &[StructuralParameterDeclaration],
            arguments: &[StructuralArgument],
            resolved_arguments: &[TerminalStructuralValue],
        ) -> Result<RuntimeRows<PlaceId, ByteSequenceBinding>, TerminalInterpretError> {
            if parameters.len() != arguments.len() || parameters.len() != resolved_arguments.len() {
                return Err(malformed());
            }
            let mut values = RuntimeRows::new(self.account.as_ref());
            for ((parameter, argument), resolved) in
                parameters.iter().zip(arguments).zip(resolved_arguments)
            {
                let declaration = self.structural_declaration(parameter.structural_type)?;
                let StructuralTypeShape::ByteSequence(carrier) = &declaration.shape else {
                    continue;
                };
                if terminal_semantics::borrowed_storage_parameter(
                    parameter,
                    self.structural_types().values(),
                ) && terminal_semantics::stored_reference_access(&declaration.shape).is_some()
                    && terminal_semantics::borrowed_storage_reference_ceiling(
                        parameter,
                        self.structural_types().values(),
                    )
                    .is_none()
                {
                    continue;
                }
                if let Some((value, binding)) =
                    self.prepare_owned_byte_descriptor_argument(parameter, argument, resolved)?
                {
                    if value != *resolved || values.insert(parameter.place, binding)?.is_some() {
                        return Err(malformed());
                    }
                    continue;
                }
                if let Some((value, binding)) =
                    self.prepare_byte_descriptor_argument(parameter, argument)?
                {
                    if value != *resolved || values.insert(parameter.place, binding)?.is_some() {
                        return Err(malformed());
                    }
                    continue;
                }
                if let terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity } = carrier
                    && parameter.access == StructuralAccess::Owned
                    && argument.access == StructuralAccess::Owned
                    && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                    && parameter.structural_type == resolved.structural_type
                    && parameter.projected_qualifications.is_empty()
                    && argument.path.is_empty()
                {
                    let bytes = crate::engine::custody::verified_structural_place(
                        &self.byte_sequence_values,
                        argument.place,
                    )?;
                    let ByteSequenceBinding::Immutable(snapshot) = self
                        .reborrow_byte_sequence_binding(
                            bytes,
                            resolved,
                            StructuralAccess::SharedBorrow,
                        )?
                    else {
                        return Err(malformed());
                    };
                    if u64::try_from(snapshot.len()).map_or(true, |length| length > *capacity)
                        || !terminal_semantics::byte_literal_qualifications_hold_in(
                            self.program.catalogs.structural_domains.iter(),
                            parameter.structural_type,
                            &parameter.qualifications,
                            snapshot.bytes(),
                        )
                        || values
                            .insert(
                                parameter.place,
                                ByteSequenceBinding::Immutable(snapshot.owned_snapshot()),
                            )?
                            .is_some()
                    {
                        return Err(malformed());
                    }
                    continue;
                }
                return Err(malformed());
            }
            Ok(values)
        }

        pub(crate) fn bind_element_view_arguments(
            &self,
            parameters: &[StructuralParameterDeclaration],
            arguments: &[StructuralArgument],
            resolved_arguments: &[TerminalStructuralValue],
        ) -> Result<
            RuntimeRows<PlaceId, crate::storage::element_views::ElementView>,
            TerminalInterpretError,
        > {
            if parameters.len() != arguments.len() || parameters.len() != resolved_arguments.len() {
                return Err(malformed());
            }
            let mut values = RuntimeRows::new(self.account.as_ref());
            for ((parameter, argument), resolved) in
                parameters.iter().zip(arguments).zip(resolved_arguments)
            {
                let declaration = self.structural_declaration(parameter.structural_type)?;
                if !matches!(declaration.shape, StructuralTypeShape::ElementView { .. }) {
                    continue;
                }
                if terminal_semantics::borrowed_storage_parameter(
                    parameter,
                    self.structural_types().values(),
                ) && terminal_semantics::borrowed_storage_reference_ceiling(
                    parameter,
                    self.structural_types().values(),
                )
                .is_none()
                {
                    continue;
                }
                let stored =
                    self.resolve_live_projection(argument.place, &argument.path, argument.access)?;
                if stored.opaque_identity != resolved.opaque_identity
                    || stored.path != resolved.path
                    || !terminal_semantics::borrowed_referent_matches(
                        self.structural_types().values(),
                        stored.structural_type,
                        resolved.structural_type,
                    )
                {
                    return Err(malformed());
                }
                let view = self.element_view_argument(argument, &stored)?;
                let access = if parameter.access == StructuralAccess::Owned {
                    if argument.access != StructuralAccess::Owned
                        || parameter.structural_type != resolved.structural_type
                        || view.owned_multiplicity() != Some(parameter.multiplicity)
                    {
                        return Err(malformed());
                    }
                    view.access()
                } else if terminal_semantics::borrowed_storage_parameter(
                    parameter,
                    self.structural_types().values(),
                ) {
                    if argument.access != parameter.access {
                        return Err(malformed());
                    }
                    self.validate_descriptor_borrow(argument, parameter.access)?;
                    terminal_semantics::borrowed_storage_reference_ceiling(
                        parameter,
                        self.structural_types().values(),
                    )
                    .ok_or_else(malformed)?
                } else {
                    if !is_direct_borrowed_rebinding(
                        parameter,
                        argument,
                        resolved,
                        &self.structural_types(),
                    ) {
                        return Err(TerminalInterpretError::StructuralArgumentPreparation {
                            stage: "rebind borrowed element view",
                            detail: format!(
                                "formal={parameter:?}, source={resolved:?}, access={:?}, producer={:?}",
                                view.access(),
                                self.program
                                    .verified_machine(self.current_machine)?
                                    .producing_operation(argument.place)
                            ),
                        });
                    }
                    parameter.access
                };
                let presented = view.subslice(0, view.len(), access).ok_or(malformed())?;
                let presented = presented.with_descriptor_home(resolved.clone());
                if values.insert(parameter.place, presented)?.is_some() {
                    return Err(malformed());
                }
            }
            Ok(values)
        }

        fn enter_call(
            &mut self,
            callee_id: MachineId,
            callee: &terminal_psi::TerminalMachine,
            scalar_arguments: &[TerminalScalarValue],
            structural_arguments: &[StructuralArgument],
            prepared_arguments: StructuralCallArguments,
            claim_transfers: &[ClaimTransfer],
            mut result: SuspendedCallResult,
        ) -> Result<(), TerminalInterpretError> {
            let values =
                bind_arguments(&callee.parameters, scalar_arguments, self.account.as_ref())
                    .map_err(|error| {
                        error.in_argument_preparation("bind scalar call parameters")
                    })?;
            prepared_arguments
                .validate_carrier_partition(&callee.structural_parameters)
                .map_err(|error| error.in_argument_preparation("join callee carrier partition"))?;
            let access_view_occurrences = self
                .bind_call_access_views(callee_id, structural_arguments, &prepared_arguments.values)
                .map_err(|error| error.in_argument_preparation("bind placed argument views"))?;
            let mut structural_values = prepared_arguments.values;
            let mut identity_cursor = prepared_arguments.local_identity_cursor;
            if let SuspendedCallResult::Structural {
                expected_reference_backings,
                ..
            } = &mut result
            {
                *expected_reference_backings = self
                    .bind_reference_return_backings(
                        &callee.structural_parameters,
                        callee.result.structural().ok_or_else(malformed)?,
                        &structural_values,
                        &prepared_arguments.aggregate_backings,
                        &prepared_arguments.primitive_loans,
                        &prepared_arguments.element_views,
                        &prepared_arguments.byte_sequences,
                        &access_view_occurrences,
                    )
                    .map_err(|error| {
                        error.in_argument_preparation("capture reference result backings")
                    })?;
            }
            let aggregate_payload = self
                .stage_owned_value_arguments(
                    &prepared_arguments.descriptor_parameters,
                    &mut structural_values,
                    &mut identity_cursor,
                )
                .map_err(|error| error.in_argument_preparation("stage owned call payloads"))?;
            let byte_sequence_values = prepared_arguments.byte_sequences;
            let mut callee_affine_frontier = crate::engine::custody::bind_affine_frontier(
                &callee.structural_parameters,
                &structural_values,
                &self.structural_types(),
            )
            .map_err(|error| error.in_argument_preparation("bind callee affine frontier"))?;
            let (remaining_claims, live_claims) = transfer_claims(
                &self.structural_types(),
                self.structural_cases.as_map(),
                &self.live_claims,
                structural_arguments,
                claim_transfers,
                &callee.structural_parameters,
                &callee.entry_claims,
                &callee.content_entry_claims,
                &structural_values,
            )
            .map_err(|error| error.in_argument_preparation("transfer argument claims"))?;
            crate::engine::custody::exclude_linear_regions(
                &self.structural_types(),
                &mut callee_affine_frontier,
                live_claims
                    .values()
                    .filter_map(|claim| claim.place.map(|place| (place, claim.path.as_slice()))),
                &|current| {
                    crate::boundary::structural_inputs::case_membership::stored_case_at_projection(
                        &self.structural_types(),
                        structural_values.get(&current.place)?,
                        self.structural_cases.as_map(),
                        &current.path,
                    )
                },
            )
            .map_err(|error| error.in_argument_preparation("exclude transferred linear regions"))?;
            let mut caller_affine_frontier = self.live_affine_frontier.clone();
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
            let (loan_completions, resume_operation) = if matches!(
                result,
                SuspendedCallResult::ExternalCompletion { .. }
                    | SuspendedCallResult::NoNormal { .. }
            ) {
                (Vec::new(), self.next_operation)
            } else {
                let caller = self.program.verified_machine(self.current_machine)?;
                let operation = caller
                    .block(&self.current)
                    .and_then(|block| block.operations.get(self.next_operation))
                    .ok_or_else(malformed)?
                    .id;
                let completions =
                    self.loan_completions_for_operation(operation)
                        .map_err(|error| {
                            error.in_argument_preparation("retain call loan completions")
                        })?;
                (completions, self.next_operation + 1)
            };
            let caller = self.suspend_with(result, loan_completions)?;
            caller.next_operation = resume_operation;
            retire_transferred_call_sources(
                structural_arguments,
                &callee.structural_parameters,
                &mut caller.structural_values,
            )?;
            caller.live_affine_frontier = caller_affine_frontier;
            caller.live_claims = remaining_claims;
            self.access_view_occurrences = access_view_occurrences;
            self.values = values;
            self.structural_values = structural_values;
            self.aggregate_argument_backings = prepared_arguments.aggregate_backings;
            self.byte_sequence_values = byte_sequence_values;
            self.primitive_loan_values = prepared_arguments.primitive_loans;
            self.local_structural_identities
                .commit_cursor(identity_cursor);
            aggregate_payload.publish(self)?;
            self.scalar_array_values = prepared_arguments.scalar_arrays;
            self.element_view_values = prepared_arguments.element_views;
            self.live_affine_frontier = callee_affine_frontier;
            self.live_claims = live_claims;
            self.current_machine = callee_id;
            self.current = callee.entry;
            self.next_operation = 0;
            Ok(())
        }

        pub(crate) fn begin_unit_call(
            &mut self,
            callee_id: MachineId,
            scalar_arguments: &[TerminalScalarValue],
            structural_arguments: &[StructuralArgument],
            prepared_arguments: StructuralCallArguments,
            claim_transfers: &[ClaimTransfer],
        ) -> Result<(), TerminalInterpretError> {
            let program = std::sync::Arc::clone(&self.program);
            let callee = program.verified_machine(callee_id)?;
            if callee.result != TerminalMachineResult::Unit {
                return Err(malformed());
            }
            self.enter_call(
                callee_id,
                callee,
                scalar_arguments,
                structural_arguments,
                prepared_arguments,
                claim_transfers,
                SuspendedCallResult::Unit,
            )
        }

        pub(crate) fn begin_external_completion_call(
            &mut self,
            callee_id: MachineId,
            boundary: semantic_vocabulary::BoundaryMachineId,
            edge: semantic_vocabulary::EdgeId,
            scalar_arguments: &[TerminalScalarValue],
            structural_arguments: &[StructuralArgument],
        ) -> Result<(), TerminalInterpretError> {
            let program = std::sync::Arc::clone(&self.program);
            let callee = program.verified_machine(callee_id)?;
            if callee.result != TerminalMachineResult::Unit {
                return Err(malformed());
            }
            let prepared =
                self.prepare_structural_call_arguments(callee_id, structural_arguments)?;
            self.enter_call(
                callee_id,
                callee,
                scalar_arguments,
                structural_arguments,
                prepared,
                &[],
                SuspendedCallResult::ExternalCompletion { boundary, edge },
            )
        }

        pub(crate) fn begin_no_normal_call(
            &mut self,
            callee_id: MachineId,
            edge: semantic_vocabulary::EdgeId,
            scalar_arguments: &[TerminalScalarValue],
            structural_arguments: &[StructuralArgument],
            claim_transfers: &[ClaimTransfer],
        ) -> Result<(), TerminalInterpretError> {
            let program = std::sync::Arc::clone(&self.program);
            let callee = program.verified_machine(callee_id)?;
            let prepared =
                self.prepare_structural_call_arguments(callee_id, structural_arguments)?;
            self.enter_call(
                callee_id,
                callee,
                scalar_arguments,
                structural_arguments,
                prepared,
                claim_transfers,
                SuspendedCallResult::NoNormal {
                    callee: callee_id,
                    edge,
                },
            )
        }

        pub(crate) fn begin_structural_scalar_call(
            &mut self,
            callee_id: MachineId,
            result: terminal_psi::ValueDeclaration,
            scalar_arguments: &[TerminalScalarValue],
            structural_arguments: &[StructuralArgument],
            prepared_arguments: StructuralCallArguments,
            claim_transfers: &[ClaimTransfer],
        ) -> Result<(), TerminalInterpretError> {
            let program = std::sync::Arc::clone(&self.program);
            let callee = program.verified_machine(callee_id)?;
            if callee.result.scalar().map(|result| result.scalar_type) != Some(result.scalar_type) {
                return Err(malformed());
            }
            self.enter_call(
                callee_id,
                callee,
                scalar_arguments,
                structural_arguments,
                prepared_arguments,
                claim_transfers,
                SuspendedCallResult::Scalar(result.id),
            )
        }

        pub(crate) fn begin_structural_result_call(
            &mut self,
            callee_id: MachineId,
            result: StructuralOperationResult,
            scalar_arguments: &[TerminalScalarValue],
            structural_arguments: &[StructuralArgument],
            prepared_arguments: StructuralCallArguments,
            claim_transfers: &[ClaimTransfer],
            claim_origin: crate::engine::execution::ResultClaimOrigin,
        ) -> Result<(), TerminalInterpretError> {
            let program = std::sync::Arc::clone(&self.program);
            let callee = program.verified_machine(callee_id)?;
            let invalid_result = |reason| TerminalInterpretError::StructuralCallResultContract {
                callee: callee_id,
                place: result.place,
                actual: result.structural_type,
                expected: callee
                    .result
                    .structural()
                    .map(|value| value.structural_type),
                reason,
            };
            let Some(callee_result) = callee.result.structural() else {
                return Err(invalid_result("callee has no structural result"));
            };
            if let crate::engine::execution::ResultClaimOrigin::TargetFamilyReturn { head, member } =
                claim_origin
                && (member != callee_id || !program.is_target_family_head(head))
            {
                return Err(invalid_result(
                    "result origin does not name this target-family member",
                ));
            }
            let mut preserved_projected_qualifications = Vec::new();
            if claim_origin == crate::engine::execution::ResultClaimOrigin::CalleeReturn
                && result
                    .projected_qualifications
                    .iter()
                    .any(|row| !callee_result.projected_qualifications.contains(row))
            {
                let caller = program.verified_machine(self.current_machine)?;
                let mut operations = caller
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .filter(|operation| {
                        operation
                            .result
                            .structural()
                            .is_some_and(|value| value.place == result.place)
                    });
                let operation = operations
                    .next()
                    .ok_or_else(|| invalid_result("result has no defining call operation"))?;
                if operations.next().is_some() || operation.result.structural() != Some(&result) {
                    return Err(invalid_result(
                        "result has multiple or inconsistent defining operations",
                    ));
                }
                let performed_arguments = structural_arguments;
                let same_call = match &operation.kind {
                    terminal_psi::call_structural_pair_pattern!(
                        callee,
                        structural_arguments,
                        ..
                    ) => {
                        *callee == callee_id
                            && structural_arguments.as_slice() == performed_arguments
                    }
                    _ => false,
                };
                if !same_call {
                    return Err(invalid_result(
                        "defining call has different callee or structural arguments",
                    ));
                }
                let preserved =
                    terminal_semantics::ReferenceResultMemberships::new(&program.catalogs)
                        .for_operation(caller, operation)
                        .map_err(TerminalInterpretError::UnsupportedSemanticVariant)?;
                for row in &result.projected_qualifications {
                    if !callee_result.projected_qualifications.contains(row) {
                        if !preserved.contains(row) {
                            return Err(invalid_result(
                                "projected result qualification has no preserved input source",
                            ));
                        }
                        preserved_projected_qualifications.push(row.clone());
                    }
                }
            }
            let refusal = if result.structural_type != callee_result.structural_type {
                Some("result carrier differs from the callee declaration")
            } else if result.multiplicity != callee_result.multiplicity {
                Some("result multiplicity differs from the callee declaration")
            } else if !claim_origin
                .result_qualifications_match(&result, &callee_result.qualifications)
            {
                Some("result qualifications differ from the callee declaration")
            } else if !claim_origin.result_projected_qualifications_match(
                &result,
                &callee_result.projected_qualifications,
                &preserved_projected_qualifications,
            ) {
                Some("projected result qualifications have no complete callee or input source")
            } else if self.structural_values.contains_key(&result.place) {
                Some("result destination already has a structural value")
            } else if self.scalar_array_values.contains_key(&result.place) {
                Some("result destination already has a scalar array")
            } else {
                None
            };
            if let Some(reason) = refusal {
                return Err(invalid_result(reason));
            }
            self.enter_call(
                callee_id,
                callee,
                scalar_arguments,
                structural_arguments,
                prepared_arguments,
                claim_transfers,
                SuspendedCallResult::Structural {
                    result,
                    claim_origin,
                    expected_reference_backings: Vec::new(),
                    preserved_projected_qualifications,
                    profile_completion: None,
                },
            )
            .map_err(|error| error.in_argument_preparation("enter structural-result callee"))
        }
    }

    #[cfg(test)]
    pub(crate) mod tests {
        use super::*;
        use crate::boundary::results::{TerminalExecutionResult, TerminalStructuralResult};
        use crate::embedding::program::AdmittedTerminalProgram;
        use crate::engine::execution::{CleanupContinuation, CompletedReturn};
        use crate::storage::byte_sequences::ByteSequenceView;
        use crate::storage::byte_sequences::storage::ByteSequenceStorage;
        use evaluation_accounting::{
            BuildEvaluationSponsor, BuildEvaluationSponsorLimits, EvaluationAccountingError,
            EvaluationMeasurementIdentity, EvaluationResource, TerminalStorageScheduleIdentity,
        };
        use semantic_vocabulary::{BlockId, ClaimId, ContractId, EdgeId, StructuralTypeId};
        use std::sync::Arc;
        use terminal_psi::{
            Block, MachineContract, NominalAffineCleanup, RegisterTargetApplicability,
            StructuralResultClaimBinding, TerminalMachine, TerminalModule, Terminator,
        };

        pub(crate) fn empty_program() -> Arc<AdmittedTerminalProgram<'static>> {
            let machine = MachineId::new(1).unwrap();
            let entry = BlockId::new(1).unwrap();
            let mut module = TerminalModule::for_entry(machine);
            module.machines.push(TerminalMachine {
                assembly_target_requirement: RegisterTargetApplicability::Any,
                id: machine,
                attachment: None,
                nominal_cleanup_subject: None,
                structural_places: Vec::new(),
                block_entry_claims: Vec::new(),
                loan_completions: Vec::new(),
                entry,
                blocks: vec![Block {
                    id: entry,
                    parameters: Vec::new(),
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: EdgeId::new(1).unwrap(),
                        cleanup_actions: Vec::new(),
                    },
                }],
                ranked_scc: None,
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                contract: terminal_psi::MachineContract {
                    declaration: None,
                    parameters: Vec::new(),
                    parameter_order: Vec::new(),
                    structural_parameters: Vec::new(),
                    entry_claims: Vec::new(),
                    content_entry_claims: Vec::new(),
                    result: TerminalMachineResult::Unit,
                    declared_service_reach: None,
                    closed_reach_application: None,
                    published_synchronous_entries: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    ..MachineContract::empty(ContractId::new(1).unwrap())
                },
            });
            Arc::new(AdmittedTerminalProgram {
                identity: terminal_codec::TerminalExecutionIdentity::portable(
                    terminal_codec::terminal_psi_identity(&module).unwrap(),
                ),
                catalogs: Arc::new(crate::embedding::program::ProgramStorage::Owned(module)),
                target_layouts: None,
                scalar_geometry: None,
            })
        }

        #[test]
        fn refused_cleanup_frame_retains_outgoing_backing_and_claims() {
            let sponsor = BuildEvaluationSponsor::new_with_measurement(
                BuildEvaluationSponsorLimits::new(100, 8, 2, 2, 1, 100, 100, 100).unwrap(),
                EvaluationMeasurementIdentity::TerminalPsi {
                    work_schedule: psi_work::WorkSchedule::CURRENT.identity(),
                    storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
                },
            )
            .unwrap();
            let account = sponsor
                .begin_invocation(sponsor.measurement(), 100)
                .unwrap();
            let mut execution = TerminalExecution::start_with_program(
                empty_program(),
                &[],
                crate::TerminalStructuralInputs::default(),
                None,
                Some(account.clone()),
            )
            .unwrap();
            let structural_type = StructuralTypeId::new(1).unwrap();
            let value = TerminalStructuralValue::minted(42, structural_type, Vec::new());
            let claim = ClaimId::new(1).unwrap();
            let mut outgoing = CompletedReturn::plain(TerminalExecutionResult::Structural(
                TerminalStructuralResult {
                    value: value.clone(),
                    claims: vec![claim],
                },
            ));
            outgoing.returned_claims = vec![StructuralResultClaimBinding {
                claim,
                path: Vec::new(),
            }];
            outgoing.byte_sequence = Some(ByteSequenceBinding::Immutable(ByteSequenceView::new(
                ByteSequenceStorage::copy(b"retained", Some(&account)).unwrap(),
            )));
            let cleanup = NominalAffineCleanup {
                retains_descendants: false,
                place: PlaceId::new(2).unwrap(),
                path: Vec::new(),
                structural_type,
                cleanup_machine: MachineId::new(1).unwrap(),
                consumed_claims: Vec::new(),
                cleanup_receiver: None,
                requirement_obligations: Vec::new(),
            };
            assert_eq!(sponsor.live_payload_bytes(), 8);
            assert!(matches!(
                execution.suspend_with(
                    SuspendedCallResult::NominalCleanups {
                        completed: (cleanup, value),
                        remaining: Vec::new().into_iter(),
                        continuation: CleanupContinuation::Return(outgoing),
                    },
                    Vec::new(),
                ),
                Err(TerminalInterpretError::Storage(
                    EvaluationAccountingError::BudgetExhausted(EvaluationResource::LiveStorage)
                ))
            ));
            assert_eq!(
                sponsor.live_payload_bytes(),
                8,
                "failed activation retains outgoing storage"
            );
            assert_eq!(sponsor.live_storage_cells(), 1);
            assert!(execution.call_stack.is_empty());
            assert_eq!(execution.next_operation, 0);
            let Some(SuspendedCallResult::NominalCleanups {
                continuation: CleanupContinuation::Return(retained),
                ..
            }) = &execution.failed_call_result
            else {
                panic!("refused capture must retain the exact cleanup continuation");
            };
            assert_eq!(
                retained.returned_claims,
                [StructuralResultClaimBinding {
                    claim,
                    path: Vec::new()
                }]
            );
            let TerminalExecutionResult::Structural(result) = &retained.value else {
                panic!("outgoing structural result was lost");
            };
            assert_eq!(result.claims, [claim]);
            assert_eq!(result.value.opaque_identity, 42);
            let Some(ByteSequenceBinding::Immutable(bytes)) = &retained.byte_sequence else {
                panic!("outgoing bytes were lost");
            };
            assert_eq!(bytes.bytes(), b"retained");
            drop(execution);
            assert_eq!(sponsor.live_payload_bytes(), 0);
            assert_eq!(sponsor.live_storage_cells(), 0);
        }
    }
}
pub(crate) mod target_families {
    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::values::TerminalScalarValue;
    use crate::embedding::program::AdmittedTerminalProgram;
    use crate::engine::capabilities::TerminalUnsupportedCapability;
    use crate::engine::custody::{
        bind_active_structural_arguments, validate_boundary_requirements,
    };
    use crate::engine::execution::{
        OperationFlow, ResultClaimOrigin, TerminalExecution, operation_kind,
    };
    use semantic_vocabulary::{BoundaryMachineId, MachineId};
    use terminal_installation::AdmittedTargetMachineSelections;
    use terminal_psi::{
        BoundaryMachineDeclaration, ClaimTransfer, Operation, OperationKind, OperationResult,
        TargetMachineCallable, TargetMachineSupply,
    };

    pub(crate) fn selected_member(
        program: &AdmittedTerminalProgram<'_>,
        selections: Option<&AdmittedTargetMachineSelections>,
        boundary: BoundaryMachineId,
    ) -> Result<TargetMachineCallable, TerminalInterpretError> {
        let head = TargetMachineCallable::Boundary(boundary);
        let selections =
            selections.ok_or(TerminalInterpretError::TargetMachineSelectionMissing(head))?;
        if selections.terminal_psi() != program.identity.semantic() {
            return Err(TerminalInterpretError::TargetMachineSelectionIdentityMismatch);
        }
        let selected = selections
            .member(head)
            .map_err(TerminalInterpretError::TargetMachineSelection)?;
        match (selected.callable(), selected.supply()) {
            (
                TargetMachineCallable::Machine(member),
                TargetMachineSupply::CheckedBody | TargetMachineSupply::Boundary,
            ) => {
                program.verified_machine(member)?;
                Ok(selected.callable())
            }
            (TargetMachineCallable::Boundary(member), supply)
                if supply != TargetMachineSupply::CheckedBody =>
            {
                program.verified_boundary_machine(&member)?;
                Ok(selected.callable())
            }
            (callable, supply) => Err(TerminalInterpretError::UnsupportedCapability(
                TerminalUnsupportedCapability::TargetMachineMemberSupply {
                    head,
                    callable,
                    supply,
                },
            )),
        }
    }

    pub(crate) fn completion_callee(
        program: &AdmittedTerminalProgram<'_>,
        selections: Option<&AdmittedTargetMachineSelections>,
        supply: Option<crate::boundary::effects::AdmittedBoundarySupply>,
        boundary: BoundaryMachineId,
    ) -> Result<Option<MachineId>, TerminalInterpretError> {
        if program.is_target_family_head(boundary)
            && let TargetMachineCallable::Machine(member) =
                selected_member(program, selections, boundary)?
        {
            return Ok(Some(member));
        }
        match supply {
            Some(crate::boundary::effects::AdmittedBoundarySupply::CheckedProvider(callee)) => {
                Ok(Some(callee))
            }
            None if program.has_provider(&boundary) => Err(
                TerminalInterpretError::ProviderInstallationMissing(boundary),
            ),
            _ => Ok(None),
        }
    }

    pub(crate) fn validate_result_transport(
        operation: &Operation,
        member: &terminal_psi::TerminalMachine,
    ) -> Result<(), TerminalInterpretError> {
        let OperationKind::BoundaryCall { boundary, .. } = operation.kind else {
            return Err(malformed());
        };
        let origin = ResultClaimOrigin::TargetFamilyReturn {
            head: boundary,
            member: member.id,
        };
        let supported = match &operation.result {
            OperationResult::Unit => member.result == terminal_psi::TerminalMachineResult::Unit,
            OperationResult::Scalar(result) => member
                .result
                .scalar()
                .is_some_and(|declared| declared.scalar_type == result.scalar_type),
            OperationResult::Structural(result) => {
                member.result.structural().is_some_and(|declared| {
                    result.structural_type == declared.structural_type
                        && result.multiplicity == declared.multiplicity
                        && origin.result_qualifications_match(result, &declared.qualifications)
                        && origin.result_projected_qualifications_match(
                            result,
                            &declared.projected_qualifications,
                            &[],
                        )
                        && result
                            .qualification_establishments
                            .iter()
                            .all(|row| declared.qualifications.contains(&row.domain))
                })
            }
        };
        if supported { Ok(()) } else { Err(malformed()) }
    }

    impl TerminalExecution<'_> {
        pub(crate) fn execute_target_family_call(
            &mut self,
            operation: &Operation,
            head: &BoundaryMachineDeclaration,
            member_id: MachineId,
            scalar_arguments: &[TerminalScalarValue],
        ) -> Result<OperationFlow, TerminalInterpretError> {
            operation_kind!(operation => BoundaryCall {
                structural_arguments, completion_receipts, ..
            });
            let program = std::sync::Arc::clone(&self.program);
            let member = program.verified_machine(member_id)?;
            validate_result_transport(operation, member)?;
            let prepared =
                self.prepare_structural_call_arguments(member_id, structural_arguments)?;
            if head.structural_parameters.len() != member.structural_parameters.len() {
                return Err(malformed());
            }
            let mut opaque_parameters = Vec::new();
            let mut opaque_values = Vec::new();
            for (head_parameter, member_parameter) in head
                .structural_parameters
                .iter()
                .zip(&member.structural_parameters)
            {
                if let Some(value) = prepared.values.get(&member_parameter.place) {
                    opaque_parameters.push(head_parameter.clone());
                    opaque_values.push(value.clone());
                } else if !prepared.scalar_arrays.contains_key(&member_parameter.place)
                    || head_parameter.structural_type != member_parameter.structural_type
                {
                    return Err(malformed());
                }
            }
            bind_active_structural_arguments(
                &opaque_parameters,
                &opaque_values,
                &self.structural_types(),
                self.structural_cases.as_map(),
                self.account.as_ref(),
            )?;
            validate_boundary_requirements(head, |argument_index| {
                member
                    .structural_parameters
                    .get(argument_index)
                    .and_then(|parameter| prepared.values.get(&parameter.place))
            })?;
            let transfers = completion_receipts
                .iter()
                .map(|receipt| ClaimTransfer {
                    claim: receipt.claim,
                    argument_index: receipt.argument_index,
                    path: receipt.path.clone(),
                })
                .collect::<Vec<_>>();
            match &operation.result {
                OperationResult::Unit => self.begin_unit_call(
                    member_id,
                    scalar_arguments,
                    structural_arguments,
                    prepared,
                    &transfers,
                )?,
                OperationResult::Scalar(result) => self.begin_structural_scalar_call(
                    member_id,
                    *result,
                    scalar_arguments,
                    structural_arguments,
                    prepared,
                    &transfers,
                )?,
                OperationResult::Structural(result) => self.begin_structural_result_call(
                    member_id,
                    result.clone(),
                    scalar_arguments,
                    structural_arguments,
                    prepared,
                    &transfers,
                    ResultClaimOrigin::TargetFamilyReturn {
                        head: head.id,
                        member: member_id,
                    },
                )?,
            }
            Ok(OperationFlow::Redispatch)
        }
    }
}

pub(crate) mod component_arguments {
    use crate::engine::custody::validate_boundary_requirements;
    use std::collections::{BTreeMap, BTreeSet};

    use semantic_vocabulary::{BlockId, ClaimId, MachineId, PlaceId};
    use terminal_psi::{
        BoundaryMachineDeclaration, CompletionReceipt, StructuralAccess, StructuralAffineDiscard,
        StructuralArgument, StructuralMultiplicity,
    };

    use super::component_empty_arguments::SharedEmptyArgument;
    use super::component_scalar_arguments::LentScalarArgument;
    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::values::{StructuralRuntimePlace, TerminalStructuralValue};
    use crate::embedding::program::TerminalProgram;
    use crate::engine::execution::TerminalExecution;

    pub(crate) struct PreparedComponentArguments {
        pub(crate) custody: ComponentInputCustody,
        pub(crate) remaining_affine: BTreeSet<StructuralAffineDiscard>,
        pub(crate) remaining_claims: BTreeMap<ClaimId, crate::engine::execution::LiveClaim>,
        pub(crate) retired_sources: Vec<PlaceId>,
        pub(crate) borrowed_mutations: Vec<StructuralRuntimePlace>,
    }

    pub(crate) struct ComponentInputCustody {
        pub(crate) source: Option<crate::boundary::effects::byte_buffers::BoundaryArguments>,
        pub(crate) values: Vec<TerminalStructuralValue>,
        pub(crate) contents: Vec<crate::TerminalStructuralContents>,
        opaque_arguments: Vec<usize>,
        scalar_arguments: Vec<LentScalarArgument>,
        empty_arguments: Vec<SharedEmptyArgument>,
        _scalar_scope: Option<crate::storage::scalar_storage::ScalarLoanScope>,
    }

    impl ComponentInputCustody {
        pub(crate) fn establish_lent_inputs(
            &self,
            child: &mut TerminalExecution<'_>,
        ) -> Result<(), TerminalInterpretError> {
            if self.source.is_none() {
                return Ok(());
            }
            let entry = child.program.verified_machine(child.current_machine)?;
            if entry.structural_parameters.len() != self.values.len() {
                return Err(malformed());
            }
            for &index in &self.opaque_arguments {
                let parameter = entry
                    .structural_parameters
                    .get(index)
                    .ok_or_else(malformed)?;
                let value = self.values.get(index).ok_or_else(malformed)?;
                if !matches!(
                    parameter.access,
                    terminal_psi::reading_borrow_pair_pattern!()
                ) {
                    return Err(malformed());
                }
                let home = StructuralRuntimePlace::from(child.structural_value(parameter.place)?);
                child.component_lent_host_atoms.insert(home.clone(), ())?;
                child.structural_host_atoms.insert(
                    home,
                    crate::storage::structural_storage::HostStructuralPayload::Opaque(
                        value.opaque_identity,
                    ),
                )?;
            }
            for argument in &self.empty_arguments {
                argument.establish(child)?;
            }
            for argument in &self.scalar_arguments {
                argument.establish(child)?;
            }
            Ok(())
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(crate) struct ComponentCallScope {
        pub(crate) namespace: Option<u64>,
        machine: MachineId,
        block: BlockId,
        depth: usize,
    }

    impl<'artifact> TerminalExecution<'artifact> {
        pub(crate) fn component_call_scope(&self) -> ComponentCallScope {
            ComponentCallScope {
                namespace: self.local_structural_identities.exposure_namespace,
                machine: self.current_machine,
                block: self.current,
                depth: self.call_stack.len(),
            }
        }

        pub(crate) fn prepare_component_arguments(
            &mut self,
            arguments: &[StructuralArgument],
            completion_receipts: &[CompletionReceipt],
            receiver: &BoundaryMachineDeclaration,
            provider: &TerminalProgram<'artifact>,
            selected: &terminal_installation::AdmittedComponentProvider,
        ) -> Result<PreparedComponentArguments, TerminalInterpretError> {
            let reject = |reason| TerminalInterpretError::UnsupportedSemanticVariant(reason);
            let supplied = provider
                .program
                .verified_boundary_machine(&selected.provider_boundary())?;
            let entry = provider.program.verified_machine(selected.candidate())?;
            if receiver.structural_parameters.len() != supplied.structural_parameters.len()
                || arguments.len() != receiver.structural_parameters.len()
                || entry.structural_parameters.len() != arguments.len()
                || !entry.content_entry_claims.is_empty()
                || entry.entry_claims.len()
                    != supplied
                        .structural_parameters
                        .iter()
                        .filter(|parameter| {
                            parameter.multiplicity == StructuralMultiplicity::Linear
                        })
                        .count()
            {
                return Err(malformed());
            }
            let prepared =
                self.prepare_boundary_arguments(&receiver.structural_parameters, arguments)?;
            crate::engine::custody::bind_active_structural_arguments(
                &receiver.structural_parameters,
                &prepared.values,
                &self.structural_types(),
                self.structural_cases.as_map(),
                self.account.as_ref(),
            )?;
            validate_boundary_requirements(receiver, |index| prepared.values.get(index))?;
            prepared.opaque_values(self.local_structural_identities.cursor())?;
            let mut prepared = prepared;
            let lending = arguments
                .iter()
                .any(|argument| argument.access != StructuralAccess::Owned);
            let mut values = if lending {
                prepared.values.clone()
            } else {
                std::mem::take(&mut prepared.values)
            };
            let mut contents = Vec::new();
            let mut opaque_arguments = Vec::new();
            let mut scalar_arguments = Vec::new();
            let mut empty_arguments = Vec::new();
            let mut remaining_affine = self.live_affine_frontier.clone();
            let receiver_types = self.structural_types();
            let provider_types = provider.program.structural_types();
            for (argument_index, (((argument, required), supplied), value)) in arguments
                .iter()
                .zip(&receiver.structural_parameters)
                .zip(&supplied.structural_parameters)
                .zip(&mut values)
                .enumerate()
            {
                let linear = required.multiplicity == StructuralMultiplicity::Linear;
                let receiver_type = receiver_types
                    .get(&required.structural_type)
                    .ok_or_else(malformed)?;
                let provider_type = provider_types
                    .get(&supplied.structural_type)
                    .ok_or_else(malformed)?;
                let owned = argument.access == StructuralAccess::Owned;
                let lent = matches!(
                    argument.access,
                    terminal_psi::reading_borrow_pair_pattern!()
                );
                if argument.access != required.access
                    || required.access != supplied.access
                    || (!owned && !lent)
                    || required.multiplicity != supplied.multiplicity
                    || !required.projected_qualifications.is_empty()
                    || !supplied.projected_qualifications.is_empty()
                    || !value.projected_qualifications.is_empty()
                    || receiver_type.owns_linear_claim != linear
                    || provider_type.owns_linear_claim != linear
                    || (!linear
                        && (!receiver_type.no_code_affine_cleanup
                            || !provider_type.no_code_affine_cleanup))
                    || !self
                        .program
                        .catalogs
                        .identities
                        .text(receiver_type.identity)
                        .zip(
                            provider
                                .program
                                .catalogs
                                .identities
                                .text(provider_type.identity),
                        )
                        .is_some_and(|(left, right)| !left.is_empty() && left == right)
                    || (owned
                        && crate::engine::custody::has_live_loan(
                            self.reference_referents.as_map(),
                            value,
                        ))
                {
                    return Err(reject(
                        "component input lacks exact owned closed Inert type/custody",
                    ));
                }
                if lent {
                    self.validate_descriptor_borrow(argument, required.access)?;
                }
                let formal = &entry.structural_parameters[argument_index];
                if formal.structural_type != supplied.structural_type
                    || formal.multiplicity != supplied.multiplicity
                    || formal.access != supplied.access
                {
                    return Err(reject(
                        "component entry differs from its admitted structural contract",
                    ));
                }
                let mut entry_claims = entry
                    .entry_claims
                    .iter()
                    .filter(|claim| claim.input == formal.place);
                let receipt_index = u32::try_from(argument_index).map_err(|_| malformed())?;
                let mut receipts = completion_receipts
                    .iter()
                    .filter(|receipt| receipt.argument_index == receipt_index);
                let carried_claim = if linear {
                    let claim = entry_claims.next().ok_or_else(|| {
                        reject("linear component input lacks its whole entry claim")
                    })?;
                    let receipt = receipts.next().ok_or_else(|| {
                        reject("linear component input lacks its actual caller claim")
                    })?;
                    let live = self
                        .live_claims
                        .get(&receipt.claim)
                        .ok_or_else(|| reject("linear component input claim is not live"))?;
                    if !claim.path.is_empty()
                        || !receipt.path.is_empty()
                        || entry_claims.next().is_some()
                        || receipts.next().is_some()
                        || live.place != Some(argument.place)
                        || live.path != argument.path
                        || live.multiplicity != Some(StructuralMultiplicity::Linear)
                    {
                        return Err(reject(
                            "linear component input lacks exactly one whole owned claim",
                        ));
                    }
                    Some(receipt.claim)
                } else if entry_claims.next().is_some() || receipts.next().is_some() {
                    return Err(reject(
                        "nonlinear component input carries unexpected claims",
                    ));
                } else {
                    None
                };
                if self.live_claims.iter().any(|(identity, claim)| {
                    claim.place == Some(argument.place)
                        && terminal_semantics::structural_paths_may_overlap(
                            &claim.path,
                            &argument.path,
                        )
                        && Some(*identity) != carried_claim
                }) {
                    return Err(reject(
                        "component input overlaps another live ownership claim",
                    ));
                }
                let home = StructuralRuntimePlace::from(&*value);
                let within_value = |stored: &StructuralRuntimePlace| {
                    stored.opaque_identity == home.opaque_identity
                        && stored.path.starts_with(&home.path)
                };
                if lent
                    && (self.structural_cases.keys().any(within_value)
                        || self.structural_byte_storage.keys().any(within_value)
                        || self.empty_array_storage.rows.keys().any(within_value))
                {
                    return Err(reject(
                        "component borrowed input needs its original materialized storage carrier",
                    ));
                }
                if self.reference_referents.keys().any(within_value)
                    || self.structural_element_view_fields.keys().any(within_value)
                    || self.original_storage_roots.keys().any(within_value)
                    || self.provider_storage.descriptors.keys().any(within_value)
                    || self
                        .provider_storage
                        .completed_profiles
                        .keys()
                        .any(within_value)
                    || self
                        .activation_storage
                        .claim_descriptors
                        .keys()
                        .any(within_value)
                    || self.structural_byte_descriptors.keys().any(within_value)
                    || self
                        .structural_byte_sequence_fields
                        .keys()
                        .any(|field| within_value(&field.parent))
                {
                    return Err(reject(
                        "component input requires a loan or resource storage transfer carrier",
                    ));
                }
                let scalar_backing = lent
                    && (self.structural_primitive_storage.keys().any(within_value)
                        || self
                            .structural_scalar_fields
                            .keys()
                            .any(|field| within_value(&field.parent)));
                if scalar_backing {
                    scalar_arguments.push(self.prepare_component_scalar_argument(
                        value,
                        argument_index,
                        required.access,
                        provider,
                        supplied.structural_type,
                    )?);
                } else if !self.forward_host_atom(value) {
                    if required.access == StructuralAccess::SharedBorrow
                        && let Some(original) =
                            self.prepare_shared_empty_argument(value, argument_index)?
                    {
                        empty_arguments.push(original);
                    } else if !owned {
                        return Err(reject(
                            "component input requires actual unchanged opaque host storage",
                        ));
                    } else {
                        contents.push(self.capture_component_contents(
                            argument,
                            value,
                            argument_index,
                            provider,
                            supplied.structural_type,
                        )?);
                    }
                } else if lent {
                    opaque_arguments.push(argument_index);
                }
                value.qualifications =
                    value
                        .qualifications
                        .iter()
                        .map(|domain| {
                            selected
                        .input_membership(argument_index, *domain)
                        .ok_or_else(|| {
                            reject("component input membership has no admitted destination theory")
                        })
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                value.qualifications.sort();
                value.structural_type = supplied.structural_type;
                if owned && required.multiplicity == StructuralMultiplicity::Affine {
                    self.consume_affine_call_argument(&mut remaining_affine, argument)?;
                }
            }
            let retired_sources = super::frames::transferred_call_sources(
                arguments,
                &receiver.structural_parameters,
                &self.structural_values,
            )?;
            let remaining_claims = crate::engine::custody::complete_claims(
                &self.live_claims,
                arguments,
                completion_receipts,
                &receiver.structural_parameters,
            )?;
            let borrowed_mutations = scalar_arguments
                .iter()
                .filter_map(|argument| argument.mutated_home().cloned())
                .collect();
            let scalar_scope = (!scalar_arguments.is_empty())
                .then(crate::storage::scalar_storage::ScalarLoanScope::new);
            let mut scalar_views = BTreeMap::new();
            let scalar_arguments = scalar_arguments
                .into_iter()
                .map(|argument| {
                    argument.lend(
                        self,
                        scalar_scope.as_ref().ok_or_else(malformed)?,
                        &mut scalar_views,
                    )
                })
                .collect::<Result<_, _>>()?;
            Ok(PreparedComponentArguments {
                custody: ComponentInputCustody {
                    source: lending.then_some(prepared),
                    values,
                    contents,
                    opaque_arguments,
                    scalar_arguments,
                    empty_arguments,
                    _scalar_scope: scalar_scope,
                },
                remaining_affine,
                remaining_claims,
                retired_sources,
                borrowed_mutations,
            })
        }
    }
}
pub(crate) mod component_calls {
    use psi_work::WorkMeter;
    use semantic_vocabulary::StructuralDomainId;
    use terminal_psi::{BoundaryMachineResult, OperationKind, OperationResult};

    use crate::boundary::effects::results::TerminalEffectResult;
    use crate::boundary::effects::{
        AdmittedBoundarySupply, TerminalEffect, TerminalEffectRejection,
    };
    use crate::boundary::errors::TerminalInterpretError;
    use crate::boundary::results::{TerminalExecutionResult, TerminalExecutionStatus};
    use crate::boundary::values::TerminalScalarValue;
    use crate::embedding::instance::ComponentBindings;
    use crate::engine::execution::{OperationFlow, TerminalExecution};

    impl<'artifact> TerminalExecution<'artifact> {
        pub(crate) fn execute_component_call(
            &mut self,
            operation: &terminal_psi::Operation,
            arguments: &[TerminalScalarValue],
            meter: &mut WorkMeter,
            components: &mut ComponentBindings<'_, 'artifact>,
        ) -> Result<OperationFlow, TerminalInterpretError> {
            let reject = |reason| TerminalInterpretError::EffectRejected {
                operation: operation.id,
                rejection: TerminalEffectRejection::new(reason),
            };
            let OperationKind::BoundaryCall {
                boundary,
                structural_arguments,
                completion_receipts,
                ..
            } = &operation.kind
            else {
                return Err(reject(
                    "component dispatch reached another operation".to_owned(),
                ));
            };
            let binding = components.get_mut(boundary).ok_or_else(|| {
                reject(
                    "component invocation has no established producing instance binding".to_owned(),
                )
            })?;
            let caller = self.component_call_scope();
            let selected = binding.selected();
            let expected_supply = AdmittedBoundarySupply::ComponentProvider {
                provider: selected.provider(),
                provider_boundary: selected.provider_boundary(),
                candidate: selected.candidate(),
            };
            if selected.receiver() != &self.program.identity
                || selected.receiving_boundary() != *boundary
                || self.boundary_supplies.get(boundary) != Some(&expected_supply)
            {
                return Err(reject(
                    "component invocation differs from the exact receiving installation".to_owned(),
                ));
            }
            if !binding.pending_at(operation.id) {
                let declarations = self.program.clone();
                let receiver = declarations.verified_boundary_machine(boundary)?;
                let prepared = self.prepare_component_arguments(
                    structural_arguments,
                    completion_receipts,
                    receiver,
                    binding.provider_program(),
                    selected,
                )?;
                binding
                    .prepare(
                        caller,
                        operation.id,
                        arguments,
                        structural_arguments,
                        prepared.custody,
                        self.account.as_ref(),
                    )
                    .map_err(&reject)?;
                self.live_affine_frontier = prepared.remaining_affine;
                self.live_claims = prepared.remaining_claims;
                for home in prepared.borrowed_mutations {
                    self.invalidate_stored_provenance(&home);
                }
                for place in prepared.retired_sources {
                    self.structural_values
                        .remove(&place)
                        .expect("owned source was staged without parent mutation");
                }
            }
            match binding
                .invoke(caller, operation.id, arguments, structural_arguments, meter)
                .map_err(&reject)?
            {
                Some(
                    status @ (TerminalExecutionStatus::SponsorExhausted(_)
                    | TerminalExecutionStatus::HostComputationPending(_)),
                ) => {
                    return Ok(OperationFlow::Yield(status));
                }
                Some(status @ TerminalExecutionStatus::AwaitingEffect(_)) => {
                    if operation.suspension_crossing.is_none() {
                        return Err(reject(
                            "component readiness requires the caller's checked suspension crossing"
                                .to_owned(),
                        ));
                    }
                    return Ok(OperationFlow::Yield(status));
                }
                Some(_) => {
                    return Err(reject(
                        "component invocation did not return normally".to_owned(),
                    ));
                }
                None => {}
            }
            let completed = binding.completed().ok_or_else(|| {
                reject("component invocation lost its owned completion".to_owned())
            })?;
            let selected = binding.selected();
            if completed.provider() != selected.provider()
                || completed.candidate() != selected.candidate()
                || Some(completed.instance_namespace())
                    == self.local_structural_identities.exposure_namespace
            {
                return Err(reject(
                    "component completion has another provider, entry or instance namespace"
                        .to_owned(),
                ));
            }
            let receiver = self
                .program
                .verified_boundary_machine(boundary)?
                .result
                .clone();
            let provider = binding.provider_program();
            let supplied = &provider
                .program
                .verified_boundary_machine(&selected.provider_boundary())?
                .result;
            let mut result_claims = self.live_claims.clone();
            let returned = match (&receiver, supplied, completed.value()) {
                (
                    BoundaryMachineResult::Unit,
                    BoundaryMachineResult::Unit,
                    TerminalExecutionResult::Unit,
                ) => TerminalEffectResult::Unit,
                (
                    BoundaryMachineResult::Scalar(left),
                    BoundaryMachineResult::Scalar(right),
                    TerminalExecutionResult::Scalar(value),
                ) if left == right && value.scalar_type() == left.scalar_type => {
                    TerminalEffectResult::Scalar(*value)
                }
                (
                    BoundaryMachineResult::Structural(left),
                    BoundaryMachineResult::Structural(right),
                    TerminalExecutionResult::Structural(returned),
                ) => {
                    let types = self.structural_types();
                    let receiver_type = types.get(&left.structural_type).ok_or_else(|| {
                        reject("component receiver lost its result type".to_owned())
                    })?;
                    let provider_types = provider.program.structural_types();
                    let provider_type =
                        provider_types.get(&right.structural_type).ok_or_else(|| {
                            reject("component producer lost its result type".to_owned())
                        })?;
                    if returned.value.structural_type != right.structural_type
                        || left.multiplicity != right.multiplicity
                        || !left.projected_qualifications.is_empty()
                        || !right.projected_qualifications.is_empty()
                        || receiver_type.owns_linear_claim
                            != (left.multiplicity == terminal_psi::StructuralMultiplicity::Linear)
                        || provider_type.owns_linear_claim != receiver_type.owns_linear_claim
                        || (!receiver_type.owns_linear_claim
                            && (!receiver_type.no_code_affine_cleanup
                                || !provider_type.no_code_affine_cleanup))
                        || !self
                            .program
                            .catalogs
                            .identities
                            .text(receiver_type.identity)
                            .zip(
                                provider
                                    .program
                                    .catalogs
                                    .identities
                                    .text(provider_type.identity),
                            )
                            .is_some_and(|(left, right)| !left.is_empty() && left == right)
                    {
                        return Err(reject(
                        "component result lacks exact closed type or transferable Inert custody"
                            .to_owned(),
                    ));
                    }
                    let mut value = returned.value.clone();
                    value.structural_type = left.structural_type;
                    value.qualifications = value
                        .qualifications
                        .iter()
                        .map(|domain| {
                            selected.result_membership(*domain).ok_or_else(|| {
                                reject("component returned an unmapped membership".to_owned())
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    value.qualifications.sort();
                    let domains = &self.program.catalogs.structural_domains;
                    let entails =
                        |actual: &[StructuralDomainId], required: &[StructuralDomainId]| {
                            required.iter().all(|identity| {
                                domains
                                    .iter()
                                    .find(|domain| domain.id == *identity)
                                    .is_some_and(|domain| {
                                        domain.carrier == left.structural_type
                                            && domain
                                                .missing_membership_requirements(domains, actual)
                                                .is_some_and(|missing| missing.is_empty())
                                    })
                            })
                        };
                    if !entails(&value.qualifications, &left.qualifications)
                        || !entails(&left.qualifications, &value.qualifications)
                    {
                        return Err(reject(format!(
                            "component result membership closures differ: returned={:?}, receiving={:?}",
                            value.qualifications, left.qualifications,
                        )));
                    }
                    value.qualifications.clone_from(&left.qualifications);
                    let claims = completed.returned_claims().ok_or_else(|| {
                        reject("component return lost its owned claim bindings".to_owned())
                    })?;
                    if let OperationResult::Structural(result) = &operation.result {
                        if self.structural_values.contains_key(&result.place)
                            || crate::engine::custody::place_is_claimed(
                                &self.live_claims,
                                result.place,
                            )
                            || self
                                .live_affine_frontier
                                .iter()
                                .any(|value| value.place == result.place)
                        {
                            return Err(reject(
                                "component receiving result place still owns custody".to_owned(),
                            ));
                        }
                        if receiver_type.owns_linear_claim {
                            if claims.len() != 1
                                || !claims[0].path.is_empty()
                                || returned.claims != [claims[0].claim]
                                || result.claims.len() != 1
                                || !result.claims[0].path.is_empty()
                            {
                                return Err(reject(
                                "linear component result lacks exactly one whole returned claim"
                                    .to_owned(),
                            ));
                            }
                        } else if !claims.is_empty()
                            || !returned.claims.is_empty()
                            || !result.claims.is_empty()
                        {
                            return Err(reject(
                                "nonlinear component result carries unexpected claims".to_owned(),
                            ));
                        }
                        for claim in &result.claims {
                            crate::engine::custody::bind_result_claim(
                                &mut result_claims,
                                claim,
                                result.place,
                                result.multiplicity,
                            )?;
                        }
                    } else {
                        return Err(reject(
                            "component structural result has no receiving home".to_owned(),
                        ));
                    }
                    self.local_structural_identities.reserve_host_at_cursor(
                        &value,
                        self.local_structural_identities.cursor(),
                    )?;
                    TerminalEffectResult::Structural(value)
                }
                _ => {
                    return Err(reject(
                        "component completion disagrees with the checked receiving result"
                            .to_owned(),
                    ));
                }
            };
            let provider = completed.provider();
            let candidate = completed.candidate();
            let instance_namespace = completed.instance_namespace();
            self.commit_boundary_result(&operation.result, &receiver, returned)?;
            self.live_claims = result_claims;
            self.effects.push(TerminalEffect::ComponentCall {
                operation: operation.id,
                boundary: *boundary,
                provider,
                candidate,
                instance_namespace,
                effects: binding.acknowledge(),
            });
            Ok(OperationFlow::Advance)
        }
    }
}
pub(crate) mod component_contents {
    use semantic_vocabulary::{CanonicalStructuralPathSegment as Path, StructuralTypeId};
    use terminal_psi::{StructuralArgument, StructuralTypeDeclaration, TerminalIdentityTable};

    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::embedding::program::{ProgramDeclarations, TerminalProgram};
    use crate::engine::execution::TerminalExecution;
    use crate::storage::structural_storage::contents::{self, StorageLeafKind};
    use crate::{
        TerminalBoundaryStorageValue, TerminalStorageContents, TerminalStructuralContents,
        TerminalStructuralValue,
    };

    impl TerminalExecution<'_> {
        pub(crate) fn component_storage_path(
            &self,
            source_type: StructuralTypeId,
            destination_type: StructuralTypeId,
            provider: &TerminalProgram<'_>,
            path: &[Path],
        ) -> Result<Vec<Path>, TerminalInterpretError> {
            self.component_contents_mapping(provider)
                .rehome_path(source_type, destination_type, path)
                .map(|(path, _, _)| path)
        }

        fn component_contents_mapping<'source, 'destination>(
            &'source self,
            provider: &'destination TerminalProgram<'_>,
        ) -> ComponentContentsMapping<'source, 'destination> {
            ComponentContentsMapping {
                source_types: self.structural_types(),
                source_identities: &self.program.catalogs.identities,
                destination_types: provider.program.structural_types(),
                destination_identities: &provider.program.catalogs.identities,
            }
        }

        pub(crate) fn capture_component_contents(
            &self,
            argument: &StructuralArgument,
            value: &TerminalStructuralValue,
            argument_index: usize,
            provider: &TerminalProgram<'_>,
            destination_type: StructuralTypeId,
        ) -> Result<TerminalStructuralContents, TerminalInterpretError> {
            let description = contents::describe(self, value, argument.access, None)?;
            if description.requires_erased_establishment
                || description.leaves.iter().any(|leaf| {
                    matches!(
                        leaf.kind,
                        StorageLeafKind::Opaque { .. } | StorageLeafKind::ByteField { .. }
                    )
                })
            {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "component contents cannot manufacture opaque backing or erased custody",
                ));
            }
            let (values, _) = contents::capture(
                self,
                argument.place,
                value,
                None,
                None,
                argument.access,
                &description.leaves,
                &[],
            )?;
            let mapping = self.component_contents_mapping(provider);
            let mut values = values
                .into_iter()
                .map(|contents| mapping.rehome(value.structural_type, destination_type, contents))
                .collect::<Result<Vec<_>, _>>()?;
            values.sort_by(|left, right| left.path.cmp(&right.path));
            Ok(TerminalStructuralContents {
                argument_index: u32::try_from(argument_index).map_err(|_| malformed())?,
                values,
            })
        }
    }

    struct ComponentContentsMapping<'source, 'destination> {
        source_types: ProgramDeclarations<'source, StructuralTypeDeclaration>,
        source_identities: &'source TerminalIdentityTable,
        destination_types: ProgramDeclarations<'destination, StructuralTypeDeclaration>,
        destination_identities: &'destination TerminalIdentityTable,
    }

    impl ComponentContentsMapping<'_, '_> {
        fn rehome(
            &self,
            source_type: StructuralTypeId,
            destination_type: StructuralTypeId,
            contents: TerminalBoundaryStorageValue,
        ) -> Result<TerminalBoundaryStorageValue, TerminalInterpretError> {
            let (path, source_type, destination_type) =
                self.rehome_path(source_type, destination_type, &contents.path)?;
            let contents = match contents.contents {
                value @ (TerminalStorageContents::Scalar { .. }
                | TerminalStorageContents::Bytes(_)) => value,
                TerminalStorageContents::Case(case) => {
                    let source = self.source_types.declaration(&source_type)?;
                    let source = terminal_psi::find_structural_case(
                        source.shape.sum_cases().ok_or_else(malformed)?,
                        case,
                    )
                    .ok_or_else(malformed)?;
                    let destination = self.destination_types.declaration(&destination_type)?;
                    let destination = terminal_psi::structural_case_by_identity(
                        destination.shape.sum_cases().ok_or_else(malformed)?,
                        self.map_identity(source.identity)?,
                    )
                    .ok_or_else(malformed)?;
                    TerminalStorageContents::Case(destination.id)
                }
            };
            Ok(TerminalBoundaryStorageValue { path, contents })
        }

        fn rehome_path(
            &self,
            mut source_type: StructuralTypeId,
            mut destination_type: StructuralTypeId,
            source_path: &[Path],
        ) -> Result<(Vec<Path>, StructuralTypeId, StructuralTypeId), TerminalInterpretError>
        {
            let mut path = Vec::with_capacity(source_path.len());
            let mut source_case_fields = None;
            let mut destination_case_fields = None;
            for (position, segment) in source_path.iter().enumerate() {
                let source = self.source_types.declaration(&source_type)?;
                let destination = self.destination_types.declaration(&destination_type)?;
                if self.map_identity(source.identity)? != destination.identity {
                    return Err(malformed());
                }
                match segment {
                    Path::Case(case) => {
                        if source_case_fields.is_some() {
                            return Err(malformed());
                        }
                        let source = terminal_psi::find_structural_case(
                            source.shape.sum_cases().ok_or_else(malformed)?,
                            *case,
                        )
                        .ok_or_else(malformed)?;
                        let identity = self.map_identity(source.identity)?;
                        let destination = terminal_psi::structural_case_by_identity(
                            destination.shape.sum_cases().ok_or_else(malformed)?,
                            identity,
                        )
                        .ok_or_else(malformed)?;
                        path.push(Path::Case(destination.id));
                        source_case_fields = Some(source.fields.as_slice());
                        destination_case_fields = Some(destination.fields.as_slice());
                    }
                    Path::Field(field) => {
                        let source_fields = source_case_fields
                            .take()
                            .or_else(|| source.shape.direct_fields())
                            .ok_or_else(malformed)?;
                        let source = source_fields
                            .iter()
                            .find(|candidate| candidate.id == *field)
                            .ok_or_else(malformed)?;
                        let destination_fields = destination_case_fields
                            .take()
                            .or_else(|| destination.shape.direct_fields())
                            .ok_or_else(malformed)?;
                        let destination = terminal_psi::live_field_by_identity(
                            destination_fields,
                            self.map_identity(source.identity)?,
                        )
                        .ok_or_else(malformed)?;
                        if source.relevance.is_erased() {
                            return Err(malformed());
                        }
                        path.push(Path::Field(destination.id));
                        match (
                            source.field_type.as_structural(),
                            destination.field_type.as_structural(),
                        ) {
                            (Some(source), Some(destination)) => {
                                source_type = source;
                                destination_type = destination;
                            }
                            (None, None) if position + 1 == source_path.len() => {}
                            _ => return Err(malformed()),
                        }
                    }
                    Path::FixedIndex(element_index) => {
                        if source_case_fields.is_some() {
                            return Err(malformed());
                        }
                        let (
                            terminal_psi::StructuralTypeShape::FixedArray {
                                element: source,
                                length: source_length,
                            },
                            terminal_psi::StructuralTypeShape::FixedArray {
                                element: destination,
                                length: destination_length,
                            },
                        ) = (&source.shape, &destination.shape)
                        else {
                            return Err(malformed());
                        };
                        if source_length.literal() != destination_length.literal()
                            || !source_length
                                .literal()
                                .is_some_and(|length| *element_index < length)
                        {
                            return Err(malformed());
                        }
                        source_type = *source;
                        destination_type = *destination;
                        path.push(*segment);
                    }
                    Path::Referent => {
                        return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                            "component contents cannot cross a reference",
                        ));
                    }
                }
            }
            if source_case_fields.is_some() {
                return Err(malformed());
            }
            if self.map_identity(self.source_types.declaration(&source_type)?.identity)?
                != self
                    .destination_types
                    .declaration(&destination_type)?
                    .identity
            {
                return Err(malformed());
            }
            Ok((path, source_type, destination_type))
        }

        fn map_identity(
            &self,
            identity: terminal_psi::TerminalIdentityIndex,
        ) -> Result<terminal_psi::TerminalIdentityIndex, TerminalInterpretError> {
            let identity = self
                .source_identities
                .text(identity)
                .filter(|identity| !identity.is_empty())
                .ok_or_else(malformed)?;
            self.destination_identities
                .index_of(identity)
                .ok_or_else(malformed)
        }
    }

    #[cfg(test)]
    pub(crate) mod tests {
        use super::*;
        use semantic_vocabulary::{ScalarType, StructuralCaseId, StructuralFieldId};
        use terminal_psi::{
            BindingRelevance, StructuralCaseDeclaration, StructuralFieldDeclaration,
            StructuralFieldType, StructuralTypeShape, TerminalIdentityTableBuilder,
        };

        struct Fixture {
            identities: TerminalIdentityTable,
            declarations: Vec<StructuralTypeDeclaration>,
            offset: u64,
        }

        impl Fixture {
            fn new(offset: u64) -> Self {
                let mut identities = TerminalIdentityTableBuilder::default();
                if offset != 0 {
                    identities.intern("unrelated table prefix");
                }
                let declaration =
                    |identities: &mut TerminalIdentityTableBuilder, ordinal, identity, shape| {
                        StructuralTypeDeclaration {
                            requires: Vec::new(),
                            quotient_carrier: None,
                            id: StructuralTypeId::new(offset + ordinal).unwrap(),
                            identity: identities.intern(identity),
                            owns_linear_claim: false,
                            copyable: true,
                            no_code_affine_cleanup: true,
                            descriptor_cleanup: false,
                            scalar_restriction: None,
                            shape,
                        }
                    };
                let field = |identities: &mut TerminalIdentityTableBuilder,
                             ordinal,
                             identity,
                             field_type| StructuralFieldDeclaration {
                    inline_carrier: None,
                    id: StructuralFieldId::new(offset + ordinal).unwrap(),
                    identity: identities.intern(identity),
                    relevance: BindingRelevance::Relevant,
                    field_type,
                    storage: None,
                };
                let common = field(
                    &mut identities,
                    11,
                    "flag",
                    StructuralFieldType::Scalar(ScalarType::Boolean),
                );
                let payload = field(
                    &mut identities,
                    12,
                    "bits",
                    StructuralFieldType::Structural(StructuralTypeId::new(offset + 2).unwrap()),
                );
                let case = StructuralCaseDeclaration {
                    id: StructuralCaseId::new(offset + 70).unwrap(),
                    identity: identities.intern("Active"),
                    fields: vec![payload],
                };
                let declarations = vec![
                    declaration(
                        &mut identities,
                        1,
                        "Envelope",
                        StructuralTypeShape::Mixed {
                            fields: vec![common],
                            cases: vec![case],
                        },
                    ),
                    declaration(
                        &mut identities,
                        2,
                        "[bool;2]",
                        StructuralTypeShape::FixedArray {
                            element: StructuralTypeId::new(offset + 3).unwrap(),
                            length: 2.into(),
                        },
                    ),
                    declaration(
                        &mut identities,
                        3,
                        "bool",
                        StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
                    ),
                ];
                Self {
                    identities: identities.build(),
                    declarations,
                    offset,
                }
            }
            fn root(&self) -> StructuralTypeId {
                self.declarations[0].id
            }
            fn payload_path(&self, element: u64) -> Vec<Path> {
                vec![
                    Path::Case(StructuralCaseId::new(self.offset + 70).unwrap()),
                    Path::Field(StructuralFieldId::new(self.offset + 12).unwrap()),
                    Path::FixedIndex(element),
                ]
            }
        }

        fn mapping<'source, 'destination>(
            source: &'source Fixture,
            destination: &'destination Fixture,
        ) -> ComponentContentsMapping<'source, 'destination> {
            ComponentContentsMapping {
                source_types: ProgramDeclarations::new(&source.declarations),
                source_identities: &source.identities,
                destination_types: ProgramDeclarations::new(&destination.declarations),
                destination_identities: &destination.identities,
            }
        }

        fn scalar(path: Vec<Path>) -> TerminalBoundaryStorageValue {
            TerminalBoundaryStorageValue {
                path,
                contents: TerminalStorageContents::Scalar {
                    value: crate::TerminalScalarValue::Boolean(true),
                    stored_scalar: ScalarType::Boolean,
                },
            }
        }

        #[test]
        fn component_contents_rehome_nominal_cases_payloads_and_array_cells() {
            let source = Fixture::new(0);
            let destination = Fixture::new(100);
            let mapping = mapping(&source, &destination);
            let selected = mapping
                .rehome(
                    source.root(),
                    destination.root(),
                    TerminalBoundaryStorageValue {
                        path: vec![],
                        contents: TerminalStorageContents::Case(StructuralCaseId::new(70).unwrap()),
                    },
                )
                .unwrap();
            assert_eq!(
                selected.contents,
                TerminalStorageContents::Case(StructuralCaseId::new(170).unwrap())
            );
            let payload = mapping
                .rehome(
                    source.root(),
                    destination.root(),
                    scalar(source.payload_path(1)),
                )
                .unwrap();
            assert_eq!(payload.path, destination.payload_path(1));
            let common = mapping
                .rehome(
                    source.root(),
                    destination.root(),
                    scalar(vec![Path::Field(StructuralFieldId::new(11).unwrap())]),
                )
                .unwrap();
            assert_eq!(
                common.path,
                vec![Path::Field(StructuralFieldId::new(111).unwrap())]
            );
        }

        #[test]
        fn component_contents_refuse_reference_crossings_bad_indexes_and_nominal_substitutions() {
            let source = Fixture::new(0);
            let mut destination = Fixture::new(100);
            assert!(
                mapping(&source, &destination)
                    .rehome(
                        source.root(),
                        destination.root(),
                        scalar(vec![Path::Referent])
                    )
                    .is_err()
            );
            assert!(
                mapping(&source, &destination)
                    .rehome(
                        source.root(),
                        destination.root(),
                        scalar(source.payload_path(2))
                    )
                    .is_err()
            );
            let mut identities = destination.identities.clone().into_builder();
            destination.declarations[2].identity = identities.intern("OtherBool");
            destination.identities = identities.build();
            assert!(
                mapping(&source, &destination)
                    .rehome(
                        source.root(),
                        destination.root(),
                        scalar(source.payload_path(1))
                    )
                    .is_err()
            );
            let mut destination = Fixture::new(100);
            let mut identities = destination.identities.clone().into_builder();
            let wrong = identities.intern("unrelated_bits");
            let StructuralTypeShape::Mixed { cases, .. } = &mut destination.declarations[0].shape
            else {
                panic!("fixture is mixed")
            };
            cases[0].fields[0].identity = wrong;
            destination.identities = identities.build();
            assert!(
                mapping(&source, &destination)
                    .rehome(
                        source.root(),
                        destination.root(),
                        scalar(source.payload_path(1))
                    )
                    .is_err()
            );
        }
    }
}
pub(crate) mod component_empty_arguments {
    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::values::{StructuralRuntimePlace, TerminalStructuralValue};
    use crate::engine::execution::TerminalExecution;
    use crate::storage::storage_addresses::{ScalarStorageAddress, ScalarStorageAddressKey};
    use crate::storage::structural_storage::contents;
    use terminal_psi::{StructuralAccess, StructuralTypeShape};

    pub(crate) struct SharedEmptyArgument {
        index: usize,
        original: ScalarStorageAddress,
    }

    fn empty_record(
        execution: &TerminalExecution<'_>,
        value: &TerminalStructuralValue,
    ) -> Result<bool, TerminalInterpretError> {
        if !matches!(execution.structural_declaration(value.structural_type)?.shape,
        StructuralTypeShape::Record { ref fields, .. } if fields.is_empty())
        {
            return Ok(false);
        }
        let described = contents::describe(execution, value, StructuralAccess::SharedBorrow, None)?;
        Ok(described.leaves.is_empty()
            && !described.requires_atomic_storage
            && !described.requires_erased_establishment)
    }

    impl TerminalExecution<'_> {
        pub(crate) fn prepare_shared_empty_argument(
            &self,
            value: &TerminalStructuralValue,
            index: usize,
        ) -> Result<Option<SharedEmptyArgument>, TerminalInterpretError> {
            if !empty_record(self, value)? {
                return Ok(None);
            }
            let home = StructuralRuntimePlace::from(value);
            let key = ScalarStorageAddressKey::Scalar(home.clone());
            let formed = self
                .local_structural_identities
                .owns_allocated(home.opaque_identity)
                && self.structural_values.values().any(|owner| {
                    owner.opaque_identity == home.opaque_identity
                        && home.path.starts_with(&owner.path)
                });
            if !formed && !self.component_borrowed_addresses.contains_key(&key) {
                return Ok(None);
            }
            Ok(Some(SharedEmptyArgument {
                index,
                original: self.scalar_storage_address(&key)?,
            }))
        }
    }

    impl SharedEmptyArgument {
        pub(crate) fn establish(
            &self,
            child: &mut TerminalExecution<'_>,
        ) -> Result<(), TerminalInterpretError> {
            let entry = child.program.verified_machine(child.current_machine)?;
            let parameter = entry
                .structural_parameters
                .get(self.index)
                .ok_or_else(malformed)?;
            if parameter.access != StructuralAccess::SharedBorrow {
                return Err(malformed());
            }
            let value = child.structural_value(parameter.place)?;
            if !empty_record(child, value)? {
                return Err(malformed());
            }
            let key = ScalarStorageAddressKey::Scalar(StructuralRuntimePlace::from(value));
            if let Some(existing) = child.component_borrowed_addresses.get(&key) {
                return if existing == &self.original {
                    Ok(())
                } else {
                    Err(malformed())
                };
            }
            child
                .component_borrowed_addresses
                .insert(key, self.original.clone())?;
            Ok(())
        }
    }
}
pub(crate) mod component_scalar_arguments {
    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::values::{StructuralRuntimePlace, TerminalStructuralValue};
    use crate::embedding::program::TerminalProgram;
    use crate::engine::execution::TerminalExecution;
    use crate::storage::primitive_storage::PrimitiveStorageEndpoint;
    use crate::storage::scalar_storage::{ScalarCell, ScalarLoanScope};
    use crate::storage::storage_addresses::{ScalarStorageAddress, ScalarStorageAddressKey};
    use crate::storage::structural_storage::contents::{self, StorageLeafKind};
    use semantic_vocabulary::{CanonicalStructuralPathSegment, ScalarType, StructuralTypeId};
    use std::collections::BTreeMap;
    use terminal_psi::{StructuralAccess, StructuralFieldType};

    pub(crate) struct PreparedScalarArgument {
        index: usize,
        root: StructuralRuntimePlace,
        access: StructuralAccess,
        cells: Vec<PreparedScalarCell>,
    }

    struct PreparedScalarCell {
        source: ScalarStorageAddressKey,
        path: Vec<CanonicalStructuralPathSegment>,
        scalar: ScalarType,
        stored_scalar: ScalarType,
        restriction: Option<StructuralFieldType>,
        address: ScalarStorageAddress,
        parent_addresses: Vec<ScalarStorageAddress>,
    }

    pub(crate) struct LentScalarArgument {
        index: usize,
        access: StructuralAccess,
        cells: Vec<(PreparedScalarCell, ScalarCell)>,
    }

    fn reject() -> TerminalInterpretError {
        TerminalInterpretError::UnsupportedSemanticVariant(
            "component borrowed input needs its original materialized storage carrier",
        )
    }

    fn scalar_key(
        endpoint: &PrimitiveStorageEndpoint,
    ) -> Result<ScalarStorageAddressKey, TerminalInterpretError> {
        match endpoint {
            PrimitiveStorageEndpoint::Scalar(home) => {
                Ok(ScalarStorageAddressKey::Scalar(home.clone()))
            }
            PrimitiveStorageEndpoint::ScalarField(field) => {
                Ok(ScalarStorageAddressKey::Field(field.clone()))
            }
            _ => Err(reject()),
        }
    }

    fn parent(key: &ScalarStorageAddressKey) -> &StructuralRuntimePlace {
        match key {
            ScalarStorageAddressKey::Scalar(home) => home,
            ScalarStorageAddressKey::Field(field) => &field.parent,
        }
    }

    fn cell<'source>(
        source: &'source TerminalExecution<'_>,
        key: &ScalarStorageAddressKey,
    ) -> Option<&'source ScalarCell> {
        match key {
            ScalarStorageAddressKey::Scalar(home) => source.structural_primitive_storage.get(home),
            ScalarStorageAddressKey::Field(field) => source.structural_scalar_fields.get(field),
        }
    }

    fn cell_mut<'source>(
        source: &'source mut TerminalExecution<'_>,
        key: &ScalarStorageAddressKey,
    ) -> Option<&'source mut ScalarCell> {
        match key {
            ScalarStorageAddressKey::Scalar(home) => {
                source.structural_primitive_storage.get_mut(home)
            }
            ScalarStorageAddressKey::Field(field) => source.structural_scalar_fields.get_mut(field),
        }
    }

    impl TerminalExecution<'_> {
        pub(crate) fn prepare_component_scalar_argument(
            &self,
            value: &TerminalStructuralValue,
            index: usize,
            access: StructuralAccess,
            provider: &TerminalProgram<'_>,
            destination_type: StructuralTypeId,
        ) -> Result<PreparedScalarArgument, TerminalInterpretError> {
            let description = contents::describe(self, value, access, None)?;
            if description.requires_erased_establishment
                || description.requires_atomic_storage
                || description.leaves.is_empty()
            {
                return Err(reject());
            }
            let root = StructuralRuntimePlace::from(value);
            let mut cells = Vec::with_capacity(description.leaves.len());
            for leaf in description.leaves {
                let StorageLeafKind::Scalar {
                    destination,
                    scalar,
                    restriction,
                    stored_scalar,
                } = leaf.kind
                else {
                    return Err(reject());
                };
                let source = scalar_key(&destination)?;
                let actual = cell(self, &source).ok_or_else(reject)?;
                if actual.scalar_type() != stored_scalar {
                    return Err(malformed());
                }
                let source_parent = parent(&source);
                if source_parent.opaque_identity != root.opaque_identity
                    || !source_parent.path.starts_with(&root.path)
                {
                    return Err(reject());
                }
                let parent_addresses = (root.path.len()..=source_parent.path.len())
                    .map(|length| {
                        self.scalar_storage_address(&ScalarStorageAddressKey::Scalar(
                            StructuralRuntimePlace {
                                opaque_identity: root.opaque_identity,
                                path: source_parent.path[..length].to_vec(),
                            },
                        ))
                    })
                    .collect::<Result<_, _>>()?;
                cells.push(PreparedScalarCell {
                    path: self.component_storage_path(
                        value.structural_type,
                        destination_type,
                        provider,
                        &leaf.path,
                    )?,
                    scalar,
                    stored_scalar,
                    restriction,
                    address: self.scalar_storage_address(&source)?,
                    source,
                    parent_addresses,
                });
            }
            Ok(PreparedScalarArgument {
                index,
                root,
                access,
                cells,
            })
        }
    }

    impl PreparedScalarArgument {
        pub(crate) fn mutated_home(&self) -> Option<&StructuralRuntimePlace> {
            (self.access == StructuralAccess::MutableBorrow).then_some(&self.root)
        }

        pub(crate) fn lend(
            self,
            source: &mut TerminalExecution<'_>,
            lifetime: &ScalarLoanScope,
            views: &mut BTreeMap<ScalarStorageAddressKey, (StructuralAccess, ScalarCell)>,
        ) -> Result<LentScalarArgument, TerminalInterpretError> {
            let mut cells = Vec::with_capacity(self.cells.len());
            for prepared in self.cells {
                let borrowed = if let Some((access, previous)) = views.get(&prepared.source) {
                    if *access != StructuralAccess::SharedBorrow
                        || self.access != StructuralAccess::SharedBorrow
                    {
                        return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                            "component inputs overlap an exclusive scalar home",
                        ));
                    }
                    previous.clone()
                } else {
                    let original = cell_mut(source, &prepared.source).ok_or_else(malformed)?;
                    let borrowed = lifetime.lend(original, self.access)?;
                    views.insert(prepared.source.clone(), (self.access, borrowed.clone()));
                    borrowed
                };
                cells.push((prepared, borrowed));
            }
            Ok(LentScalarArgument {
                index: self.index,
                access: self.access,
                cells,
            })
        }
    }

    impl LentScalarArgument {
        pub(crate) fn establish(
            &self,
            child: &mut TerminalExecution<'_>,
        ) -> Result<(), TerminalInterpretError> {
            let entry = child.program.verified_machine(child.current_machine)?;
            let parameter = entry
                .structural_parameters
                .get(self.index)
                .ok_or_else(malformed)?;
            if parameter.access != self.access {
                return Err(malformed());
            }
            let value = child.structural_value(parameter.place)?.clone();
            let root = StructuralRuntimePlace::from(&value);
            let description = contents::describe(child, &value, self.access, None)?;
            if description.requires_erased_establishment
                || description.requires_atomic_storage
                || description.leaves.len() != self.cells.len()
            {
                return Err(reject());
            }
            let mut destinations = BTreeMap::new();
            for leaf in description.leaves {
                if destinations.insert(leaf.path, leaf.kind).is_some() {
                    return Err(malformed());
                }
            }
            let mut bindings = Vec::with_capacity(self.cells.len());
            let mut addresses = BTreeMap::new();
            for (prepared, original) in &self.cells {
                let Some(StorageLeafKind::Scalar {
                    destination,
                    scalar,
                    restriction,
                    stored_scalar,
                }) = destinations.remove(&prepared.path)
                else {
                    return Err(reject());
                };
                if scalar != prepared.scalar
                    || stored_scalar != prepared.stored_scalar
                    || restriction != prepared.restriction
                {
                    return Err(reject());
                }
                let key = scalar_key(&destination)?;
                let needs_binding = if let Some(existing) = cell(child, &key) {
                    if self.access != StructuralAccess::SharedBorrow || existing != original {
                        return Err(malformed());
                    }
                    false
                } else {
                    true
                };
                let destination_parent = parent(&key);
                if destination_parent.opaque_identity != root.opaque_identity
                    || !destination_parent.path.starts_with(&root.path)
                    || destination_parent.path.len() - root.path.len() + 1
                        != prepared.parent_addresses.len()
                {
                    return Err(malformed());
                }
                for (suffix, original_address) in prepared.parent_addresses.iter().enumerate() {
                    let home = StructuralRuntimePlace {
                        opaque_identity: root.opaque_identity,
                        path: destination_parent.path[..root.path.len() + suffix].to_vec(),
                    };
                    insert_address(
                        &mut addresses,
                        ScalarStorageAddressKey::Scalar(home),
                        original_address,
                    )?;
                }
                insert_address(&mut addresses, key.clone(), &prepared.address)?;
                if needs_binding {
                    bindings.push((key, original.clone()));
                }
            }
            if !destinations.is_empty() {
                return Err(malformed());
            }
            for (key, address) in &addresses {
                if let Some(existing) = child.component_borrowed_addresses.get(key)
                    && (self.access != StructuralAccess::SharedBorrow || existing != address)
                {
                    return Err(malformed());
                }
            }
            for (key, original) in bindings {
                match key {
                    ScalarStorageAddressKey::Scalar(home) => {
                        child
                            .structural_primitive_storage
                            .insert_cell(home, original)?;
                    }
                    ScalarStorageAddressKey::Field(field) => {
                        child
                            .structural_scalar_fields
                            .insert_cell(field, original)?;
                    }
                }
            }
            for (key, address) in addresses {
                if !child.component_borrowed_addresses.contains_key(&key) {
                    child.component_borrowed_addresses.insert(key, address)?;
                }
            }
            Ok(())
        }
    }

    fn insert_address(
        addresses: &mut BTreeMap<ScalarStorageAddressKey, ScalarStorageAddress>,
        key: ScalarStorageAddressKey,
        original: &ScalarStorageAddress,
    ) -> Result<(), TerminalInterpretError> {
        if addresses
            .insert(key, original.clone())
            .is_some_and(|previous| previous != *original)
        {
            return Err(malformed());
        }
        Ok(())
    }
}
pub(crate) use component_arguments::{ComponentCallScope, ComponentInputCustody};

use crate::boundary::effects::results::TerminalEffectResult;
use crate::boundary::effects::{TerminalEffect, TerminalEffectHandler, TerminalEffectRejection};
use crate::boundary::errors::{TerminalInterpretError, malformed};
use crate::boundary::results::TerminalExecutionStatus;
use crate::boundary::values::StructuralRuntimePlace;
use crate::engine::custody::{
    bind_active_structural_arguments, bind_arguments, bind_boundary_arguments, complete_claims,
    remove_affine_root, validate_boundary_requirements,
};
use crate::engine::execution::{
    OperationFlow, SuspendedCall, SuspendedCallResult, TerminalExecution, dispatched_operation,
    operation_kind,
};
use crate::storage::runtime_rows::RuntimeRows;
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    ClaimTransfer, OperationKind, StructuralAccess, StructuralMultiplicity, TerminalMachineResult,
};

pub(crate) struct PendingBoundaryCall {
    pub(crate) site: crate::boundary::results::TerminalEffectWait,
    pub(crate) awaiting_provider_readiness: bool,
    arguments: crate::boundary::effects::byte_buffers::BoundaryArguments,
    remaining_claims: BTreeMap<semantic_vocabulary::ClaimId, crate::engine::execution::LiveClaim>,
    effect: TerminalEffect,
}

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
        dispatched_operation!(operation => CallUnit {
            callee, arguments: ref scalar_argument_ids, ref structural_arguments,
            ref claim_transfers, ..
        });
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
        dispatched_operation!(operation => CallStructuralScalar {
            callee, ref arguments, ref structural_arguments, ref claim_transfers, ..
        });
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

    pub(crate) fn execute_select_implementation(
        &mut self,
        operation: &terminal_psi::Operation,
        table: u32,
        program: &std::sync::Arc<crate::embedding::program::AdmittedTerminalProgram>,
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
            crate::boundary::values::TerminalScalarValue::Integer {
                scalar_type,
                value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(table)),
            },
        )?;
        Ok(OperationFlow::Advance)
    }

    pub(crate) fn execute_call_dynamic(
        &mut self,
        operation: &terminal_psi::Operation,
        program: &std::sync::Arc<crate::embedding::program::AdmittedTerminalProgram>,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        dispatched_operation!(operation => CallDynamic {
            selector, interface, slot, ref arguments, ref family_actuals, ref structural_arguments,
            ref claim_transfers, ..
        });
        let Some(crate::boundary::values::TerminalScalarValue::Integer {
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
                    crate::engine::execution::ResultClaimOrigin::CalleeReturn,
                )?,
        }
        Ok(OperationFlow::Redispatch)
    }

    pub(crate) fn execute_call_structural(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        dispatched_operation!(operation => CallStructural {
            callee, ref structural_arguments, ref claim_transfers, ..
        });
        let result = operation.result.structural().cloned().ok_or(
            TerminalInterpretError::VerifiedOperationContractInvalid {
                reason: "structural call operation has no structural result",
            },
        )?;
        let prepared_arguments =
            self.prepare_structural_call_arguments(callee, structural_arguments)?;
        self.begin_structural_result_call(
            callee,
            result,
            &[],
            structural_arguments,
            prepared_arguments,
            claim_transfers,
            crate::engine::execution::ResultClaimOrigin::CalleeReturn,
        )?;
        Ok(OperationFlow::Redispatch)
    }

    pub(crate) fn execute_call_structural_with_scalar_arguments(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        dispatched_operation!(operation => CallStructuralWithScalarArguments {
            callee, ref arguments, ref structural_arguments, ref claim_transfers, ..
        });
        let result = operation.result.structural().cloned().ok_or(
            TerminalInterpretError::VerifiedOperationContractInvalid {
                reason: "structural call operation has no structural result",
            },
        )?;
        let (scalar_arguments, prepared_arguments) =
            self.resolve_call_arguments(callee, arguments, structural_arguments)?;
        self.begin_structural_result_call(
            callee,
            result,
            &scalar_arguments,
            structural_arguments,
            prepared_arguments,
            claim_transfers,
            crate::engine::execution::ResultClaimOrigin::CalleeReturn,
        )?;
        Ok(OperationFlow::Redispatch)
    }

    pub(crate) fn execute_boundary_call(
        &mut self,
        operation: &terminal_psi::Operation,
        handler: &mut impl TerminalEffectHandler,
        meter: &mut psi_work::WorkMeter,
        components: &mut crate::embedding::instance::ComponentBindings<'_, 'artifact>,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        if let Some(pending) = self.awaiting_effect.as_ref() {
            if let Some(status) = self.boundary_effect_wait(operation, handler, pending)? {
                self.awaiting_effect
                    .as_mut()
                    .ok_or_else(malformed)?
                    .awaiting_provider_readiness =
                    matches!(status, TerminalExecutionStatus::AwaitingEffect(_));
                return Ok(OperationFlow::Yield(status));
            }
            let pending = self.awaiting_effect.take().ok_or_else(malformed)?;
            return self.complete_boundary_effect(operation, handler, *pending);
        }
        dispatched_operation!(operation => BoundaryCall {
            boundary, arguments: ref scalar_argument_ids, ref structural_arguments,
            ref completion_receipts, ..
        });
        let program = std::sync::Arc::clone(&self.program);
        let boundary_declaration = program.verified_boundary_machine(&boundary)?;
        if boundary_declaration
            .declaration
            .as_ref()
            .map(|row| row.execution)
            == Some(terminal_psi::CheckedBoundaryExecution::EmptyRunRelease)
        {
            self.release_empty_run(operation, boundary_declaration)?;
            return Ok(OperationFlow::Advance);
        }
        if boundary_declaration
            .declaration
            .as_ref()
            .is_some_and(|callable| callable.execution.static_interrupt_operation().is_some())
        {
            return Err(malformed());
        }
        if let Some(placement_operation) = boundary_declaration
            .declaration
            .as_ref()
            .and_then(|declaration| declaration.execution.placement_operation())
        {
            let plan = program
                .access_lifecycle_plans()
                .iter()
                .find(|plan| plan.boundary == boundary)
                .ok_or_else(malformed)?;
            match placement_operation {
                terminal_psi::TerminalAccessLifecycleOperation::View => {
                    self.establish_access_view(operation, plan)?;
                    return Ok(OperationFlow::Advance);
                }
                terminal_psi::TerminalAccessLifecycleOperation::Initialize => {
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
                    let (binding, supplied) =
                        self.lifecycle_profile(plan, 0, &storage, &profile)?;
                    let image = crate::storage::provider_storage::value_image::prepare(
                        self,
                        plan,
                        value,
                        layout_plans::ByteOrder::LittleEndian,
                    )?;
                    let mut prepared = self.provider_storage.prepare_initial_image(
                        binding,
                        supplied,
                        &plan.placement,
                        image,
                    )?;
                    if matches!(
                        plan.storage.owner,
                        terminal_psi::TerminalAccessOwner::Run { .. }
                    ) && plan.placement.layout().size != Some(binding.length)
                        && prepared.is_ok()
                    {
                        prepared = Err(crate::storage::provider_storage::initialization::InitializationRejection::IncompatibleSupply);
                    }
                    match prepared {
                        Err(rejection) => self.reject_access_transfer(
                            operation,
                            boundary_declaration,
                            plan,
                            rejection,
                        )?,
                        Ok(prepared) => self.complete_access_initialization(
                            operation,
                            boundary_declaration,
                            plan,
                            prepared,
                        )?,
                    }
                    return Ok(OperationFlow::Advance);
                }
                terminal_psi::TerminalAccessLifecycleOperation::MoveOut => {
                    self.extract_access_resident(operation, boundary_declaration, plan)?;
                    return Ok(OperationFlow::Advance);
                }
                terminal_psi::TerminalAccessLifecycleOperation::Relocate => {
                    self.relocate_access_run(operation, boundary_declaration, plan)?;
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
                terminal_psi::TargetMachineCallable::Boundary(_) => {}
            }
        }
        let supply = self.boundary_supplies.get(&boundary).copied();
        if supply == Some(crate::boundary::effects::AdmittedBoundarySupply::DeferredComponent) {
            return Err(TerminalInterpretError::EffectRejected {
                operation: operation.id,
                rejection: TerminalEffectRejection::new(
                    "component provider was called before its admitted installation",
                ),
            });
        }
        if let Some(crate::boundary::effects::AdmittedBoundarySupply::ComponentProvider {
            ..
        }) = supply
        {
            return self.execute_component_call(operation, &scalar_arguments, meter, components);
        }
        if matches!(
            supply,
            Some(crate::boundary::effects::AdmittedBoundarySupply::CheckedProvider(_))
        ) || (supply.is_none() && self.program.has_provider(&boundary))
        {
            let owned_arrays = boundary_declaration
                .structural_parameters
                .iter()
                .enumerate()
                .filter(|(_, parameter)| {
                    parameter.access == StructuralAccess::Owned
                        && terminal_semantics::scalar_array_leaf_shape(
                            self.structural_types().values(),
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
                &self.structural_types(),
                self.structural_cases.as_map(),
                self.account.as_ref(),
            )?;
            validate_boundary_requirements(boundary_declaration, |index| {
                opaque_positions
                    .iter()
                    .position(|position| *position == index)
                    .and_then(|position| boundary_arguments.values.get(position))
            })?;
            let Some(crate::boundary::effects::AdmittedBoundarySupply::CheckedProvider(callee_id)) =
                supply
            else {
                return Err(TerminalInterpretError::ProviderInstallationMissing(
                    boundary,
                ));
            };
            let callee = self.program.verified_machine(callee_id)?;
            let mut prepared_arguments = boundary_arguments.into_call_arguments(
                &select_positions(&callee.structural_parameters, &opaque_positions)?,
                &self.structural_types(),
                self.structural_cases.as_map(),
                None,
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
                {
                    return Err(malformed());
                }
                prepared_arguments.scalar_arrays.publish(
                    formal.place,
                    self.prepare_scalar_array_argument(callee, formal, argument)?,
                )?;
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
                        handler,
                    )?;
                    self.begin_structural_result_call(
                        callee_id,
                        result.clone(),
                        &scalar_arguments,
                        structural_arguments,
                        prepared_arguments,
                        &claim_transfers,
                        crate::engine::execution::ResultClaimOrigin::CheckedProvider,
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
        if self.execute_access_boundary(
            operation,
            boundary_declaration,
            &scalar_arguments,
            structural_arguments,
        )? {
            return Ok(OperationFlow::Advance);
        }
        if boundary_declaration
            .requires_checked_non_observation(|id| self.structural_types().get(&id))
        {
            return Err(TerminalInterpretError::RuntimeSupplySelectionRejected {
                boundary,
                reason: "opaque host provider requires an admitted write-only non-observation judgment bound to its exact selected implementation and receipt",
            });
        }
        let requirements = handler.storage_requirements(boundary, &scalar_arguments);
        let original_storage = requirements.original;
        let mut boundary_arguments = self
            .resolve_boundary_arguments(
                &boundary_declaration.structural_parameters,
                structural_arguments,
                requirements.arguments,
                &boundary_declaration.normal_qualifications,
                original_storage,
            )
            .map_err(|error| error.in_argument_preparation("resolve host argument tuple"))?;
        bind_active_structural_arguments(
            &boundary_declaration.structural_parameters,
            &boundary_arguments.values,
            &self.structural_types(),
            self.structural_cases.as_map(),
            self.account.as_ref(),
        )
        .map_err(|error| error.in_argument_preparation("bind host argument declarations"))?;
        validate_boundary_requirements(boundary_declaration, |index| {
            boundary_arguments.values.get(index)
        })?;
        crate::boundary::effects::boundary_predicates::validate_input_requirements(
            boundary_declaration,
            &scalar_arguments,
            self,
            structural_arguments,
            &boundary_arguments.values,
        )?;
        self.preflight_boundary_result(boundary_declaration, &operation.result)?;
        let reference_result = operation.result.structural().is_some_and(|result| {
            self.supports_captured_reference_result(boundary_declaration, result)
        });
        if reference_result && !handler.accepts_reference_results(boundary) {
            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                "host reference results require an explicit captured-input interface",
            ));
        }

        let remaining_claims = complete_claims(
            &self.live_claims,
            structural_arguments,
            completion_receipts,
            &boundary_declaration.structural_parameters,
        )?;
        boundary_arguments.capture_reference_inputs(boundary_declaration, structural_arguments)?;
        if reference_result {
            boundary_arguments
                .storage_inputs
                .require_result_captures(boundary_declaration)?;
        }

        boundary_arguments
            .prepare_original_storage(self, handler, boundary, original_storage)
            .map_err(|error| error.in_argument_preparation("realize original host storage"))?;
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
        let site = crate::TerminalEffectWait {
            program: self.program.identity.clone(),
            machine: self.current_machine,
            block: self.current,
            operation: operation.id,
            boundary,
        };
        let effect = TerminalEffect::BoundaryCall {
            site: site.clone(),
            arguments: scalar_arguments,
            structural_arguments: exported_values,
            byte_sequence_arguments: std::mem::take(&mut boundary_arguments.bytes),
            completion_receipts: completion_receipts.clone(),
            result: boundary_declaration.result.clone(),
        };
        let mut pending = PendingBoundaryCall {
            awaiting_provider_readiness: false,
            site,
            arguments: boundary_arguments,
            remaining_claims,
            effect,
        };
        if let Some(status) = self.boundary_effect_wait(operation, handler, &pending)? {
            pending.awaiting_provider_readiness =
                matches!(status, TerminalExecutionStatus::AwaitingEffect(_));
            self.awaiting_effect = Some(Box::new(pending));
            Ok(OperationFlow::Yield(status))
        } else {
            self.complete_boundary_effect(operation, handler, pending)
                .map_err(|error| error.in_argument_preparation("execute captured host call"))
        }
    }

    fn boundary_effect_wait(
        &self,
        operation: &terminal_psi::Operation,
        handler: &mut impl TerminalEffectHandler,
        pending: &PendingBoundaryCall,
    ) -> Result<Option<TerminalExecutionStatus>, TerminalInterpretError> {
        operation_kind!(operation => BoundaryCall { boundary, .. });
        if pending.site.program != self.program.identity
            || pending.site.machine != self.current_machine
            || pending.site.block != self.current
            || pending.site.operation != operation.id
            || pending.site.boundary != *boundary
        {
            return Err(malformed());
        }
        let ready = handler
            .poll_effect_readiness(&pending.effect)
            .map_err(|rejection| TerminalInterpretError::EffectRejected {
                operation: operation.id,
                rejection,
            })?;
        if !ready && operation.suspension_crossing.is_none() {
            return Err(TerminalInterpretError::EffectRejected {
                operation: operation.id,
                rejection: crate::boundary::effects::TerminalEffectRejection::new(
                    "host readiness cannot suspend a call without a checked suspension crossing",
                ),
            });
        }
        if !ready {
            return Ok(Some(TerminalExecutionStatus::AwaitingEffect(
                pending.site.clone(),
            )));
        }
        let computed = handler
            .poll_host_computation(&pending.effect)
            .map_err(|rejection| TerminalInterpretError::EffectRejected {
                operation: operation.id,
                rejection,
            })?;
        Ok(
            (!computed).then_some(TerminalExecutionStatus::HostComputationPending(
                pending.site.clone(),
            )),
        )
    }

    fn complete_boundary_effect(
        &mut self,
        operation: &terminal_psi::Operation,
        handler: &mut impl TerminalEffectHandler,
        pending: PendingBoundaryCall,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        operation_kind!(operation => BoundaryCall { boundary, structural_arguments, .. });
        let PendingBoundaryCall {
            arguments: mut boundary_arguments,
            remaining_claims,
            effect,
            ..
        } = pending;
        let program = std::sync::Arc::clone(&self.program);
        let boundary_declaration = program.verified_boundary_machine(boundary)?;
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
            let crash = self.admit_boundary_crash(boundary_declaration, &effect, cause)?;
            self.effects.push(effect);
            self.crash = Some(crash.clone());
            return Ok(OperationFlow::Yield(TerminalExecutionStatus::Crashed(
                crash,
            )));
        }
        if matches!(&returned, TerminalEffectResult::Reference(_))
            && !boundary_declaration
                .result_reference_regions
                .iter()
                .any(|region| {
                    matches!(
                        region.origin,
                        terminal_psi::TerminalDynamicReferenceOrigin::Invocation { .. }
                    )
                })
        {
            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                "materialized host reference result has no invocation-owned public region",
            ));
        }
        boundary_arguments
            .validate_writeback(self, boundary_declaration)
            .map_err(|error| error.in_argument_preparation("restore captured host storage"))?;
        let prepared_reference =
            if let TerminalEffectResult::CapturedReference(selection) = &returned {
                Some(boundary_arguments.prepare_reference_result(
                    self,
                    boundary_declaration,
                    operation.result.structural().ok_or_else(malformed)?,
                    selection,
                )?)
            } else {
                None
            };
        if let TerminalEffectResult::Structural(value) = &returned {
            boundary_arguments.reserve_host_result(&mut self.local_structural_identities, value)?;
        }
        boundary_arguments.commit_identity_cursor(self);
        if let Some(prepared) = prepared_reference {
            self.commit_boundary_reference_result(
                operation.result.structural().ok_or_else(malformed)?,
                prepared,
            )?;
        } else {
            self.commit_boundary_result(&operation.result, &boundary_declaration.result, returned)
                .map_err(|error| error.in_argument_preparation("commit host result"))?;
        }
        for (argument, parameter) in structural_arguments
            .iter()
            .zip(&boundary_declaration.structural_parameters)
            .filter(|(argument, parameter)| {
                argument.path.is_empty()
                    && parameter.access == StructuralAccess::Owned
                    && parameter.multiplicity != StructuralMultiplicity::Unrestricted
            })
        {
            match self.structural_values.remove(&argument.place) {
                Some(retired) => {
                    let origin = self
                        .reference_referents
                        .get(&StructuralRuntimePlace::from(&retired))
                        .cloned();
                    self.retired_structural_homes
                        .push((argument.place, retired, origin));
                }
                None => {
                    return Err(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                        argument.place,
                    ));
                }
            }
            if parameter.multiplicity == StructuralMultiplicity::Affine {
                remove_affine_root(&mut self.live_affine_frontier, argument.place);
            }
        }
        self.live_claims = remaining_claims;
        if let terminal_psi::OperationResult::Structural(result) = &operation.result {
            for binding in &result.claims {
                crate::engine::custody::bind_result_claim(
                    &mut self.live_claims,
                    binding,
                    result.place,
                    result.multiplicity,
                )?;
            }
        }
        boundary_arguments
            .commit(self)
            .map_err(|error| error.in_argument_preparation("commit captured host storage"))?;
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
        dispatched_operation!(operation => PortWrite { service, port, value });
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
        program: &std::sync::Arc<crate::embedding::program::AdmittedTerminalProgram>,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        dispatched_operation!(operation => Call { callee, ref arguments, .. });
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
    fn family_actual_integer(
        &self,
        actual: semantic_vocabulary::ValueId,
    ) -> Result<i128, TerminalInterpretError> {
        match self.values.get(&actual).copied() {
            Some(crate::boundary::values::TerminalScalarValue::Integer {
                value: semantic_vocabulary::IntegerValue::Signed(value),
                ..
            }) => Ok(value),
            Some(crate::boundary::values::TerminalScalarValue::Integer {
                value: semantic_vocabulary::IntegerValue::Unsigned(value),
                ..
            }) => i128::try_from(value).map_err(|_| malformed()),
            Some(_) => Err(malformed()),
            None => Err(TerminalInterpretError::VerifiedValueMissing(actual)),
        }
    }
}

fn family_tuple_member_value(identity: &str) -> Option<i128> {
    identity
        .strip_prefix("named(integer-const(")
        .and_then(|inner| inner.strip_suffix("))"))
        .and_then(|inner| inner.parse::<i128>().ok())
}
