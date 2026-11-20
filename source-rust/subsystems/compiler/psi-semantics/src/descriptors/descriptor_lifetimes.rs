use semantic_vocabulary::{BlockId, EdgeId, ObligationId, PlaceId};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) mod arrivals;
pub use arrivals::{
    DescriptorContentsQuery, DescriptorContentsStep, DescriptorFlowBinding,
    DescriptorFlowSource,
};
pub(crate) mod locations;
pub(crate) mod residences;
pub use locations::{
    canonical_descriptor_location, descriptor_location, descriptor_location_type,
    descriptor_path, fixed_runtime_index_alternatives, view_element_descriptor_prefix,
};
pub use residences::{DescriptorResidenceRules, descriptor_residence_live_positions};

pub type DescriptorPosition = (BlockId, u32);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DescriptorLocation {
    pub root: PlaceId,
    pub path: Vec<DescriptorPathSegment>,
}

pub use terminal_psi::ResolvedStructuralPathSegment as DescriptorPathSegment;

impl DescriptorLocation {
    /// Retain a captured address without translating its storage steps.
    /// A runtime selector needs its capture point; an extent is metadata.
    pub fn captured(root: PlaceId, path: &[DescriptorPathSegment]) -> Option<Self> {
        if path.iter().any(|step| {
            matches!(
                step,
                DescriptorPathSegment::RuntimeIndex { .. }
                    | DescriptorPathSegment::Extent
                    | DescriptorPathSegment::UnknownContents
            )
        }) {
            return None;
        }
        Some(Self {
            root,
            path: path.to_vec(),
        })
    }

    pub fn whole(root: PlaceId) -> Self {
        Self {
            root,
            path: Vec::new(),
        }
    }
    pub fn overlaps(&self, other: &Self) -> bool {
        self.root == other.root
            && crate::storage::place_paths::may_overlap(&self.path, &other.path)
    }
    pub fn contains(&self, other: &Self) -> bool {
        self.root == other.root && other.path.starts_with(&self.path)
    }
    pub fn transported(&self, source: &Self, destination: &Self) -> Option<Self> {
        if !source.contains(self) {
            return None;
        }
        let mut path = destination.path.clone();
        path.extend_from_slice(&self.path[source.path.len()..]);
        Some(Self {
            root: destination.root,
            path,
        })
    }
}

/// The operand order also identifies captured constructor elements. A
/// memory operation belongs to its original typed source, not to aliases
/// later added solely to keep a loan alive. Its observation is absent when
/// a replacement does not read the old value. An unavailable source cannot
/// establish permission even when its whole root is observed for liveness.
pub type DescriptorFlowInput = (
    Option<DescriptorLocation>,
    Option<(access_vocabulary::AccessOperation, DescriptorFlowSource)>,
);

#[derive(Clone, Default)]
pub struct DescriptorFlowStep {
    pub definitions: Vec<DescriptorLocation>,
    /// The subset of `definitions` that an owned move consumes rather
    /// than overwrites. Loan liveness treats both alike. Descriptor
    /// replay keeps a consumed parameter's entry value readable, so the
    /// machine's summary can still report the caller's storage behind a
    /// reference the parameter held.
    pub consumed: Vec<DescriptorLocation>,
    pub derivations: Vec<(DescriptorLocation, DescriptorLocation)>,
    /// Exact value copies, including whole records with no descriptor
    /// slots. Backing-preserving projections and content transformations
    /// remain ordinary derivations and cannot transport membership facts.
    pub value_transfers: Vec<(DescriptorLocation, DescriptorLocation)>,
    /// Exact scalar inputs stored by this operation. Expanded numeric terms
    /// can prove equalities but cannot stand in for the copied value.
    pub scalar_value_stores: Vec<(DescriptorLocation, semantic_vocabulary::ValueId)>,
    pub authority_derivations: Vec<(DescriptorLocation, DescriptorLocation)>,
    pub inputs: Vec<DescriptorFlowInput>,
    pub may_escape: bool,
    pub successors: Vec<DescriptorFlowEdge>,
}

