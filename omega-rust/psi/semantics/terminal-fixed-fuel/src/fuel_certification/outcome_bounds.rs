//! Outcome-sensitive machine and control-flow composition.

use crate::{FixedFuelError, UnboundedCycleCause};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, EdgeId, IntegerType, IntegerValue, MachineId, OperationId,
    Proposition, ScalarTerm, ScalarType, ValueId,
};
use std::collections::{BTreeMap, BTreeSet};
use terminal_fuel::TerminalFuelSchedule;
use terminal_psi::{
    OperationKind, TerminalAffineCleanupAction, TerminalMachine, TerminalModule,
    TerminalNaturalCycle, TerminalRankedScc, Terminator,
};

#[cfg(test)]
thread_local! {
    pub(super) static MACHINE_DERIVATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub(super) fn derive_maximum_entry_bound(
    module: &TerminalModule,
    machine: MachineId,
) -> Result<u64, FixedFuelError> {
    let schedule = TerminalFuelSchedule::CURRENT;
    let machines = module
        .machines
        .iter()
        .map(|machine| (machine.id, machine))
        .collect::<BTreeMap<_, _>>();
    let dynamic_call_targets = dynamic_call_targets(module);
    let provider_candidates = boundary_call_candidates(module);
    maximum_machine_outcomes(
        machine,
        &machines,
        &dynamic_call_targets,
        &provider_candidates,
        schedule,
        &mut BTreeMap::new(),
        &mut BTreeSet::new(),
    )?
    .maximum()
    .ok_or(FixedFuelError::NoTerminalPath(machine))
}

/// Every descriptor-dispatched call's verified realization, keyed by its
/// call operation. `CallDynamicScalar` and `CallDynamicUnit` resolve through
/// exactly one indirect or stored dispatch row; both lanes carry the same
/// coordinates the interpreter joins on, so the merged map mirrors runtime
/// resolution.
pub(super) fn dynamic_call_targets(
    module: &TerminalModule,
) -> BTreeMap<(MachineId, OperationId), MachineId> {
    module
        .dynamic_dispatch
        .indirect_dispatches
        .iter()
        .map(|dispatch| ((dispatch.owner, dispatch.operation), dispatch.realization))
        .chain(
            module
                .dynamic_dispatch
                .stored_dispatches
                .iter()
                .map(|dispatch| ((dispatch.owner, dispatch.operation), dispatch.realization)),
        )
        .collect()
}

/// Every checked provider an installation may select for a boundary
/// requirement. A boundary call dispatches to whichever candidate the
/// admitted installation binds, so bound composition takes the maximum over
/// the whole candidate list rather than naming one.
pub(super) fn boundary_call_candidates(
    module: &TerminalModule,
) -> BTreeMap<BoundaryMachineId, Vec<MachineId>> {
    let mut candidates = BTreeMap::<BoundaryMachineId, Vec<MachineId>>::new();
    for candidate in &module.provider_candidates {
        candidates
            .entry(candidate.boundary)
            .or_default()
            .push(candidate.candidate);
    }
    candidates
}

/// Every in-module machine one admitted operation can dispatch to.
///
/// Direct calls name their callee exactly. `CallDynamicScalar` and
/// `CallDynamicUnit` resolve through the module's indirect or stored
/// dispatch row; a missing row means the semantic invariant is broken, so
/// the bound fails closed. A `BoundaryCall` can dispatch to any checked
/// provider candidate, or to an external handler when the module retains
/// none — an external completion charges no in-module callee work. A
/// dynamic-parameter call receives its callee from the invocation's
/// descriptor table; that open callee set has no fixed ceiling, so the
/// certificate rejects rather than under-approximate.
pub(super) fn operation_callees(
    owner: MachineId,
    operation: &terminal_psi::Operation,
    dynamic_call_targets: &BTreeMap<(MachineId, OperationId), MachineId>,
    provider_candidates: &BTreeMap<BoundaryMachineId, Vec<MachineId>>,
) -> Result<Vec<MachineId>, FixedFuelError> {
    match &operation.kind {
        OperationKind::Call { callee, .. }
        | OperationKind::CallUnit { callee, .. }
        | OperationKind::CallStructuralScalar { callee, .. }
        | OperationKind::CallStructural { callee, .. }
        | OperationKind::CallStructuralWithScalarArguments { callee, .. } => Ok(vec![*callee]),
        OperationKind::CallDynamicScalar { .. } | OperationKind::CallDynamicUnit { .. } => {
            dynamic_call_targets
                .get(&(owner, operation.id))
                .map(|realization| vec![*realization])
                .ok_or(FixedFuelError::MissingDynamicDispatch {
                    owner,
                    operation: operation.id,
                })
        }
        OperationKind::CallDynamicParameterScalar { .. }
        | OperationKind::CallDynamicParameterUnit { .. } => {
            Err(FixedFuelError::InvocationBoundCallee {
                owner,
                operation: operation.id,
            })
        }
        OperationKind::BoundaryCall { boundary, .. } => Ok(provider_candidates
            .get(boundary)
            .cloned()
            .unwrap_or_default()),
        _ => Ok(Vec::new()),
    }
}

/// Per-outcome machine bounds. `returned` bounds every normal-return walk and
/// `crashed` bounds every crash-terminal walk; each is `None` when no walk of
/// its outcome exists, so callers composing one outcome never inherit work
/// from a path the callee cannot take.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct OutcomeBounds {
    pub(super) returned: Option<u64>,
    pub(super) crashed: Option<u64>,
}

impl OutcomeBounds {
    pub(super) fn maximum(self) -> Option<u64> {
        match (self.returned, self.crashed) {
            (Some(returned), Some(crashed)) => Some(returned.max(crashed)),
            (Some(units), None) | (None, Some(units)) => Some(units),
            (None, None) => None,
        }
    }

    fn with_prefix(self, prefix: u64) -> Result<Self, FixedFuelError> {
        Ok(Self {
            returned: checked_optional_add(self.returned, prefix)?,
            crashed: checked_optional_add(self.crashed, prefix)?,
        })
    }

    fn merge(self, other: Self) -> Self {
        Self {
            returned: maximum_optional(self.returned, other.returned),
            crashed: maximum_optional(self.crashed, other.crashed),
        }
    }
}

