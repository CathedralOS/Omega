#![forbid(unsafe_code)]

//! Recomputable restricted fixed-fuel certificates for terminal Psi.
//!
//! The terminal verifier accepts acyclic control flow, so the checker derives
//! an exact maximum entry-to-terminal-exit cost without precondition assumptions
//! and partitions the complete reachable graph at every reachable explicit edge.

use std::collections::{BTreeMap, BTreeSet};

use psi_core::{BlockId, EdgeId, MachineId, Proposition};
use psi_terminal::{
    OperationKind, TerminalAffineCleanupAction, TerminalMachine, TerminalModule, Terminator,
};
use psi_terminal_codec::{CodecError, TerminalPsiIdentity, terminal_psi_identity};
use psi_terminal_fuel::{FuelScheduleIdentity, TerminalFuelSchedule};
use psi_terminal_verifier::VerifiedTerminalModule;

/// Exact restricted theorem: every path from one machine entry reaches a
/// return or crash within the published current logical-fuel ceiling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedEntryFuelCertificate {
    terminal_psi: TerminalPsiIdentity,
    schedule: FuelScheduleIdentity,
    entry: MachineId,
    relevant_preconditions: Vec<Proposition>,
    ceiling_units: u64,
}

/// Exact current-vocabulary theorem for one selected machine-local path
/// segment. The segment begins before the first operation in `start_block` and
/// includes the charged `end_edge`; its endpoint may be either a jump or a
/// return. A later safe-point classifier can select eligible endpoints without
/// changing this recomputable accounting primitive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedSegmentFuelCertificate {
    terminal_psi: TerminalPsiIdentity,
    schedule: FuelScheduleIdentity,
    machine: MachineId,
    start_block: BlockId,
    end_edge: EdgeId,
    relevant_preconditions: Vec<Proposition>,
    ceiling_units: u64,
}

impl FixedSegmentFuelCertificate {
    pub const fn terminal_psi(&self) -> TerminalPsiIdentity {
        self.terminal_psi
    }

    pub const fn schedule(&self) -> FuelScheduleIdentity {
        self.schedule
    }

    pub const fn machine(&self) -> MachineId {
        self.machine
    }

    pub const fn start_block(&self) -> BlockId {
        self.start_block
    }

    pub const fn end_edge(&self) -> EdgeId {
        self.end_edge
    }

    pub fn relevant_preconditions(&self) -> &[Proposition] {
        &self.relevant_preconditions
    }

    pub const fn ceiling_units(&self) -> u64 {
        self.ceiling_units
    }
}

impl FixedEntryFuelCertificate {
    pub const fn terminal_psi(&self) -> TerminalPsiIdentity {
        self.terminal_psi
    }

    pub const fn schedule(&self) -> FuelScheduleIdentity {
        self.schedule
    }

    pub const fn entry(&self) -> MachineId {
        self.entry
    }

    pub fn relevant_preconditions(&self) -> &[Proposition] {
        &self.relevant_preconditions
    }

    pub const fn ceiling_units(&self) -> u64 {
        self.ceiling_units
    }
}

/// Derive the exact current-slice bound from a verified canonical semantic
/// module. The checker performs no search and depends on no producing compiler
/// state or proof-bundle representation.
pub fn derive_fixed_entry_fuel(
    verified: &VerifiedTerminalModule<'_>,
    entry: MachineId,
) -> Result<FixedEntryFuelCertificate, FixedFuelError> {
    let module = verified.module();
    let terminal_psi = terminal_psi_identity(module).map_err(FixedFuelError::SemanticIdentity)?;
    let machine = module
        .machines
        .iter()
        .find(|machine| machine.id == entry)
        .ok_or(FixedFuelError::UnknownEntry(entry))?;
    let ceiling_units = derive_maximum_entry_bound(module, machine.id)?;
    Ok(FixedEntryFuelCertificate {
        terminal_psi,
        schedule: TerminalFuelSchedule::CURRENT.identity(),
        entry,
        // Current control and operation costs do not depend on values. The
        // theorem therefore holds for every invocation admitted by the
        // machine contract and needs no additional premise subset.
        relevant_preconditions: Vec::new(),
        ceiling_units,
    })
}

