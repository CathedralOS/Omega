use crate::DeadCompareError;
use crate::DeadCompareReceipt;
use crate::ValidatedDeadCompare;
use crate::ValidatedSelectedAnalysis;
use crate::remove_dead_compare;
use crate::validate_dead_compare;
use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget};
use optimization_unit::{EffectLink, ValueDefinitionSite};
use register_environment::baseline_target_register_environment;
use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedBoundarySettlement,
    SelectedBoundarySettlementPayload, SelectedCallContract, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan, SelectedMemoryAccess,
    SelectedMemoryAccessOrigin, SelectedMemoryAccessRole, SelectedOperand, SelectedSuccessor,
    SelectedSuccessorRole, SelectedTerminator, VirtualRegister, VirtualRegisterId,
    VirtualRegisterOrigin,
};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType,
    IntegerValue, MachineId, ObligationId, OperationId, PlaceId, ScalarType, ValueId,
};
use target::NativeTarget;
use target_operations_to_selected_instructions::selected_instruction_plan_identity;
use terminal_psi::{
    CrashCause, CrashRouteBucket, CrashRouteGuard, SemanticFingerprint, TerminalPsiIdentity,
    VocabularyMarker,
};

fn budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(100, 100, 100_000, 100, 100).unwrap()
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

const LOAD: SelectedInstructionId = SelectedInstructionId(2);
const SECOND_LOAD: SelectedInstructionId = SelectedInstructionId(3);
const COMPARE: SelectedInstructionId = SelectedInstructionId(4);
const SECOND: SelectedInstructionId = SelectedInstructionId(5);
const TERMINAL: SelectedInstructionId = SelectedInstructionId(6);
const BOOLEAN: SelectedInstructionId = SelectedInstructionId(7);

const POINTER: VirtualRegisterId = VirtualRegisterId(0);
const INPUT: VirtualRegisterId = VirtualRegisterId(1);
const OTHER: VirtualRegisterId = VirtualRegisterId(2);
const OUTPUT: VirtualRegisterId = VirtualRegisterId(3);

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

fn successor(block: u32, edge: u64) -> SelectedSuccessor {
    SelectedSuccessor {
        role: SelectedSuccessorRole::Semantic,
        psi_edge: EdgeId::new(edge).unwrap(),
        block: SelectedBlockId(block),
        source_target: BlockId::new(u64::from(block) + 1).unwrap(),
        bindings: Vec::new(),
        structural_bindings: Vec::new(),
        structural_case: None,
        fuel: Vec::new(),
    }
}

fn returning_block(
    block: u32,
    instruction_id: u32,
    edge: u64,
    row: &RegisterInstructionConstraint,
) -> SelectedBlock {
    SelectedBlock {
        id: SelectedBlockId(block),
        origin: SelectedBlockOrigin::Source(BlockId::new(u64::from(block) + 1).unwrap()),
        instructions: Vec::new(),
        terminator: SelectedTerminator::Return {
            instruction: instruction(
                SelectedInstructionId(instruction_id),
                SelectedInstructionKind::ReturnUnit,
                row,
                &[],
            ),
            psi_return_edge: EdgeId::new(edge).unwrap(),
        },
    }
}

