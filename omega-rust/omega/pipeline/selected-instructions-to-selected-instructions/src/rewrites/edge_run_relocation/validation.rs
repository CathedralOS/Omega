//! Independent validation of cross-edge run relocation.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! relocation's legality from the source records — the contiguous run
//! the named members bound inside their block, the unconditional `Jump`
//! whose plain semantic edge is the destination block's only
//! predecessor, the destination's own gates, the landing index the
//! destination names, the schedulable bar and hazard audit every member
//! meets against every crossed position and the crossed edge's
//! instruction, the roster-ordering accounting, the transport-conflict
//! check, and the boundary-settlement exclusion — then rebuilds the
//! function the contract demands and requires the proposal to equal it.
//! Restoring the run to its source span must reproduce the complete
//! source by content. A producer admission error therefore fails
//! validation even when the proposal is exactly what that producer
//! emitted.
use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedBlockId, SelectedBlockOrigin, SelectedFunction, SelectedInstructionId,
    SelectedInstructionPlan, SelectedTerminator,
};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{EdgeRunRelocationError, EdgeRunRelocationReceipt, ValidatedEdgeRunRelocation};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{
    all_edges, edge_accounted, plain_edge, terminator_instruction, transport_conflict,
};
use crate::rewrites::window_hazards::{coupled, has_call_contract, schedulable, surface};

/// The validator's own reconstruction of the relocation the contract
/// permits: the coordinates the named members bound and the landing
/// index the destination names. It shares no state with the producer's
/// `admission` record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    function_index: usize,
    /// The run's own block: it ends in the crossed `Jump` edge.
    block_index: usize,
    block: SelectedBlockId,
    /// The run's first member index in the block body.
    first_index: usize,
    /// The run's last member index; the run is the contiguous span
    /// `first_index..=last_index` of at least two members.
    last_index: usize,
    /// The destination block's index in `function.blocks`.
    target_index: usize,
    target: SelectedBlockId,
    /// The destination instruction's position in the target body: the
    /// run lands at this index. Naming the target's terminator-carried
    /// instruction lands the run at the body end, index
    /// `target.instructions.len()`.
    landing_index: usize,
}

