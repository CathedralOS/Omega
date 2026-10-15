//! Resolve descriptor leaves through a possibly runtime-selected collection.
//! Reads and writes share the same exhaustive typed traversal. Incoming contents
//! stay symbolic until a caller supplies backing; unresolved candidates cannot
//! be discarded just because another candidate has a usable address.
use super::replay::{Origin, State, Value};
use super::*;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum ContentsTarget {
    Storage(DescriptorLocation),
    Incoming(DescriptorContentsQuery),
}

impl Replay<'_> {
    pub(super) fn read_contents(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        query: &DescriptorContentsQuery,
    ) -> Value {
        let mut origins = BTreeSet::new();
        for target in self.contents_targets(machine, state, query)? {
            match target {
                ContentsTarget::Storage(location) => {
                    origins.extend(self.read(machine, state, &location)?);
                }
                ContentsTarget::Incoming(query) => {
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
    ) -> Option<BTreeSet<ContentsTarget>> {
        if state.contents_invalidated
            && query
                .steps
                .iter()
                .any(|step| matches!(step, DescriptorContentsStep::AnyElement { .. }))
        {
            return None;
        }
        query.validate(self.types, *machine.place_types.get(&query.root)?)?;
        self.contents_targets_at(
            machine,
            state,
            &DescriptorLocation::whole(query.root),
            &query.steps,
            0,
        )
    }

    fn contents_targets_at(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        location: &DescriptorLocation,
        steps: &[DescriptorContentsStep],
        depth: usize,
    ) -> Option<BTreeSet<ContentsTarget>> {
        if depth >= 128 {
            return None;
        }
        let prefix_length = steps
            .iter()
            .position(|step| matches!(step, DescriptorContentsStep::AnyElement { .. }))
            .unwrap_or(steps.len());
        let mut selected = location.clone();
        for step in &steps[..prefix_length] {
            let DescriptorContentsStep::Address(segment) = step else {
                return None;
            };
            selected.path.push(segment.clone());
        }
        let Some(DescriptorContentsStep::AnyElement { element }) = steps.get(prefix_length) else {
            return Some(BTreeSet::from([ContentsTarget::Storage(
                self.canonical_storage(machine, state, &selected)?,
            )]));
        };
        let selected = self.canonical_storage(machine, state, &selected)?;
        let selected_type = crate::descriptor_lifetimes::descriptor_location_type(
            self.types,
            *machine.place_types.get(&selected.root)?,
            &selected.path,
        )?;
        let declaration = find_structural_type(self.types, selected_type)?;
        let suffix = &steps[prefix_length + 1..];
        match declaration.shape {
            StructuralTypeShape::FixedArray {
                element: actual,
                length,
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
                    origins.extend(self.contents_targets_at(
                        machine,
                        state,
                        &child,
                        suffix,
                        depth + 1,
                    )?);
                }
                Some(origins)
            }
            StructuralTypeShape::ElementView {
                element: actual, ..
            } if actual == *element => {
                let mut origins = BTreeSet::new();
                for origin in self.read(machine, state, &selected)? {
                    origins.extend(self.view_element_targets(
                        machine,
                        state,
                        origin,
                        *element,
                        suffix,
                        depth + 1,
                    )?);
                }
                Some(origins)
            }
            _ => None,
        }
    }

    fn view_element_targets(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        origin: Origin,
        element: StructuralTypeId,
        suffix: &[DescriptorContentsStep],
        depth: usize,
    ) -> Option<BTreeSet<ContentsTarget>> {
        let mut steps = vec![DescriptorContentsStep::AnyElement { element }];
        steps.extend_from_slice(suffix);
        let incoming = match origin {
            Origin::Incoming(location) => Some(DescriptorContentsQuery::from_location(&location)),
            Origin::IncomingContents(query) => Some(query),
            Origin::Backing(DescriptorCallOrigin::Argument { source, projection }) => {
                if projection.indirect {
                    return None;
                }
                let root_type = *machine.place_types.get(&source)?;
                // A current incoming view refers to caller-owned elements,
                // not to a fictitious inline array in its carrier.
                if is_shared_descriptor(self.types.iter(), root_type)
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
                        Some((StructuralPathSegment::FixedByteRange { .. }, prefix)) => prefix,
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
                        depth + 1,
                    );
                }
            }
            // An empty view cannot provide an element. Literal primitive bytes
            // and carrier addresses do not describe stored descriptor payloads.
            _ => return None,
        };
        let mut incoming = incoming?;
        incoming.steps.extend(steps);
        let (_, _, root_type) = self.parameter(machine, incoming.root)?;
        incoming.validate(self.types, root_type)?;
        Some(BTreeSet::from([ContentsTarget::Incoming(incoming)]))
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
        replacements: &mut BTreeMap<ContentsTarget, Value>,
    ) -> Option<()> {
        let targets = self.contents_targets(machine, before, query)?;
        if targets.is_empty() {
            return None;
        }
        let strong =
            targets.len() == 1 && matches!(targets.first(), Some(ContentsTarget::Storage(_)));
        for target in targets {
            let value = if strong {
                incoming.clone()
            } else {
                let previous = match &target {
                    ContentsTarget::Storage(location) => self.read(machine, before, location),
                    ContentsTarget::Incoming(query) => before.incoming_contents(query),
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
        replacements: BTreeMap<ContentsTarget, Value>,
    ) {
        for (target, value) in replacements {
            match target {
                ContentsTarget::Storage(location) => {
                    state
                        .slots
                        .retain(|stored, _| stored == &location || !location.contains(stored));
                    state.slots.insert(location, value);
                }
                ContentsTarget::Incoming(query) => {
                    state.contents.insert(query, value);
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
