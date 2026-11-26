use semantic_vocabulary::{EdgeId, ObligationId};
use std::collections::{BTreeMap, BTreeSet};
pub use terminal_psi::types::descriptor_locations::{
    DescriptorLocation, DescriptorPathSegment, DescriptorPosition,
};

pub(crate) mod arrivals;
pub use arrivals::{
    DescriptorContentsQuery, DescriptorContentsStep, DescriptorFlowBinding, DescriptorFlowSource,
};
pub(crate) mod locations;
pub(crate) mod residences;
pub use locations::{
    StructuralTypeIndex, canonical_descriptor_location, descriptor_location,
    descriptor_location_type, descriptor_path, fixed_runtime_index_alternatives,
    view_element_descriptor_prefix,
};
pub use residences::{DescriptorResidenceRules, descriptor_residence_live_positions};

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
    /// A view formation taking a new loan of an existing view's payload.
    pub reborrows_view: bool,
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
    pub fn replace_value_inputs(&mut self, inputs: impl IntoIterator<Item = DescriptorLocation>) {
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
                (name.root == source.root && name.path == prefix).then(|| binding.parameter.clone())
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
    pub(crate) fn is_plain_fallthrough(&self) -> bool {
        self.parameters.is_empty()
            && self.definitions.is_empty()
            && self.effects.is_empty()
            && self.bindings.is_empty()
    }

    pub(crate) fn transport_locations(
        &self,
        before: &BTreeSet<DescriptorLocation>,
        invalidate_containers: bool,
        may: bool,
    ) -> BTreeSet<DescriptorLocation> {
        if self.is_plain_fallthrough() {
            return before.clone();
        }
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
    location_live_positions(
        steps,
        birth,
        descriptor,
        DescriptorStart::AfterDefinition,
        false,
        None,
        |_, _| false,
    )
    .live
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DescriptorStart {
    AtEntry,
    AfterDefinition,
    OnEdgeTo(DescriptorPosition),
}

type LocationsByPosition = BTreeMap<DescriptorPosition, BTreeSet<DescriptorLocation>>;

struct DescriptorDemand {
    escaped: bool,
    live: BTreeSet<DescriptorPosition>,
    names: BTreeMap<DescriptorPosition, BTreeSet<DescriptorLocation>>,
}

/// Exact per-place dead-tail cutoff for `reference_ends::candidates`.
/// When `cutoff[place] == Some((block, stop_ordinal))`, `place` is never
/// touched again after `stop_ordinal - 1`, never escapes, is never borrowed
/// from, and the step from `stop_ordinal - 1` to `stop_ordinal` is a
/// single-predecessor plain fallthrough in an acyclic block graph. Neither
/// liveness nor reference-end eligibility can change past `(block, stop_ordinal)`,
/// so walks can stop propagating `place` there.
pub struct DeadTailCutoff {
    acyclic: bool,
    stops: Vec<Option<DescriptorPosition>>,
}

impl DeadTailCutoff {
    pub(crate) fn new(acyclic: bool, stops: Vec<Option<DescriptorPosition>>) -> Self {
        Self { acyclic, stops }
    }

    pub(crate) fn is_acyclic(&self) -> bool {
        self.acyclic
    }

    pub(crate) fn stops_after(
        &self,
        place: semantic_vocabulary::PlaceId,
        at: DescriptorPosition,
    ) -> bool {
        let Some(idx) = usize::try_from(place.get() - 1).ok() else {
            return false;
        };
        let Some(Some((block, stop_ordinal))) = self.stops.get(idx).copied() else {
            return false;
        };
        at.0 == block && at.1 >= stop_ordinal
    }
}

fn location_live_positions(
    steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    birth: DescriptorPosition,
    descriptor: &DescriptorLocation,
    start: DescriptorStart,
    borrowed_authority: bool,
    cutoff: Option<&DeadTailCutoff>,
    needs_contents: impl Fn(DescriptorPosition, &DescriptorLocation) -> bool,
) -> DescriptorDemand {
    type Occurrence = (DescriptorPosition, DescriptorLocation);
    let mut pending = Vec::new();
    match start {
        DescriptorStart::OnEdgeTo(target) => pending.push((target, descriptor.clone())),
        DescriptorStart::AtEntry => pending.push((birth, descriptor.clone())),
        DescriptorStart::AfterDefinition => {
            if let Some(step) = steps.get(&birth) {
                for edge in &step.successors {
                    pending.extend(
                        edge.forward(descriptor)
                            .into_iter()
                            .map(|name| (edge.target, name)),
                    );
                }
            }
        }
    }
    let mut visited = BTreeSet::new();
    let mut predecessors = BTreeMap::<Occurrence, Vec<Occurrence>>::new();
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
            if edge.is_plain_fallthrough() {
                for output in &outputs {
                    if cutoff.is_some_and(|c| c.stops_after(output.root, *position)) {
                        continue;
                    }
                    let next = (edge.target, output.clone());
                    predecessors
                        .entry(next.clone())
                        .or_default()
                        .push(occurrence.clone());
                    pending.push(next);
                }
                continue;
            }
            for output in &outputs {
                if cutoff.is_some_and(|c| c.stops_after(output.root, *position)) {
                    continue;
                }
                let mut destinations = if borrowed_authority {
                    edge.forward_authority(output)
                } else {
                    edge.forward(output)
                };
                destinations.extend(edge.bindings.iter().filter_map(|binding| {
                    let source = binding.indexed_borrow_source()?;
                    source.overlaps(output).then(|| binding.parameter.clone())
                }));
                for destination in destinations {
                    let next = (edge.target, destination);
                    predecessors
                        .entry(next.clone())
                        .or_default()
                        .push(occurrence.clone());
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
    let has_escape = !escaped.is_empty();
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
    DescriptorDemand {
        live,
        names,
        escaped: has_escape,
    }
}

pub struct DescriptorLifetime {
    /// At this formation the previous dynamic occurrence, when present,
    /// supplies the new one. The external lender still owns the lineage.
    pub(crate) recurring_authority: Option<Box<LocationsByPosition>>,
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

impl DescriptorLifetime {
    /// Every returning path lends the next view its last copy of the previous one.
    pub fn reborrows_previous(&self) -> bool {
        self.recurring_authority.is_some()
    }

    /// Permission on every path from this occurrence to the queried position.
    /// Arrivals that precede its formation have a separate lender.
    pub(crate) fn authorizes_after_formation(
        &self,
        at: DescriptorPosition,
        name: &DescriptorLocation,
    ) -> bool {
        self.recurring_authority
            .as_ref()
            .and_then(|permissions| permissions.get(&at))
            .is_some_and(|names| names.contains(name))
    }
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
    descriptor_lifetime(
        steps,
        entry,
        descriptor,
        DescriptorStart::AtEntry,
        needs_contents,
    )
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
    descriptor_lifetime(
        steps,
        birth,
        descriptor,
        DescriptorStart::AfterDefinition,
        needs_contents,
    )
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
    Some(descriptor_lifetime(
        steps,
        birth,
        descriptor,
        DescriptorStart::OnEdgeTo(target),
        |_, _| false,
    ))
}

pub struct FlowPredecessors<'a> {
    predecessors: BTreeMap<DescriptorPosition, Vec<(DescriptorPosition, &'a DescriptorFlowEdge)>>,
}

impl<'a> FlowPredecessors<'a> {
    pub fn new(steps: &'a BTreeMap<DescriptorPosition, DescriptorFlowStep>) -> Self {
        let mut predecessors =
            BTreeMap::<DescriptorPosition, Vec<(DescriptorPosition, &'a DescriptorFlowEdge)>>::new(
            );
        for (position, step) in steps {
            for edge in &step.successors {
                predecessors
                    .entry(edge.target)
                    .or_default()
                    .push((*position, edge));
            }
        }
        Self { predecessors }
    }

    pub(crate) fn count(&self, position: DescriptorPosition) -> usize {
        self.predecessors.get(&position).map_or(0, Vec::len)
    }
}

/// Same lifetime as `descriptor_location_lifetime` (including `surviving`
/// locations for join authority), reusing precomputed `predecessors` and
/// optional acyclic `cutoff` across every query on one machine.
/// `authorized` and `surviving` omit positions with no active locations.
pub fn descriptor_location_lifetime_indexed(
    steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    predecessors: &FlowPredecessors<'_>,
    cutoff: Option<&DeadTailCutoff>,
    birth: DescriptorPosition,
    descriptor: &DescriptorLocation,
) -> DescriptorLifetime {
    descriptor_lifetime_with_predecessors(
        steps,
        Some(predecessors),
        cutoff,
        birth,
        descriptor,
        DescriptorStart::AfterDefinition,
        true,
        false,
        |_, _| false,
    )
}

/// Same lifetime as `descriptor_successor_lifetime` (including `surviving`
/// locations for join authority), reusing precomputed `predecessors` and
/// optional acyclic `cutoff` across every query on one machine.
/// `authorized` and `surviving` omit positions with no active locations.
pub fn descriptor_successor_lifetime_indexed(
    steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    predecessors: &FlowPredecessors<'_>,
    cutoff: Option<&DeadTailCutoff>,
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
    Some(descriptor_lifetime_with_predecessors(
        steps,
        Some(predecessors),
        cutoff,
        birth,
        descriptor,
        DescriptorStart::OnEdgeTo(target),
        true,
        false,
        |_, _| false,
    ))
}

pub(crate) fn descriptor_entry_lifetime_with_contents_indexed(
    steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    predecessors: &FlowPredecessors<'_>,
    cutoff: Option<&DeadTailCutoff>,
    entry: DescriptorPosition,
    descriptor: &DescriptorLocation,
    needs_contents: impl Fn(DescriptorPosition, &DescriptorLocation) -> bool,
) -> DescriptorLifetime {
    descriptor_lifetime_with_predecessors(
        steps,
        Some(predecessors),
        cutoff,
        entry,
        descriptor,
        DescriptorStart::AtEntry,
        false,
        false,
        needs_contents,
    )
}

pub(crate) fn descriptor_location_lifetime_with_contents_indexed(
    steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    predecessors: &FlowPredecessors<'_>,
    cutoff: Option<&DeadTailCutoff>,
    birth: DescriptorPosition,
    descriptor: &DescriptorLocation,
    needs_contents: impl Fn(DescriptorPosition, &DescriptorLocation) -> bool,
) -> DescriptorLifetime {
    descriptor_lifetime_with_predecessors(
        steps,
        Some(predecessors),
        cutoff,
        birth,
        descriptor,
        DescriptorStart::AfterDefinition,
        false,
        false,
        needs_contents,
    )
}

pub(crate) fn descriptor_continuation_locations_indexed(
    steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    predecessors: &FlowPredecessors<'_>,
    same_block_max: Option<DescriptorPosition>,
    at: DescriptorPosition,
    descriptor: &DescriptorLocation,
) -> BTreeMap<DescriptorPosition, BTreeSet<DescriptorLocation>> {
    transported_locations_with_predecessors(
        steps,
        Some(predecessors),
        None,
        same_block_max,
        at,
        descriptor,
        DescriptorStart::AtEntry,
        false,
        false,
        None,
    )
}

fn descriptor_lifetime(
    steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    birth: DescriptorPosition,
    descriptor: &DescriptorLocation,
    start: DescriptorStart,
    needs_contents: impl Fn(DescriptorPosition, &DescriptorLocation) -> bool,
) -> DescriptorLifetime {
    descriptor_lifetime_with_predecessors(
        steps,
        None,
        None,
        birth,
        descriptor,
        start,
        true,
        true,
        needs_contents,
    )
}

#[allow(clippy::too_many_arguments)]
fn descriptor_lifetime_with_predecessors(
    steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    predecessors: Option<&FlowPredecessors<'_>>,
    cutoff: Option<&DeadTailCutoff>,
    birth: DescriptorPosition,
    descriptor: &DescriptorLocation,
    start: DescriptorStart,
    include_surviving: bool,
    pad_empty_positions: bool,
    needs_contents: impl Fn(DescriptorPosition, &DescriptorLocation) -> bool,
) -> DescriptorLifetime {
    let demand = location_live_positions(
        steps,
        birth,
        descriptor,
        start,
        true,
        cutoff,
        &needs_contents,
    );
    let recurring_authority = if start == DescriptorStart::AfterDefinition
        && demand.live.contains(&birth)
        && !demand.escaped
    {
        final_view_reborrow(steps, birth, descriptor, &demand, cutoff, &needs_contents)
            .map(Box::new)
    } else {
        None
    };
    let mut surviving = BTreeMap::new();
    let authorized = if start == DescriptorStart::AfterDefinition
        && recurring_authority.is_none()
        && !is_fresh_definition(steps, birth, descriptor, &demand.live)
    {
        BTreeMap::new()
    } else {
        transported_locations_with_predecessors(
            steps,
            predecessors,
            cutoff,
            None,
            birth,
            descriptor,
            start,
            false,
            pad_empty_positions,
            include_surviving.then_some(&mut surviving),
        )
    };
    DescriptorLifetime {
        recurring_authority,
        live: demand.live,
        demanded: demand.names,
        authorized,
        surviving,
    }
}

/// A previous occurrence can hand its permission to the next only when
/// the view being narrowed is its last demanded carrier at the formation.
/// Check its continuation separately, without seeding the new result.
fn final_view_reborrow(
    steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    birth: DescriptorPosition,
    result: &DescriptorLocation,
    demand: &DescriptorDemand,
    cutoff: Option<&DeadTailCutoff>,
    needs_contents: impl Fn(DescriptorPosition, &DescriptorLocation) -> bool,
) -> Option<LocationsByPosition> {
    let Some(formation) = steps.get(&birth) else {
        return None;
    };
    let [(source, child)] = formation.derivations.as_slice() else {
        return None;
    };
    if !formation.reborrows_view
        || child != result
        || source.overlaps(result)
        || !formation.definitions.contains(result)
        || !demand
            .names
            .get(&birth)
            .is_some_and(|names| names.len() == 1 && names.contains(source))
    {
        return None;
    }
    if !location_live_positions(
        steps,
        birth,
        source,
        DescriptorStart::AfterDefinition,
        true,
        cutoff,
        needs_contents,
    )
    .live
    .is_empty()
    {
        return None;
    }

    returns_with_previous_authority(steps, birth, source, result, cutoff)
}

/// Another reborrow can demand the same backing while holding a different
/// immediate loan. Follow permission transfers from this result on every
/// path back to its formation. Initial arrivals have no preceding result.
fn returns_with_previous_authority(
    steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    birth: DescriptorPosition,
    source: &DescriptorLocation,
    result: &DescriptorLocation,
    cutoff: Option<&DeadTailCutoff>,
) -> Option<LocationsByPosition> {
    let mut returns = FlowPredecessors {
        predecessors: BTreeMap::new(),
    };
    let mut visited = BTreeSet::new();
    let mut pending = vec![birth];
    while let Some(position) = pending.pop() {
        if !visited.insert(position) {
            continue;
        }
        let Some(step) = steps.get(&position) else {
            return None;
        };
        for edge in &step.successors {
            returns
                .predecessors
                .entry(edge.target)
                .or_default()
                .push((position, edge));
            pending.push(edge.target);
        }
    }
    let Some(arrivals) = returns.predecessors.get(&birth) else {
        return None;
    };
    let mut permissions = transported_locations_with_predecessors(
        steps,
        Some(&returns),
        cutoff,
        None,
        birth,
        result,
        DescriptorStart::AfterDefinition,
        false,
        false,
        None,
    );
    let empty = BTreeSet::new();
    let exact = arrivals.iter().all(|&(from, edge)| {
        let before = permissions.get(&from).unwrap_or(&empty);
        let mut after = steps[&from].transport_authority(before, false);
        if from == birth {
            after.insert(result.clone());
        }
        edge.transport_locations(&after, false, false)
            .contains(source)
    });
    if !exact {
        return None;
    }
    permissions.insert(birth, BTreeSet::from([source.clone()]));
    Some(permissions)
}

fn is_fresh_definition(
    steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    birth: DescriptorPosition,
    descriptor: &DescriptorLocation,
    live: &BTreeSet<DescriptorPosition>,
) -> bool {
    !live.contains(&birth)
        && steps.get(&birth).is_some_and(|step| {
            step.definitions
                .iter()
                .any(|definition| definition.contains(descriptor))
        })
}

pub(crate) fn value_locations(
    steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    establishment: DescriptorPosition,
    value: &DescriptorLocation,
    at_entry: bool,
) -> BTreeMap<DescriptorPosition, BTreeSet<DescriptorLocation>> {
    let start = if at_entry {
        DescriptorStart::AtEntry
    } else {
        let live = descriptor_location_live_positions(steps, establishment, value);
        if !is_fresh_definition(steps, establishment, value, &live) {
            return BTreeMap::new();
        }
        DescriptorStart::AfterDefinition
    };
    transported_locations(steps, establishment, value, start, true, None)
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
    pub fn includes_incoming(&self) -> bool {
        self.nodes
            .values()
            .any(|node| matches!(node, LoanChoice::Incoming(_)))
    }

    pub fn loans(&self) -> impl Iterator<Item = ObligationId> + '_ {
        self.nodes.values().filter_map(|node| match node {
            LoanChoice::Loan(parent) => Some(*parent),
            LoanChoice::Incoming(_) | LoanChoice::Arrivals(_) => None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoanChoice {
    /// Permission carried from an original incoming view on this arrival.
    Incoming(DescriptorLocation),
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
    incoming: Option<(&DescriptorLocation, &DescriptorLifetime)>,
) -> Option<DescriptorLoanChoice> {
    let sources = || {
        candidates
            .iter()
            .map(|(id, life)| (LoanChoice::Loan(*id), *life))
            .chain(incoming.map(|(input, life)| (LoanChoice::Incoming(input.clone()), life)))
    };
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
        let mut reaching = sources().filter(|(_, lifetime)| {
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
            nodes.insert(requested, identity);
            continue;
        }

        let mut arrivals = Vec::new();
        for &(from, edge) in predecessors.get(at)? {
            let names = sources()
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
    start: DescriptorStart,
    invalidate_containers: bool,
    surviving: Option<&mut LocationsByPosition>,
) -> LocationsByPosition {
    transported_locations_with_predecessors(
        steps,
        None,
        None,
        None,
        birth,
        descriptor,
        start,
        invalidate_containers,
        true,
        surviving,
    )
}

#[allow(clippy::too_many_arguments)]
fn transported_locations_with_predecessors(
    steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    predecessors: Option<&FlowPredecessors<'_>>,
    cutoff: Option<&DeadTailCutoff>,
    same_block_max: Option<DescriptorPosition>,
    birth: DescriptorPosition,
    descriptor: &DescriptorLocation,
    start: DescriptorStart,
    invalidate_containers: bool,
    pad_empty_positions: bool,
    surviving: Option<&mut LocationsByPosition>,
) -> LocationsByPosition {
    let owned_predecessors;
    let predecessors = match predecessors {
        Some(predecessors) => &predecessors.predecessors,
        None => {
            owned_predecessors = FlowPredecessors::new(steps);
            &owned_predecessors.predecessors
        }
    };
    let mut established: LocationsByPosition = BTreeMap::new();
    if start == DescriptorStart::AtEntry {
        established.insert(birth, BTreeSet::from([descriptor.clone()]));
    }
    let empty = BTreeSet::new();
    // Forward worklist for `may = true` (union): only positions reachable from
    // `birth` with non-empty sets can ever become non-empty, since `transport`
    // maps empty sets to empty sets.
    let mut worklist = BTreeSet::from([birth]);
    let mut needs_must_pass = false;
    while let Some(position) = worklist.pop_first() {
        let Some(step) = steps.get(&position) else {
            continue;
        };
        let before = established.get(&position).unwrap_or(&empty);
        if step.authority_derivations.len() > 1 {
            needs_must_pass = true;
        }
        let needs_birth_insert = start == DescriptorStart::AfterDefinition && position == birth;
        let mut after = if before.is_empty() {
            BTreeSet::new()
        } else if step.definitions.is_empty() && step.authority_derivations.is_empty() {
            before.clone()
        } else {
            transport_authority(
                before,
                &step.definitions,
                step.authority_derivations
                    .iter()
                    .map(|(source, destination)| (source, destination)),
                invalidate_containers,
                true,
            )
        };
        if needs_birth_insert {
            after.insert(descriptor.clone());
        }
        let after = &after;
        for edge in &step.successors {
            if edge.target == birth {
                continue;
            }
            if let Some((block, max_ord)) = same_block_max
                && (edge.target.0 != block || edge.target.1 > max_ord)
            {
                continue;
            }
            let edge_birth = start == DescriptorStart::OnEdgeTo(edge.target) && position == birth;
            if edge.is_plain_fallthrough() && !edge_birth {
                if after.is_empty() {
                    continue;
                }
                if let Some(cutoff) = cutoff {
                    let mut kept = after
                        .iter()
                        .filter(|item| !cutoff.stops_after(item.root, position))
                        .peekable();
                    if kept.peek().is_none() {
                        continue;
                    }
                    if predecessors
                        .get(&edge.target)
                        .is_some_and(|rows| rows.len() > 1)
                    {
                        needs_must_pass = true;
                    }
                    let target_set = established.entry(edge.target).or_default();
                    let before_len = target_set.len();
                    for item in kept {
                        target_set.insert(item.clone());
                    }
                    if target_set.len() != before_len {
                        worklist.insert(edge.target);
                    }
                    continue;
                }
                if predecessors
                    .get(&edge.target)
                    .is_some_and(|rows| rows.len() > 1)
                {
                    needs_must_pass = true;
                }
                let target_set = established.entry(edge.target).or_default();
                let before_len = target_set.len();
                for item in after {
                    target_set.insert(item.clone());
                }
                if target_set.len() != before_len {
                    worklist.insert(edge.target);
                }
                continue;
            }
            if !edge.effects.is_empty() || edge.bindings.len() > 1 {
                needs_must_pass = true;
            }
            let mut transported = if after.is_empty() {
                BTreeSet::new()
            } else if let Some(cutoff) = cutoff {
                let filtered: BTreeSet<_> = after
                    .iter()
                    .filter(|item| !cutoff.stops_after(item.root, position))
                    .cloned()
                    .collect();
                if filtered.is_empty() {
                    BTreeSet::new()
                } else {
                    edge.transport_locations(&filtered, invalidate_containers, true)
                }
            } else {
                edge.transport_locations(after, invalidate_containers, true)
            };
            if edge_birth {
                transported.insert(descriptor.clone());
            }
            if transported.is_empty() {
                continue;
            }
            if predecessors
                .get(&edge.target)
                .is_some_and(|rows| rows.len() > 1)
            {
                needs_must_pass = true;
            }
            let target_set = established.entry(edge.target).or_default();
            let before_len = target_set.len();
            target_set.extend(transported);
            if target_set.len() != before_len {
                worklist.insert(edge.target);
            }
        }
    }
    if let Some(surviving) = surviving {
        surviving.clone_from(&established);
        if pad_empty_positions {
            for &position in steps.keys() {
                surviving.entry(position).or_default();
            }
        }
    }
    // Start with every name this borrow can reach, then remove names missing
    // on any arrival. When no reached node has multiple predecessors or
    // multi-source derivations, `must` is identical to `may`.
    if needs_must_pass {
        let mut shrink_worklist: BTreeSet<DescriptorPosition> = established
            .keys()
            .copied()
            .filter(|&position| position != birth)
            .collect();
        while let Some(position) = shrink_worklist.pop_first() {
            let mut incoming =
                predecessors
                    .get(&position)
                    .into_iter()
                    .flatten()
                    .map(|(predecessor, edge)| {
                        let step = &steps[predecessor];
                        let before = established.get(predecessor).unwrap_or(&empty);
                        let mut after = if before.is_empty() {
                            BTreeSet::new()
                        } else {
                            transport_authority(
                                before,
                                &step.definitions,
                                step.authority_derivations
                                    .iter()
                                    .map(|(source, destination)| (source, destination)),
                                invalidate_containers,
                                false,
                            )
                        };
                        if start == DescriptorStart::AfterDefinition && *predecessor == birth {
                            after.insert(descriptor.clone());
                        }
                        let mut transported = if after.is_empty() {
                            BTreeSet::new()
                        } else if let Some(cutoff) = cutoff {
                            let filtered: BTreeSet<_> = after
                                .iter()
                                .filter(|item| !cutoff.stops_after(item.root, *predecessor))
                                .cloned()
                                .collect();
                            if filtered.is_empty() {
                                BTreeSet::new()
                            } else {
                                edge.transport_locations(&filtered, invalidate_containers, false)
                            }
                        } else {
                            edge.transport_locations(&after, invalidate_containers, false)
                        };
                        if start == DescriptorStart::OnEdgeTo(position) && *predecessor == birth {
                            transported.insert(descriptor.clone());
                        }
                        transported
                    });
            let mut current = incoming.next().unwrap_or_default();
            for other in incoming {
                current.retain(|location| other.contains(location));
            }
            if established.get(&position) != Some(&current) {
                established.insert(position, current);
                if let Some(step) = steps.get(&position) {
                    for edge in &step.successors {
                        if edge.target != birth && established.contains_key(&edge.target) {
                            shrink_worklist.insert(edge.target);
                        }
                    }
                }
            }
        }
    }
    if pad_empty_positions {
        for &position in steps.keys() {
            established.entry(position).or_default();
        }
    } else {
        established.retain(|_, locations| !locations.is_empty());
    }
    established
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
                .filter_map(move |(source, destination)| location.transported(source, destination))
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