pub(super) fn maximum_optional(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn checked_optional_add(value: Option<u64>, added: u64) -> Result<Option<u64>, FixedFuelError> {
    value
        .map(|value| {
            value
                .checked_add(added)
                .ok_or(FixedFuelError::BoundOverflow)
        })
        .transpose()
}

pub(super) fn maximum_machine_outcomes(
    machine: MachineId,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    dynamic_call_targets: &BTreeMap<(MachineId, OperationId), MachineId>,
    provider_candidates: &BTreeMap<BoundaryMachineId, Vec<MachineId>>,
    schedule: TerminalFuelSchedule,
    memoized_machines: &mut BTreeMap<MachineId, OutcomeBounds>,
    active_machines: &mut BTreeSet<MachineId>,
) -> Result<OutcomeBounds, FixedFuelError> {
    if let Some(bounds) = memoized_machines.get(&machine) {
        return Ok(*bounds);
    }
    if !active_machines.insert(machine) {
        return Err(FixedFuelError::CallCycle(machine));
    }
    #[cfg(test)]
    MACHINE_DERIVATIONS.with(|count| count.set(count.get() + 1));
    let machine_semantics = machines
        .get(&machine)
        .copied()
        .ok_or(FixedFuelError::UnknownEntry(machine))?;
    let blocks = machine_semantics
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect::<BTreeMap<_, _>>();
    let result = match &machine_semantics.ranked_scc {
        Some(TerminalRankedScc::Natural(components)) => natural_machine_outcomes(
            machine_semantics,
            components,
            &blocks,
            machines,
            dynamic_call_targets,
            provider_candidates,
            schedule,
            memoized_machines,
            active_machines,
        ),
        _ => outcome_bounds_from(
            machine,
            machine_semantics.entry,
            &blocks,
            machines,
            dynamic_call_targets,
            provider_candidates,
            schedule,
            &mut BTreeMap::new(),
            &mut BTreeSet::new(),
            memoized_machines,
            active_machines,
        ),
    };
    result.inspect(|bounds| {
        active_machines.remove(&machine);
        memoized_machines.insert(machine, *bounds);
    })
}

/// Per-outcome entry bound for a verified `Natural`-ranked machine.
///
/// The verifier's components partition the machine's complete cyclic topology.
/// Within one component every cycle crosses a strict edge that decreases an
/// unsigned rank value, while preserving edges alone cannot close a cycle.
/// Each member block therefore executes at most `rank_maximum + 1` times per
/// component entry, and a component cannot be re-entered once left (an edge
/// back would make the outside block part of the same component). The
/// condensation of components and ordinary blocks is acyclic, so each outcome
/// class takes its own longest-path bound over it — the returned derivation
/// charges commit-reachable visits and reports `None` when no normal-return
/// walk exists, and the crashed derivation charges crash-terminal walks and
/// reports `None` when no execution can crash. The rank bound is the
/// carrier's type maximum unless the machine contract places a lower
/// literal ceiling on every rank arriving at the component's first entry —
/// `used_contract_premises` names the consulted clauses — and the
/// certificate's merged bound is the maximum of the two outcome classes
/// rather than a single ceiling billed against both.
fn natural_machine_outcomes(
    machine: &TerminalMachine,
    components: &[TerminalNaturalCycle],
    blocks: &BTreeMap<BlockId, &terminal_psi::Block>,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    dynamic_call_targets: &BTreeMap<(MachineId, OperationId), MachineId>,
    provider_candidates: &BTreeMap<BoundaryMachineId, Vec<MachineId>>,
    schedule: TerminalFuelSchedule,
    memoized_machines: &mut BTreeMap<MachineId, OutcomeBounds>,
    active_machines: &mut BTreeSet<MachineId>,
) -> Result<OutcomeBounds, FixedFuelError> {
    let geometry = natural_component_geometry(
        machine,
        components,
        machines,
        dynamic_call_targets,
        provider_candidates,
        schedule,
        memoized_machines,
        active_machines,
    )?;
    let entry_node = geometry.node_for(machine.entry);
    let returned = natural_condensed_bound_returned(
        entry_node,
        machine,
        components,
        &geometry,
        blocks,
        &mut BTreeMap::new(),
        &mut BTreeSet::new(),
    )?
    .map(|units| u64::try_from(units).map_err(|_| FixedFuelError::BoundOverflow))
    .transpose()?;
    let crashed = natural_condensed_bound_crashed(
        entry_node,
        machine,
        components,
        &geometry,
        blocks,
        &mut BTreeMap::new(),
        &mut BTreeSet::new(),
    )?
    .map(|units| u64::try_from(units).map_err(|_| FixedFuelError::BoundOverflow))
    .transpose()?;
    Ok(OutcomeBounds { returned, crashed })
}

/// Shared geometry of a verified `Natural` ranking: the component index each
/// cyclic block belongs to, and each block's per-outcome single-visit bound.
/// `visit_units_returned` is `None` when no walk through the block can
/// return and `visit_units_crashed` is `None` when no traversal of it can
/// crash. A component's per-outcome interior sums only the completing
/// members that can still reach the outcome's frontier — an exit edge or an
/// internally returning member for the returned lane, an exit edge or a
/// member whose traversal can crash for the crashed lane — through other
/// completing members. Members on a surviving completing cycle bill at the
/// rank carrier's type maximum plus one visits each; a member left off
/// every surviving cycle is crossed at most once, and a member that can
/// never reach the frontier is never on a walk of that outcome, so it
/// contributes nothing.
pub(super) struct NaturalGeometry {
    pub(super) member_of: BTreeMap<BlockId, usize>,
    pub(super) visit_units_returned: BTreeMap<BlockId, Option<u64>>,
    pub(super) visit_units_crashed: BTreeMap<BlockId, Option<u64>>,
    pub(super) component_units_returned: Vec<Option<u128>>,
    pub(super) component_units_crashed: Vec<Option<u128>>,
}

impl NaturalGeometry {
    /// The condensed node a block belongs to: its component when the block is
    /// ranked inside one, otherwise the block itself.
    pub(super) fn node_for(&self, block: BlockId) -> NaturalGraphNode {
        self.member_of
            .get(&block)
            .map_or(NaturalGraphNode::Block(block), |&index| {
                NaturalGraphNode::Component(index)
            })
    }
}

/// Compute the condensed geometry of a verified component partition:
/// membership is total and disjoint, and each block's per-outcome visit
/// bound composes its operations, admitted call outcomes, terminator, and
/// cleanup under that outcome's accounting. A block whose call or cleanup
/// can never return has no returning visit, one that can never crash has no
/// crashing visit, and each component derives one interior bound per
/// outcome over the completing members that can still reach that outcome's
/// frontier — the same discipline the acyclic derivation and the segment
/// partition already apply to `OutcomeBounds::returned`, extended here to
/// the crash outcome.
#[allow(clippy::too_many_arguments)]
pub(super) fn natural_component_geometry(
    machine: &TerminalMachine,
    components: &[TerminalNaturalCycle],
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    dynamic_call_targets: &BTreeMap<(MachineId, OperationId), MachineId>,
    provider_candidates: &BTreeMap<BoundaryMachineId, Vec<MachineId>>,
    schedule: TerminalFuelSchedule,
    memoized_machines: &mut BTreeMap<MachineId, OutcomeBounds>,
    active_machines: &mut BTreeSet<MachineId>,
) -> Result<NaturalGeometry, FixedFuelError> {
    let mut member_of = BTreeMap::new();
    for (index, component) in components.iter().enumerate() {
        for rank in &component.ranks {
            if member_of.insert(rank.block, index).is_some() {
                return Err(FixedFuelError::InvalidRankedScc(machine.id));
            }
        }
    }
    let mut visit_units_returned = BTreeMap::new();
    let mut visit_units_crashed = BTreeMap::new();
    for block in &machine.blocks {
        visit_units_returned.insert(
            block.id,
            block_return_visit_units(
                machine,
                block,
                machines,
                dynamic_call_targets,
                provider_candidates,
                schedule,
                memoized_machines,
                active_machines,
            )?,
        );
        visit_units_crashed.insert(
            block.id,
            block_crash_visit_units(
                machine,
                block,
                machines,
                dynamic_call_targets,
                provider_candidates,
                schedule,
                memoized_machines,
                active_machines,
            )?,
        );
    }
    let blocks: BTreeMap<BlockId, &terminal_psi::Block> = machine
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect();
    let mut component_units_returned = Vec::with_capacity(components.len());
    let mut component_units_crashed = Vec::with_capacity(components.len());
    for (index, component) in components.iter().enumerate() {
        let IntegerValue::Unsigned(rank_maximum) = component.rank_type.maximum_value() else {
            return Err(FixedFuelError::InvalidRankedScc(machine.id));
        };
        // A `requires` clause that caps every rank arriving at the
        // component's first entry tightens the visit bound below the
        // carrier maximum; the consulted clauses enter the certificate's
        // `relevant_preconditions` through `used_contract_premises`.
        let rank_bound = component_entry_rank_bound(machine, component, &blocks, rank_maximum)
            .map(|entry| entry.bound)
            .unwrap_or(rank_maximum);
        let interior = component_interior(
            component,
            index,
            &blocks,
            &member_of,
            &visit_units_returned,
            &visit_units_crashed,
            rank_bound,
        )?;
        component_units_returned.push(interior.returned);
        component_units_crashed.push(interior.crashed);
    }
    Ok(NaturalGeometry {
        member_of,
        visit_units_returned,
        visit_units_crashed,
        component_units_returned,
        component_units_crashed,
    })
}

/// One component's per-outcome interior bound. `returned` covers the
/// completing members a commit-reachable walk can still traverse — those
/// reaching an exit edge or an internally returning member through other
/// completing members — and `crashed` covers the completing members a
/// crash-terminal walk can still finish — those reaching an exit edge or a
/// member whose traversal can crash. `None` marks the absence of any such
/// walk's interior.
struct ComponentInterior {
    returned: Option<u128>,
    crashed: Option<u128>,
}

/// The completing-member interior one outcome lane can still bill. A
/// completing walk finishes visits only on members whose own traversal can
/// return, so those `live` members carry every billed visit; among them,
/// the billed set is the members that can still reach the outcome's
/// frontier. The returned lane's frontier is a member carrying an exit edge
/// or a return-family terminator; the crashed lane's is a member carrying
/// an exit edge plus every member whose traversal can crash — a crash ends
/// the walk inside it, so the crashable member itself bills only its
/// crashing visit while its completing predecessors stay in the interior.
/// A member reached only through a member that can never complete is never
/// visited by a walk of the outcome at all. The bound multiplies the
/// members a surviving completing cycle can still re-enter by the
/// component's entry-rank bound plus one — the carrier's type maximum, or
/// the lower literal ceiling a `requires` clause places on every rank
/// arriving at first entry — while a member left off every
/// surviving cycle is crossed at most once, rather than billing every live
/// member at the rank ceiling.
fn component_interior(
    component: &TerminalNaturalCycle,
    index: usize,
    blocks: &BTreeMap<BlockId, &terminal_psi::Block>,
    member_of: &BTreeMap<BlockId, usize>,
    visit_units_returned: &BTreeMap<BlockId, Option<u64>>,
    visit_units_crashed: &BTreeMap<BlockId, Option<u64>>,
    rank_bound: u128,
) -> Result<ComponentInterior, FixedFuelError> {
    let live: BTreeSet<BlockId> = component
        .ranks
        .iter()
        .map(|rank| rank.block)
        .filter(|member| matches!(visit_units_returned.get(member), Some(Some(_))))
        .collect();
    // Two internal adjacencies: `adjacency` keeps every member target a
    // completing member's edge can reach — a crash-terminal walk still
    // completes a member before crossing into the one whose visit ends it —
    // while `live_adjacency` keeps only completing targets, the edges a
    // member must cross to be re-entered or to reach a return frontier.
    let mut adjacency: BTreeMap<BlockId, Vec<BlockId>> = BTreeMap::new();
    let mut live_adjacency: BTreeMap<BlockId, Vec<BlockId>> = BTreeMap::new();
    let mut return_frontier = BTreeSet::new();
    let mut crash_frontier = BTreeSet::new();
    for &member in &live {
        let block = blocks
            .get(&member)
            .copied()
            .ok_or(FixedFuelError::UnknownBlock(member))?;
        let targets = terminator_targets(&block.terminator);
        adjacency.insert(
            member,
            targets
                .iter()
                .copied()
                .filter(|target| member_of.get(target) == Some(&index))
                .collect(),
        );
        live_adjacency.insert(
            member,
            targets
                .iter()
                .copied()
                .filter(|target| member_of.get(target) == Some(&index) && live.contains(target))
                .collect(),
        );
        // An exit-taking member can still leave the component on either
        // outcome lane; a member whose own terminator returns ends a
        // commit-reachable walk inside the component.
        if targets
            .iter()
            .any(|target| member_of.get(target) != Some(&index))
        {
            return_frontier.insert(member);
            crash_frontier.insert(member);
        }
        if matches!(
            block.terminator,
            Terminator::Return { .. }
                | Terminator::ReturnUnit { .. }
                | Terminator::ReturnUnitPartialAffine { .. }
                | Terminator::ReturnUnitNominalAffine { .. }
                | Terminator::ReturnStructural { .. }
        ) {
            return_frontier.insert(member);
        }
    }
    // A crash-terminal walk ends inside the first member whose traversal
    // crashes, so every crashable member is a crash frontier — including
    // the members a completing walk can never finish.
    for rank in &component.ranks {
        if matches!(visit_units_crashed.get(&rank.block), Some(Some(_))) {
            crash_frontier.insert(rank.block);
        }
    }
    Ok(ComponentInterior {
        returned: interior_visit_bound(
            &live_adjacency,
            &live_adjacency,
            &return_frontier,
            &live,
            visit_units_returned,
            rank_bound,
        )?,
        crashed: interior_visit_bound(
            &adjacency,
            &live_adjacency,
            &crash_frontier,
            &live,
            visit_units_returned,
            rank_bound,
        )?,
    })
}

/// The interior bound over the completing members that can still reach
/// `frontier`. `reach_adjacency` walks the edges that carry the outcome's
/// reachability — live-only for the returned lane, all member targets for
/// the crashed lane — while `cycle_adjacency` decides whether a billed
/// member can still be re-entered: only a cycle of completing visits
/// returns to it, so a member whose surviving successors cannot reach it
/// again is crossed at most once. `None` when no completing member reaches
/// the frontier at all.
fn interior_visit_bound(
    reach_adjacency: &BTreeMap<BlockId, Vec<BlockId>>,
    cycle_adjacency: &BTreeMap<BlockId, Vec<BlockId>>,
    frontier: &BTreeSet<BlockId>,
    live: &BTreeSet<BlockId>,
    visit_units: &BTreeMap<BlockId, Option<u64>>,
    rank_bound: u128,
) -> Result<Option<u128>, FixedFuelError> {
    let mut reaching: BTreeSet<BlockId> = frontier.iter().copied().collect();
    let mut pending: Vec<BlockId> = frontier.iter().copied().collect();
    while let Some(member) = pending.pop() {
        for (predecessor, targets) in reach_adjacency {
            if targets.contains(&member) && reaching.insert(*predecessor) {
                pending.push(*predecessor);
            }
        }
    }
    let billed: Vec<BlockId> = reaching.intersection(live).copied().collect();
    if billed.is_empty() {
        return Ok(None);
    }
    let mut reenterable_units = 0_u128;
    let mut once_units = 0_u128;
    for member in billed {
        // The member stays re-enterable when one of its surviving
        // completing successors reaches it again — an internal cycle the
        // adjacency left intact.
        let mut reenterable = false;
        let mut pending: Vec<BlockId> = cycle_adjacency
            .get(&member)
            .into_iter()
            .flatten()
            .copied()
            .collect();
        let mut seen = BTreeSet::new();
        while let Some(next) = pending.pop() {
            if next == member {
                reenterable = true;
                break;
            }
            if seen.insert(next) {
                pending.extend(cycle_adjacency.get(&next).into_iter().flatten().copied());
            }
        }
        let units = u128::from(visit_units.get(&member).copied().flatten().unwrap_or(0));
        if reenterable {
            reenterable_units = reenterable_units
                .checked_add(units)
                .ok_or(FixedFuelError::BoundOverflow)?;
        } else {
            once_units = once_units
                .checked_add(units)
                .ok_or(FixedFuelError::BoundOverflow)?;
        }
    }
    rank_bound
        .checked_add(1)
        .and_then(|visits| visits.checked_mul(reenterable_units))
        .and_then(|bound| bound.checked_add(once_units))
        .map(Some)
        .ok_or(FixedFuelError::BoundOverflow)
}

/// A `Natural` component's contract-derived ceiling on the rank observed at
/// first entry, together with the `machine.contract.requires` clause
/// positions that ceiling rests on.
pub(super) struct EntryRankBound {
    pub(super) bound: u128,
    pub(super) clauses: BTreeSet<usize>,
}

/// The contract ceiling on one component's initial rank, or `None` when the
/// carrier's type maximum must stand. The verifier discharges the machine's
/// `requires` propositions as assumptions, so every admitted invocation
/// satisfies them; a clause that caps the value arriving as a member's rank
/// observation therefore bounds the component's initial rank directly.
/// Every way control first enters the component must be covered: the
/// machine entry itself when it is a member — the entry block declares no
/// parameters, so only a machine-parameter observation can be bounded —
/// and each edge arriving from outside the component, whose arriving rank
/// is the argument at the target's rank-parameter position. An arrival
/// reduces to a boundable value only when it is a machine parameter some
/// `requires` clause caps by a literal; an argument threaded through
/// another block's parameters, a computed value, an observed view, or a
/// structural-case payload has no contract ceiling, so one unbounded
/// arrival leaves the carrier maximum in place rather than guessing. The
/// result is `Some` only when the derived ceiling genuinely tightens the
/// type maximum — a clause that merely restates it binds nothing new.
pub(super) fn component_entry_rank_bound(
    machine: &TerminalMachine,
    component: &TerminalNaturalCycle,
    blocks: &BTreeMap<BlockId, &terminal_psi::Block>,
    rank_maximum: u128,
) -> Option<EntryRankBound> {
    let member_rank: BTreeMap<BlockId, ValueId> = component
        .ranks
        .iter()
        .map(|rank| (rank.block, rank.value))
        .collect();
    let mut arrivals = Vec::new();
    if let Some(&rank) = member_rank.get(&machine.entry) {
        arrivals.push(rank);
    }
    for source in &machine.blocks {
        if member_rank.contains_key(&source.id) {
            continue;
        }
        for (target, arguments) in successor_arguments(&source.terminator) {
            let Some(&target_rank) = member_rank.get(&target) else {
                continue;
            };
            let target_block = blocks.get(&target)?;
            // The edge binds the target's parameters positionally, so the
            // rank arriving at a parameter-observed member is that edge's
            // argument at the rank's position; a non-parameter observation —
            // a machine parameter or an observed view — is not rebound by
            // the edge and reduces to the observed value itself, which the
            // machine-parameter check below then accepts or rejects.
            let arrival = match target_block
                .parameters
                .iter()
                .position(|parameter| parameter.id == target_rank)
            {
                Some(position) => *arguments?.get(position)?,
                None => target_rank,
            };
            arrivals.push(arrival);
        }
    }
    if arrivals.is_empty() {
        return None;
    }
    let mut bound = 0_u128;
    let mut clauses = BTreeSet::new();
    for arrival in arrivals {
        if !machine.parameters.iter().any(|parameter| {
            parameter.id == arrival
                && parameter.scalar_type == ScalarType::Integer(component.rank_type)
        }) {
            return None;
        }
        let (candidate, clause) = parameter_requires_bound(machine, arrival, component.rank_type)?;
        bound = bound.max(candidate);
        clauses.insert(clause);
    }
    (bound < rank_maximum).then_some(EntryRankBound { bound, clauses })
}

/// Each successor's target and the scalar argument list binding its
/// parameters positionally. A `StructuralCase` successor carries `None`:
/// its payload-field bindings cannot name a machine parameter directly.
fn successor_arguments(terminator: &Terminator) -> Vec<(BlockId, Option<&[ValueId]>)> {
    match terminator {
        Terminator::Jump {
            target, arguments, ..
        } => vec![(*target, Some(arguments.as_slice()))],
        Terminator::Conditional {
            when_true,
            when_false,
            ..
        } => vec![
            (when_true.target, Some(when_true.arguments.as_slice())),
            (when_false.target, Some(when_false.arguments.as_slice())),
        ],
        Terminator::StructuralCase { cases, .. } => {
            cases.iter().map(|case| (case.target, None)).collect()
        }
        Terminator::Return { .. }
        | Terminator::ReturnUnit { .. }
        | Terminator::ReturnUnitPartialAffine { .. }
        | Terminator::ReturnUnitNominalAffine { .. }
        | Terminator::ReturnStructural { .. }
        | Terminator::Crash { .. } => Vec::new(),
    }
}

/// The tightest literal ceiling the machine contract's `requires` clauses
/// place on `parameter`, paired with the clause index that supplied it.
/// Clauses flatten through `Conjunction` only: a disjunctive or implied
/// bound is not an unconditional ceiling on the parameter's value.
fn parameter_requires_bound(
    machine: &TerminalMachine,
    parameter: ValueId,
    rank_type: IntegerType,
) -> Option<(u128, usize)> {
    let mut best: Option<(u128, usize)> = None;
    for (index, clause) in machine.contract.requires.iter().enumerate() {
        let mut pending = vec![clause];
        while let Some(proposition) = pending.pop() {
            match proposition {
                Proposition::Conjunction(children) => pending.extend(children),
                _ => {
                    if let Some(candidate) =
                        literal_parameter_ceiling(proposition, parameter, rank_type)
                        && best.is_none_or(|(current, _)| candidate < current)
                    {
                        best = Some((candidate, index));
                    }
                }
            }
        }
    }
    best
}

/// The literal ceiling one proposition places on `parameter` — a direct
/// `p <= k`, `p < k`, or `p == k` over an unsigned literal of the rank
/// carrier's type. Anything else — another parameter or erased formal, a
/// math term, a field observation, or a signed literal — places no ceiling
/// the derivation can trust.
fn literal_parameter_ceiling(
    proposition: &Proposition,
    parameter: ValueId,
    rank_type: IntegerType,
) -> Option<u128> {
    let is_parameter = |term: &ScalarTerm| {
        matches!(
            term,
            ScalarTerm::Value { id, scalar_type }
                if *id == parameter && *scalar_type == ScalarType::Integer(rank_type)
        )
    };
    let literal = |term: &ScalarTerm| match term {
        ScalarTerm::Integer {
            scalar_type,
            value: IntegerValue::Unsigned(value),
        } if *scalar_type == rank_type => Some(*value),
        _ => None,
    };
    match proposition {
        Proposition::LessOrEqual(left, right) if is_parameter(left) => literal(right),
        Proposition::LessThan(left, right) if is_parameter(left) => {
            literal(right).and_then(|bound| bound.checked_sub(1))
        }
        Proposition::Equal(left, right) if is_parameter(left) => literal(right),
        Proposition::Equal(left, right) if is_parameter(right) => literal(left),
        _ => None,
    }
}

/// The machine-contract premises a whole-entry certificate's bound
/// derivation used: the `requires` clauses that tighten a reachable cyclic
/// component's entry rank below its carrier maximum. The condensed bound
/// only consumes the interiors of components reachable from the machine
/// entry, so an unreachable component's clauses stay out. Callee contracts
/// never appear here — their premises are discharged as ordinary call
/// obligations at each call site rather than becoming premises on the
/// entry machine's inputs. Clauses surface in canonical contract order.
pub(super) fn used_contract_premises(machine: &TerminalMachine) -> Vec<Proposition> {
    let Some(TerminalRankedScc::Natural(components)) = &machine.ranked_scc else {
        return Vec::new();
    };
    let blocks: BTreeMap<BlockId, &terminal_psi::Block> = machine
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect();
    let mut reachable = BTreeSet::from([machine.entry]);
    let mut pending = vec![machine.entry];
    while let Some(current) = pending.pop() {
        let Some(block) = blocks.get(&current) else {
            continue;
        };
        for target in terminator_targets(&block.terminator) {
            if reachable.insert(target) {
                pending.push(target);
            }
        }
    }
    let mut used = BTreeSet::new();
    for component in components {
        if !component
            .ranks
            .iter()
            .any(|rank| reachable.contains(&rank.block))
        {
            continue;
        }
        let IntegerValue::Unsigned(rank_maximum) = component.rank_type.maximum_value() else {
            continue;
        };
        if let Some(entry) = component_entry_rank_bound(machine, component, &blocks, rank_maximum) {
            used.extend(entry.clauses);
        }
    }
    used.into_iter()
        .filter_map(|index| machine.contract.requires.get(index).cloned())
        .collect()
}

/// One condensed node: an ordinary block or a complete cyclic component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum NaturalGraphNode {
    Block(BlockId),
    Component(usize),
}

