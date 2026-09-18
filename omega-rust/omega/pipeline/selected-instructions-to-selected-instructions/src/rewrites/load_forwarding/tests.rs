//! Fixtures shared by the load forwarding tests: budgets, instructions,
//! places, accesses and the block fixtures.

mod cross_block_forwarding;
mod same_block_forwarding;

use crate::{
    StoredLoadForwardingError, StoredLoadForwardingReceipt, ValidatedStoredLoadForwarding,
    forward_selected_stored_load,
};
use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget};
use optimization_unit::ValueDefinitionSite;
use register_environment::baseline_target_register_environment;
use register_model::RegisterInstructionConstraint;
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan, SelectedMemoryAccess,
    SelectedMemoryAccessOrigin, SelectedMemoryAccessRole, SelectedOperand, SelectedSuccessor,
    SelectedSuccessorRole, SelectedTerminator, VirtualRegister, VirtualRegisterId,
    VirtualRegisterOrigin,
};
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, MachineId,
    OperationId, PlaceId, ScalarType, ValueId,
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

const LOAD: SelectedInstructionId = SelectedInstructionId(4);

const POINTER: VirtualRegisterId = VirtualRegisterId(0);

const VALUE: VirtualRegisterId = VirtualRegisterId(1);

const SCRATCH: VirtualRegisterId = VirtualRegisterId(2);

const OUTPUT: VirtualRegisterId = VirtualRegisterId(3);

/// The indexed byte load's runtime index register.
const INDEX: VirtualRegisterId = VirtualRegisterId(4);

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

