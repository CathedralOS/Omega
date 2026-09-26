//! Outcome-sensitive machine and control-flow composition.

use crate::{FixedFuelError, UnboundedCycleCause};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, EdgeId, IntegerMathLiteral, IntegerMathTerm, IntegerSign,
    IntegerType, IntegerValue, MachineId, OperationId, Proposition, ScalarTerm, ScalarType,
    ValueId,
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
            )
            .map_err(|error| {
                member_closure_error(machine, block.id, &member_of, components, error)
            })?,
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
            )
            .map_err(|error| {
                member_closure_error(machine, block.id, &member_of, components, error)
            })?,
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
        // `requires` clauses whose conjunction caps every rank arriving at
        // the component's first entry tighten the visit bound below the
        // carrier maximum; the consulted clauses enter the certificate's
        // `relevant_preconditions` through `used_contract_premises`.
        let rank_bound = component_entry_rank_bound(machine, component, &blocks, rank_maximum)
            .map(|entry| entry.bound)
            .unwrap_or(rank_maximum);
        // The interior's rank-bounded visit arithmetic is the component's
        // own bound: when it cannot be represented, the report names this
        // component with the unbounded-rank cause rather than a flat
        // overflow.
        let interior = component_interior(
            component,
            index,
            &blocks,
            &member_of,
            &visit_units_returned,
            &visit_units_crashed,
            rank_bound,
        )
        .map_err(|error| match error {
            FixedFuelError::BoundOverflow => unbounded_rank_report(machine, component),
            other => other,
        })?;
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
/// the lower ceiling the `requires` clauses' conjunction places on every
/// rank arriving at first entry — while a member left off every
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
/// satisfies them; clauses whose conjunction caps the value arriving as a
/// member's rank observation therefore bound the component's initial rank
/// directly. Every way control first enters the component must be covered:
/// the machine entry itself when it is a member — the entry block declares
/// no parameters, so only a machine-parameter observation can be bounded —
/// and each edge arriving from outside the component, whose arriving rank
/// is the argument at the target's rank-parameter position. An arrival
/// reduces to a boundable value only when it is a machine parameter the
/// contract caps — directly by a literal clause, through a relational
/// chain the clauses themselves state, through an `IntegerMath*` affine
/// bound solved over unbounded integers, or through a conditional row
/// whose arms all bound it or whose premise the ambient rows discharge, as
/// `contract_scope` derives; an argument threaded through another block's
/// parameters, a computed value, an observed view, or a structural-case
/// payload has no contract ceiling, so one unbounded arrival leaves the
/// carrier maximum in place rather than guessing. The result is `Some`
/// only when the derived ceiling genuinely tightens the type maximum — a
/// clause that merely restates it binds nothing new.
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
    let scope = contract_scope(machine, component.rank_type);
    let ceilings = relaxed_ceilings(&scope);
    let mut bound = 0_u128;
    let mut clauses = BTreeSet::new();
    for arrival in arrivals {
        if !machine.parameters.iter().any(|parameter| {
            parameter.id == arrival
                && parameter.scalar_type == ScalarType::Integer(component.rank_type)
        }) {
            return None;
        }
        let (candidate, support) = scope_bound(&scope, &ceilings, arrival)?;
        bound = bound.max(candidate);
        clauses.extend(support);
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

/// The `requires` row positions one derived fact rests on — the
/// certificate's premise set is their union. A leaf's support is its own
/// row; a conditional row's contribution also carries the rows that
/// discharged or bounded its arms.
type ClauseSupport = BTreeSet<usize>;

/// The unconditional ceiling facts one clause context provides: literal
/// `terminals` capping a value outright, relational `edges` transferring a
/// target's ceiling less a strictness cost, and `leaves` retaining every
/// unconditional leaf proposition verbatim so an implication premise that
/// restates an assumed fact can discharge against it.
#[derive(Clone, Default)]
struct ClauseScope<'a> {
    terminals: Vec<(ValueId, u128, ClauseSupport)>,
    edges: Vec<(ValueId, ValueId, u128, ClauseSupport)>,
    leaves: Vec<(ClauseSupport, &'a Proposition)>,
}

/// The value a term names when it is a plain scalar value of the rank
/// carrier's type — any other shape (a field observation, an arithmetic
/// composite, a different integer type) names no boundable parameter here.
fn rank_value(term: &ScalarTerm, rank_type: IntegerType) -> Option<ValueId> {
    match term {
        ScalarTerm::Value {
            id,
            scalar_type: actual,
        } if *actual == ScalarType::Integer(rank_type) => Some(*id),
        _ => None,
    }
}

/// An unsigned literal of the rank carrier's type — a signed or wrong-typed
/// literal caps no unsigned rank.
fn rank_literal(term: &ScalarTerm, rank_type: IntegerType) -> Option<u128> {
    match term {
        ScalarTerm::Integer {
            scalar_type: actual,
            value: IntegerValue::Unsigned(value),
        } if *actual == rank_type => Some(*value),
        _ => None,
    }
}

/// One mathematical integer literal as `i128`. A magnitude beyond the
/// signed range cannot participate in the affine solving below and leaves
/// the clause unhandled rather than misread.
fn math_literal(literal: &IntegerMathLiteral) -> Option<i128> {
    if literal.negative() {
        if literal.magnitude() == (i128::MAX as u128) + 1 {
            Some(i128::MIN)
        } else {
            i128::try_from(literal.magnitude()).ok().map(|value| -value)
        }
    } else {
        i128::try_from(literal.magnitude()).ok()
    }
}

/// One clause-side affine form: `(coefficient, variable, offset)` reading
/// the term as `coefficient * variable + offset` over unbounded integers.
/// Only the single-variable fragment solves — a product of two variables,
/// two distinct variables in one side, or a subtraction or negative
/// scaling that flips the coefficient into a lower-bound shape all return
/// `None` and leave the clause contributing nothing.
fn math_affine(term: &IntegerMathTerm) -> Option<(u128, Option<(IntegerType, ValueId)>, i128)> {
    match term {
        IntegerMathTerm::IntegerLiteral(literal) => Some((0, None, math_literal(literal)?)),
        IntegerMathTerm::MathValue { source_type, value } if !source_type.is_address() => {
            Some((1, Some((*source_type, *value)), 0))
        }
        IntegerMathTerm::Add(left, right) => {
            let (lc, lv, lo) = math_affine(left)?;
            let (rc, rv, ro) = math_affine(right)?;
            let offset = lo.checked_add(ro)?;
            match (lv, rv) {
                (Some(v), None) | (None, Some(v)) => Some((lc.checked_add(rc)?, Some(v), offset)),
                (None, None) => Some((0, None, offset)),
                (Some(v), Some(w)) if v == w => Some((lc.checked_add(rc)?, Some(v), offset)),
                _ => None,
            }
        }
        IntegerMathTerm::Subtract(left, right) => {
            let (lc, lv, lo) = math_affine(left)?;
            let (rc, rv, ro) = math_affine(right)?;
            let offset = lo.checked_sub(ro)?;
            match (lv, rv) {
                (Some(v), None) => Some((lc, Some(v), offset)),
                (None, None) => Some((0, None, offset)),
                (Some(v), Some(w)) if v == w => Some((lc.checked_sub(rc)?, Some(v), offset)),
                _ => None,
            }
        }
        IntegerMathTerm::Multiply(left, right) => {
            let (lc, lv, lo) = math_affine(left)?;
            let (rc, rv, ro) = math_affine(right)?;
            // A nonnegative literal factor scales the other side; a
            // negative scale flips the coefficient into a lower-bound
            // form, and a product of two variable terms is nonlinear.
            match (lv, rv) {
                (Some(_), Some(_)) => None,
                (Some(v), None) => {
                    let scale = u128::try_from(ro).ok()?;
                    Some((lc.checked_mul(scale)?, Some(v), lo.checked_mul(ro)?))
                }
                (None, Some(v)) => {
                    let scale = u128::try_from(lo).ok()?;
                    Some((rc.checked_mul(scale)?, Some(v), ro.checked_mul(lo)?))
                }
                (None, None) => Some((0, None, lo.checked_mul(ro)?)),
            }
        }
        IntegerMathTerm::ShiftLeft { value, count } => {
            let (vc, vv, vo) = math_affine(value)?;
            let (0, None, shift) = math_affine(count)? else {
                return None;
            };
            let shift = u32::try_from(shift).ok()?;
            let factor = 2u128.checked_pow(shift)?;
            Some((
                vc.checked_mul(factor)?,
                vv,
                vo.checked_mul(i128::try_from(factor).ok()?)?,
            ))
        }
        _ => None,
    }
}

/// The terminal or transfer edge `left OP right` contributes over
/// unbounded integer math, where `strict` 0 reads `<=` and 1 reads `<`.
/// `s*x + o (+ strict) <= k` caps `x` at `(k - o - strict) / s` floored —
/// with `s > 0` and a nonnegative numerator, else the row bounds `x` from
/// below or not at all and contributes nothing. `x + o (+ strict) <= y +
/// r` transfers `y`'s ceiling when `o - r + strict` is nonnegative —
/// `x + c <= y` — while a negative constant is a slack relation `x <= y +
/// d` the edge model cannot express, so the clause adds nothing. Floors,
/// multi-variable rows, and closed comparisons contribute nothing here.
fn math_order_fragments(
    left: &IntegerMathTerm,
    right: &IntegerMathTerm,
    strict: u128,
    support: ClauseSupport,
    scope: &mut ClauseScope,
) {
    let Some((lc, lv, lo)) = math_affine(left) else {
        return;
    };
    let Some((rc, rv, ro)) = math_affine(right) else {
        return;
    };
    let Ok(strict) = i128::try_from(strict) else {
        return;
    };
    match (lv, rv) {
        (Some((_, x)), None) if lc > 0 && rc == 0 => {
            let Some(numerator) = ro.checked_sub(lo).and_then(|n| n.checked_sub(strict)) else {
                return;
            };
            if numerator < 0 {
                return;
            }
            let Ok(divisor) = i128::try_from(lc) else {
                return;
            };
            let Ok(bound) = u128::try_from(numerator / divisor) else {
                return;
            };
            scope.terminals.push((x, bound, support));
        }
        (Some((_, x)), Some((_, y))) if lc == 1 && rc == 1 => {
            let Some(cost) = lo.checked_sub(ro).and_then(|cost| cost.checked_add(strict)) else {
                return;
            };
            let Ok(cost) = u128::try_from(cost) else {
                return;
            };
            scope.edges.push((x, y, cost, support));
        }
        _ => {}
    }
}

/// `left = right` over unbounded integer math. An affine-literal equality
/// `s*x + o = k` fixes `x` at `(k - o) / s` when the division is exact — a
/// terminal ceiling; an inexact or negative quotient caps nothing here
/// (the contradiction check decides whether the row can hold at all). `x +
/// o = y + r` states both `x + (o - r) = y` and `y + (r - o) = x`: each
/// direction yields a transfer edge only while its constant is
/// nonnegative — equal offsets keep the bidirectional zero-cost pair of
/// the scalar equality clause, otherwise the slack direction is dropped
/// rather than approximated.
fn math_equal_fragments(
    left: &IntegerMathTerm,
    right: &IntegerMathTerm,
    support: ClauseSupport,
    scope: &mut ClauseScope,
) {
    let Some((lc, lv, lo)) = math_affine(left) else {
        return;
    };
    let Some((rc, rv, ro)) = math_affine(right) else {
        return;
    };
    let exact = |coefficient: u128, offset: i128, literal: i128| -> Option<u128> {
        let Ok(divisor) = i128::try_from(coefficient) else {
            return None;
        };
        let difference = literal.checked_sub(offset)?;
        if difference < 0 || difference % divisor != 0 {
            return None;
        }
        u128::try_from(difference / divisor).ok()
    };
    match (lv, rv) {
        (Some((_, x)), None) if lc > 0 && rc == 0 => {
            if let Some(bound) = exact(lc, lo, ro) {
                scope.terminals.push((x, bound, support));
            }
        }
        (None, Some((_, y))) if rc > 0 && lc == 0 => {
            if let Some(bound) = exact(rc, ro, lo) {
                scope.terminals.push((y, bound, support));
            }
        }
        (Some((_, x)), Some((_, y))) if lc == 1 && rc == 1 => {
            for (from, to, difference) in [(x, y, lo.checked_sub(ro)), (y, x, ro.checked_sub(lo))] {
                if let Some(cost) = difference.and_then(|d| u128::try_from(d).ok()) {
                    scope.edges.push((from, to, cost, support.clone()));
                }
            }
        }
        _ => {}
    }
}

/// The machine contract's resolved clause scope at `rank_type`: every
/// `requires` row's unconditional content plus the conditional rows that
/// resolve under it. Rows enter tagged with their own position so derived
/// facts carry their premises.
fn contract_scope<'a>(machine: &'a TerminalMachine, rank_type: IntegerType) -> ClauseScope<'a> {
    let clauses: Vec<(ClauseSupport, &Proposition)> = machine
        .contract
        .requires
        .iter()
        .enumerate()
        .map(|(row, clause)| (ClauseSupport::from([row]), clause))
        .collect();
    resolve_scope(&ClauseScope::default(), &clauses, rank_type)
}

