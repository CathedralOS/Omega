//! Resolve descriptor leaves through a possibly runtime-selected collection.
//! Reads and writes share the same exhaustive typed traversal. Incoming contents
//! stay symbolic until a caller supplies backing; unresolved candidates cannot
//! be discarded just because another candidate has a usable address.
use super::engine::{DescriptorReadDepth, Origin, State, StorageResolution, Value};
use super::*;
use crate::descriptors::descriptor_lifetimes::descriptor_location;

impl Replay<'_> {
    pub(super) fn read_contents(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        query: &DescriptorContentsQuery,
    ) -> Value {
        self.read_contents_with_depth(machine, state, query, &DescriptorReadDepth::default())
    }

    pub(super) fn read_contents_with_depth(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        query: &DescriptorContentsQuery,
        depth: &DescriptorReadDepth,
    ) -> Value {
        let _read = depth.enter()?;
        query.validate(self.types, *machine.place_types.get(&query.root)?)?;
        // Access provenance can cross a reference without supplying a physical
        // address. Retain both completed formations and symbolic ingress tokens:
        // an edge inside a helper cannot resolve its callers yet. Admission
        // later substitutes every caller and requires actual formations; an
        // incoming token is not itself backing authority. Unresolved pointer
        // contents still use the invalidation checks below.
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
        // A referent step names the borrowed carrier, not the pointee value.
        // The pointee slot is often unknown; the pointer still holds every
        // current backing origin, including one selected across branches.
        if self.slot_kind.tracks_references()
            && let Some(location) = query.exact_location()
            && let Some(carrier) = self.reference_referent_carrier(machine, &location)?
        {
            return self.read_with_depth(machine, state, &carrier, depth);
        }
        // A unique address reads that leaf. Branch-selected storage has no
        // single address; the slot still holds every current backing origin.
        // Contents targets cannot name one of those addresses, so dropping
        // the slot would erase a return that already recorded both.
        if self.slot_kind.tracks_references()
            && let Some(queried) = query.exact_location()
        {
            match self.resolve_storage(machine, state, &queried) {
                Some(StorageResolution::Unique(location)) => {
                    if let Some(value) = state.slots.get(&location) {
                        // An established current leaf overrides its old entry
                        // family. This reads payload only; contents_targets
                        // still owns storage destinations and must never
                        // mistake this pointer for its slot.
                        return value.clone();
                    }
                }
                Some(StorageResolution::Alternatives) => {
                    if let Some(value) = state.slots.get(&queried) {
                        return value.clone();
                    }
                }
                None => {}
            }
        }
        // Target resolution tolerates a vacant leaf so a store can refill it;
        // a read of that leaf has no value to report.
        if state.contents_path_invalidated(query) {
            return None;
        }
        let mut origins = BTreeSet::new();
        for target in self.contents_targets_with_depth(machine, state, query, depth)? {
            match target {
                DescriptorPayloadTarget::Storage(location) => {
                    origins.extend(self.read_with_depth(machine, state, &location, depth)?);
                }
                DescriptorPayloadTarget::Incoming(query) => {
                    origins.extend(state.incoming_contents(&query)?);
                }
            }
        }
        Some(origins)
    }

    pub(super) fn contents_targets(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        query: &DescriptorContentsQuery,
    ) -> Option<BTreeSet<DescriptorPayloadTarget>> {
        self.contents_targets_with_depth(machine, state, query, &DescriptorReadDepth::default())
    }

    fn contents_targets_with_depth(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        query: &DescriptorContentsQuery,
        depth: &DescriptorReadDepth,
    ) -> Option<BTreeSet<DescriptorPayloadTarget>> {
        let _read = depth.enter()?;
        if state.contents_invalidated
            && query
                .steps
                .iter()
                .any(|step| matches!(step, DescriptorContentsStep::AnyElement { .. }))
        {
            return None;
        }
        query.validate(self.types, *machine.place_types.get(&query.root)?)?;
        // This is the current queried carrier, before any pointer identity is
        // translated into an entry-family namespace. A real invalidation of
        // an ancestor cannot be bypassed by taking the contents-query route.
        if state.contents_address_invalidated(query) {
            return None;
        }
        self.contents_targets_at(
            machine,
            state,
            &DescriptorLocation::whole(query.root),
            &query.steps,
            depth,
        )
    }

    fn contents_targets_at(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        location: &DescriptorLocation,
        steps: &[DescriptorContentsStep],
        depth: &DescriptorReadDepth,
    ) -> Option<BTreeSet<DescriptorPayloadTarget>> {
        let _read = depth.enter()?;
        let prefix_length = steps
            .iter()
            .position(|step| {
                matches!(
                    step,
                    DescriptorContentsStep::AnyElement { .. }
                        | DescriptorContentsStep::Address(DescriptorPathSegment::Referent)
                )
            })
            .unwrap_or(steps.len());
        let mut selected = location.clone();
        for step in &steps[..prefix_length] {
            let DescriptorContentsStep::Address(segment) = step else {
                return None;
            };
            selected.path.push(segment.clone());
        }
        if matches!(
            steps.get(prefix_length),
            Some(DescriptorContentsStep::Address(
                DescriptorPathSegment::Referent
            ))
        ) {
            let reference = crate::descriptors::descriptor_lifetimes::descriptor_location_type(
                self.types,
                *machine.place_types.get(&selected.root)?,
                &selected.path,
            )?;
            let StructuralTypeShape::Reference { referent, .. } =
                find_structural_type(self.types, reference)?.shape
            else {
                return None;
            };
            let mut targets = BTreeSet::new();
            for origin in self.read_with_depth(machine, state, &selected, depth)? {
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
        let Some(DescriptorContentsStep::AnyElement { element }) = steps.get(prefix_length) else {
            return Some(BTreeSet::from([DescriptorPayloadTarget::Storage(
                self.canonical_storage(machine, state, &selected)?,
            )]));
        };
        let selected = self.canonical_storage(machine, state, &selected)?;
        let selected_type = crate::descriptors::descriptor_lifetimes::descriptor_location_type(
            self.types,
            *machine.place_types.get(&selected.root)?,
            &selected.path,
        )?;
        let declaration = find_structural_type(self.types, selected_type)?;
        let suffix = &steps[prefix_length + 1..];
        match declaration.shape {
            StructuralTypeShape::FixedArray {
                element: actual,
                length: terminal_psi::ArrayExtent::Literal(length),
            } if actual == *element => {
                // Match the finite descriptor inventory's analysis bound.
                if length > 4096 {
                    return None;
                }
                let mut origins = BTreeSet::new();
                for element_index in 0..length {
                    let mut child = selected.clone();
                    child
                        .path
                        .push(DescriptorPathSegment::FixedIndex(element_index));
                    origins
                        .extend(self.contents_targets_at(machine, state, &child, suffix, depth)?);
                }
                Some(origins)
            }
            StructuralTypeShape::ElementView {
                element: actual, ..
            } if actual == *element => {
                let mut origins = BTreeSet::new();
                for origin in self.read_with_depth(machine, state, &selected, depth)? {
                    origins.extend(
                        self.view_element_targets(machine, state, origin, *element, suffix, depth)?,
                    );
                }
                Some(origins)
            }
            _ => None,
        }
    }

    /// Cross the pointer value actually retained by the prefix. An incoming
    /// value is an entry identity, never a request to reload its former slot.
    fn reference_targets(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        origin: Origin,
        reference: StructuralTypeId,
        referent: StructuralTypeId,
        suffix: &[DescriptorContentsStep],
        depth: &DescriptorReadDepth,
    ) -> Option<BTreeSet<DescriptorPayloadTarget>> {
        let _read = depth.enter()?;
        let mut incoming = match origin {
            Origin::Storage(backing) => {
                if !self.reference_backing_matches(machine, &backing, reference)? {
                    return None;
                }
                return self.contents_targets_at(machine, state, &backing, suffix, depth);
            }
            Origin::Incoming(slot) => DescriptorContentsQuery::from_location(&slot),
            Origin::IncomingContents(query, projection) if projection == Projection::new(&[]) => {
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
                    descriptor_location(self.types, source, root, &projection.path, None)?;
                if !self.reference_backing_matches(machine, &backing, reference)? {
                    return None;
                }
                return self.contents_targets_at(machine, state, &backing, suffix, depth);
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

    /// An entry-pointer family has no local carrier address. Walk its current
    /// stored descriptors one boundary at a time so a replaced intermediate
    /// pointer is neither reread as its old entry value nor silently omitted.
    fn incoming_targets_at(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        mut query: DescriptorContentsQuery,
        carrier: StructuralTypeId,
        steps: &[DescriptorContentsStep],
        depth: &DescriptorReadDepth,
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
                    DescriptorContentsStep::AnyElement { .. }
                        | DescriptorContentsStep::Address(DescriptorPathSegment::Referent)
                )
            })
            .unwrap_or(steps.len());
        let mut path = Vec::new();
        for step in &steps[..length] {
            let DescriptorContentsStep::Address(segment) = step else {
                return None;
            };
            path.push(segment.clone());
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
                    find_structural_type(self.types, selected)?.shape
                else {
                    return None;
                };
                for origin in state.incoming_contents(&query)? {
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
            DescriptorContentsStep::AnyElement { element } => {
                match find_structural_type(self.types, selected)?.shape {
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
                        for origin in state.incoming_contents(&query)? {
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

    fn view_element_targets(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        origin: Origin,
        element: StructuralTypeId,
        suffix: &[DescriptorContentsStep],
        depth: &DescriptorReadDepth,
    ) -> Option<BTreeSet<DescriptorPayloadTarget>> {
        let _read = depth.enter()?;
        let mut steps = vec![DescriptorContentsStep::AnyElement { element }];
        steps.extend_from_slice(suffix);
        let incoming = match origin {
            Origin::Backing(DescriptorCallOrigin::Empty) => return Some(BTreeSet::new()),
            // A view established over inline parameter storage retains that
            // storage origin. Follow its typed elements just as for a captured
            // backing argument; the descriptor is not an unknown payload.
            Origin::Storage(location) => {
                return self.contents_targets_at(machine, state, &location, &steps, depth);
            }
            Origin::Incoming(location) => Some(DescriptorContentsQuery::from_location(&location)),
            Origin::IncomingContents(query, projection) if projection == Projection::new(&[]) => {
                Some(query)
            }
            Origin::IncomingContents(..) => return None,
            Origin::Backing(DescriptorCallOrigin::Argument { source, projection }) => {
                if projection.indirect {
                    return None;
                }
                let root_type = *machine.place_types.get(&source)?;
                // A current incoming view refers to caller-owned elements,
                // not to a fictitious inline array in its carrier.
                if is_descriptor(self.types.iter(), root_type)
                    && projection.path.is_empty()
                    && self.parameter(machine, source).is_some()
                {
                    Some(DescriptorContentsQuery::from_location(
                        &DescriptorLocation::whole(source),
                    ))
                } else {
                    // Window coordinates may refine a captured region, but are
                    // unnecessary for a conservative union of its inline slots.
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
                    );
                }
            }
            // An empty view cannot provide an element. Literal primitive bytes
            // and carrier addresses do not describe stored descriptor payloads.
            _ => return None,
        };
        let mut incoming = incoming?;
        incoming
            .steps
            .push(DescriptorContentsStep::AnyElement { element });
        self.incoming_targets_at(machine, state, incoming, element, suffix, depth)
    }

    /// Payload provenance only. A weak update retains all old alternatives;
    /// it neither restores move debt nor authorizes a read of vacant storage.
    /// Resolve and read from `before`, including overlapping transfers, before
    /// publishing any replacements to the successor state.
    pub(super) fn transfer_contents(
        &self,
        machine: &DescriptorMachine,
        before: &State,
        after: &mut State,
        transfers: &[DescriptorContentsTransfer],
    ) {
        let mut replacements = BTreeMap::new();
        for transfer in transfers {
            if transfer.removes_source
                && self
                    .contents_targets(machine, before, &transfer.source)
                    .is_none()
            {
                after.contents_invalidated = true;
            }
            let incoming = self.read_contents(machine, before, &transfer.source);
            if self
                .contents_replacements(
                    machine,
                    before,
                    &transfer.destination,
                    incoming,
                    &mut replacements,
                )
                .is_none()
            {
                after.contents_invalidated = true;
            }
        }
        Self::install_contents(after, replacements);
    }

    /// A singleton symbolic family still contains an unknown number of slots.
    /// Only an exhaustively resolved single storage location permits replacement.
    /// All right-hand sides and previous alternatives come from the same snapshot.
    pub(super) fn contents_replacements(
        &self,
        machine: &DescriptorMachine,
        before: &State,
        query: &DescriptorContentsQuery,
        incoming: Value,
        replacements: &mut BTreeMap<DescriptorPayloadTarget, Value>,
    ) -> Option<()> {
        let targets = self.contents_targets(machine, before, query)?;
        if targets.is_empty() {
            return None;
        }
        let strong = targets.len() == 1
            && matches!(targets.first(), Some(DescriptorPayloadTarget::Storage(_)));
        for target in targets {
            let value = if strong {
                incoming.clone()
            } else {
                let previous = match &target {
                    DescriptorPayloadTarget::Storage(location) => {
                        self.read(machine, before, location)
                    }
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

    pub(super) fn install_contents(
        state: &mut State,
        replacements: BTreeMap<DescriptorPayloadTarget, Value>,
    ) {
        for (target, value) in replacements {
            match target {
                DescriptorPayloadTarget::Storage(location) => {
                    state
                        .slots_mut()
                        .retain(|stored, _| stored == &location || !location.contains(stored));
                    state.slots_mut().insert(location, value);
                }
                DescriptorPayloadTarget::Incoming(query) => {
                    state.contents_mut().insert(query, value);
                }
            }
        }
    }

    pub(super) fn actual_contents(
        &self,
        call: &DescriptorInvocation,
        parameter: usize,
        steps: &[DescriptorContentsStep],
    ) -> Option<DescriptorContentsQuery> {
        let mut query =
            DescriptorContentsQuery::from_location(call.arguments.get(parameter)?.as_ref()?);
        query.steps.extend_from_slice(steps);
        Some(query)
    }
}