/// The `returned` read of the condensed control DAG. A verified component
/// partition makes this graph acyclic; the active set is defense against
/// malformed input, not a search mechanism. Nodes charge the
/// commit-reachable visit units: an ordinary block contributes its
/// normal-return visit (`None` when no traversal through it can return), a
/// component contributes the interior its returning walks can still
/// traverse, and a component's exits are taken only through members whose
/// own visit can return — a member whose call always crashes still ends
/// every walk that reaches it, so its edges commit nothing a returning
/// walk can use. `None` propagates as "no
/// commit-reachable walk": a node whose successors all fail to reach a
/// return, or a trapped component with no returning member exit and no
/// returning member terminal edge, bounds nothing.
#[allow(clippy::too_many_arguments)]
fn natural_condensed_bound_returned(
    node: NaturalGraphNode,
    machine: &TerminalMachine,
    components: &[TerminalNaturalCycle],
    geometry: &NaturalGeometry,
    blocks: &BTreeMap<BlockId, &terminal_psi::Block>,
    memoized: &mut BTreeMap<NaturalGraphNode, Option<u128>>,
    active: &mut BTreeSet<NaturalGraphNode>,
) -> Result<Option<u128>, FixedFuelError> {
    if let Some(bound) = memoized.get(&node) {
        return Ok(*bound);
    }
    if !active.insert(node) {
        let cycle_block = match node {
            NaturalGraphNode::Block(block) => block,
            NaturalGraphNode::Component(index) => components
                .get(index)
                .and_then(|component| component.ranks.first())
                .map_or(machine.entry, |rank| rank.block),
        };
        return Err(FixedFuelError::ControlCycle(cycle_block));
    }
    let bound = match node {
        NaturalGraphNode::Block(block) => {
            let block_semantics = blocks
                .get(&block)
                .copied()
                .ok_or(FixedFuelError::UnknownBlock(block))?;
            let Some(units) = geometry
                .visit_units_returned
                .get(&block)
                .copied()
                .ok_or(FixedFuelError::UnknownBlock(block))?
            else {
                active.remove(&node);
                memoized.insert(node, None);
                return Ok(None);
            };
            let mut continuation = None;
            for target in terminator_targets(&block_semantics.terminator) {
                if let Some(tail) = natural_condensed_bound_returned(
                    geometry.node_for(target),
                    machine,
                    components,
                    geometry,
                    blocks,
                    memoized,
                    active,
                )? {
                    continuation = maximum_u128(continuation, Some(tail));
                }
            }
            match continuation {
                // A block with no successors is a return or crash terminal:
                // the crash case already settled `None` above, so a leaf
                // here is a returning walk that ends on this edge.
                None if terminator_targets(&block_semantics.terminator).is_empty() => {
                    Some(u128::from(units))
                }
                None => None,
                Some(tail) => Some(
                    u128::from(units)
                        .checked_add(tail)
                        .ok_or(FixedFuelError::BoundOverflow)?,
                ),
            }
        }
        NaturalGraphNode::Component(index) => {
            let component = components
                .get(index)
                .ok_or(FixedFuelError::InvalidRankedScc(machine.id))?;
            let Some(units) = geometry
                .component_units_returned
                .get(index)
                .copied()
                .ok_or(FixedFuelError::InvalidRankedScc(machine.id))?
            else {
                active.remove(&node);
                memoized.insert(node, None);
                return Ok(None);
            };
            // A returning walk leaves the component through an exit edge of
            // a member whose own visit returns, or ends inside on a
            // member's return-family terminator. Exits that ride a member
            // that can never return are unreachable to it.
            let mut exits = Vec::new();
            let mut returns_internally = false;
            for rank in &component.ranks {
                let member_returns = matches!(
                    geometry.visit_units_returned.get(&rank.block),
                    Some(Some(_))
                );
                if !member_returns {
                    continue;
                }
                let block = blocks
                    .get(&rank.block)
                    .copied()
                    .ok_or(FixedFuelError::UnknownBlock(rank.block))?;
                if matches!(
                    block.terminator,
                    Terminator::Return { .. }
                        | Terminator::ReturnUnit { .. }
                        | Terminator::ReturnUnitPartialAffine { .. }
                        | Terminator::ReturnUnitNominalAffine { .. }
                        | Terminator::ReturnStructural { .. }
                ) {
                    returns_internally = true;
                }
                exits.extend(
                    terminator_targets(&block.terminator)
                        .into_iter()
                        .filter(|target| geometry.member_of.get(target) != Some(&index)),
                );
            }
            let mut best = if returns_internally {
                Some(units)
            } else {
                None
            };
            for target in exits {
                if let Some(tail) = natural_condensed_bound_returned(
                    geometry.node_for(target),
                    machine,
                    components,
                    geometry,
                    blocks,
                    memoized,
                    active,
                )? {
                    best = maximum_u128(
                        best,
                        Some(
                            units
                                .checked_add(tail)
                                .ok_or(FixedFuelError::BoundOverflow)?,
                        ),
                    );
                }
            }
            best
        }
    };
    active.remove(&node);
    memoized.insert(node, bound);
    Ok(bound)
}