/// Enrich `base` with the unconditional content `clauses` add to it.
/// Conditional rows resolve in one ordered pass: implications discharge to
/// a fixpoint against the conjunctive context — a discharged premise makes
/// its conclusion unconditional, and that content can discharge a further
/// row — then each disjunction resolves against the settled scope in row
/// order, so an earlier disjunction's ceilings are visible inside a later
/// one's arms. A ceiling a disjunction itself derives never discharges a
/// sibling implication's premise; the bound stays conservative rather than
/// iterating conditional forms to a fixpoint.
fn resolve_scope<'a>(
    base: &ClauseScope<'a>,
    clauses: &[(ClauseSupport, &'a Proposition)],
    rank_type: IntegerType,
) -> ClauseScope<'a> {
    let mut scope = base.clone();
    let mut deferred: Vec<(ClauseSupport, &Proposition)> = Vec::new();
    for (support, clause) in clauses {
        collect_clause_fragments(
            clause,
            support.clone(),
            rank_type,
            &mut scope,
            &mut deferred,
        );
    }
    loop {
        let ceilings = relaxed_ceilings(&scope);
        let mut progressed = false;
        let mut index = 0;
        while index < deferred.len() {
            let Proposition::Implication { premise, .. } = deferred[index].1 else {
                index += 1;
                continue;
            };
            let Some(rows) = premise_support(premise, &scope, &ceilings, rank_type) else {
                index += 1;
                continue;
            };
            let (mut support, proposition) = deferred.remove(index);
            support.extend(rows);
            let Proposition::Implication { conclusion, .. } = proposition else {
                continue;
            };
            collect_clause_fragments(conclusion, support, rank_type, &mut scope, &mut deferred);
            progressed = true;
        }
        if !progressed {
            break;
        }
    }
    for (support, proposition) in deferred {
        let Proposition::Disjunction(arms) = proposition else {
            continue;
        };
        resolve_disjunction(&support, arms, &mut scope, rank_type);
    }
    scope
}

