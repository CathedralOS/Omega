//! Structural result custody is prepared before the return edge is paid. The
//! common completion retains it across every nominal hook and installs it once.
use super::*;
use crate::byte_sequences::binding::ByteSequenceBinding;
use crate::errors::malformed;
use crate::scalar_array::TerminalScalarArrayResult;

impl TerminalExecution<'_> {
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
            .machines()
            .get(&self.current_machine)
            .and_then(|machine| machine.result.structural())
            .cloned()
            .ok_or(malformed())?;
        // A return roster describes typed custody, including guarded payloads.
        // Only the stored discriminator activates a Case path; a catalogue row
        // neither creates a claim nor establishes a tag.
        let active_returned_claims = self.active_structural_claims(*source, returned_claims)?;
        if cleanup_actions
            .iter()
            .any(|action| action.place() == *source)
        {
            return Err(malformed());
        }
        let destination = match self.call_stack.last() {
            Some(SuspendedCall {
                structural_values,
                scalar_array_values,
                element_view_values,
                byte_sequence_values,
                live_affine_frontier,
                result:
                    SuspendedCallResult::Structural {
                        result,
                        claim_origin,
                        ..
                    },
                ..
            }) if result.structural_type == signature.structural_type
                && result.multiplicity == signature.multiplicity
                && claim_origin.result_qualifications_match(result, &signature.qualifications)
                && claim_origin.result_projected_qualifications_match(
                    result,
                    &signature.projected_qualifications,
                )
                && !structural_values.contains_key(&result.place)
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
        let mut identity_cursor = self.local_structural_identities.cursor();
        // Return the actual carrier. A placed array may retain nested
        // memberships that a packed primitive payload cannot represent.
        let mut completed = if self.scalar_array_values.contains_key(source) {
            let mut value = self.scalar_array_snapshot(
                &StructuralArgument {
                    place: *source,
                    path: Vec::new(),
                    access: StructuralAccess::Owned,
                },
                signature.structural_type,
            )?;
            if value.structural_type != signature.structural_type
                || signature.multiplicity != StructuralMultiplicity::Unrestricted
                || !signature.projected_qualifications.is_empty()
                || !returned_claims.is_empty()
                || !self.live_claims.is_empty()
                || destination.as_ref().is_some_and(|result| {
                    !result.projected_qualifications.is_empty() || !result.claims.is_empty()
                })
            {
                return Err(malformed());
            }
            self.validate_scalar_array_qualifications(&value, &signature.qualifications)?;
            value.qualifications.clone_from(&signature.qualifications);
            if let Some(destination) = &destination {
                self.validate_scalar_array_qualifications(&value, &destination.qualifications)?;
                value.qualifications.clone_from(&destination.qualifications);
            }
            CompletedReturn::plain(TerminalExecutionResult::ScalarArray(
                TerminalScalarArrayResult { value },
            ))
        } else {
            let mut value =
                crate::custody::verified_structural_place(&self.structural_values, *source)
                    .cloned()?;
            let byte_sequence = self.byte_sequence_values.get(source).cloned();
            let element_view = self.element_view_values.get(source).cloned();
            if matches!(&byte_sequence, Some(ByteSequenceBinding::Immutable(view)) if !view.matches_descriptor_home(&value))
                || element_view
                    .as_ref()
                    .is_some_and(|view| !view.matches_descriptor_home(&value))
            {
                return Err(malformed());
            }
            value.absorb_qualifications(&signature.qualifications);
            // A boundary issuance also establishes its route's domains.
            if let Some(destination) = &destination {
                value.absorb_qualifications(&destination.qualifications);
            }
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
                .filter(|(_, live)| live.place == Some(*source))
                .map(|(claim, live)| terminal_psi::StructuralResultClaimBinding {
                    claim: *claim,
                    path: live.path.clone(),
                })
                .collect::<Vec<_>>();
            if actual_claims != active_returned_claims
                || self.live_claims.keys().any(|claim| {
                    !active_returned_claims
                        .iter()
                        .any(|binding| binding.claim == *claim)
                })
            {
                return Err(malformed());
            }
            // A borrowed descriptor parameter has no owned debt in this
            // activation. Returning its mutable view establishes a result
            // descriptor while retaining the already admitted backing loan.
            let borrowed_descriptor = signature.multiplicity == StructuralMultiplicity::Affine
                && signature.qualifications.is_empty()
                && terminal_semantics::view_descriptor_projected_qualifications(
                    self.program.structural_types().values(),
                    signature.structural_type,
                    &signature.projected_qualifications,
                )
                && signature
                    .projected_qualifications
                    .iter()
                    .all(|row| value.projected_qualifications.contains(row))
                && active_returned_claims.is_empty()
                && value.qualifications.is_empty()
                && terminal_semantics::view_descriptor_projected_qualifications(
                    self.program.structural_types().values(),
                    value.structural_type,
                    &value.projected_qualifications,
                )
                && self
                    .program
                    .machines()
                    .get(&self.current_machine)
                    .and_then(|machine| terminal_semantics::arrival_parameter(machine, *source))
                    .is_some_and(|parameter| {
                        parameter.access == StructuralAccess::MutableBorrow
                            && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                            && terminal_semantics::view_descriptor_parameter(
                                parameter,
                                self.program.structural_types().values(),
                            )
                            // Stored descriptors and canonical borrowed ingress
                            // retain one pointer/count home. Geometry never pays
                            // owned debt or changes the independently checked access.
                            && terminal_semantics::borrowed_referent_presentation(
                                self.program.structural_types().values(),
                                parameter.structural_type,
                                signature.structural_type,
                            ) == Some(terminal_semantics::BorrowedReferentPresentation::Unchanged)
                            && parameter.qualifications.is_empty()
                            && terminal_semantics::view_descriptor_projected_qualifications(
                                self.program.structural_types().values(),
                                parameter.structural_type,
                                &parameter.projected_qualifications,
                            )
                    })
                && (element_view
                    .as_ref()
                    .is_some_and(|view| view.access() == StructuralAccess::MutableBorrow)
                    || (self
                        .program
                        .structural_types()
                        .get(&signature.structural_type)
                        .is_some_and(|declaration| declaration.shape.is_borrowed_byte_view())
                        && matches!(byte_sequence, Some(ref binding) if !matches!(binding, ByteSequenceBinding::Immutable(_))
                            && binding.validate_mutable_referent(self, &value).is_ok())));
            // Owned results leave with their exact affine complement. Borrowed
            // ingress contributes no source frontier to transfer.
            let carried = crate::custody::whole_value_frontier(
                &self.program.structural_types(),
                *source,
                signature.structural_type,
                if borrowed_descriptor {
                    StructuralMultiplicity::Unrestricted
                } else {
                    signature.multiplicity
                },
                self.live_claim_paths_at(*source),
                &|current| self.stored_case_at_discard(&self.structural_values, current),
            )?;
            if self
                .live_affine_frontier
                .iter()
                .filter(|entry| entry.place == *source)
                .ne(carried.iter())
            {
                return Err(TerminalInterpretError::AffineFrontierMismatch);
            }
            let rebound_claims = match self.call_stack.last() {
                Some(SuspendedCall {
                    live_claims,
                    result:
                        SuspendedCallResult::Structural {
                            result,
                            claim_origin,
                            ..
                        },
                    ..
                }) => Some(match claim_origin {
                    crate::execution::ResultClaimOrigin::CalleeReturn
                    | crate::execution::ResultClaimOrigin::CheckedProvider
                    | crate::execution::ResultClaimOrigin::TargetFamilyReturn { .. } => {
                        let mut active_result = result.clone();
                        active_result.claims =
                            self.active_structural_claims(*source, &result.claims)?;
                        rebind_structural_result_claims(
                            live_claims,
                            &self.live_claims,
                            *source,
                            &active_result,
                            &active_returned_claims,
                        )?
                    }
                }),
                Some(_) => return Err(malformed()),
                None => None,
            };
            // The member's returning edge authenticated these domains and
            // claims. Weakening reveals only the head contract; it changes no
            // placed identity, reference backing, ownership path or debt.
            if let Some(SuspendedCall {
                result:
                    SuspendedCallResult::Structural {
                        result,
                        claim_origin:
                            crate::execution::ResultClaimOrigin::TargetFamilyReturn { head, member },
                        ..
                    },
                ..
            }) = self.call_stack.last()
            {
                if *member != self.current_machine || !self.program.is_target_family_head(*head) {
                    return Err(malformed());
                }
                if result
                    .projected_qualifications
                    .iter()
                    .any(|row| !value.projected_qualifications.contains(row))
                {
                    return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                        "family result projected qualification lacks transported member evidence",
                    ));
                }
                value.qualifications.clone_from(&result.qualifications);
                value
                    .projected_qualifications
                    .clone_from(&result.projected_qualifications);
            }
            // A checked provider can retain stronger projected memberships.
            // Its own returning edge already authenticated them. Reveal only
            // the public boundary roster, without adding any membership or
            // changing the returned claims, backing, or carried ownership.
            if let Some(SuspendedCall {
                result:
                    SuspendedCallResult::Structural {
                        result,
                        claim_origin: crate::execution::ResultClaimOrigin::CheckedProvider,
                        ..
                    },
                ..
            }) = self.call_stack.last()
            {
                let mut active = Vec::new();
                for row in &result.projected_qualifications {
                    if crate::structural_inputs::case_membership::case_projection_in_value_is_active(
                        &self.program.structural_types(), &value, &self.structural_cases, &row.path,
                    )? {
                        if !value.projected_qualifications.contains(row) {
                            return Err(malformed());
                        }
                        active.push(row.clone());
                    }
                }
                value.projected_qualifications = active;
            }
            let byte_sequence = byte_sequence.map(|binding| match binding {
                ByteSequenceBinding::Immutable(view) => {
                    ByteSequenceBinding::Immutable(view.with_descriptor_home(value.clone()))
                }
                binding => binding,
            });
            if borrowed_descriptor && element_view.is_some() {
                value.opaque_identity = self
                    .local_structural_identities
                    .allocate_staged(&mut identity_cursor)?;
                value.path.clear();
            }
            let element_view = element_view.map(|view| view.with_descriptor_home(value.clone()));
            let scalar_case = if self.call_stack.is_empty()
                && signature.qualifications.is_empty()
                && signature.projected_qualifications.is_empty()
                && returned_claims.is_empty()
            {
                self.snapshot_scalar_case(&value)?
            } else {
                None
            };
            CompletedReturn {
                value: if let Some(value) = scalar_case {
                    TerminalExecutionResult::ScalarCase(TerminalScalarCaseResult { value })
                } else {
                    TerminalExecutionResult::Structural(TerminalStructuralResult {
                        value,
                        claims: active_returned_claims
                            .iter()
                            .map(|binding| binding.claim)
                            .collect(),
                    })
                },
                destination: None,
                rebound_claims,
                byte_sequence,
                element_view,
            }
        };
        completed.destination = destination;
        let cleanup_actions = block_bindings::BlockBindings::default().prepare_cleanup_actions(
            self,
            &[],
            cleanup_actions,
        )?;
        // No payload, descriptor, claim, caller frame, or cleanup root moves
        // while the sponsor cannot pay this edge.
        if let Err(error) = meter.charge_terminator(terminator) {
            return meter_status(error).map(TerminatorFlow::Yield);
        }
        self.local_structural_identities
            .commit_cursor(identity_cursor);
        self.structural_values.remove(source);
        self.scalar_array_values.remove(source);
        self.byte_sequence_values.remove(source);
        self.element_view_values.remove(source);
        self.live_affine_frontier
            .retain(|entry| entry.place != *source);
        for binding in &active_returned_claims {
            self.live_claims.remove(&binding.claim);
        }
        self.continue_cleanup_actions(
            cleanup_actions.into_iter(),
            CleanupContinuation::Return(completed),
        )
    }

    /// The embedding API accepts semantic scalar-case contents, not a second
    /// live runtime carrier. Ordinary caller returns preserve the placed home.
    fn snapshot_scalar_case(
        &self,
        value: &crate::values::TerminalStructuralValue,
    ) -> Result<Option<crate::values::TerminalScalarCaseValue>, TerminalInterpretError> {
        if !value.qualifications.is_empty() || !value.projected_qualifications.is_empty() {
            return Ok(None);
        }
        let declaration = self
            .program
            .structural_types()
            .get(&value.structural_type)
            .ok_or_else(malformed)?;
        let terminal_psi::StructuralTypeShape::Sum { cases } = &declaration.shape else {
            return Ok(None);
        };
        let root = crate::values::StructuralRuntimePlace::from(value);
        let Some(contents) = self.structural_cases.get(&root) else {
            return Ok(None);
        };
        if contents.structural_type != value.structural_type {
            return Err(malformed());
        }
        let selected =
            terminal_psi::find_structural_case(cases, contents.case).ok_or_else(malformed)?;
        if selected
            .fields
            .iter()
            .any(|field| field.relevance.is_erased() || field.field_type.scalar_type().is_none())
        {
            return Ok(None);
        }
        let fields = selected
            .fields
            .iter()
            .map(|field| {
                let cell = crate::values::StructuralScalarRuntimeField::case_payload(
                    root.clone(),
                    field.id,
                );
                self.structural_scalar_fields
                    .get(&cell)
                    .copied()
                    .map(|scalar| (field.id, scalar))
                    .ok_or_else(malformed)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let snapshot = crate::values::TerminalScalarCaseValue {
            structural_type: value.structural_type,
            result_case: contents.case,
            fields,
        };
        snapshot.validate_against(declaration)?;
        Ok(Some(snapshot))
    }
}
