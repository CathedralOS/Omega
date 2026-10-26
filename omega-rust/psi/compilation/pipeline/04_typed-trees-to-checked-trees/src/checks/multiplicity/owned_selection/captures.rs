//! Selected custody is derived from the completed value graph. The source
//! occurrence schedule still owns evaluation and all operand effects. These
//! rows describe only the exact value transport at completion, not an AST
//! recognizer or another ownership evaluator.

use arena::Handle;
use checked_trees::{CheckFacts, FlowOperandCompletion, FlowOperandFact};
use diagnostics::Diagnostic;
use facts::{PlaceRoot, PlaceSegment};
use language_semantics::Multiplicity;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, TableMatchArm};
use typed_trees::types::TypeReferenceHandle;

pub(in crate::checks::multiplicity) struct CapturedOwner {
    pub expression: ExpressionHandle,
    pub source_arm: Handle<TableMatchArm>,
    pub source: validation::CanonicalPlace,
    pub destination: Vec<PlaceSegment>,
    pub type_reference: TypeReferenceHandle,
}

pub(in crate::checks::multiplicity) struct SelectionProduction {
    value: Handle<FlowOperandFact>,
    pub expression: ExpressionHandle,
    pub evaluation: checked_trees::FlowPermissionEvaluation,
    pub destination: validation::CanonicalPlace,
    pub type_reference: TypeReferenceHandle,
    pub owners: Vec<CapturedOwner>,
    pub transported: Vec<ExpressionHandle>,
}

impl SelectionProduction {
    /// A completed source must exist before capture can transfer its custody,
    /// even when the same expression also names the outgoing receiving value.
    pub(in crate::checks::multiplicity) fn captures_anonymous_source(
        &self,
        expression: ExpressionHandle,
    ) -> bool {
        self.owners.iter().any(|owner| {
            owner.expression == expression
                && owner.source.root == PlaceRoot::Expression(expression)
                && owner.source.segments.is_empty()
        })
    }

    pub(in crate::checks::multiplicity) fn for_statement(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        machine: SymbolHandle,
        state: SymbolHandle,
        statement: usize,
        evaluation: checked_trees::FlowPermissionEvaluation,
        places: &[crate::checks::multiplicity::linear_obligations::LinearPlace],
    ) -> Result<Option<Self>, Diagnostic> {
        let Some(flow) = facts.flow.control.state(machine, state) else {
            return Ok(None);
        };
        let stored = facts.flow.state_statement(flow, statement);
        let (capture, destination) = if let Some(stored) = stored.filter(|row| {
            facts.flow.control.operands.is_valid(row.stored_value)
                && evaluation == checked_trees::FlowPermissionEvaluation::Statement
        }) {
            let operand = facts.flow.control.operands.get(stored.stored_value);
            if !has_selection(facts, operand)
                || validation::reference_result_custody::binding_multiplicity(
                    program,
                    operand.type_reference,
                ) == Multiplicity::Unrestricted
            {
                return Ok(None);
            }
            let destination = facts
                .semantic
                .places
                .is_valid(stored.stored_destination)
                .then(|| facts.semantic.places.get(stored.stored_destination))
                .and_then(|place| {
                    crate::flow::canonical_place_from_semantic_place(
                        program,
                        &facts.semantic,
                        place,
                    )
                })
                .ok_or_else(|| Diagnostic::error("selected store has no exact receiving place"))?;
            (stored.stored_value, destination)
        } else {
            let Some(outgoing) = facts
                .flow
                .control
                .outgoing
                .span_or_empty(flow.outgoing)
                .iter()
                .find(|row| {
                    row.statement_index == statement
                        && row.returned_value.is_valid()
                        && match evaluation {
                            checked_trees::FlowPermissionEvaluation::Statement => {
                                !row.transition_target.is_valid()
                            }
                            checked_trees::FlowPermissionEvaluation::TransitionTarget(target) => {
                                row.transition_target == target
                            }
                            _ => false,
                        }
                })
            else {
                return Ok(None);
            };
            (
                outgoing.returned_value,
                validation::CanonicalPlace {
                    root: PlaceRoot::Unknown,
                    segments: Vec::new(),
                },
            )
        };
        let operand = facts.flow.control.operands.get(capture);
        if !has_selection(facts, operand)
            || validation::reference_result_custody::binding_multiplicity(
                program,
                operand.type_reference,
            ) == Multiplicity::Unrestricted
        {
            return Ok(None);
        }
        let mut owners = Vec::new();
        let mut transported = Vec::new();
        collect(
            program,
            facts,
            machine,
            evaluation,
            places,
            operand,
            Handle::invalid(),
            &[],
            &mut owners,
            &mut transported,
        )?;
        if owners.is_empty()
            && validation::reference_shell(program, operand.type_reference).is_some()
        {
            // Every arm formed an ordinary loan from incoming referent access,
            // not an installed descriptor's owned storage. Loan checking owns
            // that join; no selected-source custody receipt is manufactured.
            return Ok(None);
        }
        if owners.is_empty() {
            return Err(Diagnostic::error(
                "completed owned selection has no captured storage frontier",
            ));
        }
        Ok(Some(Self {
            value: capture,
            expression: operand.expression,
            evaluation,
            destination,
            type_reference: operand.type_reference,
            owners,
            transported,
        }))
    }