/// Flatten the content `proposition` contributes when it holds under
/// `support`. A `Conjunction` dissolves into its children; each leaf seeds
/// a ceiling terminal or a relational edge — the scalar comparisons
/// directly, the `IntegerMath*` orderings through `math_affine`'s
/// single-variable affine fragment — and joins `leaves` for verbatim
/// premise discharge; a `Disjunction` or `Implication` is a conditional
/// form and defers until this level's conjunctive ceilings settle. A
/// literal on the left of `<=` bounds its parameter from below, a
/// wrong-typed or signed literal caps nothing, and a term that is neither
/// a value nor a literal — a field observation or an arithmetic composite —
/// places no ceiling the derivation can trust.
fn collect_clause_fragments<'a>(
    proposition: &'a Proposition,
    support: ClauseSupport,
    rank_type: IntegerType,
    scope: &mut ClauseScope<'a>,
    deferred: &mut Vec<(ClauseSupport, &'a Proposition)>,
) {
    let mut pending = vec![proposition];
    while let Some(proposition) = pending.pop() {
        match proposition {
            Proposition::Conjunction(children) => pending.extend(children),
            Proposition::Disjunction(_) | Proposition::Implication { .. } => {
                deferred.push((support.clone(), proposition));
            }
            leaf => {
                scope.leaves.push((support.clone(), leaf));
                match leaf {
                    Proposition::LessOrEqual(left, right) => {
                        match (
                            rank_value(left, rank_type),
                            rank_value(right, rank_type),
                            rank_literal(right, rank_type),
                        ) {
                            (Some(x), _, Some(k)) => {
                                scope.terminals.push((x, k, support.clone()));
                            }
                            (Some(x), Some(y), None) => {
                                scope.edges.push((x, y, 0, support.clone()));
                            }
                            _ => {}
                        }
                    }
                    Proposition::LessThan(left, right) => {
                        match (
                            rank_value(left, rank_type),
                            rank_value(right, rank_type),
                            rank_literal(right, rank_type),
                        ) {
                            (Some(x), _, Some(k)) => {
                                if let Some(k) = k.checked_sub(1) {
                                    scope.terminals.push((x, k, support.clone()));
                                }
                            }
                            (Some(x), Some(y), None) => {
                                scope.edges.push((x, y, 1, support.clone()));
                            }
                            _ => {}
                        }
                    }
                    Proposition::Equal(left, right) => {
                        match (
                            rank_value(left, rank_type),
                            rank_value(right, rank_type),
                            rank_literal(left, rank_type),
                            rank_literal(right, rank_type),
                        ) {
                            (Some(x), _, _, Some(k)) => {
                                scope.terminals.push((x, k, support.clone()));
                            }
                            (_, Some(y), Some(k), _) => {
                                scope.terminals.push((y, k, support.clone()));
                            }
                            (Some(x), Some(y), None, None) => {
                                scope.edges.push((x, y, 0, support.clone()));
                                scope.edges.push((y, x, 0, support.clone()));
                            }
                            _ => {}
                        }
                    }
                    Proposition::IntegerMathLessOrEqual(left, right) => {
                        math_order_fragments(left, right, 0, support.clone(), scope);
                    }
                    Proposition::IntegerMathLessThan(left, right) => {
                        math_order_fragments(left, right, 1, support.clone(), scope);
                    }
                    Proposition::IntegerMathEqual(left, right) => {
                        math_equal_fragments(left, right, support.clone(), scope);
                    }
                    _ => {}
                }
            }
        }
    }
}

