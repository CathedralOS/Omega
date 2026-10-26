//! Candidate deaths retain exact regions of an established occurrence across
//! selected transfers. Ordinary availability stays conservative; the retained
//! candidates never enter the unconditional permission ledger.
use super::cleanup_dispositions::{
    CustodyCut, RemainingCustody, StorageBindingChange, StorageEstablishment, TransferCustody,
};
use super::linear_obligations::LinearPlace;
use checked_trees::{
    ContextualCleanupFacts, FlowClaimJoinExit, FlowOwnershipFacts, FlowPermissionEventFact,
};
use facts::{PlaceRoot, PlaceSegment};
use language_semantics::{PermissionAccess, PermissionEventKind, PermissionEventSource};

#[derive(Clone, Default)]
pub(super) struct SelectedCleanup {
    owners: Vec<Owner>,
}

#[derive(Clone)]
struct Owner {
    selection: arena::Handle<checked_trees::FlowOwnedSelectionReceipt>,
    root: PlaceRoot,
    path: Vec<PlaceSegment>,
    reference: typed_trees::types::TypeReferenceHandle,
    origin: FlowPermissionEventFact,
    places: Vec<LinearPlace>,
    remaining: RemainingCustody,
    /// A receipt-backed candidate can outlive the named home's occupant.
    /// Keep the exact capture cut and subsequent physical write barriers.
    source_bindings: Vec<SourceBinding>,
}

#[derive(Clone)]
struct SourceBinding {
    statement: usize,
    evaluation: checked_trees::FlowPermissionEvaluation,
    cut: CustodyCut,
    route: Vec<crate::checks::ownership_occurrences::EvaluationBranch>,
    detached_paths: Vec<Vec<PlaceSegment>>,
    unknown_write: bool,
}

impl SourceBinding {
    fn follows_capture(
        &self,
        source: PermissionEventSource,
        evaluation: checked_trees::FlowPermissionEvaluation,
        cut: CustodyCut,
    ) -> bool {
        let statement = match source {
            PermissionEventSource::Statement { statement_index }
            | PermissionEventSource::Call {
                statement_index, ..
            } => statement_index,
            _ => return false,
        };
        statement != self.statement || evaluation != self.evaluation || cut >= self.cut
    }

    fn observe_write(&mut self, root: PlaceRoot, write: &validation::CanonicalPlace) {
        if !write.root.is_valid() {
            self.unknown_write = true;
        } else if write.root == root && !self.detached_paths.contains(&write.segments) {
            self.detached_paths.push(write.segments.clone());
        }
    }

    fn carries_path(&self, program: &dyn validation::CheckingInput, path: &[PlaceSegment]) -> bool {
        !self.unknown_write
            && !self.detached_paths.iter().any(|detached| {
                crate::flow::canonical_place_segments_may_overlap(program, detached, path)
            })
    }
}

impl SelectedCleanup {
    pub(super) fn is_empty(&self) -> bool {
        self.owners.is_empty()
    }

