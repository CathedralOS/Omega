//! Independent validation of cross-triangle run relocation.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! relocation's legality from the source records — the named run's span
//! inside the branching block's body, the two-successor conditional
//! branch on plain semantic edges, the one branch target that is the
//! join itself — the bypass — the arms the branch's other targets must
//! be, the join's predecessor census, the landing index the destination
//! names in the join's body, and the shared crossed-window audit every
//! crossed position and edge must pass — then requires the proposal to
//! place exactly the run's members on the landing index in the join
//! block in their original order, and moving the run back must restore
//! the complete source by content. A producer admission error therefore
//! fails validation even when the proposal is exactly what that
//! producer emitted.
use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedBlockId, SelectedBlockOrigin, SelectedFunction, SelectedInstructionId,
    SelectedInstructionPlan, SelectedSuccessor, SelectedTerminator,
};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{BypassRunRelocationError, BypassRunRelocationReceipt, ValidatedBypassRunRelocation};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{
    CrossingDirection, all_edges, crossed_window, edge_surface, plain_edge, terminator_instruction,
    terminator_successors,
};
use crate::rewrites::window_hazards::{RunRelocationRejection, admit_run_relocation, surface};

/// The validator's own reconstruction of the relocation the contract
/// permits: the admitted run's span inside the triangle's branching
/// head, the join's index, and the landing index the destination names
/// in the join's body. It shares no state with the producer's
/// `admission` record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    /// The run's own block: the triangle's branching head.
    block_index: usize,
    /// The run's first member index in the block body.
    first_index: usize,
    /// The run's last member index; the run is the contiguous span
    /// `first_index..=last_index` of at least two members.
    last_index: usize,
    /// The join block's index in `function.blocks`.
    target_index: usize,
    /// The destination instruction's position in the join body: the run
    /// lands at this index. Naming the join's terminator-carried
    /// instruction lands the run at the body end, index
    /// `target.instructions.len()`.
    landing_index: usize,
}

/// Keeps the family's typed rejection vocabulary over the shared audit's
/// rejection kinds: an unschedulable member or crossed position is the
/// instruction-level refusal and every window-level refusal is the pair
/// kind.
fn reject(rejection: RunRelocationRejection) -> BypassRunRelocationError {
    match rejection {
        RunRelocationRejection::Unschedulable => BypassRunRelocationError::UnsupportedInstruction,
        RunRelocationRejection::UnreachableDestination
        | RunRelocationRejection::Coupled
        | RunRelocationRejection::MemoryOrdering
        | RunRelocationRejection::TransportConflict
        | RunRelocationRejection::NonPlainEdge
        | RunRelocationRejection::Settlement => BypassRunRelocationError::UnsupportedPair,
    }
}

/// The index of the block an edge lands on.
fn block_index_of(function: &SelectedFunction, id: SelectedBlockId) -> Option<usize> {
    function
        .blocks
        .iter()
        .position(|candidate| candidate.id == id)
}

/// Whether `arm_index` is a plain arm of this bypassed triangle: a source
/// block — never the run's own block, which would make the move
/// self-referential, and never the entry block, which is reached with no
/// predecessor at all — that the branch's edges alone reach, ending in an
/// unconditional `Jump` to the join on a plain semantic edge. A second
/// predecessor into the arm would give the join a path the run never
/// executed on, and an implementation block's origin carries edge or
/// case work the bounded audit does not cross.
fn arm_into(
    function: &SelectedFunction,
    branch_edges: &[&SelectedSuccessor],
    head: SelectedBlockId,
    arm_index: usize,
    join: SelectedBlockId,
) -> bool {
    let arm = &function.blocks[arm_index];
    if arm.id == head
        || arm.id == function.entry_block
        || !matches!(arm.origin, SelectedBlockOrigin::Source(_))
    {
        return false;
    }
    let branch_edges_into_arm = branch_edges
        .iter()
        .filter(|edge| edge.block == arm.id)
        .count();
    if all_edges(function)
        .filter(|(_, edge)| edge.block == arm.id)
        .count()
        != branch_edges_into_arm
    {
        return false;
    }
    let SelectedTerminator::Jump {
        successor: arm_edge,
        ..
    } = &arm.terminator
    else {
        return false;
    };
    plain_edge(arm_edge) && arm_edge.block == join
}

