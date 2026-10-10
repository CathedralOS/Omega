//! Candidate deaths retain exact regions of an established occurrence across
//! selected transfers. Ordinary availability stays conservative; the retained
//! candidates never enter the unconditional permission ledger.
use super::cleanup_dispositions::{RemainingCustody, StorageEstablishment};
use super::linear_obligations::LinearPlace;
use checked_trees::{
    ContextualCleanupFacts, FlowClaimJoinExit, FlowOwnershipFacts, FlowPermissionEventFact,
};
use facts::{PlaceRoot, PlaceSegment};
use language_semantics::{
    PermissionAccess, PermissionEventKind, PermissionEventSource, PermissionProvenance,
};

#[derive(Clone, Default)]
pub(super) struct SelectedCleanup {
    owners: Vec<Owner>,
}

#[derive(Clone)]
struct Owner {
    root: PlaceRoot,
    path: Vec<PlaceSegment>,
    reference: typed_trees::types::TypeReferenceHandle,
    origin: FlowPermissionEventFact,
    places: Vec<LinearPlace>,
    remaining: RemainingCustody,
}

impl SelectedCleanup {
    pub(super) fn observe(
        &mut self,
        program: &dyn validation::CheckingInput,
        selected: &[typed_trees::expression::ExpressionHandle],
        places: &[LinearPlace],
        remaining: &RemainingCustody,
        events: &[FlowPermissionEventFact],
        establishments: &[StorageEstablishment],
        ownership: &mut FlowOwnershipFacts,
        output: &mut ContextualCleanupFacts,
    ) -> Result<(), diagnostics::Diagnostic> {
        let mut prefix = remaining.clone();
        for (ordinal, event) in events.iter().enumerate() {
            if event.access != PermissionAccess::Owned {
                continue;
            }
            let path = ownership.segments.span_or_empty(event.segments).to_vec();
            for establishment in establishments
                .iter()
                .filter(|entry| entry.event_ordinal == ordinal)
            {
                let destination = &establishment.destination;
                let mut retired = Vec::new();
                for (index, owner) in self.owners.iter().enumerate() {
                    if owner.root != destination.root {
                        continue;
                    }
                    if owner.path.starts_with(&destination.segments) {
                        // The statement's pre-store context owns the old
                        // receiver. The replacement cannot supply its facts.
                        owner.publish(
                            program,
                            event.clone(),
                            FlowClaimJoinExit::default(),
                            ownership,
                            output,
                        )?;
                        retired.push(index);
                    } else if destination.segments.starts_with(&owner.path) {
                        return Err(unsupported(
                            program,
                            event,
                            "a child replacement cannot restore a conditionally transferred containing owner",
                        ));
                    }
                }
                for index in retired.into_iter().rev() {
                    self.owners.remove(index);
                }
            }
            let contained_transfer = events[..ordinal].iter().any(|prior| {
                prior.root == event.root
                    && prior.expression == event.expression
                    && prior.source == event.source
                    && prior.evaluation == event.evaluation
                    && prior.kind == event.kind
                    && prior.access == PermissionAccess::Owned
                    && prior.multiplicity == language_semantics::Multiplicity::Affine
                    && matches!(
                        prior.kind,
                        PermissionEventKind::Transfer | PermissionEventKind::Consume
                    )
                    && path.starts_with(ownership.segments.span_or_empty(prior.segments))
            });
            let before_transfer = (selected.contains(&event.expression)
                && matches!(
                    event.kind,
                    PermissionEventKind::Transfer | PermissionEventKind::Consume
                ))
            .then(|| prefix.clone());
            prefix.advance(program, std::slice::from_ref(event), &ownership.segments);
            if !matches!(
                event.kind,
                PermissionEventKind::Transfer | PermissionEventKind::Consume
            ) {
                continue;
            }
            if !selected.contains(&event.expression) {
                self.owners
                    .retain(|owner| owner.root != event.root || !owner.path.starts_with(&path));
                for owner in self
                    .owners
                    .iter_mut()
                    .filter(|owner| owner.root == event.root)
                {
                    owner.remaining.transfer_storage(event.root, &path);
                }
                continue;
            }
            if contained_transfer {
                // One consuming occurrence owns one maximal cleanup region.
                // Its nested claim events still replay exact conservation.
                continue;
            }
            if event.root.as_symbol().is_none() {
                // An expression owner is born only where it is evaluated.
                continue;
            }
            let statement = match event.source {
                PermissionEventSource::Statement { statement_index }
                | PermissionEventSource::Call {
                    statement_index, ..
                } => statement_index,
                _ => 0,
            };
            let Some(reference) = validation::canonical_place_type_reference(
                program,
                event.state_symbol,
                statement,
                &validation::CanonicalPlace {
                    root: event.root,
                    segments: path.clone(),
                },
            ) else {
                return Err(unsupported(
                    program,
                    event,
                    "selected owner region has no exact declared type",
                ));
            };
            if !super::type_carries_linear_obligation(program, reference)
                && !validation::type_graph_requires_nominal_drop(
                    program,
                    reference,
                    &[],
                    &mut std::collections::BTreeSet::new(),
                )
            {
                // Regions without linear obligations or nominal hooks keep
                // ordinary no-code reconciliation, including whole values.
                // Their owned argument events need no retained cleanup receipt.
                continue;
            }
            if self
                .owners
                .iter()
                .any(|owner| owner.root == event.root && owner.path == path)
            {
                continue;
            }
            let established = places
                .iter()
                .filter(|place| place.root == event.root)
                .cloned()
                .collect::<Vec<_>>();
            let carrier = event.multiplicity == language_semantics::Multiplicity::Affine
                && event.claim_identity == language_semantics::PermissionClaimIdentity::Unknown
                && !event.obligation_live
                && !established.iter().any(|place| place.path == path);
            let exact_claims = established
                .iter()
                .filter(|place| {
                    place.live
                        && place.multiplicity == language_semantics::Multiplicity::Linear
                        && place.path.starts_with(&path)
                })
                .all(|place| {
                    !matches!(
                        place.claim_identity,
                        None | Some(language_semantics::PermissionClaimIdentity::Unknown)
                    ) && !matches!(place.provenance, None | Some(PermissionProvenance::Unknown))
                });
            if established.is_empty()
                || !established
                    .iter()
                    .any(|place| place.ever_established && (place.live || place.case_excluded))
                || if carrier {
                    // A transparent carrier has no aggregate claim lineage.
                    // Its saved receipts retain every contained live claim's
                    // exact identity and provenance instead.
                    !exact_claims
                } else {
                    event.provenance == PermissionProvenance::Unknown
                        || !established
                            .iter()
                            .any(|place| place.provenance == Some(event.provenance))
                }
                || before_transfer
                    .as_ref()
                    .is_none_or(|remaining| !remaining.is_present(event.root, &path))
            {
                return Err(unsupported(
                    program,
                    event,
                    "selected owner region has no exact established occurrence custody",
                ));
            }
            if before_transfer.as_ref().is_some_and(|remaining| {
                remaining
                    .cleanup_paths(event.root)
                    .is_some_and(|mut paths| !paths.any(<[PlaceSegment]>::is_empty))
            }) {
                // An incomplete construction dies in reverse establishment
                // order, not the completed aggregate's declaration order.
                return Err(unsupported(
                    program,
                    event,
                    "selected cleanup of an incomplete construction requires retained establishment order",
                ));
            }
            self.owners.push(Owner {
                root: event.root,
                path,
                reference,
                origin: event.clone(),
                places: established,
                remaining: before_transfer.expect("selected transfer retained its exact prefix"),
            });
        }
        Ok(())
    }

