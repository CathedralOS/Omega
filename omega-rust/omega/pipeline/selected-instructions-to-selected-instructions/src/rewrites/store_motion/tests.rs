//! Fixtures shared by the store motion tests: budgets, instructions, places,
//! settlements and the block fixtures.

mod cross_block_motion;
mod same_block_motion;

use crate::rewrites::store_motion::{
    StoreMutationMotionError, StoreMutationMotionReceipt, ValidatedStoreMutationMotion,
    sink_selected_store_mutation,
};
use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget};
use optimization_unit::ValueDefinitionSite;
use register_environment::baseline_target_register_environment;
use register_model::RegisterInstructionConstraint;
use selected_instructions::{
    PackedByteWidth, SelectedBlock, SelectedBlockId, SelectedBlockOrigin,
    SelectedBoundarySettlement, SelectedBoundarySettlementPayload, SelectedFunction,
    SelectedInstruction, SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan,
    SelectedMemoryAccess, SelectedMemoryAccessOrigin, SelectedMemoryAccessRole, SelectedOperand,
    SelectedSuccessor, SelectedSuccessorRole, SelectedTerminator, VirtualRegister,
    VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType,
    IntegerValue, MachineId, OperationId, PlaceId, ScalarType, ValueId,
};
use target::NativeTarget;
use target_operations_to_selected_instructions::selected_instruction_plan_identity;
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

fn budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(100, 100, 1000, 100, 100).unwrap()
}

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

const STORE: SelectedInstructionId = SelectedInstructionId(2);

const BETWEEN: SelectedInstructionId = SelectedInstructionId(3);

const KILLER: SelectedInstructionId = SelectedInstructionId(4);

const POINTER: VirtualRegisterId = VirtualRegisterId(0);

const VALUE: VirtualRegisterId = VirtualRegisterId(1);

const SCRATCH: VirtualRegisterId = VirtualRegisterId(2);

/// The packed store's early-clobber scratch register.
const PACKED_SCRATCH: VirtualRegisterId = VirtualRegisterId(9);

fn place() -> PlaceId {
    PlaceId::new(1).unwrap()
}

fn access(
    instruction: SelectedInstructionId,
    operation: u64,
    place: PlaceId,
    byte_offset: u32,
    role: SelectedMemoryAccessRole,
) -> SelectedMemoryAccess {
    SelectedMemoryAccess {
        instruction,
        origin: SelectedMemoryAccessOrigin::Operation(OperationId::new(operation).unwrap()),
        place,
        byte_offset,
        byte_count: 8,
        role,
    }
}

fn settlement(position: u32) -> SelectedBoundarySettlement {
    settlement_at(SelectedBlockId(0), position)
}

fn settlement_at(block: SelectedBlockId, position: u32) -> SelectedBoundarySettlement {
    SelectedBoundarySettlement {
        block,
        instruction_index: position,
        settlement: SelectedBoundarySettlementPayload::HostedWriteByteI32 {
            operation: OperationId::new(7).unwrap(),
            boundary: BoundaryMachineId::new(1).unwrap(),
            source: ValueId::new(9).unwrap(),
        },
    }
}