    pub(in crate::checks::multiplicity) fn record(
        &self,
        program: &dyn validation::CheckingInput,
        facts: &mut CheckFacts,
        machine: SymbolHandle,
        state: SymbolHandle,
        statement: usize,
        evaluated_transfers: &[super::super::cleanup_dispositions::TransferCustody],
        evaluated_observations: &[super::super::linear_validation::ObservedCustody],
        routes: &super::super::selected_evaluation::ExpressionRoutes,
        calls: &[checked_trees::FlowCallFact],
        events: &[checked_trees::FlowPermissionEventFact],
        call_frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> Result<(), Diagnostic> {
        use checked_trees::{
            FlowOwnedSelectionClaim, FlowOwnedSelectionReceipt, FlowOwnedSelectionSource,
            FlowOwnedSelectionTransfer,
        };
        use language_semantics::{
            PermissionAccess, PermissionClaimIdentity, PermissionEventKind, PermissionEventSource,
            PermissionProvenance,
        };
        let mut sources = Vec::new();
        for owner in &self.owners {
            if validation::reference_result_custody::source_value_is_copied(
                program,
                owner.type_reference,
            ) {
                // A reusable read keeps its own storage/death owner. It is not
                // a residual cleanup source for the selected receiving value.
                continue;
            }

            let Some(symbol) = owner.source.root.as_symbol() else {
                continue;
            };
            if sources
                .iter()
                .any(|source: &FlowOwnedSelectionSource| source.symbol == symbol)
            {
                continue;
            }
            if !super::source_outputs_are_attributed(
                program,
                facts,
                machine,
                state,
                statement,
                symbol,
                call_frames,
            ) {
                return Err(Diagnostic::error(
                    "selected source has an incompletely attributed retained output",
                ));
            }
            let declaration = program
                .state_by_symbol(state)
                .ok_or_else(|| Diagnostic::error("selected source state is unavailable"))?;
            let parameter = program
                .state_parameters(declaration)
                .iter()
                .enumerate()
                .find(|(_, parameter)| parameter.symbol == symbol);
            let ordinal = if let Some((position, _)) = parameter {
                position
            } else {
                program
                    .state_local_declaration(declaration, symbol)
                    .map(|(ordinal, _)| ordinal)
                    .ok_or_else(|| {
                        Diagnostic::error("selected source has no declaring storage occurrence")
                    })?
            };
            sources.push(FlowOwnedSelectionSource {
                symbol,
                statement_ordinal: ordinal as u32,
                retains_storage: self.retains_source_storage(
                    owner.source.root,
                    routes,
                    calls,
                    events,
                )?,
            });
        }
        let declaration = program
            .state_by_symbol(state)
            .ok_or_else(|| Diagnostic::error("selected source state is unavailable"))?;
        sources.sort_by_key(|source| {
            let local = program
                .state_parameters(declaration)
                .iter()
                .all(|parameter| parameter.symbol != source.symbol);
            std::cmp::Reverse((local, source.statement_ordinal))
        });
        let sources = facts.flow.ownership.selection_sources.insert_many(sources);
        let mut transfers = Vec::new();
        for owner in &self.owners {
            let source = if validation::reference_result_custody::source_value_is_copied(
                program,
                owner.type_reference,
            ) {
                Handle::invalid()
            } else {
                owner
                    .source
                    .root
                    .as_symbol()
                    .and_then(|symbol| {
                        facts
                            .flow
                            .ownership
                            .selection_sources
                            .span_or_empty(sources)
                            .iter()
                            .position(|source| source.symbol == symbol)
                    })
                    .map(|position| sources.handle_at(position as u32))
                    .unwrap_or_default()
            };
            let evaluated_places = if owner.source.root.as_symbol().is_some() {
                let (places, _, _) = evaluated_source_custody(
                    &facts.flow.ownership,
                    owner.expression,
                    &owner.source,
                    self.evaluation,
                    evaluated_transfers,
                    evaluated_observations,
                    events,
                )?;
                Some(places)
            } else {
                // An anonymous call product owns its completed value graph,
                // not a named home whose old provenance could stand in for it.
                None
            };
            let mut claims = Vec::new();
            for template in crate::checks::multiplicity::linear_validation::linear_claim_frontier(
                program,
                owner.type_reference,
            ) {
                let mut path = owner.source.segments.clone();
                path.extend_from_slice(&template.path);
                let same_completion = |event: &&checked_trees::FlowPermissionEventFact| {
                    event.expression == owner.expression
                        && event.root == owner.source.root
                        && event.machine_symbol == machine
                        && event.state_symbol == state
                        && event.evaluation == self.evaluation
                        && event.access == PermissionAccess::Owned
                        && event.source
                            == PermissionEventSource::Statement {
                                statement_index: statement,
                            }
                        && facts.flow.ownership.segments.span_or_empty(event.segments) == path
                };
                let mut matches = events
                    .iter()
                    .filter(same_completion)
                    .filter(|event| event.kind == PermissionEventKind::Transfer);
                let event = if let Some(event) = matches.next() {
                    event
                } else {
                    // The checked constructor may explicitly establish an
                    // inactive case payload. It contributes no consumed debt,
                    // but absence of a Transfer alone is never that evidence.
                    let mut inactive = events.iter().filter(same_completion).filter(|event| {
                        template.conditional
                            && event.kind == PermissionEventKind::Establish
                            && event.multiplicity == Multiplicity::Linear
                            && !event.obligation_live
                    });
                    if inactive.next().is_some() && inactive.next().is_none() {
                        continue;
                    }
                    return Err(Diagnostic::error(
                        "selected completion has no exact consumed claim occurrence",
                    ));
                };
                if matches.next().is_some() {
                    return Err(Diagnostic::error(
                        "selected completion has ambiguous claim custody",
                    ));
                }
                if let Some(places) = evaluated_places {
                    let mut matching = places
                        .iter()
                        .filter(|place| place.root == owner.source.root && place.path == path);
                    let (Some(place), None) = (matching.next(), matching.next()) else {
                        return Err(Diagnostic::error(
                            "selected claim has no unique evaluated source custody",
                        ));
                    };
                    if !place.live
                        || !place.ever_established
                        || place.case_excluded
                        || place
                            .claim_identity
                            .unwrap_or(PermissionClaimIdentity::Unknown)
                            != event.claim_identity
                        || place.provenance.unwrap_or(PermissionProvenance::Unknown)
                            != event.provenance
                    {
                        return Err(Diagnostic::error(
                            "selected claim disagrees with its evaluated source custody",
                        ));
                    }
                }
                // The exact consuming occurrence transports the current home.
                // Its snapshot authenticates the claim even when completed
                // graph lowering must resolve an unknown transport name.
                claims.push(FlowOwnedSelectionClaim {
                    path: facts.flow.ownership.segments.insert_many(path),
                    claim_identity: event.claim_identity,
                    provenance: event.provenance,
                });
            }
            let (provenance, claim_identity) = if let [claim] = claims.as_slice() {
                (claim.provenance, claim.claim_identity)
            } else if !claims.is_empty() {
                // A composite retains every child's exact claim. Their issuer
                // histories need not coincide, and cannot establish the
                // containing carrier's history. Retain a carrier summary only
                // when the evaluated snapshot has that exact affine occurrence.
                let provenance = evaluated_places
                    .and_then(|places| {
                        let mut matching = places.iter().filter(|place| {
                            place.root == owner.source.root
                                && place.path == owner.source.segments
                                && place.multiplicity == Multiplicity::Affine
                                && place.live
                                && place.ever_established
                        });
                        let (Some(place), None) = (matching.next(), matching.next()) else {
                            return None;
                        };
                        place.provenance
                    })
                    .unwrap_or(PermissionProvenance::Unknown);
                (provenance, PermissionClaimIdentity::Unknown)
            } else if validation::reference_result_custody::source_value_is_copied(
                program,
                owner.type_reference,
            ) {
                // Source use is reusable even when a local containing its
                // descriptor has affine disposal custody. Any recorded source
                // frontier must still be available at this exact read cut.
                if evaluated_places.is_some_and(|places| {
                    places.iter().any(|place| {
                        super::source_frontier_matches(
                            place,
                            owner.source.root,
                            &owner.source.segments,
                        ) && (!place.live || !place.ever_established || place.case_excluded)
                    })
                }) {
                    return Err(Diagnostic::error(
                        "selected copy has no live evaluated source custody",
                    ));
                }
                (
                    PermissionProvenance::Unknown,
                    PermissionClaimIdentity::Unknown,
                )
            } else if let Some(places) = evaluated_places {
                let depth = places
                    .iter()
                    .filter(|place| {
                        place.root == owner.source.root
                            && owner.source.segments.starts_with(&place.path)
                            && place.multiplicity == Multiplicity::Affine
                    })
                    .map(|place| place.path.len())
                    .max()
                    .ok_or_else(|| {
                        Diagnostic::error("selected descriptor has no evaluated carrier custody")
                    })?;
                let mut matching = places.iter().filter(|place| {
                    place.root == owner.source.root
                        && owner.source.segments.starts_with(&place.path)
                        && place.path.len() == depth
                        && place.multiplicity == Multiplicity::Affine
                });
                let (Some(place), None) = (matching.next(), matching.next()) else {
                    return Err(Diagnostic::error(
                        "selected descriptor has ambiguous evaluated carrier custody",
                    ));
                };
                if !place.live || !place.ever_established {
                    return Err(Diagnostic::error(
                        "selected descriptor has no live evaluated carrier custody",
                    ));
                }
                (
                    place.provenance.unwrap_or(PermissionProvenance::Unknown),
                    PermissionClaimIdentity::Unknown,
                )
            } else {
                (
                    PermissionProvenance::Unknown,
                    PermissionClaimIdentity::Unknown,
                )
            };
            // A known receiving occurrence may retain the earlier selection
            // graph. The home alone cannot select it after a replacement.
            let mut origins =
                facts
                    .flow
                    .ownership
                    .owned_selections
                    .iter()
                    .filter(|(_, receipt)| {
                        receipt.machine == machine
                            && receipt.state == state
                            && receipt.destination == owner.source.root
                            && receipt.value.is_valid()
                            && owner.source.segments.starts_with(
                                facts
                                    .flow
                                    .ownership
                                    .segments
                                    .span_or_empty(receipt.destination_path),
                            )
                            && provenance
                                == PermissionProvenance::Established {
                                    machine_symbol: machine,
                                    state_symbol: state,
                                    source: PermissionEventSource::Statement {
                                        statement_index: receipt.statement_ordinal as usize,
                                    },
                                }
                    });
            let origin_selection = origins.next().map(|(handle, _)| handle).unwrap_or_default();
            if origins.next().is_some() {
                return Err(Diagnostic::error(
                    "selected source has ambiguous receiving custody",
                ));
            }
            transfers.push(FlowOwnedSelectionTransfer {
                source_type_reference: owner.type_reference,
                root: owner.source.root,
                destination_path: facts
                    .flow
                    .ownership
                    .segments
                    .insert_many(owner.destination.iter().copied()),
                expression: owner.expression,
                source_arm: owner.source_arm,
                source,
                provenance,
                claim_identity,
                origin_selection,
                path: facts
                    .flow
                    .ownership
                    .segments
                    .insert_many(owner.source.segments.iter().copied()),
                claims: facts
                    .flow
                    .ownership
                    .selection_transfer_claims
                    .insert_many(claims),
            });
        }
        let transfers = facts
            .flow
            .ownership
            .selection_transfers
            .insert_many(transfers);
        facts
            .flow
            .ownership
            .owned_selections
            .append(FlowOwnedSelectionReceipt {
                value: self.value,
                evaluation: self.evaluation,
                machine,
                state,
                statement_ordinal: statement as u32,
                expression: self.expression,
                destination: self.destination.root,
                destination_path: facts
                    .flow
                    .ownership
                    .segments
                    .insert_many(self.destination.segments.iter().copied()),
                type_reference: self.type_reference,
                sources,
                transfers,
                death: PermissionEventSource::StateExit,
            });
        Ok(())
    }

    /// Derive residual presence through the existing completed occurrence
    /// schedule. Every branch joins immediately; a projected move or an arm
    /// which forms another value keeps this source's complement live.
    fn retains_source_storage(
        &self,
        root: PlaceRoot,
        routes: &super::super::selected_evaluation::ExpressionRoutes,
        calls: &[checked_trees::FlowCallFact],
        events: &[checked_trees::FlowPermissionEventFact],
    ) -> Result<bool, Diagnostic> {
        let mut present = true;
        let mut evaluation = super::super::selected_evaluation::SelectedEvaluation::new();
        let join = |_, current: &mut bool, sibling: &bool| {
            *current |= *sibling;
            Ok(())
        };
        for event in events {
            evaluation.enter(
                routes.event(event, calls),
                &mut present,
                |_, _| Ok(()),
                join,
            )?;
            if event.root == root
                && event.access == language_semantics::PermissionAccess::Owned
                && event.kind == language_semantics::PermissionEventKind::Transfer
                && self.owners.iter().any(|owner| {
                    owner.source.root == root
                        && owner.expression == event.expression
                        && owner.source.segments.is_empty()
                })
            {
                // Whole carrier movement remains whole even when the
                // permission stream names each contained linear claim.
                present = false;
            }
        }
        evaluation.enter(&[], &mut present, |_, _| Ok(()), join)?;
        Ok(present)
    }

    pub(in crate::checks::multiplicity) fn receives(
        &self,
        root: PlaceRoot,
        path: &[PlaceSegment],
    ) -> bool {
        root == self.root() && path.starts_with(&self.destination.segments)
    }

    pub(in crate::checks::multiplicity) fn root(&self) -> PlaceRoot {
        if self.destination.root.is_valid() {
            self.destination.root
        } else {
            PlaceRoot::Expression(self.expression)
        }
    }
}

/// Rejoin one source with its actual evaluated custody. The event range names
/// the pre-move snapshot; a descriptor read without a value Transfer instead
/// uses its exact observation. A source home or statement-entry inventory can
/// never substitute for either occurrence. The returned source cut starts with the
/// next evaluation position: a retained source association starts only after this
/// exact transfer/read, not at statement entry.
#[allow(clippy::too_many_arguments)]
pub(in crate::checks::multiplicity) fn evaluated_source_custody<'evaluation>(
    ownership: &checked_trees::FlowOwnershipFacts,
    expression: ExpressionHandle,
    source: &validation::CanonicalPlace,
    evaluation: checked_trees::FlowPermissionEvaluation,
    transfers: &'evaluation [super::super::cleanup_dispositions::TransferCustody],
    observations: &'evaluation [super::super::linear_validation::ObservedCustody],
    events: &[checked_trees::FlowPermissionEventFact],
) -> Result<
    (
        &'evaluation [crate::checks::multiplicity::linear_obligations::LinearPlace],
        &'evaluation super::super::cleanup_dispositions::RemainingCustody,
        super::super::cleanup_dispositions::CustodyCut,
    ),
    Diagnostic,
