//! Prepared arguments enter one saved-frame transition for every result form.
//! Argument preparation owns readable storage, aliasing and domain admission;
//! a structural result does not introduce another input-path/shape policy.
//! Capture returned referents from the caller's resident argument storage before
//! preparing owned copies. Copies create independent descriptor homes, not new
//! referents; their staged cells are not resident until the call commits. The
//! completion owner publishes the result only after the callee returns.

use crate::byte_sequences::binding::ByteSequenceBinding;
use crate::calls::arguments::StructuralCallArguments;
use crate::custody::bind_arguments;
use crate::custody::transfer_claims;
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::SuspendedCall;
use crate::execution::SuspendedCallResult;
use crate::execution::TerminalExecution;
use crate::values::{TerminalScalarValue, TerminalStructuralValue};
use semantic_vocabulary::{MachineId, PlaceId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    ClaimTransfer, StructuralAccess, StructuralArgument, StructuralMultiplicity,
    StructuralOperationResult, StructuralParameterDeclaration, StructuralTypeShape,
    TerminalMachineResult,
};

/// Whether the borrowed parameter directly rebinds the argument's resolved
/// value — same type, matching borrow access, and
/// no qualification delta on either side.
fn is_direct_borrowed_rebinding(
    parameter: &StructuralParameterDeclaration,
    argument: &StructuralArgument,
    resolved: &TerminalStructuralValue,
    declarations: &crate::program::ProgramDeclarations<'_, terminal_psi::StructuralTypeDeclaration>,
) -> bool {
    parameter.structural_type == resolved.structural_type
        && terminal_semantics::view_descriptor_parameter(parameter, declarations.values())
        && parameter.access == argument.access
        && matches!(
            parameter.access,
            terminal_psi::reading_borrow_pair_pattern!()
        )
        && parameter.qualifications.is_empty()
        && parameter.projected_qualifications.is_empty()
        && resolved.qualifications.is_empty()
}

/// Whole owned transfers end their source binding. A projected transfer ends
/// only subtree custody, even if it consumes the final affine child: scalar
/// siblings and the scheduled home retirement still name the root's storage.
/// This is the same storage/custody distinction constructor operands retain.
/// Resolve all transfers first so a whole source binding retires exactly once.
fn retire_transferred_call_sources(
    arguments: &[StructuralArgument],
    parameters: &[StructuralParameterDeclaration],
    values: &mut BTreeMap<PlaceId, TerminalStructuralValue>,
) -> Result<(), TerminalInterpretError> {
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
    for place in sources {
        if values.remove(&place).is_none() {
            return Err(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                place,
            ));
        }
    }
    Ok(())
}