impl DescriptorFlowStep {
    pub(crate) fn copy_value(
        &mut self,
        source: DescriptorLocation,
        destination: DescriptorLocation,
    ) {
        self.value_transfers
            .push((source.clone(), destination.clone()));
        self.derivations.push((source, destination));
    }

    pub fn transport_authority(
        &self,
        before: &BTreeSet<DescriptorLocation>,
        may: bool,
    ) -> BTreeSet<DescriptorLocation> {
        transport_authority(
            before,
            &self.definitions,
            self.authority_derivations
                .iter()
                .map(|(source, destination)| (source, destination)),
            false,
            may,
        )
    }

    pub fn observed_inputs(&self) -> impl Iterator<Item = &DescriptorLocation> {
        self.inputs.iter().filter_map(|(input, _)| input.as_ref())
    }

    /// Dropping a value observation does not erase an actual memory
    /// operation. This matters when cleanup discards an old descriptor.
    pub fn forget_observation(&mut self, index: usize) {
        if self.inputs[index].1.is_some() {
            self.inputs[index].0 = None;
        } else {
            self.inputs.remove(index);
        }
    }

    /// A value-transfer rewrite can refine the loan observations without
    /// replacing the instruction's actual memory operations. Constructor
    /// operands are never rewritten here: their order and duplicates are
    /// the captured element roster.
    pub fn replace_value_inputs(
        &mut self,
        inputs: impl IntoIterator<Item = DescriptorLocation>,
    ) {
        self.inputs.retain(|(_, access)| access.is_some());
        for (observed, _) in &mut self.inputs {
            *observed = None;
        }
        self.inputs
            .extend(inputs.into_iter().map(|input| (Some(input), None)));
    }
}

#[derive(Clone)]
pub struct DescriptorFlowEdge {
    /// Sources consumed on this selected edge. Destination parameters
    /// remain a separate roster, so swaps capture every old value before
    /// any source or destination is retired.
    pub definitions: Vec<DescriptorLocation>,
    /// Present for a genuine Terminal successor, absent on an internal
    /// operation sequence edge. Cleanup calls select this exact edge.
    pub edge: Option<semantic_vocabulary::EdgeId>,
    /// Prepared ordinary call effects, in this edge's exact action order.
    /// Their inputs are observed at the genuine source terminator cut.
    pub effects: Vec<(
        crate::descriptor_results::DescriptorInvocationOwner,
        DescriptorFlowStep,
    )>,
    pub target: DescriptorPosition,
    pub parameters: Vec<DescriptorLocation>,
    pub bindings: Vec<DescriptorFlowBinding>,
}

impl DescriptorFlowEdge {
    /// A borrowed header keeps the permission of the actual it names.
    /// When the actual crosses a reference, its immediate descriptor is
    /// the authorizing name; an inline projection keeps its root's loan.
    pub fn borrowed_authority(&self, name: &DescriptorLocation) -> Vec<DescriptorLocation> {
        self.bindings
            .iter()
            .filter(|binding| {
                binding.access != terminal_psi::StructuralAccess::Owned
                    && binding.source_access == binding.access
            })
            .filter_map(|binding| {
                let source = binding.source.exact_location()?;
                let parent = source
                    .path
                    .iter()
                    .rposition(|step| *step == DescriptorPathSegment::Referent);
                let prefix = parent.map_or(&[][..], |position| &source.path[..position]);
                (name.root == source.root && name.path == prefix)
                    .then(|| binding.parameter.clone())
            })
            .collect()
    }

    pub fn derivations(
        &self,
    ) -> impl Clone + Iterator<Item = (&DescriptorLocation, &DescriptorLocation)> {
        self.bindings
            .iter()
            .filter_map(DescriptorFlowBinding::locations)
    }

    fn authority_derivations(
        &self,
    ) -> impl Clone + Iterator<Item = (&DescriptorLocation, &DescriptorLocation)> {
        self.bindings
            .iter()
            .filter(|binding| {
                binding.access == terminal_psi::StructuralAccess::Owned
                    || binding.source_access == binding.access
            })
            .filter_map(DescriptorFlowBinding::locations)
    }

