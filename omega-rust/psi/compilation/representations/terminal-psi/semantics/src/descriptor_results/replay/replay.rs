use super::*;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Origin {
    Backing(DescriptorCallOrigin),
    Incoming(DescriptorLocation),
    IncomingContents(DescriptorContentsQuery),
    Storage(DescriptorLocation),
}
pub(super) type Value = Option<BTreeSet<Origin>>;
pub(super) type Slots = BTreeMap<DescriptorLocation, Value>;
#[derive(Clone, Default, PartialEq, Eq)]
pub(super) struct State {
    pub(super) slots: Slots,
    // Keys name incoming backing families, not addresses in local carriers.
    // Values are captured origins; reading them never replays a later write.
    pub(super) contents: BTreeMap<DescriptorContentsQuery, Value>,
    // Captured values remain valid; only later reads of untracked backing
    // contents become unavailable. The flag joins monotonically across edges.
    pub(super) contents_invalidated: bool,
}
pub(super) type States = BTreeMap<DescriptorPosition, State>;

#[derive(Clone)]
pub(super) struct ResultSlot {
    pub(super) path: Vec<DescriptorPathSegment>,
    pub(super) origins: Vec<DescriptorStoredOrigin>,
}
#[derive(Clone)]
pub(super) struct Summary {
    pub(super) result: Option<Vec<ResultSlot>>,
    pub(super) effects: Vec<DescriptorStoredEffect>,
    pub(super) invalidates_contents: bool,
}
pub(super) struct Replay<'a> {
    pub(super) types: &'a [StructuralTypeDeclaration],
    pub(super) slot_kind: DescriptorSlotKind,
    pub(super) machines: &'a [DescriptorMachine],
    pub(super) summaries: BTreeMap<MachineId, Option<Summary>>,
    pub(super) snapshots: BTreeMap<MachineId, Option<States>>,
    pub(super) arrival_snapshots: BTreeMap<MachineId, Option<States>>,
}
impl<'a> Replay<'a> {
    pub(super) fn new(
        types: &'a [StructuralTypeDeclaration],
        machines: &'a [DescriptorMachine],
    ) -> Self {
        Self::with_slot_kind(types, machines, DescriptorSlotKind::Views)
    }
    pub(super) fn with_slot_kind(
        types: &'a [StructuralTypeDeclaration],
        machines: &'a [DescriptorMachine],
        slot_kind: DescriptorSlotKind,
    ) -> Self {
        Self {
            types,
            slot_kind,
            machines,
            summaries: BTreeMap::new(),
            snapshots: BTreeMap::new(),
            arrival_snapshots: BTreeMap::new(),
        }
    }
    pub(super) fn machine(&self, identity: MachineId) -> Option<&'a DescriptorMachine> {
        let mut rows = self
            .machines
            .iter()
            .filter(|machine| machine.id == identity);
        let row = rows.next()?;
        rows.next().is_none().then_some(row)
    }
    pub(super) fn parameter(
        &self,
        machine: &'a DescriptorMachine,
        root: PlaceId,
    ) -> Option<(usize, StructuralAccess, StructuralTypeId)> {
        let mut rows = machine.places.iter().filter(|row| row.place == root);
        let row = rows.next()?;
        if rows.next().is_some() {
            return None;
        }
        match row.definition {
            DescriptorDefinition::Parameter {
                position,
                access,
                structural_type,
            } => Some((position, access, structural_type)),
            _ => None,
        }
    }
    pub(super) fn storage_projection(
        &self,
        machine: &DescriptorMachine,
        location: &DescriptorLocation,
    ) -> Option<DescriptorCallOrigin> {
        let path = descriptor_path(
            self.types,
            *machine.place_types.get(&location.root)?,
            &location.path,
        )?;
        Some(DescriptorCallOrigin::Argument {
            source: location.root,
            projection: Projection::new(&path),
        })
    }
    pub(super) fn canonical_storage(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        location: &DescriptorLocation,
    ) -> Option<DescriptorLocation> {
        if location.path.is_empty() {
            return Some(location.clone());
        }
        let Some(Some(origins)) = state.slots.get(&DescriptorLocation::whole(location.root)) else {
            return Some(location.clone());
        };
        if origins.is_empty()
            || origins
                .iter()
                .any(|origin| !matches!(origin, Origin::Storage(_)))
        {
            return Some(location.clone());
        }
        let mut locations = self.storage_origins(machine, state, location)?.into_iter();
        let first = locations.next()?;
        locations
            .all(|candidate| candidate == first)
            .then_some(first)
    }
    /// Exact storage alternatives are distinct from descriptor payload origins.
    /// Edge bindings already substituted each predecessor in the snapshot; an
    /// unknown binding or captured incoming slot cannot manufacture an address.
    pub(super) fn storage_origins(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        location: &DescriptorLocation,
    ) -> Option<Vec<DescriptorLocation>> {
        let origins = state
            .slots
            .get(&DescriptorLocation::whole(location.root))?
            .as_ref()?;
        if origins.is_empty() {
            return None;
        }
        let mut locations = Vec::with_capacity(origins.len());
        for origin in origins {
            let Origin::Storage(backing) = origin else {
                return None;
            };
            let mut suffix = location.path.as_slice();
            let root_type = *machine.place_types.get(&location.root)?;
            let backing_type = *machine.place_types.get(&backing.root)?;
            let backing_tip = crate::descriptor_lifetimes::descriptor_location_type(
                self.types,
                backing_type,
                &backing.path,
            );
            if let Some(StructuralTypeShape::Reference { referent, .. }) = self
                .types
                .iter()
                .find(|ty| ty.id == root_type)
                .map(|ty| &ty.shape)
                && backing_tip == Some(*referent)
            {
                let (first, rest) = suffix.split_first()?;
                if *first != DescriptorPathSegment::Referent {
                    return None;
                }
                suffix = rest;
            }
            let mut target = backing.clone();
            target.path.extend_from_slice(suffix);
            descriptor_path(self.types, backing_type, &target.path)?;
            if !locations.contains(&target) {
                locations.push(target);
            }
        }
        Some(locations)
    }
    pub(super) fn read(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        location: &DescriptorLocation,
    ) -> Value {
        let location = self.canonical_storage(machine, state, location)?;
        if let Some(value) = state.slots.get(&location) {
            return value.clone();
        }
        if self.slot_kind == DescriptorSlotKind::PlacedOrigins {
            // The most specific current descriptor/placed borrow supplies its
            // interpretation token. It does not supply a physical address.
            let prefix = state
                .slots
                .keys()
                .filter(|prefix| {
                    prefix.root == location.root && location.path.starts_with(&prefix.path)
                })
                .max_by_key(|prefix| prefix.path.len())?;
            let origins = state.slots.get(prefix)?.as_ref()?;
            if origins.iter().all(|origin| {
                matches!(
                    origin,
                    Origin::Incoming(_)
                        | Origin::Backing(DescriptorCallOrigin::PlacedFormation { .. })
                )
            }) {
                return Some(origins.clone());
            }
            return None;
        }
        // An opaque referent is unavailable, not an owned subtree whose
        // address can be reconstructed from the enclosing storage root.
        if state.slots.iter().any(|(prefix, value)| {
            value.is_none()
                && prefix.root == location.root
                && !prefix.path.is_empty()
                && location.path.starts_with(&prefix.path)
        }) {
            return None;
        }
        // Only a genuine storage root supplies an unmaterialized owned subtree.
        // Unknown carrier slots are explicitly present in the state as None.
        let root = DescriptorLocation::whole(location.root);
        let path = descriptor_path(
            self.types,
            *machine.place_types.get(&location.root)?,
            &location.path,
        )?;
        let projection = Projection::new(&path);
        state
            .slots
            .get(&root)?
            .as_ref()?
            .iter()
            .map(|origin| match origin {
                Origin::Storage(backing) if backing == &root => {
                    Some(Origin::Storage(location.clone()))
                }
                Origin::Backing(DescriptorCallOrigin::Argument {
                    source,
                    projection: prefix,
                }) => Some(Origin::Backing(DescriptorCallOrigin::Argument {
                    source: *source,
                    projection: prefix.then(&projection),
                })),
                _ => None,
            })
            .collect()
    }
    pub(super) fn initial(&self, machine: &DescriptorMachine) -> Option<State> {
        let mut state = State::default();
        for row in &machine.places {
            state
                .slots
                .insert(DescriptorLocation::whole(row.place), None);
            let inventory = self.slot_kind.inventory(
                self.types,
                row.place,
                *machine.place_types.get(&row.place)?,
            )?;
            for slot in inventory.slots.iter().chain(&inventory.opaque_referents) {
                state.slots.insert(slot.clone(), None);
            }
            if let DescriptorDefinition::Parameter {
                structural_type,
                access,
                ..
            } = row.definition
            {
                if access == StructuralAccess::WriteOnlyBorrow
                    && is_descriptor(self.types.iter(), structural_type)
                {
                    continue;
                }
                let origin = if self.slot_kind != DescriptorSlotKind::Views
                    && self.slot_kind.is_slot(self.types, structural_type)
                {
                    Origin::Incoming(DescriptorLocation::whole(row.place))
                } else if is_descriptor(self.types.iter(), structural_type) {
                    Origin::Backing(DescriptorCallOrigin::Argument {
                        source: row.place,
                        projection: Projection::new(&[]),
                    })
                } else {
                    Origin::Storage(DescriptorLocation::whole(row.place))
                };
                state.slots.insert(
                    DescriptorLocation::whole(row.place),
                    Some(BTreeSet::from([origin])),
                );
                for slot in inventory.slots {
                    if !slot.path.is_empty() {
                        state
                            .slots
                            .insert(slot.clone(), Some(BTreeSet::from([Origin::Incoming(slot)])));
                    }
                }
            }
        }
        Some(state)
    }
    pub(super) fn project(
        &self,
        machine: &DescriptorMachine,
        value: Value,
        projection: Option<&Projection>,
    ) -> Value {
        let Some(projection) = projection else {
            return value;
        };
        value?
            .into_iter()
            .map(|origin| match origin {
                Origin::Storage(location) => match self.storage_projection(machine, &location)? {
                    DescriptorCallOrigin::Argument {
                        source,
                        projection: prefix,
                    } => Some(Origin::Backing(DescriptorCallOrigin::Argument {
                        source,
                        projection: prefix.then(projection),
                    })),
                    _ => None,
                },
                Origin::Backing(DescriptorCallOrigin::Argument {
                    source,
                    projection: prefix,
                }) => Some(Origin::Backing(DescriptorCallOrigin::Argument {
                    source,
                    projection: prefix.then(projection),
                })),
                literal @ Origin::Backing(
                    DescriptorCallOrigin::Literal { .. }
                    | DescriptorCallOrigin::Empty
                    | DescriptorCallOrigin::PlacedFormation { .. },
                ) => Some(literal),
                // Incoming descriptor extents may be narrowed, but its storage
                // identity remains the same until substituted in the caller.
                incoming @ (Origin::Incoming(_) | Origin::IncomingContents(_)) => Some(incoming),
            })
            .collect()
    }
    pub(super) fn transfer(
        &self,
        machine: &DescriptorMachine,
        before: &State,
        definitions: &[DescriptorLocation],
        derivations: &[(DescriptorLocation, DescriptorLocation)],
        position: Option<DescriptorPosition>,
    ) -> Option<State> {
        // Operation writes follow exact current storage aliases; edge bindings
        // define new carrier names and therefore use their declared destinations.
        let canonical = |location: &DescriptorLocation| {
            if position.is_some() {
                self.canonical_storage(machine, before, location)
            } else {
                Some(location.clone())
            }
        };
        let definitions = definitions
            .iter()
            .map(canonical)
            .collect::<Option<Vec<_>>>()?;
        let mut after = before.clone();
        for (location, value) in &mut after.slots {
            if definitions
                .iter()
                .any(|definition| definition.contains(location))
            {
                *value = None;
            }
        }
        for definition in &definitions {
            after.slots.insert(definition.clone(), None);
        }
        let mut assigned = Slots::new();
        for (source, destination) in derivations {
            let projection = position.and_then(|position| {
                machine
                    .projections
                    .get(&(position, source.clone(), destination.clone()))
            });
            if position.is_some_and(|position| {
                machine.storage_derivations.contains(&(
                    position,
                    source.clone(),
                    destination.clone(),
                ))
            }) {
                // Borrowing a pointer slot captures its address. Loading or
                // moving that slot instead captures the pointer it contains.
                // Referent reads subsequently follow this storage alias, so
                // copying descendants into the pointer's own carrier would
                // both duplicate and misplace their contents.
                let storage = self.canonical_storage(machine, before, source)?;
                merge_value(
                    &mut assigned,
                    canonical(destination)?,
                    self.project(
                        machine,
                        Some(BTreeSet::from([Origin::Storage(storage)])),
                        projection,
                    ),
                );
                continue;
            }
            let contents_query = position.and_then(|position| {
                machine
                    .contents_queries
                    .get(&(position, source.clone(), destination.clone()))
            });
            let value = match contents_query {
                Some(query) => self.read_contents(machine, before, query),
                None => self.read(machine, before, source),
            };
            let value = self.project(machine, value, projection);
            let destination = canonical(destination)?;
            merge_value(&mut assigned, destination.clone(), value);
            // A query loads descriptor contents; it does not transport every
            // carrier below the conservative observation prefix into its result.
            if contents_query.is_some() {
                continue;
            }
            for location in before.slots.keys() {
                if location != source
                    && let Some(target) = location.transported(source, &destination)
                {
                    merge_value(
                        &mut assigned,
                        target,
                        self.project(machine, self.read(machine, before, location), projection),
                    );
                }
            }
        }
        after.slots.extend(assigned);
        Some(after)
    }
    pub(super) fn states(&mut self, identity: MachineId, depth: usize) -> Option<States> {
        if let Some(cached) = self.snapshots.get(&identity) {
            return cached.clone();
        }
        self.snapshots.insert(identity, None);
        self.arrival_snapshots.insert(identity, None);
        if depth >= 128 {
            return None;
        }
        let machine = self.machine(identity)?;
        if !machine.flow.contains_key(&machine.entry) {
            return None;
        }
        let mut states = States::from([(machine.entry, self.initial(machine)?)]);
        let mut arrivals = States::new();
        let mut pending = vec![machine.entry];
        let mut visits = 0usize;
        while let Some(position) = pending.pop() {
            visits += 1;
            // Malformed expanding cyclic projections must not manufacture an
            // unbounded proof search. Failure supplies no provenance authority.
            if visits > machine.flow.len().saturating_mul(4096).max(4096) {
                return None;
            }
            let before = states.get(&position)?.clone();
            let step = machine.flow.get(&position)?;
            let mut after = self.transfer(
                machine,
                &before,
                &step.definitions,
                &step.derivations,
                Some(position),
            )?;
            after.contents_invalidated |= machine.unresolved_contents_writes.contains(&position);
            if let Some(transfers) = machine.contents_transfers.get(&position) {
                self.transfer_contents(machine, &before, &mut after, transfers);
            }
            if let Some(origins) = machine.originations.get(&position) {
                for (location, origin) in origins {
                    let origin = if matches!(origin, DescriptorCallOrigin::Argument { source, projection } if *source == location.root && location.path.is_empty() && projection == &Projection::new(&[]))
                        && machine
                            .place_types
                            .get(&location.root)
                            .is_some_and(|ty| !is_descriptor(self.types.iter(), *ty))
                    {
                        Origin::Storage(location.clone())
                    } else {
                        Origin::Backing(origin.clone())
                    };
                    after
                        .slots
                        .insert(location.clone(), Some(BTreeSet::from([origin])));
                }
            }
            if let Some(call) = machine.calls.get(&position)
                && call.argument_contents.iter().all(Option::is_some)
            {
                let summary = self.summary(call.callee, depth + 1)?;
                after.contents_invalidated |= summary.invalidates_contents;
                let mut replacements = BTreeMap::new();
                let mut destinations = Vec::new();
                for effect in &summary.effects {
                    let destination =
                        self.actual_contents(call, effect.parameter, &effect.steps)?;
                    let targets = self.contents_targets(machine, &before, &destination)?;
                    let strong = targets.len() == 1
                        && matches!(targets.first(), Some(DescriptorPayloadTarget::Storage(_)));
                    for target in targets {
                        // Simultaneous effects do not define an order between
                        // aliased definite writes. Preserve the exact-slot
                        // refusal; only may-target effects can weakly union.
                        if let DescriptorPayloadTarget::Storage(location) = target {
                            if destinations.iter().any(
                                |(previous, definite): &(DescriptorLocation, bool)| {
                                    (strong || *definite)
                                        && (location.contains(previous)
                                            || previous.contains(&location))
                                },
                            ) {
                                return None;
                            }
                            destinations.push((location, strong));
                        }
                    }
                    let mut origins = BTreeSet::new();
                    for origin in &effect.origins {
                        origins.extend(self.substitute(machine, &before, call, origin)?);
                    }
                    self.contents_replacements(
                        machine,
                        &before,
                        &destination,
                        Some(origins),
                        &mut replacements,
                    )?;
                }
                if let Some(result) = &call.result {
                    for slot in summary.result.as_ref()? {
                        let mut destination = result.clone();
                        destination.path.extend_from_slice(&slot.path);
                        descriptor_path(
                            self.types,
                            *machine.place_types.get(&destination.root)?,
                            &destination.path,
                        )?;
                        let mut origins = BTreeSet::new();
                        for origin in &slot.origins {
                            origins.extend(self.substitute(machine, &before, call, origin)?);
                        }
                        replacements
                            .insert(DescriptorPayloadTarget::Storage(destination), Some(origins));
                    }
                }
                Self::install_contents(&mut after, replacements);
            }
            // Membership-only edge reads observe the completed operation,
            // including descriptor/call effects. Keep only requested edge
            // snapshots; ordinary custody queries remain pre-operation.
            if machine.payload_arrivals.contains_key(&position) {
                arrivals.insert(position, after.clone());
            }
            for edge in &step.successors {
                if !machine.flow.contains_key(&edge.target) {
                    return None;
                }
                let carried =
                    self.transfer(machine, &after, &edge.parameters, &edge.bindings, None)?;
                match states.get_mut(&edge.target) {
                    None => {
                        states.insert(edge.target, carried);
                        pending.push(edge.target);
                    }
                    Some(existing) => {
                        if merge_state(existing, &carried) {
                            pending.push(edge.target);
                        }
                    }
                }
            }
        }
        self.snapshots.insert(identity, Some(states.clone()));
        self.arrival_snapshots.insert(identity, Some(arrivals));
        Some(states)
    }
    pub(super) fn prepare_states(&mut self, identity: MachineId) -> Option<()> {
        if !self.snapshots.contains_key(&identity) {
            self.states(identity, 0)?;
        }
        self.snapshots.get(&identity)?.as_ref().map(|_| ())
    }
    pub(super) fn actual_slot(
        &self,
        call: &DescriptorInvocation,
        parameter: usize,
        path: &[DescriptorPathSegment],
    ) -> Option<DescriptorLocation> {
        let mut actual = call.arguments.get(parameter)?.as_ref()?.clone();
        actual.path.extend_from_slice(path);
        Some(actual)
    }
    pub(super) fn substitute(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        call: &DescriptorInvocation,
        origin: &DescriptorStoredOrigin,
    ) -> Value {
        match origin {
            DescriptorStoredOrigin::PlacedFormation {
                machine,
                operation,
                place,
                carrier,
            } => Some(BTreeSet::from([Origin::Backing(
                DescriptorCallOrigin::PlacedFormation {
                    machine: *machine,
                    operation: *operation,
                    place: *place,
                    carrier: *carrier,
                },
            )])),
            DescriptorStoredOrigin::Empty => Some(BTreeSet::from([Origin::Backing(
                DescriptorCallOrigin::Empty,
            )])),
            DescriptorStoredOrigin::Literal { machine, place } => {
                Some(BTreeSet::from([Origin::Backing(
                    DescriptorCallOrigin::Literal {
                        machine: *machine,
                        place: *place,
                    },
                )]))
            }
            DescriptorStoredOrigin::Parameter {
                position,
                projection,
            } => self.project(
                machine,
                self.read_contents(
                    machine,
                    state,
                    call.argument_contents.get(*position)?.as_ref()?,
                ),
                Some(projection),
            ),
            DescriptorStoredOrigin::IncomingSlot { position, path } => {
                let mut query = call.argument_contents.get(*position)?.as_ref()?.clone();
                query
                    .steps
                    .extend(path.iter().cloned().map(DescriptorContentsStep::Address));
                self.read_contents(machine, state, &query)
            }
            DescriptorStoredOrigin::IncomingContents { position, steps } => {
                let mut query = call.argument_contents.get(*position)?.as_ref()?.clone();
                query.steps.extend_from_slice(steps);
                self.read_contents(machine, state, &query)
            }
        }
    }
    pub(super) fn exported(
        &self,
        machine: &DescriptorMachine,
        origin: &Origin,
    ) -> Option<DescriptorStoredOrigin> {
        match origin {
            Origin::Backing(DescriptorCallOrigin::PlacedFormation {
                machine,
                operation,
                place,
                carrier,
            }) => Some(DescriptorStoredOrigin::PlacedFormation {
                machine: *machine,
                operation: *operation,
                place: *place,
                carrier: *carrier,
            }),
            Origin::Backing(DescriptorCallOrigin::Empty) => Some(DescriptorStoredOrigin::Empty),
            Origin::Storage(location) => self.exported(
                machine,
                &Origin::Backing(self.storage_projection(machine, location)?),
            ),
            Origin::Incoming(location) => {
                let (position, _, _) = self.parameter(machine, location.root)?;
                Some(DescriptorStoredOrigin::IncomingSlot {
                    position,
                    path: location.path.clone(),
                })
            }
            Origin::IncomingContents(query) => {
                let (position, _, root_type) = self.parameter(machine, query.root)?;
                query.validate(self.types, root_type)?;
                Some(DescriptorStoredOrigin::IncomingContents {
                    position,
                    steps: query.steps.clone(),
                })
            }
            Origin::Backing(DescriptorCallOrigin::Literal { machine, place }) => {
                Some(DescriptorStoredOrigin::Literal {
                    machine: *machine,
                    place: *place,
                })
            }
            Origin::Backing(DescriptorCallOrigin::Argument { source, projection }) => {
                let (position, access, structural_type) = self.parameter(machine, *source)?;
                if !matches!(access, terminal_psi::reading_borrow_pair_pattern!())
                    && !(access == StructuralAccess::Owned
                        && external_backing(self.types, structural_type, &projection.path))
                {
                    return None;
                }
                Some(DescriptorStoredOrigin::Parameter {
                    position,
                    projection: projection.clone().without_runtime_coordinates(),
                })
            }
        }
    }
    pub(super) fn summary(&mut self, identity: MachineId, depth: usize) -> Option<Summary> {
        if let Some(cached) = self.summaries.get(&identity) {
            return cached.clone();
        }
        self.summaries.insert(identity, None);
        let machine = self.machine(identity)?;
        let states = self.states(identity, depth)?;
        // A result's carrier stays local storage. Only its exact descriptor
        // slots export backing, under the same case/field paths as construction
        // and copying. Missing live-slot contents are unknown, never empty.
        let result = if let Some(returns) = &machine.returns {
            let mut slots: Option<Vec<ResultSlot>> = None;
            let mut result_type = None;
            for (position, location) in returns {
                if !location.path.is_empty() {
                    return None;
                }
                let structural_type = *machine.place_types.get(&location.root)?;
                if result_type.is_some_and(|previous| previous != structural_type) {
                    return None;
                }
                result_type = Some(structural_type);
                let returned = self
                    .slot_kind
                    .slots(self.types, location.root, structural_type)?;
                let accumulated = slots.get_or_insert_with(|| {
                    returned
                        .iter()
                        .map(|slot| ResultSlot {
                            path: slot.path.clone(),
                            origins: Vec::new(),
                        })
                        .collect()
                });
                if accumulated.len() != returned.len() {
                    return None;
                }
                for (slot, location) in accumulated.iter_mut().zip(returned) {
                    if slot.path != location.path {
                        return None;
                    }
                    let origins = self.read(machine, states.get(position)?, &location)?;
                    if origins.is_empty() {
                        return None;
                    }
                    for origin in origins {
                        slot.origins.push(self.exported(machine, &origin)?);
                    }
                }
            }
            Some(slots?)
        } else {
            None
        };
        let mut effects = Vec::new();
        for row in &machine.places {
            let DescriptorDefinition::Parameter {
                position: parameter,
                structural_type,
                access,
                ..
            } = row.definition
            else {
                continue;
            };
            let slots = if self.slot_kind == DescriptorSlotKind::References {
                writable_reference_slots(self.types, row.place, structural_type, access)?
            } else {
                if !matches!(access, terminal_psi::exclusive_borrow_pair_pattern!())
                    && !matches!(
                        find_structural_type(self.types, structural_type)?.shape,
                        StructuralTypeShape::Reference {
                            access: terminal_psi::exclusive_borrow_pair_pattern!(),
                            ..
                        }
                    )
                {
                    continue;
                }
                self.slot_kind
                    .slots(self.types, row.place, structural_type)?
                    .into_iter()
                    .filter(|slot| !slot.path.is_empty())
                    .collect()
            };
            for slot in slots {
                let mut origins = BTreeSet::new();
                for exit in &machine.normal_exits {
                    if let Some(state) = states.get(exit) {
                        origins.extend(self.read(machine, state, &slot)?);
                    }
                }
                if origins.is_empty() {
                    continue;
                }
                if origins == BTreeSet::from([Origin::Incoming(slot.clone())]) {
                    continue;
                }
                effects.push(DescriptorStoredEffect {
                    parameter,
                    steps: slot
                        .path
                        .into_iter()
                        .map(DescriptorContentsStep::Address)
                        .collect(),
                    origins: origins
                        .iter()
                        .map(|origin| self.exported(machine, origin))
                        .collect::<Option<_>>()?,
                });
            }
        }
        let contents = machine
            .normal_exits
            .iter()
            .filter_map(|position| states.get(position))
            .flat_map(|state| state.contents.keys().cloned())
            .collect::<BTreeSet<_>>();
        for query in contents {
            let (parameter, _, root_type) = self.parameter(machine, query.root)?;
            query.validate(self.types, root_type)?;
            let mut origins = BTreeSet::new();
            for exit in &machine.normal_exits {
                if let Some(state) = states.get(exit) {
                    origins.extend(state.incoming_contents(&query)?);
                }
            }
            effects.push(DescriptorStoredEffect {
                parameter,
                steps: query.steps,
                origins: origins
                    .iter()
                    .map(|origin| self.exported(machine, origin))
                    .collect::<Option<_>>()?,
            });
        }
        let invalidates_contents = machine.normal_exits.iter().any(|position| {
            states
                .get(position)
                .is_some_and(|state| state.contents_invalidated)
        });
        let summary = Summary {
            result,
            effects,
            invalidates_contents,
        };
        self.summaries.insert(identity, Some(summary.clone()));
        Some(summary)
    }
    pub(super) fn symbolic(
        &self,
        machine: &DescriptorMachine,
        origins: BTreeSet<Origin>,
    ) -> Option<Vec<DescriptorSourceOrigin>> {
        origins
            .into_iter()
            .map(|origin| match origin {
                Origin::Backing(origin) => Some(DescriptorSourceOrigin::Backing(origin)),
                Origin::Storage(location) => Some(DescriptorSourceOrigin::Backing(
                    self.storage_projection(machine, &location)?,
                )),
                Origin::Incoming(location) => Some(DescriptorSourceOrigin::IncomingSlot(location)),
                Origin::IncomingContents(query) => {
                    Some(DescriptorSourceOrigin::IncomingContents(query))
                }
            })
            .collect()
    }
    pub(super) fn flatten(
        &self,
        machine: &DescriptorMachine,
        origin: DescriptorSourceOrigin,
    ) -> Option<DescriptorCallOrigin> {
        match origin {
            DescriptorSourceOrigin::Backing(origin) => Some(origin),
            DescriptorSourceOrigin::IncomingSlot(location) => {
                let DescriptorCallOrigin::Argument {
                    source,
                    mut projection,
                } = self.storage_projection(machine, &location)?
                else {
                    return None;
                };
                projection.indirect = true;
                Some(DescriptorCallOrigin::Argument { source, projection })
            }
            DescriptorSourceOrigin::IncomingContents(query) => {
                let root_type = *machine.place_types.get(&query.root)?;
                Some(DescriptorCallOrigin::Argument {
                    source: query.root,
                    projection: query.conservative_projection(self.types, root_type)?,
                })
            }
        }
    }
    pub(super) fn resolved(
        &self,
        machine: &DescriptorMachine,
        origins: BTreeSet<Origin>,
    ) -> Option<Vec<DescriptorCallOrigin>> {
        self.symbolic(machine, origins)?
            .into_iter()
            .map(|origin| self.flatten(machine, origin))
            .collect()
    }
}