/// The `crashed` read of the same condensed control DAG
/// `natural_condensed_bound_returned` walks: a crash-terminal walk either
/// crashes inside a node's own traversal or completes the node and crashes
/// downstream. An ordinary block contributes its crash-visit bound — a
/// call site that can crash, a `Crash` terminator, or a cleanup machine
/// that crashes after the committed edge — or its complete-visit charge
/// plus a successor's crash bound. A component contributes the crash-lane
/// interior — completing members that can still reach an exit or a
/// crashable member, re-enterable ones at the rank ceiling and the rest
/// once — plus one crash visit when any member traversal can crash: a
/// crash ends the walk, so the crashing member is visited at most once,
/// never at the rank multiplier the old whole-graph ceiling billed — or
/// the same interior plus the worst exit's crash continuation, exits
/// riding only members whose visit can complete. `None` propagates as "no
/// crash-terminal walk": a machine that can only return reports no crash
/// outcome at all, so callers never inherit crash work the callee cannot
/// commit.
#[allow(clippy::too_many_arguments)]
fn natural_condensed_bound_crashed(
    node: NaturalGraphNode,
    machine: &TerminalMachine,
    components: &[TerminalNaturalCycle],
    geometry: &NaturalGeometry,
    blocks: &BTreeMap<BlockId, &terminal_psi::Block>,
    memoized: &mut BTreeMap<NaturalGraphNode, Option<u128>>,
    active: &mut BTreeSet<NaturalGraphNode>,
) -> Result<Option<u128>, FixedFuelError> {
    if let Some(bound) = memoized.get(&node) {
        return Ok(*bound);
    }
    if !active.insert(node) {
        let cycle_block = match node {
            NaturalGraphNode::Block(block) => block,
            NaturalGraphNode::Component(index) => components
                .get(index)
                .and_then(|component| component.ranks.first())
                .map_or(machine.entry, |rank| rank.block),
        };
        return Err(FixedFuelError::ControlCycle(cycle_block));
    }
    let bound = match node {
        NaturalGraphNode::Block(block) => {
            let block_semantics = blocks
                .get(&block)
                .copied()
                .ok_or(FixedFuelError::UnknownBlock(block))?;
            let mut best = geometry
                .visit_units_crashed
                .get(&block)
                .copied()
                .ok_or(FixedFuelError::UnknownBlock(block))?
                .map(u128::from);
            // A walk that continues pays the complete visit — every call
            // returns and the terminator edge commits — then crashes
            // downstream. A block that cannot complete contributes its
            // own crash visit only.
            let completes = geometry
                .visit_units_returned
                .get(&block)
                .copied()
                .ok_or(FixedFuelError::UnknownBlock(block))?;
            for target in terminator_targets(&block_semantics.terminator) {
                if let (Some(visit), Some(tail)) = (
                    completes,
                    natural_condensed_bound_crashed(
                        geometry.node_for(target),
                        machine,
                        components,
                        geometry,
                        blocks,
                        memoized,
                        active,
                    )?,
                ) {
                    best = maximum_u128(
                        best,
                        Some(
                            u128::from(visit)
                                .checked_add(tail)
                                .ok_or(FixedFuelError::BoundOverflow)?,
                        ),
                    );
                }
            }
            best
        }
        NaturalGraphNode::Component(index) => {
            let component = components
                .get(index)
                .ok_or(FixedFuelError::InvalidRankedScc(machine.id))?;
            // The crash-lane interior: visits on a crash-terminal walk
            // complete until the one crashing visit ends it, so the billed
            // set is the completing members that can still reach an exit or
            // a crashable member — a member that can never complete
            // contributes nothing, and one the crash frontier cannot be
            // reached from is never visited at all.
            let interior = geometry
                .component_units_crashed
                .get(index)
                .copied()
                .ok_or(FixedFuelError::InvalidRankedScc(machine.id))?;
            let mut best = None;
            // Crash inside the component: the interior plus exactly one
            // crashing visit — the crash ends the walk, so the crashing
            // member is visited at most once rather than at the rank
            // ceiling.
            let mut crash_visit = None;
            for rank in &component.ranks {
                if let Some(visit) = geometry
                    .visit_units_crashed
                    .get(&rank.block)
                    .copied()
                    .ok_or(FixedFuelError::UnknownBlock(rank.block))?
                {
                    crash_visit = maximum_u128(crash_visit, Some(u128::from(visit)));
                }
            }
            if let Some(crash_visit) = crash_visit {
                best = Some(
                    interior
                        .unwrap_or(0)
                        .checked_add(crash_visit)
                        .ok_or(FixedFuelError::BoundOverflow)?,
                );
            }
            // Crash beyond the component: the interior plus the worst
            // exit's crash continuation. An exit edge belongs to a
            // crash-terminal walk only when the member carrying it can
            // complete its own traversal — the same rule the returned
            // read applies.
            if interior.is_some() {
                for rank in &component.ranks {
                    let member_completes = matches!(
                        geometry.visit_units_returned.get(&rank.block),
                        Some(Some(_))
                    );
                    if !member_completes {
                        continue;
                    }
                    let block = blocks
                        .get(&rank.block)
                        .copied()
                        .ok_or(FixedFuelError::UnknownBlock(rank.block))?;
                    for target in terminator_targets(&block.terminator)
                        .into_iter()
                        .filter(|target| geometry.member_of.get(target) != Some(&index))
                    {
                        if let Some(tail) = natural_condensed_bound_crashed(
                            geometry.node_for(target),
                            machine,
                            components,
                            geometry,
                            blocks,
                            memoized,
                            active,
                        )? {
                            best = maximum_u128(
                                best,
                                Some(
                                    interior
                                        .unwrap_or(0)
                                        .checked_add(tail)
                                        .ok_or(FixedFuelError::BoundOverflow)?,
                                ),
                            );
                        }
                    }
                }
            }
            best
        }
    };
    active.remove(&node);
    memoized.insert(node, bound);
    Ok(bound)
}