    /// A completed selection transports one candidate; its exact pre-transfer
    /// complements remain owed. Capture every actual source cut, normalizing
    /// only paths this receipt transfers, never statement-entry custody.
    pub(super) fn observe_selection(
        &mut self,
        program: &dyn validation::CheckingInput,
        selection: arena::Handle<checked_trees::FlowOwnedSelectionReceipt>,
        ownership: &FlowOwnershipFacts,
        transfers: &[TransferCustody],
        observations: &[super::linear_validation::ObservedCustody],
        events: &[FlowPermissionEventFact],
        occurrences: &[crate::checks::ownership_occurrences::StatementOccurrence],
    ) -> Result<(), diagnostics::Diagnostic> {
        let receipt = ownership.owned_selections.get(selection);
        for source in ownership.selection_sources.span_or_empty(receipt.sources) {
            if !source.retains_storage {
                continue;
            }
            let root = PlaceRoot::Symbol(source.symbol);
            let reference = validation::canonical_place_type_reference(
                program,
                receipt.state,
                receipt.statement_ordinal as usize,
                &validation::CanonicalPlace {
                    root,
                    segments: Vec::new(),
                },
            )
            .ok_or_else(|| {
                diagnostics::Diagnostic::error(
                    "selected complement has no exact established source type",
                )
            })?;
            let selected_transfers = ownership
                .selection_transfers
                .span_or_empty(receipt.transfers)
                .iter()
                .filter(|transfer| {
                    transfer.source.is_valid()
                        && ownership.selection_sources.get(transfer.source).symbol == source.symbol
                })
                .collect::<Vec<_>>();
            let paths = selected_transfers
                .iter()
                .map(|transfer| ownership.segments.span_or_empty(transfer.path).to_vec())
                .collect::<Vec<_>>();
            for transfer in &selected_transfers {
                let source_place = validation::CanonicalPlace {
                    root: transfer.root,
                    segments: ownership.segments.span_or_empty(transfer.path).to_vec(),
                };
                let (places, remaining, cut) = super::owned_selection::evaluated_source_custody(
                    ownership,
                    transfer.expression,
                    &source_place,
                    receipt.evaluation,
                    transfers,
                    observations,
                    events,
                )?;
                let established = places
                    .iter()
                    .filter(|place| place.root == root)
                    .cloned()
                    .collect::<Vec<_>>();
                let captured_remaining = remaining.clone();
                let source_binding = SourceBinding {
                    statement: receipt.statement_ordinal as usize,
                    evaluation: receipt.evaluation,
                    cut,
                    route: occurrences
                        .get(cut.occurrence_ordinal)
                        .ok_or_else(|| {
                            diagnostics::Diagnostic::error(
                                "selected source cut has no evaluated occurrence",
                            )
                        })?
                        .route
                        .clone(),
                    detached_paths: Vec::new(),
                    unknown_write: false,
                };
                // Keep conditional claims live in the retained occurrence.
                // Its receipt chooses the actual transferred region; an
                // alternative's move does not consume skipped-path custody.
                let same = self.owners.iter().position(|owner| {
                    owner.selection == selection
                        && owner.root == root
                        && owner.path.is_empty()
                        && owner.places.len() == established.len()
                        && owner.places.iter().zip(&established).all(|(left, right)| {
                            let mut left = left.clone();
                            let mut right = right.clone();
                            if paths.iter().any(|path| left.path.starts_with(path)) {
                                left.live = false;
                                right.live = false;
                            }
                            left == right
                        })
                        && owner.remaining.same_storage_after_transfers(
                            &captured_remaining,
                            root,
                            &paths,
                        )
                });
                if let Some(index) = same {
                    let bindings = &mut self.owners[index].source_bindings;
                    if !bindings.iter().any(|binding| {
                        binding.statement == source_binding.statement
                            && binding.evaluation == source_binding.evaluation
                            && binding.cut == source_binding.cut
                            && binding.route == source_binding.route
                    }) {
                        bindings.push(source_binding);
                    }
                    continue;
                }
                self.owners.push(Owner {
                    selection,
                    root,
                    path: Vec::new(),
                    reference,
                    origin: FlowPermissionEventFact {
                        machine_symbol: receipt.machine,
                        state_symbol: receipt.state,
                        source: PermissionEventSource::Statement {
                            statement_index: receipt.statement_ordinal as usize,
                        },
                        expression: transfer.expression,
                        evaluation: receipt.evaluation,
                        kind: PermissionEventKind::Transfer,
                        multiplicity: language_semantics::Multiplicity::Affine,
                        access: PermissionAccess::Owned,
                        provenance: transfer.provenance,
                        claim_identity: transfer.claim_identity,
                        root,
                        ..Default::default()
                    },
                    places: established,
                    remaining: captured_remaining,
                    source_bindings: vec![source_binding],
                });
            }
        }
        Ok(())
    }