/// Recompute and compare every public certificate field. Consumers never need
/// to trust a producing compiler's claimed ceiling.
pub fn validate_fixed_entry_fuel(
    verified: &VerifiedTerminalModule<'_>,
    certificate: &FixedEntryFuelCertificate,
) -> Result<(), FixedFuelError> {
    let expected = derive_fixed_entry_fuel(verified, certificate.entry)?;
    if expected != *certificate {
        return Err(FixedFuelError::CertificateMismatch);
    }
    Ok(())
}

/// Derive an exact bound for one selected acyclic path segment. The charged
/// endpoint is part of the segment so adjacent certificates neither omit nor
/// double-charge an edge. A conditional edge can be an endpoint; crossing an
/// unresolved conditional without selecting its successor fails closed.
pub fn derive_fixed_segment_fuel(
    verified: &VerifiedTerminalModule<'_>,
    machine: MachineId,
    start_block: BlockId,
    end_edge: EdgeId,
) -> Result<FixedSegmentFuelCertificate, FixedFuelError> {
    let module = verified.module();
    let terminal_psi = terminal_psi_identity(module).map_err(FixedFuelError::SemanticIdentity)?;
    let machine_semantics = module
        .machines
        .iter()
        .find(|candidate| candidate.id == machine)
        .ok_or(FixedFuelError::UnknownEntry(machine))?;
    let ceiling_units = derive_segment_bound(module, machine_semantics, start_block, end_edge)?;
    Ok(FixedSegmentFuelCertificate {
        terminal_psi,
        schedule: TerminalFuelSchedule::CURRENT.identity(),
        machine,
        start_block,
        end_edge,
        // Current operation and edge costs are value-independent.
        relevant_preconditions: Vec::new(),
        ceiling_units,
    })
}

/// Recompute every public segment field from independently verified terminal
/// semantics.
pub fn validate_fixed_segment_fuel(
    verified: &VerifiedTerminalModule<'_>,
    certificate: &FixedSegmentFuelCertificate,
) -> Result<(), FixedFuelError> {
    let expected = derive_fixed_segment_fuel(
        verified,
        certificate.machine,
        certificate.start_block,
        certificate.end_edge,
    )?;
    if expected != *certificate {
        return Err(FixedFuelError::CertificateMismatch);
    }
    Ok(())
}

/// Select the complete current-vocabulary safe-point partition for one
/// machine. Every reachable explicit jump, conditional, return, or local crash
/// edge is a semantic safe point in this slice. Calls compose the callee's
/// normal-return bound into a following caller edge and terminate separately
/// at a callee crash; an all-crash call therefore makes the caller terminator
/// unreachable. No partial call or suspension state crosses an edge, and the
/// successor block begins the next segment.
/// The returned order is canonical block order followed by terminator edge
/// order, restricted to blocks reachable from the machine entry.
pub fn derive_fixed_safe_point_segments(
    verified: &VerifiedTerminalModule<'_>,
    machine: MachineId,
) -> Result<Vec<FixedSegmentFuelCertificate>, FixedFuelError> {
    let machine_semantics = verified
        .module()
        .machines
        .iter()
        .find(|candidate| candidate.id == machine)
        .ok_or(FixedFuelError::UnknownEntry(machine))?;
    let blocks = machine_semantics
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect::<BTreeMap<_, _>>();
    let machines = verified
        .module()
        .machines
        .iter()
        .map(|machine| (machine.id, machine))
        .collect::<BTreeMap<_, _>>();
    let schedule = TerminalFuelSchedule::CURRENT;
    let mut memoized_machines = BTreeMap::new();
    let mut active_machines = BTreeSet::from([machine]);
    let mut reachable = BTreeSet::new();
    let mut reachable_terminators = BTreeSet::new();
    let mut pending = vec![machine_semantics.entry];
    while let Some(current) = pending.pop() {
        if !reachable.insert(current) {
            continue;
        }
        let block = blocks
            .get(&current)
            .copied()
            .ok_or(FixedFuelError::UnknownBlock(current))?;
        let mut terminator_reachable = true;
        for operation in &block.operations {
            if let OperationKind::Call { callee, .. }
            | OperationKind::CallUnit { callee, .. }
            | OperationKind::CallStructuralScalar { callee, .. }
            | OperationKind::CallStructural { callee, .. } = &operation.kind
            {
                let callee_bounds = maximum_machine_outcomes(
                    *callee,
                    &machines,
                    schedule,
                    &mut memoized_machines,
                    &mut active_machines,
                )?;
                if callee_bounds.returned.is_none() {
                    terminator_reachable = false;
                    break;
                }
            }
        }
        if !terminator_reachable {
            continue;
        }
        reachable_terminators.insert(current);
        match &block.terminator {
            Terminator::Jump { target, .. } => pending.push(*target),
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                pending.push(when_false.target);
                pending.push(when_true.target);
            }
            Terminator::Return { .. }
            | Terminator::ReturnUnit { .. }
            | Terminator::ReturnUnitPartialAffine { .. }
            | Terminator::ReturnUnitNominalAffine { .. }
            | Terminator::ReturnStructural { .. }
            | Terminator::Crash { .. } => {}
        }
    }

    let mut segments = Vec::new();
    for block in &machine_semantics.blocks {
        if !reachable_terminators.contains(&block.id) {
            continue;
        }
        for edge in block.terminator.edges() {
            segments.push(derive_fixed_segment_fuel(
                verified, machine, block.id, edge,
            )?);
        }
    }
    Ok(segments)
}