/// A raw selected-stage unit fixture, not a source/Terminal admission claim:
/// `r1 = copy r0; store [r0+0] <- r1; r2 = copy r0; store [r0+0] <- r2;
/// return`. The first store slides forward to just before the second, which
/// is the first access on its place.
fn fixture(target: NativeTarget) -> ValidatedStoreMutationMotion {
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let copy = environment.constraint(keys.copy_i64).unwrap();
    let store = environment.constraint(keys.store.unwrap()).unwrap();
    let terminal_row = environment.constraint(keys.return_unit).unwrap();
    let class = copy.operands[0].class;
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let source_value = ValueId::new(1).unwrap();
    let registers = vec![
        VirtualRegister {
            id: POINTER,
            scalar_type,
            class,
            origin: VirtualRegisterOrigin::EntryParameter {
                source_value,
                parameter_index: 0,
            },
            definition_site: Some(ValueDefinitionSite::FunctionParameter(0)),
            entry_fixed_view: None,
        },
        VirtualRegister {
            id: VALUE,
            scalar_type,
            class,
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(1),
                source_value: ValueId::new(2).unwrap(),
            },
            definition_site: None,
            entry_fixed_view: None,
        },
        VirtualRegister {
            id: SCRATCH,
            scalar_type,
            class,
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction: BETWEEN,
                source_value: ValueId::new(3).unwrap(),
            },
            definition_site: None,
            entry_fixed_view: None,
        },
    ];
    let instructions = vec![
        instruction(
            SelectedInstructionId(1),
            SelectedInstructionKind::CopyI64,
            copy,
            &[POINTER, VALUE],
        ),
        instruction(
            STORE,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            store,
            &[POINTER, VALUE],
        ),
        instruction(
            BETWEEN,
            SelectedInstructionKind::CopyI64,
            copy,
            &[POINTER, SCRATCH],
        ),
        {
            let mut killer = instruction(
                KILLER,
                SelectedInstructionKind::Store {
                    byte_offset: 0,
                    byte_size: 8,
                },
                store,
                &[POINTER, SCRATCH],
            );
            killer.provenance.operations = vec![OperationId::new(2).unwrap()];
            killer.provenance.values = vec![ValueId::new(4).unwrap()];
            killer
        },
    ];
    let machine = MachineId::new(1).unwrap();
    let place = place();
    let plan = SelectedInstructionPlan {
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
            memory_accesses: vec![
                access(STORE, 1, place, 0, SelectedMemoryAccessRole::WritePlace),
                access(KILLER, 2, place, 0, SelectedMemoryAccessRole::WritePlace),
            ],
            boundary_settlements: Vec::new(),
            entry_block: SelectedBlockId(0),
            virtual_registers: registers,
            blocks: vec![SelectedBlock {
                id: SelectedBlockId(0),
                origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
                instructions,
                terminator: SelectedTerminator::Return {
                    instruction: instruction(
                        SelectedInstructionId(5),
                        SelectedInstructionKind::ReturnUnit,
                        terminal_row,
                        &[],
                    ),
                    psi_return_edge: EdgeId::new(1).unwrap(),
                },
            }],
        }]
        .into(),
    };
    let identity = selected_instruction_plan_identity(&plan);
    ValidatedStoreMutationMotion {
        receipt: StoreMutationMotionReceipt {
            source_selected: identity,
            transformed_selected: identity,
            optimization_unit: OptimizationUnitIdentity::from_bytes([2; 32]),
            fuel_schedule: plan.fuel_schedule,
        },
        transformed: std::sync::Arc::new(plan),
    }
}

/// Edit the single fixture function, then refresh the receipt identities so the
/// mutated plan is a well-formed analysis source.
fn mutated(
    target: NativeTarget,
    edit: impl FnOnce(&mut SelectedFunction, &register_environment::ValidatedTargetRegisterEnvironment),
) -> ValidatedStoreMutationMotion {
    let environment = baseline_target_register_environment(target).unwrap();
    let mut source = fixture(target);
    edit(
        &mut std::sync::Arc::make_mut(&mut source.transformed).functions[0],
        &environment,
    );
    let identity = selected_instruction_plan_identity(&source.transformed);
    source.receipt.source_selected = identity;
    source.receipt.transformed_selected = identity;
    source
}

/// Rewrite the fixture's moved store into a seven-byte `StorePacked` writing
/// the same range: `[use pointer, use packed value, def PACKED_SCRATCH]`
/// with the scratch defined early-clobbered by the target's packed-store
/// row, and the write row's byte count following the encoded width.
fn pack_store(
    function: &mut SelectedFunction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) {
    let packed = environment
        .constraint(environment.selected_keys().store_packed.unwrap())
        .unwrap();
    let (scalar_type, class) = {
        let register = function
            .virtual_registers
            .iter()
            .find(|register| register.id == VALUE)
            .unwrap();
        (register.scalar_type, register.class)
    };
    function.virtual_registers.push(VirtualRegister {
        id: PACKED_SCRATCH,
        scalar_type,
        class,
        origin: VirtualRegisterOrigin::InstructionResult {
            instruction: STORE,
            source_value: ValueId::new(8).unwrap(),
        },
        definition_site: None,
        entry_fixed_view: None,
    });
    function.blocks[0].instructions[1] = instruction(
        STORE,
        SelectedInstructionKind::StorePacked {
            byte_offset: 0,
            width: PackedByteWidth::Seven,
        },
        packed,
        &[POINTER, VALUE, PACKED_SCRATCH],
    );
    function.memory_accesses[0].byte_count = 7;
}