/// Reconstruct the legality of relocating the run `first_member` and
/// `last_member` bound through their block's bypassed branch onto
/// `destination` from the source records: locate the run in one block
/// in the named order, require that block's terminator to be a
/// two-successor conditional branch on plain edges, find the one
/// distinct branch target that admits as the join — every other
/// distinct target an arm ending in a plain `Jump` to it, and every
/// edge into it leaving the head or an arm — resolve the landing index
/// in the join's body, and re-derive the window's independence audit
/// through the shared `crossed_window`/`admit_run_relocation` contract —
/// no register or condition-state hazard between any member and any
/// crossed position, no member interference with any crossed edge's
/// register transports, no roster-carrying run sharing the window with
/// a second memory-access actor, no barrier, call, hosted effect, or
/// call-roster entry inside the window, and no boundary settlement
/// whose observed executed prefix changes — then account the family's
/// measured steps against the budget. Nothing in this audit reads the
/// producer's admission decision, so a producer-side legality error
/// fails here even when the proposal matches the emitted edit.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    first_member: SelectedInstructionId,
    last_member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Reconstructed<'source>, BypassRunRelocationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(BypassRunRelocationError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(BypassRunRelocationError::SourceMismatch)?;
    let (block_index, first_index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .position(|instruction| instruction.id == first_member)
                .map(|first_index| (block_index, first_index))
        })
        .ok_or(BypassRunRelocationError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    // The run is the contiguous span the two named members bound in this
    // block, in the named order; a repeated id or a last member that does
    // not follow the first names no multi-member run. A single member is
    // the one-instruction cross-triangle relocation the sibling family
    // already proves.
    let last_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == last_member)
        .filter(|position| *position > first_index)
        .ok_or(BypassRunRelocationError::UnsupportedPair)?;
    let run = &block.instructions[first_index..=last_index];
    // Only a two-successor conditional terminator gives the run's block
    // the branch this step crosses; every other terminator shape is the
    // single-edge family's case or no relocation at all.
    let branch_edges: Vec<&SelectedSuccessor> = match &block.terminator {
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
            return Err(BypassRunRelocationError::UnsupportedPair);
        }
    };
    for edge in &branch_edges {
        if !plain_edge(edge) {
            return Err(BypassRunRelocationError::UnsupportedPair);
        }
    }
    // The join is a branch target itself — the bypass edge lands on it
    // directly, which is exactly where this family parts from the
    // all-arms diamond. Each distinct edge target is tried in turn: a
    // candidate admits when every other distinct target is an arm ending
    // in a `Jump` to it and every edge into it leaves the head or an
    // arm. At most one candidate can admit — a second would make each an
    // arm of the other, and a block jumping to a candidate join is a
    // predecessor that candidate's arm check refuses — so the first and
    // only match stands; zero matches is the diamond family's shape or
    // no converging move at all.
    let mut candidates: Vec<usize> = Vec::new();
    for edge in &branch_edges {
        let index =
            block_index_of(function, edge.block).ok_or(BypassRunRelocationError::SourceMismatch)?;
        if !candidates.contains(&index) {
            candidates.push(index);
        }
    }
    let mut found: Option<usize> = None;
    for &join_index in &candidates {
        let join = &function.blocks[join_index];
        // The join must not collapse back into the triangle: landing
        // inside the run's own block is an in-block or self-loop move
        // this step does not cross, the entry block is reached with no
        // predecessor at all, and an implementation block's origin
        // carries edge or case work the bounded audit does not cross.
        if join.id == block.id
            || join.id == function.entry_block
            || !matches!(join.origin, SelectedBlockOrigin::Source(_))
        {
            continue;
        }
        let mut arm_indices: Vec<usize> = Vec::new();
        let mut closes = true;
        for &arm_index in &candidates {
            if arm_index == join_index {
                continue;
            }
            if arm_into(function, &branch_edges, block.id, arm_index, join.id) {
                arm_indices.push(arm_index);
            } else {
                closes = false;
                break;
            }
        }
        if !closes {
            continue;
        }
        // Every edge into the join must leave the head or an arm: a
        // predecessor anywhere else — including the join's own self-loop —
        // gives the join a path the run would newly execute on or execute
        // a second time. Head edges and arm edges into the join are
        // already enumerated, so the per-source check is complete.
        for (source_block, _) in all_edges(function).filter(|(_, edge)| edge.block == join.id) {
            if source_block != block.id
                && !arm_indices
                    .iter()
                    .any(|&arm| function.blocks[arm].id == source_block)
            {
                closes = false;
                break;
            }
        }
        if !closes {
            continue;
        }
        if found.is_some() {
            return Err(BypassRunRelocationError::UnsupportedPair);
        }
        found = Some(join_index);
    }
    let target_index = found.ok_or(BypassRunRelocationError::UnsupportedPair)?;
    let target = &function.blocks[target_index];
    // The destination names a position in the join body — the run lands
    // at its index — or the join's terminator-carried instruction,
    // landing the run at the body end.
    let landing_index = target
        .instructions
        .iter()
        .position(|instruction| instruction.id == destination)
        .or_else(|| {
            (terminator_instruction(&target.terminator).id == destination)
                .then_some(target.instructions.len())
        })
        .ok_or(BypassRunRelocationError::UnsupportedPair)?;
    // The crossed window is the shared derivation rather than this
    // family's own enumeration: the gates above leave only the bypass
    // head-to-join path and one head-to-arm-to-join path per arm, and
    // the walk pushes each branch edge once and each arm's `Jump` edge
    // once per branch edge feeding it — at most two pushes per branch
    // edge — so twice the branch's own out-edge count bounds it. The
    // shared audit applies the hazard, memory-roster, transport, and
    // settlement checks once across the run's members: a boundary
    // settlement positioned past the run's first index in the head or
    // past the landing index in the join observed a changed executed
    // prefix and refuses, and so does a settlement inside a crossed arm
    // — every member executes after that arm's point after the move
    // where it executed before it before.
    let edge_limit = branch_edges.len() * 2;
    let crossing = crossed_window(
        function,
        block_index,
        first_index,
        last_index,
        target_index,
        landing_index,
        CrossingDirection::Forward,
        edge_limit,
    )
    .ok_or(BypassRunRelocationError::WorkBudgetExceeded)?;
    let members: Vec<_> = run.iter().collect();
    admit_run_relocation(function, &members, &crossing).map_err(reject)?;
    // The validator's own audit walks the same surfaces the family
    // publishes: every block body and terminator instruction once to
    // locate the run's first member, and again with successor edges to
    // count the arms' and the join's predecessor edges; the path walk is
    // bounded by two pushes per branch edge; the window audit walks
    // every member-against-crossed operand and unit surface plus each
    // crossed edge's own surface, and the function's three rosters.
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
            run.iter().try_fold(total, |total, member| {
                crossing
                    .positions
                    .iter()
                    .try_fold(total, |total, (crossed_block, positions)| {
                        positions.iter().try_fold(total, |total, position| {
                            total.checked_add(surface(member))?.checked_add(surface(
                                &function.blocks[*crossed_block].instructions[*position],
                            ))
                        })
                    })
            })
        })
        .and_then(|total| {
            run.iter().try_fold(total, |total, member| {
                crossing.edges.iter().try_fold(total, |total, edge| {
                    total
                        .checked_add(surface(member))?
                        .checked_add(surface(edge.instruction))?
                        .checked_add(edge_surface(edge.successor))
                })
            })
        })
        .and_then(|total| {
            total
                .checked_add(function.memory_accesses.len())?
                .checked_add(function.calls.len())?
                .checked_add(function.boundary_settlements.len())
        })
        .ok_or(BypassRunRelocationError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| BypassRunRelocationError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(BypassRunRelocationError::WorkBudgetExceeded);
    }
    Ok(Reconstructed {
        function,
        block_index,
        first_index,
        last_index,
        target_index,
        landing_index,
    })
}

