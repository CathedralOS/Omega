use super::engine::{DescriptorReadDepth, EffectObservations, Origin, State, StorageResolution};
use super::*;
use crate::descriptors::descriptor_lifetimes::descriptor_location;
use crate::descriptors::descriptor_lifetimes::descriptor_location_type;

pub(crate) mod call_origins;

// Unchanged-entry checks of incoming sources revisit the same storage at the
// same cut. These rows belong only to this immutable graph and contain actual
// flow positions of machines the query has selected, never a raw-ID maximum.
// In-progress replay states remain outside this prepared query owner.
// Storage resolutions are not kept here: nearly every resolution asks a new
// cut and location, so a kept answer would cost more than it saves.
#[derive(Default)]
struct StorageQueries {
    machines: Vec<MachineStorageQueries>,
}

type IncomingSources = Option<Vec<DescriptorSourceOrigin>>;
type IncomingTopology =
    std::sync::Arc<crate::descriptor_results::payload_reachability::IncomingFlow>;

#[derive(Default)]
struct StorageQuery {
    incoming: LocationMap<IncomingSources>,
}

struct MachineStorageQueries {
    identity: MachineId,
    positions: Vec<(DescriptorPosition, StorageQuery)>,
    incoming: Option<Option<IncomingTopology>>,
}

impl StorageQueries {
    fn machine(&mut self, machine: &DescriptorMachine) -> &mut MachineStorageQueries {
        let index = match self
            .machines
            .binary_search_by_key(&machine.id, |row| row.identity)
        {
            Ok(index) => index,
            Err(index) => {
                self.machines.insert(
                    index,
                    MachineStorageQueries {
                        identity: machine.id,
                        incoming: None,
                        positions: machine
                            .flow
                            .keys()
                            .map(|position| (*position, StorageQuery::default()))
                            .collect(),
                    },
                );
                index
            }
        };
        &mut self.machines[index]
    }

    fn incoming_at(
        &mut self,
        machine: &DescriptorMachine,
        position: DescriptorPosition,
    ) -> Option<&mut LocationMap<IncomingSources>> {
        let positions = &mut self.machine(machine).positions;
        let index = positions
            .binary_search_by_key(&position, |(position, _)| *position)
            .ok()?;
        Some(&mut positions[index].1.incoming)
    }

    fn incoming(&mut self, machine: &DescriptorMachine) -> Option<IncomingTopology> {
        self.machine(machine)
            .incoming
            .get_or_insert_with(|| {
                crate::descriptor_results::payload_reachability::incoming_flow(&machine.flow)
                    .map(std::sync::Arc::new)
            })
            .clone()
    }
}

pub struct DescriptorAnalysis<'a> {
    replay: Replay<'a>,
    storage_queries: StorageQueries,
    flows: Vec<(
        MachineId,
        std::sync::Arc<BTreeMap<DescriptorPosition, DescriptorFlowStep>>,
    )>,
}

pub type DescriptorSourceObservations =
    BTreeMap<DescriptorPosition, BTreeMap<DescriptorLocation, DescriptorSourceObservation>>;

#[derive(Debug)]
pub struct DescriptorSourceObservation {
    pub storage: Option<DescriptorLocation>,
    pub origins: Option<Vec<DescriptorSourceOrigin>>,
}

impl<'a> DescriptorAnalysis<'a> {
    pub(crate) fn uses_source_graph(
        &self,
        types: &[StructuralTypeDeclaration],
        machines: &[DescriptorMachine],
    ) -> bool {
        std::ptr::eq(self.replay.types, types) && std::ptr::eq(self.replay.machines, machines)
    }

    pub(crate) fn slot_kind(&self) -> DescriptorSlotKind {
        self.replay.slot_kind
    }

