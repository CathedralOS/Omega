//! Invocation-local preparation and complete reachable segment partition.

use super::outcome_bounds::{
    NaturalGeometry, NaturalGraphNode, OutcomeBounds, boundary_call_candidates,
    component_entry_rank_bound, compose_cleanup_outcomes, dynamic_call_targets,
    maximum_machine_outcomes, maximum_optional, natural_component_geometry, operation_callees,
    terminator_cleanup_machines, terminator_edge_targets, unbounded_cycle_report,
    unbounded_rank_report,
};
use crate::{FixedFuelError, FixedSegmentFuelCertificate};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, EdgeId, IntegerValue, MachineId, OperationId, Proposition,
};
use std::collections::{BTreeMap, BTreeSet};
use terminal_codec::{TerminalPsiIdentity, terminal_psi_identity};
use terminal_fuel::TerminalFuelSchedule;
use terminal_psi::{
    Block, TerminalMachine, TerminalModule, TerminalNaturalCycle, TerminalRankedScc, Terminator,
};

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
        let (ceiling_units, relevant_preconditions) =
            self.segment_bound(start_block, end_edge, memoized_machines)?;
        Ok(FixedSegmentFuelCertificate {
            terminal_psi,
            schedule: TerminalFuelSchedule::CURRENT.identity(),
            machine: self.machine.id,
            start_block,
            end_edge,
            relevant_preconditions,
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
    /// is the reported displacement. The returned premises are the
    /// `machine.contract.requires` clauses the bound consulted — a cyclic
    /// component interior charged at a contract-tightened rank ceiling —
    /// in canonical contract order.
    fn segment_bound(
        &self,
        start_block: BlockId,
        end_edge: EdgeId,
        memoized_machines: &mut BTreeMap<MachineId, OutcomeBounds>,
    ) -> Result<(u64, Vec<Proposition>), FixedFuelError> {
        if !self.blocks.contains_key(&start_block) {
            return Err(FixedFuelError::UnknownBlock(start_block));
        }
        let mut active_machines = BTreeSet::from([self.machine.id]);
        if let Some(TerminalRankedScc::Natural(components)) = &self.machine.ranked_scc {
            return self.natural_segment_bound(
                start_block,
                end_edge,
                components,
                memoized_machines,
                &mut active_machines,
            );
        }
        let mut walk = SegmentWalk {
            end_edge,
            memoized_machines,
            active_machines: &mut active_machines,
            active_blocks: BTreeSet::new(),
            settled: BTreeMap::new(),
            first_dead_end: None,
        };
        match self.block_to_edge_bound(start_block, &mut walk)? {
            Some(units) => Ok((units, Vec::new())),
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
            return Err(unbounded_cycle_report(self.machine, current));
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
        let Some(units) = self.block_charge_units(
            block,
            current,
            walk.memoized_machines,
            walk.active_machines,
            &mut walk.first_dead_end,
        )?
        else {
            walk.active_blocks.remove(&current);
            walk.settled.insert(current, None);
            return Ok(None);
        };
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

    /// One block's operations, every call site's worst admitted normal-return
    /// bound, and its terminator edge — the charge a walk accrues before the
    /// endpoint either commits on this terminator or the walk continues to a
    /// successor. `None` means a call on this block admits no normally
    /// returning target, so no walk through it commits the endpoint; the dead
    /// end is recorded for a wholly unreachable request.
    fn block_charge_units(
        &self,
        block: &Block,
        block_id: BlockId,
        memoized_machines: &mut BTreeMap<MachineId, OutcomeBounds>,
        active_machines: &mut BTreeSet<MachineId>,
        first_dead_end: &mut Option<FixedFuelError>,
    ) -> Result<Option<u64>, FixedFuelError> {
        let machines = &self.subject.machines;
        let dynamic_call_targets = &self.subject.dynamic_call_targets;
        let provider_candidates = &self.subject.provider_candidates;
        let schedule = TerminalFuelSchedule::CURRENT;
        let mut units = 0_u64;
        for operation in &block.operations {
            units = units
                .checked_add(schedule.operation_units(&operation.kind))
                .ok_or(FixedFuelError::BoundOverflow)?;
            let callees = operation_callees(
                self.machine.id,
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
                        memoized_machines,
                        active_machines,
                    )?;
                    invoked_returned = maximum_optional(invoked_returned, callee_bounds.returned);
                }
                let Some(invoked) = invoked_returned else {
                    // No admitted dispatch target returns normally, so a walk
                    // through this call can never commit the endpoint.
                    if first_dead_end.is_none() {
                        *first_dead_end = Some(FixedFuelError::SegmentEndUnreachableAfterCall {
                            block: block_id,
                            callee: callees[0],
                        });
                    }
                    return Ok(None);
                };
                units = units
                    .checked_add(invoked)
                    .ok_or(FixedFuelError::BoundOverflow)?;
            }
        }
        units
            .checked_add(schedule.terminator_units(&block.terminator))
            .map(Some)
            .ok_or(FixedFuelError::BoundOverflow)
    }

    /// Maximum charge over walks that commit `end_edge` in a verified
    /// `Natural`-ranked machine. When the endpoint rides the start block's
    /// own terminator the segment is one traversal of that block and shares
    /// the acyclic walk's charge — a safe-point catalog row is per-traversal
    /// evidence, not a license to bill the enclosing cycle. Otherwise the
    /// bound is computed on the condensed graph the ranking makes acyclic: an
    /// ordinary block composes exactly as the acyclic walk charges it, while
    /// a component interior bounds committing walks by the longest member
    /// path that can still reach the endpoint when the component's other
    /// internal edges cannot cycle, and by the re-enterable members at the
    /// rank carrier's visit ceiling plus the once-only members charged once
    /// when they can — whether the endpoint commits on a member or beyond
    /// an exit.
    fn natural_segment_bound(
        &self,
        start_block: BlockId,
        end_edge: EdgeId,
        components: &[TerminalNaturalCycle],
        memoized_machines: &mut BTreeMap<MachineId, OutcomeBounds>,
        active_machines: &mut BTreeSet<MachineId>,
    ) -> Result<(u64, Vec<Proposition>), FixedFuelError> {
        let start = self
            .blocks
            .get(&start_block)
            .copied()
            .ok_or(FixedFuelError::UnknownBlock(start_block))?;
        if start.terminator.edges().any(|edge| edge == end_edge) {
            // An endpoint carried by the start block's own terminator is one
            // traversal of that block: operations, worst admitted callee
            // returns, the terminator edge, and the committed edge's ordered
            // cleanup — exactly the charge the acyclic walk composes. The
            // component's rank-multiplied bound belongs to whole-entry
            // composition and to walks that continue past this terminator;
            // a covered backedge or a loop-exit edge on the start terminator
            // is an ordinary per-traversal row, not authority to charge every
            // member visit the cycle could take. Taking the acyclic path here
            // also keeps the catalog derivable when the rank carrier's type
            // maximum itself overflows the whole-component bound. No
            // component rank bound is consulted, so no contract premise is
            // bound.
            let mut walk = SegmentWalk {
                end_edge,
                memoized_machines,
                active_machines,
                active_blocks: BTreeSet::new(),
                settled: BTreeMap::new(),
                first_dead_end: None,
            };
            return match self.block_to_edge_bound(start_block, &mut walk)? {
                Some(units) => Ok((units, Vec::new())),
                None => Err(walk
                    .first_dead_end
                    .unwrap_or(FixedFuelError::NoTerminalPath(self.machine.id))),
            };
        }
        let geometry = natural_component_geometry(
            self.machine,
            components,
            &self.subject.machines,
            &self.subject.dynamic_call_targets,
            &self.subject.provider_candidates,
            TerminalFuelSchedule::CURRENT,
            memoized_machines,
            active_machines,
        )?;
        let mut walk = NaturalSegmentWalk {
            end_edge,
            memoized_machines,
            active_machines,
            active_nodes: BTreeSet::new(),
            settled: BTreeMap::new(),
            first_dead_end: None,
            used_clauses: BTreeSet::new(),
        };
        match self.node_to_edge_bound(start_block, components, &geometry, &mut walk)? {
            Some(units) => Ok((
                u64::try_from(units).map_err(|_| FixedFuelError::BoundOverflow)?,
                walk.used_clauses
                    .iter()
                    .filter_map(|index| self.machine.contract.requires.get(*index).cloned())
                    .collect(),
            )),
            None => Err(walk
                .first_dead_end
                .unwrap_or(FixedFuelError::NoTerminalPath(self.machine.id))),
        }
    }

    /// Maximum charge from the condensed node `entry` arrives at until
    /// `end_edge` commits, or `None` when no walk through it takes that
    /// edge — the segment read of the same condensed DAG
    /// `natural_condensed_bound_returned` bounds for the entry
    /// certificate's returned outcome. `entry` is always a concrete block:
    /// an ordinary block arrives at itself, while a component is entered
    /// at the member the arriving edge names, so two continuations that
    /// enter the same component at different members settle different
    /// interior bounds rather than sharing the whole-component charge.
    fn node_to_edge_bound(
        &self,
        entry: BlockId,
        components: &[TerminalNaturalCycle],
        geometry: &NaturalGeometry,
        walk: &mut NaturalSegmentWalk<'_>,
    ) -> Result<Option<u128>, FixedFuelError> {
        if let Some(bound) = walk.settled.get(&entry) {
            return Ok(*bound);
        }
        if !walk.active_nodes.insert(entry) {
            // A revisited ordinary block sits on an unranked cyclic
            // component the retained partition did not cover — report the
            // verifier-derived component, not the traversal marker. A
            // revisited component member means the condensed graph itself
            // cycles: the retained partition is malformed.
            return Err(match geometry.node_for(entry) {
                NaturalGraphNode::Block(_) => unbounded_cycle_report(self.machine, entry),
                NaturalGraphNode::Component(_) => FixedFuelError::InvalidRankedScc(self.machine.id),
            });
        }
        let bound = match geometry.node_for(entry) {
            NaturalGraphNode::Block(block) => {
                self.block_node_to_edge_bound(block, components, geometry, walk)?
            }
            NaturalGraphNode::Component(index) => {
                self.component_node_to_edge_bound(index, entry, components, geometry, walk)?
            }
        };
        walk.active_nodes.remove(&entry);
        walk.settled.insert(entry, bound);
        Ok(bound)
    }

    /// One unranked block's charge to the endpoint: operations, calls, and the
    /// terminator are charged exactly as the acyclic walk charges them, and
    /// each successor continues on its own condensed node.
    fn block_node_to_edge_bound(
        &self,
        block_id: BlockId,
        components: &[TerminalNaturalCycle],
        geometry: &NaturalGeometry,
        walk: &mut NaturalSegmentWalk<'_>,
    ) -> Result<Option<u128>, FixedFuelError> {
        let block = self
            .blocks
            .get(&block_id)
            .copied()
            .ok_or(FixedFuelError::UnknownBlock(block_id))?;
        let Some(units) = self.block_charge_units(
            block,
            block_id,
            walk.memoized_machines,
            walk.active_machines,
            &mut walk.first_dead_end,
        )?
        else {
            return Ok(None);
        };
        if block.terminator.edges().any(|edge| edge == walk.end_edge) {
            // The charged end edge commits before any nominal cleanup
            // machines it suspends into, exactly as the acyclic walk charges
            // them.
            return Ok(Some(u128::from(
                compose_cleanup_outcomes(
                    terminator_cleanup_machines(&block.terminator),
                    OutcomeBounds {
                        returned: Some(units),
                        crashed: None,
                    },
                    &self.subject.machines,
                    &self.subject.dynamic_call_targets,
                    &self.subject.provider_candidates,
                    TerminalFuelSchedule::CURRENT,
                    walk.memoized_machines,
                    walk.active_machines,
                )?
                .maximum()
                .ok_or(FixedFuelError::NoTerminalPath(self.machine.id))?,
            )));
        }
        let continuation = match &block.terminator {
            Terminator::Jump { target, .. } => {
                self.node_to_edge_bound(*target, components, geometry, walk)?
            }
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => maximum_u128(
                self.node_to_edge_bound(when_true.target, components, geometry, walk)?,
                self.node_to_edge_bound(when_false.target, components, geometry, walk)?,
            ),
            Terminator::StructuralCase { cases, .. } => {
                let mut bound = None;
                for case in cases {
                    bound = maximum_u128(
                        bound,
                        self.node_to_edge_bound(case.target, components, geometry, walk)?,
                    );
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
        continuation
            .map(|tail| {
                u128::from(units)
                    .checked_add(tail)
                    .ok_or(FixedFuelError::BoundOverflow)
            })
            .transpose()
    }

    /// A component's charge to the endpoint. A member takes a terminator
    /// edge only by completing its own traversal, so a member whose visit
    /// is a dead end — an all-crash call, say — can neither carry the
    /// endpoint nor leave through an exit; its edges contribute nothing
    /// and the recorded dead end stands in for the walks that die inside
    /// it. Walks committing `end_edge` on
    /// a member terminator cannot re-enter the component once they leave it,
    /// so when the members that can still reach a committing traversal form
    /// an acyclic interior, every such walk is one pass bounded by the
    /// longest member-visit sum the `entry` member can start; otherwise the
    /// surviving cycles bound only the members that can still be re-entered
    /// — the rank carrier's type maximum plus one visits each — while
    /// members left off every cycle are crossed at most once, plus the
    /// committing edge's ordered cleanup work. When no member terminator
    /// carries the endpoint, the walk must leave the component through a
    /// member exit before it can commit, so each exit-taking member pairs
    /// its own interior — the members that can still reach it alone, a
    /// longest path when that remainder is acyclic and the rank-ceiling
    /// split when a surviving cycle keeps revisits possible — with the
    /// worst continuation its own exits still commit, rather than the
    /// union of every exit-reaching member against the worst exit's tail.
    /// Both reads restrict the
    /// interior to the members `entry` can still traverse: a member the
    /// walk cannot reach from the entry member is never visited, so it is
    /// never billed. A component cannot be re-entered once left, and a
    /// walk that ends on another terminal edge is covered by that edge's
    /// own segment.
    fn component_node_to_edge_bound(
        &self,
        index: usize,
        entry: BlockId,
        components: &[TerminalNaturalCycle],
        geometry: &NaturalGeometry,
        walk: &mut NaturalSegmentWalk<'_>,
    ) -> Result<Option<u128>, FixedFuelError> {
        let component = components
            .get(index)
            .ok_or(FixedFuelError::InvalidRankedScc(self.machine.id))?;
        let mut live = BTreeSet::new();
        let mut committing_members = BTreeSet::new();
        let mut exits: Vec<(BlockId, BlockId)> = Vec::new();
        for rank in &component.ranks {
            let block = self
                .blocks
                .get(&rank.block)
                .copied()
                .ok_or(FixedFuelError::UnknownBlock(rank.block))?;
            // A member takes a terminator edge only by completing its own
            // traversal: when the visit is a dead end — an all-crash call,
            // say — the member can neither commit the endpoint nor leave
            // through an exit, so its edges contribute nothing and the
            // recorded dead end stands in for the walk that died.
            if self
                .block_charge_units(
                    block,
                    rank.block,
                    walk.memoized_machines,
                    walk.active_machines,
                    &mut walk.first_dead_end,
                )?
                .is_none()
            {
                continue;
            }
            live.insert(rank.block);
            if block.terminator.edges().any(|edge| edge == walk.end_edge) {
                committing_members.insert(rank.block);
            } else {
                exits.extend(
                    super::outcome_bounds::terminator_targets(&block.terminator)
                        .into_iter()
                        .filter(|target| geometry.member_of.get(target) != Some(&index))
                        .map(|target| (rank.block, target)),
                );
            }
        }
        if !committing_members.is_empty() {
            let adjacency =
                self.interior_adjacency(component, index, geometry, walk.end_edge, &live)?;
            let forward = Self::forward_reaching(&adjacency, entry);
            let reaching = Self::reverse_reaching(&adjacency, &committing_members);
            let traversable: BTreeSet<BlockId> = forward.intersection(&reaching).copied().collect();
            let committing: BTreeSet<BlockId> = committing_members
                .intersection(&traversable)
                .copied()
                .collect();
            if committing.is_empty() {
                // No committing member is reachable from `entry` — walks
                // die on a member terminal edge or leave through an exit,
                // and those executions belong to other segments.
                self.remember_reachable_terminal(component, &forward, walk);
                return Ok(None);
            }
            // The committing edge's ordered cleanup composes on top of the
            // interior charge, covered by the worst reachable committing
            // member.
            let mut cleanup = 0_u64;
            for &member in &committing {
                let block = self
                    .blocks
                    .get(&member)
                    .copied()
                    .ok_or(FixedFuelError::UnknownBlock(member))?;
                cleanup = cleanup.max(
                    compose_cleanup_outcomes(
                        terminator_cleanup_machines(&block.terminator),
                        OutcomeBounds {
                            returned: Some(0),
                            crashed: None,
                        },
                        &self.subject.machines,
                        &self.subject.dynamic_call_targets,
                        &self.subject.provider_candidates,
                        TerminalFuelSchedule::CURRENT,
                        walk.memoized_machines,
                        walk.active_machines,
                    )?
                    .maximum()
                    .ok_or(FixedFuelError::NoTerminalPath(self.machine.id))?,
                );
            }
            match self.interior_committing_bound(
                &committing,
                &adjacency,
                &traversable,
                entry,
                walk,
            )? {
                InteriorBound::Acyclic(bound) => return Ok(Some(bound)),
                InteriorBound::Cyclic => {
                    return self
                        .cyclic_interior_bound(component, &adjacency, &traversable, cleanup, walk)
                        .map(Some);
                }
                InteriorBound::Unreachable => return Ok(None),
            }
        }
        if exits.is_empty() {
            // Walks that never leave the component end on a member's
            // terminal edge without committing the endpoint; report the
            // first such terminal the entry member can still reach, in
            // canonical member order.
            let adjacency =
                self.interior_adjacency(component, index, geometry, walk.end_edge, &live)?;
            let forward = Self::forward_reaching(&adjacency, entry);
            self.remember_reachable_terminal(component, &forward, walk);
            return Ok(None);
        }
        // The endpoint commits beyond the component, so a committing walk
        // leaves through one of the exit edges and can never return. Only
        // members the entry member can still reach participate, and a walk
        // leaving through a member crosses only the members that can still
        // reach it — the interior and the continuation pair per exit
        // member rather than the union of every exit-reaching member
        // against the worst exit's tail. When no exit target can commit,
        // the component is a dead end exactly as the old whole-component
        // charge read it.
        let adjacency =
            self.interior_adjacency(component, index, geometry, walk.end_edge, &live)?;
        let forward = Self::forward_reaching(&adjacency, entry);
        let mut exits_by_member: BTreeMap<BlockId, Vec<BlockId>> = BTreeMap::new();
        for (member, target) in exits {
            if forward.contains(&member) {
                exits_by_member.entry(member).or_default().push(target);
            }
        }
        let mut bound = None;
        for (member, targets) in exits_by_member {
            let member_reach = Self::reverse_reaching(&adjacency, &BTreeSet::from([member]));
            let member_traversable: BTreeSet<BlockId> =
                forward.intersection(&member_reach).copied().collect();
            // The walk exits on a successor edge of `member`, so its last
            // interior visit is `member`'s own traversal — the same sink a
            // committing member without cleanup machines bills — with the
            // longest-path bound when its reaching remainder is acyclic and
            // the rank-ceiling split when a surviving cycle keeps revisits
            // possible.
            let interior = match self.interior_committing_bound(
                &BTreeSet::from([member]),
                &adjacency,
                &member_traversable,
                entry,
                walk,
            )? {
                InteriorBound::Acyclic(bound) => bound,
                InteriorBound::Cyclic => {
                    self.split_interior_units(component, &adjacency, &member_traversable, walk)?
                }
                InteriorBound::Unreachable => continue,
            };
            for target in targets {
                if let Some(tail) = self.node_to_edge_bound(target, components, geometry, walk)? {
                    bound = maximum_u128(
                        bound,
                        Some(
                            interior
                                .checked_add(tail)
                                .ok_or(FixedFuelError::BoundOverflow)?,
                        ),
                    );
                }
            }
        }
        if bound.is_none() {
            self.remember_reachable_terminal(component, &forward, walk);
        }
        Ok(bound)
    }

    /// A committing walk that cannot reach the frontier ends on a member's
    /// own terminal edge; report the first such edge the entry member can
    /// still reach, in canonical member order, so an unreachable request
    /// names the terminal that displaced the endpoint rather than
    /// fabricating an interior bound.
    fn remember_reachable_terminal(
        &self,
        component: &TerminalNaturalCycle,
        forward: &BTreeSet<BlockId>,
        walk: &mut NaturalSegmentWalk<'_>,
    ) {
        if let Some(terminal_edge) = component
            .ranks
            .iter()
            .filter(|rank| forward.contains(&rank.block))
            .filter_map(|rank| self.blocks.get(&rank.block))
            .filter(|block| super::outcome_bounds::terminator_targets(&block.terminator).is_empty())
            .find_map(|block| block.terminator.edges().next())
        {
            walk.remember_dead_end(FixedFuelError::SegmentEndNotReached {
                requested: walk.end_edge,
                reached_terminal: terminal_edge,
            });
        }
    }

    /// The component's internal adjacency for an interior bound: every
    /// live member's successor edges that stay inside the component,
    /// minus `exclude`. The committing case drops the endpoint edge so a
    /// surviving cycle is real. A member whose traversal can never
    /// complete emits no usable edge — a walk that reaches it dies
    /// inside, so its successors stay unreachable through it.
    fn interior_adjacency(
        &self,
        component: &TerminalNaturalCycle,
        index: usize,
        geometry: &NaturalGeometry,
        exclude: EdgeId,
        live: &BTreeSet<BlockId>,
    ) -> Result<BTreeMap<BlockId, Vec<BlockId>>, FixedFuelError> {
        let mut adjacency: BTreeMap<BlockId, Vec<BlockId>> = BTreeMap::new();
        for rank in &component.ranks {
            if !live.contains(&rank.block) {
                continue;
            }
            let block = self
                .blocks
                .get(&rank.block)
                .copied()
                .ok_or(FixedFuelError::UnknownBlock(rank.block))?;
            adjacency.insert(
                rank.block,
                terminator_edge_targets(&block.terminator)
                    .into_iter()
                    .filter(|(edge, target)| {
                        *edge != exclude && geometry.member_of.get(target) == Some(&index)
                    })
                    .map(|(_, target)| target)
                    .collect(),
            );
        }
        Ok(adjacency)
    }

    /// Members that can still reach `frontier` through `adjacency` — the
    /// reverse reachability the interior bounds share.
    fn reverse_reaching(
        adjacency: &BTreeMap<BlockId, Vec<BlockId>>,
        frontier: &BTreeSet<BlockId>,
    ) -> BTreeSet<BlockId> {
        let mut reaching: BTreeSet<BlockId> = frontier.iter().copied().collect();
        let mut pending: Vec<BlockId> = frontier.iter().copied().collect();
        while let Some(member) = pending.pop() {
            for (predecessor, targets) in adjacency {
                if targets.contains(&member) && reaching.insert(*predecessor) {
                    pending.push(*predecessor);
                }
            }
        }
        reaching
    }

    /// Members a walk arriving at `entry` can still traverse through
    /// `adjacency` — the forward cone the interior bounds intersect with
    /// the frontier-reaching members. A member outside it is never
    /// visited by a walk starting at `entry`, so it is never billed.
    fn forward_reaching(
        adjacency: &BTreeMap<BlockId, Vec<BlockId>>,
        entry: BlockId,
    ) -> BTreeSet<BlockId> {
        let mut reaching = BTreeSet::from([entry]);
        let mut pending = vec![entry];
        while let Some(member) = pending.pop() {
            for target in adjacency.get(&member).into_iter().flatten() {
                if reaching.insert(*target) {
                    pending.push(*target);
                }
            }
        }
        reaching
    }

    /// Tighter interior bound when the endpoint commits on a member
    /// terminator. A walk that commits `end_edge` stays inside the
    /// component's other internal edges until the committing traversal —
    /// the component cannot be re-entered once left, and leaving through
    /// another terminal edge belongs to that edge's own segment — so when
    /// the member subgraph that can still reach a committing member is
    /// acyclic, every committing walk is one simple path through it and
    /// the bound is the longest member-visit sum that starts at `entry`,
    /// charged exactly like the acyclic walk, rather than the
    /// rank-multiplied whole-component charge. `Cyclic` when that
    /// subgraph retains a cycle (rank-bounded revisits remain possible
    /// and `cyclic_interior_bound` applies); `Unreachable` when no
    /// acyclic committing path leaves `entry` at all.
    fn interior_committing_bound(
        &self,
        committing: &BTreeSet<BlockId>,
        adjacency: &BTreeMap<BlockId, Vec<BlockId>>,
        reaching: &BTreeSet<BlockId>,
        entry: BlockId,
        walk: &mut NaturalSegmentWalk<'_>,
    ) -> Result<InteriorBound, FixedFuelError> {
        // Topological order over the reaching subgraph. A leftover member
        // means a cycle survives there, so rank-bounded revisits remain
        // possible and the cyclic-interior bound applies instead.
        let mut indegree: BTreeMap<BlockId, usize> = BTreeMap::new();
        for &member in reaching {
            indegree.entry(member).or_insert(0);
            for target in adjacency.get(&member).into_iter().flatten() {
                if reaching.contains(target) {
                    *indegree.entry(*target).or_insert(0) += 1;
                }
            }
        }
        let mut frontier: Vec<BlockId> = indegree
            .iter()
            .filter(|(_, degree)| **degree == 0)
            .map(|(&member, _)| member)
            .collect();
        let mut order = Vec::with_capacity(reaching.len());
        while let Some(member) = frontier.pop() {
            order.push(member);
            for target in adjacency.get(&member).into_iter().flatten() {
                if let Some(degree) = indegree.get_mut(target) {
                    *degree -= 1;
                    if *degree == 0 {
                        frontier.push(*target);
                    }
                }
            }
        }
        if order.len() != reaching.len() {
            return Ok(InteriorBound::Cyclic);
        }
        // Longest committing path over the DAG, sinks first. A member whose
        // own charge fails (an all-crash call, say) admits no committing
        // walk through it, exactly as the acyclic walk reads the same
        // block. The bound is `entry`'s own charge to the endpoint — the
        // member the segment start or arriving continuation actually
        // enters at — not the maximum over every member, since a member
        // `entry` cannot reach admits no walk through it.
        let mut dist: BTreeMap<BlockId, u128> = BTreeMap::new();
        for &member in order.iter().rev() {
            let block = self
                .blocks
                .get(&member)
                .copied()
                .ok_or(FixedFuelError::UnknownBlock(member))?;
            let Some(units) = self.block_charge_units(
                block,
                member,
                walk.memoized_machines,
                walk.active_machines,
                &mut walk.first_dead_end,
            )?
            else {
                continue;
            };
            let mut best = None;
            if committing.contains(&member) {
                best = Some(u128::from(
                    compose_cleanup_outcomes(
                        terminator_cleanup_machines(&block.terminator),
                        OutcomeBounds {
                            returned: Some(units),
                            crashed: None,
                        },
                        &self.subject.machines,
                        &self.subject.dynamic_call_targets,
                        &self.subject.provider_candidates,
                        TerminalFuelSchedule::CURRENT,
                        walk.memoized_machines,
                        walk.active_machines,
                    )?
                    .maximum()
                    .ok_or(FixedFuelError::NoTerminalPath(self.machine.id))?,
                ));
            }
            for target in adjacency.get(&member).into_iter().flatten() {
                if let Some(tail) = dist.get(target) {
                    best = maximum_u128(best, Some(u128::from(units) + tail));
                }
            }
            if let Some(bound) = best {
                dist.insert(member, bound);
            }
        }
        Ok(match dist.get(&entry) {
            Some(&bound) => InteriorBound::Acyclic(bound),
            None => InteriorBound::Unreachable,
        })
    }

    /// Interior bound when the endpoint exclusion leaves a cycle in the
    /// reaching subgraph. The committing edge's ordered cleanup composes on
    /// top of the member split `split_interior_units` computes.
    fn cyclic_interior_bound(
        &self,
        component: &TerminalNaturalCycle,
        adjacency: &BTreeMap<BlockId, Vec<BlockId>>,
        reaching: &BTreeSet<BlockId>,
        cleanup: u64,
        walk: &mut NaturalSegmentWalk<'_>,
    ) -> Result<u128, FixedFuelError> {
        self.split_interior_units(component, adjacency, reaching, walk)?
            .checked_add(u128::from(cleanup))
            .ok_or(FixedFuelError::BoundOverflow)
    }

    /// The interior member split both component bounds share. A member that
    /// can still be re-entered through the surviving internal edges may be
    /// visited up to the component's entry-rank bound plus one times before
    /// the walk leaves the component — the carrier's type maximum, or the
    /// lower literal ceiling a `requires` clause places on every rank
    /// arriving at first entry, the same per-member visit ceiling the
    /// whole-component charge uses — while a member left off every surviving
    /// cycle is crossed at most once, since the walk cannot return to it.
    /// The bound is therefore the rank-multiplied sum over the re-enterable
    /// members plus the once-only members charged once, rather than the
    /// rank-multiplied sum over every member. A member whose own charge
    /// fails (an all-crash call, say) admits no traversal that leaves the
    /// component and contributes nothing, exactly as the acyclic walk reads
    /// it.
    fn split_interior_units(
        &self,
        component: &TerminalNaturalCycle,
        adjacency: &BTreeMap<BlockId, Vec<BlockId>>,
        reaching: &BTreeSet<BlockId>,
        walk: &mut NaturalSegmentWalk<'_>,
    ) -> Result<u128, FixedFuelError> {
        let IntegerValue::Unsigned(rank_maximum) = component.rank_type.maximum_value() else {
            return Err(FixedFuelError::InvalidRankedScc(self.machine.id));
        };
        let entry_bound =
            component_entry_rank_bound(self.machine, component, &self.blocks, rank_maximum);
        if let Some(entry) = &entry_bound {
            // The rank ceiling this split bills rests on the contract
            // clauses that derived it; the certificate binds them as its
            // relevant preconditions.
            walk.used_clauses.extend(entry.clauses.iter().copied());
        }
        let rank_bound = entry_bound.map(|entry| entry.bound).unwrap_or(rank_maximum);
        let mut reenterable_units = 0_u128;
        let mut once_units = 0_u128;
        for &member in reaching {
            // The member stays re-enterable when one of its surviving
            // successors reaches it again — an internal cycle the
            // adjacency left intact.
            let mut reenterable = false;
            let mut pending: Vec<BlockId> = adjacency
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
                    pending.extend(adjacency.get(&next).into_iter().flatten().copied());
                }
            }
            let block = self
                .blocks
                .get(&member)
                .copied()
                .ok_or(FixedFuelError::UnknownBlock(member))?;
            let Some(units) = self.block_charge_units(
                block,
                member,
                walk.memoized_machines,
                walk.active_machines,
                &mut walk.first_dead_end,
            )?
            else {
                continue;
            };
            if reenterable {
                reenterable_units = reenterable_units
                    .checked_add(u128::from(units))
                    .ok_or(FixedFuelError::BoundOverflow)?;
            } else {
                once_units = once_units
                    .checked_add(u128::from(units))
                    .ok_or(FixedFuelError::BoundOverflow)?;
            }
        }
        // The rank-bounded visit arithmetic is this component's own bound:
        // when it cannot be represented in the certificate's `u64` scalar
        // ceiling the rank supplies no publishable bound, so the report
        // names the component with the unbounded-rank cause rather than a
        // flat overflow.
        let interior = rank_bound
            .checked_add(1)
            .and_then(|visits| visits.checked_mul(reenterable_units))
            .and_then(|bound| bound.checked_add(once_units))
            .ok_or_else(|| unbounded_rank_report(self.machine, component))?;
        if interior > u128::from(u64::MAX) {
            return Err(unbounded_rank_report(self.machine, component));
        }
        Ok(interior)
    }
}

fn maximum_u128(left: Option<u128>, right: Option<u128>) -> Option<u128> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(units), None) | (None, Some(units)) => Some(units),
        (None, None) => None,
    }
}