fn maximum_u128(left: Option<u128>, right: Option<u128>) -> Option<u128> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

/// The crash read of one block visit: the maximum charge a traversal of
/// `block` accrues before crashing — inside a call (the earlier calls must
/// have returned for the walk to reach it), at a `Crash` terminator, or
/// inside a nominal cleanup machine a return-family terminator suspends
/// into after the edge commits. `None` when no traversal of the block can
/// crash. The derivation still runs every callee bound so malformed
/// targets (missing dispatch rows, call cycles) fail closed exactly like
/// the return charge.
#[allow(clippy::too_many_arguments)]
pub(super) fn block_crash_visit_units(
    machine: &TerminalMachine,
    block: &terminal_psi::Block,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    dynamic_call_targets: &BTreeMap<(MachineId, OperationId), MachineId>,
    provider_candidates: &BTreeMap<BoundaryMachineId, Vec<MachineId>>,
    schedule: TerminalFuelSchedule,
    memoized_machines: &mut BTreeMap<MachineId, OutcomeBounds>,
    active_machines: &mut BTreeSet<MachineId>,
) -> Result<Option<u64>, FixedFuelError> {
    let mut normal_units = Some(0_u64);
    let mut crash_units = None;
    for operation in &block.operations {
        normal_units =
            checked_optional_add(normal_units, schedule.operation_units(&operation.kind))?;
        let callees = operation_callees(
            machine.id,
            operation,
            dynamic_call_targets,
            provider_candidates,
        )?;
        // Mutually exclusive dispatch targets merge as a maximum: a
        // crashing walk pays the largest crash bound a selected callee can
        // commit, then a continuing walk pays the largest normal-return
        // bound across the same candidates.
        let mut invoked = OutcomeBounds::default();
        for &callee in &callees {
            invoked = invoked.merge(maximum_machine_outcomes(
                callee,
                machines,
                dynamic_call_targets,
                provider_candidates,
                schedule,
                memoized_machines,
                active_machines,
            )?);
        }
        if !callees.is_empty()
            && let Some(prefix) = normal_units
        {
            crash_units =
                maximum_optional(crash_units, checked_optional_add(invoked.crashed, prefix)?);
            normal_units = checked_optional_add(invoked.returned, prefix)?;
        }
    }
    let Some(prefix) = normal_units else {
        // A call on this block admits no returning target, so no traversal
        // reaches the terminator — every crash the visit can commit is
        // already accounted above.
        return Ok(crash_units);
    };
    match &block.terminator {
        Terminator::Crash { .. } => {
            crash_units = maximum_optional(
                crash_units,
                checked_optional_add(Some(prefix), schedule.terminator_units(&block.terminator))?,
            );
        }
        Terminator::Return { .. } | Terminator::ReturnUnitNominalAffine { .. } => {
            // The committed edge suspends into each nominal cleanup machine
            // in order; a crash inside one is a machine crash after the
            // completed prefix — the same crash column
            // `compose_cleanup_outcomes` accumulates for the entry
            // derivation.
            crash_units = compose_cleanup_outcomes(
                terminator_cleanup_machines(&block.terminator),
                OutcomeBounds {
                    returned: Some(
                        prefix
                            .checked_add(schedule.terminator_units(&block.terminator))
                            .ok_or(FixedFuelError::BoundOverflow)?,
                    ),
                    crashed: crash_units,
                },
                machines,
                dynamic_call_targets,
                provider_candidates,
                schedule,
                memoized_machines,
                active_machines,
            )?
            .crashed;
        }
        _ => {}
    }
    Ok(crash_units)
}