/// Reconstruct the legality of relocating the run `first_member` and
/// `last_member` bound onto `destination` from first principles: locate
/// the run, require its block to end in an unconditional `Jump` through
/// a plain semantic edge to a single-predecessor source block that is
/// neither the run's own block nor the entry, resolve the landing index
/// the destination names, then run the window audit the gates leave —
/// every member schedulable, every crossed position (the tail behind
/// the run, the head before the landing index) schedulable and neither
/// roster-accounted against an accounted run nor coupled with any
/// member, and the crossed edge's instruction uncalled, unaccounted
/// against an accounted run, uncoupled with every member, and free of
/// transport conflicts — with no boundary settlement past the run's
/// first index or past the landing index. Hazards between the run's own
/// members are not re-checked: the members keep their relative order.
/// Nothing in this audit reads the producer's admission decision.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    first_member: SelectedInstructionId,
    last_member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
) -> Result<Reconstructed<'source>, EdgeRunRelocationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(EdgeRunRelocationError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(EdgeRunRelocationError::SourceMismatch)?;
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
        .ok_or(EdgeRunRelocationError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    // The run is the contiguous span the two named members bound in this
    // block, in the named order; a repeated id or a last member that does
    // not follow the first names no multi-member run. A single member is
    // the one-instruction cross-edge relocation the sibling family
    // proves.
    let last_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == last_member)
        .filter(|position| *position > first_index)
        .ok_or(EdgeRunRelocationError::UnsupportedPair)?;
    // Only an unconditional semantic jump edge keeps the run's execution
    // count: every traversal of the run's block leaves through it, and —
    // with the single-predecessor rule below — every traversal of the
    // destination block arrives through it. A conditional terminator
    // keeps a second exit the run would still execute on after
    // relocating.
    let SelectedTerminator::Jump { successor, .. } = &block.terminator else {
        return Err(EdgeRunRelocationError::UnsupportedPair);
    };
    // The crossed edge must be a plain semantic successor: case dispatch,
    // continuation, structural transfer, and per-edge fuel all carry
    // boundary effects this step does not cross.
    if !plain_edge(successor) {
        return Err(EdgeRunRelocationError::UnsupportedPair);
    }
    let target_index = function
        .blocks
        .iter()
        .position(|candidate| candidate.id == successor.block)
        .ok_or(EdgeRunRelocationError::SourceMismatch)?;
    let target = &function.blocks[target_index];
    // A self-edge is the in-block run family's case with a back-edge
    // transport reading, not a cross-edge window. The entry block is
    // reached with no predecessor at all, and an implementation block's
    // origin carries edge or case work the bounded audit does not cross.
    if target.id == block.id
        || target.id == function.entry_block
        || !matches!(target.origin, SelectedBlockOrigin::Source(_))
    {
        return Err(EdgeRunRelocationError::UnsupportedPair);
    }
    // Every edge into the destination block must be this one: a second
    // predecessor gives the block a path the run would newly execute on.
    if all_edges(function)
        .filter(|(_, edge)| edge.block == target.id)
        .count()
        != 1
    {
        return Err(EdgeRunRelocationError::UnsupportedPair);
    }
    // The destination names a position in the target body — the run
    // lands at its index — or the target's terminator-carried
    // instruction, landing the run at the body end.
    let landing_index = target
        .instructions
        .iter()
        .position(|instruction| instruction.id == destination)
        .or_else(|| {
            (terminator_instruction(&target.terminator).id == destination)
                .then_some(target.instructions.len())
        })
        .ok_or(EdgeRunRelocationError::UnsupportedPair)?;
    // The window the run crosses is the one the gates leave: the tail
    // behind it in its own block, the head of the destination body
    // before the landing index, and the jump edge itself — its
    // terminator instruction, its transports, and the rows the roster
    // logs against it. Every crossed contract below is the family's own
    // composition of the shared primitives, not the producer's audit
    // result.
    let run = &block.instructions[first_index..=last_index];
    let mut any_member_accounted = false;
    for member in run {
        let Some(member_accounted) = schedulable(function, member) else {
            return Err(EdgeRunRelocationError::UnsupportedInstruction);
        };
        any_member_accounted |= member_accounted;
    }
    for crossed in block.instructions[last_index + 1..]
        .iter()
        .chain(target.instructions[..landing_index].iter())
    {
        let Some(crossed_accounted) = schedulable(function, crossed) else {
            return Err(EdgeRunRelocationError::UnsupportedInstruction);
        };
        if any_member_accounted && crossed_accounted {
            return Err(EdgeRunRelocationError::UnsupportedPair);
        }
        for member in run {
            if coupled(member, crossed) {
                return Err(EdgeRunRelocationError::UnsupportedPair);
            }
        }
    }
    let terminator = terminator_instruction(&block.terminator);
    if has_call_contract(function, terminator.id) {
        return Err(EdgeRunRelocationError::UnsupportedInstruction);
    }
    if any_member_accounted && edge_accounted(function, terminator, successor) {
        return Err(EdgeRunRelocationError::UnsupportedPair);
    }
    for member in run {
        if coupled(member, terminator) {
            return Err(EdgeRunRelocationError::UnsupportedPair);
        }
        if transport_conflict(member, successor) {
            return Err(EdgeRunRelocationError::UnsupportedPair);
        }
    }
    // The run vacates its block from its first index on, and the
    // destination's prefix past the landing index gains the run: a
    // boundary settlement on either side of those cuts observes a
    // changed executed prefix. No intermediate block exists — the
    // single-edge path is the only one the gates leave.
    for settlement in &function.boundary_settlements {
        let index = settlement.instruction_index as usize;
        if (settlement.block == block.id && index > first_index)
            || (settlement.block == target.id && index > landing_index)
        {
            return Err(EdgeRunRelocationError::UnsupportedPair);
        }
    }
    Ok(Reconstructed {
        function,
        function_index,
        block_index,
        block: block.id,
        first_index,
        last_index,
        target_index,
        target: target.id,
        landing_index,
    })
}

/// The validation work this audit performs, in the measured-step
/// contract the family publishes: one step per block plus one per
/// instruction across the plan, a second scan of the reconstructed
/// function's body and terminator instructions, the function's edge
/// roster for the path walk, every member's surface against each
/// crossed position's, and the function's three rosters plus the
/// crossed edge's binding roster per member.
fn measured_steps(
    plan: &SelectedInstructionPlan,
    reconstructed: &Reconstructed<'_>,
) -> Result<u64, EdgeRunRelocationError> {
    let function = reconstructed.function;
    let block = &function.blocks[reconstructed.block_index];
    let run = &block.instructions[reconstructed.first_index..=reconstructed.last_index];
    let target = &function.blocks[reconstructed.target_index];
    let SelectedTerminator::Jump { successor, .. } = &block.terminator else {
        return Err(EdgeRunRelocationError::SourceMismatch);
    };
    let terminator = terminator_instruction(&block.terminator);
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
        .and_then(|total| total.checked_add(all_edges(function).count()))
        .and_then(|total| {
            run.iter().try_fold(total, |total, member| {
                block.instructions[reconstructed.last_index + 1..]
                    .iter()
                    .chain(target.instructions[..reconstructed.landing_index].iter())
                    .chain(std::iter::once(terminator))
                    .try_fold(total, |total, crossed| {
                        total
                            .checked_add(surface(member))?
                            .checked_add(surface(crossed))
                    })
            })
        })
        .and_then(|total| {
            total
                .checked_add(function.memory_accesses.len())?
                .checked_add(function.calls.len())?
                .checked_add(function.boundary_settlements.len())?
                .checked_add(successor.bindings.len().saturating_mul(run.len()))
        })
        .ok_or(EdgeRunRelocationError::IdentityOverflow)?;
    u64::try_from(steps).map_err(|_| EdgeRunRelocationError::IdentityOverflow)
}