/// How an interior committing pass resolves for the entry member:
/// `Acyclic` carries the segment bound when every committing walk is one
/// interior pass, `Cyclic` marks a surviving internal cycle that keeps
/// rank-bounded revisits possible, and `Unreachable` marks the defensive
/// case where the entry member's acyclic remainder reaches no committing
/// member.
enum InteriorBound {
    Acyclic(u128),
    Cyclic,
    Unreachable,
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

/// One selected segment's traversal state over the condensed graph of a
/// `Natural`-ranked machine — the same discipline as `SegmentWalk`, keyed
/// by the concrete block a walk arrives at: an ordinary block for itself,
/// or the member a component is entered at. Two continuations that reach
/// the same component through different members settle different interior
/// bounds under their own keys. `settled` memoizes the suffix bound per
/// arrival, `active_nodes` is a defensive depth-first stack on a graph
/// the verified partition already makes acyclic, and the first dead end
/// is retained so a wholly unreachable request still reports a terminal
/// edge or all-crash call. `used_clauses` records the
/// `machine.contract.requires` positions a cyclic interior bound rested
/// on, so the certificate's `relevant_preconditions` names exactly the
/// premises the derivation consulted.
struct NaturalSegmentWalk<'a> {
    end_edge: EdgeId,
    memoized_machines: &'a mut BTreeMap<MachineId, OutcomeBounds>,
    active_machines: &'a mut BTreeSet<MachineId>,
    active_nodes: BTreeSet<BlockId>,
    settled: BTreeMap<BlockId, Option<u128>>,
    first_dead_end: Option<FixedFuelError>,
    used_clauses: BTreeSet<usize>,
}

impl NaturalSegmentWalk<'_> {
    fn remember_dead_end(&mut self, dead_end: FixedFuelError) {
        if self.first_dead_end.is_none() {
            self.first_dead_end = Some(dead_end);
        }
    }
}