    pub(super) fn observe(
        &mut self,
        program: &dyn validation::CheckingInput,
        selected: &[typed_trees::expression::ExpressionHandle],
        transfers: &[TransferCustody],
        routes: &super::selected_evaluation::ExpressionRoutes,
        calls: &[checked_trees::FlowCallFact],
        events: &[FlowPermissionEventFact],
        occurrences: &[crate::checks::ownership_occurrences::StatementOccurrence],
        establishments: &[StorageEstablishment],
        binding_changes: &[StorageBindingChange],
        ownership: &mut FlowOwnershipFacts,
        output: &mut ContextualCleanupFacts,
    ) -> Result<(), diagnostics::Diagnostic> {
        if self.owners.is_empty() && selected.is_empty() && events.is_empty() {
            return Ok(());
        }
        let mut next_binding_change = 0;
        for (ordinal, event) in events.iter().enumerate() {
            while binding_changes
                .get(next_binding_change)
                .is_some_and(|change| change.cut.permission_prefix <= ordinal)
            {
                self.observe_binding_change(&binding_changes[next_binding_change]);
                next_binding_change += 1;
            }
            if event.access != PermissionAccess::Owned {
                continue;
            }
            let path = ownership.segments.span_or_empty(event.segments).to_vec();
            for establishment in establishments
                .iter()
                .filter(|entry| entry.event_ordinal == ordinal)
            {
                self.retire_transferred(routes, calls, &events[..ordinal], ownership)?;
                let destination = &establishment.destination;
                for owner in &mut self.owners {
                    for binding in &mut owner.source_bindings {
                        // These are the receiving writes after the source
                        // evaluation has joined, including the capture itself.
                        binding.observe_write(owner.root, destination);
                    }
                }
                let mut retired = Vec::new();
                for (index, owner) in self.owners.iter().enumerate() {
                    if owner.selection.is_valid() {
                        // This captured occurrence is still conditionally owed.
                        // A later write establishes a distinct occupant; it
                        // cannot redirect the earlier candidate's death.
                        continue;
                    }
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
            if !matches!(
                event.kind,
                PermissionEventKind::Transfer | PermissionEventKind::Consume
            ) {
                continue;
            }
            if !selected.contains(&event.expression) {
                self.owners.retain(|owner| {
                    owner.selection.is_valid()
                        || owner.root != event.root
                        || !owner.path.starts_with(&path)
                });
                for owner in self
                    .owners
                    .iter_mut()
                    .filter(|owner| owner.root == event.root)
                {
                    // An ordinary transfer can close a retained complement
                    // only while this exact original occupant still supplies
                    // that path. A later receiving write must not redirect an
                    // earlier candidate's death to a different value.
                    let same_source = if owner.selection.is_valid() {
                        let mut snapshots = transfers
                            .iter()
                            .filter(|snapshot| snapshot.events.contains(&ordinal));
                        if let (Some(snapshot), None) = (snapshots.next(), snapshots.next()) {
                            // A branch-local move is not an unconditional
                            // retirement of this retained candidate. Its
                            // existing selected receipt owns that disposition.
                            occurrences
                                .get(snapshot.occurrence_ordinal)
                                .is_some_and(|occurrence| occurrence.route.is_empty())
                                && !owner.source_bindings.is_empty()
                                && owner.source_bindings.iter().all(|binding| {
                                    binding.follows_capture(
                                        event.source,
                                        event.evaluation,
                                        CustodyCut {
                                            occurrence_ordinal: snapshot.occurrence_ordinal,
                                            permission_prefix: ordinal,
                                        },
                                    ) && binding.carries_path(program, &path)
                                })
                        } else {
                            false
                        }
                    } else {
                        true
                    };
                    if same_source {
                        owner.remaining.transfer_storage(event.root, &path);
                    }
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
            let transfer = transfers
                .iter()
                .find(|transfer| transfer.events.contains(&ordinal))
                .ok_or_else(|| {
                    unsupported(
                        program,
                        event,
                        "selected transfer has no producer custody snapshot",
                    )
                })?;
            let before_transfer = &transfer.remaining;
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
            // No-code disposal still ends an owned occurrence. Retain its
            // skipped-path receipt alongside executable cleanup so every
            // consumer, including a state guard, sees the same disposition.
            // Cleanup action selection decides whether any code is required;
            // absence of a hook is not absence of ownership.
            if self.owners.iter().any(|owner| {
                if owner.root != event.root {
                    return false;
                }
                if !owner.selection.is_valid() {
                    return owner.path == path;
                }
                if !path.starts_with(&owner.path) {
                    return false;
                }
                let receipt = ownership.owned_selections.get(owner.selection);
                receipt.state == event.state_symbol
                    && receipt.evaluation == event.evaluation
                    && event.source
                        == PermissionEventSource::Statement {
                            statement_index: receipt.statement_ordinal as usize,
                        }
                    && ownership
                        .selection_transfers
                        .span_or_empty(receipt.transfers)
                        .iter()
                        .any(|transfer| {
                            transfer.root == event.root
                                && transfer.expression == event.expression
                                && ownership.segments.span_or_empty(transfer.path) == path
                        })
            }) {
                continue;
            }
            let established = transfer
                .places
                .iter()
                .filter(|place| place.root == event.root)
                .cloned()
                .collect::<Vec<_>>();
            // Establishment belongs to the storage occurrence, not to a
            // surviving resource claim. A consumed linear child may leave an
            // affine sibling; its current presence is checked below against
            // the exact remaining region, without reviving the consumed child.
            // A reserved root (a declared local its stores establish field
            // by field) has no established whole place; its construction
            // frontier, checked below for the exact region, is the authority.
            let reserved = before_transfer.has_storage_frontier(event.root);
            // This prefix follows the permission producer's selected edge.
            // Known absence is an invalid use, not missing cleanup evidence;
            // do not let the later custody-gap diagnostic hide a second move.
            if (established.iter().any(|place| place.ever_established) || reserved)
                && !before_transfer.is_present(event.root, &path)
            {
                let message = "owned region is unavailable at this selected transfer: it was already transferred or has not been established";
                let Some((machine, state)) = program
                    .machine_by_symbol(event.machine_symbol)
                    .zip(program.state_by_symbol(event.state_symbol))
                else {
                    return Err(unsupported(program, event, message));
                };
                let spelling = super::borrowed_windows::place_spelling(program, event.root, &path);
                let reason = if reserved {
                    format!(
                        "cannot use `{spelling}`: this selected region was transferred or has not been established"
                    )
                } else {
                    format!(
                        "cannot use `{spelling}` after `{spelling}` was transferred or consumed"
                    )
                };
                return Err(diagnostics::Diagnostic::error(format!(
                    "{reason} in machine `{}` state `{}`",
                    machine.name.as_str(),
                    state.name.as_str(),
                ))
                .with_source_span(source_gap(program, event, message).source_span));
            }
            if established.is_empty()
                || !(established.iter().any(|place| place.ever_established) || reserved)
                || !before_transfer.is_present(event.root, &path)
            {
                return Err(unsupported(
                    program,
                    event,
                    "selected owner region has no exact established occurrence custody",
                ));
            }
            // A selected transfer out of an incomplete construction keeps
            // the retained region's death in reverse establishment order
            // beside the root's later establishments: `finish` orders every
            // death of a reserved root by the merged establishment history.
            // This is potential retained storage, not a cleanup receipt. A
            // later all-route transfer may remove the candidate altogether.
            // Surviving candidates authenticate exact claims and owner cleanup
            // eligibility in events(), where consumption actually occurs.
            self.owners.push(Owner {
                selection: Default::default(),
                root: event.root,
                path,
                reference,
                origin: event.clone(),
                places: established,
                remaining: before_transfer.clone(),
                source_bindings: Vec::new(),
            });
        }
        for change in &binding_changes[next_binding_change..] {
            self.observe_binding_change(change);
        }
        self.retire_transferred(routes, calls, events, ownership)
    }

    fn observe_binding_change(&mut self, change: &StorageBindingChange) {
        for owner in &mut self.owners {
            for binding in &mut owner.source_bindings {
                if !binding.follows_capture(change.source, change.evaluation, change.cut) {
                    continue;
                }
                let same_evaluation = matches!(change.source,
                    PermissionEventSource::Statement { statement_index }
                        | PermissionEventSource::Call { statement_index, .. }
                        if statement_index == binding.statement)
                    && change.evaluation == binding.evaluation;
                if same_evaluation
                    && !crate::checks::ownership_occurrences::routes_compatible(
                        &binding.route,
                        &change.route,
                    )
                {
                    continue;
                }
                let Some(writes) = &change.writes else {
                    binding.unknown_write = true;
                    continue;
                };
                for write in writes {
                    binding.observe_write(owner.root, write);
                }
            }
        }
    }

    /// A candidate represents custody retained by some predecessor, not an
    /// obligation to invent a skipped predecessor. Remove it only when this
    /// evaluation consumed its whole region on every route. This may-presence
    /// join is deliberately distinct from ordinary must-have availability;
    /// partial child moves cannot prove absence of their enclosing owner.
    fn retire_transferred(
        &mut self,
        routes: &super::selected_evaluation::ExpressionRoutes,
        calls: &[checked_trees::FlowCallFact],
        events: &[FlowPermissionEventFact],
        ownership: &FlowOwnershipFacts,
    ) -> Result<(), diagnostics::Diagnostic> {
        if self.owners.is_empty() {
            return Ok(());
        }
        let mut present = vec![true; self.owners.len()];
        let mut selection = super::selected_evaluation::SelectedEvaluation::new();
        let join = |_branch, current: &mut Vec<bool>, sibling: &Vec<bool>| {
            for (current, sibling) in current.iter_mut().zip(sibling) {
                *current |= *sibling;
            }
            Ok(())
        };
        for event in events
            .iter()
            .filter(|event| event.access == PermissionAccess::Owned)
        {
            selection.enter(
                routes.event(event, calls),
                &mut present,
                |_, _| Ok(()),
                join,
            )?;
            let path = ownership.segments.span_or_empty(event.segments);
            for (owner, present) in self.owners.iter().zip(&mut present) {
                if !owner.selection.is_valid()
                    && owner.root == event.root
                    && owner.path.starts_with(path)
                {
                    match event.kind {
                        PermissionEventKind::Transfer => *present = false,
                        PermissionEventKind::Establish | PermissionEventKind::Restore => {
                            *present = true;
                        }
                        // Cleanup acts on its contextual complement, not
                        // necessarily the whole named path. In particular a
                        // containing replacement's ordinary drop excludes
                        // this retained selected region; it cannot prove
                        // the region already died.
                        PermissionEventKind::Consume | PermissionEventKind::AffineDrop => {}
                    }
                }
            }
        }
        selection.enter(&[], &mut present, |_, _| Ok(()), join)?;
        let mut presence = present.into_iter();
        self.owners.retain(|_| presence.next().unwrap_or(true));
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
        let mut deaths = Vec::new();
        for event in ordinary {
            let path = ownership.segments.span_or_empty(event.segments);
            let regions = remaining.disposal_paths(program, event.root, path);
            if regions.is_empty() {
                // The ordinary event may own only an empty home's retirement.
                // Preserve that receipt without inventing a live subtree.
                deaths.push((event.clone(), None));
                continue;
            }
            for region in regions {
                let mut death = event.clone();
                death.segments = ownership.segments.insert_many(region);
                deaths.push((death, None));
            }
        }
        for (index, owner) in self.owners.iter().enumerate() {
            let captured_home = owner.selection.is_valid()
                && program.type_multiplicity(
                    ownership
                        .owned_selections
                        .get(owner.selection)
                        .type_reference,
                ) != language_semantics::Multiplicity::Linear;
            if !captured_home && remaining.definitely_transferred(owner.root, &owner.path) {
                continue;
            }
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
        // Resolve the ordinary remainder before ordering it beside retained
        // candidates: an incomplete root's empty path is a storage boundary,
        // not the establishment coordinate of every subtree it still owns.
        // Within a root under construction, deaths follow reverse
        // establishment order. A retained selected region keeps its position
        // from the owner's pre-transfer frontier; subtrees established after
        // the transfer follow it in the current frontier. A completed
        // aggregate or an ordinary root keeps its source order, where the
        // whole-root death precedes the retained candidate regions.
        let establishment_orders = roots
            .iter()
            .map(|root| {
                let mut order = Vec::<Vec<PlaceSegment>>::new();
                for owner in self.owners.iter().filter(|owner| owner.root == *root) {
                    for path in owner.remaining.established_paths(*root) {
                        if !order.contains(path) {
                            order.push(path.clone());
                        }
                    }
                }
                for path in remaining.established_paths(*root) {
                    if !order.contains(path) {
                        order.push(path.clone());
                    }
                }
                order
            })
            .collect::<Vec<_>>();
        deaths.sort_by_key(|(event, _)| {
            let root_position = roots.iter().position(|root| *root == event.root);
            let path = ownership.segments.span_or_empty(event.segments);
            let establishment = root_position
                .and_then(|position| {
                    establishment_orders[position]
                        .iter()
                        .position(|established| path.starts_with(established))
                })
                .map_or(0, |position| position + 1);
            (
                std::cmp::Reverse(root_position),
                std::cmp::Reverse(establishment),
            )
        });
        // A root whose retained path never completes the aggregate dies in
        // reverse establishment order on every path; one that completes it
        // folds into a whole value and dies in declared order.
        let execution_order = program
            .statement_table
            .admitted_execution_order(state.statement_nodes);
        let establishment_ordered_roots = roots
            .iter()
            .filter(|root| {
                self.owners.iter().any(|owner| {
                    if owner.root != **root {
                        return false;
                    }
                    let captured_at = match owner.origin.source {
                        PermissionEventSource::Statement { statement_index }
                        | PermissionEventSource::Call {
                            statement_index, ..
                        } => statement_index,
                        _ => 0,
                    };
                    let receiving_writes = output
                        .storage_establishments
                        .iter()
                        .filter_map(|(_, write)| {
                            let statement = match write.source {
                                PermissionEventSource::Statement { statement_index }
                                | PermissionEventSource::Call {
                                    statement_index, ..
                                } => statement_index,
                                _ => return None,
                            };
                            let path = output.segments.span_or_empty(write.destination.segments);
                            if write.machine == owner.origin.machine_symbol
                                && write.state == owner.origin.state_symbol
                                && write.destination.root == **root
                                && execution_order.at_or_before(captured_at, statement)
                                && execution_order.at_or_before(statement, exit.statement_index)
                                && remaining.is_present(**root, path)
                            {
                                Some(path.to_vec())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>();
                    !owner.remaining.completes_with(
                        program,
                        **root,
                        remaining.established_paths(**root),
                        &receiving_writes,
                    )
                })
            })
            .copied()
            .collect::<Vec<_>>();
        let mut candidate_remaining = self
            .owners
            .iter()
            .map(|owner| owner.remaining.clone())
            .collect::<Vec<_>>();
        for (event, candidate) in deaths {
            if let Some(index) = candidate {
                let first = output.dispositions.len();
                candidate_remaining[index].observe(
                    program,
                    &[event],
                    &ownership.segments,
                    output,
                    exit,
                    &[],
                    true,
                    &establishment_ordered_roots,
                );
                let handles = output
                    .dispositions
                    .iter()
                    .skip(first)
                    .map(|(handle, _)| handle)
                    .collect::<Vec<_>>();
                for handle in handles {
                    output.dispositions.get_mut(handle).selection = self.owners[index].selection;
                }
            } else {
                remaining.observe(
                    program,
                    &[event],
                    &ownership.segments,
                    output,
                    exit,
                    &[],
                    false,
                    &establishment_ordered_roots,
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
        let first = output.dispositions.len();
        self.remaining.clone().observe(
            program,
            &events,
            &ownership.segments,
            output,
            exit,
            &[],
            true,
            &[],
        );
        let handles = output
            .dispositions
            .iter()
            .skip(first)
            .map(|(handle, _)| handle)
            .collect::<Vec<_>>();
        for handle in handles {
            output.dispositions.get_mut(handle).selection = self.selection;
        }
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