/// Recompute the whole ordered safe-point partition. Validating certificates
/// one at a time is insufficient because a producer could omit a reachable
/// segment or present a different order.
pub fn validate_fixed_safe_point_segments(
    verified: &VerifiedTerminalModule<'_>,
    machine: MachineId,
    certificates: &[FixedSegmentFuelCertificate],
) -> Result<(), FixedFuelError> {
    let expected = derive_fixed_safe_point_segments(verified, machine)?;
    if expected != certificates {
        return Err(FixedFuelError::CertificateMismatch);
    }
    Ok(())
}

fn derive_maximum_entry_bound(
    module: &TerminalModule,
    machine: MachineId,
) -> Result<u64, FixedFuelError> {
    let schedule = TerminalFuelSchedule::CURRENT;
    let machines = module
        .machines
        .iter()
        .map(|machine| (machine.id, machine))
        .collect::<BTreeMap<_, _>>();
    maximum_machine_outcomes(
        machine,
        &machines,
        schedule,
        &mut BTreeMap::new(),
        &mut BTreeSet::new(),
    )?
    .maximum()
    .ok_or(FixedFuelError::NoTerminalPath(machine))
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct OutcomeBounds {
    returned: Option<u64>,
    crashed: Option<u64>,
}

impl OutcomeBounds {
    fn maximum(self) -> Option<u64> {
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

fn maximum_optional(left: Option<u64>, right: Option<u64>) -> Option<u64> {
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

fn maximum_machine_outcomes(
    machine: MachineId,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
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
    let machine_semantics = machines
        .get(&machine)
        .copied()
        .ok_or(FixedFuelError::UnknownEntry(machine))?;
    let blocks = machine_semantics
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect::<BTreeMap<_, _>>();
    outcome_bounds_from(
        machine_semantics.entry,
        &blocks,
        machines,
        schedule,
        &mut BTreeMap::new(),
        &mut BTreeSet::new(),
        memoized_machines,
        active_machines,
    )
    .inspect(|bounds| {
        active_machines.remove(&machine);
        memoized_machines.insert(machine, *bounds);
    })
}

fn outcome_bounds_from(
    current: BlockId,
    blocks: &BTreeMap<BlockId, &psi_terminal::Block>,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
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
        if let OperationKind::Call { callee, .. }
        | OperationKind::CallUnit { callee, .. }
        | OperationKind::CallStructuralScalar { callee, .. }
        | OperationKind::CallStructural { callee, .. } = &operation.kind
        {
            let callee_bounds = maximum_machine_outcomes(
                *callee,
                machines,
                schedule,
                memoized_machines,
                active_machines,
            )?;
            if let Some(prefix) = normal_units {
                crash_units = maximum_optional(
                    crash_units,
                    checked_optional_add(callee_bounds.crashed, prefix)?,
                );
                normal_units = checked_optional_add(callee_bounds.returned, prefix)?;
            }
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
            *target,
            blocks,
            machines,
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
            when_true.target,
            blocks,
            machines,
            schedule,
            memoized,
            active,
            memoized_machines,
            active_machines,
        )?
        .merge(outcome_bounds_from(
            when_false.target,
            blocks,
            machines,
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
    };
    active.remove(&current);
    let bounds = OutcomeBounds {
        returned: continued.returned,
        crashed: maximum_optional(crash_units, continued.crashed),
    };
    memoized.insert(current, bounds);
    Ok(bounds)
}

fn compose_cleanup_outcomes(
    cleanup_machines: impl IntoIterator<Item = MachineId>,
    mut bounds: OutcomeBounds,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
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
            schedule,
            memoized_machines,
            active_machines,
        )?;
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

fn derive_segment_bound(
    module: &TerminalModule,
    machine: &TerminalMachine,
    start_block: BlockId,
    end_edge: EdgeId,
) -> Result<u64, FixedFuelError> {
    let schedule = TerminalFuelSchedule::CURRENT;
    let machines = module
        .machines
        .iter()
        .map(|machine| (machine.id, machine))
        .collect::<BTreeMap<_, _>>();
    let mut memoized_machines = BTreeMap::new();
    let mut active_machines = BTreeSet::from([machine.id]);
    let blocks = machine
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect::<BTreeMap<_, _>>();
    if !blocks.contains_key(&start_block) {
        return Err(FixedFuelError::UnknownBlock(start_block));
    }
    let mut visited = BTreeSet::new();
    let mut current = start_block;
    let mut units = 0_u64;

    loop {
        if !visited.insert(current) {
            return Err(FixedFuelError::ControlCycle(current));
        }
        let block = blocks
            .get(&current)
            .copied()
            .ok_or(FixedFuelError::UnknownBlock(current))?;
        for operation in &block.operations {
            units = units
                .checked_add(schedule.operation_units(&operation.kind))
                .ok_or(FixedFuelError::BoundOverflow)?;
            if let OperationKind::Call { callee, .. }
            | OperationKind::CallUnit { callee, .. }
            | OperationKind::CallStructuralScalar { callee, .. }
            | OperationKind::CallStructural { callee, .. } = &operation.kind
            {
                let callee_bounds = maximum_machine_outcomes(
                    *callee,
                    &machines,
                    schedule,
                    &mut memoized_machines,
                    &mut active_machines,
                )?;
                units = units
                    .checked_add(callee_bounds.returned.ok_or(
                        FixedFuelError::SegmentEndUnreachableAfterCall {
                            block: current,
                            callee: *callee,
                        },
                    )?)
                    .ok_or(FixedFuelError::BoundOverflow)?;
            }
        }
        units = units
            .checked_add(schedule.terminator_units(&block.terminator))
            .ok_or(FixedFuelError::BoundOverflow)?;
        if block.terminator.edges().any(|edge| edge == end_edge) {
            return Ok(units);
        }
        match block.terminator {
            Terminator::Jump { target, .. } => current = target,
            Terminator::Conditional { .. } => {
                return Err(FixedFuelError::BranchingNotYetSupported(current));
            }
            Terminator::Return { edge, .. } => {
                return Err(FixedFuelError::SegmentEndNotReached {
                    requested: end_edge,
                    reached_terminal: edge,
                });
            }
            Terminator::ReturnUnit { edge, .. } => {
                return Err(FixedFuelError::SegmentEndNotReached {
                    requested: end_edge,
                    reached_terminal: edge,
                });
            }
            Terminator::ReturnUnitPartialAffine { edge, .. } => {
                return Err(FixedFuelError::SegmentEndNotReached {
                    requested: end_edge,
                    reached_terminal: edge,
                });
            }
            Terminator::ReturnUnitNominalAffine { edge, .. } => {
                return Err(FixedFuelError::SegmentEndNotReached {
                    requested: end_edge,
                    reached_terminal: edge,
                });
            }
            Terminator::ReturnStructural { edge, .. } => {
                return Err(FixedFuelError::SegmentEndNotReached {
                    requested: end_edge,
                    reached_terminal: edge,
                });
            }
            Terminator::Crash { edge, .. } => {
                return Err(FixedFuelError::SegmentEndNotReached {
                    requested: end_edge,
                    reached_terminal: edge,
                });
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixedFuelError {
    SemanticIdentity(CodecError),
    UnknownEntry(MachineId),
    UnknownBlock(BlockId),
    ControlCycle(BlockId),
    CallCycle(MachineId),
    BranchingNotYetSupported(BlockId),
    SegmentEndNotReached {
        requested: EdgeId,
        reached_terminal: EdgeId,
    },
    SegmentEndUnreachableAfterCall {
        block: BlockId,
        callee: MachineId,
    },
    NoTerminalPath(MachineId),
    BoundOverflow,
    CertificateMismatch,
}

impl std::fmt::Display for FixedFuelError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for FixedFuelError {}