pub(super) fn merge_value(state: &mut Slots, location: DescriptorLocation, value: Value) {
    match state.get_mut(&location) {
        None => {
            state.insert(location, value);
        }
        Some(existing) => match (existing.as_mut(), value) {
            (Some(existing), Some(value)) => existing.extend(value),
            _ => *existing = None,
        },
    }
}
fn merge_state(existing: &mut State, incoming: &State) -> bool {
    let before = existing.clone();
    existing.contents_invalidated |= incoming.contents_invalidated;
    for query in before
        .contents
        .keys()
        .chain(incoming.contents.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
    {
        let value = match (
            before.incoming_contents(&query),
            incoming.incoming_contents(&query),
        ) {
            (Some(mut left), Some(right)) => {
                left.extend(right);
                Some(left)
            }
            _ => None,
        };
        existing.contents.insert(query, value);
    }
    for location in before
        .slots
        .keys()
        .chain(incoming.slots.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
    {
        let value = match (
            before.slots.get(&location).and_then(Option::as_ref),
            incoming.slots.get(&location).and_then(Option::as_ref),
        ) {
            (Some(left), Some(right)) => Some(left.union(right).cloned().collect()),
            _ => None,
        };
        existing.slots.insert(location, value);
    }
    *existing != before
}

impl State {
    pub(super) fn incoming_contents(&self, query: &DescriptorContentsQuery) -> Value {
        let mut origins = BTreeSet::from([Origin::IncomingContents(query.clone())]);
        for (written, value) in &self.contents {
            if written.root != query.root {
                continue;
            }
            let overlaps = written.steps.iter().zip(&query.steps).all(|(left, right)| {
                left == right
                    || matches!(
                        (left, right),
                        (
                            DescriptorContentsStep::AnyElement { .. },
                            DescriptorContentsStep::Address(DescriptorPathSegment::FixedIndex(_))
                        ) | (
                            DescriptorContentsStep::Address(DescriptorPathSegment::FixedIndex(_)),
                            DescriptorContentsStep::AnyElement { .. }
                        )
                    )
            });
            if !overlaps {
                continue;
            }
            // A changed descriptor on the route is not the queried leaf's
            // payload. Without reconstructed intermediate backing, entry-only
            // contents would omit that replacement; do not invent provenance.
            if written.steps.len() != query.steps.len() {
                return None;
            }
            origins.extend(value.as_ref()?.iter().cloned());
        }
        Some(origins)
    }
}