/// The visit charge restricted to walks that return normally: each call
/// site charges the maximum normal-return bound across its admitted
/// targets instead of the worst outcome, a terminator-suspended cleanup
/// composes only its own return, and a block whose call or cleanup can
/// never return — or whose terminator is a crash — reports `None` because
/// no commit-reachable traversal crosses it. The derivation still runs
/// every callee bound so malformed targets (missing dispatch rows, call
/// cycles) fail closed exactly like the crash charge.
#[allow(clippy::too_many_arguments)]
pub(super) fn block_return_visit_units(
    machine: &TerminalMachine,
    block: &terminal_psi::Block,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    dynamic_call_targets: &BTreeMap<(MachineId, OperationId), MachineId>,
    provider_candidates: &BTreeMap<BoundaryMachineId, Vec<MachineId>>,
    schedule: TerminalFuelSchedule,
    memoized_machines: &mut BTreeMap<MachineId, OutcomeBounds>,
    active_machines: &mut BTreeSet<MachineId>,
) -> Result<Option<u64>, FixedFuelError> {
    let mut units = block
        .operations
        .iter()
        .try_fold(0_u64, |units, operation| {
            units
                .checked_add(schedule.operation_units(&operation.kind))
                .ok_or(FixedFuelError::BoundOverflow)
        })?;
    for operation in &block.operations {
        let callees = operation_callees(
            machine.id,
            operation,
            dynamic_call_targets,
            provider_candidates,
        )?;
        // Mutually exclusive dispatch targets: a returning visit invokes at
        // most one callee and it must return, so the charge is the maximum
        // normal-return bound across the candidates that can produce one.
        let mut invoked_returned = None;
        for &callee in &callees {
            let callee_bounds = maximum_machine_outcomes(
                callee,
                machines,
                dynamic_call_targets,
                provider_candidates,
                schedule,
                memoized_machines,
                active_machines,
            )?;
            invoked_returned = maximum_optional(invoked_returned, callee_bounds.returned);
        }
        if !callees.is_empty() {
            let Some(invoked) = invoked_returned else {
                return Ok(None);
            };
            units = units
                .checked_add(invoked)
                .ok_or(FixedFuelError::BoundOverflow)?;
        }
    }
    if matches!(block.terminator, Terminator::Crash { .. }) {
        return Ok(None);
    }
    units = units
        .checked_add(schedule.terminator_units(&block.terminator))
        .ok_or(FixedFuelError::BoundOverflow)?;
    for cleanup_machine in terminator_cleanup_machines(&block.terminator) {
        let cleanup_bounds = maximum_machine_outcomes(
            cleanup_machine,
            machines,
            dynamic_call_targets,
            provider_candidates,
            schedule,
            memoized_machines,
            active_machines,
        )?;
        // A returning traversal crosses every suspended cleanup in order, so
        // one that can never return ends commit-reachable visits here.
        let Some(returned) = cleanup_bounds.returned else {
            return Ok(None);
        };
        units = units
            .checked_add(returned)
            .ok_or(FixedFuelError::BoundOverflow)?;
    }
    Ok(Some(units))
}

