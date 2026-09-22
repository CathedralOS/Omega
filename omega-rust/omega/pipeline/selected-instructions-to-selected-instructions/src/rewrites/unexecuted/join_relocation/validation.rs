//! Independent validation of join relocation.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! relocation's legality from the source records — the member's
//! coordinates inside its converging join, the arms whose plain `Jump`
//! edges are the join's only predecessors, the one fork head every arm
//! descends from and its two-successor conditional terminator, the
//! landing index the destination names, the schedulable bar and hazard
//! audit the member meets against every crossed position and every
//! crossed edge's terminator instruction, the roster-ordering
//! accounting, the transport-conflict check, and the boundary-settlement
//! exclusion — then rebuilds the function the contract demands and
//! requires the proposal to equal it. Restoring the member to its source
//! position must reproduce the complete source by content. A producer
//! admission error therefore fails validation even when the proposal is
//! exactly what that producer emitted.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedBlockId, SelectedBlockOrigin, SelectedFunction, SelectedInstructionId,
    SelectedInstructionPlan, SelectedSuccessor, SelectedTerminator,
};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{JoinRelocationError, JoinRelocationReceipt, ValidatedJoinRelocation};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{
    PathEdge, RelocationCrossing, all_edges, edge_accounted, edge_surface, plain_edge,
    terminator_instruction, terminator_successors, transport_conflict,
};
use crate::rewrites::window_hazards::{coupled, has_call_contract, schedulable, surface};

/// The validator's own reconstruction of the relocation the contract
/// permits: the member's coordinates inside the join, the fork head it
/// lands in, and the window the gates leave. It shares no state with the
/// producer's `admission` record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    function_index: usize,
    /// The member's own block: the diamond's converging join.
    block: SelectedBlockId,
    /// The fork head every reached arm descends from.
    target: SelectedBlockId,
    /// The window the member crosses: the join's prefix behind the
    /// member, the whole body of every arm a branch edge reaches, the
    /// head's suffix at and after the landing index, and every crossed
    /// edge — each branch edge and each reached arm's `Jump` — carrying
    /// its terminator instruction and successor row.
    crossing: RelocationCrossing<'source>,
}