    pub(super) fn finish(
        &self,
        program: &dyn validation::CheckingInput,
        state: &typed_trees::state::State,
        exit: FlowClaimJoinExit,
        ordinary: &[FlowPermissionEventFact],
        remaining: &mut RemainingCustody,
        ownership: &mut FlowOwnershipFacts,
        output: &mut ContextualCleanupFacts,
    ) -> Result<(), diagnostics::Diagnostic> {
        let mut deaths = ordinary
            .iter()
            .cloned()
            .map(|event| (event, None))
            .collect::<Vec<_>>();
        for (index, owner) in self.owners.iter().enumerate() {
            let mut template = owner.origin.clone();
            template.source = PermissionEventSource::StateExit;
            template.expression = Default::default();
            template.evaluation = super::linear_validation::exit_evaluation(program, state, exit);
            for candidate in owner.events(program, template, ownership)? {
                ownership
                    .selected_cleanup_permissions
                    .append(candidate.clone());
                deaths.push((candidate, Some(index)));
            }
        }
        let mut roots = program
            .state_parameters(state)
            .iter()
            .map(|parameter| PlaceRoot::Symbol(parameter.symbol))
            .collect::<Vec<_>>();
        roots.extend(
            program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .filter_map(|statement| {
                    statement
                        .as_local_data()
                        .map(|local| PlaceRoot::Symbol(local.symbol))
                }),
        );
        deaths.sort_by_key(|(event, _)| {
            std::cmp::Reverse(roots.iter().position(|root| *root == event.root))
        });
        let mut candidate_remaining = self
            .owners
            .iter()
            .map(|owner| owner.remaining.clone())
            .collect::<Vec<_>>();
        for (event, candidate) in deaths {
            if let Some(index) = candidate {
                candidate_remaining[index].observe(
                    program,
                    &[event],
                    &ownership.segments,
                    output,
                    exit,
                    &[],
                    true,
                );
            } else {
                remaining.observe(
                    program,
                    &[event],
                    &ownership.segments,
                    output,
                    exit,
                    &[],
                    false,
                );
            }
        }
        Ok(())
    }
}