    pub(crate) fn payload_history<'input>(
        &self,
        module: &'input terminal_psi::TerminalModule,
        machine: &'input terminal_psi::TerminalMachine,
    ) -> crate::descriptor_results::PayloadMembershipHistory<'input>
    where
        'a: 'input,
    {
        crate::descriptor_results::PayloadMembershipHistory::new(
            &module.types.structural_types,
            &module.types.structural_domains,
            self.replay.machines,
            machine,
        )
    }

    pub fn source_observations(&mut self, identity: MachineId) -> DescriptorSourceObservations {
        let Some(machine) = self.replay.machine(identity) else {
            return BTreeMap::new();
        };
        let slots = machine
            .place_types
            .iter()
            .filter_map(|(place, structural_type)| {
                self.replay
                    .slot_kind
                    .inventory(self.replay.types, *place, *structural_type)
                    .map(|inventory| inventory.slots)
            })
            .flatten()
            .collect::<Vec<_>>();
        machine
            .flow
            .iter()
            .filter_map(|(position, step)| {
                let demanded = step
                    .observed_inputs()
                    .chain(step.derivations.iter().map(|(source, _)| source))
                    .chain(step.authority_derivations.iter().map(|(source, _)| source))
                    .chain(
                        step.successors
                            .iter()
                            .flat_map(|edge| edge.derivations().map(|(source, _)| source)),
                    )
                    .collect::<Vec<_>>();
                let mut requested = slots
                    .iter()
                    .filter(|slot| {
                        demanded
                            .iter()
                            .any(|source| source.contains(slot) || slot.contains(source))
                    })
                    .cloned()
                    .collect::<BTreeSet<_>>();
                for source in &demanded {
                    let Some(root) = machine.place_types.get(&source.root) else {
                        continue;
                    };
                    for length in 0..=source.path.len() {
                        let path = &source.path[..length];
                        if self.replay.type_index.descriptor_location_type(*root, path)
                            .and_then(|leaf| {
                                self.replay.type_index.find(leaf)
                            })
                            .is_some_and(|declaration| {
                                matches!(
                                    declaration.shape,
                                    StructuralTypeShape::Reference { .. }
                                )
                            })
                        {
                            requested.insert(DescriptorLocation {
                                root: source.root,
                                path: path.to_vec(),
                            });
                        }
                    }
                }
                let current = requested
                    .iter()
                    .map(|slot| {
                        (
                            slot.clone(),
                            DescriptorSourceObservation {
                                storage: self.location(identity, *position, slot),
                                origins: self
                                    .symbolic_source_origins(identity, *position, slot),
                            },
                        )
                    })
                    .collect::<BTreeMap<_, _>>();
                (!current.is_empty()).then_some((*position, current))
            })
            .collect()
    }

    pub fn new(
        types: &'a [StructuralTypeDeclaration],
        machines: &'a [DescriptorMachine],
    ) -> Self {
        Self {
            replay: Replay::new(types, machines),
            storage_queries: StorageQueries::default(),
            flows: Vec::new(),
        }
    }
    pub fn flow_feasibility(
        &mut self,
        identity: MachineId,
    ) -> BTreeMap<DescriptorPosition, FlowFeasibility> {
        let Some(machine) = self.replay.machine(identity) else {
            return BTreeMap::new();
        };
        let usable = self.replay.prepare_states(identity).is_some();
        let states = self
            .replay
            .snapshots
            .get(&identity)
            .and_then(|cached| cached.ready())
            .map(|prepared| &prepared.before);
        machine
            .flow
            .keys()
            .map(|position| {
                (
                    *position,
                    if !usable {
                        FlowFeasibility::Unknown
                    } else if states.is_some_and(|states| states.contains_key(position)) {
                        FlowFeasibility::Possible
                    } else {
                        FlowFeasibility::ProvedImpossible
                    },
                )
            })
            .collect()
    }
    pub fn edge_feasibility(
        &mut self,
        identity: MachineId,
        position: DescriptorPosition,
        ordinal: usize,
    ) -> FlowFeasibility {
        if self.replay.prepare_states(identity).is_none() {
            return FlowFeasibility::Unknown;
        }
        let Some(machine) = self.replay.machine(identity) else {
            return FlowFeasibility::Unknown;
        };
        let Some(step) = machine.flow.get(&position) else {
            return FlowFeasibility::Unknown;
        };
        if step.successors.get(ordinal).is_none() {
            return FlowFeasibility::Unknown;
        }
        let Some(before) = self
            .replay
            .snapshots
            .get(&identity)
            .and_then(|cached| cached.ready())
            .map(|prepared| &prepared.before)
            .and_then(|states| states.get(&position))
        else {
            return FlowFeasibility::ProvedImpossible;
        };
        let Some((predicate, truth)) = machine
            .case_successors
            .get(&position)
            .and_then(|cases| cases.get(ordinal))
        else {
            return FlowFeasibility::Possible;
        };
        let mut state = before.clone();
        match super::case_flow::select(&self.replay, machine, &mut state, predicate, *truth) {
            Some(false) => FlowFeasibility::ProvedImpossible,
            Some(true) => FlowFeasibility::Possible,
            None => FlowFeasibility::Unknown,
        }
    }
    pub fn case_feasibility(
        &mut self,
        identity: MachineId,
        position: DescriptorPosition,
        predicate: &DescriptorCasePredicate,
    ) -> FlowFeasibility {
        if self.replay.prepare_states(identity).is_none() {
            return FlowFeasibility::Unknown;
        }
        let Some(machine) = self.replay.machine(identity) else {
            return FlowFeasibility::Unknown;
        };
        let Some(before) = self
            .replay
            .snapshots
            .get(&identity)
            .and_then(|cached| cached.ready())
            .map(|prepared| &prepared.before)
            .and_then(|states| states.get(&position))
        else {
            return FlowFeasibility::Unknown;
        };
        super::case_flow::membership(&self.replay, machine, before, predicate)
    }
    pub fn case_is_established(
        &mut self,
        identity: MachineId,
        position: DescriptorPosition,
        predicate: &DescriptorCasePredicate,
    ) -> bool {
        if self.replay.prepare_states(identity).is_none() {
            return false;
        }
        let Some(machine) = self.replay.machine(identity) else {
            return false;
        };
        let Some(state) = self
            .replay
            .snapshots
            .get(&identity)
            .and_then(|cached| cached.ready())
            .map(|prepared| &prepared.before)
            .and_then(|states| states.get(&position))
        else {
            return false;
        };
        super::case_flow::established(&self.replay, machine, state, predicate)
    }
    /// Cleanup observes the selected edge after its case test and before
    /// its actions. Read that same narrowing without copying the whole
    /// storage state or granting membership from an absent payload.
    pub fn established_case(
        &mut self,
        identity: MachineId,
        position: DescriptorPosition,
        successor: Option<(usize, semantic_vocabulary::EdgeId)>,
        subject: &DescriptorLocation,
    ) -> Option<semantic_vocabulary::StructuralCaseId> {
        self.replay.prepare_states(identity)?;
        let machine = self.replay.machine(identity)?;
        let state = self
            .replay
            .snapshots
            .get(&identity)?
            .ready()?
            .before
            .get(&position)?;
        super::case_flow::established_case(
            &self.replay,
            machine,
            state,
            position,
            successor,
            subject,
        )
    }

    pub fn with_reference_slots(
        types: &'a [StructuralTypeDeclaration],
        machines: &'a [DescriptorMachine],
    ) -> Self {
        Self {
            replay: Replay::with_slot_kind(types, machines, DescriptorSlotKind::References),
            storage_queries: StorageQueries::default(),
            flows: Vec::new(),
        }
    }

    pub fn with_borrowed_storage_slots(
        types: &'a [StructuralTypeDeclaration],
        machines: &'a [DescriptorMachine],
    ) -> Self {
        Self {
            replay: Replay::with_slot_kind(
                types,
                machines,
                DescriptorSlotKind::BorrowedStorage,
            ),
            storage_queries: StorageQueries::default(),
            flows: Vec::new(),
        }
    }
    pub fn with_access_origins(
        types: &'a [StructuralTypeDeclaration],
        machines: &'a [DescriptorMachine],
    ) -> Self {
        Self {
            replay: Replay::with_slot_kind(types, machines, DescriptorSlotKind::AccessOrigins),
            storage_queries: StorageQueries::default(),
            flows: Vec::new(),
        }
    }
    pub(crate) fn compatibility_machines(&self) -> &[DescriptorMachine] {
        self.replay.machines
    }
    pub(crate) fn compatibility_ingress(
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
            || !self
                .replay
                .slot_kind
                .slots(self.replay.types, location.root, ty)?
                .contains(location)
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
        self.replay
            .slot_kind
            .slots(self.replay.types, *source, ty)?;
        if *projection != Projection::new(&projection.path) {
            return None;
        }
        self.replay
            .type_index
            .descriptor_location(*source, ty, &projection.path, None)?;
        Some(())
    }
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
        let state = self
            .replay
            .snapshots
            .get(&identity)?
            .ready()?
            .before
            .get(&position)?;
        Self::resolved_storage(&self.replay, machine, state, location)?
            .local_coordinate()
            .cloned()
    }

    /// One complete captured storage address, including indexed backing.
    /// Unknown or competing addresses cannot establish a singular projection.
    pub fn storage_projection(
        &mut self,
        identity: MachineId,
        position: DescriptorPosition,
        location: &DescriptorLocation,
    ) -> Option<DescriptorCallOrigin> {
        let mut family = self
            .captured_storage_family(identity, position, location)
            .ok()?;
        if family.len() != 1 {
            return None;
        }
        family.pop()
    }

    /// All captured addresses that this storage use can name. This is a
    /// complete address family, not loan permission or proof that two
    /// independently selected families choose the same member. Unknown
    /// arrivals invalidate the whole query; no partial family is returned.
    pub fn captured_storage_family(
        &mut self,
        identity: MachineId,
        position: DescriptorPosition,
        location: &DescriptorLocation,
    ) -> Result<Vec<DescriptorCallOrigin>, &'static str> {
        let machine = self
            .replay
            .machine(identity)
            .ok_or("captured storage machine is absent or stale")?;
        self.replay
            .prepare_states_checked(identity)
            .map_err(|error| {
                error
                    .detail()
                    .unwrap_or("captured storage snapshots could not be reconstructed")
            })?;
        let state = self
            .replay
            .snapshots
            .get(&identity)
            .and_then(|cached| cached.ready())
            .and_then(|ready| ready.before.get(&position))
            .ok_or("captured storage position has no reachable snapshot")?;
        let resolution = Self::resolved_storage(&self.replay, machine, state, location)
            .ok_or("captured storage family has a missing, cyclic or unknown arrival")?;
        self.replay
            .checked_storage_family(machine, state, resolution)
    }

    /// Capture an actual source before it is transported. This keeps
    /// its selector proofs and formation position; it grants no access.
    pub fn borrowed_source_storage_family(
        &mut self,
        identity: MachineId,
        position: DescriptorPosition,
        source: PlaceId,
        path: &[terminal_psi::StructuralPathSegment],
    ) -> Result<Vec<DescriptorCallOrigin>, &'static str> {
        self.replay
            .prepare_states_checked(identity)
            .map_err(|error| {
                error
                    .detail()
                    .unwrap_or("borrowed argument snapshots cannot be reconstructed")
            })?;
        let machine = self
            .replay
            .machine(identity)
            .ok_or("borrowed argument machine is absent or stale")?;
        let state = self
            .replay
            .snapshots
            .get(&identity)
            .and_then(|cached| cached.ready())
            .and_then(|ready| ready.before.get(&position))
            .ok_or("borrowed argument has no reachable formation snapshot")?;
        let root = *machine
            .place_types
            .get(&source)
            .ok_or("borrowed argument source has no declared type")?;
        let (prefix, projection, _) =
            crate::descriptors::descriptor_results::terminal_flow::captured_storage_source(
                self.replay.types,
                source,
                path,
                root,
                position,
            )
            .ok_or("borrowed argument has no complete source projection")?;
        let depth = DescriptorReadDepth::at(machine, state, None);
        let values = self
            .replay
            .captured_source_value(machine, state, &prefix, projection.as_ref(), &depth)
            .ok_or("borrowed argument has no captured source value")?;
        let resolution = self
            .replay
            .captured_value_storage(
                machine,
                state,
                &DescriptorLocation::whole(source),
                &values,
                &depth,
                None,
                BTreeMap::new(),
                None,
            )
            .ok_or("borrowed argument has missing or unresolved captured storage")?;
        self.replay
            .checked_storage_family(machine, state, resolution)
    }

    // Actions at one flow position can have different genuine prestates.
    // Their queries stay outside the position-only storage memo.
    pub fn location_at(
        &mut self,
        identity: MachineId,
        position: DescriptorPosition,
        owner: DescriptorInvocationOwner,
        location: &DescriptorLocation,
    ) -> Option<DescriptorLocation> {
        self.replay.prepare_states(identity)?;
        let machine = self.replay.machine(identity)?;
        self.replay.canonical_storage(
            machine,
            self.replay.invocation_state(identity, position, owner)?,
            location,
            None,
        )
    }

    /// The same complete-family query at an invocation action's genuine
    /// prestate. Cleanup prefixes can differ at one flow position, so this
    /// follows location_at and deliberately stays outside position-only memoization.
    pub fn captured_storage_family_at(
        &mut self,
        identity: MachineId,
        position: DescriptorPosition,
        owner: DescriptorInvocationOwner,
        location: &DescriptorLocation,
    ) -> Result<Vec<DescriptorCallOrigin>, &'static str> {
        self.replay
            .machine(identity)
            .ok_or("captured storage machine is absent or stale")?;
        self.replay
            .prepare_states_checked(identity)
            .map_err(|error| {
                error
                    .detail()
                    .unwrap_or("captured storage snapshots could not be reconstructed")
            })?;
        let machine = self
            .replay
            .machine(identity)
            .ok_or("captured storage machine is absent or stale")?;
        let state = self
            .replay
            .invocation_state(identity, position, owner)
            .ok_or("captured storage action has no reachable prestate")?;
        let resolution = self
            .replay
            .resolve_storage(machine, state, location, None)
            .ok_or("captured storage family has a missing, cyclic or unknown arrival")?;
        self.replay
            .checked_storage_family(machine, state, resolution)
    }

    /// Resolve `location` in the snapshot `state` of `machine`, afresh on
    /// every call (see `StorageQueries`).
    fn resolved_storage(
        replay: &Replay<'_>,
        machine: &DescriptorMachine,
        state: &State,
        location: &DescriptorLocation,
    ) -> Option<StorageResolution> {
        let depth = DescriptorReadDepth::at(machine, state, None);
        replay.resolve_storage_with_depth(machine, state, location, &depth)
    }

    pub fn storage_origins(
        &mut self,
        identity: MachineId,
        position: DescriptorPosition,
        location: &DescriptorLocation,
    ) -> Option<Vec<DescriptorLocation>> {
        self.replay.prepare_states(identity)?;
        let machine = self.replay.machine(identity)?;
        let states = &self.replay.snapshots.get(&identity)?.ready()?.before;
        self.replay
            .storage_origins(machine, states.get(&position)?, location)
    }

    /// A write matters even if the old value is restored before return.
    /// Read the operation's actual invalidations and transfers at its
    /// pre-operation cut, rather than comparing final descriptor values.
    /// `None` means the storage effect could not be placed; it must use
    /// the caller's conservative ceiling, never an empty frame.
    pub fn storage_writes(
        &mut self,
        identity: MachineId,
        position: DescriptorPosition,
    ) -> Result<Option<Vec<DescriptorLocation>>, &'static str> {
        self.replay.storage_writes(identity, position)
    }

    /// Completed call summaries still describe this graph. Release the
    /// operation-by-operation states when the current machine no longer needs them.
    pub(crate) fn release_storage_snapshots(&mut self) {
        self.replay.snapshots.clear();
        self.storage_queries = StorageQueries::default();
    }

    /// The frame names entry storage that this body may write, including
    /// delegated writes. It says nothing about the final values or about
    /// permission to invoke the body. Unknown calls and unresolved origins
    /// keep the conservative ceiling (`None`).
    pub fn write_frame(
        &mut self,
        identity: MachineId,
    ) -> Result<Option<Vec<DescriptorLocation>>, &'static str> {
        self.replay.prepare_write_frame(identity)?;
        let Some(frame) = self
            .replay
            .summaries
            .get(&identity)
            .and_then(Option::as_ref)
            .and_then(|summary| summary.storage_frame.writes.as_ref())
            .and_then(|writes| writes.as_ref().as_ref())
        else {
            return Ok(None);
        };
        // Keep precise crossings in the internal transitive frame. Only
        // publication widens to the existing field-only contract shape.
        let mut published = frame
            .iter()
            .cloned()
            .map(|mut write| {
                write.path.truncate(
                    write
                        .path
                        .iter()
                        .take_while(|segment| {
                            matches!(segment, DescriptorPathSegment::Field(_))
                        })
                        .count(),
                );
                write
            })
            .collect::<Vec<_>>();
        published.sort_unstable();
        let mut normalized: Vec<DescriptorLocation> = Vec::with_capacity(published.len());
        for write in published {
            if normalized
                .last()
                .is_none_or(|prior| !prior.contains(&write))
            {
                normalized.push(write);
            }
        }
        Ok(Some(normalized))
    }

    /// Keep each operation at its original instruction and operand. A
    /// complete frame also requires complete frames for actual callees,
    /// reached through the existing invocation graph. Callers traverse
    /// that shared graph with their own argument context; the summary
    /// never expands all possible call paths.
    pub fn storage_access_frame(
        &mut self,
        identity: MachineId,
    ) -> Result<Option<&[(DescriptorPosition, usize)]>, &'static str> {
        self.replay.prepare_access_frame(identity)?;
        Ok(self
            .replay
            .summaries
            .get(&identity)
            .and_then(Option::as_ref)
            .and_then(|summary| summary.storage_frame.accesses.as_ref())
            .and_then(|accesses| accesses.as_ref().as_deref()))
    }

    pub fn storage_access(
        &self,
        identity: MachineId,
        (position, input): (DescriptorPosition, usize),
    ) -> Option<(
        access_vocabulary::AccessOperation,
        &crate::descriptor_lifetimes::DescriptorFlowSource,
    )> {
        let step = self.replay.machine(identity)?.flow.get(&position)?;
        let (operation, source) = step.inputs.get(input)?.1.as_ref()?;
        Some((*operation, source))
    }

    pub fn payload_targets(
        &mut self,
        identity: MachineId,
        position: DescriptorPosition,
        query: &DescriptorContentsQuery,
    ) -> Option<Vec<DescriptorPayloadTarget>> {
        self.replay.prepare_states(identity)?;
        let machine = self.replay.machine(identity)?;
        let states = &self.replay.snapshots.get(&identity)?.ready()?.before;
        self.replay
            .contents_targets(machine, states.get(&position)?, query)
            .map(|targets| targets.into_iter().collect())
    }

    pub fn arrival_payload_targets(
        &mut self,
        identity: MachineId,
        position: DescriptorPosition,
        query: &DescriptorContentsQuery,
    ) -> Option<Vec<DescriptorPayloadTarget>> {
        self.replay.prepare_states(identity)?;
        let machine = self.replay.machine(identity)?;
        let states = &self.replay.snapshots.get(&identity)?.ready()?.arrivals;
        self.replay
            .contents_targets(machine, states.get(&position)?, query)
            .map(|targets| targets.into_iter().collect())
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
    pub fn symbolic_source_origins(
        &mut self,
        identity: MachineId,
        position: DescriptorPosition,
        location: &DescriptorLocation,
    ) -> Option<Vec<DescriptorSourceOrigin>> {
        self.replay.prepare_states(identity)?;
        let machine = self.replay.machine(identity)?;
        let states = &self.replay.snapshots.get(&identity)?.ready()?.before;
        self.replay.symbolic(
            machine,
            self.replay
                .read(machine, states.get(&position)?, location)?,
        )
    }

    /// Recover incoming contents only when their entry version still
    /// reaches this exact observation on every path. Descriptor custody
    /// alone does not establish that the borrowed bytes are unchanged.
    pub fn unchanged_incoming_source_origins(
        &mut self,
        types: &[StructuralTypeDeclaration],
        identity: MachineId,
        position: DescriptorPosition,
        location: &DescriptorLocation,
    ) -> Option<Vec<DescriptorSourceOrigin>> {
        if !std::ptr::eq(types, self.replay.types)
            || self.replay.slot_kind != DescriptorSlotKind::BorrowedStorage
        {
            return None;
        }
        let machine = self.replay.machine(identity)?;
        if let Some(cached) = self
            .storage_queries
            .incoming_at(machine, position)?
            .get(location)
        {
            return cached.clone();
        }
        let result = (|| {
            let incoming = self.storage_queries.incoming(machine)?;
            let origins = self.symbolic_source_origins(identity, position, location)?;
            if origins.is_empty() {
                return None;
            }
            for origin in &origins {
                let query = match origin {
                    DescriptorSourceOrigin::IncomingSlot(slot) => {
                        DescriptorContentsQuery::from_location(slot)
                    }
                    DescriptorSourceOrigin::IncomingContents { query, .. } => query.clone(),
                    // A view parameter already is the descriptor value;
                    // reference parameters instead carry IncomingSlot.
                    DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Argument {
                        source,
                        ..
                    }) => {
                        let DescriptorDefinition::Parameter {
                            structural_type,
                            access: StructuralAccess::SharedBorrow,
                            ..
                        } = machine.place_definition(*source)?
                        else {
                            return None;
                        };
                        if !terminal_psi::find_structural_type(types, *structural_type)?
                            .shape
                            .is_shared_readable_byte_view()
                        {
                            return None;
                        }
                        DescriptorContentsQuery::from_location(&DescriptorLocation::whole(
                            *source,
                        ))
                    }
                    _ => return None,
                };
                crate::descriptor_results::payload_versions::require_unchanged_entry_storage_with_incoming(
                    self.replay.types,
                    self.replay.machines,
                    self,
                    identity,
                    position,
                    &DescriptorPayloadTarget::Incoming(query),
                    &incoming,
                ).ok()?;
            }
            Some(origins)
        })();
        self.storage_queries
            .incoming_at(machine, position)?
            .insert(location.clone(), result.clone());
        result
    }

    /// Preserve the failing stage when an operation requires an exact
    /// origin. Optional discovery queries can still use the Option API.
    pub fn try_symbolic_source_origins(
        &mut self,
        identity: MachineId,
        position: DescriptorPosition,
        location: &DescriptorLocation,
    ) -> Result<Vec<DescriptorSourceOrigin>, String> {
        let context =
            || format!("view loan in {identity:?} at {position:?}, descriptor {location:?}");
        self.replay.prepare_states(identity).ok_or_else(|| {
            format!(
                "{}: descriptor snapshots cannot be reconstructed",
                context()
            )
        })?;
        let machine = self
            .replay
            .machine(identity)
            .ok_or_else(|| format!("{}: descriptor machine is absent", context()))?;
        let states = self
            .replay
            .snapshots
            .get(&identity)
            .and_then(|cached| cached.ready())
            .map(|prepared| &prepared.before)
            .ok_or_else(|| format!("{}: descriptor snapshots are absent", context()))?;
        let state = states
            .get(&position)
            .ok_or_else(|| format!("{}: the query point is unreachable", context()))?;
        let value = self.replay.read(machine, state, location);
        let missing_value = value.is_none();
        if let Some(value) = value
            && let Some(origins) = self.replay.symbolic(machine, value)
        {
            return Ok(origins);
        }
        let detail = if missing_value {
            "the descriptor has no established value at this point"
        } else {
            "the descriptor's original borrow cannot be represented"
        };
        Err(format!("{}: {detail}", context()))
    }

    pub fn substitute_call_source(
        &mut self,
        identity: MachineId,
        position: DescriptorPosition,
        origin: &DescriptorSourceOrigin,
    ) -> Option<Vec<DescriptorSourceOrigin>> {
        let owner = self.replay.machine(identity)?.direct_call(position)?.owner;
        self.substitute_call_source_at(identity, position, owner, origin)
    }
    pub fn substitute_call_source_at(
        &mut self,
        identity: MachineId,
        position: DescriptorPosition,
        owner: DescriptorInvocationOwner,
        origin: &DescriptorSourceOrigin,
    ) -> Option<Vec<DescriptorSourceOrigin>> {
        self.substitute_call_source_with_target(identity, position, owner, origin, false)
    }
    pub fn substitute_stored_effect_source(
        &mut self,
        identity: MachineId,
        position: DescriptorPosition,
        origin: &DescriptorSourceOrigin,
    ) -> Option<Vec<DescriptorSourceOrigin>> {
        let owner = self.replay.machine(identity)?.direct_call(position)?.owner;
        self.substitute_stored_effect_source_at(identity, position, owner, origin)
    }
    pub fn substitute_stored_effect_source_at(
        &mut self,
        identity: MachineId,
        position: DescriptorPosition,
        owner: DescriptorInvocationOwner,
        origin: &DescriptorSourceOrigin,
    ) -> Option<Vec<DescriptorSourceOrigin>> {
        self.substitute_call_source_with_target(identity, position, owner, origin, true)
    }
    fn substitute_call_source_with_target(
        &mut self,
        identity: MachineId,
        position: DescriptorPosition,
        owner: DescriptorInvocationOwner,
        origin: &DescriptorSourceOrigin,
        stored_effects: bool,
    ) -> Option<Vec<DescriptorSourceOrigin>> {
        self.replay.prepare_states(identity)?;
        let machine = self.replay.machine(identity)?;
        let call = machine.invocation(position, owner)?;
        let callee = self.replay.machine(if stored_effects {
            call.stored_effects_callee()?
        } else {
            call.callee()?
        })?;
        let exported = self.export_source_origin(callee, origin)?;
        let state = self.replay.invocation_state(identity, position, owner)?;
        self.replay.symbolic(
            machine,
            self.replay.substitute(machine, state, call, &exported)?,
        )
    }
    pub fn substitute_input_source(
        &mut self,
        identity: MachineId,
        position: DescriptorPosition,
        callee: MachineId,
        actuals: DescriptorActualInputs<'_>,
        origin: &DescriptorSourceOrigin,
    ) -> Option<Vec<DescriptorSourceOrigin>> {
        self.replay.prepare_states(identity)?;
        let machine = self.replay.machine(identity)?;
        let callee = self.replay.machine(callee)?;
        let parameter_count = callee
            .places
            .iter()
            .filter(|row| matches!(row.definition, DescriptorDefinition::Parameter { .. }))
            .count();
        if actuals.arguments.len() != parameter_count
            || actuals.argument_contents.len() != parameter_count
        {
            return None;
        }
        let exported = self.export_source_origin(callee, origin)?;
        let state = self
            .replay
            .snapshots
            .get(&identity)?
            .ready()?
            .before
            .get(&position)?;
        self.replay.symbolic(
            machine,
            self.replay
                .substitute_actuals(machine, state, actuals, &exported)?,
        )
    }

    fn export_source_origin(
        &self,
        callee: &DescriptorMachine,
        origin: &DescriptorSourceOrigin,
    ) -> Option<DescriptorStoredOrigin> {
        Some(match origin {
            DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Invocation {
                carrier_type,
                projection,
            }) => DescriptorStoredOrigin::Invocation {
                carrier_type: *carrier_type,
                projection: projection.clone(),
            },
            DescriptorSourceOrigin::Provider(_) => self
                .replay
                .exported(callee, &Replay::source_origin(origin)?)?,
            DescriptorSourceOrigin::Backing(DescriptorCallOrigin::AccessFormation {
                machine,
                operation,
                place,
                carrier,
                projection,
            }) => DescriptorStoredOrigin::AccessFormation {
                machine: *machine,
                operation: *operation,
                place: *place,
                carrier: *carrier,
                projection: projection.clone(),
            },
            DescriptorSourceOrigin::IncomingSlot(location) => {
                let (position, _, ty) = self.replay.parameter(callee, location.root)?;
                if !self
                    .replay
                    .slot_kind
                    .slots(self.replay.types, location.root, ty)?
                    .contains(location)
                {
                    return None;
                }
                DescriptorStoredOrigin::IncomingSlot {
                    position,
                    path: location.path.clone(),
                }
            }
            DescriptorSourceOrigin::IncomingContents { query, projection } => {
                let (position, _, root_type) = self.replay.parameter(callee, query.root)?;
                query.validate(self.replay.types, root_type)?;
                DescriptorStoredOrigin::IncomingContents {
                    position,
                    steps: query.steps.clone(),
                    projection: callee.export_input_projection(projection),
                }
            }
            DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Argument {
                source,
                projection,
            }) => {
                let (position, _, _) = self.replay.parameter(callee, *source)?;
                DescriptorStoredOrigin::Parameter {
                    position,
                    projection: callee.export_input_projection(projection),
                }
            }
            DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Empty) => {
                DescriptorStoredOrigin::Empty
            }
            DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Literal {
                machine,
                place,
            }) => DescriptorStoredOrigin::Literal {
                machine: *machine,
                place: *place,
            },
        })
    }

    pub fn substitute_call_storage(
        &mut self,
        identity: MachineId,
        position: DescriptorPosition,
        origin: &DescriptorCallOrigin,
    ) -> Option<DescriptorCallOrigin> {
        let owner = self.replay.machine(identity)?.direct_call(position)?.owner;
        self.substitute_call_storage_at(identity, position, owner, origin)
    }
    pub fn substitute_call_storage_at(
        &mut self,
        identity: MachineId,
        position: DescriptorPosition,
        owner: DescriptorInvocationOwner,
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
        let call = machine.invocation(position, owner)?;
        let callee = self.replay.machine(call.stored_effects_callee()?)?;
        let (parameter, _, ty) = self.replay.parameter(callee, *source)?;
        let relative =
            self.replay
                .type_index
                .descriptor_location(*source, ty, &projection.path, None)?;
        let actual = self.replay.actual_slot(call, parameter, &relative.path)?;
        let before = self.replay.invocation_state(identity, position, owner)?;
        let storage = before
            .slots
            .get(&DescriptorLocation::whole(actual.root))?
            .as_ref()?;
        if storage.is_empty()
            || storage
                .iter()
                .any(|origin| !matches!(origin, Origin::Storage(_)))
        {
            return None;
        }
        let mut actual = self
            .replay
            .canonical_storage(machine, before, &actual, None)?;
        if actual.path.is_empty() {
            let values = before.slots.get(&actual)?.as_ref()?;
            let mut values = values.iter();
            if let Origin::Storage(location) = values.next()? {
                if values.next().is_some() {
                    return None;
                }
                actual = location.clone();
            } else {
                return None;
            }
        }
        self.replay.storage_projection(machine, &actual)
    }
    pub fn result_slots(&mut self, identity: MachineId) -> Option<Vec<DescriptorResultSlot>> {
        self.replay.summary(identity, 0)?.result.into_slots()
    }

    /// Keep a failed storage replay available to callers that report
    /// why a result's origins could not be reconstructed.
    pub fn preparation_error(&self, identity: MachineId) -> Option<DescriptorGraphError> {
        match self.replay.snapshots.get(&identity)? {
            super::engine::PreparedStateCache::Failed(error) => Some(*error),
            _ => None,
        }
    }

    pub fn result_contents(&mut self, identity: MachineId) -> Option<DescriptorResultContents> {
        let summary = self.replay.summary(identity, 0)?;
        let super::engine::ResultObservations::Observed {
            slots,
            contents,
            unknown,
        } = summary.result
        else {
            return None;
        };
        Some(DescriptorResultContents {
            slots: slots?,
            unknown_contents: contents,
            unresolved_slots: unknown,
            case_facts: summary.result_cases,
        })
    }

    pub(crate) fn call_result_contents(
        &self,
        identity: MachineId,
        at: DescriptorPosition,
        operation: semantic_vocabulary::OperationId,
    ) -> Option<Vec<DescriptorResultSlot>> {
        let machine = self.replay.machine(identity)?;
        let call = crate::exactly_one::exactly_one(machine.calls_at(at).iter(), |call| {
            call.owner == DescriptorInvocationOwner::Operation(operation)
        })?;
        let summary = self.replay.cached_invocation_summary(call)?;
        Some(summary.result.contents().to_vec())
    }

    /// A helper can update referenced storage while leaving its descriptor
    /// unchanged. Every normal return must retain the invocation's original
    /// descriptor; facts about the stored data still need invalidation.
    pub fn call_preserves_descriptor_storage(
        &mut self,
        caller: MachineId,
        position: DescriptorPosition,
        parameter: usize,
        path: &[DescriptorPathSegment],
    ) -> Result<bool, &'static str> {
        let callee = {
            let machine = self
                .replay
                .machine(caller)
                .ok_or("descriptor preservation has no caller")?;
            let Some(call) = machine.direct_call(position) else {
                return Ok(false);
            };
            let Some(callee) = call.stored_effects_callee() else {
                return Ok(false);
            };
            callee
        };
        self.preserves_descriptor_storage(callee, parameter, path)
    }

    pub fn preserves_descriptor_storage(
        &mut self,
        callee: MachineId,
        parameter: usize,
        path: &[DescriptorPathSegment],
    ) -> Result<bool, &'static str> {
        let descriptor = {
            let body = self
                .replay
                .machine(callee)
                .ok_or("descriptor preservation has no callee")?;
            let root = exactly_one(body.places.iter(), |place| {
                matches!(place.definition,
                DescriptorDefinition::Parameter { position, .. } if position == parameter)
            })
            .ok_or("descriptor preservation has no unique parameter")?;
            let root_type = *body
                .place_types
                .get(&root.place)
                .ok_or("descriptor parameter has no type")?;
            let Some(selected) = self
                .replay
                .type_index
                .descriptor_location_type(root_type, path)
            else {
                return Ok(false);
            };
            if !self.replay.slot_kind.is_slot(self.replay.types, selected) {
                return Ok(false);
            }
            DescriptorLocation {
                root: root.place,
                path: path.to_vec(),
            }
        };
        if self.replay.prepare_states(callee).is_none() {
            return Ok(false);
        }
        let body = self
            .replay
            .machine(callee)
            .ok_or("descriptor preservation owner disappeared")?;
        let states = self
            .replay
            .snapshots
            .get(&callee)
            .and_then(|cached| cached.ready())
            .map(|prepared| &prepared.normal_exits)
            .ok_or("descriptor preservation snapshots disappeared")?;
        // A loop can replace the descriptor before revisiting the entry block.
        // Compare returns with invocation inputs, not the widened loop state.
        let Some(entry) = self.replay.initial(body) else {
            return Ok(false);
        };
        let Some(incoming) = self
            .replay
            .read(body, &entry, &descriptor)
            .filter(|value| !value.is_empty())
        else {
            return Ok(false);
        };
        let mut returns = false;
        for exit in &body.normal_exits {
            let Some(state) = states.get(exit) else {
                continue;
            };
            returns = true;
            if self.replay.read(body, state, &descriptor).as_ref() != Some(&incoming) {
                return Ok(false);
            }
        }
        Ok(returns)
    }

    pub fn stored_effects(
        &mut self,
        identity: MachineId,
    ) -> Option<Vec<DescriptorStoredEffect>> {
        let summary = self.replay.summary(identity, 0)?;
        if summary.invalidates_contents {
            return None;
        }
        summary.effects.into_observed()
    }
    pub fn call_stored_effects(
        &mut self,
        identity: MachineId,
        position: DescriptorPosition,
    ) -> Option<Vec<DescriptorStoredCallEffect>> {
        let owner = self.replay.machine(identity)?.direct_call(position)?.owner;
        self.call_stored_effects_at(identity, position, owner)
    }
    pub fn call_stored_effects_at(
        &mut self,
        identity: MachineId,
        position: DescriptorPosition,
        owner: DescriptorInvocationOwner,
    ) -> Option<Vec<DescriptorStoredCallEffect>> {
        let effects = self.symbolic_call_stored_effects_at(identity, position, owner)?;
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
        let owner = self.replay.machine(identity)?.direct_call(position)?.owner;
        self.symbolic_call_stored_effects_at(identity, position, owner)
    }
    pub fn symbolic_call_stored_effects_at(
        &mut self,
        identity: MachineId,
        position: DescriptorPosition,
        owner: DescriptorInvocationOwner,
    ) -> Option<Vec<DescriptorStoredCallEffect<DescriptorSourceOrigin>>> {
        self.replay.prepare_states(identity)?;
        let machine = self.replay.machine(identity)?;
        let before = self.replay.invocation_state(identity, position, owner)?;
        let call = machine.invocation(position, owner)?;
        if call.argument_contents.iter().any(Option::is_none) {
            return None;
        }
        let summary = self.replay.cached_invocation_summary(call)?;
        if summary.invalidates_contents {
            return None;
        }
        let mut replacements = BTreeMap::new();
        for effect in summary.effects.observed()? {
            let destination =
                self.replay
                    .actual_contents(machine, call, effect.parameter, &effect.steps)?;
            let mut origins = BTreeSet::new();
            for origin in &effect.origins {
                origins.extend(self.replay.substitute(machine, before, call, origin)?);
            }
            self.replay.contents_replacements(
                machine,
                before,
                &destination,
                Some(origins),
                &mut replacements,
            )?;
        }
        replacements
            .into_iter()
            .map(|(target, value)| {
                let destination = match target {
                    DescriptorPayloadTarget::Storage {
                        location,
                        completed: None,
                    } => DescriptorContentsQuery::from_location(&location),
                    DescriptorPayloadTarget::Storage {
                        completed: Some(_), ..
                    } => return None,
                    DescriptorPayloadTarget::Incoming(query) => query,
                };
                let origins = value?;
                let preserves_incoming = self
                    .replay
                    .read_contents(machine, before, &destination)
                    .is_some_and(|previous| previous.is_subset(&origins));
                Some(DescriptorStoredCallEffect {
                    destination,
                    origins: self.replay.symbolic(machine, origins)?,
                    preserves_incoming,
                })
            })
            .collect()
    }
    pub fn invocation_flow_at(
        &mut self,
        identity: MachineId,
        position: DescriptorPosition,
        owner: DescriptorInvocationOwner,
    ) -> Result<DescriptorFlowStep, DescriptorGraphError> {
        self.replay.prepare_states_checked(identity)?;
        let machine = self
            .replay
            .machine(identity)
            .ok_or(DescriptorGraphError::MissingOrAmbiguousMachine(identity))?;
        let call = machine.invocation(position, owner).ok_or(
            DescriptorGraphError::IncompleteMachine {
                machine: identity,
                position: Some(position),
                successor: None,
                detail: None,
            },
        )?;
        let before = self
            .replay
            .invocation_state(identity, position, owner)
            .ok_or(DescriptorGraphError::IncompleteMachine {
                machine: identity,
                position: Some(position),
                successor: None,
                detail: None,
            })?;
        let mut step = DescriptorFlowStep::default();
        self.append_invocation_flow(machine, position, call, before, &mut step, None)?;
        Ok(step)
    }
    pub fn flow_arc(
        &mut self,
        identity: MachineId,
    ) -> Result<std::sync::Arc<BTreeMap<DescriptorPosition, DescriptorFlowStep>>, DescriptorGraphError>
    {
        if let Ok(index) = self.flows.binary_search_by_key(&identity, |(id, _)| *id) {
            return Ok(std::sync::Arc::clone(&self.flows[index].1));
        }
        let prepared = std::sync::Arc::new(self.prepare_flow(identity, None)?);
        let index = self
            .flows
            .binary_search_by_key(&identity, |(id, _)| *id)
            .unwrap_or_else(|index| index);
        self.flows
            .insert(index, (identity, std::sync::Arc::clone(&prepared)));
        Ok(prepared)
    }

    pub fn flow(
        &mut self,
        identity: MachineId,
    ) -> Result<BTreeMap<DescriptorPosition, DescriptorFlowStep>, DescriptorGraphError>
    {
        self.flow_arc(identity).map(|arc| (*arc).clone())
    }

    pub fn residence_flow(
        &mut self,
        identity: MachineId,
        rules: &mut crate::descriptors::descriptor_lifetimes::DescriptorResidenceRules,
    ) -> Result<BTreeMap<DescriptorPosition, DescriptorFlowStep>, DescriptorGraphError> {
        let formations = std::mem::take(&mut rules.captures);
        let mut flow = self.prepare_flow(identity, Some(rules))?;
        for (position, source, destination) in formations {
            let failed = |detail| DescriptorGraphError::IncompleteMachine {
                machine: identity,
                position: Some(position),
                successor: None,
                detail: Some(detail),
            };
            let destination = self
                .location(identity, position, &destination)
                .ok_or_else(|| failed("captured reference has no local destination"))?;
            if let Some(source) = self.location(identity, position, &source) {
                rules.captures.insert((position, source, destination));
                continue;
            }
            // A block input can borrow an indexed element without a local
            // storage home. Keep each possible backing residence live through
            // the new reference. An unknown address cannot supply this edge.
            let origins = self
                .captured_storage_family(identity, position, &source)
                .map_err(failed)?;
            let machine = self
                .replay
                .machine(identity)
                .ok_or(DescriptorGraphError::MissingOrAmbiguousMachine(identity))?;
            for origin in origins {
                let DescriptorCallOrigin::Argument { source, projection } = origin else {
                    return Err(failed("captured residence has no input-relative storage"));
                };
                if projection
                    .has_captured_reference_crossing()
                    .map_err(failed)?
                {
                    return Err(failed(
                        "captured residence requires the storage on the far side of a reference",
                    ));
                }
                let fixed = machine.export_input_projection(&projection);
                let path = fixed
                    .captured_storage_path()
                    .ok_or_else(|| failed("captured residence has no fixed inline address"))?;
                let location = descriptor_location(
                    self.replay.types,
                    source,
                    *machine
                        .place_types
                        .get(&source)
                        .ok_or_else(|| failed("captured residence has no local storage type"))?,
                    &path,
                    None,
                )
                .ok_or_else(|| failed("captured residence does not name storage in its input"))?;
                let edge = (location, destination.clone());
                let step = flow
                    .get_mut(&position)
                    .ok_or_else(|| failed("captured residence has no flow step"))?;
                if !step.derivations.contains(&edge) {
                    step.derivations.push(edge.clone());
                }
                rules.captures.insert((position, edge.0, edge.1));
            }
        }
        Ok(flow)
    }

    fn prepare_flow(
        &mut self,
        identity: MachineId,
        mut residence_rules: Option<
            &mut crate::descriptors::descriptor_lifetimes::DescriptorResidenceRules,
        >,
    ) -> Result<BTreeMap<DescriptorPosition, DescriptorFlowStep>, DescriptorGraphError> {
        let incomplete = |position| DescriptorGraphError::IncompleteMachine {
            machine: identity,
            position,
            successor: None,
            detail: None,
        };
        self.replay.prepare_states_checked(identity)?;
        let machine = self
            .replay
            .machine(identity)
            .ok_or_else(|| incomplete(None))?;
        let states = self
            .replay
            .snapshots
            .get(&identity)
            .and_then(|cached| cached.ready())
            .map(|prepared| prepared.before.clone())
            .ok_or_else(|| incomplete(None))?;
        let mut flow = machine.flow.clone();
        let normal_exits = machine
            .normal_exits
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        for (position, step) in &mut flow {
            let Some(before) = states.get(position) else {
                continue;
            };
            let incomplete_step = || incomplete(Some(*position));
            let call = machine.direct_call(*position);
            // A projected address need not have a static descriptor location.
            // Its checked callee still has a body summary. When there is no
            // borrowed result, stored effect, or invalidation, the address is
            // observed at the invocation and need not survive its return.
            if let Some(call) = call
                && step.may_escape
                && matches!(call.owner, DescriptorInvocationOwner::Operation(_))
                && matches!(call.target, DescriptorInvocationTarget::Machine(_))
                && call.arguments.iter().any(Option::is_none)
                && call.result.is_none()
                && let Some(summary) = self.replay.invocation_summary(call, 0)
                && !summary.result.is_unobserved()
                && !summary.invalidates_contents
                && summary.effects.observed().is_some_and(Vec::is_empty)
            {
                step.may_escape = false;
            }
            let replay = &self.replay;
            let mut canonical = |location: &DescriptorLocation| {
                let resolution = Self::resolved_storage(replay, machine, before, location);
                replay
                    .flow_location(machine, before, location, resolution)
                    .map_err(|detail| DescriptorGraphError::IncompleteMachine {
                        machine: identity,
                        position: Some(*position),
                        successor: None,
                        detail: Some(detail),
                    })
            };
            step.definitions = step
                .definitions
                .iter()
                .map(&mut canonical)
                .collect::<Result<_, _>>()?;
            if !step.definitions.is_empty() {
                let aliases = before
                    .slots
                    .keys()
                    .filter_map(|location| {
                        let canonical_location = canonical(location).ok()?;
                        step.definitions
                            .iter()
                            .any(|definition| definition.contains(&canonical_location))
                            .then_some(location.clone())
                    })
                    .collect::<Vec<_>>();
                step.definitions.extend(aliases);
            }
            step.definitions.sort();
            step.definitions.dedup();
            // This is a derived loan-liveness account. Canonical alias
            // observations do not become additional memory operations.
            let mut observations = step
                .observed_inputs()
                .map(&mut canonical)
                .collect::<Result<Vec<_>, _>>()?;
            for input in step.observed_inputs() {
                if input.path.is_empty()
                    && let Some(Some(origins)) = before.slots.get(input)
                {
                    observations.extend(origins.iter().filter_map(|origin| match origin {
                        Origin::Storage(location) => Some(location.clone()),
                        Origin::CapturedStorage(account) => account.local_coordinate().cloned(),
                        _ => None,
                    }));
                }
            }
            let mut inputs: Vec<_> = step.observed_inputs().cloned().collect();
            inputs.extend(observations);
            inputs.sort();
            inputs.dedup();
            step.inputs = inputs
                .into_iter()
                .map(|input| (Some(input), None))
                .collect();
            step.derivations = step
                .derivations
                .iter()
                .map(|(source, destination)| Ok((canonical(source)?, canonical(destination)?)))
                .collect::<Result<_, DescriptorGraphError>>()?;
            step.value_transfers = step
                .value_transfers
                .iter()
                .map(|(source, destination)| Ok((canonical(source)?, canonical(destination)?)))
                .collect::<Result<_, DescriptorGraphError>>()?;
            step.scalar_value_stores = step
                .scalar_value_stores
                .iter()
                .map(|(destination, source)| Ok((canonical(destination)?, *source)))
                .collect::<Result<_, DescriptorGraphError>>()?;
            step.authority_derivations = step
                .authority_derivations
                .iter()
                .map(|(source, destination)| Ok((canonical(source)?, canonical(destination)?)))
                .collect::<Result<_, DescriptorGraphError>>()?;
            for edge in &mut step.successors {
                edge.definitions = edge
                    .definitions
                    .iter()
                    .map(&mut canonical)
                    .collect::<Result<_, DescriptorGraphError>>()?;
            }
            if residence_rules.is_some() {
                for edge in &mut step.successors {
                    for binding in &mut edge.bindings {
                        if let crate::descriptors::descriptor_lifetimes::DescriptorFlowSource::Location(source) =
                            &mut binding.source
                        {
                            *source = canonical(source)?;
                        }
                    }
                }
            }
            if let Some(call) =
                call.filter(|call| call.argument_contents.iter().all(Option::is_some))
            {
                self.append_invocation_flow(
                    machine,
                    *position,
                    call,
                    before,
                    step,
                    residence_rules.as_deref_mut(),
                )?;
            }
            for edge in &mut step.successors {
                edge.effects.clear();
            }
            let edge_positions = step
                .successors
                .iter()
                .enumerate()
                .filter_map(|(index, edge)| edge.edge.map(|edge| (edge, index)))
                .collect::<BTreeMap<_, _>>();
            for call in machine.calls_at(*position) {
                let DescriptorInvocationOwner::Cleanup {
                    edge: owner_edge, ..
                } = call.owner
                else {
                    continue;
                };
                let Some(before) = self
                    .replay
                    .invocation_state(identity, *position, call.owner)
                else {
                    continue;
                };
                let mut effect = DescriptorFlowStep::default();
                self.append_invocation_flow(
                    machine,
                    *position,
                    call,
                    before,
                    &mut effect,
                    residence_rules.as_deref_mut(),
                )?;
                step.inputs.extend(effect.inputs.iter().cloned());
                step.inputs.extend(
                    effect
                        .derivations
                        .iter()
                        .map(|(source, _)| (Some(source.clone()), None)),
                );
                step.may_escape |= effect.may_escape;
                if let Some(index) = edge_positions.get(&owner_edge) {
                    step.successors[*index].effects.push((call.owner, effect));
                } else if !normal_exits.contains(position) {
                    return Err(incomplete_step());
                }
            }
            let mut inputs: Vec<_> = step.observed_inputs().cloned().collect();
            inputs.sort_unstable();
            inputs.dedup();
            step.replace_value_inputs(inputs);
        }
        Ok(flow)
    }
    #[allow(clippy::too_many_arguments)]
    fn append_invocation_flow(
        &self,
        machine: &DescriptorMachine,
        position: DescriptorPosition,
        call: &DescriptorInvocation,
        before: &super::engine::State,
        step: &mut DescriptorFlowStep,
        mut residence_rules: Option<
            &mut crate::descriptors::descriptor_lifetimes::DescriptorResidenceRules,
        >,
    ) -> Result<(), DescriptorGraphError> {
        let incomplete_step = || DescriptorGraphError::IncompleteMachine {
            machine: machine.id,
            position: Some(position),
            successor: None,
            detail: None,
        };
        let canonical = |location: &DescriptorLocation| {
            let resolution = self.replay.resolve_storage(machine, before, location, None);
            self.replay
                .flow_location(machine, before, location, resolution)
                .map_err(|detail| DescriptorGraphError::IncompleteMachine {
                    machine: machine.id,
                    position: Some(position),
                    successor: None,
                    detail: Some(detail),
                })
        };
        let summary = self
            .replay
            .cached_invocation_summary(call)
            .ok_or_else(incomplete_step)?;
        step.may_escape |= summary.invalidates_contents;
        let mut outputs = Vec::new();
        if let EffectObservations::Unobserved(writes) = &summary.effects {
            let unknown = self
                .replay
                .unobserved_effect_replacements(
                    machine,
                    before,
                    call,
                    writes,
                    &DescriptorReadDepth::at(machine, before, None),
                    |_| {},
                )
                .ok_or_else(incomplete_step)?;
            if !unknown.is_empty() {
                step.may_escape = true;
                for (argument, contents) in call.arguments.iter().zip(&call.argument_contents) {
                    let input = match argument {
                        Some(source) => canonical(source)?,
                        None => contents.as_ref().ok_or_else(incomplete_step)?.observation(),
                    };
                    step.inputs.push((Some(input), None));
                }
            }
            for target in unknown.keys() {
                match target {
                    DescriptorPayloadTarget::Storage {
                        location,
                        completed: None,
                    } => step.definitions.push(canonical(location)?),
                    DescriptorPayloadTarget::Incoming(query) => {
                        step.inputs.push((Some(query.observation()), None));
                        step.may_escape = true;
                    }
                    DescriptorPayloadTarget::Storage {
                        completed: Some(_), ..
                    } => return Err(incomplete_step()),
                }
            }
        }
        for effect in summary.effects.observed().into_iter().flatten() {
            // The callee returns with this storage holding what it
            // held on entry: nothing was written, so no descriptor
            // is defined, copied or handed anywhere here. Reading it
            // as a write from unknown contents would keep every loan
            // the call uses live to the end of the machine.
            if !effect.origins.is_empty()
                && effect
                    .origins
                    .iter()
                    .all(|origin| effect.keeps_incoming(origin))
            {
                continue;
            }
            let query = self
                .replay
                .actual_contents(machine, call, effect.parameter, &effect.steps)
                .ok_or_else(incomplete_step)?;
            let targets = self
                .replay
                .contents_targets(machine, before, &query)
                .ok_or_else(incomplete_step)?;
            let strong = targets.len() == 1
                && matches!(
                    targets.first(),
                    Some(DescriptorPayloadTarget::Storage {
                        completed: None,
                        ..
                    })
                );
            for target in targets {
                match target {
                    DescriptorPayloadTarget::Storage {
                        location: destination,
                        completed: None,
                    } => {
                        outputs.push((destination, effect.origins.as_slice(), strong));
                    }
                    DescriptorPayloadTarget::Storage {
                        completed: Some(_), ..
                    } => return Err(incomplete_step()),
                    DescriptorPayloadTarget::Incoming(query) => {
                        step.inputs.push((Some(query.observation()), None));
                        step.may_escape = true;
                        for origin in &effect.origins {
                            match origin {
                                DescriptorStoredOrigin::IncomingSlot { position, path } => {
                                    step.inputs.push((
                                        Some(
                                            self.replay
                                                .actual_slot(call, *position, path)
                                                .ok_or_else(incomplete_step)?,
                                        ),
                                        None,
                                    ))
                                }
                                DescriptorStoredOrigin::IncomingContents {
                                    position,
                                    steps,
                                    ..
                                } => step.inputs.push((
                                    Some(
                                        self.replay
                                            .actual_contents(machine, call, *position, steps)
                                            .ok_or_else(incomplete_step)?
                                            .observation(),
                                    ),
                                    None,
                                )),
                                DescriptorStoredOrigin::Parameter { position, .. } => {
                                    step.inputs.push((
                                        Some(
                                            call.arguments
                                                .get(*position)
                                                .and_then(Option::as_ref)
                                                .ok_or_else(incomplete_step)?
                                                .clone(),
                                        ),
                                        None,
                                    ))
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
        if let Some(result) = &call.result {
            let result_type = *machine
                .place_types
                .get(&result.root)
                .ok_or_else(incomplete_step)?;
            let slots: &[DescriptorResultSlot] = if summary.result.is_unobserved() {
                &[]
            } else {
                summary
                    .result
                    .slots()
                    .ok_or_else(incomplete_step)?
                    .as_slice()
            };
            for slot in slots {
                slot.visit(result_type, &mut Vec::new(), &mut |path, _, slot| {
                    let mut destination = result.clone();
                    destination.path.extend_from_slice(path);
                    outputs.push((destination, slot.origins.as_slice(), true));
                    Ok::<(), DescriptorGraphError>(())
                })?;
            }
            for packet in summary.result.contents() {
                let mut destination = result.clone();
                destination.path.extend_from_slice(&packet.path);
                let expected = self
                    .replay
                    .unknown_contents_type(machine, &destination)
                    .ok_or_else(incomplete_step)?;
                step.definitions.push(destination.clone());
                if packet.origins.is_empty() {
                    if !summary
                        .result_cases
                        .excludes_payload(result_type, &packet.path)
                    {
                        return Err(incomplete_step());
                    }
                    continue;
                }
                for origin in &packet.origins {
                    if let DescriptorStoredOrigin::ProducedContents(produced) = origin {
                        if produced.structural_type != expected {
                            return Err(incomplete_step());
                        }
                        outputs.push((destination.clone(), std::slice::from_ref(origin), true));
                        continue;
                    }
                    let source = self
                        .replay
                        .returned_packet_input(machine, before, call, origin, expected)
                        .ok_or(DescriptorGraphError::IncompleteMachine {
                            machine: machine.id,
                            position: Some(position),
                            successor: None,
                            detail: Some("the returned generic value has no matching input contents"),
                        })?;
                    step.inputs.push((Some(source.clone()), None));
                    step.derivations.push((source, destination.clone()));
                }
            }
            // A result slot of unknown origin may be any descriptor
            // the call was given, or reach through one: everything
            // passed in stays live for as long as that slot is used.
            // lifetimes.md, "Carried-loan transport": an unresolved
            // selection "conservatively retains all possible sources".
            let unknown = if summary.result.is_unobserved() {
                self.replay
                    .slot_kind
                    .owned_slots(self.replay.types, result.root, result_type)
                    .ok_or_else(incomplete_step)?
                    .into_iter()
                    .map(|slot| slot.path)
                    .collect::<Vec<_>>()
            } else {
                summary.result.unknown().to_vec()
            };
            for path in &unknown {
                let mut destination = result.clone();
                destination.path.extend_from_slice(path);
                let destination = canonical(&destination)?;
                if !step.definitions.contains(&destination) {
                    step.definitions.push(destination.clone());
                }
                for (argument, contents) in call.arguments.iter().zip(&call.argument_contents) {
                    match argument {
                        Some(source) => step
                            .derivations
                            .push((canonical(source)?, destination.clone())),
                        None => {
                            step.inputs.push((
                                Some(
                                    contents
                                        .as_ref()
                                        .ok_or_else(incomplete_step)?
                                        .observation(),
                                ),
                                None,
                            ));
                            step.may_escape = true;
                        }
                    }
                }
            }
        }
        for (destination, origins, strong) in outputs {
            let destination = canonical(&destination)?;
            if strong && !step.definitions.contains(&destination) {
                step.definitions.push(destination.clone());
            }
            for location in before.slots.keys() {
                if strong
                    && let Some(canonical) = self
                        .replay
                        .canonical_storage(machine, before, location, None)
                    && destination.contains(&canonical)
                    && !step.definitions.contains(location)
                {
                    step.definitions.push(location.clone());
                }
            }
            // A provider's inputs constrain the result's lifetime,
            // not its address. Keep those loans alive without making
            // any input the provider view's spatial source.
            let mut pending = origins
                .iter()
                .map(|origin| (origin, true))
                .collect::<Vec<_>>();
            while let Some((origin, spatial)) = pending.pop() {
                let dependencies = match origin {
                    DescriptorStoredOrigin::Provider(provider) => {
                        Some(&provider.lifetime_dependencies)
                    }
                    DescriptorStoredOrigin::ProducedContents(contents) => {
                        Some(&contents.lifetime_dependencies)
                    }
                    _ => None,
                };
                if let Some(dependencies) = dependencies {
                    pending.extend(
                        dependencies
                            .iter()
                            .map(|dependency| (&dependency.origin, false)),
                    );
                    continue;
                }
                let source = match origin {
                    DescriptorStoredOrigin::Invocation { .. }
                    | DescriptorStoredOrigin::Literal { .. }
                    | DescriptorStoredOrigin::Empty
                    | DescriptorStoredOrigin::Carried { .. }
                    | DescriptorStoredOrigin::Provider(_)
                    | DescriptorStoredOrigin::ProducedContents(_)
                    | DescriptorStoredOrigin::AccessFormation { .. } => {
                        continue;
                    }
                    DescriptorStoredOrigin::IncomingSlot { position, path } => {
                        let Some(source) = call
                            .arguments
                            .get(*position)
                            .ok_or_else(incomplete_step)?
                            .as_ref()
                        else {
                            let mut query = call
                                .argument_contents
                                .get(*position)
                                .and_then(Option::as_ref)
                                .ok_or_else(incomplete_step)?
                                .clone();
                            query.steps.extend(
                                path.iter().cloned().map(DescriptorContentsStep::Address),
                            );
                            step.inputs.push((Some(query.observation()), None));
                            step.may_escape = true;
                            continue;
                        };
                        let mut source = source.clone();
                        source.path.extend_from_slice(path);
                        source
                    }
                    DescriptorStoredOrigin::IncomingContents {
                        position, steps, ..
                    } => {
                        let mut query = call
                            .argument_contents
                            .get(*position)
                            .and_then(Option::as_ref)
                            .ok_or_else(incomplete_step)?
                            .clone();
                        query.steps.extend_from_slice(steps);
                        step.inputs.push((Some(query.observation()), None));
                        step.may_escape = true;
                        continue;
                    }
                    DescriptorStoredOrigin::Parameter {
                        position,
                        projection,
                    } => {
                        let Some(source) = call
                            .arguments
                            .get(*position)
                            .ok_or_else(incomplete_step)?
                            .as_ref()
                        else {
                            let query = call
                                .argument_contents
                                .get(*position)
                                .and_then(Option::as_ref)
                                .ok_or_else(incomplete_step)?;
                            step.inputs.push((Some(query.observation()), None));
                            step.may_escape = true;
                            continue;
                        };
                        let mut source = source.clone();
                        if !projection.path.is_empty() {
                            let root_type = *machine
                                .place_types
                                .get(&source.root)
                                .ok_or_else(incomplete_step)?;
                            let mut spelling =
                                descriptor_path(self.replay.types, root_type, &source.path)
                                    .ok_or_else(incomplete_step)?;
                            spelling.extend_from_slice(&projection.path);
                            let types = self.replay.types;
                            if let Some(home) =
                                descriptor_location(types, source.root, root_type, &spelling, None)
                            {
                                source = home;
                            } else {
                                // An element of a view lives in its backing storage.
                                // Keep the lending descriptor live without treating
                                // its header as the returned element's address.
                                let segments =
                                    crate::resolved_structural_path(types.iter(), root_type, &spelling)
                                        .map_err(|_| incomplete_step())?;
                                let element = crate::descriptor_lifetimes::view_element_descriptor_prefix(
                                    types, root_type, &segments,
                                )
                                .map_err(|_| incomplete_step())?
                                .ok_or_else(incomplete_step)?;
                                let descriptor = DescriptorLocation::captured(source.root, &segments[..element])
                                    .ok_or_else(incomplete_step)?;
                                step.derivations.push((canonical(&descriptor)?, destination.clone()));
                                continue;
                            }
                        }
                        source
                    }
                };
                let pair = (canonical(&source)?, destination.clone());
                if spatial
                    && matches!(origin, DescriptorStoredOrigin::Parameter { .. })
                    && let Some(rules) = residence_rules.as_deref_mut()
                {
                    let source_type = descriptor_location_type(
                        self.replay.types,
                        *machine
                            .place_types
                            .get(&pair.0.root)
                            .ok_or_else(incomplete_step)?,
                        &pair.0.path,
                    )
                    .ok_or_else(incomplete_step)?;
                    if !DescriptorSlotKind::Views.is_slot(self.replay.types, source_type) {
                        rules
                            .captures
                            .insert((position, pair.0.clone(), pair.1.clone()));
                    }
                }
                step.derivations.push(pair.clone());
                if spatial
                    && strong
                    && match origin {
                        DescriptorStoredOrigin::Parameter { projection, .. } => {
                            // Descriptor permission follows its exact
                            // original input even when a representation
                            // view forgets element addresses. That does
                            // not recover geometry or authorize a
                            // different field. A reference to parameter
                            // storage still needs its retained call loan.
                            self.replay.slot_kind != DescriptorSlotKind::References
                                && projection.original_descriptor_path().is_some()
                                && !projection.indirect
                                && projection.window.is_none()
                                && projection.captured_at.is_none()
                        }
                        DescriptorStoredOrigin::IncomingSlot { .. } => true,
                        _ => false,
                    }
                {
                    step.authority_derivations.push(pair);
                }
            }
        }
        Ok(())
    }
}
