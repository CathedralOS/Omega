//! Fixtures shared by the dead store tests: budgets, instructions, places,
//! settlements and the block fixtures.

mod clear_cycles;
mod cross_block_eliminations;
mod same_block_eliminations;

use crate::rewrites::dead_store::{
    DeadStoreEliminationError, DeadStoreEliminationReceipt, ValidatedDeadStoreElimination,
    eliminate_selected_dead_store,
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

/// The packed dead store's early-clobber scratch: the register occurs only
/// at that `Def`, the custody the packed removal requires.
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
/// return`. The first store's bytes are overwritten unobserved by the second.
fn fixture(target: NativeTarget) -> ValidatedDeadStoreElimination {
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
    ValidatedDeadStoreElimination {
        receipt: DeadStoreEliminationReceipt {
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
) -> ValidatedDeadStoreElimination {
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

fn eliminate(
    source: &ValidatedDeadStoreElimination,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> Result<ValidatedDeadStoreElimination, DeadStoreEliminationError> {
    eliminate_selected_dead_store(source, 0, STORE, environment, budget())
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

/// The fixture stretched across one edge: block 0 keeps the dead store and
/// jumps to block 1, which opens with the covering store and returns. Every
/// path forward from the store reaches block 1, so its head still covers.
fn chained(target: NativeTarget) -> ValidatedDeadStoreElimination {
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
) -> ValidatedDeadStoreElimination {
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

/// Replace the dead `Store` with its packed form: a `StorePacked` of five
/// bytes at offset 0 on the target's packed row — `[use pointer, use value,
/// def scratch]` — with PACKED_SCRATCH occurring only at that `Def`. The
/// packed range sits inside the covering store's eight bytes at offset 0.
fn make_packed_dead(
    function: &mut SelectedFunction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) {
    let packed = environment
        .constraint(environment.selected_keys().store_packed.unwrap())
        .unwrap();
    function.virtual_registers.push(VirtualRegister {
        id: PACKED_SCRATCH,
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        class: packed.operands[0].class,
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
            width: PackedByteWidth::Five,
        },
        packed,
        &[POINTER, VALUE, PACKED_SCRATCH],
    );
    function.memory_accesses[0].byte_count = 5;
}

/// The single-block fixture with the packed dead store.
fn packed_dead(target: NativeTarget) -> ValidatedDeadStoreElimination {
    mutated(target, |function, environment| {
        make_packed_dead(function, environment)
    })
}

/// Rewrite instruction `id` into a byte-sequence `Store { 0, 1 }` through a
/// fully computed view address, and its roster row at `row` into the
/// `WriteByteSequence` carrying the payload base `offset`: the written byte
/// sits at `offset + index` for the runtime `index`, so the row's extent is
/// unbounded upward from `offset`.
fn sequence_store(
    function: &mut SelectedFunction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    id: SelectedInstructionId,
    row: usize,
    offset: u32,
    index: u64,
    value: VirtualRegisterId,
) {
    let store = environment
        .constraint(environment.selected_keys().store.unwrap())
        .unwrap();
    for block in &mut function.blocks {
        if let Some(position) = block
            .instructions
            .iter()
            .position(|instruction| instruction.id == id)
        {
            block.instructions[position] = instruction(
                id,
                SelectedInstructionKind::Store {
                    byte_offset: 0,
                    byte_size: 1,
                },
                store,
                &[POINTER, value],
            );
        }
    }
    let access = &mut function.memory_accesses[row];
    access.byte_offset = offset;
    access.byte_count = 1;
    access.role = SelectedMemoryAccessRole::WriteByteSequence {
        index: ValueId::new(index).unwrap(),
        value: ValueId::new(6).unwrap(),
        length: ValueId::new(7).unwrap(),
        obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
        accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes([3; 32]),
    };
}

/// The chained fixture with the packed dead store: the packed store sits in
/// block 0 and the covering store opens block 1 across the edge.
fn packed_dead_chained(target: NativeTarget) -> ValidatedDeadStoreElimination {
    mutated_chained(target, |function, environment| {
        make_packed_dead(function, environment)
    })
}

/// The materialize instruction the covering `CopyBytes`'s count resolves to.
const MATERIALIZE_COUNT: SelectedInstructionId = SelectedInstructionId(7);

/// The covering `CopyBytes`'s source pointer.
const SPAN_SOURCE: VirtualRegisterId = VirtualRegisterId(10);

/// The covering `CopyBytes`'s count register — the destination span's real
/// extent: its function-wide definition decides whether the written span is
/// a constant range that can cover the dead bytes.
const SPAN_COUNT: VirtualRegisterId = VirtualRegisterId(11);

/// The covering `CopyBytes`'s early-clobber scratch registers.
const SPAN_CURSOR: VirtualRegisterId = VirtualRegisterId(12);
const SPAN_BYTE: VirtualRegisterId = VirtualRegisterId(13);

/// The semantic value the destination span's `length` and the count
/// register's `source_value` share — row-instruction agreement for the
/// dynamic extent.
fn span_length() -> ValueId {
    ValueId::new(7).unwrap()
}

/// The covering byte-sequence store's index register — the register whose
/// `InstructionResult` origin carries the row's `index` value: its sole
/// clean `MaterializeI64` definition makes `byte_offset + index` a fixed
/// position, so the write can cover the exact dead byte it lands on.
const SEQUENCE_INDEX: VirtualRegisterId = VirtualRegisterId(14);

/// The byte-sequence dead store's own index register: when a covering
/// sequence write names a different `index` value, both must resolve to a
/// materialized constant whose `byte_offset + index` sum lands on the dead
/// byte.
const DEAD_SEQUENCE_INDEX: VirtualRegisterId = VirtualRegisterId(16);

/// The dead `CopyBytes`'s own source pointer, count register, and
/// early-clobber scratch registers — distinct from the covering copy's so
/// two `CopyBytes` fixtures can coexist without sharing a scratch custody.
const DEAD_SPAN_SOURCE: VirtualRegisterId = VirtualRegisterId(17);
const DEAD_SPAN_COUNT: VirtualRegisterId = VirtualRegisterId(18);
const DEAD_SPAN_CURSOR: VirtualRegisterId = VirtualRegisterId(19);
const DEAD_SPAN_BYTE: VirtualRegisterId = VirtualRegisterId(20);

/// The semantic value the dead `CopyBytes`'s destination `length` and its
/// count register's `source_value` share. Distinct from `span_length()` so
/// a covering copy's materialized count never resolves the dead copy's
/// extent by accident.
fn dead_span_length() -> ValueId {
    ValueId::new(13).unwrap()
}

/// The materialize instruction defining the dead store's index register —
/// distinct from `MATERIALIZE_COUNT` so two clean definitions can coexist
/// in one fixture.
const MATERIALIZE_INDEX: SelectedInstructionId = SelectedInstructionId(12);

/// Narrow the fixture's dead store to a one-byte `Store` at `offset` — the
/// only exact dead range a byte-sequence write's single byte can cover.
/// Works on the chained fixture too: its block 0 keeps the same head.
fn dead_byte(
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
            byte_offset: offset,
            byte_size: 1,
        },
        store,
        &[POINTER, VALUE],
    );
    function.memory_accesses[0] = SelectedMemoryAccess {
        byte_count: 1,
        ..access(
            STORE,
            1,
            place(),
            offset,
            SelectedMemoryAccessRole::WritePlace,
        )
    };
}

/// Rewrite instruction `id` into a `CopyBytes` on the target's declared row —
/// `[use source, use destination, use count]` plus the two early-clobber
/// scratch defs — and its roster row at `write_row` into the destination
/// `WriteByteSpan` claiming `length` bytes at `byte_offset`. The copy's
/// source `ReadByteSpan` rides on a disjoint second place, quiet on the dead
/// range. The `count` register's origin names `length` — the row-instruction
/// agreement a dynamic extent needs — but only an added definition decides
/// what the span really writes.
fn span_copy(
    function: &mut SelectedFunction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    id: SelectedInstructionId,
    write_row: usize,
    byte_offset: u32,
    count: VirtualRegisterId,
    length: ValueId,
) {
    span_copy_on(
        function,
        environment,
        id,
        write_row,
        byte_offset,
        SPAN_SOURCE,
        ValueId::new(11).unwrap(),
        count,
        SPAN_CURSOR,
        SPAN_BYTE,
        length,
    );
}

/// `span_copy` with caller-chosen source, scratch, and count registers, so a
/// fixture can carry two `CopyBytes` — a dead copy beside the covering one —
/// without sharing a scratch custody or a source identity.
#[allow(clippy::too_many_arguments)]
fn span_copy_on(
    function: &mut SelectedFunction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    id: SelectedInstructionId,
    write_row: usize,
    byte_offset: u32,
    source: VirtualRegisterId,
    source_value: ValueId,
    count: VirtualRegisterId,
    cursor: VirtualRegisterId,
    byte: VirtualRegisterId,
    length: ValueId,
) {
    let copy = environment
        .constraint(environment.selected_keys().copy_bytes.unwrap())
        .unwrap();
    for block in &mut function.blocks {
        if let Some(position) = block
            .instructions
            .iter()
            .position(|instruction| instruction.id == id)
        {
            block.instructions[position] = instruction(
                id,
                SelectedInstructionKind::CopyBytes,
                copy,
                &[source, POINTER, count, cursor, byte],
            );
        }
    }
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    for (register, operand) in [(cursor, 3), (byte, 4)] {
        function.virtual_registers.push(VirtualRegister {
            id: register,
            scalar_type,
            class: copy.operands[operand].class,
            origin: VirtualRegisterOrigin::InstructionScratch {
                instruction: id,
                operand: operand as u16,
            },
            definition_site: None,
            entry_fixed_view: None,
        });
    }
    function.virtual_registers.push(VirtualRegister {
        id: source,
        scalar_type,
        class: copy.operands[0].class,
        origin: VirtualRegisterOrigin::EntryParameter {
            source_value,
            parameter_index: 1,
        },
        definition_site: None,
        entry_fixed_view: None,
    });
    let write = &mut function.memory_accesses[write_row];
    write.instruction = id;
    write.byte_offset = byte_offset;
    write.byte_count = 0;
    write.role = SelectedMemoryAccessRole::WriteByteSpan {
        length,
        obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
        accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes([3; 32]),
    };
    function.memory_accesses.push(SelectedMemoryAccess {
        byte_count: 0,
        ..access(
            id,
            8,
            PlaceId::new(2).unwrap(),
            0,
            SelectedMemoryAccessRole::ReadByteSpan {
                length,
                obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                    [3; 32],
                ),
            },
        )
    });
}

/// Insert a clean `MaterializeI64` at `position` in `block` defining
/// `register` as `bits`, with the register's origin naming `source_value` —
/// the sole clean definition `materialized_bits` resolves, so a covering
/// span's `count` or a sequence write's `index` becomes the compile-time
/// `bits`.
fn define_count(
    function: &mut SelectedFunction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    block: usize,
    position: usize,
    register: VirtualRegisterId,
    source_value: ValueId,
    bits: u64,
) {
    define_count_as(
        function,
        environment,
        block,
        position,
        MATERIALIZE_COUNT,
        register,
        source_value,
        bits,
    );
}

/// `define_count` under a caller-chosen instruction id, for fixtures that
/// materialize two constants — the dead store's index beside the covering
/// write's.
#[allow(clippy::too_many_arguments)]
fn define_count_as(
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

/// Rewrite the fixture's dead store — instruction `STORE` at roster row 0 —
/// into a `CopyBytes`: the destination `WriteByteSpan` claims `length`
/// bytes at `byte_offset` on the dead place, and the source `ReadByteSpan`
/// rides on the disjoint second place, so the roster grows a second row the
/// elimination must drop beside the write. `count` is the dead copy's count
/// register: an added `MaterializeI64` carrying `length` collapses the dead
/// extent to a constant, while a `runtime_count` entry parameter leaves it
/// unbounded upward.
fn dead_span_copy(
    function: &mut SelectedFunction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    byte_offset: u32,
    count: VirtualRegisterId,
    length: ValueId,
) {
    span_copy_on(
        function,
        environment,
        STORE,
        0,
        byte_offset,
        DEAD_SPAN_SOURCE,
        ValueId::new(14).unwrap(),
        count,
        DEAD_SPAN_CURSOR,
        DEAD_SPAN_BYTE,
        length,
    );
}

/// Push `register` as an entry parameter carrying `source_value` — the
/// count-register shape a `CopyBytes` takes when its `length` has no
/// materializing producer anywhere in the function, so the span's extent
/// stays runtime.
fn runtime_count(
    function: &mut SelectedFunction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    register: VirtualRegisterId,
    source_value: ValueId,
) {
    let copy = environment
        .constraint(environment.selected_keys().copy_bytes.unwrap())
        .unwrap();
    function.virtual_registers.push(VirtualRegister {
        id: register,
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        class: copy.operands[2].class,
        origin: VirtualRegisterOrigin::EntryParameter {
            source_value,
            parameter_index: 2,
        },
        definition_site: None,
        entry_fixed_view: None,
    });
}
