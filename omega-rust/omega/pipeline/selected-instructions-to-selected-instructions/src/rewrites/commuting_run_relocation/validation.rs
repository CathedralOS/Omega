//! Independent validation of the in-block commuting run relocation.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! relocation's legality from the source records — the named run's span
//! inside one block, the destination's position outside the run, the
//! schedulable bar every window position meets, the register and
//! condition-state hazards between each member and every crossed
//! position, the commutation of every roster row that newly trades
//! order, the rowed-trading-pair accounting that keeps this family
//! disjoint from the plain run relocation, and the absence of a boundary
//! settlement inside the window's span — then requires the proposal to
//! place exactly the run's members on the destination's window edge in
//! their original order with the crossed positions shifted one run-width
//! toward the vacated span and the roster equal to the source's own rows
//! permuted into the new execution order, and restores the source by
//! content. Restoring the window's order and rows must reproduce the
//! complete selected program bit-for-bit. A producer admission error
//! therefore fails validation even when the proposal is exactly what
//! that producer emitted.
use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedFunction, SelectedInstructionId, SelectedInstructionPlan};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{
    CommutingRunRelocationError, CommutingRunRelocationReceipt, ValidatedCommutingRunRelocation,
};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::commuting_accesses as accesses;
use crate::rewrites::place_storage::structural_place_declarations;
use crate::rewrites::window_hazards::{coupled, interior_settlement, schedulable, surface};

/// The validator's own reconstruction of the relocation the contract
/// permits: the admitted run's and destination's coordinates inside
/// their block. It shares no state with the producer's `admission`
/// record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    block_index: usize,
    /// The run's first member index in the block body.
    first_index: usize,
    /// The run's last member index; the run is the contiguous span
    /// `first_index..=last_index` of at least two members.
    last_index: usize,
    /// The index whose instruction the run displaces: the window the
    /// relocation crosses is `first_index..=destination_index` in either
    /// order, with the destination always outside the run.
    destination_index: usize,
}