/// Rewrite the fixture's moved store into a byte-sequence `Store { 0, 1 }`
/// through a fully computed view address: its single `WriteByteSequence` row
/// carries the payload base `offset` and writes the byte at
/// `offset + index` for the runtime `index`, so its reach is unbounded
/// upward from `offset`.
fn sequence_store(
    function: &mut SelectedFunction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    offset: u32,
) {
    let store = environment
        .constraint(environment.selected_keys().store.unwrap())
        .unwrap();
    function.blocks[0].instructions[1] = instruction(
        STORE,
        SelectedInstructionKind::Store {
            byte_offset: 0,
            byte_size: 1,
        },
        store,
        &[POINTER, VALUE],
    );
    function.memory_accesses[0] = SelectedMemoryAccess {
        byte_count: 1,
        role: SelectedMemoryAccessRole::WriteByteSequence {
            index: ValueId::new(5).unwrap(),
            value: ValueId::new(6).unwrap(),
            length: ValueId::new(7).unwrap(),
            obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
            accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes([3; 32]),
        },
        ..access(
            STORE,
            1,
            place(),
            offset,
            SelectedMemoryAccessRole::WritePlace,
        )
    };
}

/// The intervening byte-sequence row's index register — the register whose
/// `InstructionResult` origin carries the row's `index` value: its sole
/// clean `MaterializeI64` definition makes `byte_offset + index` a fixed
/// position, so the row touches exactly that byte wherever its payload base
/// sits.
const SEQUENCE_INDEX: VirtualRegisterId = VirtualRegisterId(12);

/// The moved byte-sequence store's own index register: when its sole clean
/// `MaterializeI64` definition resolves, the moved extent collapses to the
/// one byte `byte_offset + index` before the walk.
const MOVED_SEQUENCE_INDEX: VirtualRegisterId = VirtualRegisterId(13);

/// The materialize instruction defining an intervening row's index register.
const MATERIALIZE_INDEX: SelectedInstructionId = SelectedInstructionId(10);

/// The materialize instruction defining the moved store's index register —
/// distinct from `MATERIALIZE_INDEX` so two clean definitions can coexist
/// in one fixture.
const MATERIALIZE_MOVED_INDEX: SelectedInstructionId = SelectedInstructionId(11);

/// Insert a clean `MaterializeI64` at `position` in `block` defining
/// `register` as `bits`, with the register's origin naming `source_value` —
/// the sole clean definition `materialized_bits` resolves, so a sequence
/// row's `index` becomes the compile-time `bits`.
#[allow(clippy::too_many_arguments)]
fn define_index_as(
    function: &mut SelectedFunction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    block: usize,
    position: usize,
    id: SelectedInstructionId,
    register: VirtualRegisterId,
    source_value: ValueId,
    bits: u64,
) {
    let materialize = environment
        .constraint(environment.selected_keys().materialize_i64)
        .unwrap();
    function.virtual_registers.push(VirtualRegister {
        id: register,
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        class: materialize.operands[0].class,
        origin: VirtualRegisterOrigin::InstructionResult {
            instruction: id,
            source_value,
        },
        definition_site: None,
        entry_fixed_view: None,
    });
    function.blocks[block].instructions.insert(
        position,
        instruction(
            id,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(u128::from(bits)),
            },
            materialize,
            &[register],
        ),
    );
}