/// Directed absence-of-bound report for the cyclic component an acyclic
/// traversal re-entered: the verifier-derived component containing `hit`,
/// identified by its internal edge topology, with `Unranked` as the cause —
/// a ranked component bounds through the condensed graph and never reaches
/// this report. A machine whose graph fails basic verification has no
/// component derivation to cite and keeps the plain traversal marker.
pub(super) fn unbounded_cycle_report(machine: &TerminalMachine, hit: BlockId) -> FixedFuelError {
    terminal_verifier::control_cycle_members(machine)
        .ok()
        .and_then(|components| {
            components
                .into_iter()
                .find(|members| members.contains(&hit))
        })
        .map(|members| FixedFuelError::UnboundedCycleComponent {
            component: terminal_verifier::cyclic_component_identity(machine, &members),
            cause: UnboundedCycleCause::Unranked,
        })
        .unwrap_or(FixedFuelError::ControlCycle(hit))
}

pub(super) fn terminator_targets(terminator: &Terminator) -> Vec<BlockId> {
    match terminator {
        Terminator::Jump { target, .. } => vec![*target],
        Terminator::Conditional {
            when_true,
            when_false,
            ..
        } => vec![when_true.target, when_false.target],
        Terminator::StructuralCase { cases, .. } => cases.iter().map(|case| case.target).collect(),
        Terminator::Return { .. }
        | Terminator::ReturnUnit { .. }
        | Terminator::ReturnUnitPartialAffine { .. }
        | Terminator::ReturnUnitNominalAffine { .. }
        | Terminator::ReturnStructural { .. }
        | Terminator::Crash { .. } => Vec::new(),
    }
}

/// Each successor edge paired with its target. Terminal edges carry no
/// target, so a return or crash contributes nothing — the same successor set
/// `terminator_targets` sees, with the edge identity kept alongside.
pub(super) fn terminator_edge_targets(terminator: &Terminator) -> Vec<(EdgeId, BlockId)> {
    match terminator {
        Terminator::Jump { edge, target, .. } => vec![(*edge, *target)],
        Terminator::Conditional {
            when_true,
            when_false,
            ..
        } => vec![
            (when_true.edge, when_true.target),
            (when_false.edge, when_false.target),
        ],
        Terminator::StructuralCase { cases, .. } => {
            cases.iter().map(|case| (case.edge, case.target)).collect()
        }
        Terminator::Return { .. }
        | Terminator::ReturnUnit { .. }
        | Terminator::ReturnUnitPartialAffine { .. }
        | Terminator::ReturnUnitNominalAffine { .. }
        | Terminator::ReturnStructural { .. }
        | Terminator::Crash { .. } => Vec::new(),
    }
}

/// The nominal cleanup machines a terminator's edge suspends into, in the
/// terminator's own execution order. Only `Return` cleanup actions and
/// `ReturnUnitNominalAffine` entries invoke machines; every other edge commits
/// no in-module cleanup work.
pub(super) fn terminator_cleanup_machines(terminator: &Terminator) -> Vec<MachineId> {
    match terminator {
        Terminator::Return {
            cleanup_actions, ..
        } => cleanup_actions
            .iter()
            .filter_map(|action| match action {
                TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                    Some(cleanup.cleanup_machine)
                }
                TerminalAffineCleanupAction::DiscardRoot(_)
                | TerminalAffineCleanupAction::DiscardResidual(_) => None,
            })
            .collect(),
        Terminator::ReturnUnitNominalAffine { cleanups, .. } => cleanups
            .iter()
            .map(|cleanup| cleanup.cleanup_machine)
            .collect(),
        _ => Vec::new(),
    }
}

