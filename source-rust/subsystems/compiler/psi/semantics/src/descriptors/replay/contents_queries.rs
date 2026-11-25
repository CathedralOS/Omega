use super::engine::{DescriptorReadDepth, Origin, State, Value};
use super::*;

impl Replay<'_> {
    pub(crate) fn location_capture(
        &self,
        machine: &DescriptorMachine,
        location: &DescriptorLocation,
        depth: &DescriptorReadDepth<'_>,
    ) -> Option<(MachineId, DescriptorPosition)> {
        let completed = depth.captured.get();
        if let Some(capture) = machine.location_capture(self.types, location, completed) {
            return Some(capture);
        }
        let index = location
            .path
            .iter()
            .position(|segment| matches!(segment, DescriptorPathSegment::FixedIndex(_)))?;
        let roster = DescriptorLocation {
            root: location.root,
            path: location.path[..index].to_vec(),
        };
        let (position, origins) =
            self.returned_roster_origins(machine, &roster, completed, Some(depth))?;
        if origins.is_empty()
            || !origins.iter().all(|origin| {
                matches!(
                    origin,
                    DescriptorStoredOrigin::Carried { .. } | DescriptorStoredOrigin::Empty
                )
            })
        {
            return None;
        }
        Some((machine.id, (position.0, position.1.checked_add(1)?)))
    }

    pub(crate) fn has_roster_source(
        &self,
        machine: &DescriptorMachine,
        location: &DescriptorLocation,
        depth: &DescriptorReadDepth<'_>,
    ) -> bool {
        let completed = depth.captured.get();
        (location.path.is_empty()
            && machine
                .sequence_construction(self.types, location.root, completed)
                .is_some())
            || self
                .returned_roster_origins(machine, location, completed, Some(depth))
                .is_some_and(|(_, origins)| {
                    !origins.is_empty()
                        && origins.iter().all(|origin| {
                            matches!(
                                origin,
                                DescriptorStoredOrigin::Carried { .. }
                                    | DescriptorStoredOrigin::Empty
                            )
                        })
                })
    }

    pub(crate) fn returned_roster_origins<'replay>(
        &'replay self,
        machine: &DescriptorMachine,
        location: &DescriptorLocation,
        completed: Option<(MachineId, DescriptorPosition)>,
        depth: Option<&DescriptorReadDepth<'_>>,
    ) -> Option<(DescriptorPosition, Vec<&'replay DescriptorStoredOrigin>)> {
        if !std::ptr::eq(self.machine(machine.id)?, machine)
            || completed.is_some_and(|(owner, _)| owner != machine.id)
        {
            return None;
        }
        let captured = completed.and_then(|(_, (block, index))| {
            let position = (block, index.checked_sub(1)?);
            (machine.producer(location.root) == Some(position)).then_some(position)
        });
        let position = captured.or_else(|| machine.producer(location.root))?;
        let call = machine.direct_call(position)?;
        let result = call.result.as_ref()?;
        let suffix = location.path.strip_prefix(result.path.as_slice())?;
        if result.root != location.root {
            return None;
        }
        // A public contract remains its own result account. A body that
        // merely implements it cannot supply narrower caller origins.
        let DescriptorInvocationTarget::Machine(callee) = call.target else {
            return None;
        };
        let Some(summary) = self.summaries.get(&callee).and_then(Option::as_ref) else {
            if let Some(depth) = depth {
                depth.unprepared_summary.set(true);
            }
            return None;
        };
        let slots = summary.result.slots()?;
        let slot = DescriptorResultSlot::matching(slots, suffix)?;
        Some((position, slot.origins_at(suffix)?))
    }

    pub(crate) fn resolved_location(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        location: &DescriptorLocation,
        depth: &DescriptorReadDepth<'_>,
    ) -> Option<(
        Vec<terminal_psi::StructuralPathSegment>,
        Option<StructuralTypeId>,
    )> {
        self.type_index.resolved_path(
            *machine.place_types.get(&location.root)?,
            &location.path,
            |prefix, view_type, ordinal| {
                let StructuralTypeShape::ElementView { element, .. } =
                    self.type_index.find(view_type)?.shape
                else {
                    return None;
                };
                let selected = DescriptorLocation {
                    root: location.root,
                    path: prefix.to_vec(),
                };
                let captured =
                    self.target_state(machine, state, depth.captured.get(), depth)?;
                let origins = match captured.slots.get(&selected) {
                    Some(Some(origins)) => origins.clone(),
                    Some(None) => return None,
                    None => self.read_with_depth(machine, captured, &selected, depth)?,
                };
                if origins.is_empty() {
                    return None;
                }
                for origin in origins {
                    let backing = match origin {
                        Origin::CapturedStorage(account) => account.local_coordinate()?.clone(),
                        Origin::Storage(backing) => backing,
                        _ => return None,
                    };
                    if backing.path.is_empty()
                        && let Some((_, actual, step)) = machine.sequence_construction_indexed(
                            &self.type_index,
                            backing.root,
                            depth.captured.get(),
                        )
                    {
                        let (_, destination) =
                            step.derivations.get(usize::try_from(ordinal).ok()?)?;
                        if actual != element
                            || destination.root != backing.root
                            || destination.path != [DescriptorPathSegment::FixedIndex(ordinal)]
                        {
                            return None;
                        }
                    } else {
                        let (_, origins) = self.returned_roster_origins(
                            machine,
                            &backing,
                            depth.captured.get(),
                            Some(depth),
                        )?;
                        if origins.is_empty() {
                            return None;
                        }
                        for origin in origins {
                            let DescriptorStoredOrigin::Carried {
                                element: actual,
                                elements,
                            } = origin
                            else {
                                return None;
                            };
                            if *actual != element
                                || usize::try_from(ordinal).ok()? >= elements.len()
                            {
                                return None;
                            }
                        }
                    }
                }
                Some(element)
            },
        )
    }

    pub(crate) fn unknown_contents_type(
        &self,
        machine: &DescriptorMachine,
        location: &DescriptorLocation,
    ) -> Option<StructuralTypeId> {
        let (last, path) = location.path.split_last()?;
        if *last != DescriptorPathSegment::UnknownContents {
            return None;
        }
        let root = *machine.place_types.get(&location.root)?;
        let carrier = self.type_index.descriptor_location_type(root, path)?;
        matches!(
            self.type_index.find(carrier)?.shape,
            StructuralTypeShape::TypeParameter
        )
        .then_some(carrier)
    }

    /// An indexed copy keeps its source array live even when the caller cannot
    /// name the individual element as a fixed slot. Its contents identify
    /// the copied value without granting access to any borrow inside it.
    pub(crate) fn returned_packet_input(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        call: &DescriptorInvocation,
        origin: &DescriptorStoredOrigin,
        expected: StructuralTypeId,
    ) -> Option<DescriptorLocation> {
        let query = match origin {
            DescriptorStoredOrigin::IncomingSlot { position, path } => {
                DescriptorContentsQuery::from_location(&self.actual_slot(call, *position, path)?)
            }
            DescriptorStoredOrigin::IncomingContents { position, steps, projection }
                if *projection == Projection::new(&[]) =>
            {
                let input = call.argument_contents.get(*position)?.as_ref()?.clone();
                self.extend_actual_contents(machine, call.actual_inputs(), input, steps)?
            }
            _ => return None,
        };
        if let Some(location) = query.exact_location() {
            let packet = self.canonical_contents_packet(machine, state, &location)?;
            return (self.unknown_contents_type(machine, &packet)? == expected).then_some(packet);
        }
        let root = *machine.place_types.get(&query.root)?;
        if query.steps.last()
            != Some(&DescriptorContentsStep::Address(DescriptorPathSegment::UnknownContents))
            || query.selected_type(self.types, root)? != Some(expected)
        {
            return None;
        }
        self.read_contents(machine, state, &query)?;
        let source = query.observation();
        let storage = self.resolve_storage(machine, state, &source, None);
        self.flow_location(machine, state, &source, storage).ok()
    }

    pub(crate) fn canonical_contents_packet(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        packet: &DescriptorLocation,
    ) -> Option<DescriptorLocation> {
        let expected = self.unknown_contents_type(machine, packet)?;
        let mut carrier = packet.clone();
        carrier.path.pop()?;
        let mut canonical = self
            .resolve_storage(machine, state, &carrier, None)?
            .slot_coordinate(&carrier)?;
        canonical.path.push(DescriptorPathSegment::UnknownContents);
        (self.unknown_contents_type(machine, &canonical)? == expected).then_some(canonical)
    }

    fn validate_query(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        query: &DescriptorContentsQuery,
        depth: &DescriptorReadDepth<'_>,
    ) -> Option<()> {
        let root = *machine.place_types.get(&query.root)?;
        query.validate_using(self.types, root, |carrier, path, original| {
            if original {
                let location = DescriptorLocation {
                    root: query.root,
                    path: path.to_vec(),
                };
                return Some(self.resolved_location(machine, state, &location, depth)?.1);
            }
            self.type_index.resolved_tip(carrier, path, |_, _, _| None)
        })
    }

    pub(crate) fn target_state<'state>(
        &'state self,
        machine: &DescriptorMachine,
        current: &'state State,
        completed: Option<(MachineId, DescriptorPosition)>,
        depth: &DescriptorReadDepth<'state>,
    ) -> Option<&'state State> {
        let Some((owner, position)) = completed else {
            return Some(current);
        };
        if owner != machine.id || !std::ptr::eq(self.machine(owner)?, machine) {
            return None;
        }
        let history = match depth.history {
            Some((context, history)) if std::ptr::eq(context, machine) => history,
            Some(_) => return None,
            None => self.snapshots.get(&owner)?.ready()?.before.as_ref(),
        };
        history.get(&position)
    }

    fn live_state<'state>(
        &self,
        machine: &DescriptorMachine,
        current: &'state State,
        depth: &DescriptorReadDepth<'state>,
    ) -> Option<&'state State> {
        match depth.live {
            Some((owner, state)) if std::ptr::eq(owner, machine) => Some(state),
            Some(_) => None,
            None => Some(current),
        }
    }

    pub(crate) fn read_contents(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        query: &DescriptorContentsQuery,
    ) -> Value {
        self.read_contents_with_depth(
            machine,
            state,
            query,
            &DescriptorReadDepth::at(machine, state, None),
        )
    }

    pub(crate) fn read_contents_with_depth(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        query: &DescriptorContentsQuery,
        depth: &DescriptorReadDepth<'_>,
    ) -> Value {
        let _read = depth.enter()?;
        if query.steps.last()
            == Some(&DescriptorContentsStep::Address(
                DescriptorPathSegment::UnknownContents,
            ))
        {
            return self.read_value_packet(machine, state, query, depth);
        }
        self.validate_query(machine, state, query, depth)?;
        if self.slot_kind == DescriptorSlotKind::AccessOrigins {
            if state.contents_path_invalidated(query) {
                return None;
            }
            if let Some(location) = query.exact_location()
                && let Some(origins) = self.read_with_depth(machine, state, &location, depth)
                && origins.iter().all(|origin| {
                    matches!(
                        origin,
                        Origin::Incoming(_)
                            | Origin::IncomingContents(_, _)
                            | Origin::Backing(DescriptorCallOrigin::AccessFormation { .. })
                    )
                })
            {
                return Some(origins);
            }
        }
        if self.slot_kind.tracks_references()
            && let Some(location) = query.exact_location()
            && self.location_capture(machine, &location, depth).is_none()
            && let Some(carrier) =
                self.reference_referent_carrier(machine, state, &location, depth)?
        {
            return self.read_with_depth(machine, state, &carrier, depth);
        }
        if self.slot_kind.tracks_references()
            && let Some(queried) = query.exact_location()
            && self.location_capture(machine, &queried, depth).is_none()
            && let Some(value) = self.known_current_contents(machine, state, &queried, depth)
        {
            return value.clone();
        }
        if state.contents_path_invalidated(query) {
            return None;
        }
        let mut origins = BTreeSet::new();
        for target in self.contents_targets_with_depth(machine, state, query, depth)? {
            match target {
                DescriptorPayloadTarget::Storage {
                    location,
                    completed,
                } => {
                    let state = self.target_state(machine, state, completed, depth)?;
                    // The target walk already checked this path using its
                    // selected return alternative. Reading the frozen slot
                    // must not require its ordinal in every other return.
                    let value = match (completed, state.slots.get(&location)) {
                        (Some(_), Some(value)) => value.clone(),
                        _ => self.read_with_depth(machine, state, &location, depth),
                    };
                    origins.extend(value?);
                }
                DescriptorPayloadTarget::Incoming(query) => {
                    origins.extend(self.read_incoming_contents(machine, state, &query, depth)?);
                }
            }
        }
        Some(origins)
    }

    fn read_value_packet(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        query: &DescriptorContentsQuery,
        depth: &DescriptorReadDepth<'_>,
    ) -> Value {
        if let Some(location) = query.exact_location() {
            self.unknown_contents_type(machine, &location)?;
            return self.read_with_depth(machine, state, &location, depth);
        }
        if state.contents_path_invalidated(query) {
            return None;
        }
        self.validate_query(machine, state, query, depth)?;
        let mut carrier = query.clone();
        carrier.steps.pop()?;
        let mut origins = BTreeSet::new();
        for target in self.contents_targets_with_depth(machine, state, &carrier, depth)? {
            match target {
                DescriptorPayloadTarget::Storage {
                    mut location,
                    completed,
                } => {
                    location.path.push(DescriptorPathSegment::UnknownContents);
                    let captured = self.target_state(machine, state, completed, depth)?;
                    origins.extend(self.read_with_depth(machine, captured, &location, depth)?);
                }
                DescriptorPayloadTarget::Incoming(mut incoming) => {
                    incoming.steps.push(DescriptorContentsStep::Address(
                        DescriptorPathSegment::UnknownContents,
                    ));
                    origins.extend(self.read_incoming_contents(machine, state, &incoming, depth)?);
                }
            }
        }
        (!origins.is_empty()).then_some(origins)
    }

    fn known_current_contents<'state>(
        &self,
        machine: &DescriptorMachine,
        state: &'state State,
        queried: &DescriptorLocation,
        depth: &DescriptorReadDepth<'_>,
    ) -> Option<&'state Value> {
        let reads = DescriptorReadDepth::at(machine, state, None);
        let _capture = reads.capture(depth.captured.get());
        let resolved = self.resolve_storage_with_depth(machine, state, queried, &reads);
        depth.absorb_summary_reads(&reads);
        let resolved = resolved?;
        let coordinate = resolved.local_coordinate().unwrap_or(queried);
        (!state.has_selected_inline_contents(coordinate)).then(|| state.slots.get(coordinate))?
    }

    fn read_incoming_contents(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        query: &DescriptorContentsQuery,
        depth: &DescriptorReadDepth<'_>,
    ) -> Value {
        let _read = depth.enter()?;
        let state = self.live_state(machine, state, depth)?;
        if self.slot_kind.tracks_references()
            && let Some(location) = query.exact_location()
            && self.location_capture(machine, &location, depth).is_none()
            && self.incoming_references_unchanged(machine, state, &location, depth)
            && let Some(value) = self.known_current_contents(machine, state, &location, depth)
        {
            return value.clone();
        }
        let mut origins = state.incoming_contents(query)?;
        if query.is_symbolic_location() {
            // A fixed-slot write may have happened before or after the
            // selected write. Until the selector is instantiated, a
            // local read must retain the fixed alternatives as well.
            origins.extend(self.read_contents_with_depth(
                machine,
                state,
                &query.without_selectors(),
                depth,
            )?);
        }
        Some(origins)
    }

    // A captured pointer observes the live pointee, but its old source
    // coordinate cannot follow a replacement pointer. Reuse a current
    // slot only while every reference crossed still denotes that entry
    // pointer; otherwise retain the incoming alternatives.
    fn incoming_references_unchanged(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        location: &DescriptorLocation,
        depth: &DescriptorReadDepth<'_>,
    ) -> bool {
        let mut crossed_reference = false;
        for (index, segment) in location.path.iter().enumerate() {
            if *segment != DescriptorPathSegment::Referent {
                continue;
            }
            let prefix = DescriptorLocation {
                root: location.root,
                path: location.path[..index].to_vec(),
            };
            let expected = if crossed_reference {
                Origin::IncomingContents(
                    DescriptorContentsQuery::from_location(&prefix),
                    Projection::new(&[]),
                )
            } else {
                Origin::Incoming(prefix.clone())
            };
            if self.read_with_depth(machine, state, &prefix, depth)
                != Some(BTreeSet::from([expected]))
            {
                return false;
            }
            crossed_reference = true;
        }
        crossed_reference
    }

    pub(crate) fn contents_targets(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        query: &DescriptorContentsQuery,
    ) -> Option<BTreeSet<DescriptorPayloadTarget>> {
        self.contents_targets_with_depth(
            machine,
            state,
            query,
            &DescriptorReadDepth::at(machine, state, None),
        )
    }

    pub(crate) fn contents_targets_with_depth(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        query: &DescriptorContentsQuery,
        depth: &DescriptorReadDepth<'_>,
    ) -> Option<BTreeSet<DescriptorPayloadTarget>> {
        let _read = depth.enter()?;
        if state.contents_invalidated
            && query
                .steps
                .iter()
                .any(|step| matches!(step, DescriptorContentsStep::Element { .. }))
        {
            return None;
        }
        self.validate_query(machine, state, query, depth)?;
        if self.contents_address_hidden(machine, state, query) {
            return None;
        }
        // A shared array with an open extent supplies its elements at the call.
        // An exact index into an exclusive input also identifies the caller's
        // element. Preserve the selector in both cases.
        let prefix = query.observation();
        let supplied_by_caller = query.steps.len() > prefix.path.len()
            && match self.parameter(machine, query.root) {
                Some((_, StructuralAccess::SharedBorrow, root_type)) => {
                    crate::descriptor_lifetimes::descriptor_location_type(
                        self.types,
                        root_type,
                        &prefix.path,
                    )
                    .and_then(|ty| self.type_index.find(ty))
                    .is_some_and(|declaration| {
                        matches!(declaration.shape, StructuralTypeShape::FixedArray { length, .. }
                            if length.literal().is_none())
                    })
                }
                Some((_, terminal_psi::exclusive_borrow_pair_pattern!(), _)) => {
                    query.is_symbolic_location()
                }
                _ => false,
            };
        if supplied_by_caller && self.inline_steps(machine, query)?.iter().all(|inline| *inline) {
            return Some(BTreeSet::from([DescriptorPayloadTarget::Incoming(
                query.clone(),
            )]));
        }
        let broad = query.retains_selector().then(|| query.without_selectors());
        let query = broad.as_ref().unwrap_or(query);
        self.contents_targets_at(
            machine,
            state,
            &DescriptorLocation::whole(query.root),
            &query.steps,
            depth,
            None,
        )
    }

    /// Whether an unknown value on the way to `query`'s leaf leaves the
    /// leaf's own storage undetermined, so a store to it cannot be placed.
    ///
    /// Only a reference or view decides where the storage behind it is.
    /// An unknown strict ancestor therefore hides the leaf when the path
    /// crosses one at or below that ancestor: past it the leaf could be
    /// anywhere. A record, case payload or fixed array lays its parts out
    /// inside itself, so losing its value (a field moved out to be
    /// replaced, for example) leaves every part where it was and a store
    /// can still refill it. The leaf's own value never decides where the
    /// leaf is. Reads do not use this: they stay unknown below any unknown
    /// ancestor through `contents_path_invalidated`.
    pub(crate) fn contents_address_hidden(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        query: &DescriptorContentsQuery,
    ) -> bool {
        if state.leaf_known(query) {
            return false;
        }
        let mut unknown = state
            .unknown_prefixes(query, query.steps.len().saturating_sub(1))
            .peekable();
        if unknown.peek().is_none() {
            return false;
        }
        let Some(inline) = self.inline_steps(machine, query) else {
            return true;
        };
        unknown.any(|ancestor| {
            // An entry ending in `Referent` stands for the reference
            // crossed there, so its target is what is unknown.
            ancestor.path.last() == Some(&DescriptorPathSegment::Referent)
                || inline[ancestor.path.len()..].contains(&false)
        })
    }

    /// For each step of `query`, whether it selects storage laid out
    /// inside the value it is applied to. Access views and accessors are
    /// records that stand for storage elsewhere, like references, so their
    /// fields do not count.
    pub(crate) fn inline_steps(
        &self,
        machine: &DescriptorMachine,
        query: &DescriptorContentsQuery,
    ) -> Option<Vec<bool>> {
        query.inline_steps(self.types, *machine.place_types.get(&query.root)?)
    }

    fn contents_targets_at(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        location: &DescriptorLocation,
        steps: &[DescriptorContentsStep],
        depth: &DescriptorReadDepth<'_>,
        account: Option<(
            &DescriptorLocation,
            StructuralTypeId,
            &[DescriptorResultSlot],
        )>,
    ) -> Option<BTreeSet<DescriptorPayloadTarget>> {
        let _read = depth.enter()?;
        let prefix_length = steps
            .iter()
            .position(|step| {
                matches!(
                    step,
                    DescriptorContentsStep::Element { .. }
                        | DescriptorContentsStep::Address(DescriptorPathSegment::Referent)
                )
            })
            .unwrap_or(steps.len());
        let mut selected = location.clone();
        for step in &steps[..prefix_length] {
            let DescriptorContentsStep::Address(segment) = step else {
                return None;
            };
            selected.path.push(*segment);
        }
        let relative = match account {
            Some((root, _, _)) if root.root == selected.root => {
                selected.path.strip_prefix(root.path.as_slice())?;
                Some(root.path.len())
            }
            Some(_) => return None,
            None => None,
        };
        // Enumeration has an actual alternative's account, whereas a
        // written fixed index must be valid in every possible return.
        let resolved = match (account, relative) {
            (Some((_, ty, slots)), Some(offset)) => DescriptorResultSlot::element_path(
                self.types,
                (ty, slots),
                &selected.path[offset..],
            )?,
            _ => self.resolved_location(machine, state, &selected, depth)?,
        };
        if matches!(
            steps.get(prefix_length),
            Some(DescriptorContentsStep::Address(
                DescriptorPathSegment::Referent
            ))
        ) {
            let reference = resolved.1?;
            let StructuralTypeShape::Reference { referent, .. } =
                self.type_index.find(reference)?.shape
            else {
                return None;
            };
            let mut targets = BTreeSet::new();
            let origins = match (account, relative) {
                (Some((_, _, slots)), Some(offset)) => {
                    let path = &selected.path[offset..];
                    let origins =
                        DescriptorResultSlot::matching(slots, path)?.origins_at(path)?;
                    self.account_value(machine, state, &origins, depth)?
                }
                _ => self.read_with_depth(machine, state, &selected, depth)?,
            };
            for origin in origins {
                targets.extend(self.reference_targets(
                    machine,
                    state,
                    origin,
                    reference,
                    referent,
                    &steps[prefix_length + 1..],
                    depth,
                )?);
            }
            return Some(targets);
        }
        let Some(DescriptorContentsStep::Element { element, .. }) = steps.get(prefix_length)
        else {
            return Some(BTreeSet::from([DescriptorPayloadTarget::Storage {
                location: match account {
                    Some(_) => selected.clone(),
                    None => {
                        self.canonical_storage_with_depth(machine, state, &selected, depth)?
                    }
                },
                completed: depth
                    .captured
                    .get()
                    .or_else(|| self.location_capture(machine, &selected, depth)),
            }]));
        };
        let selected = match account {
            Some(_) => selected,
            None => self.canonical_storage_with_depth(machine, state, &selected, depth)?,
        };
        let selected_type = resolved.1?;
        let declaration = self.type_index.find(selected_type)?;
        let suffix = &steps[prefix_length + 1..];
        match declaration.shape {
            StructuralTypeShape::FixedArray {
                element: actual,
                length: terminal_psi::ArrayExtent::Literal(length),
            } if actual == *element => {
                if length > 4096 {
                    return None;
                }
                let mut origins = BTreeSet::new();
                for element_index in 0..length {
                    let mut child = selected.clone();
                    child
                        .path
                        .push(DescriptorPathSegment::FixedIndex(element_index));
                    origins.extend(
                        self.contents_targets_at(
                            machine, state, &child, suffix, depth, account,
                        )?,
                    );
                }
                Some(origins)
            }
            StructuralTypeShape::ElementView {
                element: actual, ..
            } if actual == *element => {
                let mut origins = BTreeSet::new();
                if let (Some((_, _, slots)), Some(offset)) = (account, relative) {
                    let path = &selected.path[offset..];
                    for origin in
                        DescriptorResultSlot::matching(slots, path)?.origins_at(path)?
                    {
                        origins.extend(self.stored_view_targets(
                            machine,
                            state,
                            (&selected, *element),
                            origin,
                            suffix,
                            depth,
                        )?);
                    }
                    return Some(origins);
                }
                for origin in self.read_with_depth(machine, state, &selected, depth)? {
                    origins.extend(self.view_element_targets(
                        machine, state, origin, *element, suffix, depth,
                    )?);
                }
                Some(origins)
            }
            _ => None,
        }
    }

    fn reference_targets(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        origin: Origin,
        reference: StructuralTypeId,
        referent: StructuralTypeId,
        suffix: &[DescriptorContentsStep],
        depth: &DescriptorReadDepth<'_>,
    ) -> Option<BTreeSet<DescriptorPayloadTarget>> {
        let _read = depth.enter()?;
        if let Origin::CapturedStorage(account) = &origin {
            return self.reference_targets(
                machine,
                state,
                account.singular_coordinate()?.origin(),
                reference,
                referent,
                suffix,
                depth,
            );
        }
        let state = self.live_state(machine, state, depth)?;
        let _live = depth.capture(None);
        let mut incoming = match origin {
            Origin::Storage(backing) => {
                if !self
                    .reference_backing_matches(machine, state, &backing, reference, depth)?
                {
                    return None;
                }
                return self.contents_targets_at(machine, state, &backing, suffix, depth, None);
            }
            Origin::Incoming(slot) => DescriptorContentsQuery::from_location(&slot),
            Origin::IncomingContents(query, projection)
                if projection == Projection::new(&[]) =>
            {
                query
            }
            Origin::Backing(DescriptorCallOrigin::Argument { source, projection })
                if projection.exact
                    && !projection.indirect
                    && projection.window.is_none()
                    && projection.captured_at.is_none() =>
            {
                let root = *machine.place_types.get(&source)?;
                let backing =
                    self.type_index.descriptor_location(source, root, &projection.path, None)?;
                if !self
                    .reference_backing_matches(machine, state, &backing, reference, depth)?
                {
                    return None;
                }
                return self.contents_targets_at(machine, state, &backing, suffix, depth, None);
            }
            _ => return None,
        };
        if state.contents_invalidated {
            return None;
        }
        incoming.steps.push(DescriptorContentsStep::Address(
            DescriptorPathSegment::Referent,
        ));
        self.incoming_targets_at(machine, state, incoming, referent, suffix, depth)
    }

    fn incoming_targets_at(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        mut query: DescriptorContentsQuery,
        carrier: StructuralTypeId,
        steps: &[DescriptorContentsStep],
        depth: &DescriptorReadDepth<'_>,
    ) -> Option<BTreeSet<DescriptorPayloadTarget>> {
        let _read = depth.enter()?;
        if state.contents_invalidated {
            return None;
        }
        let length = steps
            .iter()
            .position(|step| {
                matches!(
                    step,
                    DescriptorContentsStep::Element { .. }
                        | DescriptorContentsStep::Address(DescriptorPathSegment::Referent)
                )
            })
            .unwrap_or(steps.len());
        let mut path = Vec::new();
        for step in &steps[..length] {
            let DescriptorContentsStep::Address(segment) = step else {
                return None;
            };
            path.push(*segment);
        }
        query.steps.extend_from_slice(&steps[..length]);
        query.validate(self.types, self.parameter(machine, query.root)?.2)?;
        let Some(boundary) = steps.get(length) else {
            return Some(BTreeSet::from([DescriptorPayloadTarget::Incoming(query)]));
        };
        let selected = crate::descriptors::descriptor_lifetimes::descriptor_location_type(
            self.types, carrier, &path,
        )?;
        let mut targets = BTreeSet::new();
        match boundary {
            DescriptorContentsStep::Address(DescriptorPathSegment::Referent) => {
                let StructuralTypeShape::Reference { referent, .. } =
                    self.type_index.find(selected)?.shape
                else {
                    return None;
                };
                for origin in self.read_incoming_contents(machine, state, &query, depth)? {
                    targets.extend(self.reference_targets(
                        machine,
                        state,
                        origin,
                        selected,
                        referent,
                        &steps[length + 1..],
                        depth,
                    )?);
                }
            }
            DescriptorContentsStep::Element { element, .. } => {
                match self.type_index.find(selected)?.shape {
                    StructuralTypeShape::FixedArray {
                        element: actual,
                        length: terminal_psi::ArrayExtent::Literal(count),
                    } if actual == *element && count <= 4096 => {
                        for index in 0..count {
                            let mut child = query.clone();
                            child.steps.push(DescriptorContentsStep::Address(
                                DescriptorPathSegment::FixedIndex(index),
                            ));
                            targets.extend(self.incoming_targets_at(
                                machine,
                                state,
                                child,
                                *element,
                                &steps[length + 1..],
                                depth,
                            )?);
                        }
                    }
                    StructuralTypeShape::ElementView {
                        element: actual, ..
                    } if actual == *element => {
                        for origin in
                            self.read_incoming_contents(machine, state, &query, depth)?
                        {
                            targets.extend(self.view_element_targets(
                                machine,
                                state,
                                origin,
                                *element,
                                &steps[length + 1..],
                                depth,
                            )?);
                        }
                    }
                    _ => return None,
                }
            }
            _ => return None,
        }
        Some(targets)
    }

    fn account_value(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        origins: &[&DescriptorStoredOrigin],
        depth: &DescriptorReadDepth<'_>,
    ) -> Value {
        let (owner, (block, completed)) = depth.captured.get()?;
        if owner != machine.id {
            return None;
        }
        let position = (block, completed.checked_sub(1)?);
        let call = machine.direct_call(position)?;
        let before = self.target_state(machine, state, Some((owner, position)), depth)?;
        let reads =
            DescriptorReadDepth::at(machine, before, depth.history.map(|(_, states)| states));
        let value = (|| {
            let mut value = BTreeSet::new();
            for origin in origins {
                value.extend(
                    self.substitute_result_with_depth(machine, before, call, origin, &reads)?,
                );
            }
            (!value.is_empty()).then_some(value)
        })();
        // Failed substitutions still consulted the child read context.
        depth.absorb_summary_reads(&reads);
        value
    }

    fn stored_view_targets(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        source: (&DescriptorLocation, StructuralTypeId),
        origin: &DescriptorStoredOrigin,
        suffix: &[DescriptorContentsStep],
        depth: &DescriptorReadDepth<'_>,
    ) -> Option<BTreeSet<DescriptorPayloadTarget>> {
        let _read = depth.enter()?;
        match origin {
            DescriptorStoredOrigin::Empty => Some(BTreeSet::new()),
            DescriptorStoredOrigin::Carried { element, elements } if *element == source.1 => {
                let mut targets = BTreeSet::new();
                for (ordinal, slots) in elements.iter().enumerate() {
                    let mut child = source.0.clone();
                    child
                        .path
                        .push(DescriptorPathSegment::FixedIndex(ordinal as u64));
                    // This enumeration has the selected alternative's
                    // actual element account. A direct fixed index still
                    // needs the ordinal in every possible alternative.
                    targets.extend(self.contents_targets_at(
                        machine,
                        state,
                        &child,
                        suffix,
                        depth,
                        Some((&child, *element, slots)),
                    )?);
                }
                Some(targets)
            }
            DescriptorStoredOrigin::Carried { .. } => None,
            _ => {
                let mut targets = BTreeSet::new();
                for origin in self.account_value(machine, state, &[origin], depth)? {
                    targets.extend(self.view_element_targets(
                        machine, state, origin, source.1, suffix, depth,
                    )?);
                }
                Some(targets)
            }
        }
    }

    fn view_element_targets(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        origin: Origin,
        element: StructuralTypeId,
        suffix: &[DescriptorContentsStep],
        depth: &DescriptorReadDepth<'_>,
    ) -> Option<BTreeSet<DescriptorPayloadTarget>> {
        let _read = depth.enter()?;
        if let Origin::CapturedStorage(account) = &origin {
            return self.view_element_targets(
                machine,
                state,
                account.singular_coordinate()?.origin(),
                element,
                suffix,
                depth,
            );
        }
        let mut steps = vec![DescriptorContentsStep::any_element(element)];
        steps.extend_from_slice(suffix);
        let incoming = match origin {
            Origin::Backing(DescriptorCallOrigin::Empty) => return Some(BTreeSet::new()),
            Origin::Storage(location) => {
                if location.path.is_empty()
                    && let Some((producer, actual, step)) = machine.sequence_construction(
                        self.types,
                        location.root,
                        depth.captured.get(),
                    )
                {
                    if actual != element {
                        return None;
                    }
                    let completed = (producer.0, producer.1.checked_add(1)?);
                    let owner = (machine.id, completed);
                    let captured = self.target_state(machine, state, Some(owner), depth)?;
                    let _capture = depth.capture(Some(owner));
                    let mut targets = BTreeSet::new();
                    for (_, destination) in &step.derivations {
                        targets.extend(self.contents_targets_at(
                            machine,
                            captured,
                            destination,
                            suffix,
                            depth,
                            None,
                        )?);
                    }
                    return Some(targets);
                }
                if let Some((producer, origins)) = self.returned_roster_origins(
                    machine,
                    &location,
                    depth.captured.get(),
                    Some(depth),
                ) {
                    let owner = (machine.id, (producer.0, producer.1.checked_add(1)?));
                    let captured = self.target_state(machine, state, Some(owner), depth)?;
                    let _capture = depth.capture(Some(owner));
                    let mut targets = BTreeSet::new();
                    for origin in origins {
                        targets.extend(self.stored_view_targets(
                            machine,
                            captured,
                            (&location, element),
                            origin,
                            suffix,
                            depth,
                        )?);
                    }
                    return Some(targets);
                }
                let state = self.live_state(machine, state, depth)?;
                let _live = depth.capture(None);
                return self
                    .contents_targets_at(machine, state, &location, &steps, depth, None);
            }
            Origin::Incoming(location) => {
                Some(DescriptorContentsQuery::from_location(&location))
            }
            Origin::IncomingContents(query, projection)
                if projection == Projection::new(&[]) =>
            {
                Some(query)
            }
            Origin::IncomingContents(..) | Origin::CapturedStorage(_) => return None,
            Origin::Backing(DescriptorCallOrigin::Argument { source, projection }) => {
                if projection.indirect {
                    return None;
                }
                let root_type = *machine.place_types.get(&source)?;
                if is_descriptor(self.types.iter(), root_type)
                    && projection.path.is_empty()
                    && self.parameter(machine, source).is_some()
                {
                    Some(DescriptorContentsQuery::from_location(
                        &DescriptorLocation::whole(source),
                    ))
                } else {
                    let path = match projection.path.split_last() {
                        Some((StructuralPathSegment::FixedRange { .. }, prefix)) => prefix,
                        _ => projection.path.as_slice(),
                    };
                    let mut backing =
                        descriptor_contents_query(self.types, source, root_type, path, None)?;
                    backing.steps.extend(steps);
                    backing.validate(self.types, root_type)?;
                    return self.contents_targets_at(
                        machine,
                        state,
                        &DescriptorLocation::whole(source),
                        &backing.steps,
                        depth,
                        None,
                    );
                }
            }
            _ => return None,
        };
        let mut incoming = incoming?;
        incoming
            .steps
            .push(DescriptorContentsStep::any_element(element));
        let state = self.live_state(machine, state, depth)?;
        let _live = depth.capture(None);
        self.incoming_targets_at(machine, state, incoming, element, suffix, depth)
    }

    pub(crate) fn transfer_contents(
        &self,
        machine: &DescriptorMachine,
        before: &State,
        after: &mut State,
        transfers: &[DescriptorContentsTransfer],
        derivations: &[(DescriptorLocation, DescriptorLocation)],
        reads: &DescriptorReadDepth<'_>,
    ) {
        let mut replacements = BTreeMap::new();
        for transfer in transfers {
            if transfer.removes_source
                && self
                    .contents_targets_with_depth(machine, before, &transfer.source, reads)
                    .is_none()
            {
                after.contents_invalidated = true;
            }
            let incoming =
                self.read_contents_with_depth(machine, before, &transfer.source, reads);
            if self
                .contents_replacements_with_depth(
                    machine,
                    before,
                    &transfer.destination,
                    incoming,
                    &mut replacements,
                    reads,
                )
                .is_none()
            {
                after.contents_invalidated = true;
            }
        }
        // A matching value transfer already installed the root and its
        // descendant facts. Installing that same root again would erase
        // those facts. Omit only proven redundant replacements; other
        // writes retain the ordinary descendant-clearing behavior.
        let mut derivations = derivations.iter().collect::<Vec<_>>();
        derivations.sort_unstable();
        for transfer in transfers {
            let (Some(source), Some(destination)) = (
                transfer.source.exact_location(),
                transfer.destination.exact_location(),
            ) else {
                continue;
            };
            if derivations
                .binary_search_by(|actual| {
                    actual
                        .0
                        .cmp(&source)
                        .then_with(|| actual.1.cmp(&destination))
                })
                .is_err()
            {
                continue;
            }
            let target = DescriptorPayloadTarget::current(destination.clone());
            let Some(Some(incoming)) = replacements.get(&target) else {
                continue;
            };
            if !incoming.is_empty()
                && after.slots.get(&destination).and_then(Option::as_ref) == Some(incoming)
            {
                replacements.remove(&target);
            }
        }
        Self::install_contents(after, replacements);
    }

    pub(crate) fn contents_replacements(
        &self,
        machine: &DescriptorMachine,
        before: &State,
        query: &DescriptorContentsQuery,
        incoming: Value,
        replacements: &mut BTreeMap<DescriptorPayloadTarget, Value>,
    ) -> Option<()> {
        self.contents_replacements_with_depth(
            machine,
            before,
            query,
            incoming,
            replacements,
            &DescriptorReadDepth::at(machine, before, None),
        )
    }

    pub(crate) fn contents_replacements_with_depth(
        &self,
        machine: &DescriptorMachine,
        before: &State,
        query: &DescriptorContentsQuery,
        incoming: Value,
        replacements: &mut BTreeMap<DescriptorPayloadTarget, Value>,
        reads: &DescriptorReadDepth<'_>,
    ) -> Option<()> {
        let targets = self.contents_targets_with_depth(machine, before, query, reads)?;
        if targets.iter().any(|target| {
            matches!(
                target,
                DescriptorPayloadTarget::Storage {
                    completed: Some(_),
                    ..
                }
            )
        }) {
            return None;
        }
        if targets.is_empty() {
            return None;
        }
        let strong = targets.len() == 1
            && targets.first().is_some_and(|target| match target {
                DescriptorPayloadTarget::Storage { completed, .. } => completed.is_none(),
                DescriptorPayloadTarget::Incoming(query) => query.is_symbolic_location(),
            });
        for target in targets {
            let value = if strong {
                incoming.clone()
            } else {
                let previous = match &target {
                    DescriptorPayloadTarget::Storage {
                        location,
                        completed: None,
                    } => self.read_with_depth(machine, before, location, reads),
                    DescriptorPayloadTarget::Storage {
                        completed: Some(_), ..
                    } => return None,
                    DescriptorPayloadTarget::Incoming(query) => before.incoming_contents(query),
                };
                match (previous, incoming.as_ref()) {
                    (Some(mut previous), Some(incoming)) => {
                        previous.extend(incoming.iter().cloned());
                        Some(previous)
                    }
                    _ => None,
                }
            };
            match replacements.get_mut(&target) {
                None => {
                    replacements.insert(target, value);
                }
                Some(existing) => match (existing.as_mut(), value) {
                    (Some(existing), Some(value)) => existing.extend(value),
                    _ => *existing = None,
                },
            }
        }
        Some(())
    }

    pub(crate) fn install_contents(
        state: &mut State,
        replacements: BTreeMap<DescriptorPayloadTarget, Value>,
    ) {
        for (target, value) in replacements {
            match target {
                DescriptorPayloadTarget::Storage {
                    location,
                    completed: None,
                } => {
                    state
                        .slots_mut()
                        .retain(|stored, _| stored == &location || !location.contains(stored));
                    state.slots_mut().insert(location, value);
                }
                DescriptorPayloadTarget::Storage {
                    completed: Some(_), ..
                } => {
                    state.contents_invalidated = true;
                }
                DescriptorPayloadTarget::Incoming(query) => {
                    state.contents_mut().insert(query, value);
                }
            }
        }
    }

    pub(crate) fn actual_contents(
        &self,
        machine: &DescriptorMachine,
        call: &DescriptorInvocation,
        parameter: usize,
        steps: &[DescriptorContentsStep],
    ) -> Option<DescriptorContentsQuery> {
        let actuals = call.actual_inputs();
        let query =
            DescriptorContentsQuery::from_location(actuals.arguments.get(parameter)?.as_ref()?);
        self.extend_actual_contents(machine, actuals, query, steps)
    }

    pub(crate) fn extend_actual_contents(
        &self,
        machine: &DescriptorMachine,
        actuals: DescriptorActualInputs<'_>,
        mut query: DescriptorContentsQuery,
        steps: &[DescriptorContentsStep],
    ) -> Option<DescriptorContentsQuery> {
        if !steps.iter().any(|step| {
            matches!(
                step,
                DescriptorContentsStep::Element {
                    selector: Some(_),
                    ..
                }
            )
        }) {
            query.steps.extend_from_slice(steps);
            return Some(query);
        }
        let mut carrier = *machine.place_types.get(&query.root)?;
        let mut path = Vec::new();
        for step in &query.steps {
            match step {
                DescriptorContentsStep::Address(segment) => path.push(*segment),
                DescriptorContentsStep::Element { element, .. } => {
                    carrier = *element;
                    path.clear();
                }
            }
        }
        for step in steps {
            match step {
                DescriptorContentsStep::Address(segment) => {
                    path.push(*segment);
                    query.steps.push(step.clone());
                }
                DescriptorContentsStep::Element { element, selector } => {
                    let selected = crate::descriptor_lifetimes::descriptor_location_type(
                        self.types, carrier, &path,
                    )?;
                    let shape = &self.type_index.find(selected)?.shape;
                    let term = selector.as_ref().and_then(|selector| {
                        crate::substitution::substitute_bound_scalar_term(
                            selector,
                            actuals.scalar_arguments,
                        )
                    });
                    let index = super::inline_selected_index(shape, term.as_ref());
                    query.steps.push(match index {
                        Some(index) => DescriptorContentsStep::Address(
                            DescriptorPathSegment::FixedIndex(index),
                        ),
                        None => DescriptorContentsStep::Element {
                            element: *element,
                            selector: term
                                .filter(|term| term.integer_value().is_none())
                                .map(Box::new),
                        },
                    });
                    carrier = *element;
                    path.clear();
                }
            }
        }
        Some(query)
    }
}