/// A raw selected-stage unit fixture, not a source/Terminal admission claim:
/// `r1 = load8 r0; r2 = load8 r0; compare r1, r2; compare r2, r1; return`.
/// The first compare's flag definitions are republished by the second before
/// any reader can observe them, so the first publishes dead state.
fn fixture(target: NativeTarget) -> ValidatedDeadCompare {
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let load = environment.constraint(keys.load8.unwrap()).unwrap();
    let compare = environment.constraint(keys.compare_i64).unwrap();
    let terminal_row = environment.constraint(keys.return_unit).unwrap();
    let class = compare.operands[0].class;
    let registers = vec![
        VirtualRegister {
            id: POINTER,
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
            class,
            origin: VirtualRegisterOrigin::EntryParameter {
                source_value: ValueId::new(1).unwrap(),
                parameter_index: 0,
            },
            definition_site: Some(ValueDefinitionSite::FunctionParameter(0)),
            entry_fixed_view: None,
        },
        register(
            INPUT,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: LOAD,
                source_value: ValueId::new(2).unwrap(),
            },
        ),
        register(
            OTHER,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: SECOND_LOAD,
                source_value: ValueId::new(3).unwrap(),
            },
        ),
    ];
    let mut compare_instruction = instruction(
        COMPARE,
        SelectedInstructionKind::CompareI64,
        compare,
        &[INPUT, OTHER],
    );
    compare_instruction.provenance.operations = vec![OperationId::new(7).unwrap()];
    compare_instruction.provenance.values = vec![ValueId::new(3).unwrap()];
    let machine = MachineId::new(1).unwrap();
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
            memory_accesses: Vec::new(),
            boundary_settlements: Vec::new(),
            entry_block: SelectedBlockId(0),
            virtual_registers: registers,
            blocks: vec![SelectedBlock {
                id: SelectedBlockId(0),
                origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
                instructions: vec![
                    instruction(
                        LOAD,
                        SelectedInstructionKind::Load8 { byte_offset: 0 },
                        load,
                        &[POINTER, INPUT],
                    ),
                    instruction(
                        SECOND_LOAD,
                        SelectedInstructionKind::Load8 { byte_offset: 0 },
                        load,
                        &[POINTER, OTHER],
                    ),
                    compare_instruction,
                    instruction(
                        SECOND,
                        SelectedInstructionKind::CompareI64,
                        compare,
                        &[OTHER, INPUT],
                    ),
                ],
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
    };
    let identity = selected_instruction_plan_identity(&plan);
    ValidatedDeadCompare {
        receipt: DeadCompareReceipt {
            source_selected: identity,
            transformed_selected: identity,
            optimization_unit: OptimizationUnitIdentity::from_bytes([2; 32]),
            fuel_schedule: plan.fuel_schedule,
        },
        transformed: std::sync::Arc::new(plan),
    }
}

fn mutated(
    target: NativeTarget,
    edit: impl FnOnce(&mut SelectedFunction, &register_environment::ValidatedTargetRegisterEnvironment),
) -> ValidatedDeadCompare {
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

fn remove(
    source: &ValidatedDeadCompare,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> Result<ValidatedDeadCompare, DeadCompareError> {
    remove_dead_compare(source, 0, COMPARE, environment, budget())
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

/// A compare whose published flag units are republished by a later compare
/// before any reader is dead: it leaves the block on every target, and the
/// result replays by content.
#[test]
fn republished_flags_remove_on_every_target() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = fixture(target);
        let result = remove(&source, &environment).unwrap();
        let function = &result.transformed().functions[0];
        let block = &function.blocks[0];
        assert_eq!(block.instructions.len(), 3);
        assert_eq!(block.instructions[0].id, LOAD);
        assert_eq!(block.instructions[1].id, SECOND_LOAD);
        assert_eq!(block.instructions[2].id, SECOND);
        assert_eq!(
            result.receipt().source_selected(),
            source.selected_identity()
        );
        assert_eq!(
            result.receipt().transformed_selected(),
            selected_instruction_plan_identity(result.transformed())
        );
        validate_dead_compare(
            &source,
            0,
            COMPARE,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
        // A detached, separately allocated proposal replays by content.
        let mut detached = result.transformed().clone();
        detached.functions = detached.functions.iter().cloned().collect();
        validate_dead_compare(&source, 0, COMPARE, &environment, budget(), detached).unwrap();
    }
}

/// The immediate and zero compare forms carry the same flag-only surface:
/// each removes under the same dead-surface audit.
#[test]
fn immediate_and_zero_forms_admit() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for (kind, key) in [
        (
            SelectedInstructionKind::CompareI64Immediate {
                immediate: IntegerValue::Unsigned(7),
            },
            environment.selected_keys().compare_i64_immediate,
        ),
        (
            SelectedInstructionKind::CompareI64Zero,
            environment.selected_keys().compare_i64_zero,
        ),
    ] {
        let row = environment.constraint(key).unwrap().clone();
        let source = mutated(target, |function, _| {
            function.blocks[0].instructions[2] = instruction(COMPARE, kind, &row, &[INPUT]);
        });
        let result = remove(&source, &environment).unwrap();
        let block = &result.transformed().functions[0].blocks[0];
        assert_eq!(block.instructions.len(), 3);
        assert!(block.instructions.iter().all(|entry| entry.id != COMPARE));
    }
}

/// A compare whose flags reach the terminator's conditional branch is live:
/// the branch is a reader, so the removal refuses.
#[test]
fn branch_reader_in_terminator_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let branch_row = environment
        .constraint(environment.selected_keys().conditional_branch)
        .unwrap()
        .clone();
    let return_row = environment
        .constraint(environment.selected_keys().return_unit)
        .unwrap()
        .clone();
    let source = mutated(target, |function, _| {
        // Drop the second compare so the branch reads the first's flags.
        function.blocks[0].instructions.remove(3);
        function.blocks[0].terminator = SelectedTerminator::ConditionalBranch {
            instruction: instruction(
                TERMINAL,
                SelectedInstructionKind::ConditionalBranchNonZero,
                &branch_row,
                &[],
            ),
            when_nonzero: successor(1, 2),
            when_zero: successor(2, 3),
        };
        function.blocks.push(returning_block(1, 10, 4, &return_row));
        function.blocks.push(returning_block(2, 11, 5, &return_row));
    });
    assert_eq!(
        remove(&source, &environment).unwrap_err(),
        DeadCompareError::UnsupportedUse
    );
}

