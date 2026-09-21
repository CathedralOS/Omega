//! Independent validation of the in-block commuting pair interchange.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! interchange's legality from the source records — the named pair's
//! order inside one block, the schedulable bar every window position
//! meets, the register and condition-state hazards between each member
//! and every crossed position, the commutation of every roster row that
//! newly trades order, the rowed-trading-pair accounting that keeps this
//! family disjoint from the plain pair interchange, and the absence of a
//! boundary settlement inside the window's span — then rebuilds the
//! function the contract demands and requires the proposal to equal it.
//! Restoring the pair's order and the window's source rows must
//! reproduce the complete source by content. A producer admission error
//! therefore fails validation even when the proposal is exactly what
//! that producer emitted.
use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedBlockId, SelectedFunction, SelectedInstructionId, SelectedInstructionPlan,
};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{
    CommutingInterchangeError, CommutingInterchangeReceipt, ValidatedCommutingInterchange,
};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::commuting_accesses as accesses;
use crate::rewrites::place_storage::structural_place_declarations;
use crate::rewrites::window_hazards::{coupled, interior_settlement, schedulable, surface};

/// The validator's own reconstruction of the interchange the contract
/// permits: the admitted pair's coordinates inside its block. It shares
/// no state with the producer's `admission` record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    function_index: usize,
    block_index: usize,
    block: SelectedBlockId,
    /// The earlier member's index in the block body.
    earlier_index: usize,
    /// The later member's index: the window the interchange crosses is
    /// `earlier_index..=later_index`, a single position when the pair is
    /// adjacent.
    later_index: usize,
}

/// Reconstruct the legality of interchanging `earlier` and `later` from
/// first principles: locate the pair in one block in the named order,
/// require every window position schedulable, refuse any register or
/// condition-state coupling between a member and a crossed position and
/// any roster row that does not commute with a crossed position's row,
/// require at least one rowed trading pair — the accounting case that
/// keeps this family disjoint — and refuse a boundary settlement inside
/// the window's span. Nothing in this audit reads the producer's
/// admission decision.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    earlier: SelectedInstructionId,
    later: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
) -> Result<Reconstructed<'source>, CommutingInterchangeError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(CommutingInterchangeError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(CommutingInterchangeError::SourceMismatch)?;
    let (block_index, earlier_index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .position(|instruction| instruction.id == earlier)
                .map(|earlier_index| (block_index, earlier_index))
        })
        .ok_or(CommutingInterchangeError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    // The pair is the two named instructions in the named order inside
    // this block; every instruction between them belongs to the window
    // the interchange crosses.
    let later_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == later)
        .filter(|position| *position > earlier_index)
        .ok_or(CommutingInterchangeError::UnsupportedPair)?;
    let window = &block.instructions[earlier_index..=later_index];
    let structural_places = structural_place_declarations(function);
    // Every window position meets the schedulable bar the interchange
    // enforces: no barrier kind, no call contract, and no unaccounted
    // memory reach.
    for position in window {
        schedulable(function, position).ok_or(CommutingInterchangeError::UnsupportedInstruction)?;
    }
    let rows = accesses::window_rows(function, window);
    // Every member trades order with every crossed position — the other
    // member and the whole interior — so each direction of every
    // register and condition-state hazard applies against each pair,
    // and every row the member carries must commute with every row the
    // crossed position carries. Interior positions keep their relative
    // order with each other and are never audited against one another.
    // A window in which no trading pair is rowed on both sides is the
    // pair interchange's own accounting case and stays with it.
    let mut rowed_trade = false;
    for (member_index, member) in [0usize, window.len() - 1]
        .into_iter()
        .map(|index| (index, &window[index]))
    {
        for (crossed_index, crossed) in window
            .iter()
            .enumerate()
            .filter(|(crossed_index, _)| *crossed_index != member_index)
        {
            if coupled(member, crossed) {
                return Err(CommutingInterchangeError::UnsupportedPair);
            }
            rowed_trade |= !rows[member_index].is_empty() && !rows[crossed_index].is_empty();
            for member_row in &rows[member_index] {
                for crossed_row in &rows[crossed_index] {
                    if !accesses::commutes(member_row, crossed_row, structural_places) {
                        return Err(CommutingInterchangeError::UnsupportedPair);
                    }
                }
            }
        }
    }
    if !rowed_trade {
        return Err(CommutingInterchangeError::UnsupportedPair);
    }
    if interior_settlement(function, block.id, earlier_index + 1..=later_index) {
        return Err(CommutingInterchangeError::UnsupportedPair);
    }
    Ok(Reconstructed {
        function,
        function_index,
        block_index,
        block: block.id,
        earlier_index,
        later_index,
    })
}