> {
    let mut snapshots = transfers.iter().filter(|snapshot| {
        snapshot.events.clone().any(|ordinal| {
            events.get(ordinal).is_some_and(|event| {
                event.expression == expression
                    && event.root == source.root
                    && event.evaluation == evaluation
                    && event.access == language_semantics::PermissionAccess::Owned
                    && event.kind == language_semantics::PermissionEventKind::Transfer
                    && ownership
                        .segments
                        .span_or_empty(event.segments)
                        .starts_with(&source.segments)
            })
        })
    });
    if let Some(snapshot) = snapshots.next() {
        if snapshots.next().is_some() {
            return Err(Diagnostic::error(
                "selected source has multiple evaluated transfer snapshots",
            ));
        }
        return Ok((
            &snapshot.places,
            &snapshot.remaining,
            super::super::cleanup_dispositions::CustodyCut {
                occurrence_ordinal: snapshot.occurrence_ordinal,
                permission_prefix: snapshot.events.end,
            },
        ));
    }
    let mut snapshots = observations
        .iter()
        .filter(|snapshot| snapshot.expression == expression && snapshot.place == *source);
    let (Some(snapshot), None) = (snapshots.next(), snapshots.next()) else {
        return Err(Diagnostic::error(
            "selected source has no unique evaluated read or transfer snapshot",
        ));
    };
    Ok((&snapshot.places, &snapshot.remaining, snapshot.cut))
}