    /// Move the available names across this exact successor. Cleanup
    /// affects names that remain in storage. A block argument instead
    /// keeps the value captured before running the cleanup.
    fn transport_locations(
        &self,
        before: &BTreeSet<DescriptorLocation>,
        invalidate_containers: bool,
        may: bool,
    ) -> BTreeSet<DescriptorLocation> {
        let definitions = self
            .parameters
            .iter()
            .chain(&self.definitions)
            .cloned()
            .collect::<Vec<_>>();
        let mut after_cleanup = before.clone();
        for (_, effect) in &self.effects {
            let derivations = if invalidate_containers {
                &effect.derivations
            } else {
                &effect.authority_derivations
            };
            after_cleanup = transport_authority(
                &after_cleanup,
                &effect.definitions,
                derivations
                    .iter()
                    .map(|(source, destination)| (source, destination)),
                invalidate_containers,
                may,
            );
        }
        let mut transported = transport_authority(
            &after_cleanup,
            &definitions,
            std::iter::empty(),
            invalidate_containers,
            may,
        );
        let captured = if invalidate_containers {
            transport_authority(
                before,
                &definitions,
                self.derivations(),
                invalidate_containers,
                may,
            )
        } else {
            transport_authority(
                before,
                &definitions,
                self.authority_derivations(),
                invalidate_containers,
                may,
            )
        };
        transported.extend(captured.into_iter().filter(|name| {
            self.bindings
                .iter()
                .any(|binding| binding.parameter.contains(name))
        }));
        if !invalidate_containers {
            for name in before {
                transported.extend(self.borrowed_authority(name));
            }
        }
        transported
    }

    fn forward_effects(
        &self,
        source: &DescriptorLocation,
        authority: bool,
    ) -> Vec<DescriptorLocation> {
        let mut names = vec![source.clone()];
        for (_, effect) in &self.effects {
            let derivations = if authority {
                &effect.authority_derivations
            } else {
                &effect.derivations
            };
            names = names
                .into_iter()
                .flat_map(|name| {
                    let retained = (!effect
                        .definitions
                        .iter()
                        .any(|definition| definition.contains(&name)))
                    .then(|| name.clone());
                    retained.into_iter().chain(
                        derivations
                            .iter()
                            .filter_map(move |(from, to)| name.transported(from, to)),
                    )
                })
                .collect();
            names.sort();
            names.dedup();
        }
        names
            .into_iter()
            .filter(|name| {
                !self
                    .parameters
                    .iter()
                    .chain(&self.definitions)
                    .any(|definition| definition.contains(name))
            })
            .collect()
    }

    fn forward_authority(&self, source: &DescriptorLocation) -> Vec<DescriptorLocation> {
        let mut result = self.forward_effects(source, true);
        result.extend(
            self.authority_derivations()
                .filter_map(|(actual, parameter)| source.transported(actual, parameter)),
        );
        result.extend(self.borrowed_authority(source));
        result
    }

    pub fn forward(&self, source: &DescriptorLocation) -> Vec<DescriptorLocation> {
        let mut result = self.forward_effects(source, false);
        result.extend(
            self.derivations()
                .filter_map(|(actual, parameter)| source.transported(actual, parameter)),
        );
        result
    }
}

pub fn descriptor_location_live_positions(
    steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    birth: DescriptorPosition,
    descriptor: &DescriptorLocation,
) -> BTreeSet<DescriptorPosition> {
    location_live_positions(steps, birth, descriptor, false, false, None, |_, _| false).live
}

struct DescriptorDemand {
    live: BTreeSet<DescriptorPosition>,
    names: BTreeMap<DescriptorPosition, BTreeSet<DescriptorLocation>>,
}