/// `define_index_as` under the intervening row's materialize id, for
/// fixtures resolving only a scanned row's index.
fn define_index(
    function: &mut SelectedFunction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    block: usize,
    position: usize,
    register: VirtualRegisterId,
    source_value: ValueId,
    bits: u64,
) {
    define_index_as(
        function,
        environment,
        block,
        position,
        MATERIALIZE_INDEX,
        register,
        source_value,
        bits,
    );
}

/// The intervening byte-sequence row landing on `offset + index`'s resolved
/// byte: a `WriteByteSequence` roster row on instruction `id` with payload
/// base `offset` and index value `index_value`. The row attaches to the
/// instruction as found — the window's interference decision reads the row,
/// not the instruction's kind.
fn sequence_row(
    function: &mut SelectedFunction,
    id: SelectedInstructionId,
    offset: u32,
    index_value: u64,
) {
    function.memory_accesses.push(SelectedMemoryAccess {
        byte_count: 1,
        ..access(
            id,
            3,
            place(),
            offset,
            SelectedMemoryAccessRole::WriteByteSequence {
                index: ValueId::new(index_value).unwrap(),
                value: ValueId::new(6).unwrap(),
                length: ValueId::new(7).unwrap(),
                obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                    [3; 32],
                ),
            },
        )
    });
}

fn sink(
    source: &ValidatedStoreMutationMotion,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> Result<ValidatedStoreMutationMotion, StoreMutationMotionError> {
    sink_selected_store_mutation(source, 0, STORE, environment, budget())
}

fn landed_ids(result: &ValidatedStoreMutationMotion) -> Vec<SelectedInstructionId> {
    result.transformed().functions[0].blocks[0]
        .instructions
        .iter()
        .map(|instruction| instruction.id)
        .collect()
}

fn successor(block: u32) -> SelectedSuccessor {
    SelectedSuccessor {
        role: SelectedSuccessorRole::Semantic,
        psi_edge: EdgeId::new(2).unwrap(),
        block: SelectedBlockId(block),
        source_target: BlockId::new(u64::from(block) + 1).unwrap(),
        bindings: Vec::new(),
        structural_bindings: Vec::new(),
        structural_case: None,
        fuel: Vec::new(),
    }
}

/// The fixture stretched across one edge: block 0 keeps the moved store and
/// jumps to block 1, which opens with the covering store and returns. Block 0
/// is block 1's only predecessor and every edge out of block 0 reaches it, so
/// the write lands at block 1's head still before the covering store.
fn chained(target: NativeTarget) -> ValidatedStoreMutationMotion {
    mutated(target, |function, environment| {
        let jump = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        let tail_instructions = function.blocks[0].instructions.split_off(3);
        let tail_terminator = std::mem::replace(
            &mut function.blocks[0].terminator,
            SelectedTerminator::Jump {
                instruction: instruction(
                    SelectedInstructionId(6),
                    SelectedInstructionKind::Jump,
                    jump,
                    &[],
                ),
                successor: successor(1),
            },
        );
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(1),
            origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
            instructions: tail_instructions,
            terminator: tail_terminator,
        });
    })
}

/// Edit the chained fixture's function, then refresh the receipt identities
/// so the mutated plan is a well-formed analysis source.
fn mutated_chained(
    target: NativeTarget,
    edit: impl FnOnce(&mut SelectedFunction, &register_environment::ValidatedTargetRegisterEnvironment),
) -> ValidatedStoreMutationMotion {
    let environment = baseline_target_register_environment(target).unwrap();
    let mut source = chained(target);
    edit(
        &mut std::sync::Arc::make_mut(&mut source.transformed).functions[0],
        &environment,
    );
    let identity = selected_instruction_plan_identity(&source.transformed);
    source.receipt.source_selected = identity;
    source.receipt.transformed_selected = identity;
    source
}

/// The edge from block 0 into the covering store's block.
fn crossed_edge(function: &mut SelectedFunction) -> &mut SelectedSuccessor {
    let SelectedTerminator::Jump { successor, .. } = &mut function.blocks[0].terminator else {
        unreachable!()
    };
    successor
}