impl Owner {
    fn events(
        &self,
        program: &dyn validation::CheckingInput,
        template: FlowPermissionEventFact,
        ownership: &mut FlowOwnershipFacts,
    ) -> Result<Vec<FlowPermissionEventFact>, diagnostics::Diagnostic> {
        let mut events = Vec::new();
        super::linear_validation::append_owned_region_cleanup_permission_events(
            program,
            self.reference,
            self.root,
            &self.path,
            &self.places,
            &self.remaining,
            template,
            &mut ownership.segments,
            &mut events,
        );
        if events.is_empty() {
            // A known live nominal claim without a cleanup hook is invalid
            // source. Failure to construct some other region remains an
            // explicit coverage gap, rather than inventing a disposition.
            let missing_disposition = self.places.iter().any(|place| {
                if place.root != self.root
                    || !place.live
                    || place.multiplicity != language_semantics::Multiplicity::Linear
                    || !place.path.starts_with(&self.path)
                {
                    return false;
                }
                validation::project_type_reference_from_segments(
                    program,
                    self.reference,
                    &place.path[self.path.len()..],
                )
                .is_some_and(|reference| {
                    let reference = validation::peel_declared_constraints(program, reference);
                    program
                        .closed_data_definition(reference)
                        .is_some_and(|data| {
                            data.properties.multiplicity == language_semantics::Multiplicity::Linear
                        })
                        && validation::nominal_cleanup_machine_for_type(program, reference, &[])
                            .is_none()
                })
            });
            let message = "cannot automatically dispose retained selected owner region: every live linear obligation requires an eligible disposition";
            if missing_disposition {
                return Err(diagnostics::Diagnostic::error(message)
                    .with_source_span(source_gap(program, &self.origin, message).source_span));
            }
            return Err(unsupported(
                program,
                &self.origin,
                "selected owner cleanup has no exact supported region disposition",
            ));
        }
        Ok(events)
    }

    fn publish(
        &self,
        program: &dyn validation::CheckingInput,
        template: FlowPermissionEventFact,
        exit: FlowClaimJoinExit,
        ownership: &mut FlowOwnershipFacts,
        output: &mut ContextualCleanupFacts,
    ) -> Result<(), diagnostics::Diagnostic> {
        let events = self.events(program, template, ownership)?;
        for event in &events {
            ownership.selected_cleanup_permissions.append(event.clone());
        }
        self.remaining.clone().observe(
            program,
            &events,
            &ownership.segments,
            output,
            exit,
            &[],
            true,
        );
        Ok(())
    }
}

fn source_gap(
    program: &dyn validation::CheckingInput,
    event: &FlowPermissionEventFact,
    message: &'static str,
) -> checked_trees::CheckedUnitPlanGap {
    use crate::execution::terminal_unit::plan_gaps::{GapSite, plan_gap};
    let site = match event.source {
        language_semantics::PermissionEventSource::Statement { statement_index }
        | language_semantics::PermissionEventSource::Call {
            statement_index, ..
        } => u32::try_from(statement_index)
            .ok()
            .map(|index| GapSite::Statement {
                state: event.state_symbol,
                index,
            }),
        _ => None,
    }
    .unwrap_or(GapSite::State(event.state_symbol));
    plan_gap(
        program,
        event.machine_symbol,
        site,
        checked_trees::CheckedUnitPlanGapReason::Unsupported(message),
    )
}

fn unsupported(
    program: &dyn validation::CheckingInput,
    event: &FlowPermissionEventFact,
    message: &'static str,
) -> diagnostics::Diagnostic {
    let gap = source_gap(program, event, message);
    diagnostics::Diagnostic::incomplete(
        message,
        gap.coverage.exceeded(gap.source_span.span.start as u64),
    )
    .with_source_span(gap.source_span)
}