fn has_selection(facts: &CheckFacts, operand: &FlowOperandFact) -> bool {
    match operand.completion {
        FlowOperandCompletion::Selected(_) => true,
        FlowOperandCompletion::Constructed(children) => facts
            .flow
            .control
            .operand_children
            .span_or_empty(children)
            .iter()
            .any(|child| has_selection(facts, facts.flow.control.operands.get(child.value))),
        FlowOperandCompletion::Copied { .. } | FlowOperandCompletion::Observation => false,
    }
}

fn collect(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    evaluation: checked_trees::FlowPermissionEvaluation,
    places: &[crate::checks::multiplicity::linear_obligations::LinearPlace],
    operand: &FlowOperandFact,
    arm: Handle<TableMatchArm>,
    destination: &[PlaceSegment],
    owners: &mut Vec<CapturedOwner>,
    transported: &mut Vec<ExpressionHandle>,
) -> Result<(), Diagnostic> {
    if validation::reference_result_custody::binding_multiplicity(program, operand.type_reference)
        == Multiplicity::Unrestricted
    {
        return Ok(());
    }
    match operand.completion {
        FlowOperandCompletion::Selected(alternatives) => {
            transported.push(operand.expression);
            let rows = facts
                .flow
                .control
                .operand_alternatives
                .span(alternatives)
                .filter(|rows| !rows.is_empty())
                .ok_or_else(|| {
                    Diagnostic::error("owned selection has no completed alternatives")
                })?;
            for alternative in rows {
                if !alternative.source_arm.is_valid()
                    || !facts.flow.control.operands.is_valid(alternative.value)
                {
                    return Err(Diagnostic::error(
                        "owned selection has an unavailable completed arm",
                    ));
                }
                collect(
                    program,
                    facts,
                    machine,
                    evaluation,
                    places,
                    facts.flow.control.operands.get(alternative.value),
                    alternative.source_arm,
                    destination,
                    owners,
                    transported,
                )?;
            }
            return Ok(());
        }
        FlowOperandCompletion::Constructed(children) => {
            // Nominal obligations belong to the completed constructor itself.
            // Transparent storage instead transports its contained owners.
            let frontier = crate::checks::multiplicity::linear_validation::linear_claim_frontier(
                program,
                operand.type_reference,
            );
            if !frontier.iter().any(|claim| claim.path.is_empty()) {
                let before = owners.len();
                for child in facts.flow.control.operand_children.span_or_empty(children) {
                    let mut path = destination.to_vec();
                    path.extend_from_slice(
                        facts
                            .flow
                            .control
                            .operand_child_paths
                            .span_or_empty(child.path),
                    );
                    collect(
                        program,
                        facts,
                        machine,
                        evaluation,
                        places,
                        facts.flow.control.operands.get(child.value),
                        arm,
                        &path,
                        owners,
                        transported,
                    )?;
                }
                if owners.len() != before {
                    transported.push(operand.expression);
                    return Ok(());
                }
            }
        }
        FlowOperandCompletion::Copied { .. } | FlowOperandCompletion::Observation => {}
    }
    let source = validation::ownership_place_from_expression_in_state(
        program,
        operand.state_symbol,
        operand.statement_index,
        operand.expression,
    )
    .ok_or_else(|| Diagnostic::error("completed owned value has no exact source place"))?;
    // Address formation strips to its referent place. Only a slot whose actual
    // carrier is this reference value can be installed descriptor storage;
    // owning T is not owning the &mut T freshly lent from that T.
    let installed_descriptor = validation::canonical_place_type_reference(
        program,
        operand.state_symbol,
        operand.statement_index,
        &source,
    )
    .is_some_and(|reference| {
        program.normalized_type_identity(reference)
            == program.normalized_type_identity(operand.type_reference)
    }) && places.iter().any(|place| {
        super::source_frontier_matches(place, source.root, &source.segments)
            && place.ever_established
    });
    if validation::reference_shell(program, operand.type_reference).is_some()
        && (forms_checked_loan(program, facts, machine, evaluation, operand)
            || (!installed_descriptor
                && program
                    .state_by_symbol(operand.state_symbol)
                    .is_some_and(|state| {
                        validation::reference_result_custody::initializer_source(
                            program,
                            state,
                            operand.expression,
                            operand.type_reference,
                        )
                        .is_some()
                    })))
    {
        // A borrowed ingress names its referent, not owned descriptor storage.
        // An installed descriptor at this exact place still transfers normally.
        return Ok(());
    }
    if source.segments.iter().any(|segment| {
        !matches!(segment, PlaceSegment::Field { symbol } if symbol.is_valid())
            && !matches!(
                segment,
                PlaceSegment::FixedIndex { .. } | PlaceSegment::Case { .. }
            )
    }) {
        return Err(Diagnostic::error(
            "completed owned selection has an unresolved source projection",
        ));
    }
    owners.push(CapturedOwner {
        expression: operand.expression,
        source_arm: arm,
        source,
        destination: destination.to_vec(),
        type_reference: operand.type_reference,
    });
    Ok(())
}

