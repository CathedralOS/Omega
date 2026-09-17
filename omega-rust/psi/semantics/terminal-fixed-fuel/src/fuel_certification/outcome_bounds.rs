//! Outcome-sensitive machine and control-flow composition.

use crate::FixedFuelError;
use semantic_vocabulary::{BlockId, BoundaryMachineId, IntegerValue, MachineId, OperationId};
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

/// Maximum entry-to-outcome bound for a verified `Natural`-ranked machine.
///
/// The verifier's components partition the machine's complete cyclic topology.
/// Within one component every cycle crosses a strict edge that decreases an
/// unsigned rank value, while preserving edges alone cannot close a cycle.
/// Each member block therefore executes at most `rank_maximum + 1` times per
/// component entry, and a component cannot be re-entered once left (an edge
/// back would make the outside block part of the same component). The
/// condensation of components and ordinary blocks is acyclic, so a
/// longest-path bound over it covers every admitted execution's operation,
/// call, edge, and cleanup costs. The rank bound is the carrier's type
/// maximum, covering every input the admitted ranking can take.
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
    let mut member_of = BTreeMap::new();
    for (index, component) in components.iter().enumerate() {
        for rank in &component.ranks {
            if member_of.insert(rank.block, index).is_some() {
                return Err(FixedFuelError::InvalidRankedScc(machine.id));
            }
        }
    }
    let mut visit_units = BTreeMap::new();
    for block in &machine.blocks {
        visit_units.insert(
            block.id,
            block_visit_units(
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
    let mut component_units = Vec::with_capacity(components.len());
    for component in components {
        let IntegerValue::Unsigned(rank_maximum) = component.rank_type.maximum_value() else {
            return Err(FixedFuelError::InvalidRankedScc(machine.id));
        };
        let member_units = component.ranks.iter().try_fold(0_u128, |units, rank| {
            let visit = visit_units
                .get(&rank.block)
                .copied()
                .ok_or(FixedFuelError::UnknownBlock(rank.block))?;
            units
                .checked_add(u128::from(visit))
                .ok_or(FixedFuelError::BoundOverflow)
        })?;
        component_units.push(
            rank_maximum
                .checked_add(1)
                .and_then(|visits| visits.checked_mul(member_units))
                .ok_or(FixedFuelError::BoundOverflow)?,
        );
    }
    let entry_node = member_of
        .get(&machine.entry)
        .map_or(NaturalGraphNode::Block(machine.entry), |&index| {
            NaturalGraphNode::Component(index)
        });
    let bound = natural_condensed_bound(
        entry_node,
        machine,
        components,
        &member_of,
        &visit_units,
        &component_units,
        blocks,
        &mut BTreeMap::new(),
        &mut BTreeSet::new(),
    )?;
    let ceiling = u64::try_from(bound).map_err(|_| FixedFuelError::BoundOverflow)?;
    // One ceiling covers both outcomes: the bound counts every admitted path,
    // so it upper-bounds returning and crashing paths alike.
    Ok(OutcomeBounds {
        returned: Some(ceiling),
        crashed: Some(ceiling),
    })
}

/// One condensed node: an ordinary block or a complete cyclic component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum NaturalGraphNode {
    Block(BlockId),
    Component(usize),
}

/// Longest-path bound over the condensed control DAG. A verified component
/// partition makes this graph acyclic; the active set is defense against
/// malformed input, not a search mechanism.
#[allow(clippy::too_many_arguments)]
fn natural_condensed_bound(
    node: NaturalGraphNode,
    machine: &TerminalMachine,
    components: &[TerminalNaturalCycle],
    member_of: &BTreeMap<BlockId, usize>,
    visit_units: &BTreeMap<BlockId, u64>,
    component_units: &[u128],
    blocks: &BTreeMap<BlockId, &terminal_psi::Block>,
    memoized: &mut BTreeMap<NaturalGraphNode, u128>,
    active: &mut BTreeSet<NaturalGraphNode>,
) -> Result<u128, FixedFuelError> {
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
    let (self_units, successors) = match node {
        NaturalGraphNode::Block(block) => {
            let block_semantics = blocks
                .get(&block)
                .copied()
                .ok_or(FixedFuelError::UnknownBlock(block))?;
            (
                u128::from(
                    visit_units
                        .get(&block)
                        .copied()
                        .ok_or(FixedFuelError::UnknownBlock(block))?,
                ),
                terminator_targets(&block_semantics.terminator),
            )
        }
        NaturalGraphNode::Component(index) => {
            let component = components
                .get(index)
                .ok_or(FixedFuelError::InvalidRankedScc(machine.id))?;
            let mut exits = Vec::new();
            for rank in &component.ranks {
                let block = blocks
                    .get(&rank.block)
                    .copied()
                    .ok_or(FixedFuelError::UnknownBlock(rank.block))?;
                exits.extend(
                    terminator_targets(&block.terminator)
                        .into_iter()
                        .filter(|target| member_of.get(target) != Some(&index)),
                );
            }
            (
                component_units
                    .get(index)
                    .copied()
                    .ok_or(FixedFuelError::InvalidRankedScc(machine.id))?,
                exits,
            )
        }
    };
    let mut continuation = 0_u128;
    for target in successors {
        let next = member_of
            .get(&target)
            .map_or(NaturalGraphNode::Block(target), |&index| {
                NaturalGraphNode::Component(index)
            });
        continuation = continuation.max(natural_condensed_bound(
            next,
            machine,
            components,
            member_of,
            visit_units,
            component_units,
            blocks,
            memoized,
            active,
        )?);
    }
    let bound = self_units
        .checked_add(continuation)
        .ok_or(FixedFuelError::BoundOverflow)?;
    active.remove(&node);
    memoized.insert(node, bound);
    Ok(bound)
}

/// Maximum work one execution of `block` can charge: every operation, each
/// call's worst outcome (return or crash; a crash ends the path, so one
/// charge covers it), the terminator edge, and nominal cleanup machines the
/// terminator invokes.
fn block_visit_units(
    machine: &TerminalMachine,
    block: &terminal_psi::Block,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    dynamic_call_targets: &BTreeMap<(MachineId, OperationId), MachineId>,
    provider_candidates: &BTreeMap<BoundaryMachineId, Vec<MachineId>>,
    schedule: TerminalFuelSchedule,
    memoized_machines: &mut BTreeMap<MachineId, OutcomeBounds>,
    active_machines: &mut BTreeSet<MachineId>,
) -> Result<u64, FixedFuelError> {
    let mut units = block
        .operations
        .iter()
        .try_fold(0_u64, |units, operation| {
            units
                .checked_add(schedule.operation_units(&operation.kind))
                .ok_or(FixedFuelError::BoundOverflow)
        })?;
    for operation in &block.operations {
        // The possible callees are mutually exclusive dispatch outcomes:
        // one visit invokes at most one of them, so the visit bound takes
        // the maximum rather than the sum. A candidate with no terminal
        // path admits an unbounded installation, so it rejects outright.
        let mut invoked = 0_u64;
        for callee in operation_callees(
            machine.id,
            operation,
            dynamic_call_targets,
            provider_candidates,
        )? {
            let callee_bounds = maximum_machine_outcomes(
                callee,
                machines,
                dynamic_call_targets,
                provider_candidates,
                schedule,
                memoized_machines,
                active_machines,
            )?;
            invoked = invoked.max(
                callee_bounds
                    .maximum()
                    .ok_or(FixedFuelError::NoTerminalPath(callee))?,
            );
        }
        units = units
            .checked_add(invoked)
            .ok_or(FixedFuelError::BoundOverflow)?;
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
        units = units
            .checked_add(
                cleanup_bounds
                    .maximum()
                    .ok_or(FixedFuelError::NoTerminalPath(cleanup_machine))?,
            )
            .ok_or(FixedFuelError::BoundOverflow)?;
    }
    Ok(units)
}

fn terminator_targets(terminator: &Terminator) -> Vec<BlockId> {
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
        return Err(FixedFuelError::ControlCycle(current));
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