impl TerminalExecution<'_> {
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

    /// Rebind whole existing byte arguments, without converting inline storage.
    pub(crate) fn bind_byte_sequence_arguments(
        &self,
        parameters: &[StructuralParameterDeclaration],
        arguments: &[StructuralArgument],
        resolved_arguments: &[TerminalStructuralValue],
    ) -> Result<BTreeMap<PlaceId, ByteSequenceBinding>, TerminalInterpretError> {
        if parameters.len() != arguments.len() || parameters.len() != resolved_arguments.len() {
            return Err(malformed());
        }
        let mut values = BTreeMap::new();
        for ((parameter, argument), resolved) in
            parameters.iter().zip(arguments).zip(resolved_arguments)
        {
            let declaration = self
                .program
                .structural_types()
                .get(&parameter.structural_type)
                .ok_or(malformed())?;
            let StructuralTypeShape::ByteSequence(carrier) = &declaration.shape else {
                continue;
            };
            if let Some((value, binding)) =
                self.prepare_owned_byte_descriptor_argument(parameter, argument, resolved)?
            {
                if value != *resolved || values.insert(parameter.place, binding).is_some() {
                    return Err(malformed());
                }
                continue;
            }
            if let Some((value, binding)) =
                self.prepare_shared_descriptor_argument(parameter, argument)?
            {
                if value != *resolved || values.insert(parameter.place, binding).is_some() {
                    return Err(malformed());
                }
                continue;
            }
            // A bounded byte owner passed by value is an independent copy of
            // the live bytes, the same detached snapshot a structural copy
            // publishes. The formal's domains must hold for those bytes.
            if let terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity } = carrier
                && parameter.access == StructuralAccess::Owned
                && argument.access == StructuralAccess::Owned
                && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                && parameter.structural_type == resolved.structural_type
                && parameter.projected_qualifications.is_empty()
                && argument.path.is_empty()
            {
                let bytes = crate::custody::verified_structural_place(
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
                            ByteSequenceBinding::Immutable(snapshot.detached_descriptor()),
                        )
                        .is_some()
                {
                    return Err(malformed());
                }
                continue;
            }
            if !carrier.is_borrowed()
                || !is_direct_borrowed_rebinding(
                    parameter,
                    argument,
                    resolved,
                    &self.program.structural_types(),
                )
                || !argument.path.is_empty()
            {
                return Err(malformed());
            }
            let bytes = crate::custody::verified_structural_place(
                &self.byte_sequence_values,
                argument.place,
            )?;
            let bytes = self.reborrow_byte_sequence_binding(bytes, resolved, parameter.access)?;
            if values.insert(parameter.place, bytes).is_some() {
                return Err(malformed());
            }
        }
        Ok(values)
    }

    /// Element-view arrivals retain the current descriptor and its pinned backing.
    /// Borrowed arrivals may attenuate referent access; owned arrivals transfer
    /// descriptor custody without changing the permission held by the view.
    pub(crate) fn bind_element_view_arguments(
        &self,
        parameters: &[StructuralParameterDeclaration],
        arguments: &[StructuralArgument],
        resolved_arguments: &[TerminalStructuralValue],
    ) -> Result<BTreeMap<PlaceId, crate::element_views::ElementView>, TerminalInterpretError> {
        if parameters.len() != arguments.len() || parameters.len() != resolved_arguments.len() {
            return Err(malformed());
        }
        let mut values = BTreeMap::new();
        for ((parameter, argument), resolved) in
            parameters.iter().zip(arguments).zip(resolved_arguments)
        {
            let declaration = self
                .program
                .structural_types()
                .get(&parameter.structural_type)
                .ok_or(malformed())?;
            if !matches!(declaration.shape, StructuralTypeShape::ElementView { .. }) {
                continue;
            }
            // Lookup uses the source's physical descriptor identity. The
            // callee may present its canonical borrowed type at that same
            // home; this must never choose another descriptor or window.
            let stored =
                self.resolve_live_projection(argument.place, &argument.path, argument.access)?;
            if stored.opaque_identity != resolved.opaque_identity
                || stored.path != resolved.path
                || !terminal_semantics::borrowed_referent_matches(
                    self.program.structural_types().values(),
                    stored.structural_type,
                    resolved.structural_type,
                )
            {
                return Err(malformed());
            }
            let view = self.element_view_argument(argument, &stored)?;
            let access = if parameter.access == StructuralAccess::Owned {
                // Argument preparation owns transfer authority and qualification
                // implication. This companion carries the selected descriptor,
                // including when its home is projected from another carrier.
                if argument.access != StructuralAccess::Owned
                    || parameter.structural_type != resolved.structural_type
                    || view.owned_multiplicity() != Some(parameter.multiplicity)
                {
                    return Err(malformed());
                }
                view.access()
            } else {
                if !is_direct_borrowed_rebinding(
                    parameter,
                    argument,
                    resolved,
                    &self.program.structural_types(),
                ) {
                    return Err(malformed());
                }
                parameter.access
            };
            let presented = view.subslice(0, view.len(), access).ok_or(malformed())?;
            let presented = presented.with_descriptor_home(resolved.clone());
            if values.insert(parameter.place, presented).is_some() {
                return Err(malformed());
            }
        }
        Ok(values)
    }

    /// Enter a callee whose result shape the caller already verified:
    /// the shared ownership and continuation transition for a prepared call.
    /// `result` is the suspended frame's completion binding.
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
        let values = bind_arguments(&callee.parameters, scalar_arguments)?;
        prepared_arguments.validate_carrier_partition(&callee.structural_parameters)?;
        let placed_view_occurrences = self.bind_call_placed_views(
            callee_id,
            structural_arguments,
            &prepared_arguments.values,
        )?;
        let mut structural_values = prepared_arguments.values;
        let mut identity_cursor = prepared_arguments.local_identity_cursor;
        if let SuspendedCallResult::Structural {
            expected_reference_backings,
            ..
        } = &mut result
        {
            *expected_reference_backings = self.bind_reference_return_backings(
                &callee.structural_parameters,
                callee.result.structural().ok_or_else(malformed)?,
                &structural_values,
                &prepared_arguments.aggregate_backings,
                &prepared_arguments.primitive_loans,
                &placed_view_occurrences,
            )?;
        }
        let aggregate_payload = self.stage_owned_value_arguments(
            &prepared_arguments.descriptor_parameters,
            &mut structural_values,
            &mut identity_cursor,
        )?;
        let byte_sequence_values = prepared_arguments.byte_sequences;
        let mut callee_affine_frontier = crate::custody::bind_affine_frontier(
            &callee.structural_parameters,
            &structural_values,
            &self.program.structural_types(),
        )?;
        let (remaining_claims, live_claims) = transfer_claims(
            &self.program.structural_types(),
            &self.structural_cases,
            &self.live_claims,
            structural_arguments,
            claim_transfers,
            &callee.structural_parameters,
            &callee.entry_claims,
            &callee.content_entry_claims,
            &structural_values,
        )?;
        crate::custody::exclude_linear_regions(
            &self.program.structural_types(),
            &mut callee_affine_frontier,
            live_claims
                .values()
                .filter_map(|claim| claim.place.map(|place| (place, claim.path.as_slice()))),
            &|current| {
                crate::structural_inputs::case_membership::stored_case_at_projection(
                    &self.program.structural_types(),
                    structural_values.get(&current.place)?,
                    &self.structural_cases,
                    &current.path,
                )
            },
        )?;
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
        let caller = self.program.verified_machine(self.current_machine)?;
        let operation = caller
            .block(&self.current)
            .and_then(|block| block.operations.get(self.next_operation))
            .ok_or_else(malformed)?
            .id;
        let loan_completions = self.loan_completions_for_operation(operation)?;
        self.next_operation += 1;
        self.live_claims = remaining_claims;
        let mut caller_structural_values = std::mem::take(&mut self.structural_values);
        retire_transferred_call_sources(
            structural_arguments,
            &callee.structural_parameters,
            &mut caller_structural_values,
        )?;
        self.call_stack.push(SuspendedCall {
            placed_view_occurrences: std::mem::take(&mut self.placed_view_occurrences),
            loan_completions,
            values: std::mem::take(&mut self.values),
            structural_values: caller_structural_values,
            aggregate_argument_backings: std::mem::take(&mut self.aggregate_argument_backings),
            byte_sequence_values: std::mem::take(&mut self.byte_sequence_values),
            primitive_loan_values: std::mem::take(&mut self.primitive_loan_values),
            scalar_array_values: std::mem::take(&mut self.scalar_array_values),
            element_view_values: std::mem::take(&mut self.element_view_values),
            live_affine_frontier: caller_affine_frontier,
            live_claims: std::mem::take(&mut self.live_claims),
            current_machine: self.current_machine,
            current: self.current,
            next_operation: self.next_operation,
            result,
        });
        self.placed_view_occurrences = placed_view_occurrences;
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

    /// Enter one structural Unit callee after the operation-specific argument
    /// checks have succeeded. Ordinary calls and admitted provider dispatch
    /// share this exact ownership and continuation transition.
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

    /// Enter one scalar-result callee after the operation-specific argument
    /// checks have succeeded. Ordinary calls prepare their arguments against
    /// the callee signature directly; admitted provider
    /// dispatch passes the boundary-prepared arguments converted by
    /// `BoundaryArguments::into_call_arguments`, the same custody conversion
    /// the Unit and structural provider results use.
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
        claim_origin: crate::execution::ResultClaimOrigin,
    ) -> Result<(), TerminalInterpretError> {
        let program = std::sync::Arc::clone(&self.program);
        let callee = program.verified_machine(callee_id)?;
        let Some(callee_result) = callee.result.structural() else {
            return Err(malformed());
        };
        if let crate::execution::ResultClaimOrigin::TargetFamilyReturn { head, member } =
            claim_origin
        {
            if member != callee_id || !program.is_target_family_head(head) {
                return Err(malformed());
            }
        }
        // Argument preparation owns projection, access, qualification and alias
        // checks. Returning a value does not impose a second argument-path or
        // result-shape language. Completion checks the produced carrier against
        // this exact declaration before installing it in the caller.
        if result.structural_type != callee_result.structural_type
            || result.multiplicity != callee_result.multiplicity
            || !claim_origin.result_qualifications_match(&result, &callee_result.qualifications)
            || !claim_origin.result_projected_qualifications_match(
                &result,
                &callee_result.projected_qualifications,
            )
            || self.structural_values.contains_key(&result.place)
            || self.scalar_array_values.contains_key(&result.place)
        {
            return Err(malformed());
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
            },
        )
    }
}