/// A materialized boolean in the same block reads the compare's flags
/// before the second compare republishes them — the compare stays live.
#[test]
fn materialization_reader_in_block_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let boolean_row = environment
        .constraint(environment.selected_keys().materialize_boolean)
        .unwrap()
        .clone();
    let source = mutated(target, |function, _| {
        let class = function.virtual_registers[1].class;
        function.virtual_registers.push(register(
            OUTPUT,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: BOOLEAN,
                source_value: ValueId::new(6).unwrap(),
            },
        ));
        function.blocks[0].instructions.insert(
            3,
            instruction(
                BOOLEAN,
                SelectedInstructionKind::MaterializeBooleanEqual,
                &boolean_row,
                &[OUTPUT],
            ),
        );
    });
    assert_eq!(
        remove(&source, &environment).unwrap_err(),
        DeadCompareError::UnsupportedUse
    );
}

/// A reader in a successor block is the same observation: the flag unit
/// stays live across the jump edge and reaches the materialization.
#[test]
fn reader_through_successor_edge_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let jump_row = environment
        .constraint(environment.selected_keys().jump)
        .unwrap()
        .clone();
    let boolean_row = environment
        .constraint(environment.selected_keys().materialize_boolean)
        .unwrap()
        .clone();
    let return_row = environment
        .constraint(environment.selected_keys().return_unit)
        .unwrap()
        .clone();
    let source = mutated(target, |function, _| {
        let class = function.virtual_registers[1].class;
        function.virtual_registers.push(register(
            OUTPUT,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: BOOLEAN,
                source_value: ValueId::new(6).unwrap(),
            },
        ));
        // The compare ends its block; the successor's first instruction
        // reads the flags it published.
        function.blocks[0].instructions.truncate(3);
        function.blocks[0].terminator = SelectedTerminator::Jump {
            instruction: instruction(TERMINAL, SelectedInstructionKind::Jump, &jump_row, &[]),
            successor: successor(1, 2),
        };
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(1),
            origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
            instructions: vec![instruction(
                BOOLEAN,
                SelectedInstructionKind::MaterializeBooleanEqual,
                &boolean_row,
                &[OUTPUT],
            )],
            terminator: SelectedTerminator::Return {
                instruction: instruction(
                    SelectedInstructionId(10),
                    SelectedInstructionKind::ReturnUnit,
                    &return_row,
                    &[],
                ),
                psi_return_edge: EdgeId::new(3).unwrap(),
            },
        });
    });
    assert_eq!(
        remove(&source, &environment).unwrap_err(),
        DeadCompareError::UnsupportedUse
    );
}