/// Formation and descriptor transport can have the same carrier and canonical
/// referent. Keep the semantic formation operation joined to its checked loan
/// occurrence rather than inferring a move from that carrier or referent.
fn forms_checked_loan(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    evaluation: checked_trees::FlowPermissionEvaluation,
    operand: &FlowOperandFact,
) -> bool {
    let Some(formation) = validation::reference_formation_source(program, operand.expression)
    else {
        return false;
    };
    let access = match formation.access {
        language_semantics::ReferenceAccess::Shared => checked_trees::BorrowAccessKind::Read,
        language_semantics::ReferenceAccess::Mutable => checked_trees::BorrowAccessKind::Mutable,
        language_semantics::ReferenceAccess::WriteOnly => {
            checked_trees::BorrowAccessKind::WriteOnly
        }
    };
    let Some((_, state)) = facts.borrow.states.iter().find(|(_, state)| {
        state.machine_symbol == machine && state.state_symbol == operand.state_symbol
    }) else {
        return false;
    };
    let local = facts
        .borrow
        .loans
        .span_or_empty(state.loans)
        .iter()
        .filter(|loan| {
            loan.statement_index == operand.statement_index
                && loan.formation_expression == operand.expression
                && loan.kind == access
        })
        .map(|loan| &loan.lineage);
    // A normal result has no local activation. Borrow production retains its
    // checked formation separately, at the exact selected return target.
    let returned = facts
        .borrow
        .returned_loans
        .span_or_empty(state.returned_loans)
        .iter()
        .filter(|loan| {
            let loan_evaluation = if loan.transition_target.is_valid() {
                checked_trees::FlowPermissionEvaluation::TransitionTarget(loan.transition_target)
            } else {
                checked_trees::FlowPermissionEvaluation::Statement
            };
            loan.statement_index == operand.statement_index
                && loan.formation_expression == operand.expression
                && loan.kind == access
                && loan_evaluation == evaluation
        })
        .map(|loan| &loan.lineage);
    let mut found = false;
    for lineage in local.chain(returned) {
        match lineage {
            checked_trees::BorrowLoanLineage::DirectRoot => {}
            checked_trees::BorrowLoanLineage::Reborrow { parent_loan } => {
                if !facts.borrow.state_owns_loan(state, *parent_loan) {
                    return false;
                }
            }
            // The formation operation still lends; an unavailable unique
            // lineage is not permission to manufacture a parent or move the
            // descriptor. Ordinary borrow admission keeps its existing fence.
            checked_trees::BorrowLoanLineage::UnretainedDerived => {}
            checked_trees::BorrowLoanLineage::SuppliedOutput
            | checked_trees::BorrowLoanLineage::LifetimeDependency => return false,
        }
        found = true;
    }
    found
}