/// The tightest ceiling per value the scope's terminals and transfer edges
/// derive. Terminals seed the map, then relational edges propagate — `x <
/// y` hands `x` the bound `y`'s ceiling less one since `x <= y - 1`, and a
/// step that would fall below zero rides an unsatisfiable path rather than
/// a usable bound. Every edge cost is nonnegative, so a tightest chain
/// never needs to revisit a value: after as many rounds as the relation
/// graph has nodes, the best simple derivation has settled.
fn relaxed_ceilings(scope: &ClauseScope) -> BTreeMap<ValueId, u128> {
    let mut ceilings: BTreeMap<ValueId, u128> = BTreeMap::new();
    for (node, bound, _) in &scope.terminals {
        ceilings
            .entry(*node)
            .and_modify(|best| *best = (*best).min(*bound))
            .or_insert(*bound);
    }
    let nodes: BTreeSet<ValueId> = scope
        .edges
        .iter()
        .flat_map(|&(x, y, _, _)| [x, y])
        .chain(ceilings.keys().copied())
        .collect();
    for _ in 0..nodes.len() {
        let mut improved = false;
        for &(x, y, cost, _) in &scope.edges {
            let Some(&bound) = ceilings.get(&y) else {
                continue;
            };
            let Some(candidate) = bound.checked_sub(cost) else {
                continue;
            };
            if ceilings.get(&x).is_none_or(|&best| candidate < best) {
                ceilings.insert(x, candidate);
                improved = true;
            }
        }
        if !improved {
            break;
        }
    }
    ceilings
}

/// Resolve one conditional disjunction row into the unconditional content
/// every arm agrees on. Under an arm the ambient facts still hold, so each
/// arm's own scope resolves against the scope so far; a value every live
/// arm caps takes the maximum arm ceiling — whichever arm holds, the bound
/// does — and a relational edge every live arm states transfers
/// unconditionally at the weakest strictness. An arm that cannot hold
/// under any valuation — `Falsehood`, an unsigned-below-zero leaf, or a
/// conjunction containing either — drops out of the maximum instead of
/// sinking the row; when no arm can hold at all the clause is vacuous and
/// contributes nothing. The derived bound rests on the row itself plus
/// whatever ambient rows each arm's achieving chain consulted, so the
/// premise set is recovered per arm and unioned.
fn resolve_disjunction<'a>(
    support: &ClauseSupport,
    arms: &'a [Proposition],
    scope: &mut ClauseScope<'a>,
    rank_type: IntegerType,
) {
    let base_edges = scope.edges.len();
    let mut arm_scopes = Vec::with_capacity(arms.len());
    for arm in arms {
        if proposition_unsatisfiable(arm) {
            arm_scopes.push(None);
            continue;
        }
        arm_scopes.push(Some(resolve_scope(
            scope,
            &[(support.clone(), arm)],
            rank_type,
        )));
    }
    let live: Vec<&ClauseScope> = arm_scopes.iter().flatten().collect();
    if live.is_empty() {
        return;
    }
    let arm_ceilings: Vec<BTreeMap<ValueId, u128>> =
        live.iter().map(|arm| relaxed_ceilings(arm)).collect();
    let mut shared_domain: BTreeSet<ValueId> = arm_ceilings[0].keys().copied().collect();
    for ceilings in &arm_ceilings[1..] {
        shared_domain.retain(|value| ceilings.contains_key(value));
    }
    for value in shared_domain {
        let bound = arm_ceilings
            .iter()
            .map(|ceilings| ceilings[&value])
            .max()
            .unwrap_or(0);
        // Recover the achieving rows per arm: the bound holds under
        // whichever arm is selected, so it rests on every arm's own
        // derivation. An arm whose support cannot be recovered leaves the
        // terminal unwritten rather than publishing a premise set that
        // does not entail the bound.
        let mut rows = support.clone();
        let mut justified = true;
        for (index, ceilings) in arm_ceilings.iter().enumerate() {
            let mut visited = BTreeSet::from([value]);
            match justify_requires_ceiling(
                value,
                ceilings[&value],
                ceilings,
                live[index],
                &mut visited,
            ) {
                Some(arm_support) => rows.extend(arm_support),
                None => {
                    justified = false;
                    break;
                }
            }
        }
        if justified {
            scope.terminals.push((value, bound, rows));
        }
    }
    // A transfer edge every live arm states holds under the row alone:
    // `(x <= y) or (x < y)` still entails `x <= y`, so the contributed
    // edge takes the weakest strictness across arms. With a single live
    // arm the fold still applies — the remaining arms are unsatisfiable,
    // so its content is the row's.
    let mut shared_edges: Option<BTreeMap<(ValueId, ValueId), (u128, ClauseSupport)>> = None;
    for arm in &live {
        let mut arm_edges: BTreeMap<(ValueId, ValueId), (u128, ClauseSupport)> = BTreeMap::new();
        for (x, y, cost, edge_support) in &arm.edges[base_edges..] {
            arm_edges
                .entry((*x, *y))
                .and_modify(|(best_cost, rows)| {
                    *best_cost = (*best_cost).min(*cost);
                    rows.extend(edge_support.iter().copied());
                })
                .or_insert((*cost, edge_support.clone()));
        }
        match &mut shared_edges {
            None => shared_edges = Some(arm_edges),
            Some(shared) => shared.retain(|edge, (best_cost, rows)| {
                let Some(&(cost, ref edge_support)) = arm_edges.get(edge) else {
                    return false;
                };
                *best_cost = (*best_cost).min(cost);
                rows.extend(edge_support.iter().copied());
                true
            }),
        }
    }
    for ((x, y), (cost, rows)) in shared_edges.unwrap_or_default() {
        scope.edges.push((x, y, cost, rows));
    }
}