/// The flag unit crossing an edge dies at the successor's own compare:
/// the removal admits through the jump.
#[test]
fn redefinition_through_successor_edge_admits() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let jump_row = environment
        .constraint(environment.selected_keys().jump)
        .unwrap()
        .clone();
    let compare_row = environment
        .constraint(environment.selected_keys().compare_i64)
        .unwrap()
        .clone();
    let return_row = environment
        .constraint(environment.selected_keys().return_unit)
        .unwrap()
        .clone();
    let source = mutated(target, |function, _| {
        function.blocks[0].instructions.truncate(3);
        function.blocks[0].terminator = SelectedTerminator::Jump {
            instruction: instruction(TERMINAL, SelectedInstructionKind::Jump, &jump_row, &[]),
            successor: successor(1, 2),
        };
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(1),
            origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
            instructions: vec![instruction(
                SECOND,
                SelectedInstructionKind::CompareI64,
                &compare_row,
                &[OTHER, INPUT],
            )],
            terminator: SelectedTerminator::Return {
                instruction: instruction(
                    SelectedInstructionId(10),
                    SelectedInstructionKind::ReturnUnit,
                    &return_row,
                    &[],
                ),
                psi_return_edge: EdgeId::new(3).unwrap(),
            },
        });
    });
    let result = remove(&source, &environment).unwrap();
    let block = &result.transformed().functions[0].blocks[0];
    assert_eq!(block.instructions.len(), 2);
    assert!(block.instructions.iter().all(|entry| entry.id != COMPARE));
}

/// A loop carrying the flag unit back into the compare's own block re-scans
/// it from the head; the compare's own redefinition ends the unit there and
/// the removal still admits.
#[test]
fn loop_carried_unit_dies_at_the_compare() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let jump_row = environment
        .constraint(environment.selected_keys().jump)
        .unwrap()
        .clone();
    let source = mutated(target, |function, _| {
        function.blocks[0].instructions.truncate(3);
        function.blocks[0].terminator = SelectedTerminator::Jump {
            instruction: instruction(TERMINAL, SelectedInstructionKind::Jump, &jump_row, &[]),
            successor: successor(1, 2),
        };
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(1),
            origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
            instructions: Vec::new(),
            terminator: SelectedTerminator::Jump {
                instruction: instruction(
                    SelectedInstructionId(10),
                    SelectedInstructionKind::Jump,
                    &jump_row,
                    &[],
                ),
                successor: successor(0, 3),
            },
        });
    });
    let result = remove(&source, &environment).unwrap();
    let block = &result.transformed().functions[0].blocks[0];
    assert_eq!(block.instructions.len(), 2);
}

/// The loop-back rescan still observes a reader that precedes the compare
/// in its own block: on the second traversal that reader would see the
/// compare's flags, so the removal refuses.
#[test]
fn loop_back_reader_before_the_compare_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let jump_row = environment
        .constraint(environment.selected_keys().jump)
        .unwrap()
        .clone();
    let boolean_row = environment
        .constraint(environment.selected_keys().materialize_boolean)
        .unwrap()
        .clone();
    let source = mutated(target, |function, _| {
        let class = function.virtual_registers[1].class;
        function.virtual_registers.push(register(
            OUTPUT,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: BOOLEAN,
                source_value: ValueId::new(6).unwrap(),
            },
        ));
        function.blocks[0].instructions.truncate(3);
        function.blocks[0].instructions.insert(
            0,
            instruction(
                BOOLEAN,
                SelectedInstructionKind::MaterializeBooleanEqual,
                &boolean_row,
                &[OUTPUT],
            ),
        );
        function.blocks[0].terminator = SelectedTerminator::Jump {
            instruction: instruction(TERMINAL, SelectedInstructionKind::Jump, &jump_row, &[]),
            successor: successor(1, 2),
        };
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(1),
            origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
            instructions: Vec::new(),
            terminator: SelectedTerminator::Jump {
                instruction: instruction(
                    SelectedInstructionId(10),
                    SelectedInstructionKind::Jump,
                    &jump_row,
                    &[],
                ),
                successor: successor(0, 3),
            },
        });
    });
    assert_eq!(
        remove(&source, &environment).unwrap_err(),
        DeadCompareError::UnsupportedUse
    );
}