fn location_live_positions(
    steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    birth: DescriptorPosition,
    descriptor: &DescriptorLocation,
    at_entry: bool,
    borrowed_authority: bool,
    successor: Option<DescriptorPosition>,
    needs_contents: impl Fn(DescriptorPosition, &DescriptorLocation) -> bool,
) -> DescriptorDemand {
    type Occurrence = (DescriptorPosition, DescriptorLocation);
    let mut pending = Vec::new();
    if let Some(target) = successor {
        pending.push((target, descriptor.clone()));
    } else if at_entry {
        pending.push((birth, descriptor.clone()));
    } else if let Some(step) = steps.get(&birth) {
        for edge in &step.successors {
            pending.extend(
                edge.forward(descriptor)
                    .into_iter()
                    .map(|name| (edge.target, name)),
            );
        }
    }
    let mut visited = BTreeSet::new();
    let mut predecessors = BTreeMap::<Occurrence, BTreeSet<Occurrence>>::new();
    let mut observations = Vec::new();
    let mut escaped = Vec::new();
    while let Some(occurrence) = pending.pop() {
        let (position, name) = &occurrence;
        if !visited.insert(occurrence.clone()) {
            continue;
        }
        let Some(step) = steps.get(position) else {
            continue;
        };
        if step
            .observed_inputs()
            .any(|input| input.contains(name) || name.contains(input))
        {
            observations.push(occurrence.clone());
            if step.may_escape {
                escaped.push(*position);
            }
        } else if needs_contents(*position, name) {
            observations.push(occurrence.clone());
        }
        let mut outputs = Vec::new();
        if !step
            .definitions
            .iter()
            .any(|definition| definition.contains(name))
        {
            outputs.push(name.clone());
        }
        outputs.extend(
            step.derivations
                .iter()
                .filter_map(|(source, result)| name.transported(source, result)),
        );
        for edge in &step.successors {
            for output in &outputs {
                let destinations = if borrowed_authority {
                    edge.forward_authority(output)
                } else {
                    edge.forward(output)
                };
                for destination in destinations {
                    let next = (edge.target, destination);
                    predecessors
                        .entry(next.clone())
                        .or_default()
                        .insert(occurrence.clone());
                    pending.push(next);
                }
            }
        }
    }
    let mut observed = BTreeSet::new();
    let mut live = BTreeSet::new();
    while let Some(occurrence) = observations.pop() {
        let position = occurrence.0;
        if !observed.insert(occurrence.clone()) {
            continue;
        }
        live.insert(position);
        observations.extend(predecessors.get(&occurrence).into_iter().flatten().cloned());
    }
    let mut escaped_positions = BTreeSet::new();
    while let Some(position) = escaped.pop() {
        if !escaped_positions.insert(position) {
            continue;
        }
        live.insert(position);
        if let Some(step) = steps.get(&position) {
            escaped.extend(step.successors.iter().map(|edge| edge.target));
        }
    }
    let mut names = BTreeMap::<DescriptorPosition, BTreeSet<DescriptorLocation>>::new();
    for (position, name) in observed.into_iter().chain(
        visited
            .into_iter()
            .filter(|(position, _)| escaped_positions.contains(position)),
    ) {
        names.entry(position).or_default().insert(name);
    }
    DescriptorDemand { live, names }
}

pub struct DescriptorLifetime {
    pub live: BTreeSet<DescriptorPosition>,
    /// Future demand of each actual carrier spelling. A dead copy can
    /// be released without rewalking the graph from every possible cut.
    pub demanded: BTreeMap<DescriptorPosition, BTreeSet<DescriptorLocation>>,
    pub authorized: BTreeMap<DescriptorPosition, BTreeSet<DescriptorLocation>>,
    /// All carrier names that can still own this resource on any arrival.
    /// Unlike `authorized`, absence of a common name at a join does not
    /// erase a surviving occurrence on one of its predecessors.
    pub surviving: BTreeMap<DescriptorPosition, BTreeSet<DescriptorLocation>>,
}

pub fn descriptor_entry_lifetime(
    steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    entry: DescriptorPosition,
    descriptor: &DescriptorLocation,
) -> DescriptorLifetime {
    descriptor_entry_lifetime_with_contents(steps, entry, descriptor, |_, _| false)
}