/// A raw selected-stage unit fixture, not a source/Terminal admission claim:
/// `r1 = copy r0; store [r0+0] <- r1; r3 = copy r0; r2 = load [r0+0]; return`.
fn fixture(target: NativeTarget) -> ValidatedStoredLoadForwarding {
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let copy = environment.constraint(keys.copy_i64).unwrap();
    let store = environment.constraint(keys.store.unwrap()).unwrap();
    let load = environment.constraint(keys.load64.unwrap()).unwrap();
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
        VirtualRegister {
            id: OUTPUT,
            scalar_type,
            class,
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction: LOAD,
                source_value: ValueId::new(4).unwrap(),
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
            let mut forwarded_load = instruction(
                LOAD,
                SelectedInstructionKind::Load64 { byte_offset: 0 },
                load,
                &[POINTER, OUTPUT],
            );
            forwarded_load.provenance.operations = vec![OperationId::new(2).unwrap()];
            forwarded_load.provenance.values = vec![ValueId::new(4).unwrap()];
            forwarded_load
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
            memory_accesses: vec![
                access(STORE, 1, place, 0, SelectedMemoryAccessRole::WritePlace),
                access(LOAD, 2, place, 0, SelectedMemoryAccessRole::ReadPlace),
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
    ValidatedStoredLoadForwarding {
        receipt: StoredLoadForwardingReceipt {
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
) -> ValidatedStoredLoadForwarding {
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

fn forward(
    source: &ValidatedStoredLoadForwarding,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> Result<ValidatedStoredLoadForwarding, StoredLoadForwardingError> {
    forward_selected_stored_load(source, 0, LOAD, environment, budget())
}

/// The fixture narrowed to an exact-width sub-word pair: `Store { byte_size }`
/// and the matching `Load8`/`Load16`/`Load32` with `byte_count`-matched rows.
fn narrowed(target: NativeTarget, byte_size: u8) -> ValidatedStoredLoadForwarding {
    mutated(target, |function, environment| {
        let keys = environment.selected_keys();
        let store = environment.constraint(keys.store.unwrap()).unwrap();
        function.blocks[0].instructions[1] = instruction(
            STORE,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size,
            },
            store,
            &[POINTER, VALUE],
        );
        let (kind, key) = match byte_size {
            1 => (
                SelectedInstructionKind::Load8 { byte_offset: 0 },
                keys.load8,
            ),
            2 => (
                SelectedInstructionKind::Load16 { byte_offset: 0 },
                keys.load16,
            ),
            _ => (
                SelectedInstructionKind::Load32 { byte_offset: 0 },
                keys.load32,
            ),
        };
        let load = environment.constraint(key.unwrap()).unwrap();
        let mut rewritten = instruction(LOAD, kind, load, &[POINTER, OUTPUT]);
        rewritten.provenance.operations = vec![OperationId::new(2).unwrap()];
        rewritten.provenance.values = vec![ValueId::new(4).unwrap()];
        function.blocks[0].instructions[3] = rewritten;
        function.memory_accesses[0].byte_count = u32::from(byte_size);
        function.memory_accesses[1].byte_count = u32::from(byte_size);
    })
}

/// Rewrite the fixture's store into the byte-sequence `Store { 0, 1 }`
/// through a fully computed view address carrying `WriteByteSequence`, and
/// its load into the indexed byte load `Load8Indexed` carrying
/// `ReadByteSequence`: both rows name payload base `offset` and runtime
/// index `index`, so the byte the store writes is the byte the load reads.
/// The fixture's two roster rows keep their order — the store's first, the
/// load's second — and the load's extra index operand takes a fresh
/// parameter register.
fn sequence_pair(
    function: &mut SelectedFunction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    offset: u32,
    index: u64,
) {
    let keys = environment.selected_keys();
    let store = environment.constraint(keys.store.unwrap()).unwrap();
    let load = environment.constraint(keys.load8_indexed.unwrap()).unwrap();
    function.virtual_registers.push(VirtualRegister {
        id: INDEX,
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        class: load.operands[0].class,
        origin: VirtualRegisterOrigin::EntryParameter {
            source_value: ValueId::new(9).unwrap(),
            parameter_index: 1,
        },
        definition_site: Some(ValueDefinitionSite::FunctionParameter(1)),
        entry_fixed_view: None,
    });
    for block in &mut function.blocks {
        for position in 0..block.instructions.len() {
            if block.instructions[position].id == STORE {
                block.instructions[position] = instruction(
                    STORE,
                    SelectedInstructionKind::Store {
                        byte_offset: 0,
                        byte_size: 1,
                    },
                    store,
                    &[POINTER, VALUE],
                );
            } else if block.instructions[position].id == LOAD {
                let mut rewritten = instruction(
                    LOAD,
                    SelectedInstructionKind::Load8Indexed,
                    load,
                    &[POINTER, INDEX, OUTPUT],
                );
                rewritten.provenance.operations = vec![OperationId::new(2).unwrap()];
                rewritten.provenance.values = vec![ValueId::new(4).unwrap()];
                block.instructions[position] = rewritten;
            }
        }
    }
    let index_value = ValueId::new(index).unwrap();
    let write = &mut function.memory_accesses[0];
    write.byte_offset = offset;
    write.byte_count = 1;
    write.role = SelectedMemoryAccessRole::WriteByteSequence {
        index: index_value,
        value: ValueId::new(6).unwrap(),
        length: ValueId::new(7).unwrap(),
        obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
        accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes([3; 32]),
    };
    let read = &mut function.memory_accesses[1];
    read.byte_offset = offset;
    read.byte_count = 1;
    read.role = SelectedMemoryAccessRole::ReadByteSequence {
        index: index_value,
        length: ValueId::new(7).unwrap(),
        obligation: semantic_vocabulary::ObligationId::new(2).unwrap(),
        accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes([4; 32]),
    };
}

/// The byte-sequence writer's index register — the register whose
/// `InstructionResult` origin carries the row's `index` value: its sole
/// clean `MaterializeI64` definition makes `byte_offset + index` a fixed
/// position, so the write lands on one known byte.
const SEQUENCE_INDEX: VirtualRegisterId = VirtualRegisterId(5);

/// The indexed byte load's own index register: when a sequence writer names
/// a different `index` value, both must resolve to materialized constants
/// whose `byte_offset + index` sums name one byte — and when the read's own
/// index resolves, the read extent collapses to that fixed byte.
const READ_INDEX: VirtualRegisterId = VirtualRegisterId(6);

/// The materialize instruction defining a sequence write's index register.
const MATERIALIZE_INDEX: SelectedInstructionId = SelectedInstructionId(7);

/// The materialize instruction defining the load's index register —
/// distinct from `MATERIALIZE_INDEX` so two clean definitions can coexist
/// in one fixture.
const MATERIALIZE_READ_INDEX: SelectedInstructionId = SelectedInstructionId(8);

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

/// `define_index_as` under the sequence write's materialize id, for fixtures
/// resolving only the writer's index.
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

/// Rewrite the roster row at `row` into the `WriteByteSequence` carrying
/// payload base `offset` and index value `index`, and its instruction `id`
/// into the byte-sequence `Store { 0, 1 }` — the intervening sequence write
/// the constant-index tests place between the source and the load.
fn sequence_write(
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

/// A plain semantic edge to `block` carrying no transfers; tests mutate its
/// bindings, structural bindings, and case metadata to exercise edge-level
/// killers.
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

/// The fixture stretched across one edge: block 0 keeps the store and jumps
/// to block 1, which holds the load and returns. Every path to the load
/// passes block 0's tail, so the store still decides on all of them.
fn chained(target: NativeTarget) -> ValidatedStoredLoadForwarding {
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
) -> ValidatedStoredLoadForwarding {
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

/// The edge from block 0 into the load's block.
fn crossed_edge(function: &mut SelectedFunction) -> &mut SelectedSuccessor {
    let SelectedTerminator::Jump { successor, .. } = &mut function.blocks[0].terminator else {
        unreachable!()
    };
    successor
}