/// A successor edge naming a block the function does not contain leaves the
/// unit's readers unprovable: the walk refuses rather than assume dead.
#[test]
fn missing_successor_target_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let jump_row = environment
        .constraint(environment.selected_keys().jump)
        .unwrap()
        .clone();
    let source = mutated(target, |function, _| {
        function.blocks[0].instructions.truncate(3);
        function.blocks[0].terminator = SelectedTerminator::Jump {
            instruction: instruction(TERMINAL, SelectedInstructionKind::Jump, &jump_row, &[]),
            successor: successor(9, 2),
        };
    });
    assert_eq!(
        remove(&source, &environment).unwrap_err(),
        DeadCompareError::UnsupportedUse
    );
}

/// Only the three compare kinds carry a flag-only surface: a non-compare
/// kind at the named id, and each emitted-shape violation — a `Def`
/// operand, a fixed view, an early clobber, a nonzero operand position, an
/// implicit use, and an empty published surface — refuses.
#[test]
fn kind_and_shape_table_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The named instruction is not a compare at all.
    let source = fixture(target);
    assert_eq!(
        remove_dead_compare(&source, 0, LOAD, &environment, budget()).unwrap_err(),
        DeadCompareError::UnsupportedInstruction
    );
    // A `Def` operand rides along on the compare.
    let defined = mutated(target, |function, _| {
        function.blocks[0].instructions[2].operands[1].access = RegisterOperandAccess::Def;
    });
    assert_eq!(
        remove(&defined, &environment).unwrap_err(),
        DeadCompareError::UnsupportedInstruction
    );
    // A fixed view narrows the operand surface.
    let viewed = mutated(target, |function, _| {
        function.blocks[0].instructions[2].operands[0].fixed_view =
            Some(register_model::RegisterViewId(0));
    });
    assert_eq!(
        remove(&viewed, &environment).unwrap_err(),
        DeadCompareError::UnsupportedInstruction
    );
    // An operand position other than its canonical index.
    let displaced = mutated(target, |function, _| {
        function.blocks[0].instructions[2].operands[1].operand = 3;
    });
    assert_eq!(
        remove(&displaced, &environment).unwrap_err(),
        DeadCompareError::UnsupportedInstruction
    );
    // An implicit use the removal would silently drop.
    let implicit_read = mutated(target, |function, _| {
        let defs = function.blocks[0].instructions[2].implicit_defs.clone();
        function.blocks[0].instructions[2].implicit_uses = defs;
    });
    assert_eq!(
        remove(&implicit_read, &environment).unwrap_err(),
        DeadCompareError::UnsupportedInstruction
    );
    // A compare publishing no condition state at all is malformed, not dead.
    let surfaceless = mutated(target, |function, _| {
        function.blocks[0].instructions[2].implicit_defs = Vec::new();
        function.blocks[0].instructions[2].clobbers = Vec::new();
    });
    assert_eq!(
        remove(&surfaceless, &environment).unwrap_err(),
        DeadCompareError::UnsupportedInstruction
    );
}

/// A constraint row that disagrees with the instruction's shape — the wrong
/// arity, a `Def` operand where the compare reads, a mismatched register
/// class, or a different implicit surface — changes what removal means and
/// refuses.
#[test]
fn constraint_mismatches_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The copy row's second operand is a `Def`.
    let copy_row = environment
        .constraint(environment.selected_keys().copy_i64)
        .unwrap()
        .key;
    let wrong_row = mutated(target, |function, _| {
        function.blocks[0].instructions[2].constraint = copy_row;
    });
    assert_eq!(
        remove(&wrong_row, &environment).unwrap_err(),
        DeadCompareError::ConstraintMismatch
    );
    // A row key that resolves nowhere the environment knows.
    let unknown_row = mutated(target, |function, _| {
        function.blocks[0].instructions[2].constraint = register_model::RegisterConstraintKey {
            family: register_model::RegisterConstraintFamily::Instruction,
            variant: 65_000,
        };
    });
    assert_eq!(
        remove(&unknown_row, &environment).unwrap_err(),
        DeadCompareError::ConstraintMismatch
    );
    // An operand register missing from the roster cannot class-check.
    let unrostered = mutated(target, |function, _| {
        function.blocks[0].instructions[2].operands[0].virtual_register = VirtualRegisterId(40);
    });
    assert_eq!(
        remove(&unrostered, &environment).unwrap_err(),
        DeadCompareError::ConstraintMismatch
    );
}