/// The validation work this audit performs, in the measured-step
/// contract the family publishes: one step per block plus one per
/// instruction across the plan, the window audit at every
/// member-against-crossed operand, unit, and roster-row surface, and a
/// scan of the function's roster, call, and settlement rows.
fn measured_steps(
    plan: &SelectedInstructionPlan,
    reconstructed: &Reconstructed<'_>,
) -> Result<u64, CommutingInterchangeError> {
    let function = reconstructed.function;
    let window = &function.blocks[reconstructed.block_index].instructions
        [reconstructed.earlier_index..=reconstructed.later_index];
    let rows = accesses::window_rows(function, window);
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .and_then(|total| {
            [0usize, window.len() - 1]
                .into_iter()
                .try_fold(total, |total, member_index| {
                    window
                        .iter()
                        .enumerate()
                        .try_fold(total, |total, (crossed_index, crossed)| {
                            if crossed_index == member_index {
                                return Some(total);
                            }
                            total
                                .checked_add(surface(&window[member_index]))?
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
        .ok_or(CommutingInterchangeError::IdentityOverflow)?;
    u64::try_from(steps).map_err(|_| CommutingInterchangeError::IdentityOverflow)
}

/// Build the function the contract demands from the validator's own
/// record: the named pair trades positions inside its block and the
/// window's roster rows follow the new execution order — the later
/// member's rows, the interior's rows, then the earlier member's rows,
/// each instruction's own rows keeping their relative order. The
/// producer's transformation is not consulted; both sides derive the
/// same function from the source alone.
fn expect(reconstructed: &Reconstructed<'_>) -> SelectedFunction {
    let source_block = &reconstructed.function.blocks[reconstructed.block_index];
    let window: Vec<SelectedInstructionId> = source_block.instructions
        [reconstructed.earlier_index..=reconstructed.later_index]
        .iter()
        .map(|instruction| instruction.id)
        .collect();
    let mut expected = reconstructed.function.clone();
    expected.blocks[reconstructed.block_index]
        .instructions
        .swap(reconstructed.earlier_index, reconstructed.later_index);
    // The window's roster positions stay fixed while the rows that fill
    // them follow the new execution order. Deriving the permutation from
    // the source — never from the proposal's own grouping — keeps a
    // stale, reordered, or diluted roster from replaying.
    let positions = accesses::window_row_positions(reconstructed.function, &window);
    let new_order: Vec<SelectedInstructionId> = std::iter::once(window[window.len() - 1])
        .chain(window[1..window.len() - 1].iter().copied())
        .chain(std::iter::once(window[0]))
        .collect();
    let ordered = accesses::rows_in_order(reconstructed.function, &new_order);
    debug_assert_eq!(positions.len(), ordered.len());
    for (position, access) in positions.iter().zip(ordered) {
        expected.memory_accesses[*position] = access;
    }
    expected
}

/// Swap the pair back and write the source order's rows back into the
/// window's roster positions: undoing the validator's expected edit must
/// restore the complete source by content — every instruction between
/// them, every other instruction, register, roster row, call,
/// settlement, and function included.
fn restore(
    reconstructed: &Reconstructed<'_>,
    proposed: &SelectedInstructionPlan,
) -> Result<SelectedInstructionPlan, CommutingInterchangeError> {
    let window: Vec<SelectedInstructionId> = reconstructed.function.blocks
        [reconstructed.block_index]
        .instructions[reconstructed.earlier_index..=reconstructed.later_index]
        .iter()
        .map(|instruction| instruction.id)
        .collect();
    let positions = accesses::window_row_positions(reconstructed.function, &window);
    let mut restored = proposed.clone();
    let function = restored
        .functions
        .get_mut(reconstructed.function_index)
        .ok_or(CommutingInterchangeError::ReplayMismatch)?;
    if function.memory_accesses.len() != reconstructed.function.memory_accesses.len() {
        return Err(CommutingInterchangeError::ReplayMismatch);
    }
    let block = function
        .blocks
        .get_mut(reconstructed.block_index)
        .ok_or(CommutingInterchangeError::ReplayMismatch)?;
    if block.id != reconstructed.block || block.instructions.len() <= reconstructed.later_index {
        return Err(CommutingInterchangeError::ReplayMismatch);
    }
    block
        .instructions
        .swap(reconstructed.earlier_index, reconstructed.later_index);
    let source_order = accesses::rows_in_order(reconstructed.function, &window);
    for (position, access) in positions.iter().zip(source_order) {
        function.memory_accesses[*position] = access;
    }
    Ok(restored)
}

/// Independently consume the proposed program: the validator reconstructs
/// the interchange's preconditions from the source, requires the proposal
/// to equal the function its own record produces, and restores the
/// complete source by content — every instruction between the pair,
/// every other instruction, register, roster row, call, settlement, and
/// function included. The producer's `admission::admit` is never
/// consulted, so a wrong legality decision fails here even when the
/// proposal matches the edit the producer emitted.
pub fn validate_commuting_interchange(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    earlier: SelectedInstructionId,
    later: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedCommutingInterchange, CommutingInterchangeError> {
    let reconstructed = reconstruct(source, function_index, earlier, later, environment)?;
    if measured_steps(source.selected_plan(), &reconstructed)? > budget.validation_steps() {
        return Err(CommutingInterchangeError::WorkBudgetExceeded);
    }
    if proposed.functions.get(function_index) != Some(&expect(&reconstructed)) {
        return Err(CommutingInterchangeError::ReplayMismatch);
    }
    if restore(&reconstructed, &proposed)? != *source.selected_plan() {
        return Err(CommutingInterchangeError::ReplayMismatch);
    }
    Ok(ValidatedCommutingInterchange {
        receipt: CommutingInterchangeReceipt {
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
        CommutingInterchangeError, CommutingInterchangeReceipt, ValidatedCommutingInterchange,
        validate_commuting_interchange,
    };

    const STORE_A: SelectedInstructionId = SelectedInstructionId(2);
    const STORE_B: SelectedInstructionId = SelectedInstructionId(3);
    const MAT_B: SelectedInstructionId = SelectedInstructionId(4);
    const LOAD_A: SelectedInstructionId = SelectedInstructionId(5);
    const LOAD_C: SelectedInstructionId = SelectedInstructionId(6);
    const TERMINAL: SelectedInstructionId = SelectedInstructionId(7);

    const POINTER: VirtualRegisterId = VirtualRegisterId(0);
    const FIRST: VirtualRegisterId = VirtualRegisterId(1);
    const SECOND: VirtualRegisterId = VirtualRegisterId(2);
    const THIRD: VirtualRegisterId = VirtualRegisterId(3);

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
    /// return. Each test assembles a source whose named pair the contract
    /// must refuse while a defective producer would still emit the
    /// interchange.
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

    fn source(plan: SelectedInstructionPlan) -> ValidatedCommutingInterchange {
        let identity = selected_instruction_plan_identity(&plan);
        ValidatedCommutingInterchange {
            transformed: Arc::new(plan),
            receipt: CommutingInterchangeReceipt {
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

    /// `store8 [r0], r1; store8 [r0], r2` — two roster-accounted writes
    /// to the same place over the same recorded bytes. Their accesses do
    /// not commute: the interchange would leave the earlier store's value
    /// where the later one's belongs. A producer that admitted the pair
    /// anyway would publish a plan whose stores trade places with the
    /// roster following the new execution order; the validator's own
    /// commutation audit must refuse with `UnsupportedPair`, not merely
    /// diff the proposal. Feeding that forged proposal is the observable
    /// proof that validation no longer relies on the producer's
    /// admission routine.
    #[test]
    fn validator_refuses_a_pair_whose_rows_do_not_commute() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        let class = store.operands[0].class;
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
            ],
            vec![
                access(STORE_A, 1, SelectedMemoryAccessRole::WritePlace, 0, 8),
                access(STORE_B, 1, SelectedMemoryAccessRole::WritePlace, 0, 8),
            ],
            Vec::new(),
            vec![
                entry_register(POINTER, class, 0, 1),
                entry_register(FIRST, class, 1, 2),
                entry_register(SECOND, class, 2, 3),
            ],
        );
        let source = source(plan);
        // The proposal a defective producer emits: the pair swapped and
        // the roster rewritten into the new execution order — the edit
        // the contract demands only when every traded row commutes.
        let mut proposed = source.transformed().clone();
        proposed.functions[0].blocks[0].instructions.swap(0, 1);
        proposed.functions[0].memory_accesses.swap(0, 1);
        assert_eq!(
            validate_commuting_interchange(
                &source,
                0,
                STORE_A,
                STORE_B,
                &environment,
                budget(),
                proposed
            )
            .unwrap_err(),
            CommutingInterchangeError::UnsupportedPair
        );
    }

    /// `r1 = materialize; r2 = load8 r1` — the later member reads the
    /// register the earlier one defines, so trading their order hands
    /// the load a different pointer. A producer that skipped the
    /// coupling audit still emits the swap; the validator's own hazard
    /// walk refuses with `UnsupportedPair`.
    #[test]
    fn validator_refuses_a_coupled_pair() {
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
                    MAT_B,
                    SelectedInstructionKind::MaterializeI64 {
                        value: IntegerValue::Unsigned(5),
                    },
                    materialize,
                    &[FIRST],
                ),
                instruction(
                    LOAD_C,
                    SelectedInstructionKind::Load8 { byte_offset: 0 },
                    load,
                    &[FIRST, SECOND],
                ),
            ],
            Vec::new(),
            Vec::new(),
            vec![
                result_register(FIRST, class, MAT_B, 2),
                result_register(SECOND, class, LOAD_C, 3),
            ],
        );
        let source = source(plan);
        // The proposal a defective producer emits: the pair swapped even
        // though the consumer now runs ahead of its producer.
        let mut proposed = source.transformed().clone();
        proposed.functions[0].blocks[0].instructions.swap(0, 1);
        assert_eq!(
            validate_commuting_interchange(
                &source,
                0,
                MAT_B,
                LOAD_C,
                &environment,
                budget(),
                proposed
            )
            .unwrap_err(),
            CommutingInterchangeError::UnsupportedPair
        );
    }

    /// `r1 = load8 r0; r2 = materialize` — a rostered load against an
    /// unrostered materialization: every trading pair carries at most
    /// one rowed side, the plain pair interchange's own accounting case
    /// this family must refuse. A producer that dropped the rowed-trade
    /// check still emits the swap; the validator's own accounting
    /// refuses with `UnsupportedPair`.
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
                    LOAD_A,
                    SelectedInstructionKind::Load8 { byte_offset: 0 },
                    load,
                    &[POINTER, FIRST],
                ),
                instruction(
                    MAT_B,
                    SelectedInstructionKind::MaterializeI64 {
                        value: IntegerValue::Unsigned(5),
                    },
                    materialize,
                    &[SECOND],
                ),
            ],
            vec![access(LOAD_A, 1, SelectedMemoryAccessRole::ReadPlace, 0, 8)],
            Vec::new(),
            vec![
                entry_register(POINTER, class, 0, 1),
                result_register(FIRST, class, LOAD_A, 2),
                result_register(SECOND, class, MAT_B, 3),
            ],
        );
        let source = source(plan);
        // The proposal a defective producer emits: the pair swapped; the
        // single roster row keeps its place under either execution
        // order.
        let mut proposed = source.transformed().clone();
        proposed.functions[0].blocks[0].instructions.swap(0, 1);
        assert_eq!(
            validate_commuting_interchange(
                &source,
                0,
                LOAD_A,
                MAT_B,
                &environment,
                budget(),
                proposed
            )
            .unwrap_err(),
            CommutingInterchangeError::UnsupportedPair
        );
    }

    /// `r1 = load8 r0; r2 = materialize; r3 = load8 r0` with a boundary
    /// settlement at the interior position — the settlement observes the
    /// executed prefix, and the interchange moves the later load ahead
    /// of it. A producer that forgot the settlement scan still emits the
    /// swap; the validator's own window walk refuses with
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
                    MAT_B,
                    SelectedInstructionKind::MaterializeI64 {
                        value: IntegerValue::Unsigned(5),
                    },
                    materialize,
                    &[SECOND],
                ),
                instruction(
                    LOAD_C,
                    SelectedInstructionKind::Load8 { byte_offset: 8 },
                    load,
                    &[POINTER, THIRD],
                ),
            ],
            vec![
                access(LOAD_A, 1, SelectedMemoryAccessRole::ReadPlace, 0, 8),
                access(LOAD_C, 2, SelectedMemoryAccessRole::ReadPlace, 8, 8),
            ],
            vec![settlement(1, 7)],
            vec![
                entry_register(POINTER, class, 0, 1),
                result_register(FIRST, class, LOAD_A, 2),
                result_register(SECOND, class, MAT_B, 3),
                result_register(THIRD, class, LOAD_C, 4),
            ],
        );
        let source = source(plan);
        // The proposal a defective producer emits: the pair swapped and
        // the roster rewritten into the new execution order — the later
        // load's row first, then the interior's, then the earlier
        // load's.
        let mut proposed = source.transformed().clone();
        proposed.functions[0].blocks[0].instructions.swap(0, 2);
        proposed.functions[0].memory_accesses.swap(0, 1);
        assert_eq!(
            validate_commuting_interchange(
                &source,
                0,
                LOAD_A,
                LOAD_C,
                &environment,
                budget(),
                proposed
            )
            .unwrap_err(),
            CommutingInterchangeError::UnsupportedPair
        );
    }
}