/// The rows discharging `premise` under the scope's unconditional content,
/// when the context proves it. An assumed verbatim leaf carries its own
/// rows, `Truth` needs none, a conjunction needs every child discharged, a
/// disjunction needs one achieving child, and a nested implication is
/// shown by its conclusion alone. A relational leaf — scalar or the affine
/// `IntegerMath*` fragment — is proved either by the settled ceilings —
/// `v <= k` holds when `v`'s derived ceiling fits — or by a chain of
/// transfer edges reaching the other side with summed strictness covering
/// the premise's constant, and binds the rows the achieving derivation
/// traversed. Everything else — a literal on the left needing a floor the
/// scope never derives, equality with a literal, or unsolved forms — stays
/// undischarged rather than guessed.
fn premise_support(
    premise: &Proposition,
    scope: &ClauseScope,
    ceilings: &BTreeMap<ValueId, u128>,
    rank_type: IntegerType,
) -> Option<ClauseSupport> {
    if let Some((support, _)) = scope.leaves.iter().find(|(_, leaf)| *leaf == premise) {
        return Some(support.clone());
    }
    match premise {
        Proposition::Truth => Some(ClauseSupport::new()),
        Proposition::Conjunction(children) => {
            let mut support = ClauseSupport::new();
            for child in children {
                support.extend(premise_support(child, scope, ceilings, rank_type)?);
            }
            Some(support)
        }
        Proposition::Disjunction(children) => children
            .iter()
            .find_map(|child| premise_support(child, scope, ceilings, rank_type)),
        // A conclusion that holds unconditionally entails the implication.
        Proposition::Implication { conclusion, .. } => {
            premise_support(conclusion, scope, ceilings, rank_type)
        }
        Proposition::LessOrEqual(left, right) => {
            comparison_support(scope, ceilings, left, right, 0, rank_type)
        }
        Proposition::LessThan(left, right) => {
            comparison_support(scope, ceilings, left, right, 1, rank_type)
        }
        Proposition::Equal(left, right) => {
            match (rank_value(left, rank_type), rank_value(right, rank_type)) {
                (Some(x), Some(y)) => {
                    let mut support = relation_path_support(x, y, 0, scope)?;
                    support.extend(relation_path_support(y, x, 0, scope)?);
                    Some(support)
                }
                _ => None,
            }
        }
        Proposition::IntegerMathLessOrEqual(left, right) => {
            math_premise_support(left, right, false, scope, ceilings)
        }
        Proposition::IntegerMathLessThan(left, right) => {
            math_premise_support(left, right, true, scope, ceilings)
        }
        Proposition::IntegerMathEqual(left, right) => {
            math_equal_premise_support(left, right, scope)
        }
        _ => None,
    }
}

/// Discharge an `IntegerMath*` ordering premise `left <= right`
/// (`strict` false) or `left < right` (`strict` true) under the settled
/// scope. `s*x + o (+ strict) <= k` holds when `x`'s derived ceiling
/// already fits — the bound's achieving rows discharge it. `x + o
/// (+ strict) <= y + r` holds when a transfer chain reaches `y` with
/// summed strictness at least `o - r + strict`. Any other shape — a
/// literal on the left needing a floor, a multi-variable or nonlinear
/// side — stays undischarged rather than guessed.
fn math_premise_support(
    left: &IntegerMathTerm,
    right: &IntegerMathTerm,
    strict: bool,
    scope: &ClauseScope,
    ceilings: &BTreeMap<ValueId, u128>,
) -> Option<ClauseSupport> {
    let (lc, lv, lo) = math_affine(left)?;
    let (rc, rv, ro) = math_affine(right)?;
    match (lv, rv) {
        (Some((_, x)), None) if lc > 0 && rc == 0 => {
            let bound = i128::try_from(*ceilings.get(&x)?).ok()?;
            let reached = i128::try_from(lc)
                .ok()?
                .checked_mul(bound)?
                .checked_add(lo)?;
            let fits = if strict { reached < ro } else { reached <= ro };
            if !fits {
                return None;
            }
            let mut visited = BTreeSet::from([x]);
            justify_requires_ceiling(x, *ceilings.get(&x)?, ceilings, scope, &mut visited)
        }
        (Some((_, x)), Some((_, y))) if lc == 1 && rc == 1 => {
            let needed = lo.checked_sub(ro)?.checked_add(i128::from(strict))?.max(0);
            relation_path_support(x, y, u128::try_from(needed).ok()?, scope)
        }
        (None, None) => {
            let holds = if strict { lo < ro } else { lo <= ro };
            holds.then(ClauseSupport::new)
        }
        _ => None,
    }
}

/// Discharge an `IntegerMathEqual` premise: `x + o = y + r` needs a
/// transfer chain each way — `x + (o - r) <= y` and `y + (r - o) <= x`.
/// An equality on a literal needs an exact value the ceiling derivation
/// never proves from above, so only the verbatim-leaf route discharges
/// it.
fn math_equal_premise_support(
    left: &IntegerMathTerm,
    right: &IntegerMathTerm,
    scope: &ClauseScope,
) -> Option<ClauseSupport> {
    let (lc, lv, lo) = math_affine(left)?;
    let (rc, rv, ro) = math_affine(right)?;
    let (Some((_, x)), Some((_, y))) = (lv, rv) else {
        return None;
    };
    if lc != 1 || rc != 1 {
        return None;
    }
    let mut support = ClauseSupport::new();
    for (from, to, difference) in [(x, y, lo.checked_sub(ro)), (y, x, ro.checked_sub(lo))] {
        let needed = difference?.max(0);
        support.extend(relation_path_support(
            from,
            to,
            u128::try_from(needed).ok()?,
            scope,
        )?);
    }
    Some(support)
}

