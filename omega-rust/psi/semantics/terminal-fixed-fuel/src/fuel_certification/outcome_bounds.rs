//! Outcome-sensitive machine and control-flow composition.

use crate::{FixedFuelError, UnboundedCycleCause};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, EdgeId, IntegerValue, MachineId, OperationId,
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
/// carrier's type maximum, covering every input the admitted ranking can
/// take, and the certificate's merged bound is the maximum of the two
/// outcome classes rather than a single ceiling billed against both.
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
/// return, `visit_units_crashed` is `None` when no traversal of it can
/// crash, and a component's returned bound sums only the members a
/// completing walk can still traverse at the rank carrier's type maximum
/// plus one visits each. The completing-member sum doubles as the
/// crash-bound interior: a crash-terminal walk's visits complete until the
/// one crashing visit ends it.
pub(super) struct NaturalGeometry {
    pub(super) member_of: BTreeMap<BlockId, usize>,
    pub(super) visit_units_returned: BTreeMap<BlockId, Option<u64>>,
    pub(super) visit_units_crashed: BTreeMap<BlockId, Option<u64>>,
    pub(super) component_units_returned: Vec<Option<u128>>,
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
/// crashing visit, and a component multiplies only the members a completing
/// walk can traverse — the same discipline the acyclic derivation and the
/// segment partition already apply to `OutcomeBounds::returned`, extended
/// here to the crash outcome.
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
    let mut component_units_returned = Vec::with_capacity(components.len());
    for component in components {
        let IntegerValue::Unsigned(rank_maximum) = component.rank_type.maximum_value() else {
            return Err(FixedFuelError::InvalidRankedScc(machine.id));
        };
        // A completing walk never finishes a visit to a member whose own
        // visit cannot return, so non-returning members contribute nothing.
        // When no member can complete at all the component admits no
        // commit-reachable traversal and reports `None`.
        let member_units_returned = component.ranks.iter().try_fold(0_u128, |units, rank| {
            let visit = visit_units_returned
                .get(&rank.block)
                .copied()
                .ok_or(FixedFuelError::UnknownBlock(rank.block))?
                .unwrap_or(0);
            units
                .checked_add(u128::from(visit))
                .ok_or(FixedFuelError::BoundOverflow)
        })?;
        component_units_returned.push(if member_units_returned == 0 {
            None
        } else {
            Some(
                rank_maximum
                    .checked_add(1)
                    .and_then(|visits| visits.checked_mul(member_units_returned))
                    .ok_or(FixedFuelError::BoundOverflow)?,
            )
        });
    }
    Ok(NaturalGeometry {
        member_of,
        visit_units_returned,
        visit_units_crashed,
        component_units_returned,
    })
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
/// component contributes its returned-member bound, and a component's exits
/// are taken only through members whose own visit can return — a member
/// whose call always crashes still ends every walk that reaches it, so its
/// edges commit nothing a returning walk can use. `None` propagates as "no
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
/// plus a successor's crash bound. A component contributes the
/// completing-member interior at the rank ceiling plus one crash visit
/// when any member traversal can crash — a crash ends the walk, so a
/// crashable member is visited at most once, never at the rank multiplier
/// the old whole-graph ceiling billed — or the same interior plus the
/// worst exit's crash continuation, exits riding only members whose visit
/// can complete. `None` propagates as "no crash-terminal walk": a machine
/// that can only return reports no crash outcome at all, so callers never
/// inherit crash work the callee cannot commit.
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
            // The completing-member interior: visits on a crash-terminal
            // walk complete until the one crashing visit ends it, so every
            // member still bills at most `rank_maximum + 1` visits and a
            // member that can never complete contributes nothing.
            let interior = geometry
                .component_units_returned
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
