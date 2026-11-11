/*
Position-sensitive descriptor contents replay engine (`DescriptorAnalysis`, `reconstruct_source_origins`,
`reconstruct_stored_effects`) tracking descriptor slot origins, case-flow narrowing, and symbolic
incoming-family joins across `TerminalMachine` control-flow graphs.
*/

use crate::descriptors::descriptor_lifetimes::{DescriptorPathSegment, descriptor_path};
use crate::descriptors::descriptor_results::*;
use crate::exactly_one::exactly_one;

pub(crate) mod analysis {
    use super::engine::Origin;
    use super::*;
    use crate::descriptors::descriptor_lifetimes::descriptor_location;
    use crate::descriptors::descriptor_lifetimes::descriptor_location_type;

    pub(crate) mod call_origins {
        use super::*;

        impl DescriptorAnalysis<'_> {
            pub fn call_origins(
                &mut self,
                identity: MachineId,
                result: PlaceId,
            ) -> Option<Vec<DescriptorCallOrigin>> {
                self.try_call_origins(identity, result).ok()
            }

            pub fn try_call_origins(
                &mut self,
                identity: MachineId,
                result: PlaceId,
            ) -> Result<Vec<DescriptorCallOrigin>, &'static str> {
                self.replay
                    .prepare_states(identity)
                    .ok_or("caller descriptor snapshots could not be reconstructed")?;
                let machine = self
                    .replay
                    .machine(identity)
                    .ok_or("caller descriptor graph is absent")?;
                let mut calls = machine.calls.iter().filter(|(_, call)| {
                    call.result
                        .as_ref()
                        .is_some_and(|location| *location == DescriptorLocation::whole(result))
                });
                let (position, call) = calls
                    .next()
                    .ok_or("descriptor result has no producing call")?;
                if calls.next().is_some() {
                    return Err("descriptor result has multiple producing calls");
                }
                if call.argument_contents.iter().any(Option::is_none) {
                    return Err("descriptor call has an unresolved argument contents coordinate");
                }
                let states = self
                    .replay
                    .snapshots
                    .get(&identity)
                    .and_then(Option::as_ref)
                    .ok_or("caller descriptor snapshots disappeared")?;
                let summary = self
                    .replay
                    .cached_invocation_summary(call)
                    .ok_or("callee descriptor summary could not be reconstructed")?;
                let mut origins = BTreeSet::new();
                for slot in summary
                    .result
                    .as_ref()
                    .ok_or("callee has no normal descriptor result summary")?
                {
                    for origin in &slot.origins {
                        origins.extend(
                            self.replay
                                .substitute_result(
                                    machine,
                                    states.get(position).ok_or(
                                        "descriptor call has no reachable pre-call snapshot",
                                    )?,
                                    call,
                                    origin,
                                )
                                .ok_or(
                                    "descriptor result origin could not be substituted at the call",
                                )?,
                        );
                    }
                }
                self.replay
                    .resolved(machine, origins)
                    .ok_or("descriptor result backing union could not be resolved")
            }
        }
    }

    pub struct DescriptorAnalysis<'a> {
        replay: Replay<'a>,
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
                        .inputs
                        .iter()
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
                            if descriptor_location_type(self.replay.types, *root, path)
                                .and_then(|leaf| {
                                    terminal_psi::find_structural_type(self.replay.types, leaf)
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
                .and_then(Option::as_ref);
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
                .and_then(Option::as_ref)
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
                .and_then(Option::as_ref)
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
                .and_then(Option::as_ref)
                .and_then(|states| states.get(&position))
            else {
                return false;
            };
            super::case_flow::established(&self.replay, machine, state, predicate)
        }
        pub fn with_reference_slots(
            types: &'a [StructuralTypeDeclaration],
            machines: &'a [DescriptorMachine],
        ) -> Self {
            Self {
                replay: Replay::with_slot_kind(types, machines, DescriptorSlotKind::References),
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
            }
        }
        pub fn with_access_origins(
            types: &'a [StructuralTypeDeclaration],
            machines: &'a [DescriptorMachine],
        ) -> Self {
            Self {
                replay: Replay::with_slot_kind(types, machines, DescriptorSlotKind::AccessOrigins),
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
            descriptor_location(self.replay.types, *source, ty, &projection.path, None)?;
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
            let states = self.replay.snapshots.get(&identity)?.as_ref()?;
            self.replay
                .canonical_storage(machine, states.get(&position)?, location)
        }
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
        pub fn payload_targets(
            &mut self,
            identity: MachineId,
            position: DescriptorPosition,
            query: &DescriptorContentsQuery,
        ) -> Option<Vec<DescriptorPayloadTarget>> {
            self.replay.prepare_states(identity)?;
            let machine = self.replay.machine(identity)?;
            let states = self.replay.snapshots.get(&identity)?.as_ref()?;
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
            let states = self.replay.arrival_snapshots.get(&identity)?.as_ref()?;
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
            let states = self.replay.snapshots.get(&identity)?.as_ref()?;
            self.replay.symbolic(
                machine,
                self.replay
                    .read(machine, states.get(&position)?, location)?,
            )
        }
        pub fn substitute_call_source(
            &mut self,
            identity: MachineId,
            position: DescriptorPosition,
            origin: &DescriptorSourceOrigin,
        ) -> Option<Vec<DescriptorSourceOrigin>> {
            self.substitute_call_source_with_target(identity, position, origin, false)
        }
        pub fn substitute_stored_effect_source(
            &mut self,
            identity: MachineId,
            position: DescriptorPosition,
            origin: &DescriptorSourceOrigin,
        ) -> Option<Vec<DescriptorSourceOrigin>> {
            self.substitute_call_source_with_target(identity, position, origin, true)
        }
        fn substitute_call_source_with_target(
            &mut self,
            identity: MachineId,
            position: DescriptorPosition,
            origin: &DescriptorSourceOrigin,
            stored_effects: bool,
        ) -> Option<Vec<DescriptorSourceOrigin>> {
            self.replay.prepare_states(identity)?;
            let machine = self.replay.machine(identity)?;
            let call = machine.calls.get(&position)?;
            let callee = self.replay.machine(if stored_effects {
                call.stored_effects_callee()?
            } else {
                call.callee()?
            })?;
            let exported = self.export_source_origin(callee, origin)?;
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
                .as_ref()?
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
                        projection: projection.clone().without_runtime_coordinates(),
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
            let callee = self.replay.machine(call.stored_effects_callee()?)?;
            let (parameter, _, ty) = self.replay.parameter(callee, *source)?;
            let relative =
                descriptor_location(self.replay.types, *source, ty, &projection.path, None)?;
            let actual = self.replay.actual_slot(call, parameter, &relative.path)?;
            let before = self
                .replay
                .snapshots
                .get(&identity)?
                .as_ref()?
                .get(&position)?;
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
            let mut actual = self.replay.canonical_storage(machine, before, &actual)?;
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
            self.replay.summary(identity, 0)?.result
        }

        pub fn stored_effects(
            &mut self,
            identity: MachineId,
        ) -> Option<Vec<DescriptorStoredEffect>> {
            let summary = self.replay.summary(identity, 0)?;
            (!summary.invalidates_contents).then_some(summary.effects)
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
            if call.argument_contents.iter().any(Option::is_none) {
                return None;
            }
            let summary = self.replay.cached_invocation_summary(call)?;
            if summary.invalidates_contents {
                return None;
            }
            let mut replacements = BTreeMap::new();
            for effect in &summary.effects {
                let destination =
                    self.replay
                        .actual_contents(call, effect.parameter, &effect.steps)?;
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
                        DescriptorPayloadTarget::Storage(location) => {
                            DescriptorContentsQuery::from_location(&location)
                        }
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
        pub fn flow(
            &mut self,
            identity: MachineId,
        ) -> Option<BTreeMap<DescriptorPosition, DescriptorFlowStep>> {
            self.prepare_flow(identity, None)
        }

        pub fn residence_flow(
            &mut self,
            identity: MachineId,
            rules: &mut crate::descriptors::descriptor_lifetimes::DescriptorResidenceRules,
        ) -> Option<BTreeMap<DescriptorPosition, DescriptorFlowStep>> {
            rules.captures = rules
                .captures
                .iter()
                .map(|(position, source, destination)| {
                    Some((
                        *position,
                        self.location(identity, *position, source)?,
                        self.location(identity, *position, destination)?,
                    ))
                })
                .collect::<Option<_>>()?;
            self.prepare_flow(identity, Some(rules))
        }

        fn prepare_flow(
            &mut self,
            identity: MachineId,
            mut residence_rules: Option<
                &mut crate::descriptors::descriptor_lifetimes::DescriptorResidenceRules,
            >,
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
                    .slots
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
                let mut observations = step
                    .inputs
                    .iter()
                    .map(canonical)
                    .collect::<Option<Vec<_>>>()?;
                for input in &step.inputs {
                    if input.path.is_empty()
                        && let Some(Some(origins)) = before.slots.get(input)
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
                    .map(|(source, destination)| {
                        Some((canonical(source)?, canonical(destination)?))
                    })
                    .collect::<Option<_>>()?;
                step.authority_derivations = step
                    .authority_derivations
                    .iter()
                    .map(|(source, destination)| {
                        Some((canonical(source)?, canonical(destination)?))
                    })
                    .collect::<Option<_>>()?;
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
                let Some(call) = machine
                    .calls
                    .get(position)
                    .filter(|call| call.argument_contents.iter().all(Option::is_some))
                else {
                    continue;
                };
                let summary = self.replay.cached_invocation_summary(call)?;
                if summary
                    .effects
                    .iter()
                    .flat_map(|effect| &effect.origins)
                    .chain(
                        summary
                            .result
                            .iter()
                            .flatten()
                            .flat_map(|slot| &slot.origins),
                    )
                    .any(|origin| matches!(origin, DescriptorStoredOrigin::Provider(_)))
                {
                    return None;
                }
                step.may_escape |= summary.invalidates_contents;
                let mut outputs = Vec::new();
                for effect in &summary.effects {
                    let query =
                        self.replay
                            .actual_contents(call, effect.parameter, &effect.steps)?;
                    let targets = self.replay.contents_targets(machine, before, &query)?;
                    let strong = targets.len() == 1
                        && matches!(targets.first(), Some(DescriptorPayloadTarget::Storage(_)));
                    for target in targets {
                        match target {
                            DescriptorPayloadTarget::Storage(destination) => {
                                outputs.push((destination, effect.origins.as_slice(), strong));
                            }
                            DescriptorPayloadTarget::Incoming(query) => {
                                step.inputs.push(query.observation());
                                step.may_escape = true;
                                for origin in &effect.origins {
                                    match origin {
                                        DescriptorStoredOrigin::IncomingSlot { position, path } => {
                                            step.inputs.push(
                                                self.replay.actual_slot(call, *position, path)?,
                                            )
                                        }
                                        DescriptorStoredOrigin::IncomingContents {
                                            position,
                                            steps,
                                            ..
                                        } => step.inputs.push(
                                            self.replay
                                                .actual_contents(call, *position, steps)?
                                                .observation(),
                                        ),
                                        DescriptorStoredOrigin::Parameter { position, .. } => step
                                            .inputs
                                            .push(call.arguments.get(*position)?.as_ref()?.clone()),
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                }
                if let Some(result) = &call.result {
                    for slot in summary.result.as_ref()? {
                        let mut destination = result.clone();
                        destination.path.extend_from_slice(&slot.path);
                        outputs.push((destination, slot.origins.as_slice(), true));
                    }
                }
                for (destination, origins, strong) in outputs {
                    let destination =
                        self.replay
                            .canonical_storage(machine, before, &destination)?;
                    if strong && !step.definitions.contains(&destination) {
                        step.definitions.push(destination.clone());
                    }
                    for location in before.slots.keys() {
                        if strong
                            && let Some(canonical) =
                                self.replay.canonical_storage(machine, before, location)
                            && destination.contains(&canonical)
                            && !step.definitions.contains(location)
                        {
                            step.definitions.push(location.clone());
                        }
                    }
                    for origin in origins {
                        let source = match origin {
                            DescriptorStoredOrigin::Invocation { .. }
                            | DescriptorStoredOrigin::Literal { .. }
                            | DescriptorStoredOrigin::Empty
                            | DescriptorStoredOrigin::Provider(_)
                            | DescriptorStoredOrigin::AccessFormation { .. } => {
                                continue;
                            }
                            DescriptorStoredOrigin::IncomingSlot { position, path } => {
                                let Some(source) = call.arguments.get(*position)?.as_ref() else {
                                    let mut query =
                                        call.argument_contents.get(*position)?.as_ref()?.clone();
                                    query.steps.extend(
                                        path.iter().cloned().map(DescriptorContentsStep::Address),
                                    );
                                    step.inputs.push(query.observation());
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
                                let mut query =
                                    call.argument_contents.get(*position)?.as_ref()?.clone();
                                query.steps.extend_from_slice(steps);
                                step.inputs.push(query.observation());
                                step.may_escape = true;
                                continue;
                            }
                            DescriptorStoredOrigin::Parameter {
                                position,
                                projection,
                            } => {
                                let Some(source) = call.arguments.get(*position)?.as_ref() else {
                                    let query = call.argument_contents.get(*position)?.as_ref()?;
                                    step.inputs.push(query.observation());
                                    step.may_escape = true;
                                    continue;
                                };
                                let mut source = source.clone();
                                if !projection.path.is_empty() {
                                    let root_type = *machine.place_types.get(&source.root)?;
                                    let mut spelling = descriptor_path(
                                        self.replay.types,
                                        root_type,
                                        &source.path,
                                    )?;
                                    spelling.extend_from_slice(&projection.path);
                                    source = descriptor_location(
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
                        if matches!(origin, DescriptorStoredOrigin::Parameter { .. })
                            && let Some(rules) = residence_rules.as_deref_mut()
                        {
                            let source_type = descriptor_location_type(
                                self.replay.types,
                                *machine.place_types.get(&pair.0.root)?,
                                &pair.0.path,
                            )?;
                            if !DescriptorSlotKind::Views.is_slot(self.replay.types, source_type) {
                                rules
                                    .captures
                                    .insert((*position, pair.0.clone(), pair.1.clone()));
                            }
                        }
                        step.derivations.push(pair.clone());
                        if strong
                            && match origin {
                                DescriptorStoredOrigin::Parameter { projection, .. } => {
                                    projection.exact
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
            }
            Some(flow)
        }
    }
}
pub(crate) mod case_flow {

    use super::engine::{Replay, State};
    use super::*;
    use semantic_vocabulary::StructuralCaseId;
    use std::sync::Arc;

    pub(crate) mod values {
        use super::*;
        use crate::descriptors::descriptor_lifetimes::{DescriptorFlowEdge, DescriptorPathSegment};
        use terminal_psi::StructuralFieldType;

        #[derive(Clone, Default, PartialEq, Eq)]
        pub(crate) struct Copies {
            classes: Vec<BTreeSet<DescriptorLocation>>,
            member_classes: BTreeMap<DescriptorLocation, usize>,
        }

        impl Copies {
            fn normalized(mut classes: Vec<BTreeSet<DescriptorLocation>>) -> Self {
                classes.retain(|members| members.len() > 1);
                classes.sort();
                let mut member_classes = BTreeMap::new();
                for (index, members) in classes.iter().enumerate() {
                    for member in members {
                        member_classes.insert(member.clone(), index);
                    }
                }
                Self {
                    classes,
                    member_classes,
                }
            }

            pub(crate) fn equivalents(
                &self,
                subject: &DescriptorLocation,
                limit: usize,
            ) -> Option<BTreeSet<DescriptorLocation>> {
                let mut found = BTreeSet::from([subject.clone()]);
                let mut pending = vec![subject.clone()];
                let mut applied = BTreeSet::new();
                while let Some(location) = pending.pop() {
                    for length in 0..=location.path.len() {
                        let prefix = DescriptorLocation {
                            root: location.root,
                            path: location.path[..length].to_vec(),
                        };
                        let Some(index) = self.member_classes.get(&prefix) else {
                            continue;
                        };
                        let suffix = &location.path[length..];
                        if suffix.contains(&DescriptorPathSegment::Referent)
                            || !applied.insert((*index, suffix.to_vec()))
                        {
                            continue;
                        }
                        if applied.len() > limit {
                            return None;
                        }
                        for member in &self.classes[*index] {
                            let mut target = member.clone();
                            target.path.extend_from_slice(suffix);
                            if found.insert(target.clone()) {
                                if found.len() > limit {
                                    return None;
                                }
                                pending.push(target);
                            }
                        }
                    }
                }
                Some(found)
            }

            pub(crate) fn insert_class(
                &mut self,
                mut members: BTreeSet<DescriptorLocation>,
                limit: usize,
            ) -> Option<()> {
                if members.len() < 2 {
                    return Some(());
                }
                let affected = members
                    .iter()
                    .filter_map(|member| self.member_classes.get(member).copied())
                    .collect::<BTreeSet<_>>();
                let mut classes = Vec::with_capacity(self.classes.len() + 1);
                for (index, existing) in std::mem::take(&mut self.classes).into_iter().enumerate() {
                    if affected.contains(&index) {
                        members.extend(existing);
                    } else {
                        classes.push(existing);
                    }
                }
                classes.push(members);
                *self = Self::normalized(classes);
                (self.member_classes.len() <= limit).then_some(())
            }

            pub(crate) fn retain(&mut self, keep: impl Fn(&DescriptorLocation) -> bool) {
                let classes = std::mem::take(&mut self.classes)
                    .into_iter()
                    .map(|mut members| {
                        members.retain(&keep);
                        members
                    })
                    .collect();
                *self = Self::normalized(classes);
            }

            pub(crate) fn meet(&self, other: &Self) -> Self {
                let mut groups = BTreeMap::<(usize, usize), BTreeSet<DescriptorLocation>>::new();
                for (left, members) in self.classes.iter().enumerate() {
                    for member in members {
                        if let Some(right) = other.member_classes.get(member) {
                            groups
                                .entry((left, *right))
                                .or_default()
                                .insert(member.clone());
                        }
                    }
                }
                Self::normalized(groups.into_values().collect())
            }
        }

        #[derive(Clone, Copy)]
        pub(crate) enum Transfer<'edge> {
            Operation(DescriptorPosition),
            Arrival(&'edge DescriptorFlowEdge),
        }

        impl Transfer<'_> {
            pub(crate) fn position(self) -> Option<DescriptorPosition> {
                match self {
                    Self::Operation(position) => Some(position),
                    Self::Arrival(_) => None,
                }
            }

            pub(crate) fn transports_value(
                self,
                machine: &DescriptorMachine,
                source: &DescriptorLocation,
                destination: &DescriptorLocation,
            ) -> bool {
                match self {
                    Self::Operation(position) => {
                        let key = (position, source.clone(), destination.clone());
                        !machine.calls.contains_key(&position)
                            && !machine.storage_derivations.contains(&key)
                            && !machine.projections.contains_key(&key)
                            && !machine.contents_queries.contains_key(&key)
                    }
                    Self::Arrival(edge) => edge
                        .bindings
                        .iter()
                        .any(|binding| binding.locations() == Some((source, destination))),
                }
            }

            pub(crate) fn copies_value(
                self,
                machine: &DescriptorMachine,
                source: &DescriptorLocation,
                destination: &DescriptorLocation,
            ) -> bool {
                self.transports_value(machine, source, destination)
                    && match self {
                        Self::Operation(_) => true,
                        Self::Arrival(edge) => edge.bindings.iter().any(|binding| {
                            binding.access == StructuralAccess::Owned
                                && binding.locations() == Some((source, destination))
                        }),
                    }
            }
        }

        pub(crate) struct Storage {
            roots: BTreeSet<PlaceId>,
            case_types: BTreeSet<StructuralTypeId>,
            pub(crate) limit: usize,
        }

        impl Storage {
            pub(crate) fn new(replay: &Replay<'_>, machine: &DescriptorMachine) -> Self {
                let mut roots = BTreeSet::new();
                let mut ordinary_types = BTreeMap::new();
                let mut case_types = BTreeMap::new();
                for row in &machine.places {
                    let Some(structural_type) = machine.place_types.get(&row.place) else {
                        continue;
                    };
                    if !*ordinary_types.entry(*structural_type).or_insert_with(|| {
                        ordinary_type(replay, *structural_type, &mut BTreeSet::new())
                    }) {
                        continue;
                    }
                    contains_case(
                        replay.types,
                        *structural_type,
                        &mut case_types,
                        &mut BTreeSet::new(),
                    );
                    let owned = match row.definition {
                        DescriptorDefinition::Parameter { access, .. }
                        | DescriptorDefinition::Arrival { access } => {
                            access == StructuralAccess::Owned
                        }
                        DescriptorDefinition::Local => {
                            machine
                                .payload_producers
                                .values()
                                .any(|root| *root == row.place)
                                || owned_arrival(machine, row.place)
                        }
                        DescriptorDefinition::Literal => true,
                    };
                    if owned {
                        roots.insert(row.place);
                    }
                }
                Self {
                    roots,
                    case_types: case_types
                        .into_iter()
                        .filter_map(|(identity, tagged)| tagged.then_some(identity))
                        .collect(),
                    limit: machine.flow.len().saturating_mul(4096).max(4096),
                }
            }

            pub(crate) fn contains(&self, location: &DescriptorLocation) -> bool {
                self.roots.contains(&location.root)
                    && !location.path.contains(&DescriptorPathSegment::Referent)
            }

            pub(crate) fn copy_type(
                &self,
                replay: &Replay<'_>,
                machine: &DescriptorMachine,
                source: &DescriptorLocation,
                destination: &DescriptorLocation,
            ) -> bool {
                if !self.contains(source) || !self.contains(destination) {
                    return false;
                }
                let Some(source_type) = location_type(replay, machine, source) else {
                    return false;
                };
                self.case_types.contains(&source_type)
                    && location_type(replay, machine, destination) == Some(source_type)
                    && find_structural_type(replay.types, source_type)
                        .is_some_and(|row| row.copyable)
            }
        }

        fn location_type(
            replay: &Replay<'_>,
            machine: &DescriptorMachine,
            location: &DescriptorLocation,
        ) -> Option<StructuralTypeId> {
            crate::descriptors::descriptor_lifetimes::descriptor_location_type(
                replay.types,
                *machine.place_types.get(&location.root)?,
                &location.path,
            )
        }

        pub(crate) fn transported_subject(
            replay: &Replay<'_>,
            machine: &DescriptorMachine,
            subject: &DescriptorLocation,
            source: &DescriptorLocation,
            destination: &DescriptorLocation,
        ) -> Option<DescriptorLocation> {
            let target = subject.transported(source, destination)?;
            if subject.path[source.path.len()..].contains(&DescriptorPathSegment::Referent) {
                return None;
            }
            let source_type = location_type(replay, machine, subject)?;
            (location_type(replay, machine, &target) == Some(source_type)).then_some(target)
        }

        fn owned_arrival(machine: &DescriptorMachine, root: PlaceId) -> bool {
            let parameter = DescriptorLocation::whole(root);
            let mut found = false;
            for edge in machine.flow.values().flat_map(|step| &step.successors) {
                if !edge.parameters.contains(&parameter) {
                    continue;
                }
                found = true;
                let mut bindings = edge
                    .bindings
                    .iter()
                    .filter(|binding| binding.parameter == parameter);
                let Some(binding) = bindings.next() else {
                    return false;
                };
                if bindings.next().is_some() || binding.access != StructuralAccess::Owned {
                    return false;
                }
            }
            found
        }

        pub(crate) fn contains_case(
            types: &[StructuralTypeDeclaration],
            structural_type: StructuralTypeId,
            complete: &mut BTreeMap<StructuralTypeId, bool>,
            active: &mut BTreeSet<StructuralTypeId>,
        ) -> bool {
            if let Some(result) = complete.get(&structural_type) {
                return *result;
            }
            if !active.insert(structural_type) {
                return false;
            }
            let Some(declaration) = find_structural_type(types, structural_type) else {
                return false;
            };
            let mut child =
                |field: &terminal_psi::StructuralFieldDeclaration| match field.field_type {
                    StructuralFieldType::Structural(child) => {
                        contains_case(types, child, complete, active)
                    }
                    _ => false,
                };
            let result = match &declaration.shape {
                StructuralTypeShape::Record { fields, .. } => fields
                    .iter()
                    .fold(false, |found, field| child(field) | found),
                StructuralTypeShape::Mixed { fields, cases } => fields
                    .iter()
                    .chain(cases.iter().flat_map(|case| &case.fields))
                    .fold(!cases.is_empty(), |found, field| child(field) | found),
                StructuralTypeShape::Sum { cases } => cases
                    .iter()
                    .flat_map(|case| &case.fields)
                    .fold(!cases.is_empty(), |found, field| child(field) | found),
                StructuralTypeShape::FixedArray { element, .. } => {
                    contains_case(types, *element, complete, active)
                }
                _ => false,
            };
            active.remove(&structural_type);
            complete.insert(structural_type, result);
            result
        }

        fn ordinary_type(
            replay: &Replay<'_>,
            structural_type: StructuralTypeId,
            active: &mut BTreeSet<StructuralTypeId>,
        ) -> bool {
            if !plain_owned_shape(replay.types.iter(), structural_type)
                || !active.insert(structural_type)
            {
                return false;
            }
            let Some(declaration) = find_structural_type(replay.types, structural_type) else {
                return false;
            };
            let mut child =
                |field: &terminal_psi::StructuralFieldDeclaration| match field.field_type {
                    StructuralFieldType::Structural(child) => ordinary_type(replay, child, active),
                    _ => true,
                };
            let ordinary = match &declaration.shape {
                StructuralTypeShape::Record {
                    storage, fields, ..
                } => {
                    !matches!(
                        storage,
                        terminal_psi::StructuralRecordStorage::AccessView(_)
                            | terminal_psi::StructuralRecordStorage::Accessor(_)
                    ) && fields.iter().all(&mut child)
                }
                StructuralTypeShape::Mixed { fields, cases } => fields
                    .iter()
                    .chain(cases.iter().flat_map(|case| &case.fields))
                    .all(&mut child),
                StructuralTypeShape::Sum { cases } => {
                    cases.iter().flat_map(|case| &case.fields).all(&mut child)
                }
                StructuralTypeShape::FixedArray { element, .. } => {
                    ordinary_type(replay, *element, active)
                }
                _ => true,
            };
            active.remove(&structural_type);
            ordinary
        }

        pub(crate) fn overlaps(left: &DescriptorLocation, right: &DescriptorLocation) -> bool {
            if left.root != right.root {
                return false;
            }
            for (left, right) in left.path.iter().zip(&right.path) {
                if left == right {
                    continue;
                }
                return !matches!(
                    (left, right),
                    (
                        DescriptorPathSegment::Field(_),
                        DescriptorPathSegment::Field(_)
                    ) | (
                        DescriptorPathSegment::FixedIndex(_),
                        DescriptorPathSegment::FixedIndex(_)
                    )
                );
            }
            true
        }
    }
    pub(crate) use values::{Storage, Transfer, contains_case};
    pub(crate) mod result {

        use super::*;

        #[cfg(test)]
        mod tests {
            use super::*;
            use crate::descriptors::descriptor_lifetimes::{
                DescriptorFlowEdge, DescriptorFlowStep,
            };
            use semantic_vocabulary::{BlockId, OperationId};
            use terminal_psi::{StructuralCaseDeclaration, TerminalIdentityIndex};

            fn position(index: u32) -> DescriptorPosition {
                (BlockId::new(1).unwrap(), index)
            }
            fn place(index: u64) -> DescriptorLocation {
                DescriptorLocation::whole(PlaceId::new(index).unwrap())
            }
            fn machine(id: u64, root: u64, successors: &[Vec<u32>]) -> DescriptorMachine {
                DescriptorMachine {
                    id: MachineId::new(id).unwrap(),
                    returns: None,
                    places: vec![DescriptorPlace {
                        place: place(root).root,
                        definition: DescriptorDefinition::Local,
                    }],
                    place_types: BTreeMap::from([(
                        place(root).root,
                        StructuralTypeId::new(1).unwrap(),
                    )]),
                    entry: position(0),
                    flow: successors
                        .iter()
                        .enumerate()
                        .map(|(index, targets)| {
                            (
                                position(index as u32),
                                DescriptorFlowStep {
                                    successors: targets
                                        .iter()
                                        .map(|target| DescriptorFlowEdge {
                                            target: position(*target),
                                            parameters: vec![],
                                            bindings: vec![],
                                        })
                                        .collect(),
                                    ..Default::default()
                                },
                            )
                        })
                        .collect(),
                    case_constructors: BTreeMap::new(),
                    case_successors: BTreeMap::new(),
                    originations: BTreeMap::new(),
                    projections: BTreeMap::new(),
                    contents_queries: BTreeMap::new(),
                    contents_transfers: BTreeMap::new(),
                    payload_producers: BTreeMap::new(),
                    payload_entry_predicates: None,
                    payload_scalar_definitions: BTreeMap::new(),
                    payload_scalar_postconditions: BTreeMap::new(),
                    payload_scalar_parameters: BTreeMap::new(),
                    scalar_payload_reads: BTreeMap::new(),
                    payload_edge_predicates: BTreeMap::new(),
                    payload_scalar_bindings: BTreeMap::new(),
                    scalar_payload_writes: BTreeMap::new(),
                    payload_invalidations: BTreeMap::new(),
                    unresolved_contents_writes: BTreeSet::new(),
                    storage_derivations: BTreeSet::new(),
                    calls: BTreeMap::new(),
                    normal_exits: vec![position((successors.len() - 1) as u32)],
                }
            }

            #[test]
            fn completed_return_tags_union_all_exits_and_unknown_or_public_returns_grant_nothing() {
                let types = vec![StructuralTypeDeclaration {
                    requires: Vec::new(),
                    id: StructuralTypeId::new(1).unwrap(),
                    identity: TerminalIdentityIndex(1),
                    quotient_carrier: None,
                    owns_linear_claim: false,
                    no_code_affine_cleanup: true,
                    copyable: true,
                    descriptor_cleanup: false,
                    scalar_restriction: None,
                    shape: StructuralTypeShape::Sum {
                        cases: (1..=3)
                            .map(|id| StructuralCaseDeclaration {
                                id: StructuralCaseId::new(id).unwrap(),
                                identity: TerminalIdentityIndex(id as u32),
                                fields: vec![],
                            })
                            .collect(),
                    },
                }];
                let mut caller = machine(1, 1, &[vec![1], vec![]]);
                caller
                    .flow
                    .get_mut(&position(0))
                    .unwrap()
                    .definitions
                    .push(place(1));
                caller.calls.insert(
                    position(0),
                    DescriptorInvocation {
                        operation: OperationId::new(1).unwrap(),
                        target: DescriptorInvocationTarget::Machine(MachineId::new(2).unwrap()),
                        arguments: vec![],
                        argument_contents: vec![],
                        result: Some(place(1)),
                    },
                );
                let mut callee = machine(2, 2, &[vec![1, 3], vec![2], vec![], vec![4], vec![]]);
                callee.returns = Some(vec![(position(2), place(2)), (position(4), place(2))]);
                callee.normal_exits = vec![position(2), position(4)];
                for (point, case) in [(1, 1), (3, 2)] {
                    callee.case_constructors.insert(
                        position(point),
                        vec![DescriptorCasePredicate {
                            subject: place(2),
                            case: StructuralCaseId::new(case).unwrap(),
                        }],
                    );
                }
                let mut graph = vec![caller, callee];
                let predicate = |case| DescriptorCasePredicate {
                    subject: place(1),
                    case: StructuralCaseId::new(case).unwrap(),
                };
                {
                    let mut analysis = DescriptorAnalysis::with_reference_slots(&types, &graph);
                    assert_eq!(
                        analysis.case_feasibility(graph[0].id, position(1), &predicate(1)),
                        FlowFeasibility::Possible
                    );
                    assert_eq!(
                        analysis.case_feasibility(graph[0].id, position(1), &predicate(2)),
                        FlowFeasibility::Possible
                    );
                    assert_eq!(
                        analysis.case_feasibility(graph[0].id, position(1), &predicate(3)),
                        FlowFeasibility::ProvedImpossible
                    );
                    assert!(!analysis.case_is_established(graph[0].id, position(1), &predicate(1)));
                }
                graph[1].case_constructors.remove(&position(3));
                {
                    let mut analysis = DescriptorAnalysis::with_reference_slots(&types, &graph);
                    assert_eq!(
                        analysis.case_feasibility(graph[0].id, position(1), &predicate(3)),
                        FlowFeasibility::Unknown
                    );
                }
                graph[1].case_constructors.insert(
                    position(3),
                    vec![DescriptorCasePredicate {
                        subject: place(2),
                        case: StructuralCaseId::new(1).unwrap(),
                    }],
                );
                graph[0].calls.get_mut(&position(0)).unwrap().target =
                    DescriptorInvocationTarget::Public {
                        effects_machine: Some(graph[1].id),
                        realizations: vec![],
                        extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
                        results: vec![],
                        invalidates_contents: false,
                    };
                let mut analysis = DescriptorAnalysis::with_reference_slots(&types, &graph);
                assert_eq!(
                    analysis.case_feasibility(graph[0].id, position(1), &predicate(3)),
                    FlowFeasibility::Unknown
                );
            }
        }
        use crate::descriptors::descriptor_lifetimes::DescriptorPathSegment;

        #[derive(Clone, Default)]
        pub(crate) struct ResultCases {
            facts: BTreeMap<Vec<DescriptorPathSegment>, BTreeSet<StructuralCaseId>>,
        }

        impl ResultCases {
            pub(crate) fn capture(
                replay: &Replay<'_>,
                machine: &DescriptorMachine,
                state: &State,
                returned: &DescriptorLocation,
            ) -> Self {
                let Some(storage) = replay.canonical_storage(machine, state, returned) else {
                    return Self::default();
                };
                Self {
                    facts: state
                        .cases
                        .facts
                        .iter()
                        .filter(|(subject, _)| {
                            subject.root == storage.root
                                && subject.path.starts_with(&storage.path)
                                && !subject.path[storage.path.len()..]
                                    .contains(&DescriptorPathSegment::Referent)
                        })
                        .map(|(subject, cases)| {
                            (subject.path[storage.path.len()..].to_vec(), cases.clone())
                        })
                        .collect(),
                }
            }

            pub(crate) fn join(&mut self, other: &Self) {
                self.facts.retain(|path, cases| {
                    let Some(other) = other.facts.get(path) else {
                        return false;
                    };
                    cases.extend(other);
                    true
                });
            }

            pub(crate) fn install(
                &self,
                replay: &Replay<'_>,
                machine: &DescriptorMachine,
                state: &mut State,
                result: &DescriptorLocation,
            ) -> Option<()> {
                if self.facts.is_empty() {
                    return Some(());
                }
                let storage = replay.canonical_storage(machine, state, result)?;
                for (path, active) in &self.facts {
                    let mut subject = result.clone();
                    subject.path.extend_from_slice(path);
                    for case in active {
                        valid_subject(
                            replay,
                            machine,
                            &DescriptorCasePredicate {
                                subject: subject.clone(),
                                case: *case,
                            },
                        )?;
                    }
                    let mut destination = storage.clone();
                    destination.path.extend_from_slice(path);
                    Arc::make_mut(&mut state.cases)
                        .facts
                        .insert(destination, active.clone());
                }
                Some(())
            }
        }
    }
    pub(crate) use result::ResultCases;

    #[derive(Clone, Default, PartialEq, Eq)]
    pub(crate) struct CurrentCases {
        facts: BTreeMap<DescriptorLocation, BTreeSet<StructuralCaseId>>,
        copies: values::Copies,
    }

    pub(crate) fn validate_graph(replay: &Replay<'_>, machine: &DescriptorMachine) -> Option<()> {
        let mut visited = BTreeSet::new();
        let mut pending = vec![machine.entry];
        while let Some(position) = pending.pop() {
            if !visited.insert(position) {
                continue;
            }
            let step = machine.flow.get(&position)?;
            pending.extend(step.successors.iter().map(|edge| edge.target));
        }
        if visited.len() != machine.flow.len() {
            return None;
        }
        for (position, predicates) in &machine.case_constructors {
            machine.flow.get(position)?;
            for predicate in predicates {
                valid_subject(replay, machine, predicate)?;
            }
        }
        for (position, predicates) in &machine.case_successors {
            if predicates.len() != machine.flow.get(position)?.successors.len() {
                return None;
            }
            for (predicate, _) in predicates {
                valid_subject(replay, machine, predicate)?;
            }
        }
        Some(())
    }

    fn valid_subject(
        replay: &Replay<'_>,
        machine: &DescriptorMachine,
        predicate: &DescriptorCasePredicate,
    ) -> Option<()> {
        let ty = crate::descriptors::descriptor_lifetimes::descriptor_location_type(
            replay.types,
            *machine.place_types.get(&predicate.subject.root)?,
            &predicate.subject.path,
        )?;
        let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
            &find_structural_type(replay.types, ty)?.shape
        else {
            return None;
        };
        terminal_psi::find_structural_case(cases, predicate.case).map(|_| ())
    }

    pub(crate) fn establish(
        replay: &Replay<'_>,
        machine: &DescriptorMachine,
        state: &mut State,
        predicate: &DescriptorCasePredicate,
    ) -> Option<()> {
        valid_subject(replay, machine, predicate)?;
        let storage = replay.canonical_storage(machine, state, &predicate.subject)?;
        let subjects = state
            .cases
            .copies
            .equivalents(&storage, machine.flow.len().saturating_mul(4096).max(4096))?;
        let cases = Arc::make_mut(&mut state.cases);
        for subject in subjects {
            cases
                .facts
                .insert(subject, BTreeSet::from([predicate.case]));
        }
        Some(())
    }

    pub(crate) fn select(
        replay: &Replay<'_>,
        machine: &DescriptorMachine,
        state: &mut State,
        predicate: &DescriptorCasePredicate,
        truth: bool,
    ) -> Option<bool> {
        valid_subject(replay, machine, predicate)?;
        let Some(storage) = replay.canonical_storage(machine, state, &predicate.subject) else {
            return Some(true);
        };
        let subjects = state
            .cases
            .copies
            .equivalents(&storage, machine.flow.len().saturating_mul(4096).max(4096))?;
        let ty = crate::descriptors::descriptor_lifetimes::descriptor_location_type(
            replay.types,
            *machine.place_types.get(&predicate.subject.root)?,
            &predicate.subject.path,
        )?;
        let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
            &find_structural_type(replay.types, ty)?.shape
        else {
            return None;
        };
        let mut possible = cases
            .iter()
            .map(|case| case.id)
            .filter(|case| (*case == predicate.case) == truth)
            .collect::<BTreeSet<_>>();
        for subject in &subjects {
            if let Some(known) = state.cases.facts.get(subject) {
                possible.retain(|case| known.contains(case));
            }
        }
        if possible.is_empty() {
            return Some(false);
        }
        let cases = Arc::make_mut(&mut state.cases);
        for subject in subjects {
            cases.facts.insert(subject, possible.clone());
        }
        Some(true)
    }

    pub(crate) fn membership(
        replay: &Replay<'_>,
        machine: &DescriptorMachine,
        state: &State,
        predicate: &DescriptorCasePredicate,
    ) -> FlowFeasibility {
        if valid_subject(replay, machine, predicate).is_none() {
            return FlowFeasibility::Unknown;
        }
        let Some(storage) = replay.canonical_storage(machine, state, &predicate.subject) else {
            return FlowFeasibility::Unknown;
        };
        match state.cases.facts.get(&storage) {
            Some(cases) if !cases.contains(&predicate.case) => FlowFeasibility::ProvedImpossible,
            Some(_) => FlowFeasibility::Possible,
            None => FlowFeasibility::Unknown,
        }
    }

    pub(crate) fn established(
        replay: &Replay<'_>,
        machine: &DescriptorMachine,
        state: &State,
        predicate: &DescriptorCasePredicate,
    ) -> bool {
        if valid_subject(replay, machine, predicate).is_none() {
            return false;
        }
        let Some(storage) = replay.canonical_storage(machine, state, &predicate.subject) else {
            return false;
        };
        state
            .cases
            .facts
            .get(&storage)
            .is_some_and(|cases| cases.len() == 1 && cases.contains(&predicate.case))
    }

    fn written_locations(
        replay: &Replay<'_>,
        machine: &DescriptorMachine,
        before: &State,
        definitions: &[DescriptorLocation],
        position: Option<DescriptorPosition>,
    ) -> Option<Vec<DescriptorLocation>> {
        let mut writes = definitions.to_vec();
        let Some(position) = position else {
            return Some(writes);
        };
        let mut queries = Vec::new();
        for query in machine
            .payload_invalidations
            .get(&position)
            .into_iter()
            .flatten()
        {
            queries.push(query.as_ref()?);
        }
        for transfer in machine
            .contents_transfers
            .get(&position)
            .into_iter()
            .flatten()
        {
            queries.push(&transfer.destination);
            if transfer.removes_source {
                queries.push(&transfer.source);
            }
        }
        for query in queries {
            let targets = replay.contents_targets(machine, before, query)?;
            if targets.is_empty() {
                return None;
            }
            for target in targets {
                let DescriptorPayloadTarget::Storage(location) = target else {
                    return None;
                };
                writes.push(location);
            }
        }
        Some(writes)
    }

    fn survives(
        storage: &Storage,
        writes: Option<&[DescriptorLocation]>,
        subject: &DescriptorLocation,
    ) -> bool {
        let Some(writes) = writes else {
            return false;
        };
        writes.is_empty()
            || (storage.contains(subject)
                && writes
                    .iter()
                    .all(|write| storage.contains(write) && !values::overlaps(write, subject)))
    }

    pub(crate) fn transport<'locations>(
        replay: &Replay<'_>,
        machine: &DescriptorMachine,
        before: &State,
        definitions: &[DescriptorLocation],
        derivations: impl Iterator<
            Item = (
                &'locations DescriptorLocation,
                &'locations DescriptorLocation,
            ),
        >,
        transfer: Transfer<'_>,
        storage: &Storage,
    ) -> Option<Arc<CurrentCases>> {
        let position = transfer.position();
        let writes = written_locations(replay, machine, before, definitions, position);
        let mut derivations = derivations.peekable();
        if derivations.peek().is_none() && writes.as_ref().is_some_and(Vec::is_empty) {
            return Some(before.cases.clone());
        }
        let keep = |subject: &DescriptorLocation| survives(storage, writes.as_deref(), subject);
        let mut cases = (*before.cases).clone();
        cases.facts.retain(|subject, _| keep(subject));
        cases.copies.retain(keep);
        let derivations = derivations.collect::<Vec<_>>();
        let mut assigned = BTreeMap::<DescriptorLocation, BTreeSet<StructuralCaseId>>::new();
        let mut copies = Vec::new();
        for (source, destination) in &derivations {
            if !transfer.transports_value(machine, source, destination) {
                continue;
            }
            let Some(actual_source) = replay.canonical_storage(machine, before, source) else {
                continue;
            };
            let actual_destination = match position {
                Some(_) => replay.canonical_storage(machine, before, destination),
                None => Some((*destination).clone()),
            };
            let Some(actual_destination) = actual_destination else {
                continue;
            };
            for (subject, active) in &before.cases.facts {
                if let Some(target) = values::transported_subject(
                    replay,
                    machine,
                    subject,
                    &actual_source,
                    &actual_destination,
                ) {
                    assigned.entry(target).or_default().extend(active);
                }
            }
            let distinct_assignment =
                derivations.iter().all(|(other_source, other_destination)| {
                    (*other_source == *source && *other_destination == *destination)
                        || !values::overlaps(other_destination, destination)
                });
            let source_home = DescriptorLocation::whole(actual_source.root);
            let established_source = before.slots.get(&source_home).is_some_and(|value| {
                value.as_ref().is_some_and(|origins| {
                    !origins.is_empty()
                        && origins.iter().all(|origin| {
                            matches!(origin,
                        super::engine::Origin::Storage(home) if home == &source_home)
                        })
                })
            });
            if distinct_assignment
                && established_source
                && transfer.copies_value(machine, source, destination)
                && storage.copy_type(replay, machine, &actual_source, &actual_destination)
            {
                copies.push((
                    before
                        .cases
                        .copies
                        .equivalents(&actual_source, storage.limit)?,
                    actual_destination,
                ));
            }
        }
        cases.facts.extend(assigned);
        let mut captured = BTreeMap::<
            DescriptorLocation,
            (BTreeSet<DescriptorLocation>, BTreeSet<DescriptorLocation>),
        >::new();
        for (sources, destination) in copies {
            let representative = sources.first()?.clone();
            let (old_members, receivers) = captured.entry(representative).or_default();
            old_members.extend(sources);
            receivers.insert(destination);
        }
        for (_, (old_members, receivers)) in captured {
            let members = old_members
                .into_iter()
                .filter(|source| keep(source))
                .chain(receivers)
                .collect();
            cases.copies.insert_class(members, storage.limit)?;
        }
        Some(if cases == *before.cases {
            before.cases.clone()
        } else {
            Arc::new(cases)
        })
    }

    pub(crate) fn retire_query(
        replay: &Replay<'_>,
        machine: &DescriptorMachine,
        before: &State,
        after: &mut State,
        query: &DescriptorContentsQuery,
        storage: &Storage,
    ) {
        let writes = replay
            .contents_targets(machine, before, query)
            .and_then(|targets| {
                if targets.is_empty() {
                    return None;
                }
                targets
                    .into_iter()
                    .map(|target| match target {
                        DescriptorPayloadTarget::Storage(location) => Some(location),
                        DescriptorPayloadTarget::Incoming(_) => None,
                    })
                    .collect::<Option<Vec<_>>>()
            });
        let cases = Arc::make_mut(&mut after.cases);
        cases
            .facts
            .retain(|subject, _| survives(storage, writes.as_deref(), subject));
        cases
            .copies
            .retain(|subject| survives(storage, writes.as_deref(), subject));
    }

    pub(crate) fn join(left: &CurrentCases, right: &CurrentCases) -> CurrentCases {
        CurrentCases {
            facts: left
                .facts
                .iter()
                .filter_map(|(subject, cases)| {
                    let mut cases = cases.clone();
                    cases.extend(right.facts.get(subject)?);
                    Some((subject.clone(), cases))
                })
                .collect(),
            copies: left.copies.meet(&right.copies),
        }
    }
}
pub(crate) mod contents_queries {
    use super::engine::{DescriptorReadDepth, Origin, State, StorageResolution, Value};
    use super::*;
    use crate::descriptors::descriptor_lifetimes::descriptor_location;
    use crate::descriptors::descriptor_lifetimes::descriptor_location_type;

    impl Replay<'_> {
        pub(crate) fn read_contents(
            &self,
            machine: &DescriptorMachine,
            state: &State,
            query: &DescriptorContentsQuery,
        ) -> Value {
            self.read_contents_with_depth(machine, state, query, &DescriptorReadDepth::default())
        }

        pub(crate) fn read_contents_with_depth(
            &self,
            machine: &DescriptorMachine,
            state: &State,
            query: &DescriptorContentsQuery,
            depth: &DescriptorReadDepth,
        ) -> Value {
            let _read = depth.enter()?;
            query.validate(self.types, *machine.place_types.get(&query.root)?)?;
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
                && let Some(carrier) = self.reference_referent_carrier(machine, &location)?
            {
                return self.read_with_depth(machine, state, &carrier, depth);
            }
            if self.slot_kind.tracks_references()
                && let Some(queried) = query.exact_location()
            {
                match self.resolve_storage(machine, state, &queried) {
                    Some(StorageResolution::Unique(location)) => {
                        if let Some(value) = state.slots.get(&location) {
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

        pub(crate) fn contents_targets(
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
            if self.contents_address_hidden(machine, state, query) {
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
        fn inline_steps(
            &self,
            machine: &DescriptorMachine,
            query: &DescriptorContentsQuery,
        ) -> Option<Vec<bool>> {
            let mut base = *machine.place_types.get(&query.root)?;
            let mut path = Vec::new();
            let mut inline = Vec::with_capacity(query.steps.len());
            for step in &query.steps {
                let container = || {
                    let container = descriptor_location_type(self.types, base, &path)?;
                    Some(&find_structural_type(self.types, container)?.shape)
                };
                inline.push(match step {
                    DescriptorContentsStep::Address(DescriptorPathSegment::Referent) => false,
                    DescriptorContentsStep::Address(DescriptorPathSegment::Case(_)) => true,
                    DescriptorContentsStep::Address(DescriptorPathSegment::Field(_)) => {
                        matches!(path.last(), Some(DescriptorPathSegment::Case(_)))
                            || container().is_some_and(|shape| match shape {
                                StructuralTypeShape::Record { storage, .. } => !matches!(
                                    storage,
                                    terminal_psi::StructuralRecordStorage::AccessView(_)
                                        | terminal_psi::StructuralRecordStorage::Accessor(_)
                                ),
                                StructuralTypeShape::Mixed { .. } => true,
                                _ => false,
                            })
                    }
                    DescriptorContentsStep::Address(DescriptorPathSegment::FixedIndex(_))
                    | DescriptorContentsStep::AnyElement { .. } => {
                        container().is_some_and(|shape| {
                            matches!(shape, StructuralTypeShape::FixedArray { .. })
                        })
                    }
                });
                match step {
                    DescriptorContentsStep::Address(segment) => path.push(segment.clone()),
                    DescriptorContentsStep::AnyElement { element } => {
                        base = *element;
                        path.clear();
                    }
                }
            }
            Some(inline)
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
            let Some(DescriptorContentsStep::AnyElement { element }) = steps.get(prefix_length)
            else {
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
                            self.contents_targets_at(machine, state, &child, suffix, depth)?,
                        );
                    }
                    Some(origins)
                }
                StructuralTypeShape::ElementView {
                    element: actual, ..
                } if actual == *element => {
                    let mut origins = BTreeSet::new();
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
                Origin::Storage(location) => {
                    return self.contents_targets_at(machine, state, &location, &steps, depth);
                }
                Origin::Incoming(location) => {
                    Some(DescriptorContentsQuery::from_location(&location))
                }
                Origin::IncomingContents(query, projection)
                    if projection == Projection::new(&[]) =>
                {
                    Some(query)
                }
                Origin::IncomingContents(..) => return None,
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
                        );
                    }
                }
                _ => return None,
            };
            let mut incoming = incoming?;
            incoming
                .steps
                .push(DescriptorContentsStep::AnyElement { element });
            self.incoming_targets_at(machine, state, incoming, element, suffix, depth)
        }

        pub(crate) fn transfer_contents(
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

        pub(crate) fn contents_replacements(
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

        pub(crate) fn install_contents(
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

        pub(crate) fn actual_contents(
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
}
pub(crate) mod dispatch {
    use super::engine::Summary;
    use super::*;

    pub(crate) fn join(mut joined: Summary, summaries: &[Summary]) -> Summary {
        joined.invalidates_contents |= summaries.iter().any(|summary| summary.invalidates_contents);
        for summary in summaries {
            for effect in &summary.effects {
                if !joined
                    .effects
                    .iter()
                    .any(|prior| prior.parameter == effect.parameter && prior.steps == effect.steps)
                {
                    joined.effects.push(DescriptorStoredEffect {
                        parameter: effect.parameter,
                        steps: effect.steps.clone(),
                        origins: Vec::new(),
                    });
                }
            }
        }
        for effect in &mut joined.effects {
            for summary in summaries {
                if let Some(source) = summary.effects.iter().find(|source| {
                    source.parameter == effect.parameter && source.steps == effect.steps
                }) {
                    for origin in &source.origins {
                        effect.origins.push(origin.clone());
                    }
                } else {
                    let incoming = DescriptorStoredOrigin::IncomingContents {
                        position: effect.parameter,
                        steps: effect.steps.clone(),
                        projection: Projection::new(&[]),
                    };
                    effect.origins.push(incoming);
                }
            }
        }
        joined
    }
}
pub(crate) mod engine {
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
        Storage(DescriptorLocation),
    }
    pub(crate) type Value = Option<BTreeSet<Origin>>;
    pub(crate) type Slots = BTreeMap<DescriptorLocation, Value>;
    #[derive(Clone, Default, PartialEq, Eq)]
    pub(crate) struct State {
        pub(crate) cases: Arc<super::case_flow::CurrentCases>,
        pub(crate) slots: Arc<Slots>,
        pub(crate) contents: Arc<BTreeMap<DescriptorContentsQuery, Value>>,
        pub(crate) contents_invalidated: bool,
    }
    pub(crate) type States = BTreeMap<DescriptorPosition, State>;

    pub(crate) enum StorageResolution {
        Unique(DescriptorLocation),
        Alternatives,
    }

    #[derive(Clone)]
    pub(crate) struct Summary {
        pub(crate) result: Option<Vec<DescriptorResultSlot>>,
        pub(crate) result_cases: super::case_flow::ResultCases,
        pub(crate) effects: Vec<DescriptorStoredEffect>,
        pub(crate) invalidates_contents: bool,
    }

    fn direct_target_summary(target: &DescriptorInvocationTarget) -> Option<Summary> {
        match target {
            DescriptorInvocationTarget::Machine(_) => None,
            DescriptorInvocationTarget::Provider {
                results,
                effects,
                invalidates_contents,
            } => Some(Summary {
                result: Some(results.clone()),
                result_cases: Default::default(),
                effects: effects.clone(),
                invalidates_contents: *invalidates_contents,
            }),
            DescriptorInvocationTarget::Public {
                results,
                invalidates_contents,
                ..
            } => Some(Summary {
                result: Some(results.clone()),
                result_cases: Default::default(),
                effects: Vec::new(),
                invalidates_contents: *invalidates_contents,
            }),
        }
    }

    pub(crate) struct Replay<'a> {
        pub(crate) types: &'a [StructuralTypeDeclaration],
        pub(crate) slot_kind: DescriptorSlotKind,
        pub(crate) machines: &'a [DescriptorMachine],
        pub(crate) summaries: BTreeMap<MachineId, Option<Summary>>,
        pub(crate) snapshots: BTreeMap<MachineId, Option<Arc<States>>>,
        pub(crate) arrival_snapshots: BTreeMap<MachineId, Option<States>>,
    }
    #[derive(Default)]
    pub(crate) struct DescriptorReadDepth {
        depth: std::cell::Cell<usize>,
    }

    pub(crate) struct DescriptorReadGuard<'depth> {
        depth: &'depth std::cell::Cell<usize>,
    }

    impl DescriptorReadDepth {
        pub(crate) fn enter(&self) -> Option<DescriptorReadGuard<'_>> {
            let depth = self.depth.get();
            if depth >= 128 {
                return None;
            }
            self.depth.set(depth + 1);
            Some(DescriptorReadGuard { depth: &self.depth })
        }
    }

    impl Drop for DescriptorReadGuard<'_> {
        fn drop(&mut self) {
            self.depth.set(self.depth.get() - 1);
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
                arrival_snapshots: BTreeMap::new(),
            }
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
        pub(crate) fn canonical_storage(
            &self,
            machine: &DescriptorMachine,
            state: &State,
            location: &DescriptorLocation,
        ) -> Option<DescriptorLocation> {
            match self.resolve_storage(machine, state, location)? {
                StorageResolution::Unique(location) => Some(location),
                StorageResolution::Alternatives => None,
            }
        }

        pub(crate) fn reference_backing_matches(
            &self,
            machine: &DescriptorMachine,
            backing: &DescriptorLocation,
            reference: StructuralTypeId,
        ) -> Option<bool> {
            let root = *machine.place_types.get(&backing.root)?;
            let path = descriptor_path(self.types, root, &backing.path)?;
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
        ) -> Option<StorageResolution> {
            let mut current = location.clone();
            let mut expansions = BTreeMap::new();
            let mut expand = |equation, remaining| {
                if expansions
                    .insert(equation, remaining)
                    .is_some_and(|previous| remaining >= previous)
                {
                    None
                } else {
                    Some(())
                }
            };
            loop {
                let root_type = *machine.place_types.get(&current.root)?;
                let definition =
                    &exactly_one(machine.places.iter(), |row| row.place == current.root)?
                        .definition;
                let borrowed = match definition {
                    DescriptorDefinition::Parameter { access, .. }
                    | DescriptorDefinition::Arrival { access } => {
                        *access != StructuralAccess::Owned
                    }
                    DescriptorDefinition::Local | DescriptorDefinition::Literal => false,
                };
                if current.path.is_empty() && !borrowed {
                    return Some(StorageResolution::Unique(current));
                }
                let aliases_backing = borrowed
                    || matches!(
                        find_structural_type(self.types, root_type)?.shape,
                        StructuralTypeShape::Reference { .. }
                    )
                    || is_descriptor(self.types.iter(), root_type);
                if aliases_backing
                    && let Some(Some(origins)) =
                        state.slots.get(&DescriptorLocation::whole(current.root))
                    && !origins.is_empty()
                    && origins
                        .iter()
                        .all(|origin| matches!(origin, Origin::Storage(_)))
                {
                    let mut locations = self.storage_origins(machine, state, &current)?.into_iter();
                    let first = locations.next()?;
                    if !locations.all(|candidate| candidate == first) {
                        return Some(StorageResolution::Alternatives);
                    }
                    if first != current {
                        expand(DescriptorLocation::whole(current.root), current.path.len())?;
                        current = first;
                        continue;
                    }
                }
                let mut next = None;
                for (position, segment) in current.path.iter().enumerate() {
                    if *segment != DescriptorPathSegment::Referent {
                        continue;
                    }
                    let prefix = DescriptorLocation {
                        root: current.root,
                        path: current.path[..position].to_vec(),
                    };
                    let Some(Some(origins)) = state.slots.get(&prefix) else {
                        continue;
                    };
                    if origins.is_empty()
                        || origins
                            .iter()
                            .any(|origin| !matches!(origin, Origin::Storage(_)))
                    {
                        continue;
                    }
                    let reference = descriptor_location_type(
                        self.types,
                        *machine.place_types.get(&prefix.root)?,
                        &prefix.path,
                    )?;
                    if !matches!(
                        find_structural_type(self.types, reference)?.shape,
                        StructuralTypeShape::Reference { .. }
                    ) {
                        return None;
                    }
                    if origins.len() == 1 && origins.contains(&Origin::Storage(prefix.clone())) {
                        continue;
                    }
                    let mut alternatives = false;
                    for origin in origins {
                        let Origin::Storage(backing) = origin else {
                            return None;
                        };
                        let backing_type = *machine.place_types.get(&backing.root)?;
                        if !self.reference_backing_matches(machine, backing, reference)? {
                            return None;
                        }
                        let mut target = backing.clone();
                        target.path.extend_from_slice(&current.path[position + 1..]);
                        descriptor_path(self.types, backing_type, &target.path)?;
                        if next.as_ref().is_some_and(|previous| previous != &target) {
                            alternatives = true;
                        }
                        next = Some(target);
                    }
                    if alternatives {
                        return Some(StorageResolution::Alternatives);
                    }
                    expand(prefix, current.path.len() - position - 1)?;
                    break;
                }
                match next {
                    Some(next) if next != current => current = next,
                    Some(_) => return None,
                    None => return Some(StorageResolution::Unique(current)),
                }
            }
        }
        pub(crate) fn storage_origins(
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
                let mut suffix = location.path.as_slice();
                let root_type = *machine.place_types.get(&location.root)?;
                let backing_type = *machine.place_types.get(&backing.root)?;
                if matches!(
                    find_structural_type(self.types, root_type)?.shape,
                    StructuralTypeShape::Reference { .. }
                ) && self.reference_backing_matches(machine, backing, root_type)?
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
        pub(crate) fn reference_referent_carrier(
            &self,
            machine: &DescriptorMachine,
            location: &DescriptorLocation,
        ) -> Option<Option<DescriptorLocation>> {
            if !self.slot_kind.tracks_references() {
                return Some(None);
            }
            let Some((DescriptorPathSegment::Referent, path)) = location.path.split_last() else {
                return Some(None);
            };
            let root_type = *machine.place_types.get(&location.root)?;
            let endpoint = descriptor_location_type(self.types, root_type, &location.path)?;
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
            self.read_with_depth(machine, state, location, &DescriptorReadDepth::default())
        }

        pub(crate) fn read_with_depth(
            &self,
            machine: &DescriptorMachine,
            state: &State,
            location: &DescriptorLocation,
            depth: &DescriptorReadDepth,
        ) -> Value {
            let _read = depth.enter()?;
            let location = match self.resolve_storage(machine, state, location)? {
                StorageResolution::Unique(location) => location,
                StorageResolution::Alternatives => location.clone(),
            };
            let root = DescriptorLocation::whole(location.root);
            if let Some(carrier) = self.reference_referent_carrier(machine, &location)? {
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
                return value.clone();
            }
            if self.slot_kind.tracks_references()
                && location.path.contains(&DescriptorPathSegment::Referent)
                && descriptor_location_type(
                    self.types,
                    *machine.place_types.get(&location.root)?,
                    &location.path,
                )
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
                    Origin::Storage(backing) if backing.root != location.root => {
                        let suffix = match location.path.first() {
                            Some(DescriptorPathSegment::Referent) => &location.path[1..],
                            _ => location.path.as_slice(),
                        };
                        let mut target = backing.clone();
                        target.path.extend_from_slice(suffix);
                        descriptor_path(
                            self.types,
                            *machine.place_types.get(&target.root)?,
                            &target.path,
                        )?;
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
                    DescriptorSlotInventory {
                        slots: self.slot_kind.owned_slots(
                            self.types,
                            row.place,
                            *machine.place_types.get(&row.place)?,
                        )?,
                        opaque_referents: Vec::new(),
                    }
                } else {
                    self.slot_kind.inventory(
                        self.types,
                        row.place,
                        *machine.place_types.get(&row.place)?,
                    )?
                };
                for slot in inventory.slots.iter().chain(&inventory.opaque_referents) {
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
                    Origin::Storage(location) => {
                        match self.storage_projection(machine, &location)? {
                            DescriptorCallOrigin::Argument {
                                source,
                                projection: prefix,
                            } => Some(Origin::Backing(DescriptorCallOrigin::Argument {
                                source,
                                projection: prefix.then(projection),
                            })),
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
            let position = transfer.position();
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
                    let value = if let Some(carrier) =
                        self.reference_referent_carrier(machine, source)?
                    {
                        self.read(machine, before, &carrier)
                    } else {
                        match self.resolve_storage(machine, before, source)? {
                            StorageResolution::Unique(storage) => {
                                Some(BTreeSet::from([Origin::Storage(storage)]))
                            }
                            StorageResolution::Alternatives => self.read(machine, before, source),
                        }
                    };
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
                    Some(query) => self.read_contents(machine, before, query),
                    None => self.read(machine, before, source),
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
                            self.project(machine, self.read(machine, before, location), projection),
                        );
                    }
                }
            }
            let mut after = before.clone();
            after.cases = super::case_flow::transport(
                self,
                machine,
                before,
                &definitions,
                derivations,
                transfer,
                case_storage,
            )?;
            if !definitions.is_empty() {
                for (location, value) in before.slots.iter() {
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
        fn transfer_edge(
            &self,
            machine: &DescriptorMachine,
            before: &State,
            edge: &descriptor_lifetimes::DescriptorFlowEdge,
            case_storage: &super::case_flow::Storage,
        ) -> Option<State> {
            let mut after = self.transfer(
                machine,
                before,
                &edge.parameters,
                edge.derivations(),
                super::case_flow::Transfer::Arrival(edge),
                case_storage,
            )?;
            for binding in &edge.bindings {
                let query = match &binding.source {
                    DescriptorFlowSource::Location(location) => {
                        Some(DescriptorContentsQuery::from_location(location))
                    }
                    DescriptorFlowSource::Contents(query) => Some(query.clone()),
                    DescriptorFlowSource::Unavailable(_) => None,
                };
                if let Some(query) = query {
                    let structural_type = *machine.place_types.get(&binding.parameter.root)?;
                    let mut captured = Slots::from([(
                        binding.parameter.clone(),
                        self.read_contents(machine, before, &query),
                    )]);
                    for slot in self.slot_kind.owned_slots(
                        self.types,
                        binding.parameter.root,
                        structural_type,
                    )? {
                        if slot == binding.parameter {
                            continue;
                        }
                        let mut source = query.clone();
                        source.steps.extend(
                            slot.path
                                .iter()
                                .cloned()
                                .map(DescriptorContentsStep::Address),
                        );
                        captured.insert(slot, self.read_contents(machine, before, &source));
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
                if is_descriptor(self.types.iter(), structural_type)
                    || matches!(
                        shape,
                        StructuralTypeShape::Reference { .. }
                            | StructuralTypeShape::Record {
                                storage: terminal_psi::StructuralRecordStorage::AccessView(_)
                                    | terminal_psi::StructuralRecordStorage::Accessor(_),
                                ..
                            }
                    )
                {
                    continue;
                }
                let value = Some(BTreeSet::from([Origin::Storage(parameter.clone())]));
                if after.slots.get(parameter) != Some(&value) {
                    after.slots_mut().insert(parameter.clone(), value);
                }
            }
            Some(after)
        }
        pub(crate) fn states(&mut self, identity: MachineId, depth: usize) -> Option<Arc<States>> {
            if let Some(cached) = self.snapshots.get(&identity) {
                return cached.clone();
            }
            self.snapshots.insert(identity, None);
            self.arrival_snapshots.insert(identity, None);
            if depth >= 128 {
                return None;
            }
            let machine = self.machine(identity)?;
            super::case_flow::validate_graph(self, machine)?;
            let case_storage = super::case_flow::Storage::new(self, machine);
            if !machine.flow.contains_key(&machine.entry) {
                return None;
            }
            let mut states = States::from([(machine.entry, self.initial(machine)?)]);
            let mut arrivals = States::new();
            let mut pending = vec![machine.entry];
            let mut visits = 0usize;
            while let Some(position) = pending.pop() {
                visits += 1;
                if visits > machine.flow.len().saturating_mul(4096).max(4096) {
                    return None;
                }
                let before = states.get(&position)?.clone();
                let step = machine.flow.get(&position)?;
                let mut after = self.transfer(
                    machine,
                    &before,
                    &step.definitions,
                    step.derivations
                        .iter()
                        .map(|(source, destination)| (source, destination)),
                    super::case_flow::Transfer::Operation(position),
                    &case_storage,
                )?;
                after.contents_invalidated |=
                    machine.unresolved_contents_writes.contains(&position);
                if step.may_escape
                    || machine.unresolved_contents_writes.contains(&position)
                    || machine
                        .calls
                        .get(&position)
                        .is_some_and(|call| !call.argument_contents.iter().all(Option::is_some))
                {
                    after.cases = Arc::default();
                }
                if let Some(constructors) = machine.case_constructors.get(&position) {
                    for constructor in constructors {
                        super::case_flow::establish(self, machine, &mut after, constructor)?;
                    }
                }
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
                            .slots_mut()
                            .insert(location.clone(), Some(BTreeSet::from([origin])));
                    }
                }
                if let Some(call) = machine.calls.get(&position)
                    && call.argument_contents.iter().all(Option::is_some)
                {
                    let summary = self.invocation_summary(call, depth + 1)?;
                    after.contents_invalidated |= summary.invalidates_contents;
                    if summary.invalidates_contents {
                        after.cases = Arc::default();
                    }
                    let mut replacements = BTreeMap::new();
                    let mut destinations = Vec::new();
                    for effect in &summary.effects {
                        if !effect.origins.is_empty()
                            && effect
                                .origins
                                .iter()
                                .all(|origin| incoming_field_kept(effect, origin))
                            && effect.origins.iter().any(|origin| {
                                self.substitute(machine, &before, call, origin).is_none()
                            })
                        {
                            continue;
                        }
                        let destination =
                            self.actual_contents(call, effect.parameter, &effect.steps)?;
                        super::case_flow::retire_query(
                            self,
                            machine,
                            &before,
                            &mut after,
                            &destination,
                            &case_storage,
                        );
                        let targets = self.contents_targets(machine, &before, &destination)?;
                        let strong = targets.len() == 1
                            && matches!(targets.first(), Some(DescriptorPayloadTarget::Storage(_)));
                        for target in targets {
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
                        let mut dropped_retired_incoming = false;
                        for origin in &effect.origins {
                            match self.substitute(machine, &before, call, origin) {
                                Some(substituted) => origins.extend(substituted),
                                None if incoming_field_kept(effect, origin) => {
                                    dropped_retired_incoming = true;
                                }
                                None => return None,
                            }
                        }
                        if origins.is_empty() && dropped_retired_incoming {
                            continue;
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
                            let mut known = true;
                            for origin in &slot.origins {
                                match self.substitute_result(machine, &before, call, origin) {
                                    Some(substituted) => origins.extend(substituted),
                                    None => {
                                        known = false;
                                        break;
                                    }
                                }
                            }
                            replacements.insert(
                                DescriptorPayloadTarget::Storage(destination),
                                known.then_some(origins),
                            );
                        }
                    }
                    Self::install_contents(&mut after, replacements);
                    if let Some(result) = &call.result {
                        summary
                            .result_cases
                            .install(self, machine, &mut after, result)?;
                    }
                }
                if step.successors.iter().any(|edge| {
                    edge.bindings
                        .iter()
                        .any(|binding| matches!(binding.source, DescriptorFlowSource::Contents(_)))
                }) {
                    arrivals.insert(position, after.clone());
                }
                if machine
                    .case_successors
                    .get(&position)
                    .is_some_and(|cases| cases.len() != step.successors.len())
                {
                    return None;
                }
                for (ordinal, edge) in step.successors.iter().enumerate() {
                    if !machine.flow.contains_key(&edge.target) {
                        return None;
                    }
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
                    let carried = self.transfer_edge(machine, &selected, edge, &case_storage)?;
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
            self.snapshots.insert(identity, Some(states.clone()));
            self.arrival_snapshots.insert(identity, Some(arrivals));
            Some(states)
        }
        pub(crate) fn prepare_states(&mut self, identity: MachineId) -> Option<()> {
            if !self.snapshots.contains_key(&identity) {
                self.states(identity, 0)?;
            }
            self.snapshots.get(&identity)?.as_ref().map(|_| ())
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
            let value = self.substitute(machine, state, call, origin);
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
            if let DescriptorStoredOrigin::Provider(provider) = origin {
                return self.substitute_provider(machine, state, call, provider);
            }
            self.substitute_actuals(machine, state, call.actual_inputs(), origin)
        }

        pub(crate) fn substitute_actuals(
            &self,
            machine: &DescriptorMachine,
            state: &State,
            actuals: DescriptorActualInputs<'_>,
            origin: &DescriptorStoredOrigin,
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
                DescriptorStoredOrigin::Provider(_) => None,
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
                    let address = actuals
                        .arguments
                        .get(*position)
                        .and_then(Option::as_ref)
                        .and_then(|argument| self.read(machine, state, argument));
                    let value = match address {
                        Some(origins) if !origins.is_empty() => Some(origins),
                        _ => self.read_contents(
                            machine,
                            state,
                            actuals.argument_contents.get(*position)?.as_ref()?,
                        ),
                    };
                    self.project(machine, value, Some(projection))
                }
                DescriptorStoredOrigin::IncomingSlot { position, path } => {
                    let mut query = actuals.argument_contents.get(*position)?.as_ref()?.clone();
                    query
                        .steps
                        .extend(path.iter().cloned().map(DescriptorContentsStep::Address));
                    self.read_contents(machine, state, &query)
                }
                DescriptorStoredOrigin::IncomingContents {
                    position,
                    steps,
                    projection,
                } => {
                    let mut query = actuals.argument_contents.get(*position)?.as_ref()?.clone();
                    query.steps.extend_from_slice(steps);
                    self.project(
                        machine,
                        self.read_contents(machine, state, &query),
                        Some(projection),
                    )
                }
            }
        }
        pub(crate) fn exported(
            &self,
            machine: &DescriptorMachine,
            origin: &Origin,
        ) -> Option<DescriptorStoredOrigin> {
            match origin {
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
            let mut result_cases: Option<super::case_flow::ResultCases> = None;
            let result = if let Some(returns) = &machine.returns {
                let mut slots: Option<Vec<DescriptorResultSlot>> = None;
                let mut result_type = None;
                for (position, location) in returns {
                    if !location.path.is_empty() {
                        return None;
                    }
                    let Some(state) = states.get(position) else {
                        continue;
                    };
                    let structural_type = *machine.place_types.get(&location.root)?;
                    let cases =
                        super::case_flow::ResultCases::capture(self, machine, state, location);
                    if let Some(accumulated) = &mut result_cases {
                        accumulated.join(&cases);
                    } else {
                        result_cases = Some(cases);
                    }
                    if result_type.is_some_and(|previous| previous != structural_type) {
                        return None;
                    }
                    result_type = Some(structural_type);
                    let returned =
                        self.slot_kind
                            .owned_slots(self.types, location.root, structural_type)?;
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
                        let origins = self.read(machine, state, &location)?;
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
                result_cases: result_cases.unwrap_or_default(),
                effects,
                invalidates_contents,
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
                before.incoming_contents(&query),
                incoming.incoming_contents(&query),
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
        let mut slot_updates = Vec::new();
        {
            let mut left = before.slots.iter().peekable();
            let mut right = incoming.slots.iter().peekable();
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
            let slots = existing.slots_mut();
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
                                    DescriptorContentsStep::AnyElement { .. }
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

        pub(crate) fn incoming_contents(&self, query: &DescriptorContentsQuery) -> Value {
            let mut origins = BTreeSet::from([Origin::IncomingContents(
                query.clone(),
                Projection::new(&[]),
            )]);
            for (written, value) in self.contents.iter() {
                if written.root != query.root {
                    continue;
                }
                let overlaps = written.steps.iter().zip(&query.steps).all(|(left, right)| {
                    left == right
                        || matches!(
                            (left, right),
                            (
                                DescriptorContentsStep::AnyElement { .. },
                                DescriptorContentsStep::Address(DescriptorPathSegment::FixedIndex(
                                    _
                                ))
                            ) | (
                                DescriptorContentsStep::Address(DescriptorPathSegment::FixedIndex(
                                    _
                                )),
                                DescriptorContentsStep::AnyElement { .. }
                            )
                        )
                });
                if !overlaps {
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

    fn incoming_field_kept(
        effect: &DescriptorStoredEffect,
        origin: &DescriptorStoredOrigin,
    ) -> bool {
        let DescriptorStoredOrigin::IncomingSlot { position, path } = origin else {
            return false;
        };
        *position == effect.parameter
            && path.len() == effect.steps.len()
            && path.iter().zip(&effect.steps).all(|(segment, step)| {
                matches!(step, DescriptorContentsStep::Address(actual) if actual == segment)
            })
    }

    #[cfg(test)]
    mod snapshot_tests {
        use super::*;

        fn location(number: u64) -> DescriptorLocation {
            DescriptorLocation::whole(PlaceId::new(number).unwrap())
        }

        #[test]
        fn completed_position_maps_share_ownership_preserve_snapshots_and_failure_fences() {
            use crate::descriptors::descriptor_lifetimes::{
                DescriptorFlowEdge, DescriptorFlowStep,
            };
            use semantic_vocabulary::{BlockId, IntegerSign, IntegerType, ScalarType};
            use terminal_psi::TerminalIdentityIndex;

            let id = MachineId::new(1).unwrap();
            let block = BlockId::new(1).unwrap();
            let positions = [(block, 0), (block, 1), (block, 2)];
            let parameter = location(1);
            let replacement_parameter = location(2);
            let scalar = StructuralTypeId::new(1).unwrap();
            let carrier = StructuralTypeId::new(2).unwrap();
            let collection = StructuralTypeId::new(3).unwrap();
            let element = StructuralTypeId::new(4).unwrap();
            let referent = StructuralTypeDeclaration {
                requires: Vec::new(),
                quotient_carrier: None,
                id: scalar,
                identity: TerminalIdentityIndex(1),
                owns_linear_claim: false,
                no_code_affine_cleanup: true,
                copyable: true,
                descriptor_cleanup: false,
                shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                )),
                scalar_restriction: None,
            };
            let reference = StructuralTypeDeclaration {
                id: carrier,
                identity: TerminalIdentityIndex(2),
                shape: StructuralTypeShape::Reference {
                    referent: collection,
                    access: StructuralAccess::SharedBorrow,
                },
                ..referent.clone()
            };
            let pointer = StructuralTypeDeclaration {
                id: element,
                identity: TerminalIdentityIndex(4),
                shape: StructuralTypeShape::Reference {
                    referent: scalar,
                    access: StructuralAccess::SharedBorrow,
                },
                ..reference.clone()
            };
            let array = StructuralTypeDeclaration {
                id: collection,
                identity: TerminalIdentityIndex(3),
                shape: StructuralTypeShape::FixedArray {
                    element,
                    length: terminal_psi::ArrayExtent::Literal(1),
                },
                ..reference.clone()
            };
            let rows = StructuralTypeId::new(5).unwrap();
            let holder = location(3);
            let nested = StructuralTypeDeclaration {
                id: rows,
                identity: TerminalIdentityIndex(5),
                shape: StructuralTypeShape::FixedArray {
                    element: collection,
                    length: terminal_psi::ArrayExtent::Literal(1),
                },
                ..reference.clone()
            };
            let types = vec![referent, reference, array, pointer, nested];
            let mut flow = BTreeMap::new();
            for (index, position) in positions.iter().copied().enumerate() {
                let mut step = DescriptorFlowStep::default();
                if let Some(&target) = positions.get(index + 1) {
                    step.successors.push(DescriptorFlowEdge {
                        target,
                        parameters: Vec::new(),
                        bindings: Vec::new(),
                    });
                }
                if index == 0 {
                    step.definitions.push(parameter.clone());
                }
                flow.insert(position, step);
            }
            let mut graph = vec![DescriptorMachine {
                id,
                returns: None,
                places: vec![
                    DescriptorPlace {
                        place: parameter.root,
                        definition: DescriptorDefinition::Parameter {
                            position: 0,
                            access: StructuralAccess::Owned,
                            structural_type: carrier,
                        },
                    },
                    DescriptorPlace {
                        place: replacement_parameter.root,
                        definition: DescriptorDefinition::Parameter {
                            position: 1,
                            access: StructuralAccess::Owned,
                            structural_type: carrier,
                        },
                    },
                    DescriptorPlace {
                        place: holder.root,
                        definition: DescriptorDefinition::Parameter {
                            position: 2,
                            access: StructuralAccess::Owned,
                            structural_type: rows,
                        },
                    },
                ],
                place_types: BTreeMap::from([
                    (parameter.root, carrier),
                    (replacement_parameter.root, carrier),
                    (holder.root, rows),
                ]),
                entry: positions[0],
                flow,
                case_constructors: BTreeMap::new(),
                case_successors: BTreeMap::new(),
                originations: BTreeMap::new(),
                projections: BTreeMap::new(),
                contents_queries: BTreeMap::new(),
                contents_transfers: BTreeMap::new(),
                payload_producers: BTreeMap::new(),
                payload_entry_predicates: None,
                payload_scalar_definitions: BTreeMap::new(),
                payload_scalar_postconditions: BTreeMap::new(),
                payload_scalar_parameters: BTreeMap::new(),
                scalar_payload_reads: BTreeMap::new(),
                payload_edge_predicates: BTreeMap::new(),
                payload_scalar_bindings: BTreeMap::new(),
                scalar_payload_writes: BTreeMap::new(),
                payload_invalidations: BTreeMap::new(),
                unresolved_contents_writes: BTreeSet::new(),
                storage_derivations: BTreeSet::new(),
                calls: BTreeMap::new(),
                normal_exits: vec![positions[2]],
            }];
            let mut replay = Replay::with_slot_kind(&types, &graph, DescriptorSlotKind::References);
            let completed = replay.states(id, 0).unwrap();
            assert_eq!(completed.keys().copied().collect::<Vec<_>>(), positions);
            let original = Some(BTreeSet::from([Origin::Incoming(parameter.clone())]));
            assert_eq!(
                completed[&positions[0]].slots.get(&parameter),
                Some(&original)
            );
            assert_eq!(completed[&positions[1]].slots.get(&parameter), Some(&None));
            assert!(Arc::ptr_eq(
                &completed,
                replay.snapshots[&id].as_ref().unwrap()
            ));
            assert!(Arc::ptr_eq(&completed, &replay.states(id, 0).unwrap()));
            assert!(Arc::ptr_eq(&completed, &replay.states(id, 128).unwrap()));
            replay.prepare_states(id).unwrap();
            let mut edited = completed[&positions[0]].clone();
            edited.slots_mut().insert(parameter.clone(), None);
            assert_eq!(
                completed[&positions[0]].slots.get(&parameter),
                Some(&original)
            );
            assert_eq!(edited.slots.get(&parameter), Some(&None));

            let mut ancestor = parameter.clone();
            ancestor.path.push(DescriptorPathSegment::Referent);
            let mut leaf = ancestor.clone();
            leaf.path.push(DescriptorPathSegment::FixedIndex(0));
            let query = DescriptorContentsQuery::from_location(&leaf);
            let mut current = completed[&positions[0]].clone();
            let observed = replay.read(&graph[0], &current, &leaf);
            assert!(observed.is_some());
            assert_eq!(replay.read_contents(&graph[0], &current, &query), observed);
            current.slots_mut().insert(ancestor, None);
            assert!(replay.read(&graph[0], &current, &leaf).is_none());
            assert!(replay.read_contents(&graph[0], &current, &query).is_none());
            let mut replacement_leaf = leaf.clone();
            replacement_leaf.root = replacement_parameter.root;
            let replacement = replay.read(&graph[0], &current, &replacement_leaf);
            assert!(replacement.is_some());
            assert_ne!(replacement, observed);
            current
                .slots_mut()
                .insert(leaf.clone(), replacement.clone());
            assert_eq!(replay.read(&graph[0], &current, &leaf), replacement);
            assert_eq!(
                replay.read_contents(&graph[0], &current, &query),
                replacement
            );

            let mut vacant = completed[&positions[0]].clone();
            vacant.slots_mut().insert(leaf.clone(), None);
            assert!(vacant.contents_path_invalidated(&query));
            assert!(!replay.contents_address_hidden(&graph[0], &vacant, &query));
            assert!(
                replay
                    .contents_targets(&graph[0], &vacant, &query)
                    .is_some_and(|targets| !targets.is_empty())
            );
            assert!(replay.read_contents(&graph[0], &vacant, &query).is_none());
            let mut ancestor_query = query.clone();
            ancestor_query.steps.pop();
            let mut below = ancestor_query.clone();
            below.steps.push(DescriptorContentsStep::Address(
                DescriptorPathSegment::FixedIndex(0),
            ));
            let mut hidden = completed[&positions[0]].clone();
            hidden.slots_mut().insert(
                DescriptorLocation {
                    root: ancestor_query.root,
                    path: ancestor_query
                        .steps
                        .iter()
                        .map(|step| match step {
                            DescriptorContentsStep::Address(segment) => segment.clone(),
                            DescriptorContentsStep::AnyElement { .. } => unreachable!(),
                        })
                        .collect(),
                },
                None,
            );
            assert!(hidden.contents_path_invalidated(&below));
            assert!(replay.contents_address_hidden(&graph[0], &hidden, &below));
            assert!(
                replay
                    .contents_targets(&graph[0], &hidden, &below)
                    .is_none()
            );

            // A moved-out aggregate keeps its parts in place: the row is
            // unknown, yet a store still finds the cell inside it, while a
            // read of that cell stays unknown.
            let mut row = holder.clone();
            row.path.push(DescriptorPathSegment::FixedIndex(0));
            let mut cell = row.clone();
            cell.path.push(DescriptorPathSegment::FixedIndex(0));
            let cell_query = DescriptorContentsQuery::from_location(&cell);
            let mut moved = completed[&positions[0]].clone();
            moved.slots_mut().insert(row, None);
            moved.slots_mut().insert(cell.clone(), None);
            assert!(moved.contents_path_invalidated(&cell_query));
            assert!(!replay.contents_address_hidden(&graph[0], &moved, &cell_query));
            assert_eq!(
                replay.contents_targets(&graph[0], &moved, &cell_query),
                Some(BTreeSet::from([DescriptorPayloadTarget::Storage(cell)]))
            );
            assert!(
                replay
                    .read_contents(&graph[0], &moved, &cell_query)
                    .is_none()
            );
            drop(replay);

            let mut limited =
                Replay::with_slot_kind(&types, &graph, DescriptorSlotKind::References);
            assert!(limited.states(id, 128).is_none());
            assert!(limited.snapshots.get(&id).is_some_and(Option::is_none));
            assert!(limited.states(id, 0).is_none());
            drop(limited);
            graph[0]
                .flow
                .insert((block, 99), DescriptorFlowStep::default());
            let mut unavailable =
                Replay::with_slot_kind(&types, &graph, DescriptorSlotKind::References);
            assert!(unavailable.states(id, 0).is_none());
            assert!(unavailable.snapshots.get(&id).is_some_and(Option::is_none));
            assert_eq!(completed.len(), 3);
            assert_eq!(
                completed[&positions[0]].slots.get(&parameter),
                Some(&original)
            );
        }

        #[test]
        fn replacement_preserves_both_maps_in_pre_write_snapshot() {
            let slot = location(1);
            let query = DescriptorContentsQuery {
                root: slot.root,
                steps: Vec::new(),
            };
            let old = Some(BTreeSet::from([Origin::Storage(location(2))]));
            let new = Some(BTreeSet::from([Origin::Storage(location(3))]));
            let mut before = State::default();
            before.slots_mut().insert(slot.clone(), old.clone());
            before.contents_mut().insert(query.clone(), old.clone());
            let mut after = before.clone();
            Replay::install_contents(
                &mut after,
                BTreeMap::from([
                    (DescriptorPayloadTarget::Storage(slot.clone()), new.clone()),
                    (
                        DescriptorPayloadTarget::Incoming(query.clone()),
                        new.clone(),
                    ),
                ]),
            );
            after.contents_invalidated = true;
            assert_eq!(before.slots.get(&slot), Some(&old));
            assert_eq!(before.contents.get(&query), Some(&old));
            assert!(!before.contents_invalidated);
            assert_eq!(after.slots.get(&slot), Some(&new));
            assert_eq!(after.contents.get(&query), Some(&new));
        }

        #[test]
        fn equal_input_states_still_materialize_incoming_contents_at_join() {
            let query = DescriptorContentsQuery {
                root: location(1).root,
                steps: Vec::new(),
            };
            let original = BTreeSet::from([Origin::Storage(location(2))]);
            let mut before = State::default();
            before
                .contents_mut()
                .insert(query.clone(), Some(original.clone()));
            let mut joined = before.clone();
            assert!(merge_state(&mut joined, &before));
            let mut expected = original.clone();
            expected.insert(Origin::IncomingContents(
                query.clone(),
                Projection::new(&[]),
            ));
            assert_eq!(joined.contents.get(&query), Some(&Some(expected)));
            assert_eq!(before.contents.get(&query), Some(&Some(original)));
            assert!(!merge_state(&mut joined, &before));
        }
        #[test]
        fn ordered_slot_join_preserves_absence_unknown_and_captured_origins() {
            let variants = [
                None,
                Some(None),
                Some(Some(BTreeSet::new())),
                Some(Some(BTreeSet::from([Origin::Storage(location(8))]))),
                Some(Some(BTreeSet::from([Origin::Storage(location(9))]))),
            ];
            for left in &variants {
                for right in &variants {
                    for (left_invalid, right_invalid) in
                        [(false, false), (false, true), (true, false), (true, true)]
                    {
                        let mut before = State::default();
                        let mut incoming = State::default();
                        let mut descendant = location(3);
                        descendant.path.push(DescriptorPathSegment::FixedIndex(2));
                        for (key, value) in [
                            (location(1), left),
                            (location(3), right),
                            (descendant.clone(), left),
                        ] {
                            if let Some(value) = value {
                                before.slots_mut().insert(key, value.clone());
                            }
                        }
                        for (key, value) in [
                            (location(2), right),
                            (location(3), left),
                            (descendant, right),
                            (location(5), left),
                        ] {
                            if let Some(value) = value {
                                incoming.slots_mut().insert(key, value.clone());
                            }
                        }
                        before.contents_invalidated = left_invalid;
                        incoming.contents_invalidated = right_invalid;
                        let original_slots = (*before.slots).clone();
                        let snapshot = before.clone();
                        let mut expected = before.clone();
                        expected.contents_invalidated |= right_invalid;
                        let keys = before
                            .slots
                            .keys()
                            .chain(incoming.slots.keys())
                            .cloned()
                            .collect::<BTreeSet<_>>();
                        for key in keys {
                            let value = match (before.slots.get(&key), incoming.slots.get(&key)) {
                                (Some(Some(left)), Some(Some(right))) => {
                                    Some(left.union(right).cloned().collect())
                                }
                                _ => None,
                            };
                            expected.slots_mut().insert(key, value);
                        }
                        let expected_changed = expected != snapshot;
                        let changed = merge_state(&mut before, &incoming);
                        assert_eq!(changed, expected_changed);
                        assert_eq!(before.slots, expected.slots);
                        assert_eq!(before.contents_invalidated, expected.contents_invalidated);
                        assert!(before.contents == expected.contents);
                        assert_eq!(*snapshot.slots, original_slots);
                    }
                }
            }
        }
        #[test]
        fn join_slot_updates_release_only_the_scratch_snapshot_before_writing() {
            let mut existing = State::default();
            for index in 1..=64 {
                existing.slots_mut().insert(
                    location(index),
                    Some(BTreeSet::from([Origin::Storage(location(100 + index))])),
                );
            }
            let mut incoming = State {
                slots: Arc::new((*existing.slots).clone()),
                ..State::default()
            };
            incoming.slots_mut().insert(
                location(64),
                Some(BTreeSet::from([Origin::Storage(location(200))])),
            );
            assert_eq!(Arc::strong_count(&existing.slots), 1);
            assert_eq!(Arc::weak_count(&existing.slots), 0);
            let unique_map = Arc::as_ptr(&existing.slots);
            assert!(merge_state(&mut existing, &incoming));
            assert_eq!(Arc::as_ptr(&existing.slots), unique_map);
            assert_eq!(
                existing.slots.get(&location(64)),
                Some(&Some(BTreeSet::from([
                    Origin::Storage(location(164)),
                    Origin::Storage(location(200)),
                ]))),
            );
            let retained = existing.clone();
            incoming.slots = Arc::new((*existing.slots).clone());
            assert!(!merge_state(&mut existing, &incoming));
            assert!(Arc::ptr_eq(&existing.slots, &retained.slots));
            let original_slots = (*retained.slots).clone();
            incoming.slots_mut().insert(
                location(64),
                Some(BTreeSet::from([Origin::Storage(location(201))])),
            );
            assert!(merge_state(&mut existing, &incoming));
            assert!(!Arc::ptr_eq(&existing.slots, &retained.slots));
            assert_eq!(*retained.slots, original_slots);
            assert_eq!(
                existing.slots.get(&location(1)),
                retained.slots.get(&location(1))
            );
            assert_eq!(
                existing
                    .slots
                    .get(&location(64))
                    .unwrap()
                    .as_ref()
                    .unwrap()
                    .len(),
                3
            );
        }

        fn scalar_type(id: u64) -> StructuralTypeDeclaration {
            use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};
            use terminal_psi::TerminalIdentityIndex;
            StructuralTypeDeclaration {
                requires: Vec::new(),
                quotient_carrier: None,
                id: StructuralTypeId::new(id).unwrap(),
                identity: TerminalIdentityIndex(id as u32),
                owns_linear_claim: false,
                no_code_affine_cleanup: true,
                copyable: true,
                descriptor_cleanup: false,
                shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                )),
                scalar_restriction: None,
            }
        }

        fn reference_type(
            id: u64,
            referent: StructuralTypeId,
            access: StructuralAccess,
        ) -> StructuralTypeDeclaration {
            let scalar = scalar_type(id);
            StructuralTypeDeclaration {
                id: StructuralTypeId::new(id).unwrap(),
                identity: terminal_psi::TerminalIdentityIndex(id as u32),
                shape: StructuralTypeShape::Reference { referent, access },
                ..scalar
            }
        }

        fn bare_machine(
            id: MachineId,
            places: Vec<DescriptorPlace>,
            place_types: BTreeMap<PlaceId, StructuralTypeId>,
            storage_derivations: BTreeSet<(
                DescriptorPosition,
                DescriptorLocation,
                DescriptorLocation,
            )>,
        ) -> DescriptorMachine {
            let entry = (semantic_vocabulary::BlockId::new(1).unwrap(), 0);
            DescriptorMachine {
                id,
                returns: None,
                places,
                place_types,
                entry,
                flow: BTreeMap::new(),
                case_constructors: BTreeMap::new(),
                case_successors: BTreeMap::new(),
                originations: BTreeMap::new(),
                projections: BTreeMap::new(),
                contents_queries: BTreeMap::new(),
                contents_transfers: BTreeMap::new(),
                payload_producers: BTreeMap::new(),
                payload_entry_predicates: None,
                payload_scalar_definitions: BTreeMap::new(),
                payload_scalar_postconditions: BTreeMap::new(),
                payload_scalar_parameters: BTreeMap::new(),
                scalar_payload_reads: BTreeMap::new(),
                payload_edge_predicates: BTreeMap::new(),
                payload_scalar_bindings: BTreeMap::new(),
                scalar_payload_writes: BTreeMap::new(),
                payload_invalidations: BTreeMap::new(),
                unresolved_contents_writes: BTreeSet::new(),
                storage_derivations,
                calls: BTreeMap::new(),
                normal_exits: vec![entry],
            }
        }

        #[test]
        fn referent_contents_keep_the_pointer_backing_origin() {
            let scalar = StructuralTypeId::new(1).unwrap();
            let carrier = StructuralTypeId::new(2).unwrap();
            let types = vec![
                scalar_type(1),
                reference_type(2, scalar, StructuralAccess::MutableBorrow),
            ];
            let parameter = location(1);
            let machine = bare_machine(
                MachineId::new(1).unwrap(),
                vec![DescriptorPlace {
                    place: parameter.root,
                    definition: DescriptorDefinition::Parameter {
                        position: 0,
                        access: StructuralAccess::MutableBorrow,
                        structural_type: carrier,
                    },
                }],
                BTreeMap::from([(parameter.root, carrier)]),
                BTreeSet::new(),
            );
            let replay = Replay::with_slot_kind(
                &types,
                std::slice::from_ref(&machine),
                DescriptorSlotKind::References,
            );
            let state = replay.initial(&machine).unwrap();
            let mut referent = parameter.clone();
            referent.path.push(DescriptorPathSegment::Referent);
            let query = DescriptorContentsQuery::from_location(&referent);
            let pointer = replay.read(&machine, &state, &parameter);
            assert_eq!(
                pointer,
                Some(BTreeSet::from([Origin::Incoming(parameter.clone())]))
            );
            assert_eq!(replay.read_contents(&machine, &state, &query), pointer);
        }

        #[test]
        fn branch_selected_storage_keeps_every_backing_origin() {
            let scalar = StructuralTypeId::new(1).unwrap();
            let types = vec![scalar_type(1)];
            let left = location(1);
            let right = location(2);
            let selected = location(3);
            let captured = location(4);
            let position = (semantic_vocabulary::BlockId::new(1).unwrap(), 0);
            let machine = bare_machine(
                MachineId::new(1).unwrap(),
                vec![
                    DescriptorPlace {
                        place: left.root,
                        definition: DescriptorDefinition::Parameter {
                            position: 0,
                            access: StructuralAccess::MutableBorrow,
                            structural_type: scalar,
                        },
                    },
                    DescriptorPlace {
                        place: right.root,
                        definition: DescriptorDefinition::Parameter {
                            position: 1,
                            access: StructuralAccess::MutableBorrow,
                            structural_type: scalar,
                        },
                    },
                    DescriptorPlace {
                        place: selected.root,
                        definition: DescriptorDefinition::Arrival {
                            access: StructuralAccess::MutableBorrow,
                        },
                    },
                    DescriptorPlace {
                        place: captured.root,
                        definition: DescriptorDefinition::Local,
                    },
                ],
                BTreeMap::from([
                    (left.root, scalar),
                    (right.root, scalar),
                    (selected.root, scalar),
                    (captured.root, scalar),
                ]),
                BTreeSet::from([(position, selected.clone(), captured.clone())]),
            );
            let replay = Replay::with_slot_kind(
                &types,
                std::slice::from_ref(&machine),
                DescriptorSlotKind::References,
            );
            let origins = BTreeSet::from([
                Origin::Storage(left.clone()),
                Origin::Storage(right.clone()),
            ]);
            let mut before = State::default();
            before
                .slots_mut()
                .insert(selected.clone(), Some(origins.clone()));
            let case_storage = super::case_flow::Storage::new(&replay, &machine);
            let after = replay
                .transfer(
                    &machine,
                    &before,
                    std::slice::from_ref(&captured),
                    [(&selected, &captured)].into_iter(),
                    super::case_flow::Transfer::Operation(position),
                    &case_storage,
                )
                .unwrap();
            assert!(matches!(
                replay.resolve_storage(&machine, &before, &selected),
                Some(StorageResolution::Alternatives)
            ));
            assert_eq!(after.slots.get(&captured), Some(&Some(origins.clone())));
            assert_eq!(replay.read(&machine, &after, &captured), Some(origins));
        }
    }
}
pub(crate) mod provider {
    use super::engine::{Origin, State, Value};
    use super::*;

    impl<Dependency> DescriptorProviderOrigin<Dependency> {
        pub(crate) fn map_dependencies<Mapped>(
            &self,
            mut map: impl FnMut(&Dependency) -> Option<Mapped>,
        ) -> Option<DescriptorProviderOrigin<Mapped>> {
            Some(DescriptorProviderOrigin {
                boundary: self.boundary,
                output: self.output,
                invocations: self.invocations.clone(),
                projection: self.projection.clone(),
                lifetime_dependencies: self
                    .lifetime_dependencies
                    .iter()
                    .map(|debt| {
                        Some(DescriptorLifetimeDebt {
                            origin: map(&debt.origin)?,
                            access: debt.access,
                        })
                    })
                    .collect::<Option<_>>()?,
            })
        }
    }

    impl Replay<'_> {
        pub(crate) fn substitute_provider(
            &self,
            machine: &DescriptorMachine,
            state: &State,
            call: &DescriptorInvocation,
            provider: &DescriptorProviderOrigin<DescriptorStoredOrigin>,
        ) -> Value {
            let mut dependencies = BTreeSet::new();
            for dependency in &provider.lifetime_dependencies {
                for captured in self.substitute(machine, state, call, &dependency.origin)? {
                    match captured {
                        Origin::Provider(previous) => {
                            dependencies.extend(previous.lifetime_dependencies);
                        }
                        ordinary => {
                            dependencies.insert(DescriptorLifetimeDebt {
                                origin: ordinary,
                                access: dependency.access,
                            });
                        }
                    }
                }
            }
            let mut invocations = vec![DescriptorInvocationSite {
                machine: machine.id,
                operation: call.operation,
            }];
            invocations.extend_from_slice(&provider.invocations);
            Some(BTreeSet::from([Origin::Provider(
                DescriptorProviderOrigin {
                    boundary: provider.boundary,
                    output: provider.output,
                    invocations,
                    projection: provider.projection.clone(),
                    lifetime_dependencies: dependencies.into_iter().collect(),
                },
            )]))
        }

        pub(crate) fn source_origin(origin: &DescriptorSourceOrigin) -> Option<Origin> {
            Some(match origin {
                DescriptorSourceOrigin::Provider(provider) => {
                    Origin::Provider(provider.map_dependencies(Self::source_origin)?)
                }
                DescriptorSourceOrigin::Backing(origin) => Origin::Backing(origin.clone()),
                DescriptorSourceOrigin::IncomingSlot(location) => {
                    Origin::Incoming(location.clone())
                }
                DescriptorSourceOrigin::IncomingContents { query, projection } => {
                    Origin::IncomingContents(query.clone(), projection.clone())
                }
            })
        }
    }
}
pub(crate) mod summary_interface {
    use super::engine::{Replay, Summary};
    use super::*;

    impl Replay<'_> {
        pub(crate) fn empty_interface_summary(
            &self,
            machine: &DescriptorMachine,
        ) -> Option<Summary> {
            if machine.returns.is_some() {
                return None;
            }
            for row in &machine.places {
                let DescriptorDefinition::Parameter {
                    structural_type,
                    access,
                    ..
                } = row.definition
                else {
                    continue;
                };
                if !self
                    .slot_kind
                    .writable_slots(self.types, row.place, structural_type, access)?
                    .is_empty()
                    || self.slot_kind.may_write_indirect_contents(
                        self.types,
                        structural_type,
                        &StructuralArgument {
                            place: row.place,
                            path: Vec::new(),
                            access,
                        },
                    )
                {
                    return None;
                }
            }
            let mut pending = vec![machine.id];
            let mut visited = BTreeSet::new();
            let mut invalidates_contents = false;
            while let Some(identity) = pending.pop() {
                if !visited.insert(identity) {
                    continue;
                }
                let body = self.machine(identity)?;
                super::case_flow::validate_graph(self, body)?;
                invalidates_contents |= !body.unresolved_contents_writes.is_empty();
                for call in body.calls.values() {
                    invalidates_contents |= call.argument_contents.iter().any(Option::is_none);
                    match &call.target {
                        DescriptorInvocationTarget::Machine(callee) => pending.push(*callee),
                        DescriptorInvocationTarget::Provider {
                            invalidates_contents: effect,
                            ..
                        } => {
                            invalidates_contents |= effect;
                        }
                        DescriptorInvocationTarget::Public {
                            effects_machine,
                            realizations,
                            invalidates_contents: effect,
                            ..
                        } => {
                            invalidates_contents |= effect;
                            pending.extend(effects_machine);
                            pending.extend(realizations);
                        }
                    }
                }
            }
            Some(Summary {
                result: None,
                result_cases: Default::default(),
                effects: Vec::new(),
                invalidates_contents,
            })
        }
    }
}
use crate::runtime_structural_path_tip;
pub use analysis::{DescriptorAnalysis, DescriptorSourceObservation, DescriptorSourceObservations};
use engine::Replay;
use terminal_psi::find_structural_type;

#[derive(Clone)]
pub enum DescriptorStoredOrigin {
    Invocation {
        carrier_type: StructuralTypeId,
        projection: Projection,
    },
    Provider(DescriptorProviderOrigin<DescriptorStoredOrigin>),
    AccessFormation {
        machine: MachineId,
        operation: semantic_vocabulary::OperationId,
        place: PlaceId,
        carrier: StructuralTypeId,
        projection: Projection,
    },
    Empty,
    Parameter {
        position: usize,
        projection: Projection,
    },
    IncomingSlot {
        position: usize,
        path: Vec<DescriptorPathSegment>,
    },
    IncomingContents {
        position: usize,
        steps: Vec<DescriptorContentsStep>,
        projection: Projection,
    },
    Literal {
        machine: MachineId,
        place: PlaceId,
    },
}
impl DescriptorStoredOrigin {
    pub fn captured_entry_slot(
        &self,
    ) -> Option<(usize, std::borrow::Cow<'_, [DescriptorPathSegment]>)> {
        match self {
            Self::IncomingSlot { position, path } => {
                Some((*position, std::borrow::Cow::Borrowed(path)))
            }
            Self::IncomingContents {
                position,
                steps,
                projection,
            } if *projection == Projection::new(&[]) => Some((
                *position,
                std::borrow::Cow::Owned(
                    steps
                        .iter()
                        .map(|step| match step {
                            DescriptorContentsStep::Address(segment) => Some(segment.clone()),
                            DescriptorContentsStep::AnyElement { .. } => None,
                        })
                        .collect::<Option<Vec<_>>>()?,
                ),
            )),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub struct DescriptorStoredEffect {
    pub parameter: usize,
    pub steps: Vec<DescriptorContentsStep>,
    pub origins: Vec<DescriptorStoredOrigin>,
}
impl DescriptorStoredEffect {
    pub fn exact_path(&self) -> Option<Vec<DescriptorPathSegment>> {
        self.steps
            .iter()
            .map(|step| match step {
                DescriptorContentsStep::Address(segment) => Some(segment.clone()),
                DescriptorContentsStep::AnyElement { .. } => None,
            })
            .collect()
    }
}
pub struct DescriptorStoredCallEffect<Origin = DescriptorCallOrigin> {
    pub destination: DescriptorContentsQuery,
    pub origins: Vec<Origin>,
    pub preserves_incoming: bool,
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
fn result_origin(
    types: &[StructuralTypeDeclaration],
    machine: &DescriptorMachine,
    origin: DescriptorStoredOrigin,
) -> Option<DescriptorResultOrigin> {
    match origin {
        DescriptorStoredOrigin::Invocation {
            carrier_type,
            projection,
        } => Some(DescriptorResultOrigin::Invocation {
            carrier_type,
            projection,
        }),
        DescriptorStoredOrigin::AccessFormation { .. } | DescriptorStoredOrigin::Provider(_) => {
            None
        }
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
        DescriptorStoredOrigin::IncomingContents {
            position,
            steps,
            projection,
        } => {
            let parameter = machine.places.iter().find_map(|row| match row.definition {
                DescriptorDefinition::Parameter {
                    position: actual,
                    structural_type,
                    ..
                } if actual == position => Some((row.place, structural_type)),
                _ => None,
            })?;
            let query = DescriptorContentsQuery {
                root: parameter.0,
                steps,
            };
            Some(DescriptorResultOrigin::Parameter {
                position,
                projection: query
                    .conservative_projection(types, parameter.1)?
                    .then(&projection),
            })
        }
    }
}
pub(crate) fn result_origins(
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
        .find(|slot| slot.path.is_empty())?
        .origins
        .into_iter()
        .map(|origin| result_origin(types, machine, origin))
        .collect()
}
pub(crate) fn return_origins(
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
pub(crate) fn return_slots_have_external_backing(
    types: &[StructuralTypeDeclaration],
    machines: &[DescriptorMachine],
    identity: MachineId,
    position: DescriptorPosition,
    source: PlaceId,
) -> Option<()> {
    let mut replay = Replay::new(types, machines);
    let machine = replay.machine(identity)?;
    let slots =
        DescriptorSlotKind::Views.owned_slots(types, source, *machine.place_types.get(&source)?)?;
    if slots.is_empty() || slots.iter().any(|slot| slot.path.is_empty()) {
        return None;
    }
    let states = replay.states(identity, 0)?;
    let state = states.get(&position)?;
    for slot in slots {
        let origins = replay.read(machine, state, &slot)?;
        if origins.is_empty() {
            return None;
        }
        for origin in origins {
            replay.exported(machine, &origin)?;
        }
    }
    Some(())
}

pub(crate) fn call_origins(
    types: &[StructuralTypeDeclaration],
    machines: &[DescriptorMachine],
    identity: MachineId,
    result: PlaceId,
) -> Option<Vec<DescriptorCallOrigin>> {
    DescriptorAnalysis::new(types, machines).call_origins(identity, result)
}

fn may_contain_descriptor(
    kind: DescriptorSlotKind,
    scope: DescriptorStorageScope,
    types: &[StructuralTypeDeclaration],
    ty: StructuralTypeId,
    active: &mut BTreeSet<StructuralTypeId>,
) -> bool {
    if !active.insert(ty) {
        return true;
    }
    let Some(declaration) = find_structural_type(types, ty) else {
        return true;
    };
    let mut fields_contain = |fields: &[terminal_psi::StructuralFieldDeclaration]| {
        fields.iter().any(|field| match field.field_type {
            terminal_psi::StructuralFieldType::Structural(ty) => {
                may_contain_descriptor(kind, scope, types, ty, active)
            }
            _ => kind.is_view_field_slot(&field.field_type),
        })
    };
    let contains = match &declaration.shape {
        StructuralTypeShape::Record { fields, .. } => fields_contain(fields),
        StructuralTypeShape::Mixed { fields, cases } => {
            fields_contain(fields) || cases.iter().any(|case| fields_contain(&case.fields))
        }
        StructuralTypeShape::Sum { cases } => cases.iter().any(|case| fields_contain(&case.fields)),
        StructuralTypeShape::Reference { referent, .. } => {
            kind.is_slot(types, ty)
                || (scope == DescriptorStorageScope::Reachable
                    && may_contain_descriptor(kind, scope, types, *referent, active))
        }
        StructuralTypeShape::FixedArray { element, .. } => {
            may_contain_descriptor(kind, scope, types, *element, active)
        }
        _ => kind.is_slot(types, ty),
    };
    active.remove(&ty);
    contains
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DescriptorSlotKind {
    Views,
    References,
    BorrowedStorage,
    AccessOrigins,
}
impl DescriptorSlotKind {
    pub fn tracks_returned_value(
        self,
        types: &[StructuralTypeDeclaration],
        place: PlaceId,
        structural_type: StructuralTypeId,
    ) -> bool {
        self.owned_slots(types, place, structural_type)
            .is_some_and(|slots| !slots.is_empty())
            || case_flow::contains_case(
                types,
                structural_type,
                &mut BTreeMap::new(),
                &mut BTreeSet::new(),
            )
    }

    pub fn tracks_views(self) -> bool {
        matches!(self, Self::Views | Self::BorrowedStorage)
    }

    pub fn tracks_references(self) -> bool {
        matches!(self, Self::References | Self::BorrowedStorage)
    }

    pub fn is_slot(self, types: &[StructuralTypeDeclaration], ty: StructuralTypeId) -> bool {
        match self {
            Self::Views => is_descriptor(types.iter(), ty),
            Self::AccessOrigins => find_structural_type(types, ty).is_some_and(|declaration| {
                matches!(
                    declaration.shape,
                    StructuralTypeShape::Reference { .. }
                        | StructuralTypeShape::Record {
                            storage: terminal_psi::StructuralRecordStorage::AccessView(_)
                                | terminal_psi::StructuralRecordStorage::Accessor(_),
                            ..
                        }
                )
            }),
            Self::References => find_structural_type(types, ty).is_some_and(|declaration| {
                matches!(declaration.shape, StructuralTypeShape::Reference { .. })
            }),
            Self::BorrowedStorage => {
                Self::Views.is_slot(types, ty) || Self::References.is_slot(types, ty)
            }
        }
    }

    pub(crate) fn parameter_is_incoming_slot(
        self,
        types: &[StructuralTypeDeclaration],
        ty: StructuralTypeId,
    ) -> bool {
        match self {
            Self::BorrowedStorage => Self::References.is_slot(types, ty),
            Self::Views => false,
            Self::References | Self::AccessOrigins => self.is_slot(types, ty),
        }
    }
    pub(crate) fn is_view_field_slot(self, field_type: &terminal_psi::StructuralFieldType) -> bool {
        self.tracks_views()
            && field_type
                .byte_sequence_carrier()
                .is_some_and(|carrier| carrier.is_borrowed())
    }
    pub fn slots(
        self,
        types: &[StructuralTypeDeclaration],
        root: PlaceId,
        structural_type: StructuralTypeId,
    ) -> Option<Vec<DescriptorLocation>> {
        let inventory = self.inventory(types, root, structural_type)?;
        inventory
            .opaque_referents
            .is_empty()
            .then_some(inventory.slots)
    }
    pub fn owned_slots(
        self,
        types: &[StructuralTypeDeclaration],
        root: PlaceId,
        structural_type: StructuralTypeId,
    ) -> Option<Vec<DescriptorLocation>> {
        Some(
            slot_inventory(
                self,
                DescriptorStorageScope::Owned,
                types,
                root,
                structural_type,
            )?
            .slots,
        )
    }

    pub fn inventory(
        self,
        types: &[StructuralTypeDeclaration],
        root: PlaceId,
        structural_type: StructuralTypeId,
    ) -> Option<DescriptorSlotInventory> {
        slot_inventory(
            self,
            DescriptorStorageScope::Reachable,
            types,
            root,
            structural_type,
        )
    }

    pub fn may_write_indirect_contents(
        self,
        types: &[StructuralTypeDeclaration],
        root_type: StructuralTypeId,
        argument: &StructuralArgument,
    ) -> bool {
        if argument.access == StructuralAccess::SharedBorrow {
            return false;
        }
        let Some(selected) = runtime_structural_path_tip(types.iter(), root_type, &argument.path)
        else {
            return true;
        };
        let behind_view = argument.path.iter().enumerate().any(|(position, segment)| {
            matches!(
                segment,
                StructuralPathSegment::RuntimeIndex { .. } | StructuralPathSegment::FixedIndex(_)
            ) && runtime_structural_path_tip(types.iter(), root_type, &argument.path[..position])
                .is_none_or(|prefix| {
                    matches!(prefix.shape, StructuralTypeShape::ElementView { .. })
                })
        });
        indirect_contents_contain_slot(self, types, selected.id, behind_view, &mut BTreeSet::new())
    }

    pub fn overwritten_slots(
        self,
        types: &[StructuralTypeDeclaration],
        root: PlaceId,
        structural_type: StructuralTypeId,
        path: &[StructuralPathSegment],
        field: Option<semantic_vocabulary::StructuralFieldId>,
    ) -> Option<Vec<DescriptorLocation>> {
        let mut destination = path.to_vec();
        if let Some(field) = field {
            let declaration =
                crate::projected_field_declaration(types.iter(), structural_type, path, field)?;
            destination.push(StructuralPathSegment::Field(declaration.identity));
        } else {
            runtime_structural_path_tip(types.iter(), structural_type, path)?;
        }
        let mut overwritten = Vec::new();
        for slot in self.slots(types, root, structural_type)? {
            let resident = descriptor_path(types, structural_type, &slot.path)?;
            let mut overlaps = true;
            for (position, selected) in destination.iter().enumerate() {
                if matches!(selected, StructuralPathSegment::RepresentationView { .. }) {
                    break;
                }
                if !resident.get(position).is_some_and(|stored| {
                    stored == selected
                        || matches!(
                            (selected, stored),
                            (
                                StructuralPathSegment::RuntimeIndex { .. },
                                StructuralPathSegment::FixedIndex(_)
                            )
                        )
                }) {
                    overlaps = false;
                    break;
                }
            }
            if overlaps {
                overwritten.push(slot);
            }
        }
        Some(overwritten)
    }
}

fn indirect_contents_contain_slot(
    kind: DescriptorSlotKind,
    types: &[StructuralTypeDeclaration],
    structural_type: StructuralTypeId,
    behind_view: bool,
    active: &mut BTreeSet<StructuralTypeId>,
) -> bool {
    if behind_view && kind.is_slot(types, structural_type) {
        return true;
    }
    if !active.insert(structural_type) {
        return true;
    }
    let Some(declaration) = find_structural_type(types, structural_type) else {
        return true;
    };
    let mut fields_contain = |fields: &[terminal_psi::StructuralFieldDeclaration]| {
        fields.iter().any(|field| match field.field_type {
            terminal_psi::StructuralFieldType::Structural(child) => {
                indirect_contents_contain_slot(kind, types, child, behind_view, active)
            }
            _ => behind_view && kind.is_view_field_slot(&field.field_type),
        })
    };
    let contains = match &declaration.shape {
        StructuralTypeShape::ElementView { element, .. } => {
            indirect_contents_contain_slot(kind, types, *element, true, active)
        }
        StructuralTypeShape::FixedArray { element, .. }
        | StructuralTypeShape::Reference {
            referent: element, ..
        } => indirect_contents_contain_slot(kind, types, *element, behind_view, active),
        StructuralTypeShape::Record { fields, .. } => fields_contain(fields),
        StructuralTypeShape::Mixed { fields, cases } => {
            fields_contain(fields) || cases.iter().any(|case| fields_contain(&case.fields))
        }
        StructuralTypeShape::Sum { cases } => cases.iter().any(|case| fields_contain(&case.fields)),
        _ => false,
    };
    active.remove(&structural_type);
    contains
}

impl DescriptorSlotKind {
    pub fn writable_slots(
        self,
        types: &[StructuralTypeDeclaration],
        root: PlaceId,
        structural_type: StructuralTypeId,
        access: StructuralAccess,
    ) -> Option<Vec<DescriptorLocation>> {
        if access == StructuralAccess::SharedBorrow {
            return Some(Vec::new());
        }
        if self.tracks_views()
            && matches!(
                find_structural_type(types, structural_type)?.shape,
                StructuralTypeShape::ByteSequence(
                    ByteSequenceCarrier::BorrowedView { access: None }
                        | ByteSequenceCarrier::BoundedView { access: None, .. }
                ) | StructuralTypeShape::ElementView { access: None, .. }
            )
        {
            return Some(Vec::new());
        }
        let inventory = self.inventory(types, root, structural_type)?;
        let writable = |path: &[DescriptorPathSegment]| -> Option<bool> {
            let mut external = access != StructuralAccess::Owned;
            let mut permission = access;
            for (index, segment) in path.iter().enumerate() {
                if *segment == DescriptorPathSegment::Referent {
                    if matches!(
                        permission,
                        StructuralAccess::SharedBorrow | StructuralAccess::WriteOnlyBorrow
                    ) {
                        return Some(false);
                    }
                    let ty = crate::descriptors::descriptor_lifetimes::descriptor_location_type(
                        types,
                        structural_type,
                        &path[..index],
                    )?;
                    let StructuralTypeShape::Reference { access, .. } =
                        find_structural_type(types, ty)?.shape
                    else {
                        return None;
                    };
                    external = true;
                    permission = access;
                }
            }
            Some(external && matches!(permission, terminal_psi::exclusive_borrow_pair_pattern!()))
        };
        for opaque in inventory.opaque_referents {
            if writable(&opaque.path)? {
                return None;
            }
        }
        let mut slots = Vec::new();
        for slot in inventory.slots {
            if writable(&slot.path)? {
                slots.push(slot);
            }
        }
        Some(slots)
    }
}

pub struct DescriptorSlotInventory {
    pub slots: Vec<DescriptorLocation>,
    pub opaque_referents: Vec<DescriptorLocation>,
}

pub fn descriptor_slots(
    types: &[StructuralTypeDeclaration],
    root: PlaceId,
    structural_type: StructuralTypeId,
) -> Option<Vec<DescriptorLocation>> {
    DescriptorSlotKind::Views.slots(types, root, structural_type)
}

pub fn descriptor_slot_inventory(
    types: &[StructuralTypeDeclaration],
    root: PlaceId,
    structural_type: StructuralTypeId,
) -> Option<DescriptorSlotInventory> {
    DescriptorSlotKind::Views.inventory(types, root, structural_type)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DescriptorStorageScope {
    Owned,
    Reachable,
}

fn slot_inventory(
    kind: DescriptorSlotKind,
    scope: DescriptorStorageScope,
    types: &[StructuralTypeDeclaration],
    root: PlaceId,
    structural_type: StructuralTypeId,
) -> Option<DescriptorSlotInventory> {
    fn walk(
        kind: DescriptorSlotKind,
        scope: DescriptorStorageScope,
        types: &[StructuralTypeDeclaration],
        ty: StructuralTypeId,
        location: DescriptorLocation,
        active: &mut BTreeSet<StructuralTypeId>,
        result: &mut DescriptorSlotInventory,
    ) -> Option<()> {
        if !active.insert(ty) || result.slots.len() + result.opaque_referents.len() > 4096 {
            return None;
        }
        let declaration = exactly_one(types.iter(), |declaration| declaration.id == ty)?;
        if kind.is_slot(types, ty) {
            result.slots.push(location.clone());
            if !kind.tracks_references() || scope == DescriptorStorageScope::Owned {
                active.remove(&ty);
                return Some(());
            }
        }
        match &declaration.shape {
            StructuralTypeShape::Reference { referent, .. }
                if scope == DescriptorStorageScope::Reachable =>
            {
                let mut child = location.clone();
                child.path.push(DescriptorPathSegment::Referent);
                if active.contains(referent) || find_structural_type(types, *referent).is_none() {
                    result.opaque_referents.push(child);
                } else {
                    walk(kind, scope, types, *referent, child, active, result)?;
                }
            }
            terminal_psi::record_or_mixed_shape_pattern!(fields) => {
                for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                    let mut child = location.clone();
                    child.path.push(DescriptorPathSegment::Field(field.id));
                    match field.field_type {
                        terminal_psi::StructuralFieldType::Structural(ty) => {
                            walk(kind, scope, types, ty, child, active, result)?
                        }
                        _ if kind.is_view_field_slot(&field.field_type) => result.slots.push(child),
                        _ => {}
                    }
                }
            }
            StructuralTypeShape::FixedArray { element, length } => {
                if !may_contain_descriptor(kind, scope, types, *element, &mut BTreeSet::new()) {
                    active.remove(&ty);
                    return Some(());
                }
                let length = length.literal()?;
                if length > 4096 {
                    return None;
                }
                for index in 0..length {
                    let mut child = location.clone();
                    child.path.push(DescriptorPathSegment::FixedIndex(index));
                    walk(kind, scope, types, *element, child, active, result)?;
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
                            walk(kind, scope, types, ty, child, active, result)?
                        }
                        _ if kind.is_view_field_slot(&field.field_type) => result.slots.push(child),
                        _ => {}
                    }
                }
            }
        }
        active.remove(&ty);
        Some(())
    }
    let mut result = DescriptorSlotInventory {
        slots: Vec::new(),
        opaque_referents: Vec::new(),
    };
    walk(
        kind,
        scope,
        types,
        structural_type,
        DescriptorLocation::whole(root),
        &mut BTreeSet::new(),
        &mut result,
    )?;
    Some(result)
}