/// Discharge one relational leaf `left <= right` (`cost` 0) or `left <
/// right` (`cost` 1): a literal right side holds when the value's settled
/// ceiling already fits the bound, and a value right side holds when a
/// transfer chain reaches it — strictness needs the chain to cross at
/// least one strict edge.
fn comparison_support(
    scope: &ClauseScope,
    ceilings: &BTreeMap<ValueId, u128>,
    left: &ScalarTerm,
    right: &ScalarTerm,
    cost: u128,
    rank_type: IntegerType,
) -> Option<ClauseSupport> {
    match (
        rank_value(left, rank_type),
        rank_value(right, rank_type),
        rank_literal(right, rank_type),
    ) {
        (Some(x), _, Some(k)) => {
            let bound = *ceilings.get(&x)?;
            if (cost == 0 && bound > k) || (cost == 1 && bound >= k) {
                return None;
            }
            let mut visited = BTreeSet::from([x]);
            justify_requires_ceiling(x, bound, ceilings, scope, &mut visited)
        }
        (Some(x), Some(y), None) => relation_path_support(x, y, cost, scope),
        _ => None,
    }
}

/// The rows on one achieving transfer chain `from -> to` whose summed
/// strictness reaches `needed`. Every edge subtracts its cost from the
/// target's bound, so a chain of total cost `C` proves `from + C <= to`;
/// the premise `from + needed <= to` follows whenever `C >= needed`.
/// `needed` 0 is bare reachability, 1 is a strict crossing, and larger
/// constants discharge affine premises like `x + 2 <= y`.
fn relation_path_support(
    from: ValueId,
    to: ValueId,
    needed: u128,
    scope: &ClauseScope,
) -> Option<ClauseSupport> {
    let path = relation_path_cost(from, to, needed, &scope.edges)?;
    let mut rows = ClauseSupport::new();
    for index in path {
        rows.extend(scope.edges[index].3.iter().copied());
    }
    Some(rows)
}

/// One edge chain `from -> to` whose costs sum to at least `needed`, as
/// edge positions. `needed` 0 is bare reachability — breadth-first keeps
/// that chain short. Otherwise depth-first enumerates simple paths until
/// one accumulates `needed`: in a satisfiable contract no positive-cost
/// cycle exists, so every achievable total already appears on a simple
/// path, and a contradictory clause that admits one merely fails closed.
/// A bounded expansion budget keeps a dense clause graph from ballooning
/// the search; fixed edge order keeps the recovered chain deterministic.
fn relation_path_cost(
    from: ValueId,
    to: ValueId,
    needed: u128,
    edges: &[(ValueId, ValueId, u128, ClauseSupport)],
) -> Option<Vec<usize>> {
    if needed == 0 {
        return relation_path(from, to, edges);
    }
    if from == to {
        return None;
    }
    let mut stack = vec![(from, 0_u128, Vec::new(), BTreeSet::from([from]))];
    let mut budget = 8192_usize;
    while let Some((node, cost, path, seen)) = stack.pop() {
        budget = budget.checked_sub(1)?;
        for (index, &(a, b, edge_cost, _)) in edges.iter().enumerate() {
            if a != node || seen.contains(&b) {
                continue;
            }
            let next_cost = cost.saturating_add(edge_cost);
            if b == to {
                if next_cost >= needed {
                    let mut achieved = path.clone();
                    achieved.push(index);
                    return Some(achieved);
                }
                // Reaching `to` short of the needed cost is a dead end —
                // a simple path cannot leave and revisit `to`.
                continue;
            }
            let mut next_seen = seen.clone();
            next_seen.insert(b);
            let mut next_path = path.clone();
            next_path.push(index);
            stack.push((b, next_cost, next_path, next_seen));
        }
    }
    None
}

/// One edge chain `from -> to` as edge positions, or `None` when no chain
/// reaches `to`. Breadth-first keeps the recovered chain short — the
/// premise rows a discharged comparison binds stay readable.
fn relation_path(
    from: ValueId,
    to: ValueId,
    edges: &[(ValueId, ValueId, u128, ClauseSupport)],
) -> Option<Vec<usize>> {
    if from == to {
        return Some(Vec::new());
    }
    let mut predecessors: BTreeMap<ValueId, (ValueId, usize)> = BTreeMap::new();
    let mut seen = BTreeSet::from([from]);
    let mut frontier = vec![from];
    while !frontier.is_empty() {
        let mut next_frontier = Vec::new();
        for node in frontier {
            for (index, &(x, y, _, _)) in edges.iter().enumerate() {
                if x != node || !seen.insert(y) {
                    continue;
                }
                predecessors.insert(y, (node, index));
                if y == to {
                    let mut path = vec![index];
                    let mut current = node;
                    while current != from {
                        let (previous, edge) = predecessors[&current];
                        path.push(edge);
                        current = previous;
                    }
                    path.reverse();
                    return Some(path);
                }
                next_frontier.push(y);
            }
        }
        frontier = next_frontier;
    }
    None
}