/// Independently consume the proposed program: the validator re-derives
/// the admitted run, triangle, and landing index from the source without
/// the producer's admission routine, the proposal must place exactly the
/// run's members on the landing index in the join block in their
/// original order, and moving the run back must restore the complete
/// source by content — every crossed instruction, every other block and
/// instruction, register, roster row, call, settlement, and edge
/// included.
pub fn validate_bypass_run_relocation(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    first_member: SelectedInstructionId,
    last_member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedBypassRunRelocation, BypassRunRelocationError> {
    let reconstructed = reconstruct(
        source,
        function_index,
        first_member,
        last_member,
        destination,
        environment,
        budget,
    )?;
    let run_len = reconstructed.last_index - reconstructed.first_index + 1;
    if proposed
        .functions
        .get(function_index)
        .and_then(|function| function.blocks.get(reconstructed.target_index))
        .and_then(|block| {
            block
                .instructions
                .get(reconstructed.landing_index..reconstructed.landing_index + run_len)
        })
        != Some(
            &reconstructed.function.blocks[reconstructed.block_index].instructions
                [reconstructed.first_index..=reconstructed.last_index],
        )
    {
        return Err(BypassRunRelocationError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    let run: Vec<_> = restored.functions[function_index].blocks[reconstructed.target_index]
        .instructions
        .drain(reconstructed.landing_index..reconstructed.landing_index + run_len)
        .collect();
    restored.functions[function_index].blocks[reconstructed.block_index]
        .instructions
        .splice(reconstructed.first_index..reconstructed.first_index, run);
    if restored != *source.selected_plan() {
        return Err(BypassRunRelocationError::ReplayMismatch);
    }
    Ok(ValidatedBypassRunRelocation {
        receipt: BypassRunRelocationReceipt {
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
        BypassRunRelocationError, BypassRunRelocationReceipt, ValidatedBypassRunRelocation,
        validate_bypass_run_relocation,
    };

    const LEAD: SelectedInstructionId = SelectedInstructionId(2);
    const RUN_A: SelectedInstructionId = SelectedInstructionId(3);
    const RUN_B: SelectedInstructionId = SelectedInstructionId(4);
    const TRAIL: SelectedInstructionId = SelectedInstructionId(5);
    const T_HEAD: SelectedInstructionId = SelectedInstructionId(6);
    const T_TAIL: SelectedInstructionId = SelectedInstructionId(7);
    const HEAD: SelectedInstructionId = SelectedInstructionId(8);
    const MID: SelectedInstructionId = SelectedInstructionId(9);
    const TAIL: SelectedInstructionId = SelectedInstructionId(10);
    const BRANCH: SelectedInstructionId = SelectedInstructionId(11);
    const T_JUMP: SelectedInstructionId = SelectedInstructionId(12);
    const RET: SelectedInstructionId = SelectedInstructionId(13);
    const T2_HEAD: SelectedInstructionId = SelectedInstructionId(14);
    const T2_JUMP: SelectedInstructionId = SelectedInstructionId(15);
    const FOREIGN_JUMP: SelectedInstructionId = SelectedInstructionId(16);

    const POINTER: VirtualRegisterId = VirtualRegisterId(0);
    const R_LEAD: VirtualRegisterId = VirtualRegisterId(1);
    const R_MOVE_A: VirtualRegisterId = VirtualRegisterId(2);
    const R_MOVE_B: VirtualRegisterId = VirtualRegisterId(3);
    const R_TRAIL: VirtualRegisterId = VirtualRegisterId(4);
    const R_THEAD: VirtualRegisterId = VirtualRegisterId(5);
    const R_TTAIL: VirtualRegisterId = VirtualRegisterId(6);
    const R_HEAD: VirtualRegisterId = VirtualRegisterId(7);
    const R_MID: VirtualRegisterId = VirtualRegisterId(8);
    const R_TAIL: VirtualRegisterId = VirtualRegisterId(9);
    const R_T2HEAD: VirtualRegisterId = VirtualRegisterId(10);

    const BLOCK_B: SelectedBlockId = SelectedBlockId(0);
    const BLOCK_T: SelectedBlockId = SelectedBlockId(1);
    const BLOCK_J: SelectedBlockId = SelectedBlockId(2);
    const BLOCK_T2: SelectedBlockId = SelectedBlockId(3);
    const BLOCK_C: SelectedBlockId = SelectedBlockId(4);

    const EDGE_BT: u64 = 20;
    const EDGE_BJ: u64 = 21;
    const EDGE_TJ: u64 = 22;
    const EDGE_BT2: u64 = 24;
    const EDGE_T2J: u64 = 25;
    const EDGE_CJ: u64 = 26;
    const EDGE_CT: u64 = 27;

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

    fn materialization(
        environment: &ValidatedTargetRegisterEnvironment,
        id: SelectedInstructionId,
        register: VirtualRegisterId,
        value: u64,
    ) -> SelectedInstruction {
        instruction(
            id,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(value.into()),
            },
            environment
                .constraint(environment.selected_keys().materialize_i64)
                .unwrap(),
            &[register],
        )
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
        value: u64,
    ) -> VirtualRegister {
        register(
            id,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction,
                source_value: ValueId::new(value).unwrap(),
            },
        )
    }

    fn registers(class: register_model::RegisterClassId) -> Vec<VirtualRegister> {
        vec![
            register(
                POINTER,
                class,
                VirtualRegisterOrigin::EntryParameter {
                    source_value: ValueId::new(1).unwrap(),
                    parameter_index: 0,
                },
            ),
            result_register(R_LEAD, class, LEAD, 2),
            result_register(R_MOVE_A, class, RUN_A, 3),
            result_register(R_MOVE_B, class, RUN_B, 4),
            result_register(R_TRAIL, class, TRAIL, 5),
            result_register(R_THEAD, class, T_HEAD, 6),
            result_register(R_TTAIL, class, T_TAIL, 7),
            result_register(R_HEAD, class, HEAD, 8),
            result_register(R_MID, class, MID, 9),
            result_register(R_TAIL, class, TAIL, 10),
            result_register(R_T2HEAD, class, T2_HEAD, 11),
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

    fn jump_terminator(
        environment: &ValidatedTargetRegisterEnvironment,
        instruction_id: SelectedInstructionId,
        block: SelectedBlockId,
        source_target: u64,
        edge: u64,
    ) -> SelectedTerminator {
        SelectedTerminator::Jump {
            instruction: instruction(
                instruction_id,
                SelectedInstructionKind::Jump,
                environment
                    .constraint(environment.selected_keys().jump)
                    .unwrap(),
                &[],
            ),
            successor: successor(block, source_target, edge),
        }
    }

    fn return_terminator(
        environment: &ValidatedTargetRegisterEnvironment,
        instruction_id: SelectedInstructionId,
        edge: u64,
    ) -> SelectedTerminator {
        SelectedTerminator::Return {
            instruction: instruction(
                instruction_id,
                SelectedInstructionKind::ReturnUnit,
                environment
                    .constraint(environment.selected_keys().return_unit)
                    .unwrap(),
                &[],
            ),
            psi_return_edge: EdgeId::new(edge).unwrap(),
        }
    }

    fn block(
        id: SelectedBlockId,
        source_target: u64,
        instructions: Vec<SelectedInstruction>,
        terminator: SelectedTerminator,
    ) -> SelectedBlock {
        SelectedBlock {
            id,
            origin: SelectedBlockOrigin::Source(BlockId::new(source_target).unwrap()),
            instructions,
            terminator,
        }
    }

    /// The branching head `B`: the run's own block, ending in the
    /// two-successor conditional the family crosses.
    fn head(
        environment: &ValidatedTargetRegisterEnvironment,
        instructions: Vec<SelectedInstruction>,
        when_nonzero: SelectedSuccessor,
        when_zero: SelectedSuccessor,
    ) -> SelectedBlock {
        block(
            BLOCK_B,
            1,
            instructions,
            SelectedTerminator::ConditionalBranch {
                instruction: instruction(
                    BRANCH,
                    SelectedInstructionKind::ConditionalBranchNonZero,
                    environment
                        .constraint(environment.selected_keys().conditional_branch)
                        .unwrap(),
                    &[],
                ),
                when_nonzero,
                when_zero,
            },
        )
    }

    /// A plain arm: a source block ending in an unconditional `Jump` to
    /// the named target.
    fn arm(
        environment: &ValidatedTargetRegisterEnvironment,
        id: SelectedBlockId,
        source_target: u64,
        jump_instruction: SelectedInstructionId,
        instructions: Vec<SelectedInstruction>,
        target: SelectedBlockId,
        target_source: u64,
        edge: u64,
    ) -> SelectedBlock {
        block(
            id,
            source_target,
            instructions,
            jump_terminator(environment, jump_instruction, target, target_source, edge),
        )
    }

    /// A raw selected-stage unit fixture, not a source/Terminal admission
    /// claim. Each test mutates one legality premise, so the source names
    /// a relocation the contract must refuse while a defective producer
    /// would still emit it.
    fn plan(
        target: NativeTarget,
        blocks: Vec<SelectedBlock>,
        registers: Vec<VirtualRegister>,
        boundary_settlements: Vec<SelectedBoundarySettlement>,
    ) -> SelectedInstructionPlan {
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
                boundary_settlements,
                entry_block: BLOCK_B,
                virtual_registers: registers,
                blocks,
            }]
            .into(),
        }
    }

    fn source(plan: SelectedInstructionPlan) -> ValidatedBypassRunRelocation {
        let identity = selected_instruction_plan_identity(&plan);
        ValidatedBypassRunRelocation {
            transformed: Arc::new(plan),
            receipt: BypassRunRelocationReceipt {
                source_selected: identity,
                transformed_selected: identity,
                optimization_unit: OptimizationUnitIdentity::from_bytes([2; 32]),
                fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
            },
        }
    }

    /// The proposal a defective producer emits for this source: the run
    /// leaves the head's body as one span and lands at the join's head —
    /// the edit the contract demands only when the relocation's legality
    /// holds. Feeding it is the observable proof that validation no
    /// longer relies on the producer's admission routine.
    fn forged(source: &ValidatedBypassRunRelocation) -> SelectedInstructionPlan {
        let mut proposed = source.transformed().clone();
        let function = &mut proposed.functions[0];
        let head = function
            .blocks
            .iter()
            .position(|block| block.id == BLOCK_B)
            .unwrap();
        let run: Vec<_> = function.blocks[head].instructions.drain(1..=2).collect();
        let join = function
            .blocks
            .iter()
            .position(|block| block.id == BLOCK_J)
            .unwrap();
        function.blocks[join].instructions.splice(0..0, run);
        proposed
    }

    fn budget() -> OptimizationWorkBudget {
        OptimizationWorkBudget::new(100, 100, 100_000, 100, 100).unwrap()
    }

    /// `B = [LEAD; RUN_A; RUN_B; TRAIL] -> branch -> T = [T_HEAD; T_TAIL]
    /// -> jump -> J` and `B -> J = [HEAD; MID; TAIL] -> return`, moving
    /// `RUN_A..=RUN_B` onto `HEAD`'s position — the triangle whose bypass
    /// edge lands on the join directly.
    fn triangle(
        environment: &ValidatedTargetRegisterEnvironment,
        head_instructions: Vec<SelectedInstruction>,
        arm_instructions: Vec<SelectedInstruction>,
        join_instructions: Vec<SelectedInstruction>,
        extra_blocks: Vec<SelectedBlock>,
    ) -> Vec<SelectedBlock> {
        let mut blocks = vec![
            head(
                environment,
                head_instructions,
                successor(BLOCK_T, 2, EDGE_BT),
                successor(BLOCK_J, 3, EDGE_BJ),
            ),
            arm(
                environment,
                BLOCK_T,
                2,
                T_JUMP,
                arm_instructions,
                BLOCK_J,
                3,
                EDGE_TJ,
            ),
            block(
                BLOCK_J,
                3,
                join_instructions,
                return_terminator(environment, RET, 23),
            ),
        ];
        blocks.extend(extra_blocks);
        blocks
    }

    fn materialized_body(
        environment: &ValidatedTargetRegisterEnvironment,
    ) -> Vec<SelectedInstruction> {
        vec![
            materialization(environment, LEAD, R_LEAD, 5),
            materialization(environment, RUN_A, R_MOVE_A, 7),
            materialization(environment, RUN_B, R_MOVE_B, 9),
            materialization(environment, TRAIL, R_TRAIL, 11),
        ]
    }

    fn arm_body(environment: &ValidatedTargetRegisterEnvironment) -> Vec<SelectedInstruction> {
        vec![
            materialization(environment, T_HEAD, R_THEAD, 13),
            materialization(environment, T_TAIL, R_TTAIL, 15),
        ]
    }

    fn join_body(environment: &ValidatedTargetRegisterEnvironment) -> Vec<SelectedInstruction> {
        vec![
            materialization(environment, HEAD, R_HEAD, 17),
            materialization(environment, MID, R_MID, 19),
            materialization(environment, TAIL, R_TAIL, 21),
        ]
    }

    /// A legal relocation validates on a forged proposal alone: the
    /// validator reconstructs the contract edit from the source and the
    /// proposal needs no producer receipt to pass. The run lands at J's
    /// head in its own order, and the receipt binds the proposal's
    /// identity rather than the source's.
    #[test]
    fn a_legal_forged_relocation_validates() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let class = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap()
            .operands[0]
            .class;
        let plan = plan(
            target,
            triangle(
                &environment,
                materialized_body(&environment),
                arm_body(&environment),
                join_body(&environment),
                Vec::new(),
            ),
            registers(class),
            Vec::new(),
        );
        let source = source(plan);
        let proposed = forged(&source);
        let proposed_identity = selected_instruction_plan_identity(&proposed);
        let validated = validate_bypass_run_relocation(
            &source,
            0,
            RUN_A,
            RUN_B,
            HEAD,
            &environment,
            budget(),
            proposed,
        )
        .unwrap();
        let moved = &validated.transformed().functions[0];
        let order = |block: &SelectedBlock| {
            block
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>()
        };
        assert_eq!(order(&moved.blocks[0]), vec![LEAD, TRAIL]);
        assert_eq!(order(&moved.blocks[2]), vec![RUN_A, RUN_B, HEAD, MID, TAIL]);
        assert_eq!(
            validated.receipt().transformed_selected(),
            proposed_identity
        );
        assert_eq!(
            validated.receipt().source_selected(),
            source.receipt().source_selected()
        );
    }

    /// `RUN_B` copies `R_MOVE_A` into `R_MOVE_B` — internally coupled
    /// with `RUN_A`, which the run carries with it — while `TRAIL` copies
    /// `R_MOVE_B` into `R_TRAIL`, reading the register the run's second
    /// member defines. Moving the run past `TRAIL` hands the copy a
    /// different value. A producer that skipped the coupling audit still
    /// emits the move; the validator's own hazard walk refuses with
    /// `UnsupportedPair`, not merely a diff of the proposal.
    #[test]
    fn validator_refuses_a_member_coupled_with_the_crossed_tail() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.selected_keys();
        let copy = environment.constraint(keys.copy_i64).unwrap();
        let class = environment
            .constraint(keys.materialize_i64)
            .unwrap()
            .operands[0]
            .class;
        let plan = plan(
            target,
            triangle(
                &environment,
                vec![
                    materialization(&environment, LEAD, R_LEAD, 5),
                    materialization(&environment, RUN_A, R_MOVE_A, 7),
                    instruction(
                        RUN_B,
                        SelectedInstructionKind::CopyI64,
                        copy,
                        &[R_MOVE_A, R_MOVE_B],
                    ),
                    instruction(
                        TRAIL,
                        SelectedInstructionKind::CopyI64,
                        copy,
                        &[R_MOVE_B, R_TRAIL],
                    ),
                ],
                arm_body(&environment),
                join_body(&environment),
                Vec::new(),
            ),
            registers(class),
            Vec::new(),
        );
        let source = source(plan);
        assert_eq!(
            validate_bypass_run_relocation(
                &source,
                0,
                RUN_A,
                RUN_B,
                HEAD,
                &environment,
                budget(),
                forged(&source)
            )
            .unwrap_err(),
            BypassRunRelocationError::UnsupportedPair
        );
    }

    /// `store [r0], r7` with no roster row — a memory-capable kind the
    /// roster does not account for can never trade order, as member or as
    /// crossed position. A producer that forgot the schedulable bar still
    /// emits the move; the validator's own audit refuses with
    /// `UnsupportedInstruction`.
    #[test]
    fn validator_refuses_an_unaccounted_memory_member() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.selected_keys();
        let store = environment.constraint(keys.store.unwrap()).unwrap();
        let class = environment
            .constraint(keys.materialize_i64)
            .unwrap()
            .operands[0]
            .class;
        let plan = plan(
            target,
            triangle(
                &environment,
                vec![
                    materialization(&environment, LEAD, R_LEAD, 5),
                    materialization(&environment, RUN_A, R_MOVE_A, 7),
                    instruction(
                        RUN_B,
                        SelectedInstructionKind::Store {
                            byte_offset: 0,
                            byte_size: 8,
                        },
                        store,
                        &[POINTER, R_MOVE_B],
                    ),
                    materialization(&environment, TRAIL, R_TRAIL, 11),
                ],
                arm_body(&environment),
                join_body(&environment),
                Vec::new(),
            ),
            registers(class),
            Vec::new(),
        );
        let source = source(plan);
        assert_eq!(
            validate_bypass_run_relocation(
                &source,
                0,
                RUN_A,
                RUN_B,
                HEAD,
                &environment,
                budget(),
                forged(&source)
            )
            .unwrap_err(),
            BypassRunRelocationError::UnsupportedInstruction
        );
    }

    /// A boundary settlement on the run's block at index 2 sits inside
    /// the run's own span — past the run's first index: the relocation
    /// moves the members ahead of it, so the executed prefix the
    /// settlement observes changes. A producer that skipped the
    /// settlement scan still emits the move; the validator's own walk
    /// refuses with `UnsupportedPair`.
    #[test]
    fn validator_refuses_a_settlement_past_the_vacated_run() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let class = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap()
            .operands[0]
            .class;
        let plan = plan(
            target,
            triangle(
                &environment,
                materialized_body(&environment),
                arm_body(&environment),
                join_body(&environment),
                Vec::new(),
            ),
            registers(class),
            vec![settlement(BLOCK_B, 2, 7)],
        );
        let source = source(plan);
        assert_eq!(
            validate_bypass_run_relocation(
                &source,
                0,
                RUN_A,
                RUN_B,
                HEAD,
                &environment,
                budget(),
                forged(&source)
            )
            .unwrap_err(),
            BypassRunRelocationError::UnsupportedPair
        );
    }

    /// Both branch edges feeding arms — the all-arms diamond — names no
    /// bypass: no branch edge lands on the join directly, so the shape is
    /// the diamond family's case. A producer that mistook the diamond for
    /// this triangle still emits the run's move onto the common join; the
    /// validator's own candidate scan finds no admitting join and refuses
    /// with `UnsupportedPair`.
    #[test]
    fn validator_refuses_the_all_arms_diamond() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let class = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap()
            .operands[0]
            .class;
        let blocks = vec![
            head(
                &environment,
                materialized_body(&environment),
                successor(BLOCK_T, 2, EDGE_BT),
                successor(BLOCK_T2, 4, EDGE_BT2),
            ),
            arm(
                &environment,
                BLOCK_T,
                2,
                T_JUMP,
                arm_body(&environment),
                BLOCK_J,
                3,
                EDGE_TJ,
            ),
            arm(
                &environment,
                BLOCK_T2,
                4,
                T2_JUMP,
                vec![materialization(&environment, T2_HEAD, R_T2HEAD, 23)],
                BLOCK_J,
                3,
                EDGE_T2J,
            ),
            block(
                BLOCK_J,
                3,
                join_body(&environment),
                return_terminator(&environment, RET, 23),
            ),
        ];
        let plan = plan(target, blocks, registers(class), Vec::new());
        let source = source(plan);
        assert_eq!(
            validate_bypass_run_relocation(
                &source,
                0,
                RUN_A,
                RUN_B,
                HEAD,
                &environment,
                budget(),
                forged(&source)
            )
            .unwrap_err(),
            BypassRunRelocationError::UnsupportedPair
        );
    }

    /// A second edge into the join from a foreign block gives it a path
    /// the relocated run would newly execute on. A producer that forgot
    /// the join's predecessor census still emits the move; the
    /// validator's own edge scan refuses with `UnsupportedPair`.
    #[test]
    fn validator_refuses_a_second_predecessor_into_the_join() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let class = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap()
            .operands[0]
            .class;
        let plan = plan(
            target,
            triangle(
                &environment,
                materialized_body(&environment),
                arm_body(&environment),
                join_body(&environment),
                vec![arm(
                    &environment,
                    BLOCK_C,
                    5,
                    FOREIGN_JUMP,
                    Vec::new(),
                    BLOCK_J,
                    3,
                    EDGE_CJ,
                )],
            ),
            registers(class),
            Vec::new(),
        );
        let source = source(plan);
        assert_eq!(
            validate_bypass_run_relocation(
                &source,
                0,
                RUN_A,
                RUN_B,
                HEAD,
                &environment,
                budget(),
                forged(&source)
            )
            .unwrap_err(),
            BypassRunRelocationError::UnsupportedPair
        );
    }

    /// A second edge into the arm from a foreign block gives the join a
    /// path the run never executed on. A producer that forgot the arm's
    /// predecessor census still emits the move; the validator's own edge
    /// scan refuses with `UnsupportedPair`.
    #[test]
    fn validator_refuses_a_second_predecessor_into_the_arm() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let class = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap()
            .operands[0]
            .class;
        let plan = plan(
            target,
            triangle(
                &environment,
                materialized_body(&environment),
                arm_body(&environment),
                join_body(&environment),
                vec![arm(
                    &environment,
                    BLOCK_C,
                    5,
                    FOREIGN_JUMP,
                    Vec::new(),
                    BLOCK_T,
                    2,
                    EDGE_CT,
                )],
            ),
            registers(class),
            Vec::new(),
        );
        let source = source(plan);
        assert_eq!(
            validate_bypass_run_relocation(
                &source,
                0,
                RUN_A,
                RUN_B,
                HEAD,
                &environment,
                budget(),
                forged(&source)
            )
            .unwrap_err(),
            BypassRunRelocationError::UnsupportedPair
        );
    }
}