/// Reconstruct the legality of relocating the run `first_member` and
/// `last_member` bound to `destination` from first principles: locate
/// the run and the destination in one block in the named order, require
/// every window position schedulable, refuse any register or
/// condition-state coupling between a member and a crossed position and
/// any roster row that does not commute with a crossed position's row,
/// require at least one rowed trading pair — the accounting case that
/// keeps this family disjoint — and refuse a boundary settlement inside
/// the window's span. Nothing in this audit reads the producer's
/// admission decision.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    first_member: SelectedInstructionId,
    last_member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Reconstructed<'source>, CommutingRunRelocationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(CommutingRunRelocationError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(CommutingRunRelocationError::SourceMismatch)?;
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
        .ok_or(CommutingRunRelocationError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    // The run is the contiguous span the two named members bound in this
    // block, in the named order; a repeated id or a last member that does
    // not follow the first names no multi-member run. A single member is
    // the one-instruction relocation the sibling family already proves.
    let last_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == last_member)
        .filter(|position| *position > first_index)
        .ok_or(CommutingRunRelocationError::UnsupportedPair)?;
    // The destination sits outside the run in the same block; inside the
    // run it would name a member's own position, and outside the block it
    // names no in-block window.
    let destination_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == destination)
        .filter(|position| !(first_index..=last_index).contains(position))
        .ok_or(CommutingRunRelocationError::UnsupportedPair)?;
    let (first, last) = (
        first_index.min(destination_index),
        last_index.max(destination_index),
    );
    let window = &block.instructions[first..=last];
    let run_len = last_index - first_index + 1;
    let structural_places = structural_place_declarations(function);
    // Every window position meets the same schedulable bar the run
    // relocation enforces: no barrier kind, no call contract, and no
    // unaccounted memory reach. What changes here is only the accounting
    // rule — a roster-carrying member may trade order with a crossed
    // position whose own rows commute with it.
    for position in window {
        schedulable(function, position)
            .ok_or(CommutingRunRelocationError::UnsupportedInstruction)?;
    }
    let rows = accesses::window_rows(function, window);
    // Every member trades order with every crossed position, so each
    // direction of every register and condition-state hazard applies
    // against each pair, and every row the member carries must commute
    // with every row the crossed position carries. Members of the run keep
    // their relative order and never face this audit against each other,
    // and crossed positions keep their relative order with each other. A
    // window in which no trading pair is rowed on both sides is the run
    // relocation's own accounting case and stays with it.
    let mut rowed_trade = false;
    for (member_index, member) in window.iter().enumerate() {
        if !(first_index - first..first_index - first + run_len).contains(&member_index) {
            continue;
        }
        for (crossed_index, crossed) in window.iter().enumerate() {
            if (first_index - first..first_index - first + run_len).contains(&crossed_index) {
                continue;
            }
            if coupled(member, crossed) {
                return Err(CommutingRunRelocationError::UnsupportedPair);
            }
            rowed_trade |= !rows[member_index].is_empty() && !rows[crossed_index].is_empty();
            for member_row in &rows[member_index] {
                for crossed_row in &rows[crossed_index] {
                    if !accesses::commutes(member_row, crossed_row, structural_places) {
                        return Err(CommutingRunRelocationError::UnsupportedPair);
                    }
                }
            }
        }
    }
    if !rowed_trade {
        return Err(CommutingRunRelocationError::UnsupportedPair);
    }
    if interior_settlement(function, block.id, first + 1..=last) {
        return Err(CommutingRunRelocationError::UnsupportedPair);
    }
    // The audit walks the plan's body and terminator instructions once;
    // the window audit touches every member-against-crossed operand,
    // unit, and roster-row surface plus the function's three rosters.
    let run_range = first_index - first..first_index - first + run_len;
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .and_then(|total| {
            window
                .iter()
                .enumerate()
                .try_fold(total, |total, (member_index, member)| {
                    if !run_range.contains(&member_index) {
                        return Some(total);
                    }
                    window
                        .iter()
                        .enumerate()
                        .try_fold(total, |total, (crossed_index, crossed)| {
                            if run_range.contains(&crossed_index) {
                                return Some(total);
                            }
                            total
                                .checked_add(surface(member))?
                                .checked_add(surface(crossed))?
                                .checked_add(
                                    rows[member_index]
                                        .len()
                                        .checked_mul(rows[crossed_index].len())?,
                                )
                        })
                })
        })
        .and_then(|total| {
            total
                .checked_add(function.memory_accesses.len())?
                .checked_add(function.calls.len())?
                .checked_add(function.boundary_settlements.len())
        })
        .ok_or(CommutingRunRelocationError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| CommutingRunRelocationError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(CommutingRunRelocationError::WorkBudgetExceeded);
    }
    Ok(Reconstructed {
        function,
        block_index,
        first_index,
        last_index,
        destination_index,
    })
}