/// Reconstruct the legality of relocating `member` out of its converging
/// join onto `destination` from first principles: locate the member,
/// require its block to be a source-origin join reached only by arms —
/// plain source blocks, each distinct from the join and the entry, whose
/// terminator is an unconditional `Jump` to the join on a plain semantic
/// edge — require every edge into every arm to leave one common fork
/// head, require the head to be a source block outside the region whose
/// two-successor conditional terminator names only reached arms on plain
/// edges, resolve the landing index the destination names, then run the
/// window audit the gates leave — the member schedulable, every crossed
/// position schedulable and neither roster-accounted against an
/// accounted member nor coupled with it, and every crossed edge plain,
/// its terminator instruction uncalled, unaccounted against an accounted
/// member, uncoupled, and free of transport conflicts — with no boundary
/// settlement past the member's index in the join, past the landing
/// index in the head, or inside a crossed arm. Nothing in this audit
/// reads the producer's admission decision.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
) -> Result<Reconstructed<'source>, JoinRelocationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(JoinRelocationError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(JoinRelocationError::SourceMismatch)?;
    let (block_index, member_index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .position(|instruction| instruction.id == member)
                .map(|member_index| (block_index, member_index))
        })
        .ok_or(JoinRelocationError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    let member_instruction = &block.instructions[member_index];
    // The member's block must be a converging join: every edge into it
    // leaves an arm — a plain source block, distinct from the join and
    // the entry, whose terminator is an unconditional `Jump` to the join
    // on a plain semantic edge. A predecessor shaped any other way hands
    // the join a path the member's new position cannot supply, and a
    // join with no predecessors is unreachable: hoisting the member
    // would start its execution.
    if block.id == function.entry_block || !matches!(block.origin, SelectedBlockOrigin::Source(_)) {
        return Err(JoinRelocationError::UnsupportedPair);
    }
    let mut arm_indices: Vec<usize> = Vec::new();
    for (source_block, _) in all_edges(function).filter(|(_, edge)| edge.block == block.id) {
        let arm_index = function
            .blocks
            .iter()
            .position(|candidate| candidate.id == source_block)
            .ok_or(JoinRelocationError::SourceMismatch)?;
        let arm = &function.blocks[arm_index];
        if arm.id == block.id
            || arm.id == function.entry_block
            || !matches!(arm.origin, SelectedBlockOrigin::Source(_))
        {
            return Err(JoinRelocationError::UnsupportedPair);
        }
        // The predecessor's edge into the join is its `Jump` successor
        // itself: a conditional or non-plain edge into the join is a
        // converging path this step does not cross.
        let SelectedTerminator::Jump {
            successor: arm_edge,
            ..
        } = &arm.terminator
        else {
            return Err(JoinRelocationError::UnsupportedPair);
        };
        if !plain_edge(arm_edge) {
            return Err(JoinRelocationError::UnsupportedPair);
        }
        arm_indices.push(arm_index);
    }
    if arm_indices.is_empty() {
        return Err(JoinRelocationError::UnsupportedPair);
    }
    // The arms must descend from one fork head alone: every edge into
    // every arm leaves the same block. A second predecessor into an arm
    // would hand the join a path that never crossed the member's new
    // position — the partial-supply failure the totality rule refuses.
    let mut target_index: Option<usize> = None;
    for &arm_index in &arm_indices {
        for (source_block, _) in
            all_edges(function).filter(|(_, edge)| edge.block == function.blocks[arm_index].id)
        {
            let head_index = function
                .blocks
                .iter()
                .position(|candidate| candidate.id == source_block)
                .ok_or(JoinRelocationError::SourceMismatch)?;
            match target_index {
                Some(existing) if existing != head_index => {
                    return Err(JoinRelocationError::UnsupportedPair);
                }
                Some(_) => {}
                None => target_index = Some(head_index),
            }
        }
    }
    let target_index = target_index.ok_or(JoinRelocationError::UnsupportedPair)?;
    let target = &function.blocks[target_index];
    // The head must not collapse back into the region: landing inside
    // the join or inside an arm is an in-block or self-loop move this
    // step does not cross, and an implementation block's origin carries
    // edge or case work the bounded audit does not cross.
    if target.id == block.id
        || arm_indices
            .iter()
            .any(|&arm| function.blocks[arm].id == target.id)
        || !matches!(target.origin, SelectedBlockOrigin::Source(_))
    {
        return Err(JoinRelocationError::UnsupportedPair);
    }
    // Only a two-successor conditional terminator gives the landing
    // block the fork this step rises through; every other terminator
    // shape is the single-edge family's case or no relocation at all.
    let branch_edges: Vec<&SelectedSuccessor> = match &target.terminator {
        SelectedTerminator::ConditionalBranch {
            when_nonzero,
            when_zero,
            ..
        } => vec![when_nonzero, when_zero],
        SelectedTerminator::ConditionalBranchU64LessThan {
            when_less,
            when_not_less,
            ..
        }
        | SelectedTerminator::ConditionalBranchI64LessThan {
            when_less,
            when_not_less,
            ..
        } => vec![when_less, when_not_less],
        SelectedTerminator::Jump { .. }
        | SelectedTerminator::Crash { .. }
        | SelectedTerminator::HostedExitProcess { .. }
        | SelectedTerminator::Return { .. } => {
            return Err(JoinRelocationError::UnsupportedPair);
        }
    };
    // Every edge the head names must reach an arm: an edge leaving the
    // region would run the member on a traversal the join never saw.
    for edge in &branch_edges {
        if !plain_edge(edge)
            || !arm_indices
                .iter()
                .any(|&arm| function.blocks[arm].id == edge.block)
        {
            return Err(JoinRelocationError::UnsupportedPair);
        }
    }
    // The destination names a position in the head body — the member
    // lands at its index — or the head's terminator-carried instruction,
    // landing the member at the body end.
    let landing_index = target
        .instructions
        .iter()
        .position(|instruction| instruction.id == destination)
        .or_else(|| {
            (terminator_instruction(&target.terminator).id == destination)
                .then_some(target.instructions.len())
        })
        .ok_or(JoinRelocationError::UnsupportedPair)?;
    // The window the member crosses is the one the gates leave: the
    // join's prefix behind the member, the head's suffix at and after
    // the landing index, the whole body of every arm a branch edge
    // reaches, and the crossed edges — each branch edge's terminator
    // instruction and successor row plus each reached arm's `Jump` edge.
    // An arm the head never names feeds the join but lies on no path
    // between the head and the join, so it crosses nothing — the same
    // boundary the path-derived window draws. Every crossed contract
    // below is the family's own composition of the shared primitives,
    // not the producer's audit result.
    let mut positions: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    positions
        .entry(block_index)
        .or_default()
        .extend(0..member_index);
    positions
        .entry(target_index)
        .or_default()
        .extend(landing_index..target.instructions.len());
    let mut seen_edges = BTreeSet::new();
    let mut edges: Vec<PathEdge<'source>> = Vec::new();
    for edge in &branch_edges {
        if seen_edges.insert((target.id, edge.psi_edge)) {
            edges.push(PathEdge {
                block: target.id,
                instruction: terminator_instruction(&target.terminator),
                successor: edge,
            });
        }
        // The arm this branch edge feeds contributes its whole body and
        // the `Jump` edge that leaves it.
        let Some(&arm_index) = arm_indices
            .iter()
            .find(|&&arm| function.blocks[arm].id == edge.block)
        else {
            continue;
        };
        let arm = &function.blocks[arm_index];
        positions
            .entry(arm_index)
            .or_default()
            .extend(0..arm.instructions.len());
        let SelectedTerminator::Jump {
            instruction: jump_instruction,
            successor: arm_edge,
        } = &arm.terminator
        else {
            return Err(JoinRelocationError::UnsupportedPair);
        };
        if seen_edges.insert((arm.id, arm_edge.psi_edge)) {
            edges.push(PathEdge {
                block: arm.id,
                instruction: jump_instruction,
                successor: arm_edge,
            });
        }
    }
    let crossing = RelocationCrossing {
        reachable: true,
        positions: positions
            .into_iter()
            .map(|(block, ordinals)| (block, ordinals.into_iter().collect()))
            .collect(),
        edges,
        run_block: block_index,
        run_start: member_index,
        run_end: member_index,
        destination_block: target_index,
        landing_index,
    };
    let Some(member_accounted) = schedulable(function, member_instruction) else {
        return Err(JoinRelocationError::UnsupportedInstruction);
    };
    for (crossed_block, ordinals) in &crossing.positions {
        let crossed_block = &function.blocks[*crossed_block];
        for &position in ordinals {
            let crossed = &crossed_block.instructions[position];
            let Some(crossed_accounted) = schedulable(function, crossed) else {
                return Err(JoinRelocationError::UnsupportedInstruction);
            };
            if member_accounted && crossed_accounted {
                return Err(JoinRelocationError::UnsupportedPair);
            }
            if coupled(member_instruction, crossed) {
                return Err(JoinRelocationError::UnsupportedPair);
            }
        }
    }
    for edge in &crossing.edges {
        if !plain_edge(edge.successor) {
            return Err(JoinRelocationError::UnsupportedPair);
        }
        if has_call_contract(function, edge.instruction.id) {
            return Err(JoinRelocationError::UnsupportedInstruction);
        }
        if member_accounted && edge_accounted(function, edge.instruction, edge.successor) {
            return Err(JoinRelocationError::UnsupportedPair);
        }
        if coupled(member_instruction, edge.instruction) {
            return Err(JoinRelocationError::UnsupportedPair);
        }
        if transport_conflict(member_instruction, edge.successor) {
            return Err(JoinRelocationError::UnsupportedPair);
        }
    }
    // The member vacates the join's prefix from its index on, and the
    // head's suffix past the landing index gains it: a boundary
    // settlement on either side of those cuts observes a changed
    // executed prefix. A settlement inside a crossed arm refuses too —
    // the member executes before that arm's point after the move where
    // it executed after it before — decided by the block key, so an
    // empty-bodied arm's index-0 settlement refuses the same way.
    for settlement in &function.boundary_settlements {
        let index = settlement.instruction_index as usize;
        let refused = if settlement.block == block.id {
            index > member_index
        } else if settlement.block == target.id {
            index > landing_index
        } else {
            crossing
                .positions
                .iter()
                .any(|(crossed_block, _)| function.blocks[*crossed_block].id == settlement.block)
        };
        if refused {
            return Err(JoinRelocationError::UnsupportedPair);
        }
    }
    Ok(Reconstructed {
        function,
        function_index,
        block: block.id,
        target: target.id,
        crossing,
    })
}