/// Whether `proposition` admits no valuation under any context — a
/// `Falsehood`, an unsigned term constrained strictly below zero, a
/// literal-literal comparison that is false outright, a conjunction
/// containing any of those, or a disjunction whose arms all fail. Only
/// locally visible contradictions count; a clash the arithmetic does not
/// see keeps the clause live and merely unhelpful.
fn proposition_unsatisfiable(proposition: &Proposition) -> bool {
    let unsigned_zero = |term: &ScalarTerm| match term {
        ScalarTerm::Integer {
            scalar_type,
            value: IntegerValue::Unsigned(0),
        } => scalar_type.sign() == IntegerSign::Unsigned,
        _ => false,
    };
    // A comparison of two closed literals of the same sign is decidable by
    // evaluation — the operand-type rule already pins one sign to both
    // sides, so a cross-sign pair is malformed rather than decidable and
    // stays live. An unsigned literal beyond `i128` declines to evaluate
    // and stays live too.
    let closed = |left: &ScalarTerm, right: &ScalarTerm| match (left, right) {
        (
            ScalarTerm::Integer {
                value: IntegerValue::Unsigned(a),
                ..
            },
            ScalarTerm::Integer {
                value: IntegerValue::Unsigned(b),
                ..
            },
        ) => Some((i128::try_from(*a).ok()?, i128::try_from(*b).ok()?)),
        (
            ScalarTerm::Integer {
                value: IntegerValue::Signed(a),
                ..
            },
            ScalarTerm::Integer {
                value: IntegerValue::Signed(b),
                ..
            },
        ) => Some((*a, *b)),
        _ => None,
    };
    match proposition {
        Proposition::Falsehood => true,
        Proposition::Conjunction(children) => children.iter().any(proposition_unsatisfiable),
        Proposition::Disjunction(children) => children.iter().all(proposition_unsatisfiable),
        Proposition::LessThan(left, right) => {
            unsigned_zero(right) || closed(left, right).is_some_and(|(a, b)| a >= b)
        }
        Proposition::LessOrEqual(left, right) => closed(left, right).is_some_and(|(a, b)| a > b),
        Proposition::Equal(left, right) => closed(left, right).is_some_and(|(a, b)| a != b),
        Proposition::IntegerMathLessThan(left, right) => {
            math_unsatisfiable(left, right, MathRelation::LessThan)
        }
        Proposition::IntegerMathLessOrEqual(left, right) => {
            math_unsatisfiable(left, right, MathRelation::LessOrEqual)
        }
        Proposition::IntegerMathEqual(left, right) => {
            math_unsatisfiable(left, right, MathRelation::Equal)
        }
        _ => false,
    }
}

/// The three decidable `IntegerMath*` orderings for local contradiction
/// checks.
#[derive(Clone, Copy)]
enum MathRelation {
    LessOrEqual,
    LessThan,
    Equal,
}

/// Whether `left OP right` over unbounded integer math admits no
/// valuation — the contradiction must be visible without assuming any
/// ambient fact. A false closed comparison is vacuous outright. A
/// variable side over an unsigned carrier bottoms out at its offset:
/// `s*x + o <= k` needs `o <= k`, `s*x + o < k` needs `o < k`, and
/// `s*x + o = k` needs `(k - o)` nonnegative, divisible by `s`, and
/// inside the carrier's range; a variable on the right instead top-outs
/// at `s*max + o`, which `k` must not exceed (`<=`) or reach (`<`), and
/// equality solves the same quotient. Anything else — a term that does
/// not solve to the affine fragment, a two-variable row, a coefficient
/// the arithmetic cannot lift — stays live rather than guessed.
fn math_unsatisfiable(left: &IntegerMathTerm, right: &IntegerMathTerm, kind: MathRelation) -> bool {
    let Some((lc, lv, lo)) = math_affine(left) else {
        return false;
    };
    let Some((rc, rv, ro)) = math_affine(right) else {
        return false;
    };
    let range = |source_type: IntegerType| -> Option<(i128, i128)> {
        let minimum = match source_type.minimum_value() {
            IntegerValue::Signed(v) => v,
            IntegerValue::Unsigned(v) => i128::try_from(v).ok()?,
        };
        let maximum = match source_type.maximum_value() {
            IntegerValue::Signed(v) => v,
            IntegerValue::Unsigned(v) => i128::try_from(v).ok()?,
        };
        Some((minimum, maximum))
    };
    // Whether `value` can solve `s*value + o = k` inside `source_type`.
    let equality_unsatisfiable =
        |coefficient: u128, offset: i128, literal: i128, source_type: IntegerType| -> bool {
            let Some(difference) = literal.checked_sub(offset) else {
                return true;
            };
            let Ok(divisor) = i128::try_from(coefficient) else {
                return false;
            };
            if divisor <= 0 || difference % divisor != 0 {
                return true;
            }
            let quotient = difference / divisor;
            let Some((minimum, maximum)) = range(source_type) else {
                return false;
            };
            quotient < minimum || quotient > maximum
        };
    match (lv, rv) {
        (None, None) => match kind {
            MathRelation::LessOrEqual => lo > ro,
            MathRelation::LessThan => lo >= ro,
            MathRelation::Equal => lo != ro,
        },
        (Some((source_type, _)), None) if lc > 0 && rc == 0 => match kind {
            // `x >= min` bottoms the side out at `s*min + o`; an unsigned
            // carrier's minimum is zero.
            MathRelation::LessOrEqual => {
                let Some((minimum, _)) = range(source_type) else {
                    return false;
                };
                i128::try_from(lc)
                    .ok()
                    .and_then(|s| s.checked_mul(minimum))
                    .and_then(|m| m.checked_add(lo))
                    .is_some_and(|minimum_side| ro < minimum_side)
            }
            MathRelation::LessThan => {
                let Some((minimum, _)) = range(source_type) else {
                    return false;
                };
                i128::try_from(lc)
                    .ok()
                    .and_then(|s| s.checked_mul(minimum))
                    .and_then(|m| m.checked_add(lo))
                    .is_some_and(|minimum_side| ro <= minimum_side)
            }
            MathRelation::Equal => equality_unsatisfiable(lc, lo, ro, source_type),
        },
        (None, Some((source_type, _))) if rc > 0 && lc == 0 => match kind {
            // `s*y + o` tops out at `s*max + o`.
            MathRelation::LessOrEqual => {
                let Some((_, maximum)) = range(source_type) else {
                    return false;
                };
                i128::try_from(rc)
                    .ok()
                    .and_then(|s| s.checked_mul(maximum))
                    .and_then(|m| m.checked_add(ro))
                    .is_some_and(|maximum_side| lo > maximum_side)
            }
            MathRelation::LessThan => {
                let Some((_, maximum)) = range(source_type) else {
                    return false;
                };
                i128::try_from(rc)
                    .ok()
                    .and_then(|s| s.checked_mul(maximum))
                    .and_then(|m| m.checked_add(ro))
                    .is_some_and(|maximum_side| lo >= maximum_side)
            }
            MathRelation::Equal => equality_unsatisfiable(rc, ro, lo, source_type),
        },
        _ => false,
    }
}