/// Independently consume the proposed program: the validator's own
/// record re-derives the admitted run and window from the source, the
/// touched block's window must equal exactly the source window's own
/// instructions rotated — the run on the destination's window edge, the
/// crossed positions one run-width toward the vacated span in their
/// original order — the roster must equal the source's rows permuted
/// into the new execution order, and restoring the window's order and
/// rows must recover the complete source by content: every instruction
/// before and after the window, every other instruction, register,
/// roster row, call, settlement, and function included. The producer's
/// `admission::admit` is never consulted, so a wrong legality decision
/// fails here even when the proposal matches the edit the producer
/// emitted.
pub fn validate_commuting_run_relocation(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    first_member: SelectedInstructionId,
    last_member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedCommutingRunRelocation, CommutingRunRelocationError> {
    let reconstructed = reconstruct(
        source,
        function_index,
        first_member,
        last_member,
        destination,
        environment,
        budget,
    )?;
    let source_block = &reconstructed.function.blocks[reconstructed.block_index];
    let proposed_function = proposed
        .functions
        .get(function_index)
        .ok_or(CommutingRunRelocationError::ReplayMismatch)?;
    let proposed_block = proposed_function
        .blocks
        .get(reconstructed.block_index)
        .ok_or(CommutingRunRelocationError::ReplayMismatch)?;
    let first = reconstructed
        .first_index
        .min(reconstructed.destination_index);
    let last = reconstructed
        .last_index
        .max(reconstructed.destination_index);
    let source_window = source_block
        .instructions
        .get(first..=last)
        .ok_or(CommutingRunRelocationError::ReplayMismatch)?;
    let run_len = reconstructed.last_index - reconstructed.first_index + 1;
    // The proposal's window must be the source window's own instructions
    // rotated — the leading group moving to the window's far edge: the
    // crossed prefix when the run moved up, the run itself when it moved
    // down — no other member, order, or content.
    let mut expected: Vec<_> = source_window.to_vec();
    if reconstructed.destination_index < reconstructed.first_index {
        expected.rotate_left(reconstructed.first_index - first);
    } else {
        expected.rotate_left(run_len);
    }
    if proposed_block
        .instructions
        .get(first..=last)
        .map(|window| window.iter().collect::<Vec<_>>())
        != Some(expected.iter().collect())
    {
        return Err(CommutingRunRelocationError::ReplayMismatch);
    }
    // The roster is the family's one content change beyond the rotation:
    // the source's window rows, permuted into the new execution order and
    // nothing else. Comparing against the permutation derived from the
    // source — never from the proposal's own grouping — keeps a stale,
    // reordered, or diluted roster from replaying.
    let window: Vec<SelectedInstructionId> = source_window
        .iter()
        .map(|instruction| instruction.id)
        .collect();
    let positions = accesses::window_row_positions(reconstructed.function, &window);
    let mut new_order = window.clone();
    if reconstructed.destination_index < reconstructed.first_index {
        new_order.rotate_left(reconstructed.first_index - first);
    } else {
        new_order.rotate_left(run_len);
    }
    let ordered = accesses::rows_in_order(reconstructed.function, &new_order);
    if positions.len() != ordered.len() {
        return Err(CommutingRunRelocationError::ReplayMismatch);
    }
    let mut expected_roster = reconstructed.function.memory_accesses.clone();
    for (position, access) in positions.iter().zip(ordered) {
        expected_roster[*position] = access;
    }
    if proposed_function.memory_accesses != expected_roster {
        return Err(CommutingRunRelocationError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    restored.functions[function_index].blocks[reconstructed.block_index]
        .instructions
        .splice(first..=last, source_window.iter().cloned());
    let restored_function = &mut restored.functions[function_index];
    let source_order = accesses::rows_in_order(reconstructed.function, &window);
    for (position, access) in positions.iter().zip(source_order) {
        restored_function.memory_accesses[*position] = access;
    }
    if restored != *source.selected_plan() {
        return Err(CommutingRunRelocationError::ReplayMismatch);
    }
    Ok(ValidatedCommutingRunRelocation {
        receipt: CommutingRunRelocationReceipt {
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
        SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan,
        SelectedMemoryAccess, SelectedMemoryAccessOrigin, SelectedMemoryAccessRole,
        SelectedOperand, SelectedTerminator, VirtualRegister, VirtualRegisterId,
        VirtualRegisterOrigin,
    };
    use semantic_vocabulary::{
        BlockId, BoundaryMachineId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType,
        IntegerValue, MachineId, OperationId, PlaceId, ScalarType, ValueId,
    };
    use target::NativeTarget;
    use target_operations_to_selected_instructions::selected_instruction_plan_identity;
    use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    use super::{
        CommutingRunRelocationError, CommutingRunRelocationReceipt,
        ValidatedCommutingRunRelocation, validate_commuting_run_relocation,
    };

    const STORE_A: SelectedInstructionId = SelectedInstructionId(2);
    const STORE_B: SelectedInstructionId = SelectedInstructionId(3);
    const STORE_C: SelectedInstructionId = SelectedInstructionId(4);
    const MAT_D: SelectedInstructionId = SelectedInstructionId(5);
    const LOAD_A: SelectedInstructionId = SelectedInstructionId(6);
    const LOAD_B: SelectedInstructionId = SelectedInstructionId(7);
    const LOAD_C: SelectedInstructionId = SelectedInstructionId(8);
    const MAT_A: SelectedInstructionId = SelectedInstructionId(9);
    const MAT_B: SelectedInstructionId = SelectedInstructionId(10);
    const TERMINAL: SelectedInstructionId = SelectedInstructionId(11);

    const POINTER: VirtualRegisterId = VirtualRegisterId(0);
    const FIRST: VirtualRegisterId = VirtualRegisterId(1);
    const SECOND: VirtualRegisterId = VirtualRegisterId(2);
    const THIRD: VirtualRegisterId = VirtualRegisterId(3);
    const FOURTH: VirtualRegisterId = VirtualRegisterId(4);

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

    fn u64_scalar() -> ScalarType {
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
    }

    fn entry_register(
        id: VirtualRegisterId,
        class: register_model::RegisterClassId,
        parameter_index: usize,
        source_value: u64,
    ) -> VirtualRegister {
        VirtualRegister {
            id,
            scalar_type: u64_scalar(),
            class,
            origin: VirtualRegisterOrigin::EntryParameter {
                source_value: ValueId::new(source_value).unwrap(),
                parameter_index,
            },
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
        VirtualRegister {
            id,
            scalar_type: u64_scalar(),
            class,
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction,
                source_value: ValueId::new(source_value).unwrap(),
            },
            definition_site: None,
            entry_fixed_view: None,
        }
    }

    fn access(
        instruction: SelectedInstructionId,
        place: u64,
        role: SelectedMemoryAccessRole,
        byte_offset: u32,
        byte_count: u32,
    ) -> SelectedMemoryAccess {
        SelectedMemoryAccess {
            instruction,
            origin: SelectedMemoryAccessOrigin::Operation(OperationId::new(31).unwrap()),
            place: PlaceId::new(place).unwrap(),
            byte_offset,
            byte_count,
            role,
        }
    }

    fn settlement(position: u32, operation: u64) -> SelectedBoundarySettlement {
        SelectedBoundarySettlement {
            block: SelectedBlockId(0),
            instruction_index: position,
            settlement: SelectedBoundarySettlementPayload::HostedWriteByteI32 {
                operation: OperationId::new(operation).unwrap(),
                boundary: BoundaryMachineId::new(1).unwrap(),
                source: ValueId::new(9).unwrap(),
            },
        }
    }

    /// A single-block raw selected-stage fixture, not a source/Terminal
    /// admission claim: the caller chooses the body, the roster rows, the
    /// settlements, and the registers; the block always ends in a unit
    /// return. Each test assembles a source whose named run the contract
    /// must refuse while a defective producer would still emit the
    /// relocation.
    fn plan(
        environment: &ValidatedTargetRegisterEnvironment,
        target: NativeTarget,
        instructions: Vec<SelectedInstruction>,
        memory_accesses: Vec<SelectedMemoryAccess>,
        boundary_settlements: Vec<SelectedBoundarySettlement>,
        registers: Vec<VirtualRegister>,
    ) -> SelectedInstructionPlan {
        let terminal_row = environment
            .constraint(environment.selected_keys().return_unit)
            .unwrap();
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
                memory_accesses,
                boundary_settlements,
                entry_block: SelectedBlockId(0),
                virtual_registers: registers,
                blocks: vec![SelectedBlock {
                    id: SelectedBlockId(0),
                    origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
                    instructions,
                    terminator: SelectedTerminator::Return {
                        instruction: instruction(
                            TERMINAL,
                            SelectedInstructionKind::ReturnUnit,
                            terminal_row,
                            &[],
                        ),
                        psi_return_edge: EdgeId::new(1).unwrap(),
                    },
                }],
            }]
            .into(),
        }
    }

    fn source(plan: SelectedInstructionPlan) -> ValidatedCommutingRunRelocation {
        let identity = selected_instruction_plan_identity(&plan);
        ValidatedCommutingRunRelocation {
            transformed: Arc::new(plan),
            receipt: CommutingRunRelocationReceipt {
                source_selected: identity,
                transformed_selected: identity,
                optimization_unit: OptimizationUnitIdentity::from_bytes([2; 32]),
                fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
            },
        }
    }

    fn budget() -> OptimizationWorkBudget {
        OptimizationWorkBudget::new(100, 100, 100_000, 100, 100).unwrap()
    }

    /// The proposal a defective producer emits when it admits the run the
    /// tests refuse: the window's own instructions rotated — the run on
    /// the destination's window edge, the crossed positions one run-width
    /// toward the vacated span — with the window's roster rows rewritten
    /// into the new execution order.
    fn forged(
        source: &ValidatedCommutingRunRelocation,
        member_count: usize,
        destination_index: usize,
    ) -> SelectedInstructionPlan {
        let mut proposed = source.transformed().clone();
        let window = &mut proposed.functions[0].blocks[0].instructions[0..=destination_index];
        window.rotate_left(member_count);
        proposed
    }

    /// `store8 [r0], r1; store8 [r0], r2; store8 [r0], r3` — three
    /// roster-accounted writes over the same recorded bytes. The run's
    /// members keep their own order, but each trades order with the
    /// crossed store, and none of those pairs commute: the relocation
    /// would leave a different last writer ahead of every later observer.
    /// A producer that admitted the run anyway would publish the rotated
    /// window with the roster following the new execution order; the
    /// validator's own commutation audit must refuse with
    /// `UnsupportedPair`, not merely diff the proposal. Feeding that
    /// forged proposal is the observable proof that validation no longer
    /// relies on the producer's admission routine.
    #[test]
    fn validator_refuses_a_run_whose_rows_do_not_commute() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        let class = store.operands[0].class;
        let materialize = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap();
        let plan = plan(
            &environment,
            target,
            vec![
                instruction(
                    STORE_A,
                    SelectedInstructionKind::Store {
                        byte_offset: 0,
                        byte_size: 8,
                    },
                    store,
                    &[POINTER, FIRST],
                ),
                instruction(
                    STORE_B,
                    SelectedInstructionKind::Store {
                        byte_offset: 0,
                        byte_size: 8,
                    },
                    store,
                    &[POINTER, SECOND],
                ),
                instruction(
                    STORE_C,
                    SelectedInstructionKind::Store {
                        byte_offset: 0,
                        byte_size: 8,
                    },
                    store,
                    &[POINTER, THIRD],
                ),
                instruction(
                    MAT_D,
                    SelectedInstructionKind::MaterializeI64 {
                        value: IntegerValue::Unsigned(7),
                    },
                    materialize,
                    &[FOURTH],
                ),
            ],
            vec![
                access(STORE_A, 1, SelectedMemoryAccessRole::WritePlace, 0, 8),
                access(STORE_B, 1, SelectedMemoryAccessRole::WritePlace, 0, 8),
                access(STORE_C, 1, SelectedMemoryAccessRole::WritePlace, 0, 8),
            ],
            Vec::new(),
            vec![
                entry_register(POINTER, class, 0, 1),
                entry_register(FIRST, class, 1, 2),
                entry_register(SECOND, class, 2, 3),
                entry_register(THIRD, class, 3, 4),
                result_register(FOURTH, class, MAT_D, 5),
            ],
        );
        let source = source(plan);
        let mut proposed = forged(&source, 2, 3);
        // The roster follows the new execution order: the crossed
        // store's row, then the run's own rows in their order.
        proposed.functions[0].memory_accesses.rotate_right(1);
        assert_eq!(
            validate_commuting_run_relocation(
                &source,
                0,
                STORE_A,
                STORE_B,
                MAT_D,
                &environment,
                budget(),
                proposed
            )
            .unwrap_err(),
            CommutingRunRelocationError::UnsupportedPair
        );
    }

    /// `r1 = load8 r0; r2 = load8 r0; store8 [r0], r1` — the crossed
    /// store reads the register the run's first member defines, so
    /// moving the run ahead of it hands the store a different value. A
    /// producer that skipped the coupling audit still emits the rotation;
    /// the validator's own hazard walk refuses with `UnsupportedPair`.
    #[test]
    fn validator_refuses_a_run_coupled_to_a_crossed_position() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.selected_keys();
        let materialize = environment.constraint(keys.materialize_i64).unwrap();
        let store = environment.constraint(keys.store.unwrap()).unwrap();
        let load = environment.constraint(keys.load8.unwrap()).unwrap();
        let class = materialize.operands[0].class;
        let plan = plan(
            &environment,
            target,
            vec![
                instruction(
                    LOAD_A,
                    SelectedInstructionKind::Load8 { byte_offset: 0 },
                    load,
                    &[POINTER, FIRST],
                ),
                instruction(
                    LOAD_B,
                    SelectedInstructionKind::Load8 { byte_offset: 8 },
                    load,
                    &[POINTER, SECOND],
                ),
                instruction(
                    STORE_C,
                    SelectedInstructionKind::Store {
                        byte_offset: 16,
                        byte_size: 8,
                    },
                    store,
                    &[POINTER, FIRST],
                ),
                instruction(
                    MAT_D,
                    SelectedInstructionKind::MaterializeI64 {
                        value: IntegerValue::Unsigned(7),
                    },
                    materialize,
                    &[FOURTH],
                ),
            ],
            vec![
                access(LOAD_A, 1, SelectedMemoryAccessRole::ReadPlace, 0, 8),
                access(LOAD_B, 2, SelectedMemoryAccessRole::ReadPlace, 8, 8),
                access(STORE_C, 3, SelectedMemoryAccessRole::WritePlace, 16, 8),
            ],
            Vec::new(),
            vec![
                entry_register(POINTER, class, 0, 1),
                result_register(FIRST, class, LOAD_A, 2),
                result_register(SECOND, class, LOAD_B, 3),
                result_register(FOURTH, class, MAT_D, 5),
            ],
        );
        let source = source(plan);
        let mut proposed = forged(&source, 2, 3);
        proposed.functions[0].memory_accesses.rotate_right(1);
        assert_eq!(
            validate_commuting_run_relocation(
                &source,
                0,
                LOAD_A,
                LOAD_B,
                MAT_D,
                &environment,
                budget(),
                proposed
            )
            .unwrap_err(),
            CommutingRunRelocationError::UnsupportedPair
        );
    }

    /// `r1 = materialize; r2 = materialize; r3 = load8 r0` — a row-less
    /// run against a single rowed crossed position: every trading pair
    /// carries at most one rowed side, the plain run relocation's own
    /// accounting case this family must refuse. A producer that dropped
    /// the rowed-trade check still emits the rotation; the validator's
    /// own accounting refuses with `UnsupportedPair`.
    #[test]
    fn validator_refuses_a_window_with_no_rowed_trade() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.selected_keys();
        let materialize = environment.constraint(keys.materialize_i64).unwrap();
        let load = environment.constraint(keys.load8.unwrap()).unwrap();
        let class = materialize.operands[0].class;
        let plan = plan(
            &environment,
            target,
            vec![
                instruction(
                    MAT_A,
                    SelectedInstructionKind::MaterializeI64 {
                        value: IntegerValue::Unsigned(5),
                    },
                    materialize,
                    &[FIRST],
                ),
                instruction(
                    MAT_B,
                    SelectedInstructionKind::MaterializeI64 {
                        value: IntegerValue::Unsigned(6),
                    },
                    materialize,
                    &[SECOND],
                ),
                instruction(
                    LOAD_C,
                    SelectedInstructionKind::Load8 { byte_offset: 0 },
                    load,
                    &[POINTER, THIRD],
                ),
                instruction(
                    MAT_D,
                    SelectedInstructionKind::MaterializeI64 {
                        value: IntegerValue::Unsigned(7),
                    },
                    materialize,
                    &[FOURTH],
                ),
            ],
            vec![access(LOAD_C, 1, SelectedMemoryAccessRole::ReadPlace, 0, 8)],
            Vec::new(),
            vec![
                entry_register(POINTER, class, 0, 1),
                result_register(FIRST, class, MAT_A, 2),
                result_register(SECOND, class, MAT_B, 3),
                result_register(THIRD, class, LOAD_C, 4),
                result_register(FOURTH, class, MAT_D, 5),
            ],
        );
        let source = source(plan);
        // The proposal a defective producer emits: the run rotated past
        // the window's far edge; the single roster row keeps its place
        // under either execution order.
        let proposed = forged(&source, 2, 3);
        assert_eq!(
            validate_commuting_run_relocation(
                &source,
                0,
                MAT_A,
                MAT_B,
                MAT_D,
                &environment,
                budget(),
                proposed
            )
            .unwrap_err(),
            CommutingRunRelocationError::UnsupportedPair
        );
    }

    /// `r1 = load8 r0; r2 = load8 r0; r3 = load8 r0` with a boundary
    /// settlement at the crossed position — the settlement observes the
    /// executed prefix, and the relocation moves the run ahead of it. A
    /// producer that forgot the settlement scan still emits the
    /// rotation; the validator's own window walk refuses with
    /// `UnsupportedPair`.
    #[test]
    fn validator_refuses_a_window_crossing_a_settlement() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.selected_keys();
        let materialize = environment.constraint(keys.materialize_i64).unwrap();
        let load = environment.constraint(keys.load8.unwrap()).unwrap();
        let class = materialize.operands[0].class;
        let plan = plan(
            &environment,
            target,
            vec![
                instruction(
                    LOAD_A,
                    SelectedInstructionKind::Load8 { byte_offset: 0 },
                    load,
                    &[POINTER, FIRST],
                ),
                instruction(
                    LOAD_B,
                    SelectedInstructionKind::Load8 { byte_offset: 8 },
                    load,
                    &[POINTER, SECOND],
                ),
                instruction(
                    LOAD_C,
                    SelectedInstructionKind::Load8 { byte_offset: 16 },
                    load,
                    &[POINTER, THIRD],
                ),
                instruction(
                    MAT_D,
                    SelectedInstructionKind::MaterializeI64 {
                        value: IntegerValue::Unsigned(7),
                    },
                    materialize,
                    &[FOURTH],
                ),
            ],
            vec![
                access(LOAD_A, 1, SelectedMemoryAccessRole::ReadPlace, 0, 8),
                access(LOAD_B, 2, SelectedMemoryAccessRole::ReadPlace, 8, 8),
                access(LOAD_C, 3, SelectedMemoryAccessRole::ReadPlace, 16, 8),
            ],
            vec![settlement(2, 7)],
            vec![
                entry_register(POINTER, class, 0, 1),
                result_register(FIRST, class, LOAD_A, 2),
                result_register(SECOND, class, LOAD_B, 3),
                result_register(THIRD, class, LOAD_C, 4),
                result_register(FOURTH, class, MAT_D, 5),
            ],
        );
        let source = source(plan);
        let mut proposed = forged(&source, 2, 3);
        proposed.functions[0].memory_accesses.rotate_right(1);
        assert_eq!(
            validate_commuting_run_relocation(
                &source,
                0,
                LOAD_A,
                LOAD_B,
                MAT_D,
                &environment,
                budget(),
                proposed
            )
            .unwrap_err(),
            CommutingRunRelocationError::UnsupportedPair
        );
    }
}