/// Build the function the contract demands from the validator's own
/// record: the run leaves its block's body as one span and lands at the
/// destination's index, its members keeping their order. The producer's
/// transformation is not consulted; both sides derive the same function
/// from the source alone.
fn expect(reconstructed: &Reconstructed<'_>) -> SelectedFunction {
    let mut expected = reconstructed.function.clone();
    let run: Vec<_> = expected.blocks[reconstructed.block_index]
        .instructions
        .drain(reconstructed.first_index..=reconstructed.last_index)
        .collect();
    expected.blocks[reconstructed.target_index]
        .instructions
        .splice(
            reconstructed.landing_index..reconstructed.landing_index,
            run,
        );
    expected
}

/// Move the run back: undoing the validator's expected edit must
/// restore the complete source by content — every crossed instruction,
/// every other block and instruction, register, roster row, call,
/// settlement, and edge included.
fn restore(
    reconstructed: &Reconstructed<'_>,
    proposed: &SelectedInstructionPlan,
) -> Result<SelectedInstructionPlan, EdgeRunRelocationError> {
    let mut restored = proposed.clone();
    let function = restored
        .functions
        .get_mut(reconstructed.function_index)
        .ok_or(EdgeRunRelocationError::ReplayMismatch)?;
    if function
        .blocks
        .get(reconstructed.block_index)
        .map(|block| block.id)
        != Some(reconstructed.block)
        || function
            .blocks
            .get(reconstructed.target_index)
            .map(|block| block.id)
            != Some(reconstructed.target)
    {
        return Err(EdgeRunRelocationError::ReplayMismatch);
    }
    let run_len = reconstructed.last_index - reconstructed.first_index + 1;
    let Some(run_end) = reconstructed.landing_index.checked_add(run_len) else {
        return Err(EdgeRunRelocationError::ReplayMismatch);
    };
    if function.blocks[reconstructed.target_index]
        .instructions
        .len()
        < run_end
    {
        return Err(EdgeRunRelocationError::ReplayMismatch);
    }
    let run: Vec<_> = function.blocks[reconstructed.target_index]
        .instructions
        .drain(reconstructed.landing_index..run_end)
        .collect();
    let block = &mut function.blocks[reconstructed.block_index];
    if block.instructions.len() < reconstructed.first_index {
        return Err(EdgeRunRelocationError::ReplayMismatch);
    }
    block
        .instructions
        .splice(reconstructed.first_index..reconstructed.first_index, run);
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
pub fn validate_edge_run_relocation(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    first_member: SelectedInstructionId,
    last_member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedEdgeRunRelocation, EdgeRunRelocationError> {
    let reconstructed = reconstruct(
        source,
        function_index,
        first_member,
        last_member,
        destination,
        environment,
    )?;
    if measured_steps(source.selected_plan(), &reconstructed)? > budget.validation_steps() {
        return Err(EdgeRunRelocationError::WorkBudgetExceeded);
    }
    if proposed.functions.get(function_index) != Some(&expect(&reconstructed)) {
        return Err(EdgeRunRelocationError::ReplayMismatch);
    }
    if restore(&reconstructed, &proposed)? != *source.selected_plan() {
        return Err(EdgeRunRelocationError::ReplayMismatch);
    }
    Ok(ValidatedEdgeRunRelocation {
        receipt: EdgeRunRelocationReceipt {
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
        EdgeRunRelocationError, EdgeRunRelocationReceipt, ValidatedEdgeRunRelocation,
        validate_edge_run_relocation,
    };

    const LEAD: SelectedInstructionId = SelectedInstructionId(2);
    const RUN_A: SelectedInstructionId = SelectedInstructionId(3);
    const RUN_B: SelectedInstructionId = SelectedInstructionId(4);
    const TRAIL: SelectedInstructionId = SelectedInstructionId(5);
    const HEAD: SelectedInstructionId = SelectedInstructionId(6);
    const MID: SelectedInstructionId = SelectedInstructionId(7);
    const TAIL: SelectedInstructionId = SelectedInstructionId(8);
    const JUMP: SelectedInstructionId = SelectedInstructionId(9);
    const RET: SelectedInstructionId = SelectedInstructionId(10);
    const JUMP_C: SelectedInstructionId = SelectedInstructionId(11);

    const POINTER: VirtualRegisterId = VirtualRegisterId(0);
    const R_LEAD: VirtualRegisterId = VirtualRegisterId(1);
    const R_MOVE_A: VirtualRegisterId = VirtualRegisterId(2);
    const R_TRAIL: VirtualRegisterId = VirtualRegisterId(3);
    const R_HEAD: VirtualRegisterId = VirtualRegisterId(4);
    const R_MID: VirtualRegisterId = VirtualRegisterId(5);
    const R_TAIL: VirtualRegisterId = VirtualRegisterId(6);
    const R_MOVE_B: VirtualRegisterId = VirtualRegisterId(7);

    const BLOCK_A: SelectedBlockId = SelectedBlockId(0);
    const BLOCK_B: SelectedBlockId = SelectedBlockId(1);
    const BLOCK_C: SelectedBlockId = SelectedBlockId(2);

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

    fn successor(block: SelectedBlockId, source_target: BlockId, edge: u64) -> SelectedSuccessor {
        SelectedSuccessor {
            role: SelectedSuccessorRole::Semantic,
            structural_case: None,
            structural_bindings: Vec::new(),
            psi_edge: EdgeId::new(edge).unwrap(),
            block,
            source_target,
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
            successor: successor(block, BlockId::new(source_target).unwrap(), edge),
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

    /// A raw selected-stage unit fixture, not a source/Terminal
    /// admission claim: `A = [LEAD; RUN_A; RUN_B; TRAIL] -> jump -> B =
    /// [HEAD; MID; TAIL] -> return`, moving the run `RUN_A..=RUN_B` onto
    /// `HEAD`'s position. Each test mutates one legality premise, so the
    /// source names a relocation the contract must refuse while a
    /// defective producer would still emit it.
    fn plan(
        environment: &ValidatedTargetRegisterEnvironment,
        target: NativeTarget,
        a_instructions: Vec<SelectedInstruction>,
        b_instructions: Vec<SelectedInstruction>,
        extra_blocks: Vec<SelectedBlock>,
        registers: Vec<VirtualRegister>,
        boundary_settlements: Vec<SelectedBoundarySettlement>,
    ) -> SelectedInstructionPlan {
        let mut blocks = vec![
            block(
                BLOCK_A,
                1,
                a_instructions,
                jump_terminator(environment, JUMP, BLOCK_B, 2, 10),
            ),
            block(
                BLOCK_B,
                2,
                b_instructions,
                return_terminator(environment, RET, 12),
            ),
        ];
        blocks.extend(extra_blocks);
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
                entry_block: BLOCK_A,
                virtual_registers: registers,
                blocks,
            }]
            .into(),
        }
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
            result_register(R_HEAD, class, HEAD, 6),
            result_register(R_MID, class, MID, 7),
            result_register(R_TAIL, class, TAIL, 8),
        ]
    }

    fn source(plan: SelectedInstructionPlan) -> ValidatedEdgeRunRelocation {
        let identity = selected_instruction_plan_identity(&plan);
        ValidatedEdgeRunRelocation {
            transformed: Arc::new(plan),
            receipt: EdgeRunRelocationReceipt {
                source_selected: identity,
                transformed_selected: identity,
                optimization_unit: OptimizationUnitIdentity::from_bytes([2; 32]),
                fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
            },
        }
    }

    /// The proposal a defective producer emits for this source: the run
    /// leaves block A's body as one span and lands at block B's head —
    /// the edit the contract demands only when the relocation's legality
    /// holds.
    fn forged(source: &ValidatedEdgeRunRelocation) -> SelectedInstructionPlan {
        let mut proposed = source.transformed().clone();
        let run: Vec<_> = proposed.functions[0].blocks[0]
            .instructions
            .drain(1..=2)
            .collect();
        proposed.functions[0].blocks[1]
            .instructions
            .splice(0..0, run);
        proposed
    }

    fn budget() -> OptimizationWorkBudget {
        OptimizationWorkBudget::new(100, 100, 100_000, 100, 100).unwrap()
    }

    /// A legal relocation validates on a forged proposal alone: the
    /// validator reconstructs the contract edit from the source and the
    /// proposal needs no producer receipt to pass. The run lands at B's
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
            &environment,
            target,
            vec![
                materialization(&environment, LEAD, R_LEAD, 5),
                materialization(&environment, RUN_A, R_MOVE_A, 7),
                materialization(&environment, RUN_B, R_MOVE_B, 9),
                materialization(&environment, TRAIL, R_TRAIL, 11),
            ],
            vec![
                materialization(&environment, HEAD, R_HEAD, 13),
                materialization(&environment, MID, R_MID, 15),
                materialization(&environment, TAIL, R_TAIL, 17),
            ],
            Vec::new(),
            registers(class),
            Vec::new(),
        );
        let source = source(plan);
        let proposed = forged(&source);
        let proposed_identity = selected_instruction_plan_identity(&proposed);
        let validated = validate_edge_run_relocation(
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
        assert_eq!(order(&moved.blocks[1]), vec![RUN_A, RUN_B, HEAD, MID, TAIL]);
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
    /// with `RUN_A`, which the run carries with it — while `TRAIL`
    /// copies `R_MOVE_B` into `R_TRAIL`, reading the register the run's
    /// second member defines. Moving the run past `TRAIL` hands the copy
    /// a different value. A producer that skipped the coupling audit
    /// still emits the move; the validator's own hazard walk refuses
    /// with `UnsupportedPair`, not merely a diff of the proposal.
    /// Feeding that forged proposal is the observable proof that
    /// validation no longer relies on the producer's admission routine.
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
            &environment,
            target,
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
            vec![
                materialization(&environment, HEAD, R_HEAD, 13),
                materialization(&environment, MID, R_MID, 15),
                materialization(&environment, TAIL, R_TAIL, 17),
            ],
            Vec::new(),
            registers(class),
            Vec::new(),
        );
        let source = source(plan);
        assert_eq!(
            validate_edge_run_relocation(
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
            EdgeRunRelocationError::UnsupportedPair
        );
    }

    /// `store8 [r0], r7` with no roster row — a memory-capable kind the
    /// roster does not account for can never trade order, as member or
    /// as crossed position. A producer that forgot the schedulable bar
    /// still emits the move; the validator's own audit refuses with
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
            &environment,
            target,
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
            vec![
                materialization(&environment, HEAD, R_HEAD, 13),
                materialization(&environment, MID, R_MID, 15),
                materialization(&environment, TAIL, R_TAIL, 17),
            ],
            Vec::new(),
            registers(class),
            Vec::new(),
        );
        let source = source(plan);
        assert_eq!(
            validate_edge_run_relocation(
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
            EdgeRunRelocationError::UnsupportedInstruction
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
            &environment,
            target,
            vec![
                materialization(&environment, LEAD, R_LEAD, 5),
                materialization(&environment, RUN_A, R_MOVE_A, 7),
                materialization(&environment, RUN_B, R_MOVE_B, 9),
                materialization(&environment, TRAIL, R_TRAIL, 11),
            ],
            vec![
                materialization(&environment, HEAD, R_HEAD, 13),
                materialization(&environment, MID, R_MID, 15),
                materialization(&environment, TAIL, R_TAIL, 17),
            ],
            Vec::new(),
            registers(class),
            vec![settlement(BLOCK_A, 2, 7)],
        );
        let source = source(plan);
        assert_eq!(
            validate_edge_run_relocation(
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
            EdgeRunRelocationError::UnsupportedPair
        );
    }

    /// A second edge into the destination block gives it a path the
    /// relocated run would newly execute on. A producer that forgot the
    /// predecessor count still emits the move; the validator's own edge
    /// scan refuses with `UnsupportedPair`.
    #[test]
    fn validator_refuses_a_second_predecessor_into_the_destination() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let class = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap()
            .operands[0]
            .class;
        let plan = plan(
            &environment,
            target,
            vec![
                materialization(&environment, LEAD, R_LEAD, 5),
                materialization(&environment, RUN_A, R_MOVE_A, 7),
                materialization(&environment, RUN_B, R_MOVE_B, 9),
                materialization(&environment, TRAIL, R_TRAIL, 11),
            ],
            vec![
                materialization(&environment, HEAD, R_HEAD, 13),
                materialization(&environment, MID, R_MID, 15),
                materialization(&environment, TAIL, R_TAIL, 17),
            ],
            vec![block(
                BLOCK_C,
                3,
                Vec::new(),
                jump_terminator(&environment, JUMP_C, BLOCK_B, 2, 11),
            )],
            registers(class),
            Vec::new(),
        );
        let source = source(plan);
        assert_eq!(
            validate_edge_run_relocation(
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
            EdgeRunRelocationError::UnsupportedPair
        );
    }
}