/// The validation work this audit performs, in the measured-step
/// contract the family publishes: one step per block plus one per
/// instruction across the plan, a second scan of the reconstructed
/// function's body and terminator instructions, the function's edge
/// roster, twice the branch's own out-edge count for the path walk,
/// the member's surface against each crossed position's, the member's
/// surface plus each crossed edge's instruction and transport surfaces,
/// and the function's three rosters.
fn measured_steps(
    plan: &SelectedInstructionPlan,
    reconstructed: &Reconstructed<'_>,
) -> Result<u64, JoinRelocationError> {
    let function = reconstructed.function;
    let member_instruction = &function.blocks[reconstructed.crossing.run_block].instructions
        [reconstructed.crossing.run_start];
    let target = &function.blocks[reconstructed.crossing.destination_block];
    let edge_limit = terminator_successors(&target.terminator).len() * 2;
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .and_then(|total| {
            function.blocks.iter().try_fold(total, |total, candidate| {
                candidate
                    .instructions
                    .iter()
                    .chain(std::iter::once(terminator_instruction(
                        &candidate.terminator,
                    )))
                    .try_fold(total, |total, _| total.checked_add(1))
            })
        })
        .and_then(|total| {
            function.blocks.iter().try_fold(total, |total, candidate| {
                terminator_successors(&candidate.terminator)
                    .iter()
                    .try_fold(total, |total, _| total.checked_add(1))
            })
        })
        .and_then(|total| total.checked_add(edge_limit))
        .and_then(|total| {
            reconstructed.crossing.positions.iter().try_fold(
                total,
                |total, (crossed_block, ordinals)| {
                    ordinals.iter().try_fold(total, |total, position| {
                        total
                            .checked_add(surface(member_instruction))?
                            .checked_add(surface(
                                &function.blocks[*crossed_block].instructions[*position],
                            ))
                    })
                },
            )
        })
        .and_then(|total| {
            reconstructed
                .crossing
                .edges
                .iter()
                .try_fold(total, |total, edge| {
                    total
                        .checked_add(surface(member_instruction))?
                        .checked_add(surface(edge.instruction))?
                        .checked_add(edge_surface(edge.successor))
                })
        })
        .and_then(|total| {
            total
                .checked_add(function.memory_accesses.len())?
                .checked_add(function.calls.len())?
                .checked_add(function.boundary_settlements.len())
        })
        .ok_or(JoinRelocationError::IdentityOverflow)?;
    u64::try_from(steps).map_err(|_| JoinRelocationError::IdentityOverflow)
}

