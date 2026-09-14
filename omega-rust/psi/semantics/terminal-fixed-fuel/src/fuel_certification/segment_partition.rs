//! Invocation-local preparation and complete reachable segment partition.

use super::outcome_bounds::{
    OutcomeBounds, dynamic_call_targets, maximum_machine_outcomes, operation_callee,
};
use crate::{FixedFuelError, FixedSegmentFuelCertificate};
use semantic_vocabulary::{BlockId, EdgeId, MachineId, OperationId};
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
                if let Some(callee) = operation_callee(machine, operation, dynamic_call_targets) {
                    let callee_bounds = maximum_machine_outcomes(
                        callee,
                        machines,
                        dynamic_call_targets,
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

    fn segment_bound(
        &self,
        start_block: BlockId,
        end_edge: EdgeId,
        memoized_machines: &mut BTreeMap<MachineId, OutcomeBounds>,
    ) -> Result<u64, FixedFuelError> {
        let machine = self.machine;
        let blocks = &self.blocks;
        let machines = &self.subject.machines;
        let dynamic_call_targets = &self.subject.dynamic_call_targets;
        let schedule = TerminalFuelSchedule::CURRENT;
        let mut active_machines = BTreeSet::from([machine.id]);
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
                if let Some(callee) = operation_callee(machine.id, operation, dynamic_call_targets)
                {
                    let callee_bounds = maximum_machine_outcomes(
                        callee,
                        machines,
                        dynamic_call_targets,
                        schedule,
                        memoized_machines,
                        &mut active_machines,
                    )?;
                    units = units
                        .checked_add(callee_bounds.returned.ok_or(
                            FixedFuelError::SegmentEndUnreachableAfterCall {
                                block: current,
                                callee,
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
                Terminator::Conditional { .. } | Terminator::StructuralCase { .. } => {
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
}