/// Roster rows that would orphan the removed instruction — a call contract
/// or a memory-access row naming it — refuse.
#[test]
fn instruction_surface_rows_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let memory_row = mutated(target, |function, _| {
        function.memory_accesses.push(SelectedMemoryAccess {
            instruction: COMPARE,
            origin: SelectedMemoryAccessOrigin::Operation(OperationId::new(11).unwrap()),
            place: PlaceId::new(1).unwrap(),
            byte_offset: 0,
            byte_count: 8,
            role: SelectedMemoryAccessRole::ReadPlace,
        });
    });
    assert_eq!(
        remove(&memory_row, &environment).unwrap_err(),
        DeadCompareError::UnsupportedInstruction
    );
    let call_row = mutated(target, |function, _| {
        function.calls.push(SelectedCallContract {
            instruction: COMPARE,
            operation: OperationId::new(41).unwrap(),
            call: legalized_operations::LegalizedScalarCall {
                source: legalized_operations::NativeCallOrigin::Authored,
                callee: MachineId::new(42).unwrap(),
                call_plan: calling_conventions::CallPlan {
                    policy: calling_conventions::CallingPolicy::MicrosoftX64,
                    parameters: Vec::new(),
                    result: None,
                    callback_materializations: Vec::new(),
                    ordinary_clobbers: calling_conventions::RegisterSet::new(std::iter::empty()),
                    stack_alignment: 16,
                    shadow_bytes: 0,
                    entry_control: calling_conventions::EntryControl::CallReturn,
                },
                arguments: Vec::new(),
                result_placement: None,
                structural_result: None,
                claim_transfers: Vec::new(),
                requirement_obligations: vec![ObligationId::new(43).unwrap()],
                crash_continuations: vec![CrashRouteBucket {
                    cause: CrashCause::Trap,
                    alternatives: vec![CrashRouteGuard::Truth],
                }],
            },
            effect: EffectLink {
                input: 0,
                output: 0,
            },
            ownership: Vec::new(),
        });
    });
    assert_eq!(
        remove(&call_row, &environment).unwrap_err(),
        DeadCompareError::UnsupportedInstruction
    );
}

/// Boundary settlements positioned after the removed compare shift one
/// ordinal earlier; positions at or before it stay put.
#[test]
fn settlements_shift_over_the_removed_ordinal() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        // The compare sits at body index 2: position 2 settles before it and
        // stays; positions 3 (before the second compare) and 4 (after the
        // body) shift to 2 and 3.
        function.boundary_settlements =
            vec![settlement(2, 20), settlement(3, 21), settlement(4, 22)];
    });
    let result = remove(&source, &environment).unwrap();
    let positions: Vec<u32> = result.transformed().functions[0]
        .boundary_settlements
        .iter()
        .map(|settlement| settlement.instruction_index)
        .collect();
    assert_eq!(positions, vec![2, 2, 3]);
}

#[test]
fn source_identity_mismatches_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    assert_eq!(
        remove_dead_compare(&source, 1, COMPARE, &environment, budget()).unwrap_err(),
        DeadCompareError::SourceMismatch
    );
    assert_eq!(
        remove_dead_compare(
            &source,
            0,
            SelectedInstructionId(99),
            &environment,
            budget()
        )
        .unwrap_err(),
        DeadCompareError::SourceMismatch
    );
    let wrong_environment =
        baseline_target_register_environment(NativeTarget::macos_arm64()).unwrap();
    assert_eq!(
        remove_dead_compare(&source, 0, COMPARE, &wrong_environment, budget()).unwrap_err(),
        DeadCompareError::SourceMismatch
    );
}