/// The scope's ceiling on `parameter` and the contract rows achieving it.
/// The certificate binds exactly the rows the derived ceiling rests on:
/// recover one achieving simple path — a chain that revisits a value only
/// detours through a cycle deriving nothing. Failure to justify a bound
/// the relaxation computed cannot occur when every ceiling traces to a
/// terminal, but the derivation stays fail-closed rather than publishing a
/// premise set that does not entail the bound.
fn scope_bound(
    scope: &ClauseScope,
    ceilings: &BTreeMap<ValueId, u128>,
    parameter: ValueId,
) -> Option<(u128, ClauseSupport)> {
    let bound = *ceilings.get(&parameter)?;
    let mut visited = BTreeSet::from([parameter]);
    justify_requires_ceiling(parameter, bound, ceilings, scope, &mut visited)
        .map(|support| (bound, support))
}

/// Recover the contract rows one achieved ceiling rests on: at each value
/// the earliest literal ceiling stating its bound, else the earliest
/// relational edge whose target's own bound transfers it. The returned set
/// is the traversing chain's rows alone — a failed branch returns `None`
/// with nothing added, so rows shared with a live sibling edge are never
/// retracted by a detour's cleanup. `visited` keeps the chain simple — a
/// revisit means this branch detoured through a cycle that derives nothing
/// the shorter chain did not.
fn justify_requires_ceiling(
    node: ValueId,
    residual: u128,
    ceilings: &BTreeMap<ValueId, u128>,
    scope: &ClauseScope,
    visited: &mut BTreeSet<ValueId>,
) -> Option<ClauseSupport> {
    if let Some((_, _, rows)) = scope
        .terminals
        .iter()
        .find(|(value, bound, _)| *value == node && *bound == residual)
    {
        return Some(rows.clone());
    }
    let mut candidates: Vec<(usize, usize, ValueId)> = scope
        .edges
        .iter()
        .enumerate()
        .filter(|(_, (x, y, cost, _))| {
            *x == node
                && ceilings
                    .get(y)
                    .and_then(|bound| bound.checked_sub(*cost))
                    .is_some_and(|candidate| candidate == residual)
        })
        .map(|(index, (_, y, _, rows))| {
            (rows.iter().next().copied().unwrap_or(usize::MAX), index, *y)
        })
        .collect();
    candidates.sort_unstable();
    for (_, index, y) in candidates {
        if !visited.insert(y) {
            continue;
        }
        if let Some(mut support) =
            justify_requires_ceiling(y, ceilings[&y], ceilings, scope, visited)
        {
            support.extend(scope.edges[index].3.iter().copied());
            return Some(support);
        }
        visited.remove(&y);
    }
    None
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
        return Err(condensed_cycle_report(machine, node));
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
            // The certificate's ceiling is a `u64` scalar: an interior the
            // rank arithmetic derived but the ceiling cannot state means
            // this component's rank supplies no publishable bound — the
            // spec's unbounded-rank cause names this component, not a flat
            // overflow.
            if units > u128::from(u64::MAX) {
                return Err(unbounded_rank_report(machine, component));
            }
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
        return Err(condensed_cycle_report(machine, node));
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
            // The certificate's ceiling is a `u64` scalar: an interior the
            // rank arithmetic derived but the ceiling cannot state means
            // this component's rank supplies no publishable bound.
            if interior.is_some_and(|units| units > u128::from(u64::MAX)) {
                return Err(unbounded_rank_report(machine, component));
            }
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

/// Closure failures on a component member name the verifier-derived
/// component identity and a directed cause — the responsible edge plus its
/// cyclic component — instead of a flat operation report. An open
/// descriptor-table callee (`InvocationBoundCallee`) becomes
/// `OpenCalleeSet`; every other error already names its own obstruction or
/// marks malformed input, and passes through unchanged. Blocks outside every
/// component keep the flat error — no component exists to cite.
fn member_closure_error(
    machine: &TerminalMachine,
    block: BlockId,
    member_of: &BTreeMap<BlockId, usize>,
    components: &[TerminalNaturalCycle],
    error: FixedFuelError,
) -> FixedFuelError {
    let Some(&index) = member_of.get(&block) else {
        return error;
    };
    match error {
        FixedFuelError::InvocationBoundCallee { operation, .. } => {
            let members: Vec<BlockId> = components[index]
                .ranks
                .iter()
                .map(|rank| rank.block)
                .collect();
            FixedFuelError::UnboundedCycleComponent {
                component: terminal_verifier::cyclic_component_identity(machine, &members),
                cause: UnboundedCycleCause::OpenCalleeSet { operation },
            }
        }
        other => other,
    }
}

/// Directed absence-of-bound report for a condensed-graph revisit: an
/// ordinary block the retained partition did not cover sits on an unranked
/// cyclic component, so the report names that verifier-derived component
/// with the `Unranked` cause. A revisited component node means the
/// condensed graph itself cycles — the retained partition is malformed,
/// not unbounded, so the report is the broken invariant instead.
fn condensed_cycle_report(machine: &TerminalMachine, node: NaturalGraphNode) -> FixedFuelError {
    match node {
        NaturalGraphNode::Block(block) => unbounded_cycle_report(machine, block),
        NaturalGraphNode::Component(_) => FixedFuelError::InvalidRankedScc(machine.id),
    }
}

/// Directed absence-of-bound report for a ranked component whose
/// rank-derived interior bound cannot be represented in the certificate's
/// scalar ceiling — the spec's `unbounded rank` cause: the ranking row
/// exists but the visit arithmetic it drives supplies no publishable bound.
pub(super) fn unbounded_rank_report(
    machine: &TerminalMachine,
    component: &TerminalNaturalCycle,
) -> FixedFuelError {
    let members: Vec<BlockId> = component.ranks.iter().map(|rank| rank.block).collect();
    FixedFuelError::UnboundedCycleComponent {
        component: terminal_verifier::cyclic_component_identity(machine, &members),
        cause: UnboundedCycleCause::UnboundedRank,
    }
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