/// Build the function the contract demands from the validator's own
/// record: the member leaves the join's body and lands at the head's
/// index, every later head position keeping its order one slot later.
/// The producer's transformation is not consulted; both sides derive
/// the same function from the source alone.
fn expect(reconstructed: &Reconstructed<'_>) -> SelectedFunction {
    let mut expected = reconstructed.function.clone();
    let member = expected.blocks[reconstructed.crossing.run_block]
        .instructions
        .remove(reconstructed.crossing.run_start);
    expected.blocks[reconstructed.crossing.destination_block]
        .instructions
        .insert(reconstructed.crossing.landing_index, member);
    expected
}

/// Move the member back: undoing the validator's expected edit must
/// restore the complete source by content — every crossed instruction,
/// every other block and instruction, register, roster row, call,
/// settlement, and edge included.
fn restore(
    reconstructed: &Reconstructed<'_>,
    proposed: &SelectedInstructionPlan,
) -> Result<SelectedInstructionPlan, JoinRelocationError> {
    let mut restored = proposed.clone();
    let function = restored
        .functions
        .get_mut(reconstructed.function_index)
        .ok_or(JoinRelocationError::ReplayMismatch)?;
    if function
        .blocks
        .get(reconstructed.crossing.run_block)
        .map(|block| block.id)
        != Some(reconstructed.block)
        || function
            .blocks
            .get(reconstructed.crossing.destination_block)
            .map(|block| block.id)
            != Some(reconstructed.target)
    {
        return Err(JoinRelocationError::ReplayMismatch);
    }
    if function.blocks[reconstructed.crossing.destination_block]
        .instructions
        .len()
        <= reconstructed.crossing.landing_index
    {
        return Err(JoinRelocationError::ReplayMismatch);
    }
    let member = function.blocks[reconstructed.crossing.destination_block]
        .instructions
        .remove(reconstructed.crossing.landing_index);
    let block = &mut function.blocks[reconstructed.crossing.run_block];
    if block.instructions.len() < reconstructed.crossing.run_start {
        return Err(JoinRelocationError::ReplayMismatch);
    }
    block
        .instructions
        .insert(reconstructed.crossing.run_start, member);
    Ok(restored)
}