#[test]
fn validation_budget_covers_the_dead_audit() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let tiny = OptimizationWorkBudget::new(100, 100, 4, 100, 100).unwrap();
    assert_eq!(
        remove_dead_compare(&source, 0, COMPARE, &environment, tiny).unwrap_err(),
        DeadCompareError::WorkBudgetExceeded
    );
}

/// The measured validation-step boundary: admission charges one step per
/// block plus one per instruction across the plan, repeats the scan over the
/// admitted function together with its register roster and call/access
/// rows, then charges the dead-unit audit at two block traversals plus the
/// successor edges per published flag unit — so the exact count admits the
/// removal on both the proposal and the independent replay path while one
/// step below rejects both, on the single-block fixture and on a second
/// whose audit crosses an edge into the killer's block.
#[test]
fn measured_validation_step_boundary_admits_and_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let jump_row = environment
        .constraint(environment.selected_keys().jump)
        .unwrap()
        .clone();
    let compare_row = environment
        .constraint(environment.selected_keys().compare_i64)
        .unwrap()
        .clone();
    let return_row = environment
        .constraint(environment.selected_keys().return_unit)
        .unwrap()
        .clone();
    // The killer moves into a jump target: the flag unit stays live across
    // the edge, so the audit's edge term is nonzero.
    let edge_crossing = mutated(target, |function, _| {
        function.blocks[0].instructions.truncate(3);
        function.blocks[0].terminator = SelectedTerminator::Jump {
            instruction: instruction(TERMINAL, SelectedInstructionKind::Jump, &jump_row, &[]),
            successor: successor(1, 2),
        };
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(1),
            origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
            instructions: vec![instruction(
                SECOND,
                SelectedInstructionKind::CompareI64,
                &compare_row,
                &[OTHER, INPUT],
            )],
            terminator: SelectedTerminator::Return {
                instruction: instruction(
                    SelectedInstructionId(10),
                    SelectedInstructionKind::ReturnUnit,
                    &return_row,
                    &[],
                ),
                psi_return_edge: EdgeId::new(3).unwrap(),
            },
        });
    });
    for source in [
        // One block carrying four body instructions plus the terminator; the
        // flag unit dies at the second compare, so no edge is crossed.
        fixture(target),
        // Three body instructions plus the jump in the compare's block and
        // one body instruction plus the return in the target; the flag unit
        // reaches the killer across the jump's one successor edge.
        edge_crossing,
    ] {
        let function = &source.transformed().functions[0];
        let plan_scan: u64 = source
            .transformed()
            .functions
            .iter()
            .flat_map(|function| function.blocks.iter())
            .map(|block| block.instructions.len() as u64 + 1)
            .sum();
        let function_scan: u64 = function
            .blocks
            .iter()
            .map(|block| block.instructions.len() as u64 + 1)
            .sum();
        let edge_count: u64 = function
            .blocks
            .iter()
            .map(|block| match &block.terminator {
                SelectedTerminator::Jump { .. } => 1u64,
                SelectedTerminator::ConditionalBranch { .. }
                | SelectedTerminator::ConditionalBranchU64LessThan { .. }
                | SelectedTerminator::ConditionalBranchI64LessThan { .. } => 2u64,
                SelectedTerminator::HostedExitProcess { .. }
                | SelectedTerminator::Return { .. } => 0u64,
            })
            .sum();
        let compare = function
            .blocks
            .iter()
            .flat_map(|block| block.instructions.iter())
            .find(|instruction| instruction.id == COMPARE)
            .unwrap();
        let units = compare
            .implicit_defs
            .iter()
            .chain(compare.clobbers.iter())
            .collect::<std::collections::BTreeSet<_>>()
            .len() as u64;
        let registers = function.virtual_registers.len() as u64;
        let calls = function.calls.len() as u64;
        let accesses = function.memory_accesses.len() as u64;
        let exact_steps = plan_scan
            + function_scan
            + registers
            + calls
            + accesses
            + units * (2 * function_scan + edge_count);
        let exact = OptimizationWorkBudget::new(1, 1, exact_steps, 1, 1).unwrap();
        let result = remove_dead_compare(&source, 0, COMPARE, &environment, exact).unwrap();
        validate_dead_compare(
            &source,
            0,
            COMPARE,
            &environment,
            exact,
            result.transformed().clone(),
        )
        .unwrap();
        let starved = OptimizationWorkBudget::new(1, 1, exact_steps - 1, 1, 1).unwrap();
        assert_eq!(
            remove_dead_compare(&source, 0, COMPARE, &environment, starved).unwrap_err(),
            DeadCompareError::WorkBudgetExceeded
        );
        assert_eq!(
            validate_dead_compare(
                &source,
                0,
                COMPARE,
                &environment,
                starved,
                result.transformed().clone(),
            )
            .unwrap_err(),
            DeadCompareError::WorkBudgetExceeded
        );
    }
}

