//! Structural result custody is prepared before the return edge is paid. The
//! common completion retains it across every nominal hook and installs it once.
use super::*;
use crate::byte_sequences::binding::ByteSequenceBinding;
use crate::errors::malformed;
use crate::scalar_array::TerminalScalarArrayResult;

impl TerminalExecution {
    pub(crate) fn settle_return_structural(
        &mut self,
        terminator: &Terminator,
        meter: &mut TerminalFuelMeter,
    ) -> Result<TerminatorFlow, TerminalInterpretError> {
        let Terminator::ReturnStructural {
            source,
            returned_claims,
            cleanup_actions,
            ..
        } = terminator
        else {
            unreachable!("dispatched settle_return_structural")
        };
        let signature = self
            .program
            .machines
            .get(&self.current_machine)
            .and_then(|machine| machine.result.structural())
            .cloned()
            .ok_or(malformed())?;
        if cleanup_actions
            .iter()
            .any(|action| action.place() == *source)
        {
            return Err(malformed());
        }
        let destination = match self.call_stack.last() {
            Some(SuspendedCall {
                structural_values,
                scalar_case_values,
                scalar_array_values,
                element_view_values,
                byte_sequence_values,
                live_affine_frontier,
                result: SuspendedCallResult::Structural { result, .. },
                ..
            }) if result.structural_type == signature.structural_type
                && result.multiplicity == signature.multiplicity
                && result.qualifications == signature.qualifications
                && !structural_values.contains_key(&result.place)
                && !scalar_case_values.contains_key(&result.place)
                && !scalar_array_values.contains_key(&result.place)
                && !element_view_values.contains_key(&result.place)
                && !byte_sequence_values.contains_key(&result.place)
                && live_affine_frontier
                    .iter()
                    .all(|entry| entry.place != result.place) =>
            {
                Some(result.clone())
            }
            Some(_) => return Err(malformed()),
            None => None,
        };
        let mut completed = if terminal_semantics::scalar_array_leaf_shape(
            self.program.structural_types.values(),
            signature.structural_type,
        )
        .is_some()
        {
            let value = self
                .scalar_array_values
                .get(source)
                .cloned()
                .ok_or(malformed())?;
            if value.structural_type != signature.structural_type
                || signature.multiplicity != StructuralMultiplicity::Unrestricted
                || !signature.qualifications.is_empty()
                || !signature.projected_qualifications.is_empty()
                || !returned_claims.is_empty()
                || !self.live_claims.is_empty()
                || !self.claim_free_structural_destination()
            {
                return Err(malformed());
            }
            CompletedReturn::plain(TerminalExecutionResult::ScalarArray(
                TerminalScalarArrayResult { value },
            ))
        } else if let Some(value) = self.scalar_case_values.get(source).cloned() {
            if value.structural_type != signature.structural_type
                || !signature.qualifications.is_empty()
                || !returned_claims.is_empty()
                || !self.live_claims.is_empty()
                || !self.claim_free_structural_destination()
            {
                return Err(malformed());
            }
            CompletedReturn::plain(TerminalExecutionResult::ScalarCase(
                TerminalScalarCaseResult { value },
            ))
        } else {
            let mut value = self.structural_values.get(source).cloned().ok_or(
                TerminalInterpretError::VerifiedStructuralPlaceMissing(*source),
            )?;
            let byte_sequence = self.byte_sequence_values.get(source).cloned();
            let element_view = self.element_view_values.get(source).cloned();
            if matches!(&byte_sequence, Some(ByteSequenceBinding::Immutable(view)) if !view.matches_descriptor_home(&value))
                || element_view
                    .as_ref()
                    .is_some_and(|view| !view.matches_descriptor_home(&value))
            {
                return Err(malformed());
            }
            for domain in &signature.qualifications {
                if !value.qualifications.contains(domain) {
                    value.qualifications.push(*domain);
                }
            }
            value.qualifications.sort();
            self.validate_reference_return(&signature, &value)?;
            if value.structural_type != signature.structural_type
                || signature
                    .qualifications
                    .iter()
                    .any(|domain| !value.qualifications.contains(domain))
            {
                return Err(malformed());
            }
            let actual_claims = self
                .live_claims
                .iter()
                .filter_map(|(claim, live)| (live.place == Some(*source)).then_some(*claim))
                .collect::<Vec<_>>();
            if actual_claims != *returned_claims
                || self
                    .live_claims
                    .keys()
                    .any(|claim| !returned_claims.contains(claim))
            {
                return Err(malformed());
            }
            let rebound_claims = match self.call_stack.last() {
                Some(SuspendedCall {
                    live_claims,
                    result:
                        SuspendedCallResult::Structural {
                            result,
                            returned_claim_transfers,
                            ..
                        },
                    ..
                }) => Some(
                    if !result.claims.is_empty() && returned_claim_transfers.is_empty() {
                        self.mint_boundary_result_claims(live_claims, *source, result)?
                    } else {
                        rebind_structural_result_claims(
                            live_claims,
                            &self.live_claims,
                            *source,
                            result,
                            returned_claim_transfers,
                            returned_claims,
                        )?
                    },
                ),
                Some(_) => return Err(malformed()),
                None => None,
            };
            let byte_sequence = byte_sequence.map(|binding| match binding {
                ByteSequenceBinding::Immutable(view) => {
                    ByteSequenceBinding::Immutable(view.with_descriptor_home(value.clone()))
                }
                binding => binding,
            });
            let element_view = element_view.map(|view| view.with_descriptor_home(value.clone()));
            CompletedReturn {
                value: TerminalExecutionResult::Structural(TerminalStructuralResult {
                    value,
                    claims: returned_claims.clone(),
                }),
                destination: None,
                rebound_claims,
                byte_sequence,
                element_view,
            }
        };
        completed.destination = destination;
        // No payload, descriptor, claim, caller frame, or cleanup root moves
        // while the sponsor cannot pay this edge.
        if let Err(error) = meter.charge_terminator(terminator) {
            return meter_status(error).map(TerminatorFlow::Yield);
        }
        self.structural_values.remove(source);
        self.scalar_case_values.remove(source);
        self.scalar_array_values.remove(source);
        self.byte_sequence_values.remove(source);
        self.element_view_values.remove(source);
        remove_affine_root(&mut self.live_affine_frontier, *source);
        for claim in returned_claims {
            self.live_claims.remove(claim);
        }
        self.retire_return_inputs();
        self.continue_cleanup_actions(
            cleanup_actions.clone().into_iter(),
            CleanupContinuation::Return(completed),
        )
    }

    fn claim_free_structural_destination(&self) -> bool {
        match self.call_stack.last() {
            Some(SuspendedCall {
                result:
                    SuspendedCallResult::Structural {
                        result,
                        returned_claim_transfers,
                        ..
                    },
                ..
            }) => {
                result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && result.claims.is_empty()
                    && returned_claim_transfers.is_empty()
            }
            Some(_) => false,
            None => true,
        }
    }
}