/// Independently consume the proposed program: the validator
/// reconstructs the relocation's preconditions from the source,
/// requires the proposal to equal the function its own record produces,
/// and restores the complete source by content — every crossed
/// instruction, every other block and instruction, register, roster
/// row, call, settlement, and edge included. The producer's
/// `admission::admit` is never consulted, so a wrong legality decision
/// fails here even when the proposal matches the edit the producer
/// emitted.
pub fn validate_join_relocation(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedJoinRelocation, JoinRelocationError> {
    let reconstructed = reconstruct(source, function_index, member, destination, environment)?;
    if measured_steps(source.selected_plan(), &reconstructed)? > budget.validation_steps() {
        return Err(JoinRelocationError::WorkBudgetExceeded);
    }
    if proposed.functions.get(function_index) != Some(&expect(&reconstructed)) {
        return Err(JoinRelocationError::ReplayMismatch);
    }
    if restore(&reconstructed, &proposed)? != *source.selected_plan() {
        return Err(JoinRelocationError::ReplayMismatch);
    }
    Ok(ValidatedJoinRelocation {
        receipt: JoinRelocationReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}

#[cfg(test)]
mod independence_tests {
    use std::sync::Arc;

    use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget};
    use register_environment::{
        ValidatedTargetRegisterEnvironment, baseline_target_register_environment,
    };
    use register_model::RegisterInstructionConstraint;
    use selected_instructions::{
        SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedBoundarySettlement,
        SelectedBoundarySettlementPayload, SelectedFunction, SelectedInstruction,
        SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan, SelectedOperand,
        SelectedSuccessor, SelectedSuccessorRole, SelectedTerminator, VirtualRegister,
        VirtualRegisterId, VirtualRegisterOrigin,
    };
    use semantic_vocabulary::{
        BlockId, BoundaryMachineId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType,
        IntegerValue, MachineId, OperationId, ScalarType, ValueId,
    };
    use target::NativeTarget;
    use target_operations_to_selected_instructions::selected_instruction_plan_identity;
    use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    use super::{
        JoinRelocationError, JoinRelocationReceipt, ValidatedJoinRelocation,
        validate_join_relocation,
    };

    const LEAD: SelectedInstructionId = SelectedInstructionId(2);
    const LATE: SelectedInstructionId = SelectedInstructionId(3);
    const TRAIL: SelectedInstructionId = SelectedInstructionId(4);
    const BRANCH: SelectedInstructionId = SelectedInstructionId(5);
    const T_HEAD: SelectedInstructionId = SelectedInstructionId(6);
    const T_TAIL: SelectedInstructionId = SelectedInstructionId(7);
    const T_JUMP: SelectedInstructionId = SelectedInstructionId(8);
    const F_HEAD: SelectedInstructionId = SelectedInstructionId(9);
    const F_TAIL: SelectedInstructionId = SelectedInstructionId(10);
    const F_JUMP: SelectedInstructionId = SelectedInstructionId(11);
    const HEAD: SelectedInstructionId = SelectedInstructionId(12);
    const MOVING: SelectedInstructionId = SelectedInstructionId(13);
    const TAIL: SelectedInstructionId = SelectedInstructionId(14);
    const RET: SelectedInstructionId = SelectedInstructionId(15);
    const X_JUMP: SelectedInstructionId = SelectedInstructionId(30);

    const POINTER: VirtualRegisterId = VirtualRegisterId(0);
    const R_LEAD: VirtualRegisterId = VirtualRegisterId(1);
    const R_LATE: VirtualRegisterId = VirtualRegisterId(2);
    const R_TRAIL: VirtualRegisterId = VirtualRegisterId(3);
    const R_THEAD: VirtualRegisterId = VirtualRegisterId(4);
    const R_TTAIL: VirtualRegisterId = VirtualRegisterId(5);
    const R_FHEAD: VirtualRegisterId = VirtualRegisterId(6);
    const R_FTAIL: VirtualRegisterId = VirtualRegisterId(7);
    const R_HEAD: VirtualRegisterId = VirtualRegisterId(8);
    const R_MOVE: VirtualRegisterId = VirtualRegisterId(9);
    const R_TAIL: VirtualRegisterId = VirtualRegisterId(10);
    const R_BOUND: VirtualRegisterId = VirtualRegisterId(11);

    const BLOCK_B: SelectedBlockId = SelectedBlockId(0);
    const BLOCK_T: SelectedBlockId = SelectedBlockId(1);
    const BLOCK_F: SelectedBlockId = SelectedBlockId(2);
    const BLOCK_J: SelectedBlockId = SelectedBlockId(3);
    const BLOCK_X: SelectedBlockId = SelectedBlockId(4);
    const EDGE_BT: u64 = 20;
    const EDGE_BF: u64 = 21;
    const EDGE_TJ: u64 = 22;
    const EDGE_FJ: u64 = 23;
    const EDGE_XF: u64 = 26;

    fn instruction(
        id: SelectedInstructionId,
        kind: SelectedInstructionKind,
        row: &RegisterInstructionConstraint,
        registers: &[VirtualRegisterId],
    ) -> SelectedInstruction {
        SelectedInstruction {
            id,
            kind,
            constraint: row.key,
            operands: row
                .operands
                .iter()
                .zip(registers)
                .map(|(operand, register)| SelectedOperand {
                    operand: operand.operand,
                    virtual_register: *register,
                    access: operand.access,
                    class: operand.class,
                    fixed_view: operand.fixed_view,
                    tied_to: operand.tied_to,
                    early_clobber: operand.early_clobber,
                })
                .collect(),
            implicit_uses: row.implicit_uses.clone(),
            implicit_defs: row.implicit_defs.clone(),
            clobbers: row.clobbers.clone(),
            provenance: Default::default(),
        }
    }

    fn register(
        id: VirtualRegisterId,
        class: register_model::RegisterClassId,
        origin: VirtualRegisterOrigin,
    ) -> VirtualRegister {
        VirtualRegister {
            id,
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
            class,
            origin,
            definition_site: None,
            entry_fixed_view: None,
        }
    }

    fn result_register(
        id: VirtualRegisterId,
        class: register_model::RegisterClassId,
        instruction: SelectedInstructionId,
        source_value: u64,
    ) -> VirtualRegister {
        register(
            id,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction,
                source_value: ValueId::new(source_value).unwrap(),
            },
        )
    }

    fn registers(class: register_model::RegisterClassId) -> Vec<VirtualRegister> {
        vec![
            VirtualRegister {
                id: POINTER,
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                ),
                class,
                origin: VirtualRegisterOrigin::EntryParameter {
                    source_value: ValueId::new(1).unwrap(),
                    parameter_index: 0,
                },
                definition_site: None,
                entry_fixed_view: None,
            },
            result_register(R_LEAD, class, LEAD, 2),
            result_register(R_LATE, class, LATE, 3),
            result_register(R_TRAIL, class, TRAIL, 4),
            result_register(R_THEAD, class, T_HEAD, 5),
            result_register(R_TTAIL, class, T_TAIL, 6),
            result_register(R_FHEAD, class, F_HEAD, 7),
            result_register(R_FTAIL, class, F_TAIL, 8),
            result_register(R_HEAD, class, HEAD, 9),
            result_register(R_MOVE, class, MOVING, 10),
            result_register(R_TAIL, class, TAIL, 11),
            result_register(R_BOUND, class, LEAD, 12),
        ]
    }

    fn successor(block: SelectedBlockId, source_target: u64, edge: u64) -> SelectedSuccessor {
        SelectedSuccessor {
            role: SelectedSuccessorRole::Semantic,
            structural_case: None,
            structural_bindings: Vec::new(),
            psi_edge: EdgeId::new(edge).unwrap(),
            block,
            source_target: BlockId::new(source_target).unwrap(),
            bindings: Vec::new(),
            fuel: Vec::new(),
        }
    }

    fn settlement(
        block: SelectedBlockId,
        position: u32,
        operation: u64,
    ) -> SelectedBoundarySettlement {
        SelectedBoundarySettlement {
            block,
            instruction_index: position,
            settlement: SelectedBoundarySettlementPayload::HostedWriteByteI32 {
                operation: OperationId::new(operation).unwrap(),
                boundary: BoundaryMachineId::new(1).unwrap(),
                source: ValueId::new(9).unwrap(),
            },
        }
    }

    fn materialization(
        environment: &ValidatedTargetRegisterEnvironment,
        id: SelectedInstructionId,
        register: VirtualRegisterId,
        value: u128,
    ) -> SelectedInstruction {
        let row = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap();
        instruction(
            id,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(value),
            },
            row,
            &[register],
        )
    }

    fn jump_terminator(
        environment: &ValidatedTargetRegisterEnvironment,
        jump: SelectedInstructionId,
        block: SelectedBlockId,
        source_target: u64,
        edge: u64,
    ) -> SelectedTerminator {
        let row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        SelectedTerminator::Jump {
            instruction: instruction(jump, SelectedInstructionKind::Jump, row, &[]),
            successor: successor(block, source_target, edge),
        }
    }

    fn return_terminator(
        environment: &ValidatedTargetRegisterEnvironment,
        instruction_id: SelectedInstructionId,
        edge: u64,
    ) -> SelectedTerminator {
        let row = environment
            .constraint(environment.selected_keys().return_unit)
            .unwrap();
        SelectedTerminator::Return {
            instruction: instruction(
                instruction_id,
                SelectedInstructionKind::ReturnUnit,
                row,
                &[],
            ),
            psi_return_edge: EdgeId::new(edge).unwrap(),
        }
    }

    /// A raw selected-stage unit fixture, not a source/Terminal
    /// admission claim: `B = [LEAD; LATE; TRAIL] -> branch -> {T, F}`,
    /// `T = [T_HEAD; T_TAIL] -> jump -> J`, `F = [F_HEAD; F_TAIL] ->
    /// jump -> J`, `J = [HEAD; MOVING; TAIL] -> return`, moving the
    /// member `MOVING` onto `TRAIL`'s position in the head. Each test
    /// mutates one legality premise, so the source names a relocation
    /// the contract must refuse while a defective producer would still
    /// emit it.
    fn plan(
        environment: &ValidatedTargetRegisterEnvironment,
        target: NativeTarget,
    ) -> SelectedInstructionPlan {
        let branch_row = environment
            .constraint(environment.selected_keys().conditional_branch)
            .unwrap()
            .clone();
        let machine = MachineId::new(1).unwrap();
        SelectedInstructionPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([1; 32]),
            },
            fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
            target,
            entry: machine,
            functions: vec![SelectedFunction {
                machine,
                attachment: None,
                provenance: Default::default(),
                structural: None,
                local_storage_slots: Vec::new(),
                outgoing_arguments: Vec::new(),
                calls: Vec::new(),
                normalized_foreign_calls: Vec::new(),
                memory_accesses: Vec::new(),
                boundary_settlements: Vec::new(),
                entry_block: BLOCK_B,
                virtual_registers: registers(
                    environment
                        .constraint(environment.selected_keys().materialize_i64)
                        .unwrap()
                        .operands[0]
                        .class,
                ),
                blocks: vec![
                    SelectedBlock {
                        id: BLOCK_B,
                        origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
                        instructions: vec![
                            materialization(environment, LEAD, R_LEAD, 5),
                            materialization(environment, LATE, R_LATE, 6),
                            materialization(environment, TRAIL, R_TRAIL, 9),
                        ],
                        terminator: SelectedTerminator::ConditionalBranch {
                            instruction: instruction(
                                BRANCH,
                                SelectedInstructionKind::ConditionalBranchNonZero,
                                &branch_row,
                                &[],
                            ),
                            when_nonzero: successor(BLOCK_T, 2, EDGE_BT),
                            when_zero: successor(BLOCK_F, 3, EDGE_BF),
                        },
                    },
                    SelectedBlock {
                        id: BLOCK_T,
                        origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
                        instructions: vec![
                            materialization(environment, T_HEAD, R_THEAD, 11),
                            materialization(environment, T_TAIL, R_TTAIL, 13),
                        ],
                        terminator: jump_terminator(environment, T_JUMP, BLOCK_J, 4, EDGE_TJ),
                    },
                    SelectedBlock {
                        id: BLOCK_F,
                        origin: SelectedBlockOrigin::Source(BlockId::new(3).unwrap()),
                        instructions: vec![
                            materialization(environment, F_HEAD, R_FHEAD, 15),
                            materialization(environment, F_TAIL, R_FTAIL, 17),
                        ],
                        terminator: jump_terminator(environment, F_JUMP, BLOCK_J, 4, EDGE_FJ),
                    },
                    SelectedBlock {
                        id: BLOCK_J,
                        origin: SelectedBlockOrigin::Source(BlockId::new(4).unwrap()),
                        instructions: vec![
                            materialization(environment, HEAD, R_HEAD, 19),
                            materialization(environment, MOVING, R_MOVE, 7),
                            materialization(environment, TAIL, R_TAIL, 23),
                        ],
                        terminator: return_terminator(environment, RET, 24),
                    },
                ],
            }]
            .into(),
        }
    }

    fn source(plan: SelectedInstructionPlan) -> ValidatedJoinRelocation {
        let identity = selected_instruction_plan_identity(&plan);
        ValidatedJoinRelocation {
            transformed: Arc::new(plan),
            receipt: JoinRelocationReceipt {
                source_selected: identity,
                transformed_selected: identity,
                optimization_unit: OptimizationUnitIdentity::from_bytes([2; 32]),
                fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
            },
        }
    }

    /// The proposal a defective producer emits for this source: the
    /// member leaves the join's body and lands at `TRAIL`'s index in the
    /// head — the edit the contract demands only when the relocation's
    /// legality holds.
    fn forged(source: &ValidatedJoinRelocation) -> SelectedInstructionPlan {
        let mut proposed = source.transformed().clone();
        let member = proposed.functions[0].blocks[3].instructions.remove(1);
        proposed.functions[0].blocks[0]
            .instructions
            .insert(2, member);
        proposed
    }

    fn budget() -> OptimizationWorkBudget {
        OptimizationWorkBudget::new(100, 100, 100_000, 100, 100).unwrap()
    }

    /// A legal relocation validates on a forged proposal alone: the
    /// validator reconstructs the contract edit from the source and the
    /// proposal needs no producer receipt to pass. The member lands at
    /// `TRAIL`'s index, and the receipt binds the proposal's identity
    /// rather than the source's.
    #[test]
    fn a_legal_forged_relocation_validates() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let source = source(plan(&environment, target));
        let proposed = forged(&source);
        let proposed_identity = selected_instruction_plan_identity(&proposed);
        let validated =
            validate_join_relocation(&source, 0, MOVING, TRAIL, &environment, budget(), proposed)
                .unwrap();
        let moved = &validated.transformed().functions[0];
        let order = |block: &SelectedBlock| {
            block
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>()
        };
        assert_eq!(order(&moved.blocks[0]), vec![LEAD, LATE, MOVING, TRAIL]);
        assert_eq!(order(&moved.blocks[3]), vec![HEAD, TAIL]);
        assert_eq!(
            validated.receipt().transformed_selected(),
            proposed_identity
        );
        assert_eq!(
            validated.receipt().source_selected(),
            source.receipt().source_selected()
        );
    }

    /// `T_TAIL` copies `R_MOVE` into `R_TTAIL` — reading the register
    /// the member defines inside a crossed arm. Hoisting the member past
    /// that read hands the copy a different value. A producer that
    /// skipped the coupling audit still emits the move; the validator's
    /// own hazard walk refuses with `UnsupportedPair`, not merely a diff
    /// of the proposal. Feeding that forged proposal is the observable
    /// proof that validation no longer relies on the producer's
    /// admission routine.
    #[test]
    fn validator_refuses_a_member_coupled_with_a_crossed_arm() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        let mut plan = plan(&environment, target);
        plan.functions[0].blocks[1].instructions[1] = instruction(
            T_TAIL,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[R_MOVE, R_TTAIL],
        );
        let source = source(plan);
        assert_eq!(
            validate_join_relocation(
                &source,
                0,
                MOVING,
                TRAIL,
                &environment,
                budget(),
                forged(&source)
            )
            .unwrap_err(),
            JoinRelocationError::UnsupportedPair
        );
    }

    /// `store [r0], r_move` with no roster row — a memory-capable kind
    /// the roster does not account for can never trade order, as member
    /// or as crossed position. A producer that forgot the schedulable
    /// bar still emits the move; the validator's own audit refuses with
    /// `UnsupportedInstruction`.
    #[test]
    fn validator_refuses_an_unaccounted_memory_member() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
        let mut plan = plan(&environment, target);
        plan.functions[0].blocks[3].instructions[1] = instruction(
            MOVING,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            &store,
            &[POINTER, R_MOVE],
        );
        let source = source(plan);
        assert_eq!(
            validate_join_relocation(
                &source,
                0,
                MOVING,
                TRAIL,
                &environment,
                budget(),
                forged(&source)
            )
            .unwrap_err(),
            JoinRelocationError::UnsupportedInstruction
        );
    }

    /// A boundary settlement inside a crossed arm observes the member
    /// execute before that arm's point after the move where it executed
    /// after it before. A producer that skipped the settlement scan
    /// still emits the move; the validator's own walk refuses with
    /// `UnsupportedPair`.
    #[test]
    fn validator_refuses_a_settlement_inside_a_crossed_arm() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let mut plan = plan(&environment, target);
        plan.functions[0]
            .boundary_settlements
            .push(settlement(BLOCK_T, 0, 7));
        let source = source(plan);
        assert_eq!(
            validate_join_relocation(
                &source,
                0,
                MOVING,
                TRAIL,
                &environment,
                budget(),
                forged(&source)
            )
            .unwrap_err(),
            JoinRelocationError::UnsupportedPair
        );
    }

    /// A second edge into an arm leaves a block other than the one fork
    /// head — a partial supply: the join would hand its readers a member
    /// that never ran on that path. A producer that forgot the totality
    /// count still emits the move; the validator's own predecessor scan
    /// refuses with `UnsupportedPair`.
    #[test]
    fn validator_refuses_a_second_head_into_an_arm() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let mut plan = plan(&environment, target);
        plan.functions[0].blocks.push(SelectedBlock {
            id: BLOCK_X,
            origin: SelectedBlockOrigin::Source(BlockId::new(5).unwrap()),
            instructions: Vec::new(),
            terminator: jump_terminator(&environment, X_JUMP, BLOCK_F, 3, EDGE_XF),
        });
        let source = source(plan);
        assert_eq!(
            validate_join_relocation(
                &source,
                0,
                MOVING,
                TRAIL,
                &environment,
                budget(),
                forged(&source)
            )
            .unwrap_err(),
            JoinRelocationError::UnsupportedPair
        );
    }

    /// A fork edge leaving the region would run the member on a
    /// traversal the join never saw — the other direction of the same
    /// totality failure. A producer that forgot the reach check still
    /// emits the move; the validator's own edge scan refuses with
    /// `UnsupportedPair`.
    #[test]
    fn validator_refuses_a_branch_edge_leaving_the_region() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let mut plan = plan(&environment, target);
        let when_zero = match &mut plan.functions[0].blocks[0].terminator {
            SelectedTerminator::ConditionalBranch { when_zero, .. } => when_zero,
            _ => unreachable!(),
        };
        when_zero.block = BLOCK_X;
        let source = source(plan);
        assert_eq!(
            validate_join_relocation(
                &source,
                0,
                MOVING,
                TRAIL,
                &environment,
                budget(),
                forged(&source)
            )
            .unwrap_err(),
            JoinRelocationError::UnsupportedPair
        );
    }

    /// An arm's `Jump` edge carrying continuation custody is no longer a
    /// plain semantic successor — boundary work the move does not cross.
    /// A producer that dropped the edge check still emits the move; the
    /// validator's own audit refuses with `UnsupportedPair`.
    #[test]
    fn validator_refuses_a_non_plain_arm_edge() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let mut plan = plan(&environment, target);
        let arm_edge = match &mut plan.functions[0].blocks[2].terminator {
            SelectedTerminator::Jump { successor, .. } => successor,
            _ => unreachable!(),
        };
        arm_edge.role = SelectedSuccessorRole::EdgeTransferContinuation;
        let source = source(plan);
        assert_eq!(
            validate_join_relocation(
                &source,
                0,
                MOVING,
                TRAIL,
                &environment,
                budget(),
                forged(&source)
            )
            .unwrap_err(),
            JoinRelocationError::UnsupportedPair
        );
    }
}