/// Replay refuses anything but the exact removal: a proposal that keeps the
/// compare, that drops the wrong instruction, that leaves the settlements
/// unshifted, or that drifts in a block the removal never touched all
/// mismatch.
#[test]
fn replay_rejects_anything_but_the_removal() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // The proposal still carries the compare.
    assert_eq!(
        validate_dead_compare(
            &source,
            0,
            COMPARE,
            &environment,
            budget(),
            source.transformed().clone(),
        )
        .unwrap_err(),
        DeadCompareError::ReplayMismatch
    );
    let result = remove(&source, &environment).unwrap();
    // The wrong instruction was removed instead.
    let mut wrong_member = result.transformed().clone();
    wrong_member.functions[0].blocks[0].instructions[2].id = SelectedInstructionId(90);
    assert_eq!(
        validate_dead_compare(&source, 0, COMPARE, &environment, budget(), wrong_member)
            .unwrap_err(),
        DeadCompareError::ReplayMismatch
    );
    // Drift in an instruction the removal never touched.
    let mut drifted = result.transformed().clone();
    drifted.functions[0].blocks[0].instructions[0].kind =
        SelectedInstructionKind::Load8 { byte_offset: 8 };
    assert_eq!(
        validate_dead_compare(&source, 0, COMPARE, &environment, budget(), drifted).unwrap_err(),
        DeadCompareError::ReplayMismatch
    );
    // A settlement the shift did not apply.
    let mut unshifted = result.transformed().clone();
    unshifted.functions[0]
        .boundary_settlements
        .push(settlement(4, 30));
    assert_eq!(
        validate_dead_compare(&source, 0, COMPARE, &environment, budget(), unshifted).unwrap_err(),
        DeadCompareError::ReplayMismatch
    );
    // Drift outside the function under rewrite.
    let mut foreign = result.transformed().clone();
    foreign.functions[0].virtual_registers[0].scalar_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap());
    assert_eq!(
        validate_dead_compare(&source, 0, COMPARE, &environment, budget(), foreign).unwrap_err(),
        DeadCompareError::ReplayMismatch
    );
}

/// The removal is deterministic — two runs publish identical plans — and
/// terminal: the published artifact feeds back through the sealed analysis
/// boundary as a legal second input, re-admission at the removed compare's
/// site refuses, and the surviving second compare — itself dead behind the
/// return — still admits through the published plan.
#[test]
fn removal_is_deterministic_and_terminal() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let first = remove(&source, &environment).unwrap();
    let second = remove(&source, &environment).unwrap();
    assert_eq!(first.transformed(), second.transformed());
    // Re-admission at the removed compare's id refuses: it no longer exists.
    assert_eq!(
        remove_dead_compare(&first, 0, COMPARE, &environment, budget()).unwrap_err(),
        DeadCompareError::SourceMismatch
    );
    // The second compare's flags die at the return: it removes from the
    // published plan through the same boundary.
    let chained = remove_dead_compare(&first, 0, SECOND, &environment, budget()).unwrap();
    let block = &chained.transformed().functions[0].blocks[0];
    assert_eq!(block.instructions.len(), 2);
    assert!(
        block
            .instructions
            .iter()
            .all(|entry| entry.id != COMPARE && entry.id != SECOND)
    );
}
