//! One position-sensitive contents replay over the descriptor cohort graph.
use super::*;
use crate::descriptor_lifetimes::{DescriptorPathSegment, descriptor_path};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Origin {
    Backing(DescriptorCallOrigin),
    Incoming(DescriptorLocation),
    Storage(DescriptorLocation),
}
type Value = Option<BTreeSet<Origin>>;
type State = BTreeMap<DescriptorLocation, Value>;
type States = BTreeMap<DescriptorPosition, State>;

#[derive(Clone)]
pub enum DescriptorStoredOrigin {
    Empty,
    Parameter {
        position: usize,
        projection: Projection,
    },
    IncomingSlot {
        position: usize,
        path: Vec<DescriptorPathSegment>,
    },
    Literal {
        machine: MachineId,
        place: PlaceId,
    },
}
#[derive(Clone)]
pub struct DescriptorStoredEffect {
    pub parameter: usize,
    pub path: Vec<DescriptorPathSegment>,
    pub origins: Vec<DescriptorStoredOrigin>,
}
pub struct DescriptorStoredCallEffect<Origin = DescriptorCallOrigin> {
    pub destination: DescriptorLocation,
    /// All possible post-call origins, including the pre-call slot when preserved.
    pub origins: Vec<Origin>,
    pub preserves_incoming: bool,
}
#[derive(Clone)]
struct Summary {
    result: Option<Vec<DescriptorStoredOrigin>>,
    effects: Vec<DescriptorStoredEffect>,
}
struct Replay<'a> {
    types: &'a [StructuralTypeDeclaration],
    machines: &'a [DescriptorMachine],
    summaries: BTreeMap<MachineId, Option<Summary>>,
    snapshots: BTreeMap<MachineId, Option<States>>,
}
impl<'a> Replay<'a> {
    fn new(types: &'a [StructuralTypeDeclaration], machines: &'a [DescriptorMachine]) -> Self {
        Self {
            types,
            machines,
            summaries: BTreeMap::new(),
            snapshots: BTreeMap::new(),
        }
    }
    fn machine(&self, identity: MachineId) -> Option<&'a DescriptorMachine> {
        let mut rows = self
            .machines
            .iter()
            .filter(|machine| machine.id == identity);
        let row = rows.next()?;
        rows.next().is_none().then_some(row)
    }
    fn parameter(
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
    fn storage_projection(
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
    fn canonical_storage(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        location: &DescriptorLocation,
    ) -> Option<DescriptorLocation> {
        if location.path.is_empty() {
            return Some(location.clone());
        }
        let Some(Some(origins)) = state.get(&DescriptorLocation::whole(location.root)) else {
            return Some(location.clone());
        };
        let mut canonical = None;
        for origin in origins {
            let Origin::Storage(backing) = origin else {
                return Some(location.clone());
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
            if canonical
                .as_ref()
                .is_some_and(|previous| previous != &target)
            {
                return None;
            }
            canonical = Some(target);
        }
        canonical.or_else(|| Some(location.clone()))
    }
    fn read(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        location: &DescriptorLocation,
    ) -> Value {
        let location = self.canonical_storage(machine, state, location)?;
        if let Some(value) = state.get(&location) {
            return value.clone();
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
    fn initial(&self, machine: &DescriptorMachine) -> Option<State> {
        let mut state = State::new();
        for row in &machine.places {
            state.insert(DescriptorLocation::whole(row.place), None);
            if let Some(structural_type) = machine.place_types.get(&row.place) {
                for slot in descriptor_slots(self.types, row.place, *structural_type)? {
                    state.insert(slot, None);
                }
            }
            if let DescriptorDefinition::Parameter {
                structural_type,
                access,
                ..
            } = row.definition
            {
                if access == StructuralAccess::WriteOnlyBorrow
                    && is_shared_descriptor(self.types.iter(), structural_type)
                {
                    continue;
                }
                let origin = if is_shared_descriptor(self.types.iter(), structural_type) {
                    Origin::Backing(DescriptorCallOrigin::Argument {
                        source: row.place,
                        projection: Projection::new(&[]),
                    })
                } else {
                    Origin::Storage(DescriptorLocation::whole(row.place))
                };
                state.insert(
                    DescriptorLocation::whole(row.place),
                    Some(BTreeSet::from([origin])),
                );
                for slot in descriptor_slots(self.types, row.place, structural_type)? {
                    if !slot.path.is_empty() {
                        state.insert(slot.clone(), Some(BTreeSet::from([Origin::Incoming(slot)])));
                    }
                }
            }
        }
        Some(state)
    }
    fn project(
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
                    DescriptorCallOrigin::Literal { .. } | DescriptorCallOrigin::Empty,
                ) => Some(literal),
                // Incoming descriptor extents may be narrowed, but its storage
                // identity remains the same until substituted in the caller.
                incoming @ Origin::Incoming(_) => Some(incoming),
            })
            .collect()
    }
    fn transfer(
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
        for (location, value) in &mut after {
            if definitions
                .iter()
                .any(|definition| definition.contains(location))
            {
                *value = None;
            }
        }
        for definition in &definitions {
            after.insert(definition.clone(), None);
        }
        let mut assigned = State::new();
        for (source, destination) in derivations {
            let projection = position.and_then(|position| {
                machine
                    .projections
                    .get(&(position, source.clone(), destination.clone()))
            });
            let value = self.project(machine, self.read(machine, before, source), projection);
            let destination = canonical(destination)?;
            merge_value(&mut assigned, destination.clone(), value);
            for location in before.keys() {
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
        after.extend(assigned);
        Some(after)
    }
    fn states(&mut self, identity: MachineId, depth: usize) -> Option<States> {
        if let Some(cached) = self.snapshots.get(&identity) {
            return cached.clone();
        }
        self.snapshots.insert(identity, None);
        if depth >= 128 {
            return None;
        }
        let machine = self.machine(identity)?;
        if !machine.flow.contains_key(&machine.entry) {
            return None;
        }
        let mut states = States::from([(machine.entry, self.initial(machine)?)]);
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
            if let Some(origins) = machine.originations.get(&position) {
                for (location, origin) in origins {
                    let origin = if matches!(origin, DescriptorCallOrigin::Argument { source, projection } if *source == location.root && location.path.is_empty() && projection == &Projection::new(&[]))
                        && machine
                            .place_types
                            .get(&location.root)
                            .is_some_and(|ty| !is_shared_descriptor(self.types.iter(), *ty))
                    {
                        Origin::Storage(location.clone())
                    } else {
                        Origin::Backing(origin.clone())
                    };
                    after.insert(location.clone(), Some(BTreeSet::from([origin])));
                }
            }
            if let Some(call) = machine.calls.get(&position)
                && call.arguments.iter().all(Option::is_some)
            {
                let summary = self.summary(call.callee, depth + 1)?;
                let mut replacements = State::new();
                for effect in &summary.effects {
                    let destination = self.canonical_storage(
                        machine,
                        &before,
                        &self.actual_slot(call, effect.parameter, &effect.path)?,
                    )?;
                    let mut origins = BTreeSet::new();
                    for origin in &effect.origins {
                        origins.extend(self.substitute(machine, &before, call, origin)?);
                    }
                    // Aliased outputs are not an order in which to replay
                    // writes. Until disjointness is proved, refuse ambiguity.
                    if replacements
                        .keys()
                        .any(|other| destination.contains(other) || other.contains(&destination))
                    {
                        return None;
                    }
                    replacements.insert(destination, Some(origins));
                }
                if let Some(result) = &call.result {
                    let mut origins = BTreeSet::new();
                    for origin in summary.result.as_ref()? {
                        origins.extend(self.substitute(machine, &before, call, origin)?);
                    }
                    replacements.insert(result.clone(), Some(origins));
                }
                for (destination, value) in replacements {
                    after.retain(|location, _| {
                        location == &destination || !destination.contains(location)
                    });
                    after.insert(destination, value);
                }
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
        Some(states)
    }
    fn prepare_states(&mut self, identity: MachineId) -> Option<()> {
        if !self.snapshots.contains_key(&identity) {
            self.states(identity, 0)?;
        }
        self.snapshots.get(&identity)?.as_ref().map(|_| ())
    }
    fn actual_slot(
        &self,
        call: &DescriptorInvocation,
        parameter: usize,
        path: &[DescriptorPathSegment],
    ) -> Option<DescriptorLocation> {
        let mut actual = call.arguments.get(parameter)?.as_ref()?.clone();
        actual.path.extend_from_slice(path);
        Some(actual)
    }
    fn substitute(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        call: &DescriptorInvocation,
        origin: &DescriptorStoredOrigin,
    ) -> Value {
        match origin {
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
                self.read(machine, state, call.arguments.get(*position)?.as_ref()?),
                Some(projection),
            ),
            DescriptorStoredOrigin::IncomingSlot { position, path } => {
                self.read(machine, state, &self.actual_slot(call, *position, path)?)
            }
        }
    }
    fn exported(
        &self,
        machine: &DescriptorMachine,
        origin: &Origin,
    ) -> Option<DescriptorStoredOrigin> {
        match origin {
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
            Origin::Backing(DescriptorCallOrigin::Literal { machine, place }) => {
                Some(DescriptorStoredOrigin::Literal {
                    machine: *machine,
                    place: *place,
                })
            }
            Origin::Backing(DescriptorCallOrigin::Argument { source, projection }) => {
                let (position, access, structural_type) = self.parameter(machine, *source)?;
                if !matches!(
                    access,
                    StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                ) && !(access == StructuralAccess::Owned
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
    fn summary(&mut self, identity: MachineId, depth: usize) -> Option<Summary> {
        if let Some(cached) = self.summaries.get(&identity) {
            return cached.clone();
        }
        self.summaries.insert(identity, None);
        let machine = self.machine(identity)?;
        let states = self.states(identity, depth)?;
        let mut result = machine.returns.as_ref().map(|_| Vec::new());
        if let Some(returns) = &machine.returns {
            for (position, location) in returns {
                for origin in self.read(machine, states.get(position)?, location)? {
                    result.as_mut()?.push(self.exported(machine, &origin)?);
                }
            }
            if result.as_ref()?.is_empty() {
                return None;
            }
        }
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
            if !matches!(
                access,
                StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
            ) && !matches!(
                self.types.iter().find(|ty| ty.id == structural_type)?.shape,
                StructuralTypeShape::Reference {
                    access: StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow,
                    ..
                }
            ) {
                continue;
            }
            for slot in descriptor_slots(self.types, row.place, structural_type)?
                .into_iter()
                .filter(|slot| !slot.path.is_empty())
            {
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
                    path: slot.path,
                    origins: origins
                        .iter()
                        .map(|origin| self.exported(machine, origin))
                        .collect::<Option<_>>()?,
                });
            }
        }
        let summary = Summary { result, effects };
        self.summaries.insert(identity, Some(summary.clone()));
        Some(summary)
    }
    fn symbolic(
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
            })
            .collect()
    }
    fn flatten(
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
        }
    }
    fn resolved(
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

fn merge_value(state: &mut State, location: DescriptorLocation, value: Value) {
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
    for location in before
        .keys()
        .chain(incoming.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
    {
        let value = match (
            before.get(&location).and_then(Option::as_ref),
            incoming.get(&location).and_then(Option::as_ref),
        ) {
            (Some(left), Some(right)) => Some(left.union(right).cloned().collect()),
            _ => None,
        };
        existing.insert(location, value);
    }
    *existing != before
}

pub fn reconstruct_source_origins(
    types: &[StructuralTypeDeclaration],
    machines: &[DescriptorMachine],
    identity: MachineId,
    position: DescriptorPosition,
    location: &DescriptorLocation,
) -> Option<Vec<DescriptorCallOrigin>> {
    DescriptorAnalysis::new(types, machines).source_origins(identity, position, location)
}
pub fn reconstruct_stored_effects(
    types: &[StructuralTypeDeclaration],
    machines: &[DescriptorMachine],
    identity: MachineId,
) -> Option<Vec<DescriptorStoredEffect>> {
    DescriptorAnalysis::new(types, machines).stored_effects(identity)
}
pub fn reconstruct_call_stored_effects(
    types: &[StructuralTypeDeclaration],
    machines: &[DescriptorMachine],
    identity: MachineId,
    position: DescriptorPosition,
) -> Option<Vec<DescriptorStoredCallEffect>> {
    DescriptorAnalysis::new(types, machines).call_stored_effects(identity, position)
}
fn result_origin(
    types: &[StructuralTypeDeclaration],
    machine: &DescriptorMachine,
    origin: DescriptorStoredOrigin,
) -> Option<DescriptorResultOrigin> {
    match origin {
        DescriptorStoredOrigin::Empty => Some(DescriptorResultOrigin::Empty),
        DescriptorStoredOrigin::Parameter {
            position,
            projection,
        } => Some(DescriptorResultOrigin::Parameter {
            position,
            projection,
        }),
        DescriptorStoredOrigin::Literal { machine, place } => {
            Some(DescriptorResultOrigin::Literal { machine, place })
        }
        DescriptorStoredOrigin::IncomingSlot { position, path } => {
            let root_type = machine.places.iter().find_map(|row| match row.definition {
                DescriptorDefinition::Parameter {
                    position: p,
                    structural_type,
                    ..
                } if p == position => Some(structural_type),
                _ => None,
            })?;
            let mut projection = Projection::new(&descriptor_path(types, root_type, &path)?);
            projection.indirect = true;
            Some(DescriptorResultOrigin::Parameter {
                position,
                projection,
            })
        }
    }
}
pub(super) fn result_origins(
    types: &[StructuralTypeDeclaration],
    machines: &[DescriptorMachine],
    identity: MachineId,
) -> Option<Vec<DescriptorResultOrigin>> {
    let mut replay = Replay::new(types, machines);
    let machine = replay.machine(identity)?;
    replay
        .summary(identity, 0)?
        .result?
        .into_iter()
        .map(|origin| result_origin(types, machine, origin))
        .collect()
}
pub(super) fn return_origins(
    types: &[StructuralTypeDeclaration],
    machines: &[DescriptorMachine],
    identity: MachineId,
    position: DescriptorPosition,
    location: &DescriptorLocation,
) -> Option<Vec<DescriptorResultOrigin>> {
    let mut replay = Replay::new(types, machines);
    let machine = replay.machine(identity)?;
    let states = replay.states(identity, 0)?;
    replay
        .read(machine, states.get(&position)?, location)?
        .iter()
        .map(|origin| result_origin(types, machine, replay.exported(machine, origin)?))
        .collect()
}
pub(super) fn call_origins(
    types: &[StructuralTypeDeclaration],
    machines: &[DescriptorMachine],
    identity: MachineId,
    result: PlaceId,
) -> Option<Vec<DescriptorCallOrigin>> {
    DescriptorAnalysis::new(types, machines).call_origins(identity, result)
}

fn may_contain_descriptor(
    types: &[StructuralTypeDeclaration],
    ty: StructuralTypeId,
    active: &mut BTreeSet<StructuralTypeId>,
) -> bool {
    if !active.insert(ty) {
        return true;
    }
    let Some(declaration) = types.iter().find(|declaration| declaration.id == ty) else {
        return true;
    };
    let mut fields_contain = |fields: &[terminal_psi::StructuralFieldDeclaration]| {
        fields.iter().any(|field| match field.field_type {
            terminal_psi::StructuralFieldType::Structural(ty) => {
                may_contain_descriptor(types, ty, active)
            }
            _ => matches!(
                field.field_type.byte_sequence_carrier(),
                Some(ByteSequenceCarrier::BorrowedView {
                    access: None | Some(StructuralAccess::SharedBorrow)
                })
            ),
        })
    };
    let contains = match &declaration.shape {
        StructuralTypeShape::Record { fields } => fields_contain(fields),
        StructuralTypeShape::Mixed { fields, cases } => {
            fields_contain(fields) || cases.iter().any(|case| fields_contain(&case.fields))
        }
        StructuralTypeShape::Sum { cases } => cases.iter().any(|case| fields_contain(&case.fields)),
        StructuralTypeShape::Reference { referent, .. } => {
            may_contain_descriptor(types, *referent, active)
        }
        StructuralTypeShape::FixedArray { element, .. } => {
            may_contain_descriptor(types, *element, active)
        }
        _ => is_shared_descriptor(types.iter(), ty),
    };
    active.remove(&ty);
    contains
}

/// Enumerate actual shared carrier slots, retaining reference boundaries.
pub fn descriptor_slots(
    types: &[StructuralTypeDeclaration],
    root: PlaceId,
    structural_type: StructuralTypeId,
) -> Option<Vec<DescriptorLocation>> {
    fn walk(
        types: &[StructuralTypeDeclaration],
        ty: StructuralTypeId,
        location: DescriptorLocation,
        active: &mut BTreeSet<StructuralTypeId>,
        result: &mut Vec<DescriptorLocation>,
    ) -> Option<()> {
        if !active.insert(ty) || result.len() > 4096 {
            return None;
        }
        let mut matching = types.iter().filter(|declaration| declaration.id == ty);
        let declaration = matching.next()?;
        if matching.next().is_some() {
            return None;
        }
        if is_shared_descriptor(types.iter(), ty) {
            result.push(location);
            active.remove(&ty);
            return Some(());
        }
        match &declaration.shape {
            StructuralTypeShape::Reference { referent, .. } => {
                let mut child = location.clone();
                child.path.push(DescriptorPathSegment::Referent);
                walk(types, *referent, child, active, result)?;
            }
            StructuralTypeShape::Record { fields } | StructuralTypeShape::Mixed { fields, .. } => {
                for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                    let mut child = location.clone();
                    child.path.push(DescriptorPathSegment::Field(field.id));
                    match field.field_type {
                        terminal_psi::StructuralFieldType::Structural(ty) => {
                            walk(types, ty, child, active, result)?
                        }
                        _ if matches!(
                            field.field_type.byte_sequence_carrier(),
                            Some(ByteSequenceCarrier::BorrowedView {
                                access: None | Some(StructuralAccess::SharedBorrow)
                            })
                        ) =>
                        {
                            result.push(child)
                        }
                        _ => {}
                    }
                }
            }
            StructuralTypeShape::FixedArray { element, length } => {
                if !may_contain_descriptor(types, *element, &mut BTreeSet::new()) {
                    active.remove(&ty);
                    return Some(());
                }
                if *length > 4096 {
                    return None;
                }
                for index in 0..*length {
                    let mut child = location.clone();
                    child.path.push(DescriptorPathSegment::FixedIndex(index));
                    walk(types, *element, child, active, result)?;
                }
            }
            _ => {}
        }
        if let StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } =
            &declaration.shape
        {
            for case in cases {
                for field in case
                    .fields
                    .iter()
                    .filter(|field| !field.relevance.is_erased())
                {
                    let mut child = location.clone();
                    child.path.extend([
                        DescriptorPathSegment::Case(case.id),
                        DescriptorPathSegment::Field(field.id),
                    ]);
                    match field.field_type {
                        terminal_psi::StructuralFieldType::Structural(ty) => {
                            walk(types, ty, child, active, result)?
                        }
                        _ if matches!(
                            field.field_type.byte_sequence_carrier(),
                            Some(ByteSequenceCarrier::BorrowedView {
                                access: None | Some(StructuralAccess::SharedBorrow)
                            })
                        ) =>
                        {
                            result.push(child)
                        }
                        _ => {}
                    }
                }
            }
        }
        active.remove(&ty);
        Some(())
    }
    let mut result = Vec::new();
    walk(
        types,
        structural_type,
        DescriptorLocation::whole(root),
        &mut BTreeSet::new(),
        &mut result,
    )?;
    Some(result)
}

/// Disposable current-body analysis. Snapshots and summaries are shared across
/// queries; every source observation denotes the state before its operation.
pub struct DescriptorAnalysis<'a> {
    replay: Replay<'a>,
}
impl<'a> DescriptorAnalysis<'a> {
    pub fn new(types: &'a [StructuralTypeDeclaration], machines: &'a [DescriptorMachine]) -> Self {
        Self {
            replay: Replay::new(types, machines),
        }
    }
    pub(super) fn compatibility_machines(&self) -> &[DescriptorMachine] {
        self.replay.machines
    }
    pub(super) fn compatibility_ingress(
        &self,
        identity: MachineId,
        requirement: &DescriptorCompatibilityRequirement,
    ) -> Option<()> {
        let machine = self.replay.machine(identity)?;
        machine.flow.get(&requirement.position)?;
        let DescriptorSourceOrigin::IncomingSlot(location) = &requirement.loan else {
            return None;
        };
        let (_, access, ty) = self.replay.parameter(machine, location.root)?;
        if location.path.is_empty()
            || access == StructuralAccess::WriteOnlyBorrow
            || !descriptor_slots(self.replay.types, location.root, ty)?.contains(location)
        {
            return None;
        }
        let DescriptorCallOrigin::Argument { source, projection } = &requirement.demand else {
            return None;
        };
        let (_, access, ty) = self.replay.parameter(machine, *source)?;
        if !matches!(
            access,
            StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow
                | StructuralAccess::Owned
        ) || !projection.exact
            || projection.indirect
            || projection.window.is_some()
            || projection.captured_at.is_some()
        {
            return None;
        }
        // Reuse the catalog walker to reject recursive/ambiguous structural
        // declarations. With finite array extents, every accepted static path
        // then belongs to a finite universe; relation closure needs no row cap.
        descriptor_slots(self.replay.types, *source, ty)?;
        if *projection != Projection::new(&projection.path) {
            return None;
        }
        crate::descriptor_lifetimes::descriptor_location(
            self.replay.types,
            *source,
            ty,
            &projection.path,
            None,
        )?;
        Some(())
    }
    /// Conservative geometry for callers which cannot retain symbolic contents.
    /// This conversion deliberately does not prove an incoming payload disjoint
    /// from the memory occupied by its descriptor slot.
    pub fn flatten_source_origin(
        &self,
        identity: MachineId,
        origin: &DescriptorSourceOrigin,
    ) -> Option<DescriptorCallOrigin> {
        self.replay
            .flatten(self.replay.machine(identity)?, origin.clone())
    }
    pub fn location(
        &mut self,
        identity: MachineId,
        position: DescriptorPosition,
        location: &DescriptorLocation,
    ) -> Option<DescriptorLocation> {
        self.replay.prepare_states(identity)?;
        let machine = self.replay.machine(identity)?;
        let states = self.replay.snapshots.get(&identity)?.as_ref()?;
        self.replay
            .canonical_storage(machine, states.get(&position)?, location)
    }
    pub fn source_origins(
        &mut self,
        identity: MachineId,
        position: DescriptorPosition,
        location: &DescriptorLocation,
    ) -> Option<Vec<DescriptorCallOrigin>> {
        let origins = self.symbolic_source_origins(identity, position, location)?;
        let machine = self.replay.machine(identity)?;
        origins
            .into_iter()
            .map(|origin| self.replay.flatten(machine, origin))
            .collect()
    }
    /// Read the captured descriptor contents before this operation, retaining
    /// incoming slot symbols rather than confusing their payload with storage.
    pub fn symbolic_source_origins(
        &mut self,
        identity: MachineId,
        position: DescriptorPosition,
        location: &DescriptorLocation,
    ) -> Option<Vec<DescriptorSourceOrigin>> {
        self.replay.prepare_states(identity)?;
        let machine = self.replay.machine(identity)?;
        let states = self.replay.snapshots.get(&identity)?.as_ref()?;
        self.replay.symbolic(
            machine,
            self.replay
                .read(machine, states.get(&position)?, location)?,
        )
    }
    /// Substitute a callee backing observation at the exact pre-call snapshot.
    pub fn substitute_call_source(
        &mut self,
        identity: MachineId,
        position: DescriptorPosition,
        origin: &DescriptorSourceOrigin,
    ) -> Option<Vec<DescriptorSourceOrigin>> {
        self.replay.prepare_states(identity)?;
        let machine = self.replay.machine(identity)?;
        let call = machine.calls.get(&position)?;
        let callee = self.replay.machine(call.callee)?;
        let exported = match origin {
            DescriptorSourceOrigin::IncomingSlot(location) => {
                let (position, _, ty) = self.replay.parameter(callee, location.root)?;
                if !descriptor_slots(self.replay.types, location.root, ty)?.contains(location) {
                    return None;
                }
                DescriptorStoredOrigin::IncomingSlot {
                    position,
                    path: location.path.clone(),
                }
            }
            DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Argument {
                source,
                projection,
            }) => {
                let (position, _, _) = self.replay.parameter(callee, *source)?;
                DescriptorStoredOrigin::Parameter {
                    position,
                    projection: projection.clone().without_runtime_coordinates(),
                }
            }
            DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Empty) => {
                DescriptorStoredOrigin::Empty
            }
            DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Literal { machine, place }) => {
                DescriptorStoredOrigin::Literal {
                    machine: *machine,
                    place: *place,
                }
            }
        };
        let state = self
            .replay
            .snapshots
            .get(&identity)?
            .as_ref()?
            .get(&position)?;
        self.replay.symbolic(
            machine,
            self.replay.substitute(machine, state, call, &exported)?,
        )
    }
    /// Substitute a demanded storage address without loading descriptor contents
    /// from the addressed slot. Only exact static ingress addresses are usable.
    pub fn substitute_call_storage(
        &mut self,
        identity: MachineId,
        position: DescriptorPosition,
        origin: &DescriptorCallOrigin,
    ) -> Option<DescriptorCallOrigin> {
        let DescriptorCallOrigin::Argument { source, projection } = origin else {
            return None;
        };
        if !projection.exact
            || projection.indirect
            || projection.window.is_some()
            || projection.captured_at.is_some()
        {
            return None;
        }
        self.replay.prepare_states(identity)?;
        let machine = self.replay.machine(identity)?;
        let call = machine.calls.get(&position)?;
        let callee = self.replay.machine(call.callee)?;
        let (parameter, _, ty) = self.replay.parameter(callee, *source)?;
        let relative = crate::descriptor_lifetimes::descriptor_location(
            self.replay.types,
            *source,
            ty,
            &projection.path,
            None,
        )?;
        let actual = self.replay.actual_slot(call, parameter, &relative.path)?;
        let before = self
            .replay
            .snapshots
            .get(&identity)?
            .as_ref()?
            .get(&position)?;
        // A typed endpoint does not turn a payload alias into carrier storage.
        // Reference forwarding supplies Storage origins; unresolved descriptor
        // payload aliases cannot invent an independently addressed root here.
        let storage = before
            .get(&DescriptorLocation::whole(actual.root))?
            .as_ref()?;
        if storage.is_empty()
            || storage
                .iter()
                .any(|origin| !matches!(origin, Origin::Storage(_)))
        {
            return None;
        }
        let mut actual = self.replay.canonical_storage(machine, before, &actual)?;
        if actual.path.is_empty() {
            let values = before.get(&actual)?.as_ref()?;
            let mut values = values.iter();
            if let Origin::Storage(location) = values.next()? {
                if values.next().is_some() {
                    return None;
                }
                actual = location.clone();
            } else {
                // A descriptor root denotes payload for reads, and a carrier
                // for storage writes. Without storage identity do not conflate them.
                return None;
            }
        }
        self.replay.storage_projection(machine, &actual)
    }
    pub fn call_origins(
        &mut self,
        identity: MachineId,
        result: PlaceId,
    ) -> Option<Vec<DescriptorCallOrigin>> {
        self.replay.prepare_states(identity)?;
        let machine = self.replay.machine(identity)?;
        let mut calls = machine.calls.iter().filter(|(_, call)| {
            call.result
                .as_ref()
                .is_some_and(|location| *location == DescriptorLocation::whole(result))
        });
        let (position, call) = calls.next()?;
        if calls.next().is_some() || call.arguments.iter().any(Option::is_none) {
            return None;
        }
        let states = self.replay.snapshots.get(&identity)?.as_ref()?;
        let summary = self.replay.summaries.get(&call.callee)?.as_ref()?;
        let mut origins = BTreeSet::new();
        for origin in summary.result.as_ref()? {
            origins.extend(
                self.replay
                    .substitute(machine, states.get(position)?, call, origin)?,
            );
        }
        self.replay.resolved(machine, origins)
    }
    pub fn stored_effects(&mut self, identity: MachineId) -> Option<Vec<DescriptorStoredEffect>> {
        Some(self.replay.summary(identity, 0)?.effects)
    }
    pub fn call_stored_effects(
        &mut self,
        identity: MachineId,
        position: DescriptorPosition,
    ) -> Option<Vec<DescriptorStoredCallEffect>> {
        let effects = self.symbolic_call_stored_effects(identity, position)?;
        let machine = self.replay.machine(identity)?;
        effects
            .into_iter()
            .map(|effect| {
                Some(DescriptorStoredCallEffect {
                    destination: effect.destination,
                    origins: effect
                        .origins
                        .into_iter()
                        .map(|origin| self.replay.flatten(machine, origin))
                        .collect::<Option<_>>()?,
                    preserves_incoming: effect.preserves_incoming,
                })
            })
            .collect()
    }
    pub fn symbolic_call_stored_effects(
        &mut self,
        identity: MachineId,
        position: DescriptorPosition,
    ) -> Option<Vec<DescriptorStoredCallEffect<DescriptorSourceOrigin>>> {
        self.replay.prepare_states(identity)?;
        let machine = self.replay.machine(identity)?;
        let states = self.replay.snapshots.get(&identity)?.as_ref()?;
        let before = states.get(&position)?;
        let call = machine.calls.get(&position)?;
        if call.arguments.iter().any(Option::is_none) {
            return None;
        }
        let summary = self.replay.summaries.get(&call.callee)?.as_ref()?;
        summary.effects.iter().map(|effect| {
            let destination = self.replay.canonical_storage(machine, before, &self.replay.actual_slot(call, effect.parameter, &effect.path)?)?;
            let preserves_incoming = effect.origins.iter().any(|origin| matches!(origin, DescriptorStoredOrigin::IncomingSlot { position, path } if *position == effect.parameter && *path == effect.path));
            let mut origins = BTreeSet::new();
            for origin in &effect.origins { origins.extend(self.replay.substitute(machine, before, call, origin)?); }
            Some(DescriptorStoredCallEffect { destination, origins: self.replay.symbolic(machine, origins)?, preserves_incoming })
        }).collect()
    }
    /// Materialize exactly the same replacements and call substitutions used
    /// by origin replay into the shared cohort graph. Consumers may attach
    /// independently reconstructed access/escape observations to this graph.
    pub fn flow(
        &mut self,
        identity: MachineId,
    ) -> Option<BTreeMap<DescriptorPosition, DescriptorFlowStep>> {
        self.replay.prepare_states(identity)?;
        let machine = self.replay.machine(identity)?;
        let states = self.replay.snapshots.get(&identity)?.as_ref()?;
        let mut flow = machine.flow.clone();
        for (position, step) in &mut flow {
            let Some(before) = states.get(position) else {
                continue;
            };
            let canonical = |location: &DescriptorLocation| {
                self.replay.canonical_storage(machine, before, location)
            };
            step.definitions = step
                .definitions
                .iter()
                .map(canonical)
                .collect::<Option<_>>()?;
            let aliases = before
                .keys()
                .filter_map(|location| {
                    let canonical_location = canonical(location)?;
                    step.definitions
                        .iter()
                        .any(|definition| definition.contains(&canonical_location))
                        .then_some(location.clone())
                })
                .collect::<Vec<_>>();
            step.definitions.extend(aliases);
            step.definitions.sort();
            step.definitions.dedup();
            // The reference carrier and the referent slot both remain observed.
            // Replacing one slot kills equivalent alias names, not copied values.
            let mut observations = step
                .inputs
                .iter()
                .map(canonical)
                .collect::<Option<Vec<_>>>()?;
            for input in &step.inputs {
                if input.path.is_empty()
                    && let Some(Some(origins)) = before.get(input)
                {
                    observations.extend(origins.iter().filter_map(|origin| match origin {
                        Origin::Storage(location) => Some(location.clone()),
                        _ => None,
                    }));
                }
            }
            step.inputs.extend(observations);
            step.inputs.sort();
            step.inputs.dedup();
            step.derivations = step
                .derivations
                .iter()
                .map(|(source, destination)| Some((canonical(source)?, canonical(destination)?)))
                .collect::<Option<_>>()?;
            step.authority_derivations = step
                .authority_derivations
                .iter()
                .map(|(source, destination)| Some((canonical(source)?, canonical(destination)?)))
                .collect::<Option<_>>()?;
            let Some(call) = machine
                .calls
                .get(position)
                .filter(|call| call.arguments.iter().all(Option::is_some))
            else {
                continue;
            };
            let summary = self.replay.summaries.get(&call.callee)?.as_ref()?;
            let mut outputs = summary
                .effects
                .iter()
                .map(|effect| {
                    Some((
                        self.replay
                            .actual_slot(call, effect.parameter, &effect.path)?,
                        effect.origins.as_slice(),
                    ))
                })
                .collect::<Option<Vec<_>>>()?;
            if let Some(result) = &call.result {
                outputs.push((result.clone(), summary.result.as_ref()?.as_slice()));
            }
            for (destination, origins) in outputs {
                let destination = self
                    .replay
                    .canonical_storage(machine, before, &destination)?;
                if !step.definitions.contains(&destination) {
                    step.definitions.push(destination.clone());
                }
                for location in before.keys() {
                    if let Some(canonical) =
                        self.replay.canonical_storage(machine, before, location)
                        && destination.contains(&canonical)
                        && !step.definitions.contains(location)
                    {
                        step.definitions.push(location.clone());
                    }
                }
                for origin in origins {
                    let source = match origin {
                        DescriptorStoredOrigin::Literal { .. } | DescriptorStoredOrigin::Empty => {
                            continue;
                        }
                        DescriptorStoredOrigin::IncomingSlot { position, path } => {
                            self.replay.actual_slot(call, *position, path)?
                        }
                        DescriptorStoredOrigin::Parameter {
                            position,
                            projection,
                        } => {
                            let mut source = call.arguments.get(*position)?.as_ref()?.clone();
                            if !projection.path.is_empty() {
                                let root_type = *machine.place_types.get(&source.root)?;
                                let mut spelling =
                                    descriptor_path(self.replay.types, root_type, &source.path)?;
                                spelling.extend_from_slice(&projection.path);
                                source = crate::descriptor_lifetimes::descriptor_location(
                                    self.replay.types,
                                    source.root,
                                    root_type,
                                    &spelling,
                                    None,
                                )?;
                            }
                            source
                        }
                    };
                    let pair = (
                        self.replay.canonical_storage(machine, before, &source)?,
                        destination.clone(),
                    );
                    step.derivations.push(pair.clone());
                    step.authority_derivations.push(pair);
                }
            }
        }
        Some(flow)
    }
}

pub fn reconstruct_flow(
    types: &[StructuralTypeDeclaration],
    machines: &[DescriptorMachine],
    identity: MachineId,
) -> Option<BTreeMap<DescriptorPosition, DescriptorFlowStep>> {
    DescriptorAnalysis::new(types, machines).flow(identity)
}