pub(crate) fn descriptor_entry_lifetime_with_contents(
    steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    entry: DescriptorPosition,
    descriptor: &DescriptorLocation,
    needs_contents: impl Fn(DescriptorPosition, &DescriptorLocation) -> bool,
) -> DescriptorLifetime {
    let demand =
        location_live_positions(steps, entry, descriptor, true, true, None, needs_contents);
    DescriptorLifetime {
        live: demand.live,
        demanded: demand.names,
        authorized: transported_locations(steps, entry, descriptor, true, false, false, None),
        surviving: transported_locations(steps, entry, descriptor, true, false, true, None),
    }
}

pub(crate) fn descriptor_continuation_locations(
    steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    at: DescriptorPosition,
    descriptor: &DescriptorLocation,
) -> BTreeMap<DescriptorPosition, BTreeSet<DescriptorLocation>> {
    transported_locations(steps, at, descriptor, true, false, false, None)
}

pub fn descriptor_location_lifetime(
    steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    birth: DescriptorPosition,
    descriptor: &DescriptorLocation,
) -> DescriptorLifetime {
    descriptor_location_lifetime_with_contents(steps, birth, descriptor, |_, _| false)
}

pub(crate) fn descriptor_location_lifetime_with_contents(
    steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    birth: DescriptorPosition,
    descriptor: &DescriptorLocation,
    needs_contents: impl Fn(DescriptorPosition, &DescriptorLocation) -> bool,
) -> DescriptorLifetime {
    let demand =
        location_live_positions(steps, birth, descriptor, false, true, None, needs_contents);
    let authorized = authorized_locations(steps, birth, descriptor, &demand.live);
    let surviving = if authorized.is_empty() {
        BTreeMap::new()
    } else {
        transported_locations(steps, birth, descriptor, false, false, true, None)
    };
    DescriptorLifetime {
        live: demand.live,
        demanded: demand.names,
        authorized,
        surviving,
    }
}

/// A new edge borrow starts under the selected successor name. Its
/// lifetime follows the same copies and captures as an explicit reference.
pub fn descriptor_successor_lifetime(
    steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    birth: DescriptorPosition,
    target: DescriptorPosition,
    descriptor: &DescriptorLocation,
) -> Option<DescriptorLifetime> {
    let mut matching = steps
        .get(&birth)?
        .successors
        .iter()
        .filter(|edge| edge.target == target);
    let selected = matching.next()?;
    if matching.next().is_some()
        || !selected.bindings.iter().any(|binding| {
            binding.parameter == *descriptor
                && binding.access != terminal_psi::StructuralAccess::Owned
        })
    {
        return None;
    }
    let demand = location_live_positions(
        steps,
        birth,
        descriptor,
        false,
        true,
        Some(target),
        |_, _| false,
    );
    let authorized =
        transported_locations(steps, birth, descriptor, false, false, false, Some(target));
    let surviving =
        transported_locations(steps, birth, descriptor, false, false, true, Some(target));
    Some(DescriptorLifetime {
        live: demand.live,
        demanded: demand.names,
        authorized,
        surviving,
    })
}

fn authorized_locations(
    steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    birth: DescriptorPosition,
    descriptor: &DescriptorLocation,
    live: &BTreeSet<DescriptorPosition>,
) -> BTreeMap<DescriptorPosition, BTreeSet<DescriptorLocation>> {
    let Some(formation) = steps.get(&birth) else {
        return BTreeMap::new();
    };
    if !formation
        .definitions
        .iter()
        .any(|definition| definition.contains(descriptor))
    {
        return BTreeMap::new();
    }
    if live.contains(&birth) {
        return BTreeMap::new();
    }
    transported_locations(steps, birth, descriptor, false, false, false, None)
}

