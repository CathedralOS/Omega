//! Preserve producer-authored custody when publishing a completed source value.
//! Checked operand memberships describe the denoted value: for a reference
//! descriptor they qualify its referent, never its pointer geometry. Copies
//! and owned transport share that carrier/path join; neither establishes a new
//! loan or changes the selected backing.

use super::{
    Emission, LoweringError, OperationBuffer, PlaceId, StructuralAccess, StructuralArgument,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    StructuralPlaceKind, unsupported,
};

impl Emission<'_, '_, '_> {
    /// A private value edge changes storage identity, not membership.
    /// Carry the actual source declaration into its new parameter instead of
    /// erasing predicates or treating a receiver annotation as evidence.
    pub(super) fn transported_parameter(
        &self,
        source: &StructuralArgument,
        place: PlaceId,
        position: u32,
        structural_type: semantic_vocabulary::StructuralTypeId,
        multiplicity: StructuralMultiplicity,
    ) -> Result<StructuralParameterDeclaration, LoweringError> {
        let (qualifications, projected_qualifications) = self
            .completed_memberships(source.place)
            .ok_or(LoweringError::Unsupported(
            "owned transport has no source membership declaration",
        ))?;
        let (mut qualifications, mut projected_qualifications) =
            terminal_psi::StructuralPathQualification::select(
                qualifications,
                projected_qualifications,
                &source.path,
            );
        terminal_semantics::present_structural_memberships(
            self.structural_types.iter(),
            self.domain_declarations.iter(),
            structural_type,
            &mut qualifications,
            &mut projected_qualifications,
        )
        .map_err(LoweringError::Unsupported)?;
        Ok(StructuralParameterDeclaration {
            place,
            position,
            structural_type,
            multiplicity,
            access: StructuralAccess::Owned,
            qualifications,
            projected_qualifications,
        })
    }

    /// A receiving annotation constrains the source; it does not erase the
    /// initializer's checked postconditions or conserved Content. Preserve the
    /// actual roster across the owned edge, and require every declared member
    /// to be present already rather than treating the annotation as evidence.
    pub(super) fn value_binding_parameter(
        &self,
        _identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        source: &StructuralArgument,
        place: PlaceId,
        position: u32,
    ) -> Result<StructuralParameterDeclaration, LoweringError> {
        let parameter = self.transported_parameter(
            source,
            place,
            position,
            self.structural_type,
            self.multiplicity,
        )?;
        if !self.binding_type_reference.is_valid() {
            return Ok(parameter);
        }
        let mut whole = validation::view_result_carrier_qualifications(
            &self.checked.typed,
            self.binding_type_reference,
        )
        .map_err(LoweringError::Unsupported)?
        .into_iter()
        .map(|domain| {
            crate::terminal_identities::lookup_domain_id_for_carrier(
                self.domain_ids,
                domain,
                self.structural_type,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
        whole.sort();
        whole.dedup();
        // Nested/case memberships belong to the actual completed payload.
        // The nominal target can mention inactive alternatives, which are not
        // new observations established by this owned edge.
        if whole
            .iter()
            .any(|domain| !parameter.qualifications.contains(domain))
        {
            return Err(LoweringError::at_statement(
                self.checked,
                self.machine,
                self.state,
                self.statement,
                "owned binding cannot establish a receiving membership",
            ));
        }
        Ok(parameter)
    }

    /// Producer and ingress homes retain their actual occurrence-relative
    /// memberships. A receiving annotation is never a source contract.
    pub(super) fn completed_memberships(
        &self,
        place: PlaceId,
    ) -> Option<(
        &[semantic_vocabulary::StructuralDomainId],
        &[terminal_psi::StructuralPathQualification],
    )> {
        self.operations
            .structural_result_contract(place)
            .map(|result| {
                (
                    result.qualifications.as_slice(),
                    result.projected_qualifications.as_slice(),
                )
            })
            .or_else(|| {
                // A literal view's establishment names its place and the
                // memberships it was established with.
                self.operations
                    .iter()
                    .find_map(|operation| match &operation.kind {
                        terminal_psi::OperationKind::EstablishByteSequenceLiteral {
                            destination,
                            qualifications,
                            ..
                        } if *destination == place => Some((qualifications.as_slice(), &[][..])),
                        _ => None,
                    })
            })
            .or_else(|| {
                self.evaluation
                    .structural_root_parameters()
                    .find(|parameter| parameter.place == place)
                    .map(|parameter| {
                        (
                            parameter.qualifications.as_slice(),
                            parameter.projected_qualifications.as_slice(),
                        )
                    })
            })
    }

    /// The exact subject of source-value qualification on the emitted carrier.
    pub(super) fn checked_membership_subject(
        &self,
    ) -> Result<
        (
            semantic_vocabulary::StructuralTypeId,
            &'static [terminal_psi::StructuralPathSegment],
        ),
        LoweringError,
    > {
        let declaration =
            terminal_psi::find_structural_type(self.structural_types, self.structural_type).ok_or(
                LoweringError::Unsupported("checked membership carrier is absent"),
            )?;
        Ok(match declaration.shape {
            terminal_psi::StructuralTypeShape::Reference { referent, .. } => {
                (referent, &[terminal_psi::StructuralPathSegment::Referent])
            }
            _ => (self.structural_type, &[]),
        })
    }

    pub(super) fn checked_membership_domain(
        &self,
        decision: &checked_trees::CheckedValueQualification,
        carrier: semantic_vocabulary::StructuralTypeId,
    ) -> Result<semantic_vocabulary::StructuralDomainId, LoweringError> {
        crate::terminal_identities::lookup_domain_id_for_carrier(
            self.domain_ids,
            decision.semantic_domain,
            carrier,
        )
        .map_err(|cause| LoweringError::InvalidOccurrenceMembership {
            machine: self.checked.symbols.display_path(self.machine, "::"),
            state: self.checked.symbols.display_path(self.state, "::"),
            statement_index: self.statement,
            domain: self
                .checked
                .typed
                .semantic_domains
                .name(decision.semantic_domain)
                .unwrap_or("<unregistered domain>")
                .to_owned(),
            carrier,
            cause: Box::new(cause),
        })
    }

    /// The Terminal domains of checked memberships on their exact subject.
    pub(super) fn checked_membership_domains(
        &self,
        decisions: arena::HandleSpan<checked_trees::CheckedValueQualification>,
        carrier: semantic_vocabulary::StructuralTypeId,
    ) -> Result<Vec<semantic_vocabulary::StructuralDomainId>, LoweringError> {
        let mut domains = self
            .checked
            .facts
            .qualifications
            .value_formations
            .span(decisions)
            .ok_or(LoweringError::Unsupported(
                "operand membership decisions are stale",
            ))?
            .iter()
            .map(|decision| self.checked_membership_domain(decision, carrier))
            .collect::<Result<Vec<_>, _>>()?;
        domains.sort();
        domains.dedup();
        Ok(domains)
    }

    /// A fresh construction publishes its occurrence-bound source judgments
    /// before joining a continuation. This never relabels a parameter, source
    /// home or call ABI result, and grants no additional claim or disposal right.
    /// Both explicit fields and omitted-field zero provisioning use this join.
    pub(super) fn complete_construction(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        place: PlaceId,
        decisions: arena::HandleSpan<checked_trees::CheckedValueQualification>,
        completion: super::CompletionOccurrence,
        continuation: Option<&mut super::ValueContinuation>,
    ) -> Result<PlaceId, LoweringError> {
        // Transparent containers transport their children's completed names.
        // Array multiplicity inherits its element's usage rule, but its exact
        // claim frontier still has indexed children, not a fresh whole root.
        // Only a whole claim in the checked type frontier introduces a root
        // at this construction, even beneath a selected expression.
        let node = self.structural_value_node(completion.value);
        // A zero image provisions storage without establishing the declared
        // linear value. Its operation is deliberately affine and carries no
        // claim; the later checked assignment introduces that obligation.
        if !matches!(
            &node.kind,
            checked_trees::CheckedStructuralValueKind::ZeroedStructural { .. }
        ) && validation::linear_claim_frontier(&self.checked.typed, node.type_reference)
            .iter()
            .any(|claim| claim.path.is_empty())
        {
            let mut prepared = crate::emission::claim_bindings::prepare_establishment_claims(
                identities,
                self.checked,
                self.machine,
                self.state,
                self.statement as usize,
                facts::PlaceRoot::Expression(node.expression),
                node.type_reference,
                self.structural_type,
                self.structural_types,
            )?;
            prepared.retain(|claim| claim.path.is_empty());
            if prepared.len() != 1 {
                return Err(LoweringError::MissingConstructionClaim {
                    machine: self.machine,
                    state: self.state,
                    statement_index: self.statement,
                    expression: node.expression,
                    structural_type: self.structural_type,
                    root_candidates: prepared.len(),
                });
            }
            let claims = crate::emission::claim_bindings::emit_establishment_claims(
                &prepared,
                &mut self.calls.claims,
            )?;
            let introductions = crate::emission::claim_bindings::direct_establishment_claims(
                &prepared,
                &self.calls.claims,
            )?;
            Self::bind_completion_claims(self.operations, place, &claims, &introductions)?;
        }
        let domains = self.checked_membership_domains(decisions, self.structural_type)?;
        crate::emission::formation_memberships::publish(
            self.operations,
            place,
            self.structural_type,
            domains,
        )?;
        self.completed(identities, place, completion, continuation)
    }

    /// Publish checked operand facts without relabeling its producer's ABI
    /// contract or an existing home. An unrestricted capture retains additional
    /// facts through the ordinary value-copy operation. An owned affine or
    /// linear value instead transfers its original custody to a fresh occurrence.
    /// Already qualified values need neither operation. This is not an issuer.
    pub(super) fn retain_checked_memberships(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        source: StructuralArgument,
        decisions: arena::HandleSpan<checked_trees::CheckedValueQualification>,
        expression: typed_trees::expression::ExpressionHandle,
        first_new_place: u64,
    ) -> Result<StructuralArgument, LoweringError> {
        let source = self.refresh_memberships(identities, source, expression, first_new_place)?;
        let memberships = self
            .checked
            .facts
            .qualifications
            .value_formations
            .span(decisions)
            .ok_or(LoweringError::Unsupported(
                "operand membership decisions are stale",
            ))?;
        if memberships.is_empty() {
            return Ok(source);
        }
        // Source memberships qualify a reference's referent, not its affine
        // permission descriptor. Rejoin the already captured rows at that
        // boundary; a decision cannot mint a domain on the pointer carrier.
        let (membership_carrier, relative) = self.checked_membership_subject()?;
        let mut membership_path = source.path.clone();
        membership_path.extend_from_slice(relative);
        let (qualifications, projected_qualifications) = self
            .completed_memberships(source.place)
            .ok_or(LoweringError::Unsupported(
            "completed operand membership source is missing",
        ))?;
        let (qualifications, _) = terminal_psi::StructuralPathQualification::select(
            qualifications,
            projected_qualifications,
            &membership_path,
        );
        let mut established = Vec::new();
        for decision in memberships {
            let domain = self.checked_membership_domain(decision, membership_carrier)?;
            if !qualifications.contains(&domain) {
                // Direct Terminal routes do not include every inherited or
                // aliased restriction. Consult the resolved source declaration,
                // never its spelling or an empty route roster in the catalog.
                if validation::field_domains::domain_requires_provenance(
                    &self.checked.typed,
                    decision.domain_symbol,
                ) {
                    return unsupported(
                        "owned membership transport cannot establish routed authority",
                    );
                }
                established.push(domain);
            }
        }
        if established.is_empty() {
            return Ok(source);
        }
        if self.multiplicity != StructuralMultiplicity::Unrestricted {
            if source.access != StructuralAccess::Owned {
                return unsupported("membership transfer requires owned value custody");
            }
            let completed =
                self.capture_operand(identities, source, expression, first_new_place)?;
            let source = &completed.argument;
            let (whole, projected) =
                self.completed_memberships(source.place)
                    .ok_or(LoweringError::Unsupported(
                        "owned membership source disappeared",
                    ))?;
            let (mut qualifications, mut projected_qualifications) =
                terminal_psi::StructuralPathQualification::select(whole, projected, &source.path);
            if relative.is_empty() {
                qualifications.extend(established);
                qualifications.sort();
                qualifications.dedup();
            } else {
                terminal_psi::StructuralPathQualification::append_under(
                    &established,
                    &[],
                    relative,
                    &mut projected_qualifications,
                );
                projected_qualifications.sort();
                projected_qualifications.dedup();
            }
            return self.move_completed_operand(
                completed,
                qualifications,
                projected_qualifications,
            );
        }
        self.copied_leaf_place(identities, &source, decisions, expression)
            .map(|place| StructuralArgument {
                place,
                path: Vec::new(),
                access: StructuralAccess::Owned,
            })
    }
    /// Separate an owned subtree from its original home before that home's
    /// checked complement dies. Membership transport and projections use the
    /// same move; neither a new annotation nor cleanup can mint source claims.
    pub(super) fn move_completed_operand(
        &mut self,
        completed: super::super::operand_claims::CompletedOperand,
        qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
        projected_qualifications: Vec<terminal_psi::StructuralPathQualification>,
    ) -> Result<StructuralArgument, LoweringError> {
        let source = completed.argument;
        if source.access != StructuralAccess::Owned {
            return unsupported("owned subtree move requires exact source custody");
        }
        let source_place = source.place;
        let source_path = source.path.clone();
        let transported_claims = completed.claims;
        let place = super::place_id(super::allocate_dense(self.environment.next_place)?);
        let operation = self.operations.allocate();
        self.operations.push(terminal_psi::Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: operation,
            result: terminal_psi::OperationResult::Structural(
                terminal_psi::StructuralOperationResult {
                    place,
                    structural_type: self.structural_type,
                    multiplicity: self.multiplicity,
                    qualifications,
                    projected_qualifications,
                    qualification_establishments: Vec::new(),
                    claims: transported_claims.clone(),
                },
            ),
            kind: terminal_psi::OperationKind::MoveStructuralValue { source },
        });
        self.evaluation
            .retire_structural_operation_operands(operation, self.operations)?;
        self.environment
            .private_places
            .push(super::operation_result_declaration(
                place,
                operation,
                self.structural_type,
            ));
        self.operations
            .register_structural_materialization(place, self.state, self.statement)?;
        // This is the exact owned transfer just emitted, not a new
        // establishment. Retain its source-relative names at the fresh
        // result and remove only the moved source subtree's associations.
        // An absent historical home stays absent rather than inventing a
        // whole-home namespace from a projected consuming receipt.
        if self.evaluation.current_claims.contains(source_place) {
            self.evaluation
                .current_claims
                .replace(source_place, &source_path, &[])?;
        }
        self.evaluation
            .current_claims
            .publish(place, transported_claims)?;
        Ok(StructuralArgument {
            place,
            path: Vec::new(),
            access: StructuralAccess::Owned,
        })
    }
}

/// Registration retains the completed storage contract, rather than creating
/// an unqualified approximation of the value. Source replay and each emitted
/// operation own establishment; a continuation owns its arrival declaration.
pub(super) fn completed_result<'a>(
    place: PlaceId,
    declaration: &StructuralPlaceDeclaration,
    operations: &OperationBuffer,
    mut parameters: impl Iterator<Item = &'a StructuralParameterDeclaration> + Clone,
    arrival_claims: &[terminal_psi::StructuralResultClaimBinding],
) -> Result<terminal_psi::StructuralOperationResult, LoweringError> {
    let mut producers = operations.iter().filter_map(|operation| {
        operation
            .result
            .structural()
            .filter(|result| result.place == place)
            .map(|result| (operation.id, result))
    });
    if let Some((producer, result)) = producers.next() {
        if producers.next().is_some()
            || parameters.clone().any(|parameter| parameter.place == place)
            || declaration.kind
                != (StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type: result.structural_type,
                })
        {
            return unsupported("completed value does not have one exact producer");
        }
        return Ok(result.clone());
    }
    let StructuralPlaceDeclaration {
        kind: StructuralPlaceKind::BlockParameter { .. },
        ..
    } = declaration
    else {
        return unsupported("completed value has no producer or arrival declaration");
    };
    let parameter = parameters
        .find(|parameter| parameter.place == place)
        .ok_or(LoweringError::Unsupported(
            "completed value has no current arrival",
        ))?;
    if arrival_claims.windows(2).any(|pair| pair[0] >= pair[1])
        || (parameter.multiplicity == terminal_psi::StructuralMultiplicity::Linear
            && arrival_claims.is_empty())
    {
        return unsupported("completed linear arrival lost its exact claim frontier");
    }
    Ok(terminal_psi::StructuralOperationResult {
        place,
        structural_type: parameter.structural_type,
        multiplicity: parameter.multiplicity,
        qualifications: parameter.qualifications.clone(),
        projected_qualifications: parameter.projected_qualifications.clone(),
        qualification_establishments: Vec::new(),
        claims: arrival_claims.to_vec(),
    })
}
