use super::dispatch::join;
use super::*;
use crate::descriptors::descriptor_lifetimes;
use crate::descriptors::descriptor_lifetimes::DescriptorFlowSource;
use crate::descriptors::descriptor_lifetimes::descriptor_location_type;
use crate::exactly_one::exactly_one;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Origin {
    Provider(DescriptorProviderOrigin<Origin>),
    Backing(DescriptorCallOrigin),
    Incoming(DescriptorLocation),
    IncomingContents(DescriptorContentsQuery, Projection),
    ProducedContents(ProducedContents<Origin>),
    Storage(DescriptorLocation),
    /// A borrowed header captures an address account, not a fresh lookup
    /// through a descriptor that may subsequently be replaced.
    CapturedStorage(StorageResolution),
}
pub(crate) type Value = Option<BTreeSet<Origin>>;
pub(crate) type Slots = LocationMap<Value>;
#[derive(Clone, Default, PartialEq, Eq)]
pub(crate) struct State {
    pub(crate) cases: Arc<super::case_flow::CurrentCases>,
    pub(crate) slots: Arc<Slots>,
    /// Borrowed non-descriptor headers hold addresses independently of
    /// the values stored through them. Replacing a referent must not
    /// replace the header's captured backing with the replacement value.
    pub(crate) arrival_backing: Arc<Slots>,
    pub(crate) contents: Arc<BTreeMap<DescriptorContentsQuery, Value>>,
    pub(crate) contents_invalidated: bool,
}
pub(crate) type States = BTreeMap<DescriptorPosition, State>;
pub(crate) type InvocationStates =
    BTreeMap<(DescriptorPosition, DescriptorInvocationOwner), State>;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum StorageResolution {
    Unique(DescriptorLocation),
    CapturedEntry {
        location: DescriptorLocation,
        entry: DescriptorLocation,
    },
    Projected(DescriptorCallOrigin),
    Alternatives(Vec<DescriptorCallOrigin>),
    /// These preserve the logical-coordinate query without claiming
    /// that its current/opaque pointer is a complete captured address.
    OpaqueLocation(DescriptorLocation, &'static str),
    OpaqueProjection(DescriptorCallOrigin, &'static str),
    OpaqueAlternatives(Vec<DescriptorCallOrigin>, &'static str),
}

/// The singular coordinate visible to logical storage and origin queries.
/// Capture authority stays on the resolution and is checked by checked_storage_family.
pub(super) enum StorageCoordinate<'a> {
    Local(&'a DescriptorLocation),
    Projected(&'a DescriptorCallOrigin),
}

impl StorageCoordinate<'_> {
    pub(super) fn origin(self) -> Origin {
        match self {
            Self::Local(location) => Origin::Storage(location.clone()),
            Self::Projected(origin) => Origin::Backing(origin.clone()),
        }
    }

    fn projection(
        self,
        replay: &Replay<'_>,
        machine: &DescriptorMachine,
    ) -> Option<DescriptorCallOrigin> {
        match self {
            Self::Local(location) => replay.storage_projection(machine, location),
            Self::Projected(origin) => Some(origin.clone()),
        }
    }
}

impl StorageResolution {
    pub(super) fn singular_coordinate(&self) -> Option<StorageCoordinate<'_>> {
        match self {
            Self::Unique(location)
            | Self::CapturedEntry { location, .. }
            | Self::OpaqueLocation(location, _) => Some(StorageCoordinate::Local(location)),
            Self::Projected(origin) | Self::OpaqueProjection(origin, _) => {
                Some(StorageCoordinate::Projected(origin))
            }
            Self::Alternatives(_) | Self::OpaqueAlternatives(..) => None,
        }
    }

    pub(super) fn local_coordinate(&self) -> Option<&DescriptorLocation> {
        match self.singular_coordinate()? {
            StorageCoordinate::Local(location) => Some(location),
            StorageCoordinate::Projected(_) => None,
        }
    }

    /// Slot transfers and unknown-contents packets use the original carrier
    /// for a singular projected address. A family has no single carrier.
    pub(super) fn slot_coordinate(
        &self,
        carrier: &DescriptorLocation,
    ) -> Option<DescriptorLocation> {
        match self.singular_coordinate()? {
            StorageCoordinate::Local(location) => Some(location.clone()),
            StorageCoordinate::Projected(_) => Some(carrier.clone()),
        }
    }

    fn unavailable(&self) -> Option<&'static str> {
        match self {
            Self::OpaqueLocation(_, reason)
            | Self::OpaqueProjection(_, reason)
            | Self::OpaqueAlternatives(_, reason) => Some(*reason),
            _ => None,
        }
    }
}

/// Invalidation and semantic accesses are requested independently. A
/// missing component has not been prepared; a prepared None is unknown.
/// Sharing the prepared vectors keeps ordinary Summary clones cheap.
#[derive(Clone, Default)]
pub(crate) struct StorageFrame {
    pub(crate) writes: Option<Arc<Option<Vec<DescriptorLocation>>>>,
    pub(crate) accesses: Option<Arc<Option<Vec<(DescriptorPosition, usize)>>>>,
}

impl StorageFrame {
    pub(crate) fn unknown() -> Self {
        Self {
            writes: Some(Arc::new(None)),
            accesses: Some(Arc::new(None)),
        }
    }
}

#[derive(Clone)]
pub(crate) enum ResultObservations {
    Observed {
        slots: Option<Vec<DescriptorResultSlot>>,
        /// Whole unknown values retain their entry contents identity.
        /// These are value packets, never captured reference addresses.
        contents: Vec<DescriptorResultSlot>,
        /// These paths have no established spatial origin. Their flow
        /// conservatively retains every possible actual input loan;
        /// retaining a loan does not establish one input as the backing.
        unknown: Vec<Vec<DescriptorPathSegment>>,
    },
    /// Complete body replay observed no normal exit. This does not change
    /// the source-visible outcome or remove the caller's continuation.
    NoNormalBodyObservation,
}

impl ResultObservations {
    pub(crate) fn contents(&self) -> &[DescriptorResultSlot] {
        match self {
            Self::Observed { contents, .. } => contents,
            Self::NoNormalBodyObservation => &[],
        }
    }
    pub(crate) fn slots(&self) -> Option<&Vec<DescriptorResultSlot>> {
        match self {
            Self::Observed { slots, .. } => slots.as_ref(),
            Self::NoNormalBodyObservation => None,
        }
    }
    pub(crate) fn into_slots(self) -> Option<Vec<DescriptorResultSlot>> {
        match self {
            Self::Observed {
                slots, contents, ..
            } => contents.is_empty().then_some(slots)?,
            Self::NoNormalBodyObservation => None,
        }
    }
    pub(crate) fn unknown(&self) -> &[Vec<DescriptorPathSegment>] {
        match self {
            Self::Observed { unknown, .. } => unknown,
            Self::NoNormalBodyObservation => &[],
        }
    }
    pub(crate) fn is_unobserved(&self) -> bool {
        matches!(self, Self::NoNormalBodyObservation)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct UnobservedWrite {
    pub(crate) parameter: usize,
    pub(crate) steps: Vec<DescriptorContentsStep>,
}

#[derive(Clone)]
pub(crate) enum EffectObservations {
    Observed(Vec<DescriptorStoredEffect>),
    /// None is an unplaceable writable ceiling, never an empty write set.
    Unobserved(Option<Vec<UnobservedWrite>>),
}

impl EffectObservations {
    pub(crate) fn observed(&self) -> Option<&Vec<DescriptorStoredEffect>> {
        match self {
            Self::Observed(effects) => Some(effects),
            Self::Unobserved(_) => None,
        }
    }
    pub(crate) fn into_observed(self) -> Option<Vec<DescriptorStoredEffect>> {
        match self {
            Self::Observed(effects) => Some(effects),
            Self::Unobserved(_) => None,
        }
    }
}

#[derive(Clone)]
pub(crate) struct Summary {
    pub(crate) result: ResultObservations,
    pub(crate) result_cases: super::case_flow::ResultCases,
    pub(crate) effects: EffectObservations,
    pub(crate) invalidates_contents: bool,
    pub(crate) storage_frame: StorageFrame,
}

fn direct_target_summary(target: &DescriptorInvocationTarget) -> Option<Summary> {
    match target {
        DescriptorInvocationTarget::Machine(_) => None,
        DescriptorInvocationTarget::Provider {
            results,
            effects,
            invalidates_contents,
        } => Some(Summary {
            result: ResultObservations::Observed {
                slots: Some(results.clone()),
                contents: Vec::new(),
                unknown: Vec::new(),
            },
            result_cases: Default::default(),
            effects: EffectObservations::Observed(effects.clone()),
            invalidates_contents: *invalidates_contents,
            storage_frame: StorageFrame::unknown(),
        }),
        DescriptorInvocationTarget::Public {
            results,
            contents,
            invalidates_contents,
            ..
        } => Some(Summary {
            result: ResultObservations::Observed {
                slots: Some(results.clone()),
                contents: contents.clone(),
                unknown: Vec::new(),
            },
            result_cases: Default::default(),
            effects: EffectObservations::Observed(Vec::new()),
            invalidates_contents: *invalidates_contents,
            storage_frame: StorageFrame::unknown(),
        }),
    }
}

/// One completed preparation owns every observation of a machine. Keeping
/// these views together prevents a before-call history from being mistaken
/// for its cleanup-action or normal-exit history.
pub(crate) struct PreparedStates {
    pub(crate) before: Arc<States>,
    pub(crate) arrivals: States,
    pub(crate) invocations: InvocationStates,
    pub(crate) normal_exits: States,
}

pub(crate) enum PreparedStateCache {
    Preparing,
    Ready(PreparedStates),
    Failed(DescriptorGraphError),
}

impl PreparedStateCache {
    pub(crate) fn ready(&self) -> Option<&PreparedStates> {
        match self {
            Self::Ready(states) => Some(states),
            Self::Preparing | Self::Failed(_) => None,
        }
    }
}

// Plain data occupies its resident home. References, views and access
// records carry a location or permission for other storage.
fn carries_backing(shape: &StructuralTypeShape) -> bool {
    shape.is_borrowed_descriptor()
        || matches!(
            shape,
            StructuralTypeShape::Record {
                storage: terminal_psi::StructuralRecordStorage::AccessView(_)
                    | terminal_psi::StructuralRecordStorage::Accessor(_),
                ..
            }
        )
}

pub(crate) struct Replay<'a> {
    pub(crate) types: &'a [StructuralTypeDeclaration],
    pub(crate) slot_kind: DescriptorSlotKind,
    pub(crate) machines: &'a [DescriptorMachine],
    pub(crate) summaries: BTreeMap<MachineId, Option<Summary>>,
    pub(crate) snapshots: BTreeMap<MachineId, PreparedStateCache>,
}
#[derive(Default)]
pub(crate) struct DescriptorReadDepth<'state> {
    depth: std::cell::Cell<usize>,
    // A later result account can change a query that consulted this gap.
    pub(super) unprepared_summary: std::cell::Cell<bool>,
    pub(super) captured: std::cell::Cell<Option<(MachineId, DescriptorPosition)>>,
    pub(super) history: Option<(&'state DescriptorMachine, &'state States)>,
    pub(super) live: Option<(&'state DescriptorMachine, &'state State)>,
}

pub(crate) struct DescriptorReadGuard<'depth> {
    depth: &'depth std::cell::Cell<usize>,
    captured: &'depth std::cell::Cell<Option<(MachineId, DescriptorPosition)>>,
    previous: Option<(MachineId, DescriptorPosition)>,
    entered: bool,
}

impl<'state> DescriptorReadDepth<'state> {
    pub(super) fn absorb_summary_reads(&self, child: &DescriptorReadDepth<'_>) {
        if child.unprepared_summary.get() {
            self.unprepared_summary.set(true);
        }
    }

    pub(crate) fn at(
        machine: &'state DescriptorMachine,
        live: &'state State,
        history: Option<&'state States>,
    ) -> Self {
        Self {
            history: history.map(|history| (machine, history)),
            live: Some((machine, live)),
            ..Default::default()
        }
    }

    pub(crate) fn enter(&self) -> Option<DescriptorReadGuard<'_>> {
        let depth = self.depth.get();
        if depth >= 128 {
            return None;
        }
        self.depth.set(depth + 1);
        Some(DescriptorReadGuard {
            depth: &self.depth,
            captured: &self.captured,
            previous: self.captured.get(),
            entered: true,
        })
    }

    pub(crate) fn capture(
        &self,
        cut: Option<(MachineId, DescriptorPosition)>,
    ) -> DescriptorReadGuard<'_> {
        let guard = DescriptorReadGuard {
            depth: &self.depth,
            captured: &self.captured,
            previous: self.captured.get(),
            entered: false,
        };
        self.captured.set(cut);
        guard
    }
}

impl Drop for DescriptorReadGuard<'_> {
    fn drop(&mut self) {
        self.captured.set(self.previous);
        if self.entered {
            self.depth.set(self.depth.get() - 1);
        }
    }
}