fn outcome_bounds_from(
    machine: MachineId,
    current: BlockId,
    blocks: &BTreeMap<BlockId, &terminal_psi::Block>,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    dynamic_call_targets: &BTreeMap<(MachineId, OperationId), MachineId>,
    provider_candidates: &BTreeMap<BoundaryMachineId, Vec<MachineId>>,
    schedule: TerminalFuelSchedule,
    memoized: &mut BTreeMap<BlockId, OutcomeBounds>,
    active: &mut BTreeSet<BlockId>,
    memoized_machines: &mut BTreeMap<MachineId, OutcomeBounds>,
    active_machines: &mut BTreeSet<MachineId>,
) -> Result<OutcomeBounds, FixedFuelError> {
    if let Some(bounds) = memoized.get(&current) {
        return Ok(*bounds);
    }
    if !active.insert(current) {
        return Err(machines
            .get(&machine)
            .copied()
            .map(|semantics| unbounded_cycle_report(semantics, current))
            .unwrap_or(FixedFuelError::ControlCycle(current)));
    }
    let block = blocks
        .get(&current)
        .copied()
        .ok_or(FixedFuelError::UnknownBlock(current))?;
    let mut normal_units = Some(0_u64);
    let mut crash_units = None;
    for operation in &block.operations {
        normal_units =
            checked_optional_add(normal_units, schedule.operation_units(&operation.kind))?;
        let callees = operation_callees(
            machine,
            operation,
            dynamic_call_targets,
            provider_candidates,
        )?;
        // One invocation dispatches to at most one callee, so the
        // candidates' outcome bounds merge as a maximum, not a sum. The
        // derivation still runs when the call is unreachable so malformed
        // targets (missing dispatch rows, call cycles) fail closed.
        let mut invoked = OutcomeBounds::default();
        for callee in &callees {
            invoked = invoked.merge(maximum_machine_outcomes(
                *callee,
                machines,
                dynamic_call_targets,
                provider_candidates,
                schedule,
                memoized_machines,
                active_machines,
            )?);
        }
        if !callees.is_empty()
            && let Some(prefix) = normal_units
        {
            crash_units =
                maximum_optional(crash_units, checked_optional_add(invoked.crashed, prefix)?);
            normal_units = checked_optional_add(invoked.returned, prefix)?;
        }
    }
    let terminator_units = schedule.terminator_units(&block.terminator);
    let continued = match (&block.terminator, normal_units) {
        (_, None) => OutcomeBounds::default(),
        (
            Terminator::ReturnUnit { .. }
            | Terminator::ReturnUnitPartialAffine { .. }
            | Terminator::ReturnStructural { .. },
            Some(prefix),
        ) => OutcomeBounds {
            returned: Some(
                prefix
                    .checked_add(terminator_units)
                    .ok_or(FixedFuelError::BoundOverflow)?,
            ),
            crashed: None,
        },
        (
            Terminator::Return {
                cleanup_actions, ..
            },
            Some(prefix),
        ) => compose_cleanup_outcomes(
            cleanup_actions.iter().filter_map(|action| match action {
                TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                    Some(cleanup.cleanup_machine)
                }
                TerminalAffineCleanupAction::DiscardRoot(_)
                | TerminalAffineCleanupAction::DiscardResidual(_) => None,
            }),
            OutcomeBounds {
                returned: Some(
                    prefix
                        .checked_add(terminator_units)
                        .ok_or(FixedFuelError::BoundOverflow)?,
                ),
                crashed: None,
            },
            machines,
            dynamic_call_targets,
            provider_candidates,
            schedule,
            memoized_machines,
            active_machines,
        )?,
        (Terminator::ReturnUnitNominalAffine { cleanups, .. }, Some(prefix)) => {
            compose_cleanup_outcomes(
                cleanups.iter().map(|cleanup| cleanup.cleanup_machine),
                OutcomeBounds {
                    returned: Some(
                        prefix
                            .checked_add(terminator_units)
                            .ok_or(FixedFuelError::BoundOverflow)?,
                    ),
                    crashed: None,
                },
                machines,
                dynamic_call_targets,
                provider_candidates,
                schedule,
                memoized_machines,
                active_machines,
            )?
        }
        (Terminator::Crash { .. }, Some(prefix)) => OutcomeBounds {
            returned: None,
            crashed: Some(
                prefix
                    .checked_add(terminator_units)
                    .ok_or(FixedFuelError::BoundOverflow)?,
            ),
        },
        (Terminator::Jump { target, .. }, Some(prefix)) => outcome_bounds_from(
            machine,
            *target,
            blocks,
            machines,
            dynamic_call_targets,
            provider_candidates,
            schedule,
            memoized,
            active,
            memoized_machines,
            active_machines,
        )?
        .with_prefix(
            prefix
                .checked_add(terminator_units)
                .ok_or(FixedFuelError::BoundOverflow)?,
        )?,
        (
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            },
            Some(prefix),
        ) => outcome_bounds_from(
            machine,
            when_true.target,
            blocks,
            machines,
            dynamic_call_targets,
            provider_candidates,
            schedule,
            memoized,
            active,
            memoized_machines,
            active_machines,
        )?
        .merge(outcome_bounds_from(
            machine,
            when_false.target,
            blocks,
            machines,
            dynamic_call_targets,
            provider_candidates,
            schedule,
            memoized,
            active,
            memoized_machines,
            active_machines,
        )?)
        .with_prefix(
            prefix
                .checked_add(terminator_units)
                .ok_or(FixedFuelError::BoundOverflow)?,
        )?,
        (Terminator::StructuralCase { cases, .. }, Some(prefix)) => {
            let mut cases = cases.iter();
            let first = cases
                .next()
                .ok_or(FixedFuelError::BranchingNotYetSupported(current))?;
            let mut bounds = outcome_bounds_from(
                machine,
                first.target,
                blocks,
                machines,
                dynamic_call_targets,
                provider_candidates,
                schedule,
                memoized,
                active,
                memoized_machines,
                active_machines,
            )?;
            for case in cases {
                bounds = bounds.merge(outcome_bounds_from(
                    machine,
                    case.target,
                    blocks,
                    machines,
                    dynamic_call_targets,
                    provider_candidates,
                    schedule,
                    memoized,
                    active,
                    memoized_machines,
                    active_machines,
                )?);
            }
            bounds.with_prefix(
                prefix
                    .checked_add(terminator_units)
                    .ok_or(FixedFuelError::BoundOverflow)?,
            )?
        }
    };
    active.remove(&current);
    let bounds = OutcomeBounds {
        returned: continued.returned,
        crashed: maximum_optional(crash_units, continued.crashed),
    };
    memoized.insert(current, bounds);
    Ok(bounds)
}

/// Sequence the ordered nominal cleanup machines of a committed edge into
/// outcome bounds. Each cleanup runs only when every earlier one returned; a
/// cleanup that crashes after the edge charge still counts its own bound plus
/// the completed prefix. A cleanup machine with no terminal outcome is a
/// broken semantic invariant, not an empty contribution.
pub(super) fn compose_cleanup_outcomes(
    cleanup_machines: impl IntoIterator<Item = MachineId>,
    mut bounds: OutcomeBounds,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    dynamic_call_targets: &BTreeMap<(MachineId, OperationId), MachineId>,
    provider_candidates: &BTreeMap<BoundaryMachineId, Vec<MachineId>>,
    schedule: TerminalFuelSchedule,
    memoized_machines: &mut BTreeMap<MachineId, OutcomeBounds>,
    active_machines: &mut BTreeSet<MachineId>,
) -> Result<OutcomeBounds, FixedFuelError> {
    for cleanup_machine in cleanup_machines {
        let Some(cleanup_prefix) = bounds.returned else {
            break;
        };
        let cleanup_bounds = maximum_machine_outcomes(
            cleanup_machine,
            machines,
            dynamic_call_targets,
            provider_candidates,
            schedule,
            memoized_machines,
            active_machines,
        )?;
        if cleanup_bounds.maximum().is_none() {
            return Err(FixedFuelError::NoTerminalPath(cleanup_machine));
        }
        bounds = OutcomeBounds {
            returned: checked_optional_add(cleanup_bounds.returned, cleanup_prefix)?,
            crashed: maximum_optional(
                bounds.crashed,
                checked_optional_add(cleanup_bounds.crashed, cleanup_prefix)?,
            ),
        };
    }
    Ok(bounds)
}
