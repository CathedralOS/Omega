use super::*;

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
    pub(in crate::descriptor_results) fn compatibility_machines(&self) -> &[DescriptorMachine] {
        self.replay.machines
    }
    pub(in crate::descriptor_results) fn compatibility_ingress(
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
    /// Complete storage addresses at this exact current-body position. This
    /// never flattens descriptor contents or incoming slots into owned storage.
    pub fn storage_origins(
        &mut self,
        identity: MachineId,
        position: DescriptorPosition,
        location: &DescriptorLocation,
    ) -> Option<Vec<DescriptorLocation>> {
        self.replay.prepare_states(identity)?;
        let machine = self.replay.machine(identity)?;
        let states = self.replay.snapshots.get(&identity)?.as_ref()?;
        self.replay
            .storage_origins(machine, states.get(&position)?, location)
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
        let slot = summary
            .result
            .as_ref()?
            .iter()
            .find(|slot| slot.path.is_empty())?;
        for origin in &slot.origins {
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
                for slot in summary.result.as_ref()? {
                    let mut destination = result.clone();
                    destination.path.extend_from_slice(&slot.path);
                    outputs.push((destination, slot.origins.as_slice()));
                }
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
