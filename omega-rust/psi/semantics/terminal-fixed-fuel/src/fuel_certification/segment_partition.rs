//! Invocation-local preparation and complete reachable segment partition.

use super::outcome_bounds::{
    OutcomeBounds, boundary_call_candidates, compose_cleanup_outcomes, dynamic_call_targets,
    maximum_machine_outcomes, maximum_optional, operation_callees, terminator_cleanup_machines,
};
use crate::{FixedFuelError, FixedSegmentFuelCertificate};
use semantic_vocabulary::{BlockId, BoundaryMachineId, EdgeId, MachineId, OperationId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_codec::{TerminalPsiIdentity, terminal_psi_identity};
use terminal_fuel::TerminalFuelSchedule;
use terminal_psi::{Block, TerminalMachine, TerminalModule, Terminator};

#[cfg(test)]
thread_local! {
    pub(super) static IDENTITIES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static PREPARATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Module facts and lazily requested identity belong to this exact invocation.
/// Lazy identity preserves rejection order and permits unsealed empty rosters.
pub(super) struct PreparedFuelModule<'module> {
    module: &'module TerminalModule,
    terminal_psi: std::cell::OnceCell<Result<TerminalPsiIdentity, terminal_codec::CodecError>>,
    machines: BTreeMap<MachineId, &'module TerminalMachine>,
    dynamic_call_targets: BTreeMap<(MachineId, OperationId), MachineId>,
    provider_candidates: BTreeMap<BoundaryMachineId, Vec<MachineId>>,
}

impl<'module> PreparedFuelModule<'module> {
    pub(super) fn new(module: &'module TerminalModule) -> Self {
        #[cfg(test)]
        PREPARATIONS.with(|count| count.set(count.get() + 1));
        Self {
            module,
            terminal_psi: std::cell::OnceCell::new(),
            machines: module
                .machines
                .iter()
                .map(|machine| (machine.id, machine))
                .collect(),
            dynamic_call_targets: dynamic_call_targets(module),
            provider_candidates: boundary_call_candidates(module),
        }
    }

    pub(super) fn identity(&self) -> Result<TerminalPsiIdentity, FixedFuelError> {
        self.terminal_psi
            .get_or_init(|| {
                #[cfg(test)]
                IDENTITIES.with(|count| count.set(count.get() + 1));
                terminal_psi_identity(self.module)
            })
            .clone()
            .map_err(FixedFuelError::SemanticIdentity)
    }
}

/// Selected-machine geometry borrows immutable module preparation. Outcome
/// summaries remain separate mutable working state for each full roster replay.
pub(super) struct PreparedSegments<'prepared, 'module> {
    subject: &'prepared PreparedFuelModule<'module>,
    machine: &'module TerminalMachine,
    blocks: BTreeMap<BlockId, &'module Block>,
}

impl<'prepared, 'module> PreparedSegments<'prepared, 'module> {
    pub(super) fn new(
        subject: &'prepared PreparedFuelModule<'module>,
        machine: MachineId,
    ) -> Result<Self, FixedFuelError> {
        let machine = subject
            .machines
            .get(&machine)
            .copied()
            .ok_or(FixedFuelError::UnknownEntry(machine))?;
        Ok(Self {
            subject,
            machine,
            blocks: machine
                .blocks
                .iter()
                .map(|block| (block.id, block))
                .collect(),
        })
    }

    pub(super) fn derive_catalog(
        &self,
    ) -> Result<Vec<FixedSegmentFuelCertificate>, FixedFuelError> {
        let machine_semantics = self.machine;
        let machine = machine_semantics.id;
        let blocks = &self.blocks;
        let machines = &self.subject.machines;
        let dynamic_call_targets = &self.subject.dynamic_call_targets;
        let provider_candidates = &self.subject.provider_candidates;
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
                let callees = operation_callees(
                    machine,
                    operation,
                    dynamic_call_targets,
                    provider_candidates,
                )?;
                // The caller's terminator stays reachable when at least one
                // admitted dispatch target can return normally.
                if !callees.is_empty() {
                    let mut any_returns = false;
                    for callee in callees {
                        let callee_bounds = maximum_machine_outcomes(
                            callee,
                            machines,
                            dynamic_call_targets,
                            provider_candidates,
                            schedule,
                            &mut memoized_machines,
                            &mut active_machines,
                        )?;
                        any_returns |= callee_bounds.returned.is_some();
                    }
                    if !any_returns {
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
                Terminator::StructuralCase { cases, .. } => {
                    pending.extend(cases.iter().map(|case| case.target));
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
                segments.push(self.segment_certificate(block.id, edge, &mut memoized_machines)?);
            }
        }
        Ok(segments)
    }

    pub(super) fn segment_certificate(
        &self,
        start_block: BlockId,
        end_edge: EdgeId,
        memoized_machines: &mut BTreeMap<MachineId, OutcomeBounds>,
    ) -> Result<FixedSegmentFuelCertificate, FixedFuelError> {
        let terminal_psi = self.subject.identity()?;
        let ceiling_units = self.segment_bound(start_block, end_edge, memoized_machines)?;
        Ok(FixedSegmentFuelCertificate {
            terminal_psi,
            schedule: TerminalFuelSchedule::CURRENT.identity(),
            machine: self.machine.id,
            start_block,
            end_edge,
            relevant_preconditions: Vec::new(),
            ceiling_units,
        })
    }

    /// Exact maximum charge for walks beginning at `start_block` that commit
    /// `end_edge`: operations, each call site's worst admitted callee bound,
    /// and every crossed edge accrue exactly as the entry bound charges them,
    /// so an interior conditional or case contributes its maximum arm rather
    /// than a guessed successor. A walk that leaves the machine through
    /// another terminal edge never commits `end_edge`; that execution is
    /// covered by the other edge's own segment certificate. When no walk
    /// commits the endpoint, the first dead end observed in traversal order
    /// is the reported displacement.
    fn segment_bound(
        &self,
        start_block: BlockId,
        end_edge: EdgeId,
        memoized_machines: &mut BTreeMap<MachineId, OutcomeBounds>,
    ) -> Result<u64, FixedFuelError> {
        if !self.blocks.contains_key(&start_block) {
            return Err(FixedFuelError::UnknownBlock(start_block));
        }
        let mut active_machines = BTreeSet::from([self.machine.id]);
        let mut walk = SegmentWalk {
            end_edge,
            memoized_machines,
            active_machines: &mut active_machines,
            active_blocks: BTreeSet::new(),
            settled: BTreeMap::new(),
            first_dead_end: None,
        };
        match self.block_to_edge_bound(start_block, &mut walk)? {
            Some(units) => Ok(units),
            None => Err(walk
                .first_dead_end
                .unwrap_or(FixedFuelError::NoTerminalPath(self.machine.id))),
        }
    }

    /// Maximum charge from the top of `current` until `end_edge` commits, or
    /// `None` when no walk from `current` takes that edge. Acyclic branching
    /// merges arms as a maximum; a block whose successors' walks are all dead
    /// ends settles as `None` so a reconverging sibling reads it directly.
    fn block_to_edge_bound(
        &self,
        current: BlockId,
        walk: &mut SegmentWalk<'_>,
    ) -> Result<Option<u64>, FixedFuelError> {
        if let Some(bound) = walk.settled.get(&current) {
            return Ok(*bound);
        }
        if !walk.active_blocks.insert(current) {
            return Err(FixedFuelError::ControlCycle(current));
        }
        let machine = self.machine;
        let machines = &self.subject.machines;
        let dynamic_call_targets = &self.subject.dynamic_call_targets;
        let provider_candidates = &self.subject.provider_candidates;
        let schedule = TerminalFuelSchedule::CURRENT;
        let block = self
            .blocks
            .get(&current)
            .copied()
            .ok_or(FixedFuelError::UnknownBlock(current))?;
        let mut units = 0_u64;
        for operation in &block.operations {
            units = units
                .checked_add(schedule.operation_units(&operation.kind))
                .ok_or(FixedFuelError::BoundOverflow)?;
            let callees = operation_callees(
                machine.id,
                operation,
                dynamic_call_targets,
                provider_candidates,
            )?;
            if !callees.is_empty() {
                // Mutually exclusive dispatch targets: the segment's call
                // charge is the maximum normal-return bound across the
                // candidates an admitted dispatch could select.
                let mut invoked_returned = None;
                for &callee in &callees {
                    let callee_bounds = maximum_machine_outcomes(
                        callee,
                        machines,
                        dynamic_call_targets,
                        provider_candidates,
                        schedule,
                        walk.memoized_machines,
                        walk.active_machines,
                    )?;
                    invoked_returned = maximum_optional(invoked_returned, callee_bounds.returned);
                }
                let Some(invoked) = invoked_returned else {
                    // No admitted dispatch target returns normally, so a walk
                    // through this call can never commit the endpoint.
                    walk.remember_dead_end(FixedFuelError::SegmentEndUnreachableAfterCall {
                        block: current,
                        callee: callees[0],
                    });
                    walk.active_blocks.remove(&current);
                    walk.settled.insert(current, None);
                    return Ok(None);
                };
                units = units
                    .checked_add(invoked)
                    .ok_or(FixedFuelError::BoundOverflow)?;
            }
        }
        units = units
            .checked_add(schedule.terminator_units(&block.terminator))
            .ok_or(FixedFuelError::BoundOverflow)?;
        let bound = if block.terminator.edges().any(|edge| edge == walk.end_edge) {
            // The charged end edge commits before any nominal cleanup
            // machines it suspends into; those run as ordinary in-module
            // work inside this segment, so the bound composes them in
            // order exactly like the entry bound does. Other terminators
            // invoke no cleanup machines and return `units` unchanged.
            Some(
                compose_cleanup_outcomes(
                    terminator_cleanup_machines(&block.terminator),
                    OutcomeBounds {
                        returned: Some(units),
                        crashed: None,
                    },
                    machines,
                    dynamic_call_targets,
                    provider_candidates,
                    schedule,
                    walk.memoized_machines,
                    walk.active_machines,
                )?
                .maximum()
                .ok_or(FixedFuelError::NoTerminalPath(machine.id))?,
            )
        } else {
            let successor = match &block.terminator {
                Terminator::Jump { target, .. } => self.block_to_edge_bound(*target, walk)?,
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => maximum_optional(
                    self.block_to_edge_bound(when_true.target, walk)?,
                    self.block_to_edge_bound(when_false.target, walk)?,
                ),
                Terminator::StructuralCase { cases, .. } => {
                    let mut bound = None;
                    for case in cases {
                        bound =
                            maximum_optional(bound, self.block_to_edge_bound(case.target, walk)?);
                    }
                    bound
                }
                Terminator::Return { edge, .. }
                | Terminator::ReturnUnit { edge, .. }
                | Terminator::ReturnUnitPartialAffine { edge, .. }
                | Terminator::ReturnUnitNominalAffine { edge, .. }
                | Terminator::ReturnStructural { edge, .. }
                | Terminator::Crash { edge, .. } => {
                    walk.remember_dead_end(FixedFuelError::SegmentEndNotReached {
                        requested: walk.end_edge,
                        reached_terminal: *edge,
                    });
                    None
                }
            };
            successor
                .map(|tail| units.checked_add(tail).ok_or(FixedFuelError::BoundOverflow))
                .transpose()?
        };
        walk.active_blocks.remove(&current);
        walk.settled.insert(current, bound);
        Ok(bound)
    }
}

/// One selected segment's traversal state. `settled` memoizes the suffix
/// bound per block — with the endpoint fixed for the walk, the maximum charge
/// from a block to committing `end_edge` is path-independent, so reconverging
/// arms share it. `active_blocks` is the depth-first stack: a revisit is a
/// real cycle, while a diamond simply reads the settled suffix. The first
/// dead end encountered is retained so a wholly unreachable request still
/// reports the terminal edge or all-crash call that displaced the endpoint.
struct SegmentWalk<'a> {
    end_edge: EdgeId,
    memoized_machines: &'a mut BTreeMap<MachineId, OutcomeBounds>,
    active_machines: &'a mut BTreeSet<MachineId>,
    active_blocks: BTreeSet<BlockId>,
    settled: BTreeMap<BlockId, Option<u64>>,
    first_dead_end: Option<FixedFuelError>,
}

impl SegmentWalk<'_> {
    fn remember_dead_end(&mut self, dead_end: FixedFuelError) {
        if self.first_dead_end.is_none() {
            self.first_dead_end = Some(dead_end);
        }
    }
}