impl<'a> Replay<'a> {
    pub(crate) fn new(
        types: &'a [StructuralTypeDeclaration],
        machines: &'a [DescriptorMachine],
    ) -> Self {
        Self::with_slot_kind(types, machines, DescriptorSlotKind::Views)
    }
    pub(crate) fn with_slot_kind(
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
        }
    }
    pub(crate) fn storage_writes(
        &mut self,
        identity: MachineId,
        position: DescriptorPosition,
    ) -> Result<Option<Vec<DescriptorLocation>>, &'static str> {
        let machine = self
            .machine(identity)
            .ok_or("storage write has no owning machine")?;
        if !machine.flow.contains_key(&position) {
            return Err("storage write has no operation or edge cut");
        }
        if self.prepare_states(identity).is_none() {
            return Ok(None);
        }
        let machine = self
            .machine(identity)
            .ok_or("storage write owner disappeared")?;
        let step = machine
            .flow
            .get(&position)
            .ok_or("storage write cut disappeared")?;
        if machine.unresolved_contents_writes.contains(&position) {
            return Ok(None);
        }
        if matches!(machine.calls.get(&position), Some(None)) {
            return Ok(None);
        }
        for call in machine.calls_at(position) {
            if self
                .invocation_state(identity, position, call.owner)
                .is_none()
            {
                continue;
            }
            let DescriptorInvocationTarget::Machine(callee) = &call.target else {
                return Ok(None);
            };
            self.prepare_write_frame(*callee)?;
        }
        let states = self
            .snapshots
            .get(&identity)
            .and_then(|cached| cached.ready())
            .map(|prepared| &prepared.before)
            .ok_or("storage write has no prepared flow")?;
        let Some(before) = states.get(&position) else {
            return Ok(Some(Vec::new()));
        };
        let mut writes = step.definitions.clone();
        // A known direct call uses its checked frame instead of the
        // conservative source-call ceiling. Cleanup remains additional to
        // the terminator's own writes and uses each action's real prestate.
        let queries = if machine.direct_call(position).is_some() {
            Vec::new()
        } else {
            let Some(queries) = super::case_flow::written_queries(machine, position) else {
                return Ok(None);
            };
            queries
        };
        for query in queries {
            let Some(root_type) = machine.place_types.get(&query.root) else {
                return Err("storage write has no declared root type");
            };
            if query.validate(self.types, *root_type).is_none() {
                return Err("storage write query does not follow its declared type");
            }
            // An indexed write covers its collection. Do not enumerate
            // every possible element to infer that conservative region.
            // A view's descriptor is not its backing, so an unresolved
            // view element cannot be replaced by the descriptor's address.
            let prefix = query.observation();
            if prefix.path.len() < query.steps.len() {
                let Some(selected) =
                    descriptor_location_type(self.types, *root_type, &prefix.path)
                else {
                    return Err("storage write has no declared collection prefix");
                };
                if !find_structural_type(self.types, selected).is_some_and(|declaration| {
                    matches!(declaration.shape, StructuralTypeShape::FixedArray { .. })
                }) {
                    return Ok(None);
                }
                // Coarsening a fixed-array element write may discard
                // inline fields and indices, but never a dereference into
                // an element's reference or view backing.
                let Some(inline) = self.inline_steps(machine, query) else {
                    return Ok(None);
                };
                if inline[prefix.path.len()..].contains(&false) {
                    return Ok(None);
                }
            }
            let Some(targets) = self.contents_targets(
                machine,
                before,
                &DescriptorContentsQuery::from_location(&prefix),
            ) else {
                return Ok(None);
            };
            if targets.is_empty() {
                return Ok(None);
            }
            for target in targets {
                let DescriptorPayloadTarget::Storage {
                    location,
                    completed: None,
                } = target
                else {
                    return Ok(None);
                };
                writes.push(location);
            }
        }
        let mut located = Vec::with_capacity(writes.len());
        for write in writes {
            let Some(write) = self.canonical_storage(machine, before, &write, None) else {
                return Ok(None);
            };
            located.push(write);
        }
        for call in machine.calls_at(position) {
            let Some(before) = self.invocation_state(identity, position, call.owner) else {
                // An infeasible cleanup action has no executable effects.
                continue;
            };
            let DescriptorInvocationTarget::Machine(callee) = &call.target else {
                return Ok(None);
            };
            let Some(frame) = self
                .summaries
                .get(callee)
                .and_then(Option::as_ref)
                .and_then(|summary| summary.storage_frame.writes.as_ref())
                .and_then(|writes| writes.as_ref().as_ref())
            else {
                return Ok(None);
            };
            let callee = self
                .machine(*callee)
                .ok_or("write frame call has no callee")?;
            for written in frame {
                let Some((parameter, _, _)) = self.parameter(callee, written.root) else {
                    return Err("published write does not name a callee formal");
                };
                let Some(actual) = self.actual_slot(call, parameter, &written.path) else {
                    return Ok(None);
                };
                let Some(actual) = self.canonical_storage(machine, before, &actual, None)
                else {
                    return Ok(None);
                };
                located.push(actual);
            }
        }
        located.sort_unstable();
        located.dedup();
        Ok(Some(located))
    }

    pub(crate) fn prepare_write_frame(
        &mut self,
        identity: MachineId,
    ) -> Result<(), &'static str> {
        if self.machine(identity).is_none() {
            return Err("write frame has no owning machine");
        }
        if self.summary(identity, 0).is_none() {
            return Ok(());
        }
        let summary = self
            .summaries
            .get_mut(&identity)
            .and_then(Option::as_mut)
            .ok_or("write frame has no prepared body summary")?;
        if summary.storage_frame.writes.is_some() {
            return Ok(());
        }
        // A recursive dependency sees an unknown frame. It cannot turn
        // incomplete inference into an empty set of writes.
        summary.storage_frame.writes = Some(Arc::new(None));
        let writes = self.infer_write_frame(identity)?;
        self.summaries
            .get_mut(&identity)
            .and_then(Option::as_mut)
            .ok_or("write frame body summary disappeared")?
            .storage_frame
            .writes = Some(Arc::new(writes));
        Ok(())
    }

    pub(crate) fn prepare_access_frame(
        &mut self,
        identity: MachineId,
    ) -> Result<(), &'static str> {
        if self.machine(identity).is_none() {
            return Err("access frame has no owning machine");
        }
        if self.summary(identity, 0).is_none() {
            return Ok(());
        }
        let summary = self
            .summaries
            .get_mut(&identity)
            .and_then(Option::as_mut)
            .ok_or("access frame has no prepared body summary")?;
        if summary.storage_frame.accesses.is_some() {
            return Ok(());
        }
        summary.storage_frame.accesses = Some(Arc::new(None));
        let accesses = self.infer_access_frame(identity)?;
        self.summaries
            .get_mut(&identity)
            .and_then(Option::as_mut)
            .ok_or("access frame body summary disappeared")?
            .storage_frame
            .accesses = Some(Arc::new(accesses));
        Ok(())
    }

    fn infer_access_frame(
        &mut self,
        identity: MachineId,
    ) -> Result<Option<Vec<(DescriptorPosition, usize)>>, &'static str> {
        if self.prepare_states(identity).is_none() {
            return Ok(None);
        }
        let machine = self
            .machine(identity)
            .ok_or("storage frame owner disappeared")?;
        // Borrow the graph's inputs and retain only their coordinates.
        // Queries stay at their original pre-operation cut; a caller
        // substitutes through each actual invocation when consuming it.
        let positions: Vec<_> = self
            .snapshots
            .get(&identity)
            .and_then(|cached| cached.ready())
            .ok_or("storage frame has no prepared flow")?
            .before
            .keys()
            .copied()
            .collect();
        let mut accesses = Vec::new();
        for position in positions {
            let step = machine
                .flow
                .get(&position)
                .ok_or("storage access cut disappeared")?;
            for (input, (_, access)) in step.inputs.iter().enumerate() {
                if access.is_some() {
                    accesses.push((position, input));
                }
            }
            let Some(invocation) = machine.calls.get(&position) else {
                continue;
            };
            let Some(invocations) = invocation else {
                return Ok(None);
            };
            for invocation in invocations {
                if self
                    .invocation_state(identity, position, invocation.owner)
                    .is_none()
                {
                    continue;
                }
                // Public and provider calls need their retained interface
                // effect owner. A possible private implementation is not it.
                let Some(callee) = invocation.callee() else {
                    return Ok(None);
                };
                self.prepare_access_frame(callee)?;
                let complete = self
                    .summaries
                    .get(&callee)
                    .and_then(Option::as_ref)
                    .and_then(|summary| summary.storage_frame.accesses.as_ref())
                    .is_some_and(|accesses| accesses.as_ref().is_some());
                if !complete {
                    return Ok(None);
                }
            }
        }
        Ok(Some(accesses))
    }

    fn infer_write_frame(
        &mut self,
        identity: MachineId,
    ) -> Result<Option<Vec<DescriptorLocation>>, &'static str> {
        if self.prepare_states(identity).is_none() {
            return Ok(None);
        }
        let machine = self
            .machine(identity)
            .ok_or("write frame owner disappeared")?;
        let positions: Vec<_> = self
            .snapshots
            .get(&identity)
            .and_then(|cached| cached.ready())
            .map(|prepared| &prepared.before)
            .ok_or("write frame has no prepared storage flow")?
            .keys()
            .copied()
            .collect();
        let mut frame = Vec::new();
        for position in positions {
            let Some(writes) = self.storage_writes(identity, position)? else {
                return Ok(None);
            };
            for write in writes {
                let Some(place) =
                    exactly_one(machine.places.iter(), |row| row.place == write.root)
                else {
                    return Err("write frame has no unique storage origin");
                };
                match place.definition {
                    DescriptorDefinition::Local => continue,
                    DescriptorDefinition::Parameter {
                        access: StructuralAccess::Owned,
                        ..
                    } if self
                        .inline_steps(machine, &DescriptorContentsQuery::from_location(&write))
                        .is_some_and(|steps| steps.into_iter().all(|inline| inline)) =>
                    {
                        // By-value arguments have private working storage.
                        // Keep its writes in the local flow, but publish
                        // only effects that can reach the caller's storage.
                        // A reference or view crossing is not inline.
                        continue;
                    }
                    DescriptorDefinition::Parameter { .. } => {}
                    DescriptorDefinition::Literal | DescriptorDefinition::Arrival { .. } => {
                        return Ok(None);
                    }
                }
                frame.push(write);
            }
        }
        frame.sort_unstable();
        let mut normalized: Vec<DescriptorLocation> = Vec::with_capacity(frame.len());
        for write in frame {
            if !normalized
                .last()
                .is_some_and(|prior| prior.contains(&write))
            {
                normalized.push(write);
            }
        }
        Ok(Some(normalized))
    }

    pub(crate) fn machine(&self, identity: MachineId) -> Option<&'a DescriptorMachine> {
        let mut rows = self
            .machines
            .iter()
            .filter(|machine| machine.id == identity);
        let row = rows.next()?;
        rows.next().is_none().then_some(row)
    }
    pub(crate) fn parameter(
        &self,
        machine: &'a DescriptorMachine,
        root: PlaceId,
    ) -> Option<(usize, StructuralAccess, StructuralTypeId)> {
        let row = exactly_one(machine.places.iter(), |row| row.place == root)?;
        match row.definition {
            DescriptorDefinition::Parameter {
                position,
                access,
                structural_type,
            } => Some((position, access, structural_type)),
            _ => None,
        }
    }
    pub(crate) fn storage_projection(
        &self,
        machine: &DescriptorMachine,
        location: &DescriptorLocation,
    ) -> Option<DescriptorCallOrigin> {
        let path = machine.location_path(self.types, location, None)?;
        Some(DescriptorCallOrigin::Argument {
            source: location.root,
            projection: Projection::new(&path),
        })
    }
    pub(crate) fn checked_storage_family(
        &self,
        machine: &DescriptorMachine,
        _state: &State,
        resolution: StorageResolution,
    ) -> Result<Vec<DescriptorCallOrigin>, &'static str> {
        if let Some(reason) = resolution.unavailable() {
            return Err(reason);
        }
        let origins = match resolution {
            StorageResolution::Unique(location)
            | StorageResolution::CapturedEntry { location, .. } => vec![
                self.storage_projection(machine, &location)
                    .ok_or("captured storage location has no exact declared path")?,
            ],
            StorageResolution::Projected(origin) => vec![origin],
            StorageResolution::Alternatives(origins) => origins,
            StorageResolution::OpaqueLocation(..)
            | StorageResolution::OpaqueProjection(..)
            | StorageResolution::OpaqueAlternatives(..) => {
                unreachable!("unavailable family was refused")
            }
        };
        if origins.is_empty() {
            return Err("captured storage family is empty");
        }
        for origin in &origins {
            let DescriptorCallOrigin::Argument { source, projection } = origin else {
                return Err("captured storage family has no original storage source");
            };
            let ty = *machine
                .place_types
                .get(source)
                .ok_or("captured storage source is absent or stale")?;
            machine
                .place_definition(*source)
                .ok_or("captured storage source has no current declaration")?;
            if projection.has_captured_reference_crossing()?
                && !self.slot_kind.tracks_references()
            {
                return Err("captured storage crossing needs reference-tracking replay");
            }
            let path = projection
                .validate_captured_storage_address(|at| machine.flow.contains_key(&at))?;
            if let Some(path) = path {
                // A byte view's selected element has a stored address
                // without a separate structural declaration.
                let tip = crate::runtime_read_projection_tip(self.types.iter(), ty, &path);
                if tip.is_none()
                    && crate::runtime_structural_field(self.types.iter(), ty, &path).is_none()
                {
                    return Err("captured storage source has no exact declared path");
                }
            } else if projection.has_captured_reference_crossing()? {
                return Err("captured storage window has an unresolved reference crossing");
            }
        }
        Ok(origins)
    }

    pub(crate) fn flow_location(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        location: &DescriptorLocation,
        resolution: Option<StorageResolution>,
    ) -> Result<DescriptorLocation, &'static str> {
        match resolution.ok_or(INEXACT_REFERENCE_BACKING)? {
            StorageResolution::Unique(location)
            | StorageResolution::CapturedEntry { location, .. }
            | StorageResolution::OpaqueLocation(location, _) => Ok(location),
            resolution @ (StorageResolution::Projected(_)
            | StorageResolution::Alternatives(_)) => {
                self.checked_storage_family(machine, state, resolution)?;
                // A reference variable keeps its own lifetime when different
                // paths give it different captured addresses. Track that
                // variable here; the address family cannot establish that
                // another variable holds the same reference.
                Ok(location.clone())
            }
            // Unknown contents still have a local variable whose uses keep
            // its possible input loans live. This grants no address authority.
            StorageResolution::OpaqueProjection(..) => Ok(location.clone()),
            StorageResolution::OpaqueAlternatives(_, reason) => Err(reason),
        }
    }

    pub(crate) fn canonical_storage(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        location: &DescriptorLocation,
        completed: Option<(MachineId, DescriptorPosition)>,
    ) -> Option<DescriptorLocation> {
        self.resolve_storage(machine, state, location, completed)?
            .local_coordinate()
            .cloned()
    }

    pub(crate) fn canonical_storage_with_depth(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        location: &DescriptorLocation,
        depth: &DescriptorReadDepth<'_>,
    ) -> Option<DescriptorLocation> {
        self.resolve_storage_with_depth(machine, state, location, depth)?
            .local_coordinate()
            .cloned()
    }

    pub(crate) fn reference_backing_matches(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        backing: &DescriptorLocation,
        reference: StructuralTypeId,
        depth: &DescriptorReadDepth<'_>,
    ) -> Option<bool> {
        let path = self.resolved_location(machine, state, backing, depth)?.0;
        let root = *machine.place_types.get(&backing.root)?;
        Some(crate::reference_source_projection_compatible(
            self.types.iter(),
            root,
            &path,
            reference,
        ))
    }

    pub(crate) fn resolve_storage(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        location: &DescriptorLocation,
        completed: Option<(MachineId, DescriptorPosition)>,
    ) -> Option<StorageResolution> {
        let depth = DescriptorReadDepth::at(machine, state, None);
        let _capture = depth.capture(completed);
        self.resolve_storage_with_depth(machine, state, location, &depth)
    }

    fn root_storage_origins<'state>(
        &self,
        machine: &DescriptorMachine,
        state: &'state State,
        root: PlaceId,
    ) -> Option<&'state Value> {
        let ty = *machine.place_types.get(&root)?;
        if matches!(machine.place_definition(root)?, DescriptorDefinition::Arrival { access } if *access != StructuralAccess::Owned)
            && !is_descriptor(self.types.iter(), ty)
        {
            state.arrival_backing.get(&DescriptorLocation::whole(root))
        } else {
            state.slots.get(&DescriptorLocation::whole(root))
        }
    }

    pub(crate) fn resolve_storage_with_depth(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        location: &DescriptorLocation,
        depth: &DescriptorReadDepth<'_>,
    ) -> Option<StorageResolution> {
        self.resolve_storage_branch(
            machine,
            state,
            location.clone(),
            depth,
            None,
            BTreeMap::new(),
        )
    }

    fn expand_storage(
        expansions: &mut BTreeMap<DescriptorLocation, usize>,
        equation: DescriptorLocation,
        remaining: usize,
    ) -> Option<()> {
        if expansions
            .insert(equation, remaining)
            .is_some_and(|previous| remaining >= previous)
        {
            None
        } else {
            Some(())
        }
    }

    fn storage_alternatives(
        &self,
        machine: &DescriptorMachine,
        resolutions: impl IntoIterator<Item = StorageResolution>,
    ) -> Option<StorageResolution> {
        let mut origins = BTreeSet::new();
        let mut unavailable = None;
        for resolution in resolutions {
            unavailable = unavailable.or(resolution.unavailable());
            match resolution {
                StorageResolution::Unique(location)
                | StorageResolution::CapturedEntry { location, .. }
                | StorageResolution::OpaqueLocation(location, _) => {
                    origins.insert(self.storage_projection(machine, &location)?);
                }
                StorageResolution::Projected(origin) | StorageResolution::OpaqueProjection(origin, _) => {
                    origins.insert(origin);
                }
                StorageResolution::Alternatives(alternatives)
                | StorageResolution::OpaqueAlternatives(alternatives, _) => origins.extend(alternatives),
            }
        }
        match (origins.len(), unavailable) {
            (0, _) => None,
            (1, None) => origins.into_iter().next().map(StorageResolution::Projected),
            (1, Some(reason)) => origins
                .into_iter()
                .next()
                .map(|origin| StorageResolution::OpaqueProjection(origin, reason)),
            (_, None) => Some(StorageResolution::Alternatives(
                origins.into_iter().collect(),
            )),
            (_, Some(reason)) => Some(StorageResolution::OpaqueAlternatives(
                origins.into_iter().collect(),
                reason,
            )),
        }
    }

    fn captured_storage_origins(&self, resolution: StorageResolution) -> BTreeSet<Origin> {
        match resolution {
            StorageResolution::Unique(location)
                if !location.path.contains(&DescriptorPathSegment::Referent) =>
            {
                BTreeSet::from([Origin::Storage(location)])
            }
            StorageResolution::Projected(origin) if matches!(&origin, DescriptorCallOrigin::Argument { projection, .. } if projection.has_captured_reference_crossing() == Ok(false)) => {
                BTreeSet::from([Origin::Backing(origin)])
            }
            StorageResolution::Alternatives(origins) => origins
                .into_iter()
                .flat_map(|origin| {
                    self.captured_storage_origins(StorageResolution::Projected(origin))
                })
                .collect(),
            StorageResolution::OpaqueAlternatives(origins, reason) => origins
                .into_iter()
                .map(|origin| {
                    Origin::CapturedStorage(StorageResolution::OpaqueProjection(origin, reason))
                })
                .collect(),
            other => BTreeSet::from([Origin::CapturedStorage(other)]),
        }
    }

    fn storage_account_matches_reference(
        &self,
        machine: &DescriptorMachine,
        account: &StorageResolution,
        reference: StructuralTypeId,
    ) -> Option<bool> {
        // This is a declared-type check. append_storage_capture retains the
        // original capture or its reason for being opaque.
        let origin = account.singular_coordinate()?.projection(self, machine)?;
        let DescriptorCallOrigin::Argument { source, projection } = origin else {
            return None;
        };
        let path = projection.captured_address_path().ok()?;
        Some(crate::reference_source_projection_compatible(
            self.types.iter(),
            *machine.place_types.get(&source)?,
            &path,
            reference,
        ))
    }

    /// An already captured account never consults a replacement pointer for
    /// its prefix. Only a newly projected crossing needs current lineage.
    fn append_storage_capture(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        account: &StorageResolution,
        suffix: &[StructuralPathSegment],
        depth: &DescriptorReadDepth<'_>,
        expansions: BTreeMap<DescriptorLocation, usize>,
    ) -> Option<StorageResolution> {
        let (origin, entry, unavailable) = match account {
            StorageResolution::Unique(location) => {
                (self.storage_projection(machine, location)?, None, None)
            }
            StorageResolution::CapturedEntry { location, entry } => (
                self.storage_projection(machine, location)?,
                Some(entry.clone()),
                None,
            ),
            StorageResolution::OpaqueLocation(location, reason) => (
                self.storage_projection(machine, location)?,
                None,
                Some(*reason),
            ),
            StorageResolution::Projected(origin) => (origin.clone(), None, None),
            StorageResolution::OpaqueProjection(origin, reason) => {
                (origin.clone(), None, Some(*reason))
            }
            _ => return None,
        };
        let DescriptorCallOrigin::Argument { source, projection } = origin else {
            return None;
        };
        let composed = DescriptorCallOrigin::Argument {
            source,
            projection: projection.then(&Projection::new(suffix)),
        };
        if let Some(reason) = unavailable {
            if matches!(account, StorageResolution::OpaqueLocation(..)) {
                let DescriptorCallOrigin::Argument { projection, .. } = &composed else {
                    return None;
                };
                if let Ok(path) = projection.captured_address_path()
                    && let Some(location) = crate::descriptor_lifetimes::descriptor_location(
                        self.types,
                        source,
                        *machine.place_types.get(&source)?,
                        &path,
                        None,
                    )
                {
                    return Some(StorageResolution::OpaqueLocation(location, reason));
                }
            }
            return Some(StorageResolution::OpaqueProjection(composed, reason));
        }
        if !suffix.contains(&StructuralPathSegment::Referent) {
            if let Some(entry) = entry {
                let DescriptorCallOrigin::Argument { projection, .. } = &composed else {
                    return None;
                };
                let path = projection.captured_address_path().ok()?;
                let location = crate::descriptor_lifetimes::descriptor_location(
                    self.types,
                    source,
                    *machine.place_types.get(&source)?,
                    &path,
                    None,
                )?;
                return Some(StorageResolution::CapturedEntry { location, entry });
            }
            return Some(StorageResolution::Projected(composed));
        }
        let base = projection.captured_address_path().ok()?;
        let ty = *machine.place_types.get(&source)?;
        let target = crate::descriptor_lifetimes::descriptor_location(
            self.types,
            source,
            ty,
            &projection
                .then(&Projection::new(suffix))
                .captured_address_path()
                .ok()?,
            None,
        );
        let Some(target) = target else {
            return Some(StorageResolution::OpaqueProjection(
                composed,
                "captured suffix crossing has no exact descriptor coordinate",
            ));
        };
        let entry = entry.or_else(|| {
            let crossing = base
                .iter()
                .rposition(|step| *step == StructuralPathSegment::Referent)?;
            crate::descriptor_lifetimes::descriptor_location(
                self.types,
                source,
                ty,
                &base[..crossing],
                None,
            )
        });
        self.resolve_storage_branch(machine, state, target, depth, entry, expansions)
    }

    /// Formation and forwarding interpret the same captured values.
    /// Each alternative keeps its own expansion history and entry account.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn captured_value_storage(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        current: &DescriptorLocation,
        origins: &BTreeSet<Origin>,
        depth: &DescriptorReadDepth<'_>,
        captured_entry: Option<DescriptorLocation>,
        expansions: BTreeMap<DescriptorLocation, usize>,
        unavailable: Option<&'static str>,
    ) -> Option<StorageResolution> {
        let suffix_path = machine.location_path(self.types, current, None)?;
        let suffix = Projection::new(&suffix_path);
        let mut resolutions = Vec::with_capacity(origins.len());
        for origin in origins {
            let (source, prefix) = match origin {
                Origin::CapturedStorage(account) => {
                    resolutions.push(self.append_storage_capture(
                        machine,
                        state,
                        account,
                        &suffix_path,
                        depth,
                        expansions.clone(),
                    )?);
                    continue;
                }
                Origin::Storage(backing) => {
                    let target =
                        self.project_storage_origin(machine, state, current, backing, depth)?;
                    resolutions.push(self.resolve_storage_branch(
                        machine,
                        state,
                        target,
                        depth,
                        captured_entry.clone(),
                        expansions.clone(),
                    )?);
                    continue;
                }
                Origin::Backing(DescriptorCallOrigin::Argument { source, projection }) => {
                    (*source, projection.clone())
                }
                Origin::IncomingContents(query, projection) => {
                    let location = query.exact_location()?;
                    let DescriptorCallOrigin::Argument {
                        source,
                        projection: prefix,
                    } = self.storage_projection(machine, &location)?
                    else {
                        return None;
                    };
                    (source, prefix.then(projection))
                }
                _ => return None,
            };
            let origin = DescriptorCallOrigin::Argument {
                source,
                projection: prefix.then(&suffix),
            };
            let reason =
                unavailable.or_else(|| {
                    prefix.has_captured_reference_crossing().unwrap_or(true).then_some(
                        "captured indirect projection has no retained entry-reference account")
                });
            resolutions.push(match reason {
                Some(reason) => StorageResolution::OpaqueProjection(origin, reason),
                None => StorageResolution::Projected(origin),
            });
        }
        if resolutions.len() == 1 {
            resolutions.pop()
        } else {
            self.storage_alternatives(machine, resolutions)
        }
    }

    /// Each branch owns its expansion history. Sharing a visited set between
    /// alternatives would mistake reconvergence for a cycle and drop a real source.
    fn resolve_storage_branch(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        mut current: DescriptorLocation,
        depth: &DescriptorReadDepth<'_>,
        mut captured_entry: Option<DescriptorLocation>,
        mut expansions: BTreeMap<DescriptorLocation, usize>,
    ) -> Option<StorageResolution> {
        let _guard = depth.enter()?;
        let mut unavailable = None;
        loop {
            let root_type = *machine.place_types.get(&current.root)?;
            let definition = machine.place_definition(current.root)?;
            if current.path.is_empty() && definition.has_independent_storage() {
                return Some(match unavailable {
                    Some(reason) => StorageResolution::OpaqueLocation(current, reason),
                    None => StorageResolution::Unique(current),
                });
            }
            // A machine parameter names the lender's original home. A
            // whole-value store replaces its contents, not that address;
            // following the stored value would retarget retained loans to
            // the consumed replacement. Block arrivals do transport their
            // backing address, as do actual descriptor values.
            let aliases_backing = matches!(
                definition,
                DescriptorDefinition::Arrival { access }
                    if *access != StructuralAccess::Owned
            ) || matches!(
                find_structural_type(self.types, root_type)?.shape,
                StructuralTypeShape::Reference { .. }
            ) || is_descriptor(self.types.iter(), root_type);
            let borrowed_arrival = matches!(definition, DescriptorDefinition::Arrival { access } if *access != StructuralAccess::Owned)
                && !is_descriptor(self.types.iter(), root_type);
            let origins = self.root_storage_origins(machine, state, current.root);
            if borrowed_arrival
                && origins
                    .and_then(Option::as_ref)
                    .is_none_or(|origins| origins.is_empty())
            {
                return None;
            }
            if borrowed_arrival
                && let Some(Some(origins)) = origins
                && origins
                    .iter()
                    .any(|origin| !matches!(origin, Origin::Storage(_)))
            {
                Self::expand_storage(
                    &mut expansions,
                    DescriptorLocation::whole(current.root),
                    current.path.len(),
                )?;
                return self.captured_value_storage(
                    machine,
                    state,
                    &current,
                    origins,
                    depth,
                    captured_entry,
                    expansions,
                    unavailable,
                );
            }

            // An Incoming origin names the entry value, not whatever its
            // original descriptor home holds after a later reassignment.
            let captured_crossing = captured_entry
                .as_ref()
                .filter(|entry| {
                    current.root == entry.root
                        && current.path.starts_with(&entry.path)
                        && current.path.get(entry.path.len())
                            == Some(&DescriptorPathSegment::Referent)
                })
                .map(|entry| entry.path.len());
            if aliases_backing
                && captured_crossing.is_none()
                && let Some(Some(origins)) = origins
                && !origins.is_empty()
                && !origins.iter().all(|origin| matches!(origin, Origin::Storage(backing) if *backing == DescriptorLocation::whole(current.root)))
                && origins
                    .iter()
                    .all(|origin| matches!(origin, Origin::Storage(_)))
                && !(matches!(find_structural_type(self.types, root_type)?.shape, StructuralTypeShape::ElementView { .. })
                    && origins.iter().all(|origin| matches!(origin, Origin::Storage(backing)
                        if self.has_roster_source(machine, backing, depth))))
            {
                let locations = self.storage_origins_with_depth(machine, state, &current, depth)?;
                let first = locations.first()?.clone();
                if locations.iter().any(|candidate| *candidate != first) {
                    Self::expand_storage(&mut expansions,
                        DescriptorLocation::whole(current.root), current.path.len())?;
                    let resolutions = locations.into_iter().map(|target| {
                        self.resolve_storage_branch(machine, state, target, depth,
                            captured_entry.clone(), expansions.clone())
                    }).collect::<Option<Vec<_>>>()?;
                    return self.storage_alternatives(machine, resolutions);
                }
                if first != current {
                    Self::expand_storage(&mut expansions,
                        DescriptorLocation::whole(current.root), current.path.len())?;
                    current = first;
                    continue;
                }
            }
            let mut next = None;
            for (position, segment) in current.path.iter().enumerate() {
                if *segment != DescriptorPathSegment::Referent {
                    continue;
                }
                if captured_crossing.is_some_and(|captured| position <= captured) {
                    continue;
                }
                let prefix = DescriptorLocation {
                    root: current.root,
                    path: current.path[..position].to_vec(),
                };
                let Some(Some(origins)) = state.slots.get(&prefix) else {
                    unavailable
                        .get_or_insert("reference descriptor has no current origin account");
                    continue;
                };
                if origins.is_empty() {
                    unavailable.get_or_insert("reference descriptor origin account is empty");
                    continue;
                }
                let reference = self.resolved_location(machine, state, &prefix, depth)?.1?;
                if !matches!(
                    find_structural_type(self.types, reference)?.shape,
                    StructuralTypeShape::Reference { .. }
                ) {
                    return None;
                }
                if origins.len() == 1 && origins.contains(&Origin::Incoming(prefix.clone())) {
                    captured_entry = Some(prefix.clone());
                    continue;
                }
                if origins.len() == 1 && origins.contains(&Origin::Storage(prefix.clone())) {
                    unavailable
                        .get_or_insert("reference descriptor names only its storage home");
                    continue;
                }
                if origins.iter().any(|origin| {
                    !matches!(
                        origin,
                        Origin::Storage(_)
                            | Origin::Incoming(_)
                            | Origin::IncomingContents(_, _)
                            | Origin::Backing(DescriptorCallOrigin::Argument { .. })
                            | Origin::CapturedStorage(_)
                    )
                }) {
                    unavailable.get_or_insert(
                        "reference descriptor has no supported captured address account",
                    );
                    continue;
                }
                let mut targets = Vec::with_capacity(origins.len());
                let mut projected = Vec::new();
                for origin in origins {
                    let mut next_entry = None;
                    let mut target = match origin {
                        Origin::Backing(DescriptorCallOrigin::Argument {
                            source,
                            projection,
                        }) => {
                            // A formed reference can retain an indexed address
                            // that has no descriptor-slot location. Crossing it
                            // uses that exact captured address, not its carrier home.
                            let full = machine.location_path(self.types, &current, None)?;
                            let prefix_path =
                                machine.location_path(self.types, &prefix, None)?;
                            if full.get(prefix_path.len())
                                != Some(&StructuralPathSegment::Referent)
                            {
                                return None;
                            }
                            let suffix = &full[prefix_path.len() + 1..];
                            let target_path = match projection.captured_address_path() {
                                Ok(path) => path,
                                Err(_) => {
                                    unavailable.get_or_insert(
                                        "reference backing needs captured finite geometry",
                                    );
                                    continue;
                                }
                            };
                            let target_type = *machine.place_types.get(source)?;
                            if !crate::reference_source_projection_compatible(
                                self.types.iter(),
                                target_type,
                                &target_path,
                                reference,
                            ) {
                                unavailable.get_or_insert(
                                    "reference backing changes its declared referee type",
                                );
                                continue;
                            }
                            if projection.has_captured_reference_crossing().unwrap_or(true) {
                                unavailable.get_or_insert(
                                    "reference backing has no retained captured-entry identity",
                                );
                                continue;
                            }
                            projected.push(self.append_storage_capture(
                                machine,
                                state,
                                &StorageResolution::Projected(DescriptorCallOrigin::Argument {
                                    source: *source,
                                    projection: projection.clone(),
                                }),
                                suffix,
                                depth,
                                expansions.clone(),
                            )?);
                            continue;
                        }
                        Origin::CapturedStorage(account) => {
                            if self
                                .storage_account_matches_reference(machine, account, reference)
                                != Some(true)
                            {
                                unavailable.get_or_insert("captured reference backing changes or cannot identify its declared referee type");
                                continue;
                            }
                            let full = machine.location_path(self.types, &current, None)?;
                            let prefix_path =
                                machine.location_path(self.types, &prefix, None)?;
                            projected.push(self.append_storage_capture(
                                machine,
                                state,
                                account,
                                &full[prefix_path.len() + 1..],
                                depth,
                                expansions.clone(),
                            )?);
                            continue;
                        }
                        Origin::Storage(backing) => backing.clone(),
                        Origin::IncomingContents(query, projection) => {
                            if *projection != Projection::new(&[]) || state.contents_invalidated
                            {
                                unavailable.get_or_insert("incoming reference contents has no unchanged exact entry account");
                                continue;
                            }
                            let Some(descriptor) = query.exact_location() else {
                                unavailable.get_or_insert("incoming reference contents has no exact descriptor coordinate");
                                continue;
                            };
                            let (_, _, root_type) = self.parameter(machine, descriptor.root)?;
                            query.validate(self.types, root_type)?;
                            let source = self
                                .resolved_location(machine, state, &descriptor, depth)?
                                .1?;
                            if !matches!(
                                find_structural_type(self.types, source)?.shape,
                                StructuralTypeShape::Reference { .. }
                            ) {
                                return None;
                            }
                            let mut backing = descriptor.clone();
                            backing.path.push(DescriptorPathSegment::Referent);
                            next_entry = Some(descriptor);
                            backing
                        }
                        Origin::Incoming(descriptor) => {
                            // A returned or copied incoming reference still names
                            // that entry descriptor's referent, not the temporary
                            // that now stores its bits. Preserve the crossing.
                            let source = self
                                .resolved_location(machine, state, descriptor, depth)?
                                .1?;
                            if !matches!(
                                find_structural_type(self.types, source)?.shape,
                                StructuralTypeShape::Reference { .. }
                            ) {
                                return None;
                            }
                            let mut backing = descriptor.clone();
                            backing.path.push(DescriptorPathSegment::Referent);
                            next_entry = Some(descriptor.clone());
                            backing
                        }
                        _ => return None,
                    };
                    if !self
                        .reference_backing_matches(machine, state, &target, reference, depth)?
                    {
                        return None;
                    }
                    target.path.extend_from_slice(&current.path[position + 1..]);
                    self.resolved_location(machine, state, &target, depth)?;
                    let target = (target, next_entry);
                    if !targets.contains(&target) {
                        targets.push(target);
                    }
                }
                if unavailable.is_some() {
                    continue;
                }
                Self::expand_storage(
                    &mut expansions,
                    prefix,
                    current.path.len() - position - 1,
                )?;
                if !projected.is_empty() || targets.len() > 1 {
                    for (target, entry) in targets {
                        projected.push(self.resolve_storage_branch(
                            machine,
                            state,
                            target,
                            depth,
                            entry.or_else(|| captured_entry.clone()),
                            expansions.clone(),
                        )?);
                    }
                    return if projected.len() == 1 {
                        projected.pop()
                    } else {
                        self.storage_alternatives(machine, projected)
                    };
                }
                let (target, entry) = targets.pop()?;
                next = Some(target);
                if let Some(entry) = entry {
                    captured_entry = Some(entry);
                }
                break;
            }
            match next {
                Some(next) if next != current => current = next,
                Some(_) => return None,
                None => {
                    return Some(match unavailable {
                        Some(reason) => StorageResolution::OpaqueLocation(current, reason),
                        None => match captured_entry {
                            Some(entry) => StorageResolution::CapturedEntry {
                                location: current,
                                entry,
                            },
                            None => StorageResolution::Unique(current),
                        },
                    });
                }
            }
        }
    }
    pub(crate) fn storage_origins(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        location: &DescriptorLocation,
    ) -> Option<Vec<DescriptorLocation>> {
        self.storage_origins_with_depth(
            machine,
            state,
            location,
            &DescriptorReadDepth::at(machine, state, None),
        )
    }

    fn storage_origins_with_depth(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        location: &DescriptorLocation,
        depth: &DescriptorReadDepth<'_>,
    ) -> Option<Vec<DescriptorLocation>> {
        // A store can fill a vacant descriptor home before its contents
        // name any backing. Its declaration still identifies the slot.
        if location.path.is_empty()
            && machine
                .place_definition(location.root)?
                .has_independent_storage()
        {
            return Some(vec![location.clone()]);
        }
        let origins = self
            .root_storage_origins(machine, state, location.root)?
            .as_ref()?;
        if origins.is_empty() {
            return None;
        }
        if location.path.is_empty() {
            let mut locations = Vec::new();
            for origin in origins {
                let Origin::Storage(backing) = origin else {
                    return None;
                };
                if !locations.contains(backing) {
                    locations.push(backing.clone());
                }
            }
            return (!locations.is_empty()).then_some(locations);
        }
        let mut locations = Vec::with_capacity(origins.len());
        for origin in origins {
            let Origin::Storage(backing) = origin else {
                return None;
            };
            let target =
                self.project_storage_origin(machine, state, location, backing, depth)?;
            if !locations.contains(&target) {
                locations.push(target);
            }
        }
        Some(locations)
    }
    fn project_storage_origin(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        location: &DescriptorLocation,
        backing: &DescriptorLocation,
        depth: &DescriptorReadDepth<'_>,
    ) -> Option<DescriptorLocation> {
        if location.path.is_empty() {
            self.resolved_location(machine, state, backing, depth)?;
            return Some(backing.clone());
        }
        let mut suffix = location.path.as_slice();
        let root_type = *machine.place_types.get(&location.root)?;
        if matches!(
            find_structural_type(self.types, root_type)?.shape,
            StructuralTypeShape::Reference { .. }
        ) && self.reference_backing_matches(machine, state, backing, root_type, depth)?
        {
            let (first, rest) = suffix.split_first()?;
            if *first != DescriptorPathSegment::Referent {
                return None;
            }
            suffix = rest;
        }
        let mut target = backing.clone();
        target.path.extend_from_slice(suffix);
        self.resolved_location(machine, state, &target, depth)?;
        Some(target)
    }

    pub(crate) fn reference_referent_carrier(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        location: &DescriptorLocation,
        depth: &DescriptorReadDepth<'_>,
    ) -> Option<Option<DescriptorLocation>> {
        if !self.slot_kind.tracks_references() {
            return Some(None);
        }
        let Some((DescriptorPathSegment::Referent, path)) = location.path.split_last() else {
            return Some(None);
        };
        let endpoint = self.resolved_location(machine, state, location, depth)?.1?;
        Some(
            (!self.slot_kind.is_slot(self.types, endpoint)
                && !is_descriptor(self.types.iter(), endpoint))
            .then(|| DescriptorLocation {
                root: location.root,
                path: path.to_vec(),
            }),
        )
    }

    pub(crate) fn read(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        location: &DescriptorLocation,
    ) -> Value {
        self.read_with_depth(
            machine,
            state,
            location,
            &DescriptorReadDepth::at(machine, state, None),
        )
    }

    pub(crate) fn read_with_depth(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        location: &DescriptorLocation,
        depth: &DescriptorReadDepth<'_>,
    ) -> Value {
        self.read_slot(machine, state, location, depth, true)
    }

    fn read_slot(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        location: &DescriptorLocation,
        depth: &DescriptorReadDepth<'_>,
        include_selected: bool,
    ) -> Value {
        let _read = depth.enter()?;
        if location.path.last() == Some(&DescriptorPathSegment::UnknownContents) {
            let ty = self.unknown_contents_type(machine, location)?;
            if !matches!(
                find_structural_type(self.types, ty)?.shape,
                StructuralTypeShape::TypeParameter
            ) {
                return None;
            }
            return state.slots.get(location)?.clone();
        }
        if location.path.is_empty()
            && matches!(machine.place_definition(location.root)?, DescriptorDefinition::Arrival { access } if *access != StructuralAccess::Owned)
            && !is_descriptor(self.types.iter(), *machine.place_types.get(&location.root)?)
            && !self
                .slot_kind
                .is_slot(self.types, *machine.place_types.get(&location.root)?)
        {
            return state.arrival_backing.get(location)?.clone();
        }
        let resolution = self.resolve_storage_with_depth(machine, state, location, depth)?;
        let location = resolution.local_coordinate().unwrap_or(location).clone();
        let root = DescriptorLocation::whole(location.root);
        if let Some(carrier) =
            self.reference_referent_carrier(machine, state, &location, depth)?
        {
            return self.read_with_depth(machine, state, &carrier, depth);
        }
        let external_pointee =
            state
                .slots
                .get(&root)
                .and_then(Option::as_ref)
                .is_some_and(|origins| {
                    origins.iter().any(|origin| {
                    matches!(origin, Origin::Storage(backing) if backing.root != root.root)
                })
                });
        if let Some(value) = state.slots.get(&location)
            && (value.is_some() || location.path.is_empty() || !external_pointee)
        {
            return if include_selected {
                state.selected_inline_contents(&location, value.clone())
            } else {
                value.clone()
            };
        }
        if self.slot_kind.tracks_references()
            && location.path.contains(&DescriptorPathSegment::Referent)
            && self
                .resolved_location(machine, state, &location, depth)
                .and_then(|(_, endpoint)| endpoint)
                .is_some_and(|endpoint| self.slot_kind.is_slot(self.types, endpoint))
        {
            return self.read_contents_with_depth(
                machine,
                state,
                &DescriptorContentsQuery::from_location(&location),
                depth,
            );
        }
        if self.slot_kind == DescriptorSlotKind::AccessOrigins {
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
                        | Origin::IncomingContents(_, _)
                        | Origin::Backing(DescriptorCallOrigin::AccessFormation { .. })
                )
            }) {
                return Some(origins.clone());
            }
            return None;
        }
        if !external_pointee
            && state.slots.iter().any(|(prefix, value)| {
                value.is_none()
                    && prefix.root == location.root
                    && !prefix.path.is_empty()
                    && location.path.starts_with(&prefix.path)
            })
        {
            return None;
        }
        let path = self.resolved_location(machine, state, &location, depth)?.0;
        let projection = Projection::new(&path);
        state
            .slots
            .get(&root)?
            .as_ref()?
            .iter()
            .map(|origin| match origin {
                Origin::ProducedContents(_) => None,
                Origin::CapturedStorage(account) => {
                    let values = self.project(
                        machine,
                        Some(BTreeSet::from([Origin::CapturedStorage(account.clone())])),
                        Some(&projection),
                    )?;
                    (values.len() == 1)
                        .then(|| values.into_iter().next())
                        .flatten()
                }
                Origin::Storage(backing) if backing == &root => {
                    Some(Origin::Storage(location.clone()))
                }
                Origin::Storage(backing) if backing.root != location.root => {
                    let suffix = match location.path.first() {
                        Some(DescriptorPathSegment::Referent) => &location.path[1..],
                        _ => location.path.as_slice(),
                    };
                    let mut target = backing.clone();
                    target.path.extend_from_slice(suffix);
                    self.resolved_location(machine, state, &target, depth)?;
                    Some(Origin::Storage(target))
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
    pub(crate) fn initial(&self, machine: &DescriptorMachine) -> Option<State> {
        let mut state = State::default();
        for row in &machine.places {
            state
                .slots_mut()
                .insert(DescriptorLocation::whole(row.place), None);
            let inventory = if self.slot_kind.tracks_references() {
                self.slot_kind.owned_inventory(
                    self.types,
                    row.place,
                    *machine.place_types.get(&row.place)?,
                )?
            } else {
                self.slot_kind.inventory(
                    self.types,
                    row.place,
                    *machine.place_types.get(&row.place)?,
                )?
            };
            for slot in inventory.slots.iter().chain(
                inventory
                    .unknown_contents
                    .iter()
                    .map(|unknown| &unknown.location),
            ) {
                state.slots_mut().insert(slot.clone(), None);
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
                let origin = if self
                    .slot_kind
                    .parameter_is_incoming_slot(self.types, structural_type)
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
                state.slots_mut().insert(
                    DescriptorLocation::whole(row.place),
                    Some(BTreeSet::from([origin])),
                );
                for slot in inventory.slots {
                    if !slot.path.is_empty() {
                        state.slots_mut().insert(
                            slot.clone(),
                            Some(BTreeSet::from([Origin::Incoming(slot)])),
                        );
                    }
                }
                if access != StructuralAccess::WriteOnlyBorrow {
                    for contents in inventory.unknown_contents {
                        if contents.kind == UnknownContentsKind::Value {
                            let location = contents.location;
                            state.slots_mut().insert(
                                location.clone(),
                                Some(BTreeSet::from([Origin::Incoming(location)])),
                            );
                        }
                    }
                }
            }
        }
        Some(state)
    }
    pub(crate) fn project(
        &self,
        machine: &DescriptorMachine,
        value: Value,
        projection: Option<&Projection>,
    ) -> Value {
        let Some(projection) = projection else {
            return value;
        };
        if *projection == Projection::new(&[]) {
            return value;
        }
        value?
            .into_iter()
            .map(|origin| match origin {
                Origin::ProducedContents(_) => None,
                Origin::CapturedStorage(account) => {
                    let reason = account.unavailable().or_else(|| {
                        projection.has_captured_reference_crossing().unwrap_or(true).then_some(
                        "projected captured reference crossing needs its actual prestate")
                    });
                    let origin = account.singular_coordinate()?.origin();
                    let mut values = self.project(
                        machine,
                        Some(BTreeSet::from([origin])),
                        Some(projection),
                    )?;
                    let Origin::Backing(origin) = values.pop_first()? else {
                        return None;
                    };
                    Some(Origin::CapturedStorage(match reason {
                        Some(reason) => StorageResolution::OpaqueProjection(origin, reason),
                        None => StorageResolution::Projected(origin),
                    }))
                }
                Origin::Storage(location) => {
                    match self.storage_projection(machine, &location)? {
                        DescriptorCallOrigin::Argument {
                            source,
                            projection: prefix,
                        } => {
                            let composed = prefix.then(projection);
                            if composed.exact
                                && !composed.indirect
                                && composed.window.is_none()
                                && composed.captured_at.is_none()
                            {
                                // Exact projections keep their original
                                // storage identity, including any checked
                                // presentation step between its fields.
                                crate::descriptor_lifetimes::descriptor_location(
                                    self.types,
                                    source,
                                    *machine.place_types.get(&source)?,
                                    &composed.path,
                                    None,
                                )
                                .map(Origin::Storage)
                            } else {
                                Some(Origin::Backing(DescriptorCallOrigin::Argument {
                                    source,
                                    projection: composed,
                                }))
                            }
                        }
                        _ => None,
                    }
                }
                Origin::Backing(DescriptorCallOrigin::Invocation {
                    carrier_type,
                    projection: prefix,
                }) => Some(Origin::Backing(DescriptorCallOrigin::Invocation {
                    carrier_type,
                    projection: prefix.then(projection),
                })),
                Origin::Provider(mut provider) => {
                    provider.projection = provider.projection.then(projection);
                    Some(Origin::Provider(provider))
                }
                Origin::Backing(DescriptorCallOrigin::Argument {
                    source,
                    projection: prefix,
                }) => Some(Origin::Backing(DescriptorCallOrigin::Argument {
                    source,
                    projection: prefix.then(projection),
                })),
                Origin::Backing(DescriptorCallOrigin::AccessFormation {
                    machine,
                    operation,
                    place,
                    carrier,
                    projection: prefix,
                }) => Some(Origin::Backing(DescriptorCallOrigin::AccessFormation {
                    machine,
                    operation,
                    place,
                    carrier,
                    projection: prefix.then(projection),
                })),
                literal @ Origin::Backing(
                    DescriptorCallOrigin::Literal { .. } | DescriptorCallOrigin::Empty,
                ) => Some(literal),
                Origin::Incoming(location) => Some(Origin::IncomingContents(
                    DescriptorContentsQuery::from_location(&location),
                    projection.clone(),
                )),
                Origin::IncomingContents(query, prefix) => {
                    Some(Origin::IncomingContents(query, prefix.then(projection)))
                }
            })
            .collect()
    }
    pub(crate) fn transfer<'locations>(
        &self,
        machine: &DescriptorMachine,
        before: &State,
        definitions: &[DescriptorLocation],
        consumed: &[DescriptorLocation],
        derivations: impl Clone
        + Iterator<
            Item = (
                &'locations DescriptorLocation,
                &'locations DescriptorLocation,
            ),
        >,
        transfer: super::case_flow::Transfer<'_>,
        case_storage: &super::case_flow::Storage,
    ) -> Option<State> {
        self.transfer_states(
            machine,
            (before, before),
            definitions,
            consumed,
            derivations,
            transfer,
            case_storage,
        )
    }
    pub(crate) fn captured_source_value(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        source: &DescriptorLocation,
        projection: Option<&Projection>,
        reads: &DescriptorReadDepth<'_>,
    ) -> Value {
        let storage =
            || {
                Some(self.captured_storage_origins(
                    self.resolve_storage(machine, state, source, None)?,
                ))
            };
        let Some(projection) = projection else {
            return storage();
        };
        let ty = self.resolved_location(machine, state, source, reads)?.1?;
        let value = if is_descriptor(self.types.iter(), ty) {
            self.read_with_depth(machine, state, source, reads)
        } else {
            storage()
        };
        self.project(machine, value, Some(projection))
    }

    /// Capture the address that a reference or inline collection view
    /// borrows. Replacing a resident value never changes that address.
    fn capture_storage(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        source: &DescriptorLocation,
        reads: &DescriptorReadDepth<'_>,
    ) -> Value {
        if let Some(carrier) = self.reference_referent_carrier(machine, state, source, reads)? {
            return self.read_with_depth(machine, state, &carrier, reads);
        }
        Some(self.captured_storage_origins(
            self.resolve_storage_with_depth(machine, state, source, reads)?,
        ))
    }

    fn location_carries_backing(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        location: &DescriptorLocation,
        reads: &DescriptorReadDepth<'_>,
    ) -> Option<bool> {
        let (path, endpoint) = self.resolved_location(machine, state, location, reads)?;
        if let Some(endpoint) = endpoint {
            return Some(carries_backing(
                &find_structural_type(self.types, endpoint)?.shape,
            ));
        }
        // Inline scalar and byte-view fields do not need separate type
        // declarations. Only a borrowed view field carries a payload.
        let field = crate::runtime_structural_field(
            self.types.iter(),
            *machine.place_types.get(&location.root)?,
            &path,
        )?;
        Some(
            field
                .field_type
                .byte_sequence_carrier()
                .is_some_and(|carrier| carrier.is_borrowed()),
        )
    }

    fn transfer_states<'locations>(
        &self,
        machine: &DescriptorMachine,
        states: (&State, &State),
        definitions: &[DescriptorLocation],
        consumed: &[DescriptorLocation],
        derivations: impl Clone
        + Iterator<
            Item = (
                &'locations DescriptorLocation,
                &'locations DescriptorLocation,
            ),
        >,
        transfer: super::case_flow::Transfer<'_>,
        case_storage: &super::case_flow::Storage,
    ) -> Option<State> {
        let (before, base) = states;
        let reads = transfer.reads();
        let position = transfer.position();
        let canonical = |location: &DescriptorLocation| {
            if position.is_some() {
                self.resolve_storage(machine, before, location, None)?
                    .slot_coordinate(location)
            } else {
                Some(location.clone())
            };
        // Moving an owned parameter ends its name for loan liveness, but
        // its entry value stays readable here: no operation reads a moved
        // value, and the machine's summary still reads the parameter at
        // exit to report what the caller's storage behind a reference it
        // held contains. Other consumed carriers are forgotten.
        let mut overwritten = definitions.to_vec();
        for consumed in consumed {
            let parameter = machine.places.iter().any(|row| {
                row.place == consumed.root
                    && matches!(row.definition, DescriptorDefinition::Parameter { .. })
            });
            if !parameter {
                continue;
            }
            let position = overwritten
                .iter()
                .position(|definition| definition == consumed)?;
            overwritten.swap_remove(position);
        }
        let definitions = overwritten
            .iter()
            .map(canonical)
            .collect::<Option<Vec<_>>>()?;
        let mut assigned = Slots::new();
        for (source, destination) in derivations.clone() {
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
                let value = self.capture_storage(machine, before, source, reads);
                merge_value(
                    &mut assigned,
                    canonical(destination)?,
                    self.project(machine, value, projection),
                );
                continue;
            }
            let contents_query = position.and_then(|position| {
                machine
                    .contents_queries
                    .get(&(position, source.clone(), destination.clone()))
            });
            let value = match contents_query {
                Some(query) => self.read_contents_with_depth(machine, before, query, reads),
                None => self.read_with_depth(machine, before, source, reads),
            };
            let value = self.project(machine, value, projection);
            let destination = canonical(destination)?;
            merge_value(&mut assigned, destination.clone(), value);
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
                        self.project(
                            machine,
                            self.read_with_depth(machine, before, location, reads),
                            projection,
                        ),
                    );
                }
            }
        }
        let mut after = base.clone();
        after.cases = super::case_flow::transport(
            self,
            machine,
            (before, base),
            &definitions,
            derivations,
            transfer,
            case_storage,
        )?;
        if !definitions.is_empty() {
            for (location, value) in base.slots.iter() {
                if value.is_some()
                    && !assigned.contains_key(location)
                    && definitions
                        .iter()
                        .any(|definition| definition.contains(location))
                {
                    after.slots_mut().insert(location.clone(), None);
                }
            }
            for definition in &definitions {
                if !assigned.contains_key(definition)
                    && after.slots.get(definition) != Some(&None)
                {
                    after.slots_mut().insert(definition.clone(), None);
                }
            }
        }
        for (location, value) in assigned {
            if after.slots.get(&location) != Some(&value) {
                after.slots_mut().insert(location, value);
            }
        }
        Some(after)
    }
    #[allow(clippy::too_many_arguments)]
    fn transfer_edge(
        &self,
        machine: &DescriptorMachine,
        before: &State,
        base: &State,
        edge: &descriptor_lifetimes::DescriptorFlowEdge,
        case_storage: &super::case_flow::Storage,
        reads: &DescriptorReadDepth<'_>,
    ) -> Option<State> {
        let definitions = edge
            .parameters
            .iter()
            .chain(&edge.definitions)
            .cloned()
            .collect::<Vec<_>>();
        let mut after = self.transfer_states(
            machine,
            (before, base),
            &definitions,
            &[],
            edge.derivations(),
            super::case_flow::Transfer::Arrival { edge, reads },
            case_storage,
        )?;
        // A captured indexed address is an exact projection, even when
        // it cannot be expressed as a descriptor-slot location. Forward
        // that address before installing the destination header.
        let storage_origin =
            |source: &DescriptorLocation| {
                Some(self.captured_storage_origins(
                    self.resolve_storage(machine, before, source, None)?,
                ))
            };
        for binding in &edge.bindings {
            let query = match &binding.source {
                DescriptorFlowSource::Location(location) => {
                    Some(DescriptorContentsQuery::from_location(location))
                }
                DescriptorFlowSource::Contents(query) => Some(query.clone()),
                DescriptorFlowSource::Unavailable(_) => None,
            };
            let structural_type = *machine.place_types.get(&binding.parameter.root)?;
            // The address of a borrowed scalar need not have a descriptor
            // contents slot. Retain its captured address independently;
            // nested descriptor contents still require their own query.
            let borrowed_storage = binding.access != StructuralAccess::Owned
                && !is_descriptor(self.types.iter(), structural_type);
            if borrowed_storage || query.is_some() {
                let value = if borrowed_storage {
                    if let Some((descriptor, projection)) = &binding.captured_storage {
                        self.captured_source_value(
                            machine,
                            before,
                            descriptor,
                            Some(projection),
                            reads,
                        )
                    } else {
                        binding.source.exact_location().and_then(&storage_origin)
                    }
                } else {
                    self.read_contents_with_depth(machine, before, query.as_ref()?, reads)
                };
                let mut captured = Slots::new();
                if borrowed_storage {
                    Arc::make_mut(&mut after.arrival_backing)
                        .insert(binding.parameter.clone(), value);
                } else {
                    captured.insert(binding.parameter.clone(), value);
                }

                for slot in self.slot_kind.owned_slots(
                    self.types,
                    binding.parameter.root,
                    structural_type,
                )? {
                    if slot == binding.parameter {
                        continue;
                    }
                    let mut source = query.as_ref()?.clone();
                    source.steps.extend(
                        slot.path
                            .iter()
                            .cloned()
                            .map(DescriptorContentsStep::Address),
                    );
                    captured.insert(
                        slot,
                        self.read_contents_with_depth(machine, before, &source, reads),
                    );
                }
                for (location, value) in captured {
                    if after.slots.get(&location) != Some(&value) {
                        after.slots_mut().insert(location, value);
                    }
                }
            }
            if binding.access != StructuralAccess::Owned {
                continue;
            }
            let parameter = &binding.parameter;
            if !parameter.path.is_empty() {
                return None;
            }
            let structural_type = *machine.place_types.get(&parameter.root)?;
            let shape = &find_structural_type(self.types, structural_type)?.shape;
            if carries_backing(shape) {
                continue;
            }
            let value = Some(BTreeSet::from([Origin::Storage(parameter.clone())]));
            if after.slots.get(parameter) != Some(&value) {
                after.slots_mut().insert(parameter.clone(), value);
            }
        }
        Some(after)
    }
    pub(crate) fn unobserved_effect_replacements(
        &self,
        machine: &DescriptorMachine,
        before: &State,
        call: &DescriptorInvocation,
        writes: &Option<Vec<UnobservedWrite>>,
        reads: &DescriptorReadDepth<'_>,
        mut retire: impl FnMut(&DescriptorContentsQuery),
    ) -> Option<BTreeMap<DescriptorPayloadTarget, Value>> {
        let mut replacements = BTreeMap::new();
        for write in writes.as_ref()? {
            let query = self.actual_contents(machine, call, write.parameter, &write.steps)?;
            retire(&query);
            self.contents_replacements_with_depth(
                machine,
                before,
                &query,
                None,
                &mut replacements,
                reads,
            )?;
        }
        Some(replacements)
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_invocation(
        &mut self,
        machine: &DescriptorMachine,
        before: &State,
        after: &mut State,
        call: &DescriptorInvocation,
        depth: usize,
        case_storage: &super::case_flow::Storage,
        reads: &DescriptorReadDepth<'_>,
    ) -> Result<(), &'static str> {
        let summary = self
            .invocation_summary(call, depth + 1)
            .ok_or("the callee's borrowed-storage summary is unavailable")?;
        after.contents_invalidated |= summary.invalidates_contents;
        if summary.invalidates_contents {
            after.cases = Arc::default();
            let mut actuals = call.arguments.iter().flatten().cloned().collect::<Vec<_>>();
            actuals.extend(
                call.argument_contents
                    .iter()
                    .flatten()
                    .map(DescriptorContentsQuery::observation),
            );
            let changed = after
                .slots
                .keys()
                .filter(|location| {
                    location.path.last() == Some(&DescriptorPathSegment::UnknownContents)
                        && actuals.iter().any(|actual| actual.contains(location))
                })
                .cloned()
                .collect::<Vec<_>>();
            // The value remains established, but an unknown mutation
            // cannot preserve its entry contents as an exact snapshot.
            for location in changed {
                after.slots_mut().insert(location, None);
            }
        }
        if let Some(result) = &call.result
            && summary.result.is_unobserved()
        {
            // Ordinary observed calls keep canonical definition/contract
            // handling. This account has no normal observation to refill
            // the retired result, including when a loop reuses its home.
            let ty = *machine
                .place_types
                .get(&result.root)
                .ok_or("the call result has no declared storage type")?;
            self.slot_kind
                .owned_slots(self.types, result.root, ty)
                .ok_or("the call result's borrowed fields cannot be identified")?;
            after
                .slots_mut()
                .retain(|stored, _| !result.contains(stored));
            after.slots_mut().insert(result.clone(), None);
            after
                .contents_mut()
                .retain(|query, _| !result.contains(&query.observation()));
            super::case_flow::retire_location(after, result);
        }
        let mut replacements = match &summary.effects {
            EffectObservations::Observed(_) => BTreeMap::new(),
            EffectObservations::Unobserved(writes) => {
                let unknown = self
                    .unobserved_effect_replacements(
                        machine,
                        before,
                        call,
                        writes,
                        reads,
                        |query| {
                            super::case_flow::retire_query(
                                self,
                                machine,
                                before,
                                after,
                                query,
                                case_storage,
                                reads,
                            );
                        },
                    )
                    .ok_or("cannot locate a write through a borrowed call argument")?;
                if unknown.keys().any(|target| {
                    matches!(
                        target,
                        DescriptorPayloadTarget::Storage {
                            completed: Some(_),
                            ..
                        }
                    )
                }) {
                    return Err("a call write targets an earlier result");
                }
                unknown
            }
        };
        let mut destinations = Vec::new();
        for effect in summary.effects.observed().into_iter().flatten() {
            if !effect.origins.is_empty()
                && effect
                    .origins
                    .iter()
                    .all(|origin| effect.keeps_incoming_slot(origin))
                && effect.origins.iter().any(|origin| {
                    self.substitute_with_depth(machine, before, call, origin, reads)
                        .is_none()
                })
            {
                continue;
            }
            let destination = self
                .actual_contents(machine, call, effect.parameter, &effect.steps)
                .ok_or("a callee write has no corresponding caller argument")?;
            super::case_flow::retire_query(
                self,
                machine,
                before,
                after,
                &destination,
                case_storage,
                reads,
            );
            let targets = self
                .contents_targets_with_depth(machine, before, &destination, reads)
                .ok_or("cannot locate the storage a borrowed call argument writes")?;
            if targets.iter().any(|target| {
                matches!(
                    target,
                    DescriptorPayloadTarget::Storage {
                        completed: Some(_),
                        ..
                    }
                )
            }) {
                return Err("a call write targets an earlier result");
            }
            let strong = targets.len() == 1
                && matches!(
                    targets.first(),
                    Some(DescriptorPayloadTarget::Storage {
                        completed: None,
                        ..
                    })
                );
            for target in targets {
                if let DescriptorPayloadTarget::Storage {
                    location,
                    completed: None,
                } = target
                {
                    if destinations.iter().any(
                        |(previous, definite): &(DescriptorLocation, bool)| {
                            (strong || *definite)
                                && (location.contains(previous) || previous.contains(&location))
                        },
                    ) {
                        return Err("two call writes overlap in the caller");
                    }
                    destinations.push((location, strong));
                }
            }
            let mut origins = BTreeSet::new();
            let mut dropped_retired_incoming = false;
            for origin in &effect.origins {
                match self.substitute_with_depth(machine, before, call, origin, reads) {
                    Some(substituted) => origins.extend(substituted),
                    None if effect.keeps_incoming_slot(origin) => {
                        dropped_retired_incoming = true;
                    }
                    None => {
                        return Err(
                            "a reference written by the callee has no source in this call",
                        );
                    }
                }
            }
            if origins.is_empty() && dropped_retired_incoming {
                continue;
            }
            self.contents_replacements_with_depth(
                machine,
                before,
                &destination,
                Some(origins),
                &mut replacements,
                reads,
            )
            .ok_or("cannot record the references written by the callee")?;
        }
        if let Some(result) = &call.result {
            let result_type = *machine
                .place_types
                .get(&result.root)
                .ok_or("the call result has no declared storage type")?;
            let empty = Vec::new();
            let slots = if summary.result.is_unobserved() {
                &empty
            } else {
                summary
                    .result
                    .slots()
                    .ok_or("the callee has no account of its returned references")?
            };
            for slot in slots {
                slot.visit(
                    result_type,
                    &mut Vec::new(),
                    &mut |path, containing_type, slot| {
                        let mut destination = result.clone();
                        destination.path.extend_from_slice(path);
                        descriptor_path(self.types, containing_type, &slot.path).ok_or(())?;
                        let mut origins = BTreeSet::new();
                        let mut known = true;
                        for origin in &slot.origins {
                            if matches!(origin, DescriptorStoredOrigin::Carried { .. }) {
                                origins.insert(Origin::Storage(destination.clone()));
                                continue;
                            }
                            match self.substitute_result_with_depth(
                                machine, before, call, origin, reads,
                            ) {
                                Some(substituted) => origins.extend(substituted),
                                None => {
                                    known = false;
                                    break;
                                }
                            }
                        }
                        let incoming = known.then_some(origins);
                        match replacements
                            .get_mut(&DescriptorPayloadTarget::current(destination.clone()))
                        {
                            None => {
                                replacements.insert(
                                    DescriptorPayloadTarget::current(destination),
                                    incoming,
                                );
                            }
                            Some(existing) => match (existing.as_mut(), incoming) {
                                (Some(existing), Some(incoming)) => existing.extend(incoming),
                                _ => *existing = None,
                            },
                        }
                        Ok::<(), ()>(())
                    },
                )
                .map_err(
                    |()| "a returned reference does not match the result's declared type",
                )?;
            }
            for packet in summary.result.contents() {
                let mut destination = result.clone();
                destination.path.extend_from_slice(&packet.path);
                let expected = self
                    .unknown_contents_type(machine, &destination)
                    .ok_or("borrowed contents do not match the result's declared type")?;
                let mut origins = BTreeSet::new();
                for origin in &packet.origins {
                    if let DescriptorStoredOrigin::ProducedContents(produced) = origin {
                        if produced.structural_type != expected {
                            return Err("returned contents changed their generic value type");
                        }
                        let dependencies = self
                            .substitute_lifetime_dependencies(
                                machine,
                                before,
                                call,
                                &produced.lifetime_dependencies,
                                reads,
                            )
                            .ok_or("returned contents do not retain their input loans")?;
                        origins.insert(Origin::ProducedContents(
                            produced.with_dependencies(dependencies),
                        ));
                        continue;
                    }
                    origins.extend(
                        self.substitute_result_with_depth(machine, before, call, origin, reads)
                            .ok_or("returned borrowed contents have no source in this call")?,
                    );
                }
                if origins.is_empty() {
                    if !summary
                        .result_cases
                        .excludes_payload(result_type, &packet.path)
                    {
                        return Err(
                            "a reachable result contains borrowed storage with no source",
                        );
                    }
                    // Absence supplies no contents or access. The result's
                    // case facts below keep this payload unreachable.
                    replacements.insert(DescriptorPayloadTarget::current(destination), None);
                } else {
                    replacements
                        .insert(DescriptorPayloadTarget::current(destination), Some(origins));
                }
            }
            let unknown = if summary.result.is_unobserved() {
                self.slot_kind
                    .owned_slots(self.types, result.root, result_type)
                    .ok_or("the call result's borrowed fields cannot be identified")?
                    .into_iter()
                    .map(|slot| slot.path)
                    .collect::<Vec<_>>()
            } else {
                summary.result.unknown().to_vec()
            };
            for path in &unknown {
                let mut destination = result.clone();
                destination.path.extend_from_slice(path);
                replacements.insert(DescriptorPayloadTarget::current(destination), None);
            }
        }
        Self::install_contents(after, replacements);
        if let Some(result) = &call.result {
            summary
                .result_cases
                .install(self, machine, after, result)
                .ok_or("the callee's returned cases do not match the call result")?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_cleanup_calls(
        &mut self,
        machine: &DescriptorMachine,
        position: DescriptorPosition,
        edge: Option<semantic_vocabulary::EdgeId>,
        mut current: State,
        history: &States,
        depth: usize,
        case_storage: &super::case_flow::Storage,
        snapshots: &mut InvocationStates,
    ) -> Option<State> {
        let Some(edge) = edge else {
            return Some(current);
        };
        let calls = machine.calls_at(position);
        let first = calls.partition_point(|call| {
            call.owner
                < DescriptorInvocationOwner::Cleanup {
                    edge,
                    action_ordinal: 0,
                }
        });
        let mut previous = None;
        for call in &calls[first..] {
            let DescriptorInvocationOwner::Cleanup {
                edge: owner_edge,
                action_ordinal,
            } = call.owner
            else {
                return None;
            };
            if owner_edge != edge {
                break;
            }
            if previous.is_some_and(|ordinal| ordinal >= action_ordinal)
                || call.argument_contents.iter().any(Option::is_none)
            {
                return None;
            }
            previous = Some(action_ordinal);
            snapshots.insert((position, call.owner), current.clone());
            let reads = DescriptorReadDepth::at(machine, &current, Some(history));
            let mut after = current.clone();
            self.apply_invocation(
                machine,
                &current,
                &mut after,
                call,
                depth,
                case_storage,
                &reads,
            )
            .ok()?;
            current = after;
        }
        Some(current)
    }

    pub(crate) fn invocation_state(
        &self,
        machine: MachineId,
        position: DescriptorPosition,
        owner: DescriptorInvocationOwner,
    ) -> Option<&State> {
        self.machine(machine)?.invocation(position, owner)?;
        match owner {
            DescriptorInvocationOwner::Operation(_)
            | DescriptorInvocationOwner::InvokeNoNormal(_) => {
                self.snapshots.get(&machine)?.ready()?.before.get(&position)
            }
            DescriptorInvocationOwner::Cleanup { .. } => self
                .snapshots
                .get(&machine)?
                .ready()?
                .invocations
                .get(&(position, owner)),
        }
    }

    pub(crate) fn states(&mut self, identity: MachineId, depth: usize) -> Option<Arc<States>> {
        if let Some(cached) = self.snapshots.get(&identity) {
            return cached.ready().map(|prepared| prepared.before.clone());
        }
        self.snapshots
            .insert(identity, PreparedStateCache::Preparing);
        let incomplete = |position, detail| DescriptorGraphError::IncompleteMachine {
            machine: identity,
            position,
            successor: None,
            detail: Some(detail),
        };
        let mut failure =
            incomplete(None, "descriptor state preparation reached its depth bound");
        let prepared = self.prepare_state_rows(identity, depth, &mut failure);
        if prepared.is_none() {
            self.snapshots
                .insert(identity, PreparedStateCache::Failed(failure));
        }
        prepared
    }

    fn prepare_state_rows(
        &mut self,
        identity: MachineId,
        depth: usize,
        failure: &mut DescriptorGraphError,
    ) -> Option<Arc<States>> {
        let incomplete = |position, detail| DescriptorGraphError::IncompleteMachine {
            machine: identity,
            position,
            successor: None,
            detail: Some(detail),
        };
        if depth >= 128 {
            return None;
        }
        *failure = DescriptorGraphError::MissingOrAmbiguousMachine(identity);
        let machine = self.machine(identity)?;
        *failure = incomplete(
            None,
            "descriptor case graph or invocation owners are incomplete",
        );
        super::case_flow::validate_graph(self, machine)?;
        let normal_exit_positions = machine
            .normal_exits
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        for (position, calls) in &machine.calls {
            *failure = incomplete(
                Some(*position),
                "descriptor invocation order or cleanup edge is incomplete",
            );
            let Some(calls) = calls else {
                continue;
            };
            let step = machine.flow.get(position)?;
            let normal_exit = normal_exit_positions.contains(position);
            let edges = step
                .successors
                .iter()
                .filter_map(|edge| edge.edge)
                .collect::<BTreeSet<_>>();
            if calls.windows(2).any(|pair| pair[0].owner >= pair[1].owner) {
                return None;
            }
            let mut exit_edge = None;
            let mut operation_seen = false;
            for call in calls {
                if matches!(
                    call.owner,
                    DescriptorInvocationOwner::Operation(_)
                        | DescriptorInvocationOwner::InvokeNoNormal(_)
                ) {
                    if operation_seen {
                        return None;
                    }
                    operation_seen = true;
                }
                if let DescriptorInvocationOwner::Cleanup { edge, .. } = call.owner {
                    if normal_exit {
                        if !step.successors.is_empty()
                            || exit_edge.is_some_and(|previous| previous != edge)
                        {
                            return None;
                        }
                        exit_edge = Some(edge);
                    } else if !edges.contains(&edge) {
                        return None;
                    }
                }
            }
        }
        *failure = incomplete(
            Some(machine.entry),
            "descriptor entry or initial slot inventory is incomplete",
        );
        let case_storage = super::case_flow::Storage::new(self, machine);
        if !machine.flow.contains_key(&machine.entry) {
            return None;
        }
        let mut states = States::from([(machine.entry, self.initial(machine)?)]);
        let mut arrivals = States::new();
        let mut invocation_states = InvocationStates::new();
        let mut normal_exits = States::new();
        let mut pending = vec![machine.entry];
        let mut visits = 0usize;
        while let Some(position) = pending.pop() {
            *failure = incomplete(
                Some(position),
                "descriptor state replay did not stabilize within its bound",
            );
            visits += 1;
            if visits > machine.flow.len().saturating_mul(4096).max(4096) {
                return None;
            }
            *failure = incomplete(
                Some(position),
                "descriptor flow point has no exact incoming state",
            );
            let before = states.get(&position)?.clone();
            let step = machine.flow.get(&position)?;
            let after = {
                let reads = DescriptorReadDepth::at(machine, &before, Some(&states));
                *failure = incomplete(
                    Some(position),
                    "descriptor storage transfer cannot be reconstructed",
                );
                let mut after = self.transfer(
                    machine,
                    &before,
                    &step.definitions,
                    &step.consumed,
                    step.derivations
                        .iter()
                        .map(|(source, destination)| (source, destination)),
                    super::case_flow::Transfer::Operation {
                        position,
                        reads: &reads,
                    },
                    &case_storage,
                )?;
                after.contents_invalidated |=
                    machine.unresolved_contents_writes.contains(&position);
                // Escaping a value keeps its loans alive; it does not write
                // its case tag. The transfer above retires case facts at
                // actual writes, including unresolved storage queries.
                if machine.unresolved_contents_writes.contains(&position)
                    || machine
                        .direct_call(position)
                        .is_some_and(|call| !call.argument_contents.iter().all(Option::is_some))
                {
                    after.cases = Arc::default();
                }
                *failure = incomplete(
                    Some(position),
                    "descriptor case establishment has no exact storage subject",
                );
                if let Some(constructors) = machine.case_constructors.get(&position) {
                    for constructor in constructors {
                        super::case_flow::establish(self, machine, &mut after, constructor)?;
                    }
                }
                if let Some(transfers) = machine.contents_transfers.get(&position) {
                    self.transfer_contents(
                        machine,
                        &before,
                        &mut after,
                        transfers,
                        &step.derivations,
                        &reads,
                    );
                }
                *failure = incomplete(
                    Some(position),
                    "descriptor origination has no exact completion coordinate",
                );
                if let Some(origins) = machine.originations.get(&position) {
                    let completed =
                        Some((machine.id, (position.0, position.1.checked_add(1)?)));
                    for (location, origin) in origins {
                        let origin = if matches!(origin, DescriptorCallOrigin::Argument { source, projection } if *source == location.root && location.path.is_empty() && projection == &Projection::new(&[]))
                            && machine.place_types.get(&location.root).is_some_and(|ty| {
                                !is_descriptor(self.types.iter(), *ty)
                                    || machine
                                        .sequence_construction(
                                            self.types,
                                            location.root,
                                            completed,
                                        )
                                        .is_some()
                            }) {
                            Origin::Storage(location.clone())
                        } else {
                            Origin::Backing(origin.clone())
                        };
                        after
                            .slots_mut()
                            .insert(location.clone(), Some(BTreeSet::from([origin])));
                    }
                }
                if let Some(call) = machine.direct_call(position)
                    && call.argument_contents.iter().all(Option::is_some)
                    && let Err(detail) = self.apply_invocation(
                        machine,
                        &before,
                        &mut after,
                        call,
                        depth,
                        &case_storage,
                        &reads,
                    )
                {
                    *failure = self
                        .failed_invocation_preparation(call)
                        .unwrap_or_else(|| incomplete(Some(position), detail));
                    return None;
                }
                after
            };
            if step.successors.iter().any(|edge| {
                edge.bindings
                    .iter()
                    .any(|binding| matches!(binding.source, DescriptorFlowSource::Contents(_)))
            }) {
                arrivals.insert(position, after.clone());
            }
            *failure = incomplete(
                Some(position),
                "descriptor case successors differ from actual control flow",
            );
            if machine
                .case_successors
                .get(&position)
                .is_some_and(|cases| cases.len() != step.successors.len())
            {
                return None;
            }
            if normal_exit_positions.contains(&position) {
                let calls = machine
                    .calls
                    .get(&position)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .filter_map(|call| match call.owner {
                        DescriptorInvocationOwner::Cleanup { edge, .. } => Some(edge),
                        _ => None,
                    })
                    .collect::<BTreeSet<_>>();
                if calls.len() > 1 {
                    return None;
                }
                *failure = incomplete(
                    Some(position),
                    "descriptor normal-exit cleanup state cannot be reconstructed",
                );
                let completed = self.apply_cleanup_calls(
                    machine,
                    position,
                    calls.first().copied(),
                    after.clone(),
                    &states,
                    depth,
                    &case_storage,
                    &mut invocation_states,
                )?;
                normal_exits.insert(position, completed);
            }
            for (ordinal, edge) in step.successors.iter().enumerate() {
                *failure = incomplete(
                    Some(position),
                    "descriptor successor has no actual target flow point",
                );
                if !machine.flow.contains_key(&edge.target) {
                    return None;
                }
                *failure = incomplete(
                    Some(position),
                    "descriptor case selection cannot be reconstructed",
                );
                let mut selected = after.clone();
                if let Some((predicate, truth)) = machine
                    .case_successors
                    .get(&position)
                    .and_then(|cases| cases.get(ordinal))
                    && !super::case_flow::select(
                        self,
                        machine,
                        &mut selected,
                        predicate,
                        *truth,
                    )?
                {
                    continue;
                }
                *failure = incomplete(
                    Some(position),
                    "descriptor successor cleanup state cannot be reconstructed",
                );
                let completed = self.apply_cleanup_calls(
                    machine,
                    position,
                    edge.edge,
                    selected.clone(),
                    &states,
                    depth,
                    &case_storage,
                    &mut invocation_states,
                )?;
                *failure = incomplete(
                    Some(position),
                    "descriptor successor binding cannot be reconstructed",
                );
                let carried = {
                    let reads = DescriptorReadDepth::at(machine, &selected, Some(&states));
                    self.transfer_edge(
                        machine,
                        &selected,
                        &completed,
                        edge,
                        &case_storage,
                        &reads,
                    )?
                };
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
        let states = Arc::new(states);
        self.snapshots.insert(
            identity,
            PreparedStateCache::Ready(PreparedStates {
                before: states.clone(),
                arrivals,
                invocations: invocation_states,
                normal_exits,
            }),
        );
        Some(states)
    }

    pub(crate) fn prepare_states(&mut self, identity: MachineId) -> Option<()> {
        self.prepare_states_checked(identity).ok()
    }

    pub(crate) fn prepare_states_checked(
        &mut self,
        identity: MachineId,
    ) -> Result<(), DescriptorGraphError> {
        if !self.snapshots.contains_key(&identity) {
            let _ = self.states(identity, 0);
        }
        match self.snapshots.get(&identity) {
            Some(PreparedStateCache::Ready(_)) => Ok(()),
            Some(PreparedStateCache::Failed(error)) => Err(*error),
            Some(PreparedStateCache::Preparing) => {
                Err(DescriptorGraphError::IncompleteMachine {
                    machine: identity,
                    position: None,
                    successor: None,
                    detail: Some("descriptor state preparation is recursively in progress"),
                })
            }
            None => Err(DescriptorGraphError::MissingOrAmbiguousMachine(identity)),
        }
    }

    fn failed_invocation_preparation(
        &self,
        call: &DescriptorInvocation,
    ) -> Option<DescriptorGraphError> {
        let failed = |identity| match self.snapshots.get(&identity) {
            Some(PreparedStateCache::Failed(error)) => Some(*error),
            _ => None,
        };
        match &call.target {
            DescriptorInvocationTarget::Machine(identity) => failed(*identity),
            DescriptorInvocationTarget::Public {
                effects_machine: Some(identity),
                ..
            } => failed(*identity),
            DescriptorInvocationTarget::Public { realizations, .. } => {
                realizations.iter().find_map(|identity| failed(*identity))
            }
            DescriptorInvocationTarget::Provider { .. } => None,
        }
    }
    pub(crate) fn invocation_summary(
        &mut self,
        call: &DescriptorInvocation,
        depth: usize,
    ) -> Option<Summary> {
        match &call.target {
            DescriptorInvocationTarget::Machine(machine) => self.summary(*machine, depth),
            target @ DescriptorInvocationTarget::Public {
                effects_machine: Some(machine),
                ..
            } => {
                let effects = self.summary(*machine, depth)?;
                let mut summary = direct_target_summary(target)?;
                summary.effects = effects.effects;
                summary.invalidates_contents |= effects.invalidates_contents;
                Some(summary)
            }
            DescriptorInvocationTarget::Public { realizations, .. }
                if !realizations.is_empty() =>
            {
                let summaries = realizations
                    .iter()
                    .map(|machine| self.summary(*machine, depth))
                    .collect::<Option<Vec<_>>>()?;
                Some(join(direct_target_summary(&call.target)?, &summaries))
            }
            target => direct_target_summary(target),
        }
    }

    pub(crate) fn cached_invocation_summary(
        &self,
        call: &DescriptorInvocation,
    ) -> Option<Summary> {
        match &call.target {
            DescriptorInvocationTarget::Machine(machine) => {
                self.summaries.get(machine)?.clone()
            }
            target @ DescriptorInvocationTarget::Public {
                effects_machine: Some(machine),
                ..
            } => {
                let effects = self.summaries.get(machine)?.as_ref()?;
                let mut summary = direct_target_summary(target)?;
                summary.effects = effects.effects.clone();
                summary.invalidates_contents |= effects.invalidates_contents;
                Some(summary)
            }
            DescriptorInvocationTarget::Public { realizations, .. }
                if !realizations.is_empty() =>
            {
                let summaries = realizations
                    .iter()
                    .map(|machine| self.summaries.get(machine)?.clone())
                    .collect::<Option<Vec<_>>>()?;
                Some(join(direct_target_summary(&call.target)?, &summaries))
            }
            target => direct_target_summary(target),
        }
    }

    pub(crate) fn substitute_result(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        call: &DescriptorInvocation,
        origin: &DescriptorStoredOrigin,
    ) -> Value {
        self.substitute_result_with_depth(
            machine,
            state,
            call,
            origin,
            &DescriptorReadDepth::at(machine, state, None),
        )
    }

    pub(crate) fn substitute_result_with_depth(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        call: &DescriptorInvocation,
        origin: &DescriptorStoredOrigin,
        reads: &DescriptorReadDepth<'_>,
    ) -> Value {
        let value = self.substitute_with_depth(machine, state, call, origin, reads);
        if matches!(
            call.target,
            DescriptorInvocationTarget::Public {
                extent: semantic_vocabulary::ReferenceOriginExtent::WithinRegion,
                ..
            }
        ) {
            let mut region = Projection::new(&[]);
            region.forget_address();
            self.project(machine, value, Some(&region))
        } else {
            value
        }
    }

    pub(crate) fn actual_slot(
        &self,
        call: &DescriptorInvocation,
        parameter: usize,
        path: &[DescriptorPathSegment],
    ) -> Option<DescriptorLocation> {
        let mut actual = call.arguments.get(parameter)?.as_ref()?.clone();
        actual.path.extend_from_slice(path);
        Some(actual)
    }
    pub(crate) fn substitute(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        call: &DescriptorInvocation,
        origin: &DescriptorStoredOrigin,
    ) -> Value {
        self.substitute_with_depth(
            machine,
            state,
            call,
            origin,
            &DescriptorReadDepth::at(machine, state, None),
        )
    }

    pub(crate) fn substitute_with_depth(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        call: &DescriptorInvocation,
        origin: &DescriptorStoredOrigin,
        reads: &DescriptorReadDepth<'_>,
    ) -> Value {
        if let DescriptorStoredOrigin::Provider(provider) = origin {
            return self.substitute_provider_with_depth(machine, state, call, provider, reads);
        }
        self.substitute_actuals_with_depth(machine, state, call.actual_inputs(), origin, reads)
    }

    pub(crate) fn substitute_actuals(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        actuals: DescriptorActualInputs<'_>,
        origin: &DescriptorStoredOrigin,
    ) -> Value {
        self.substitute_actuals_with_depth(
            machine,
            state,
            actuals,
            origin,
            &DescriptorReadDepth::at(machine, state, None),
        )
    }

    pub(crate) fn substitute_actuals_with_depth(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        actuals: DescriptorActualInputs<'_>,
        origin: &DescriptorStoredOrigin,
        reads: &DescriptorReadDepth<'_>,
    ) -> Value {
        match origin {
            DescriptorStoredOrigin::Invocation {
                carrier_type,
                projection,
            } => Some(BTreeSet::from([Origin::Backing(
                DescriptorCallOrigin::Invocation {
                    carrier_type: *carrier_type,
                    projection: projection.clone(),
                },
            )])),
            DescriptorStoredOrigin::Provider(_)
            | DescriptorStoredOrigin::ProducedContents(_) => None,
            DescriptorStoredOrigin::AccessFormation {
                machine,
                operation,
                place,
                carrier,
                projection,
            } => Some(BTreeSet::from([Origin::Backing(
                DescriptorCallOrigin::AccessFormation {
                    machine: *machine,
                    operation: *operation,
                    place: *place,
                    carrier: *carrier,
                    projection: projection.clone(),
                },
            )])),
            DescriptorStoredOrigin::Empty => Some(BTreeSet::from([Origin::Backing(
                DescriptorCallOrigin::Empty,
            )])),
            // Only the caller's result slot can name the moved roster;
            // the returned-slot replay installs it before substitution.
            DescriptorStoredOrigin::Carried { .. } => None,
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
            } => {
                if let Some(argument) = actuals.arguments.get(*position)?.as_ref()
                    && !self.location_carries_backing(machine, state, argument, reads)?
                {
                    // A view returned from inline storage borrows the
                    // evaluated argument's address, not the local value
                    // whose bytes were last copied into that argument.
                    return self.project(
                        machine,
                        self.capture_storage(machine, state, argument, reads),
                        Some(projection),
                    );
                }
                let address = actuals
                    .arguments
                    .get(*position)
                    .and_then(Option::as_ref)
                    .and_then(|argument| self.read_with_depth(machine, state, argument, reads));
                let value = match address {
                    Some(origins) if !origins.is_empty() => Some(origins),
                    _ => self.read_contents_with_depth(
                        machine,
                        state,
                        actuals.argument_contents.get(*position)?.as_ref()?,
                        reads,
                    ),
                };
                self.project(machine, value, Some(projection))
            }
            DescriptorStoredOrigin::IncomingSlot { position, path } => {
                let mut query = actuals.argument_contents.get(*position)?.as_ref()?.clone();
                query
                    .steps
                    .extend(path.iter().cloned().map(DescriptorContentsStep::Address));
                self.read_contents_with_depth(machine, state, &query, reads)
            }
            DescriptorStoredOrigin::IncomingContents {
                position,
                steps,
                projection,
            } => {
                let query = self.extend_actual_contents(
                    machine,
                    actuals,
                    actuals.argument_contents.get(*position)?.as_ref()?.clone(),
                    steps,
                )?;
                self.project(
                    machine,
                    self.read_contents_with_depth(machine, state, &query, reads),
                    Some(projection),
                )
            }
        }
    }
    // Returning an owned sequence moves its roster, not ownership of the
    // referents held by its elements. Export each completed ordinal's
    // descriptor slots before claiming that the roster itself is carried.
    // A local referent still cannot be exported by `exported`.
    fn exported_result(
        &self,
        machine: &DescriptorMachine,
        states: &States,
        origin: &Origin,
        slot: (&[DescriptorPathSegment], Option<StructuralTypeId>),
        depth: usize,
    ) -> Option<DescriptorResultSlot> {
        if let Origin::CapturedStorage(account) = origin {
            return self.exported_result(
                machine,
                states,
                &account.singular_coordinate()?.origin(),
                slot,
                depth,
            );
        }
        if depth > 64 {
            return None;
        }
        let origins = if let Origin::Storage(backing) = origin
            && self.owns_roster_type(slot.1)
        {
            if slot.1 != machine.location_type(self.types, backing, None) {
                return None;
            }
            self.exported_roster(machine, states, backing, depth)?
        } else {
            vec![self.exported(machine, origin)?]
        };
        Some(DescriptorResultSlot {
            path: slot.0.to_vec(),
            origins,
        })
    }

    fn owns_roster_type(&self, ty: Option<StructuralTypeId>) -> bool {
        ty.and_then(|ty| find_structural_type(self.types, ty))
            .is_some_and(|declaration| {
                !declaration.is_copyable()
                    && matches!(
                        declaration.shape,
                        StructuralTypeShape::ElementView { access: None, .. }
                    )
            })
    }

    fn exported_roster(
        &self,
        machine: &DescriptorMachine,
        states: &States,
        backing: &DescriptorLocation,
        depth: usize,
    ) -> Option<Vec<DescriptorStoredOrigin>> {
        if depth > 64 || self.parameter(machine, backing.root).is_some() {
            return None;
        }
        let StructuralTypeShape::ElementView {
            element: expected_element,
            access: None,
        } = find_structural_type(
            self.types,
            machine.location_type(self.types, backing, None)?,
        )?
        .shape
        else {
            return None;
        };
        // A returned roster may have arrived through another ordinary
        // call. Rebase its existing result account at that call's own
        // input cut; callee coordinates never become caller history.
        if let Some((position, origins)) =
            self.returned_roster_origins(machine, backing, None, None)
        {
            let call = machine.direct_call(position)?;
            let before = states.get(&position)?;
            let reads = DescriptorReadDepth::at(machine, before, Some(states));
            let expected = machine.location_type(self.types, backing, None);
            let mut rebased = Vec::new();
            for origin in origins {
                rebased.extend(self.reexported_result_origin(
                    machine,
                    call,
                    origin,
                    expected,
                    &reads,
                    depth + 1,
                )?);
            }
            return Some(rebased);
        }
        if !backing.path.is_empty() {
            return None;
        }
        let (producer, element_type, step) =
            machine.sequence_construction(self.types, backing.root, None)?;
        if element_type != expected_element {
            return None;
        }
        let state = states.get(&producer)?;
        let reads = DescriptorReadDepth::at(machine, state, Some(states));
        let mut elements = Vec::with_capacity(step.inputs.len());
        for (element, _) in &step.inputs {
            let element = element.as_ref()?;
            if machine.location_type(self.types, element, None)? != element_type {
                return None;
            }
            let mut slots = Vec::new();
            for relative in
                self.slot_kind
                    .owned_slots(self.types, element.root, element_type)?
            {
                let mut source = element.clone();
                source.path.extend_from_slice(&relative.path);
                let origins = self.read_with_depth(machine, state, &source, &reads)?;
                if origins.is_empty() {
                    return None;
                }
                let mut exported = DescriptorResultSlot {
                    path: relative.path.clone(),
                    origins: Vec::new(),
                };
                for origin in origins {
                    let mut incoming = self.exported_result(
                        machine,
                        states,
                        &origin,
                        (
                            &source.path,
                            machine.location_type(self.types, &source, None),
                        ),
                        depth + 1,
                    )?;
                    incoming.path = relative.path.clone();
                    exported.merge(incoming)?;
                }
                slots.push(exported);
            }
            elements.push(slots);
        }
        Some(vec![DescriptorStoredOrigin::Carried {
            element: element_type,
            elements,
        }])
    }

    fn reexported_result_origin(
        &self,
        machine: &DescriptorMachine,
        call: &DescriptorInvocation,
        origin: &DescriptorStoredOrigin,
        expected: Option<StructuralTypeId>,
        reads: &DescriptorReadDepth<'_>,
        depth: usize,
    ) -> Option<Vec<DescriptorStoredOrigin>> {
        if depth > 64 {
            return None;
        }
        let (context, before) = reads.live?;
        let (history_owner, states) = reads.history?;
        if !std::ptr::eq(context, machine) || !std::ptr::eq(history_owner, machine) {
            return None;
        }
        if let DescriptorStoredOrigin::Carried { element, elements } = origin {
            if !self.owns_roster_type(expected)
                || !matches!(find_structural_type(self.types, expected?)?.shape,
                    StructuralTypeShape::ElementView { element: actual, access: None } if actual == *element)
            {
                return None;
            }
            let elements = elements
                .iter()
                .map(|slots| {
                    slots
                        .iter()
                        .map(|slot| {
                            descriptor_path(self.types, *element, &slot.path)?;
                            let expected =
                                crate::descriptor_lifetimes::descriptor_location_type(
                                    self.types, *element, &slot.path,
                                );
                            let mut origins = Vec::new();
                            for origin in &slot.origins {
                                origins.extend(self.reexported_result_origin(
                                    machine,
                                    call,
                                    origin,
                                    expected,
                                    reads,
                                    depth + 1,
                                )?);
                            }
                            (!origins.is_empty()).then(|| DescriptorResultSlot {
                                path: slot.path.clone(),
                                origins,
                            })
                        })
                        .collect::<Option<Vec<_>>>()
                })
                .collect::<Option<Vec<_>>>()?;
            return Some(vec![DescriptorStoredOrigin::Carried {
                element: *element,
                elements,
            }]);
        }
        let mut origins = Vec::new();
        for actual in self.substitute_result_with_depth(machine, before, call, origin, reads)? {
            // Keep the original returned leaf's declared custody. An owned
            // local backing cannot turn its borrowed view into an owner.
            origins.extend(
                self.exported_result(machine, states, &actual, (&[], expected), depth + 1)?
                    .origins,
            );
        }
        (!origins.is_empty()).then_some(origins)
    }

    pub(crate) fn exported(
        &self,
        machine: &DescriptorMachine,
        origin: &Origin,
    ) -> Option<DescriptorStoredOrigin> {
        match origin {
            Origin::ProducedContents(produced) => {
                let dependencies = produced
                    .lifetime_dependencies
                    .iter()
                    .map(|debt| {
                        Some(DescriptorLifetimeDebt {
                            origin: self.exported(machine, &debt.origin)?,
                            access: debt.access,
                        })
                    })
                    .collect::<Option<_>>()?;
                Some(DescriptorStoredOrigin::ProducedContents(
                    produced.with_dependencies(dependencies),
                ))
            }
            Origin::CapturedStorage(account) => {
                self.exported(machine, &account.singular_coordinate()?.origin())
            }
            Origin::Backing(DescriptorCallOrigin::Invocation {
                carrier_type,
                projection,
            }) => Some(DescriptorStoredOrigin::Invocation {
                carrier_type: *carrier_type,
                projection: projection.clone().without_runtime_coordinates(),
            }),

            Origin::Provider(provider) => Some(DescriptorStoredOrigin::Provider(
                provider.map_dependencies(|dependency| self.exported(machine, dependency))?,
            )),
            Origin::Backing(DescriptorCallOrigin::AccessFormation {
                machine,
                operation,
                place,
                carrier,
                projection,
            }) => Some(DescriptorStoredOrigin::AccessFormation {
                machine: *machine,
                operation: *operation,
                place: *place,
                carrier: *carrier,
                projection: projection.clone().without_runtime_coordinates(),
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
            Origin::IncomingContents(query, projection) => {
                let (position, _, root_type) = self.parameter(machine, query.root)?;
                query.validate(self.types, root_type)?;
                Some(DescriptorStoredOrigin::IncomingContents {
                    position,
                    steps: query.steps.clone(),
                    projection: projection.clone().without_runtime_coordinates(),
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
                    terminal_psi::reading_borrow_pair_pattern!()
                        | StructuralAccess::WriteOnlyBorrow
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
    pub(crate) fn summary(&mut self, identity: MachineId, depth: usize) -> Option<Summary> {
        if let Some(cached) = self.summaries.get(&identity) {
            return cached.clone();
        }
        let machine = self.machine(identity)?;
        if let Some(summary) = self.empty_interface_summary(machine) {
            self.summaries.insert(identity, Some(summary.clone()));
            return Some(summary);
        }
        self.summaries.insert(identity, None);
        let states = self.states(identity, depth)?;
        let completed = &self.snapshots.get(&identity)?.ready()?.normal_exits;
        if completed.is_empty() {
            let mut writable = Some(Vec::new());
            for row in &machine.places {
                let DescriptorDefinition::Parameter {
                    position,
                    structural_type,
                    access,
                } = row.definition
                else {
                    continue;
                };
                let Some(slots) = self.slot_kind.writable_slots(
                    self.types,
                    row.place,
                    structural_type,
                    access,
                ) else {
                    writable = None;
                    break;
                };
                if slots.is_empty()
                    && self.slot_kind.may_write_indirect_contents(
                        self.types,
                        structural_type,
                        &StructuralArgument {
                            place: row.place,
                            path: Vec::new(),
                            access,
                        },
                    )
                {
                    writable = None;
                    break;
                }
                for slot in slots {
                    writable.as_mut()?.push(UnobservedWrite {
                        parameter: position,
                        steps: slot
                            .path
                            .into_iter()
                            .map(DescriptorContentsStep::Address)
                            .collect(),
                    });
                }
            }
            let summary = Summary {
                result: ResultObservations::NoNormalBodyObservation,
                result_cases: Default::default(),
                effects: EffectObservations::Unobserved(writable),
                invalidates_contents: states.values().any(|state| state.contents_invalidated),
                storage_frame: StorageFrame::default(),
            };
            self.summaries.insert(identity, Some(summary.clone()));
            return Some(summary);
        }
        let mut result_cases: Option<super::case_flow::ResultCases> = None;
        let mut unknown_results: Vec<Vec<DescriptorPathSegment>> = Vec::new();
        let mut result_contents: Option<Vec<DescriptorResultSlot>> = None;
        let result = if let Some(returns) = &machine.returns {
            let mut slots: Option<Vec<DescriptorResultSlot>> = None;
            let mut result_type = None;
            for (position, location) in returns {
                if !location.path.is_empty() {
                    return None;
                }
                let Some(state) = completed.get(position) else {
                    continue;
                };
                let structural_type = *machine.place_types.get(&location.root)?;
                let cases =
                    super::case_flow::ResultCases::capture(self, machine, state, location);
                if result_type.is_some_and(|previous| previous != structural_type) {
                    return None;
                }
                result_type = Some(structural_type);
                let inventory = self.slot_kind.owned_inventory(
                    self.types,
                    location.root,
                    structural_type,
                )?;
                let contents = result_contents.get_or_insert_with(|| {
                    inventory
                        .unknown_contents
                        .iter()
                        .map(|packet| DescriptorResultSlot {
                            path: packet.location.path.clone(),
                            origins: Vec::new(),
                        })
                        .collect()
                });
                if contents.len() != inventory.unknown_contents.len() {
                    return None;
                }
                for (packet, returned) in contents.iter_mut().zip(inventory.unknown_contents) {
                    if returned.kind != UnknownContentsKind::Value
                        || packet.path != returned.location.path
                    {
                        return None;
                    }
                    self.unknown_contents_type(machine, &returned.location)?;
                    if cases.excludes_payload(structural_type, &packet.path) {
                        continue;
                    }
                    let origins = self.read(machine, state, &returned.location)?;
                    if origins.is_empty() {
                        return None;
                    }
                    for origin in origins {
                        packet.origins.push(self.exported(machine, &origin)?);
                    }
                }
                let returned = inventory.slots;
                let accumulated = slots.get_or_insert_with(|| {
                    returned
                        .iter()
                        .map(|slot| DescriptorResultSlot {
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
                    // `read` has no answer where this body does not
                    // establish what the slot holds, as for a view a
                    // boundary returned. That is not a failed analysis
                    // of the body: its effects are known, and only what
                    // backs this one slot is not.
                    let Some(origins) = self.read(machine, state, &location) else {
                        if !unknown_results.contains(&slot.path) {
                            unknown_results.push(slot.path.clone());
                        }
                        continue;
                    };
                    if origins.is_empty() {
                        return None;
                    }
                    for origin in origins {
                        slot.merge(self.exported_result(
                            machine,
                            &states,
                            &origin,
                            (
                                &location.path,
                                machine.location_type(self.types, &location, None),
                            ),
                            0,
                        )?)?;
                    }
                }
                if let Some(accumulated) = &mut result_cases {
                    accumulated.join(&cases);
                } else {
                    result_cases = Some(cases);
                }
            }
            if result_contents.as_ref().is_some_and(|packets| {
                packets.iter().any(|packet| {
                    packet.origins.is_empty()
                        && !result_type
                            .zip(result_cases.as_ref())
                            .is_some_and(|(ty, cases)| cases.excludes_payload(ty, &packet.path))
                })
            }) {
                return None;
            }
            let mut slots = slots?;
            slots.retain(|slot| !unknown_results.contains(&slot.path));
            DescriptorResultSlot::canonicalize(&mut slots)?;
            Some(slots)
        } else {
            None
        };
        let mut written: Vec<(usize, Vec<DescriptorContentsStep>, BTreeSet<Origin>)> =
            Vec::new();
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
            let slots = if self.slot_kind.tracks_references() {
                self.slot_kind
                    .writable_slots(self.types, row.place, structural_type, access)?
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
                    if let Some(state) = completed.get(exit) {
                        origins.extend(self.read_slot(
                            machine,
                            state,
                            &slot,
                            &DescriptorReadDepth::at(machine, state, None),
                            false,
                        )?);
                    }
                }
                if origins.is_empty() {
                    continue;
                }
                if origins == BTreeSet::from([Origin::Incoming(slot.clone())]) {
                    continue;
                }
                written.push((
                    parameter,
                    slot.path
                        .into_iter()
                        .map(DescriptorContentsStep::Address)
                        .collect::<Vec<_>>(),
                    origins,
                ));
            }
        }
        let contents = machine
            .normal_exits
            .iter()
            .filter_map(|position| completed.get(position))
            .flat_map(|state| state.contents.keys().cloned())
            .collect::<BTreeSet<_>>();
        for query in contents {
            let (parameter, _, root_type) = self.parameter(machine, query.root)?;
            query.validate(self.types, root_type)?;
            let mut origins = BTreeSet::new();
            for exit in &machine.normal_exits {
                if let Some(state) = completed.get(exit) {
                    origins.extend(state.effect_contents(&query)?);
                }
            }
            // A slot reached through a reference the formal holds is read
            // above from the same replaced contents this loop publishes
            // (`read` crosses the reference into `state.contents`). That
            // is one destination, so it is one effect holding both
            // readings: a caller refuses two effects of one call that
            // write the same storage, since nothing orders them.
            match written
                .iter_mut()
                .find(|(position, steps, _)| *position == parameter && *steps == query.steps)
            {
                Some((_, _, published)) => published.extend(origins),
                None => written.push((parameter, query.steps, origins)),
            }
        }
        let mut effects = Vec::with_capacity(written.len());
        for (parameter, steps, origins) in written {
            effects.push(DescriptorStoredEffect {
                parameter,
                steps,
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
        let result_cases = result_cases.unwrap_or_default();
        let contents = result_contents.unwrap_or_default();
        let summary = Summary {
            result: ResultObservations::Observed {
                slots: result,
                contents,
                unknown: unknown_results,
            },
            result_cases,
            effects: EffectObservations::Observed(effects),
            invalidates_contents,
            storage_frame: StorageFrame::default(),
        };
        self.summaries.insert(identity, Some(summary.clone()));
        Some(summary)
    }
    pub(crate) fn symbolic(
        &self,
        machine: &DescriptorMachine,
        origins: BTreeSet<Origin>,
    ) -> Option<Vec<DescriptorSourceOrigin>> {
        origins
            .into_iter()
            .map(|origin| match origin {
                Origin::CapturedStorage(account) => Some(DescriptorSourceOrigin::Backing(
                    account.singular_coordinate()?.projection(self, machine)?,
                )),
                Origin::Provider(provider) => Some(DescriptorSourceOrigin::Provider(
                    provider.map_dependencies(|dependency| {
                        let mut origins =
                            self.symbolic(machine, BTreeSet::from([dependency.clone()]))?;
                        (origins.len() == 1).then(|| origins.remove(0))
                    })?,
                )),
                Origin::Backing(origin) => Some(DescriptorSourceOrigin::Backing(origin)),
                Origin::Storage(location) => Some(DescriptorSourceOrigin::Backing(
                    self.storage_projection(machine, &location)?,
                )),
                Origin::Incoming(location) => {
                    Some(DescriptorSourceOrigin::IncomingSlot(location))
                }
                Origin::IncomingContents(query, projection) => {
                    Some(DescriptorSourceOrigin::IncomingContents { query, projection })
                }
                Origin::ProducedContents(_) => None,
            })
            .collect()
    }
    pub(crate) fn flatten(
        &self,
        machine: &DescriptorMachine,
        origin: DescriptorSourceOrigin,
    ) -> Option<DescriptorCallOrigin> {
        match origin {
            DescriptorSourceOrigin::Provider(_) => None,
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
            DescriptorSourceOrigin::IncomingContents { query, projection } => {
                let root_type = *machine.place_types.get(&query.root)?;
                Some(DescriptorCallOrigin::Argument {
                    source: query.root,
                    projection: query
                        .conservative_projection(self.types, root_type)?
                        .then(&projection),
                })
            }
        }
    }
    pub(crate) fn resolved(
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

pub(crate) fn merge_value(state: &mut Slots, location: DescriptorLocation, value: Value) {
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
    let mut changed = !before.contents_invalidated && incoming.contents_invalidated;
    let cases = super::case_flow::join(&before.cases, &incoming.cases);
    if cases != *existing.cases {
        existing.cases = Arc::new(cases);
        changed = true;
    }
    existing.contents_invalidated |= incoming.contents_invalidated;
    for query in before
        .contents
        .keys()
        .chain(incoming.contents.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
    {
        let value = match (
            before.effect_contents(&query),
            incoming.effect_contents(&query),
        ) {
            (Some(mut left), Some(right)) => {
                left.extend(right);
                Some(left)
            }
            _ => None,
        };
        if existing.contents.get(&query) != Some(&value) {
            existing.contents_mut().insert(query, value);
            changed = true;
        }
    }
    drop(before);
    changed |= merge_slots(&mut existing.slots, &incoming.slots);
    changed |= merge_slots(&mut existing.arrival_backing, &incoming.arrival_backing);
    changed
}
fn merge_slots(existing: &mut Arc<Slots>, incoming: &Slots) -> bool {
    let before = existing.clone();
    let mut changed = false;
    let mut slot_updates = Vec::new();
    {
        let mut left = before.iter().peekable();
        let mut right = incoming.iter().peekable();
        while left.peek().is_some() || right.peek().is_some() {
            let order = match (left.peek(), right.peek()) {
                (Some((left, _)), Some((right, _))) => left.cmp(right),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => unreachable!(),
            };
            let (location, previous, arriving) = match order {
                std::cmp::Ordering::Less => {
                    let (location, value) = left.next().unwrap();
                    (location, Some(value), None)
                }
                std::cmp::Ordering::Greater => {
                    let (location, value) = right.next().unwrap();
                    (location, None, Some(value))
                }
                std::cmp::Ordering::Equal => {
                    let (location, previous) = left.next().unwrap();
                    let (_, arriving) = right.next().unwrap();
                    (location, Some(previous), Some(arriving))
                }
            };
            if previous == arriving {
                continue;
            }
            let value = match (
                previous.and_then(Option::as_ref),
                arriving.and_then(Option::as_ref),
            ) {
                (Some(left), Some(right)) => Some(left.union(right).cloned().collect()),
                _ => None,
            };
            if previous != Some(&value) {
                slot_updates.push((location.clone(), value));
                changed = true;
            }
        }
    }
    drop(before);
    if !slot_updates.is_empty() {
        let slots = Arc::make_mut(existing);
        for (location, value) in slot_updates {
            slots.insert(location, value);
        }
    }
    changed
}

impl State {
    /// Whether a read of `query` must be unknown: the leaf has no known
    /// value and the leaf or one of its ancestors is recorded as unknown.
    pub(crate) fn contents_path_invalidated(&self, query: &DescriptorContentsQuery) -> bool {
        !self.leaf_known(query)
            && self
                .unknown_prefixes(query, query.steps.len())
                .next()
                .is_some()
    }

    pub(crate) fn leaf_known(&self, query: &DescriptorContentsQuery) -> bool {
        query
            .exact_location()
            .is_some_and(|location| self.slots.get(&location).is_some_and(Option::is_some))
    }

    /// Locations at most `depth` steps along `query` whose value is
    /// recorded as unknown.
    pub(crate) fn unknown_prefixes<'state>(
        &'state self,
        query: &'state DescriptorContentsQuery,
        depth: usize,
    ) -> impl Iterator<Item = &'state DescriptorLocation> {
        self.slots.iter().filter_map(move |(prefix, value)| {
            (value.is_none()
                && prefix.root == query.root
                && !prefix.path.is_empty()
                && prefix.path.len() <= depth
                && prefix.path.iter().zip(&query.steps).all(|(segment, step)| {
                    matches!(step, DescriptorContentsStep::Address(actual) if actual == segment)
                        || matches!(
                            (segment, step),
                            (
                                DescriptorPathSegment::FixedIndex(_),
                                DescriptorContentsStep::Element { .. }
                            )
                        )
                }))
            .then_some(prefix)
        })
    }

    pub(crate) fn slots_mut(&mut self) -> &mut Slots {
        Arc::make_mut(&mut self.slots)
    }

    pub(crate) fn contents_mut(&mut self) -> &mut BTreeMap<DescriptorContentsQuery, Value> {
        Arc::make_mut(&mut self.contents)
    }

    fn selected_inline_updates<'a>(
        &'a self,
        location: &'a DescriptorLocation,
    ) -> impl Iterator<Item = &'a Value> {
        self.contents_for(location.root)
            .filter_map(|(written, value)| {
                (written.is_symbolic_location()
                    && written.steps.len() == location.path.len()
                    && written
                        .steps
                        .iter()
                        .zip(&location.path)
                        .all(|(step, segment)| {
                            step.may_match(&DescriptorContentsStep::Address(*segment))
                        }))
                .then_some(value)
            })
    }

    pub(crate) fn has_selected_inline_contents(&self, location: &DescriptorLocation) -> bool {
        self.selected_inline_updates(location).next().is_some()
    }

    fn selected_inline_contents(&self, location: &DescriptorLocation, value: Value) -> Value {
        let mut origins = value?;
        for selected in self.selected_inline_updates(location) {
            origins.extend(selected.as_ref()?.iter().cloned());
        }
        Some(origins)
    }

    fn contents_for(
        &self,
        root: PlaceId,
    ) -> impl Iterator<Item = (&DescriptorContentsQuery, &Value)> {
        let start = DescriptorContentsQuery {
            root,
            steps: Vec::new(),
        };
        self.contents
            .range(start..)
            .take_while(move |(query, _)| query.root == root)
    }

    fn incoming_effect(query: &DescriptorContentsQuery) -> BTreeSet<Origin> {
        BTreeSet::from([Origin::IncomingContents(
            query.clone(),
            Projection::new(&[]),
        )])
    }

    fn effect_contents(&self, query: &DescriptorContentsQuery) -> Value {
        // Publish each logical selected write once. Possible overlap is
        // accounted for at a local read or after actual arguments are
        // substituted, so it does not become a second fixed-slot write.
        if query.is_symbolic_location() {
            return self
                .contents
                .get(query)
                .cloned()
                .unwrap_or_else(|| Some(Self::incoming_effect(query)));
        }
        self.incoming_contents(query)
    }

    pub(crate) fn incoming_contents(&self, query: &DescriptorContentsQuery) -> Value {
        let selected = query.is_symbolic_location();
        let mut origins = if selected {
            self.effect_contents(query)?
        } else {
            Self::incoming_effect(query)
        };
        for (written, value) in self.contents_for(query.root) {
            if selected && written == query {
                continue;
            }
            if !written
                .steps
                .iter()
                .zip(&query.steps)
                .all(|(left, right)| left.may_match(right))
            {
                continue;
            }
            if written.steps.len() != query.steps.len() {
                return None;
            }
            origins.extend(value.as_ref()?.iter().cloned());
        }
        Some(origins)
    }
}

#[cfg(test)]
mod snapshot_tests;