pub(crate) fn value_locations(
    steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    establishment: DescriptorPosition,
    value: &DescriptorLocation,
    at_entry: bool,
) -> BTreeMap<DescriptorPosition, BTreeSet<DescriptorLocation>> {
    if !at_entry {
        let live = descriptor_location_live_positions(steps, establishment, value);
        if live.contains(&establishment)
            || !steps.get(&establishment).is_some_and(|step| {
                step.definitions
                    .iter()
                    .any(|definition| definition.contains(value))
            })
        {
            return BTreeMap::new();
        }
    }
    transported_locations(steps, establishment, value, at_entry, true, false, None)
}

pub type LoanChoiceLocation = (DescriptorPosition, DescriptorLocation);

/// The descriptor carries exactly one loan on each predecessor. A node
/// keeps the edge that selected its parent even after later moves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescriptorLoanChoice {
    pub root: LoanChoiceLocation,
    pub nodes: BTreeMap<LoanChoiceLocation, LoanChoice>,
}

impl DescriptorLoanChoice {
    pub fn loans(&self) -> impl Iterator<Item = ObligationId> + '_ {
        self.nodes.values().filter_map(|node| match node {
            LoanChoice::Loan(parent) => Some(*parent),
            LoanChoice::Arrivals(_) => None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoanChoice {
    Loan(ObligationId),
    Arrivals(Vec<LoanChoiceArrival>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoanChoiceArrival {
    /// Internal steps and genuine Terminal edges use the same flow.
    /// A genuine edge keeps its identity even if another edge has the
    /// same starting position and target.
    pub edge: Option<EdgeId>,
    pub source: LoanChoiceLocation,
}

pub(crate) fn joined_loan_authority(
    steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    location: LoanChoiceLocation,
    candidates: &[(ObligationId, &DescriptorLifetime)],
) -> Option<DescriptorLoanChoice> {
    let mut predecessors =
        BTreeMap::<DescriptorPosition, Vec<(DescriptorPosition, &DescriptorFlowEdge)>>::new();
    for (position, step) in steps {
        for edge in &step.successors {
            predecessors
                .entry(edge.target)
                .or_default()
                .push((*position, edge));
        }
    }

    let mut without_choice = BTreeSet::new();
    let mut pending = vec![(location.clone(), false)];
    let mut nodes = BTreeMap::new();
    while let Some((requested, leaving)) = pending.pop() {
        if leaving {
            without_choice.remove(&requested);
            continue;
        }
        if without_choice.contains(&requested) {
            // A cycle without a known loan at its header does not
            // establish that every arrival owns the descriptor.
            return None;
        }
        if nodes.contains_key(&requested) {
            continue;
        }

        let (at, descriptor) = &requested;
        let mut reaching = candidates.iter().filter(|(_, lifetime)| {
            lifetime
                .surviving
                .get(at)
                .is_some_and(|names| names.contains(descriptor))
        });
        let (identity, lifetime) = reaching.next()?;
        if reaching.next().is_none()
            && lifetime.live.contains(at)
            && lifetime
                .authorized
                .get(at)
                .is_some_and(|names| names.contains(descriptor))
        {
            nodes.insert(requested, LoanChoice::Loan(*identity));
            continue;
        }

        let mut arrivals = Vec::new();
        for &(from, edge) in predecessors.get(at)? {
            let names = candidates
                .iter()
                .filter_map(|(_, lifetime)| lifetime.surviving.get(&from))
                .flatten()
                .collect::<BTreeSet<_>>();
            let mut matching = names.into_iter().filter(|source| {
                let before = BTreeSet::from([(*source).clone()]);
                let after = steps[&from].transport_authority(&before, false);
                edge.transport_locations(&after, false, false)
                    .contains(descriptor)
            });
            let source = matching.next()?.clone();
            if matching.next().is_some() {
                return None;
            }
            arrivals.push(LoanChoiceArrival {
                edge: edge.edge,
                source: (from, source),
            });
        }
        if arrivals.is_empty() {
            return None;
        }
        without_choice.insert(requested.clone());
        pending.push((requested.clone(), true));
        pending.extend(
            arrivals
                .iter()
                .map(|arrival| (arrival.source.clone(), false)),
        );
        nodes.insert(requested, LoanChoice::Arrivals(arrivals));
    }
    Some(DescriptorLoanChoice {
        root: location,
        nodes,
    })
}

fn transported_locations(
    steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    birth: DescriptorPosition,
    descriptor: &DescriptorLocation,
    at_entry: bool,
    invalidate_containers: bool,
    may: bool,
    arrival: Option<DescriptorPosition>,
) -> BTreeMap<DescriptorPosition, BTreeSet<DescriptorLocation>> {
    let mut predecessors =
        BTreeMap::<DescriptorPosition, Vec<(DescriptorPosition, &DescriptorFlowEdge)>>::new();
    for (position, step) in steps {
        for edge in &step.successors {
            predecessors
                .entry(edge.target)
                .or_default()
                .push((*position, edge));
        }
    }
    let mut established = if may {
        steps
            .keys()
            .map(|position| (*position, BTreeSet::new()))
            .collect::<BTreeMap<_, _>>()
    } else {
        // Must transport descends from the possible names at each cut.
        // Starting at empty loses unchanged authority at every loop join.
        // The possible names come only from this genuine formation, so
        // disconnected cycles gain nothing. Empty incoming paths still
        // participate in the intersection below; possibility never
        // substitutes for a path's retained authority.
        transported_locations(
            steps,
            birth,
            descriptor,
            at_entry,
            invalidate_containers,
            true,
            arrival,
        )
    };
    if at_entry {
        established.insert(birth, BTreeSet::from([descriptor.clone()]));
    }
    loop {
        let mut changed = false;
        for position in steps.keys().copied() {
            if position == birth {
                continue;
            }
            let mut incoming =
                predecessors
                    .get(&position)
                    .into_iter()
                    .flatten()
                    .map(|(predecessor, edge)| {
                        let step = &steps[predecessor];
                        let before = &established[predecessor];
                        let mut after = transport_authority(
                            before,
                            &step.definitions,
                            step.authority_derivations
                                .iter()
                                .map(|(source, destination)| (source, destination)),
                            invalidate_containers,
                            may,
                        );
                        if !at_entry && arrival.is_none() && *predecessor == birth {
                            after.insert(descriptor.clone());
                        }
                        let mut transported =
                            edge.transport_locations(&after, invalidate_containers, may);
                        if arrival == Some(position) && *predecessor == birth {
                            transported.insert(descriptor.clone());
                        }
                        transported
                    });
            let mut current = incoming.next().unwrap_or_default();
            for other in incoming {
                if may {
                    current.extend(other);
                } else {
                    current.retain(|location| other.contains(location));
                }
            }
            if established.get(&position) != Some(&current) {
                established.insert(position, current);
                changed = true;
            }
        }
        if !changed {
            return established;
        }
    }
}

fn transport_authority<'locations>(
    before: &BTreeSet<DescriptorLocation>,
    definitions: &[DescriptorLocation],
    derivations: impl Clone
    + Iterator<
        Item = (
            &'locations DescriptorLocation,
            &'locations DescriptorLocation,
        ),
    >,
    invalidate_containers: bool,
    may: bool,
) -> BTreeSet<DescriptorLocation> {
    let mut after = before
        .iter()
        .filter(|location| {
            !definitions.iter().any(|definition| {
                definition.contains(location)
                    || (invalidate_containers && location.contains(definition))
            })
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    let candidates = before
        .iter()
        .flat_map(|location| {
            derivations
                .clone()
                .filter_map(move |(source, destination)| {
                    location.transported(source, destination)
                })
        })
        .collect::<BTreeSet<_>>();
    for candidate in candidates {
        let sources = derivations
            .clone()
            .filter_map(|(source, destination)| candidate.transported(destination, source))
            .collect::<Vec<_>>();
        if !sources.is_empty()
            && if may {
                sources.iter().any(|source| before.contains(source))
            } else {
                sources.iter().all(|source| before.contains(source))
            }
        {
            after.insert(candidate);
        }
    }
    after
}

#[cfg(test)]
mod resource_tests;
