use crate::DeadCompareError;
use crate::RedundantCompareError;
use crate::RedundantCompareReceipt;
use crate::ValidatedRedundantCompare;
use crate::ValidatedSelectedAnalysis;
use crate::remove_dead_compare;
use crate::remove_redundant_compare;
use crate::validate_redundant_compare;
use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget};
use optimization_unit::{EffectLink, ValueDefinitionSite};
use register_environment::baseline_target_register_environment;
use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedBoundarySettlement,
    SelectedBoundarySettlementPayload, SelectedCallContract, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan, SelectedMemoryAccess,
    SelectedMemoryAccessOrigin, SelectedMemoryAccessRole, SelectedOperand, SelectedSuccessor,
    SelectedSuccessorRole, SelectedTerminator, SelectedValueBinding, SelectedValueTransport,
    VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
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
const REDUNDANT: SelectedInstructionId = SelectedInstructionId(5);
const THIRD_LOAD: SelectedInstructionId = SelectedInstructionId(6);
const TERMINAL: SelectedInstructionId = SelectedInstructionId(7);
const BOOLEAN: SelectedInstructionId = SelectedInstructionId(8);
const MOVED: SelectedInstructionId = SelectedInstructionId(9);

const POINTER: VirtualRegisterId = VirtualRegisterId(0);
const INPUT: VirtualRegisterId = VirtualRegisterId(1);
const OTHER: VirtualRegisterId = VirtualRegisterId(2);
const OUTPUT: VirtualRegisterId = VirtualRegisterId(3);
const FRESH: VirtualRegisterId = VirtualRegisterId(4);

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

/// A raw selected-stage unit fixture, not a source/Terminal admission claim:
/// `r1 = load8 r0; r2 = load8 r0; compare r1, r2; compare r1, r2; return`.
/// The second compare republishes flag state identical to what the first
/// left behind, so removing it changes no observable condition state —
/// whether or not a reader still observes the flags.
fn fixture(target: NativeTarget) -> ValidatedRedundantCompare {
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
                    instruction(
                        COMPARE,
                        SelectedInstructionKind::CompareI64,
                        compare,
                        &[INPUT, OTHER],
                    ),
                    instruction(
                        REDUNDANT,
                        SelectedInstructionKind::CompareI64,
                        compare,
                        &[INPUT, OTHER],
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
    ValidatedRedundantCompare {
        receipt: RedundantCompareReceipt {
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
) -> ValidatedRedundantCompare {
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
    source: &ValidatedRedundantCompare,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> Result<ValidatedRedundantCompare, RedundantCompareError> {
    remove_redundant_compare(source, 0, REDUNDANT, environment, budget())
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

/// A compare shadowed by a flag-equivalent earlier compare in its own block
/// is removed on every target, and the result replays by content.
#[test]
fn shadowed_flags_remove_on_every_target() {
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
        assert_eq!(block.instructions[2].id, COMPARE);
        assert_eq!(
            result.receipt().source_selected(),
            source.selected_identity()
        );
        assert_eq!(
            result.receipt().transformed_selected(),
            selected_instruction_plan_identity(result.transformed())
        );
        validate_redundant_compare(
            &source,
            0,
            REDUNDANT,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
        // A detached, separately allocated proposal replays by content.
        let mut detached = result.transformed().clone();
        detached.functions = detached.functions.iter().cloned().collect();
        validate_redundant_compare(&source, 0, REDUNDANT, &environment, budget(), detached)
            .unwrap();
    }
}

/// The immediate and zero compare forms admit the same way: the shadow must
/// carry the identical kind — immediate payload included — and operands.
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
            function.blocks[0].instructions[3] = instruction(REDUNDANT, kind, &row, &[INPUT]);
        });
        let result = remove(&source, &environment).unwrap();
        let block = &result.transformed().functions[0].blocks[0];
        assert_eq!(block.instructions.len(), 3);
        assert!(block.instructions.iter().all(|entry| entry.id != REDUNDANT));
    }
}

/// The family's reason to exist: a flag reader after the redundant compare
/// does not block removal — the reader observes the shadow's identical
/// publication. The dead family refuses this same plan.
#[test]
fn flag_readers_do_not_block() {
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
        function.blocks[0].instructions.push(instruction(
            BOOLEAN,
            SelectedInstructionKind::MaterializeBooleanEqual,
            &boolean_row,
            &[OUTPUT],
        ));
    });
    let result = remove(&source, &environment).unwrap();
    let ids: Vec<SelectedInstructionId> = result.transformed().functions[0].blocks[0]
        .instructions
        .iter()
        .map(|instruction| instruction.id)
        .collect();
    assert_eq!(ids, vec![LOAD, SECOND_LOAD, COMPARE, BOOLEAN]);
    // The dead family must refuse the same removal: the reader observes the
    // redundant compare's publication — the families answer different
    // questions about the same surface.
    assert_eq!(
        remove_dead_compare(&source, 0, REDUNDANT, &environment, budget()).unwrap_err(),
        DeadCompareError::UnsupportedUse
    );
}

/// A write to either shared operand register between the shadow and the
/// compare refuses: the compare may compute flags from a different value
/// than the shadow observed.
#[test]
fn operand_rewritten_between_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let copy_row = environment
        .constraint(environment.selected_keys().copy_i64)
        .unwrap()
        .clone();
    for (source_register, written) in [(OTHER, INPUT), (INPUT, OTHER)] {
        let copy_row = copy_row.clone();
        let source = mutated(target, move |function, _| {
            function.blocks[0].instructions.insert(
                3,
                instruction(
                    MOVED,
                    SelectedInstructionKind::CopyI64,
                    &copy_row,
                    &[source_register, written],
                ),
            );
        });
        assert_eq!(
            remove(&source, &environment).unwrap_err(),
            RedundantCompareError::UnsupportedUse
        );
    }
}

/// An instruction between the shadow and the compare that writes no shared
/// operand register and publishes no flag unit does not disturb the
/// equivalence.
#[test]
fn inert_instruction_between_admits() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap()
            .clone();
        let class = function.virtual_registers[1].class;
        function.virtual_registers.push(register(
            FRESH,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: THIRD_LOAD,
                source_value: ValueId::new(7).unwrap(),
            },
        ));
        function.blocks[0].instructions.insert(
            3,
            instruction(
                THIRD_LOAD,
                SelectedInstructionKind::Load8 { byte_offset: 0 },
                &load,
                &[POINTER, FRESH],
            ),
        );
    });
    let result = remove(&source, &environment).unwrap();
    let ids: Vec<SelectedInstructionId> = result.transformed().functions[0].blocks[0]
        .instructions
        .iter()
        .map(|instruction| instruction.id)
        .collect();
    assert_eq!(ids, vec![LOAD, SECOND_LOAD, COMPARE, THIRD_LOAD]);
}

/// A nearer identical compare is the shadow the reaching scan resolves:
/// the last in-block flag event wins and the removal still admits.
#[test]
fn nearest_shadow_wins() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let compare = environment
            .constraint(environment.selected_keys().compare_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions.insert(
            3,
            instruction(
                THIRD_LOAD,
                SelectedInstructionKind::CompareI64,
                &compare,
                &[INPUT, OTHER],
            ),
        );
    });
    let result = remove(&source, &environment).unwrap();
    let ids: Vec<SelectedInstructionId> = result.transformed().functions[0].blocks[0]
        .instructions
        .iter()
        .map(|instruction| instruction.id)
        .collect();
    assert_eq!(ids, vec![LOAD, SECOND_LOAD, COMPARE, THIRD_LOAD]);
}

/// A different flag event nearer the compare is the resolved shadow; when
/// it is not flag-equivalent — here a swapped operand pair, a different
/// subtraction — admission refuses.
#[test]
fn intervening_unequal_flag_event_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let compare = environment
            .constraint(environment.selected_keys().compare_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions.insert(
            3,
            instruction(
                THIRD_LOAD,
                SelectedInstructionKind::CompareI64,
                &compare,
                &[OTHER, INPUT],
            ),
        );
    });
    assert_eq!(
        remove(&source, &environment).unwrap_err(),
        RedundantCompareError::UnsupportedUse
    );
}

/// The same kind with swapped operand registers is not flag-equivalent:
/// operand positions are the subtraction's order.
#[test]
fn swapped_operands_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let compare = environment
            .constraint(environment.selected_keys().compare_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[3] = instruction(
            REDUNDANT,
            SelectedInstructionKind::CompareI64,
            &compare,
            &[OTHER, INPUT],
        );
    });
    assert_eq!(
        remove(&source, &environment).unwrap_err(),
        RedundantCompareError::UnsupportedUse
    );
}

/// A different compare kind — even over the same operand registers — is
/// not flag-equivalent, and neither is a different immediate payload.
#[test]
fn different_kind_or_immediate_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let immediate_row = environment
        .constraint(environment.selected_keys().compare_i64_immediate)
        .unwrap()
        .clone();
    // Different immediate payload against an immediate shadow.
    let source = mutated(target, |function, _| {
        function.blocks[0].instructions[2] = instruction(
            COMPARE,
            SelectedInstructionKind::CompareI64Immediate {
                immediate: IntegerValue::Unsigned(7),
            },
            &immediate_row,
            &[INPUT],
        );
        function.blocks[0].instructions[3] = instruction(
            REDUNDANT,
            SelectedInstructionKind::CompareI64Immediate {
                immediate: IntegerValue::Unsigned(9),
            },
            &immediate_row,
            &[INPUT],
        );
    });
    assert_eq!(
        remove(&source, &environment).unwrap_err(),
        RedundantCompareError::UnsupportedUse
    );
    // A different kind over the same operand register against a register
    // shadow.
    let zero_row = environment
        .constraint(environment.selected_keys().compare_i64_zero)
        .unwrap()
        .clone();
    let source = mutated(target, |function, _| {
        function.blocks[0].instructions[3] = instruction(
            REDUNDANT,
            SelectedInstructionKind::CompareI64Zero,
            &zero_row,
            &[INPUT],
        );
    });
    assert_eq!(
        remove(&source, &environment).unwrap_err(),
        RedundantCompareError::UnsupportedUse
    );
}

/// A shadow that clobbers a unit the compare defines is not equivalent:
/// removing the compare would leave readers an unspecified unit where the
/// compare published real flags.
#[test]
fn role_mismatch_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        let shadow = &mut function.blocks[0].instructions[2];
        shadow.clobbers = shadow.implicit_defs.clone();
        shadow.implicit_defs = Vec::new();
    });
    assert_eq!(
        remove(&source, &environment).unwrap_err(),
        RedundantCompareError::UnsupportedUse
    );
}

/// A block that can reach itself still admits when the shadow precedes
/// the compare in the same body: a re-entering traversal's last flag
/// event is that traversal's own shadow execution, so the in-block
/// equivalence describes every arrival.
#[test]
fn cyclic_self_loop_admits() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        // The compare's block jumps back to itself: on every traversal the
        // shadow still executes immediately before the redundant compare.
        function.blocks[0].terminator = SelectedTerminator::Jump {
            instruction: instruction(TERMINAL, SelectedInstructionKind::Jump, &jump_row, &[]),
            successor: successor(0, 2),
        };
    });
    let result = remove(&source, &environment).unwrap();
    let ids: Vec<SelectedInstructionId> = result.transformed().functions[0].blocks[0]
        .instructions
        .iter()
        .map(|instruction| instruction.id)
        .collect();
    assert_eq!(ids, vec![LOAD, SECOND_LOAD, COMPARE]);
    validate_redundant_compare(
        &source,
        0,
        REDUNDANT,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
}

/// A block reachable from itself through a second block is the same
/// admission: the cycle does not have to be a self-edge.
#[test]
fn two_block_cycle_admits() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
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
    let ids: Vec<SelectedInstructionId> = result.transformed().functions[0].blocks[0]
        .instructions
        .iter()
        .map(|instruction| instruction.id)
        .collect();
    assert_eq!(ids, vec![LOAD, SECOND_LOAD, COMPARE]);
}

/// A loop tail that rewrites an operand register between the shadow and
/// the next traversal's compare is the cyclic refusal: the redundant
/// compare heads a self-looping block, the entry block's compare shadows
/// the first arrival and the in-loop shadow the later ones, and the
/// exposed interval crossing the back edge holds the rewrite.
#[test]
fn cyclic_tail_rewrite_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        let copy_row = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        let compare_row = environment
            .constraint(environment.selected_keys().compare_i64)
            .unwrap()
            .clone();
        let redundant = function.blocks[0].instructions.remove(3);
        function.blocks[0].terminator = SelectedTerminator::Jump {
            instruction: instruction(TERMINAL, SelectedInstructionKind::Jump, &jump_row, &[]),
            successor: successor(1, 2),
        };
        // Block 1: `[redundant, shadow, rewrite, jump->self]` — every
        // arrival's last flag event is an equivalent compare (the entry
        // block's shadow on the first traversal, the in-loop shadow on
        // later ones), but the loop tail rewrites an operand register
        // inside the exposed interval.
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(1),
            origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
            instructions: vec![
                redundant,
                instruction(
                    SelectedInstructionId(12),
                    SelectedInstructionKind::CompareI64,
                    &compare_row,
                    &[INPUT, OTHER],
                ),
                instruction(
                    MOVED,
                    SelectedInstructionKind::CopyI64,
                    &copy_row,
                    &[INPUT, OTHER],
                ),
            ],
            terminator: SelectedTerminator::Jump {
                instruction: instruction(
                    SelectedInstructionId(10),
                    SelectedInstructionKind::Jump,
                    &jump_row,
                    &[],
                ),
                successor: successor(1, 3),
            },
        });
    });
    assert_eq!(
        remove(&source, &environment).unwrap_err(),
        RedundantCompareError::UnsupportedUse
    );
}

/// The compare's own earlier execution is itself a valid reaching site:
/// the redundant compare heads a self-looping block whose tail writes
/// only an unrelated register, so the first arrival's reaching event is
/// the entry shadow and every later arrival's is the compare itself —
/// both flag-equivalent, and the exposed interval keeps the operands
/// stable.
#[test]
fn cyclic_self_site_admits() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        let load_row = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap()
            .clone();
        let class = function.virtual_registers[1].class;
        function.virtual_registers.push(register(
            FRESH,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: THIRD_LOAD,
                source_value: ValueId::new(7).unwrap(),
            },
        ));
        let redundant = function.blocks[0].instructions.remove(3);
        function.blocks[0].terminator = SelectedTerminator::Jump {
            instruction: instruction(TERMINAL, SelectedInstructionKind::Jump, &jump_row, &[]),
            successor: successor(1, 2),
        };
        // Block 1: `[redundant, unrelated load, jump->self]` — arrival 1's
        // last flag event is the entry block's shadow; every later
        // arrival's is the redundant compare's own previous execution.
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(1),
            origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
            instructions: vec![
                redundant,
                instruction(
                    THIRD_LOAD,
                    SelectedInstructionKind::Load8 { byte_offset: 0 },
                    &load_row,
                    &[POINTER, FRESH],
                ),
            ],
            terminator: SelectedTerminator::Jump {
                instruction: instruction(
                    SelectedInstructionId(10),
                    SelectedInstructionKind::Jump,
                    &jump_row,
                    &[],
                ),
                successor: successor(1, 3),
            },
        });
    });
    let result = remove(&source, &environment).unwrap();
    let ids: Vec<SelectedInstructionId> = result.transformed().functions[0].blocks[1]
        .instructions
        .iter()
        .map(|instruction| instruction.id)
        .collect();
    assert_eq!(ids, vec![THIRD_LOAD]);
    validate_redundant_compare(
        &source,
        0,
        REDUNDANT,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
}

/// An equivalent compare in a predecessor block shadows across the edge:
/// every path into the redundant compare's block last observed the
/// shadow's publication, so the removal replays clean.
#[test]
fn cross_block_shadow_admits() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let return_row = environment
        .constraint(environment.selected_keys().return_unit)
        .unwrap()
        .clone();
    let source = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        let redundant = function.blocks[0].instructions.remove(3);
        function.blocks[0].terminator = SelectedTerminator::Jump {
            instruction: instruction(TERMINAL, SelectedInstructionKind::Jump, &jump_row, &[]),
            successor: successor(1, 2),
        };
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(1),
            origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
            instructions: vec![redundant],
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
    assert!(
        result.transformed().functions[0].blocks[1]
            .instructions
            .is_empty()
    );
    validate_redundant_compare(
        &source,
        0,
        REDUNDANT,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
}

/// A write to a shared operand register between the cross-block shadow
/// and the compare refuses: the exposed interval crosses the edge, so the
/// compare may compute flags from a different value.
#[test]
fn cross_block_operand_rewrite_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let return_row = environment
        .constraint(environment.selected_keys().return_unit)
        .unwrap()
        .clone();
    let source = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        let copy_row = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        let redundant = function.blocks[0].instructions.remove(3);
        function.blocks[0].instructions.push(instruction(
            MOVED,
            SelectedInstructionKind::CopyI64,
            &copy_row,
            &[INPUT, OTHER],
        ));
        function.blocks[0].terminator = SelectedTerminator::Jump {
            instruction: instruction(TERMINAL, SelectedInstructionKind::Jump, &jump_row, &[]),
            successor: successor(1, 2),
        };
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(1),
            origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
            instructions: vec![redundant],
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
        RedundantCompareError::UnsupportedUse
    );
}

/// An edge transport defining a shared operand register on the crossed
/// edge refuses: the compare would read the transported value, not the
/// one the shadow computed flags from.
#[test]
fn cross_block_edge_parameter_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let return_row = environment
        .constraint(environment.selected_keys().return_unit)
        .unwrap()
        .clone();
    let source = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        let redundant = function.blocks[0].instructions.remove(3);
        let mut edge = successor(1, 2);
        edge.bindings.push(SelectedValueBinding {
            semantic: abstract_operations::ValueBinding {
                parameter: ValueId::new(8).unwrap(),
                argument: ValueId::new(3).unwrap(),
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                ),
            },
            transport: SelectedValueTransport::Registers {
                argument: OTHER,
                parameter: INPUT,
            },
        });
        function.blocks[0].terminator = SelectedTerminator::Jump {
            instruction: instruction(TERMINAL, SelectedInstructionKind::Jump, &jump_row, &[]),
            successor: edge,
        };
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(1),
            origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
            instructions: vec![redundant],
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
        RedundantCompareError::UnsupportedUse
    );
}

/// A diamond whose arms each republish the same compare admits: the
/// reaching set resolves to one equivalent site per arm and no position
/// between either and the join's compare writes a shared operand.
#[test]
fn diamond_shadows_admit() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let keys = environment.selected_keys();
        let branch_row = environment
            .constraint(keys.conditional_branch)
            .unwrap()
            .clone();
        let jump_row = environment.constraint(keys.jump).unwrap().clone();
        let return_row = environment.constraint(keys.return_unit).unwrap().clone();
        let redundant = function.blocks[0].instructions.remove(3);
        let shadow = function.blocks[0].instructions.remove(2);
        function.blocks[0].terminator = SelectedTerminator::ConditionalBranch {
            instruction: instruction(
                SelectedInstructionId(11),
                SelectedInstructionKind::ConditionalBranchNonZero,
                &branch_row,
                &[],
            ),
            when_nonzero: successor(1, 2),
            when_zero: successor(2, 3),
        };
        for (block, edge) in [(1u32, 4u64), (2u32, 5u64)] {
            let mut arm_shadow = shadow.clone();
            arm_shadow.id = SelectedInstructionId(11 + block);
            function.blocks.push(SelectedBlock {
                id: SelectedBlockId(block),
                origin: SelectedBlockOrigin::Source(BlockId::new(u64::from(block) + 1).unwrap()),
                instructions: vec![arm_shadow],
                terminator: SelectedTerminator::Jump {
                    instruction: instruction(
                        SelectedInstructionId(20 + block),
                        SelectedInstructionKind::Jump,
                        &jump_row,
                        &[],
                    ),
                    successor: successor(3, edge),
                },
            });
        }
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(3),
            origin: SelectedBlockOrigin::Source(BlockId::new(4).unwrap()),
            instructions: vec![redundant],
            terminator: SelectedTerminator::Return {
                instruction: instruction(
                    SelectedInstructionId(30),
                    SelectedInstructionKind::ReturnUnit,
                    &return_row,
                    &[],
                ),
                psi_return_edge: EdgeId::new(6).unwrap(),
            },
        });
    });
    let result = remove(&source, &environment).unwrap();
    assert!(
        result.transformed().functions[0].blocks[3]
            .instructions
            .is_empty()
    );
    validate_redundant_compare(
        &source,
        0,
        REDUNDANT,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
}

/// One divergent arm refuses: the leg whose last event is a compare with
/// swapped operand registers publishes a different subtraction, so the
/// reaching set does not resolve to flag-equivalent sites.
#[test]
fn divergent_leg_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let keys = environment.selected_keys();
        let compare_row = environment.constraint(keys.compare_i64).unwrap().clone();
        let branch_row = environment
            .constraint(keys.conditional_branch)
            .unwrap()
            .clone();
        let jump_row = environment.constraint(keys.jump).unwrap().clone();
        let return_row = environment.constraint(keys.return_unit).unwrap().clone();
        let redundant = function.blocks[0].instructions.remove(3);
        let shadow = function.blocks[0].instructions.remove(2);
        function.blocks[0].terminator = SelectedTerminator::ConditionalBranch {
            instruction: instruction(
                SelectedInstructionId(11),
                SelectedInstructionKind::ConditionalBranchNonZero,
                &branch_row,
                &[],
            ),
            when_nonzero: successor(1, 2),
            when_zero: successor(2, 3),
        };
        let swapped = instruction(
            SelectedInstructionId(13),
            SelectedInstructionKind::CompareI64,
            &compare_row,
            &[OTHER, INPUT],
        );
        for (block, edge, arm_shadow) in [(1u32, 4u64, shadow), (2u32, 5u64, swapped)] {
            let mut arm_shadow = arm_shadow.clone();
            arm_shadow.id = SelectedInstructionId(11 + block);
            function.blocks.push(SelectedBlock {
                id: SelectedBlockId(block),
                origin: SelectedBlockOrigin::Source(BlockId::new(u64::from(block) + 1).unwrap()),
                instructions: vec![arm_shadow],
                terminator: SelectedTerminator::Jump {
                    instruction: instruction(
                        SelectedInstructionId(20 + block),
                        SelectedInstructionKind::Jump,
                        &jump_row,
                        &[],
                    ),
                    successor: successor(3, edge),
                },
            });
        }
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(3),
            origin: SelectedBlockOrigin::Source(BlockId::new(4).unwrap()),
            instructions: vec![redundant],
            terminator: SelectedTerminator::Return {
                instruction: instruction(
                    SelectedInstructionId(30),
                    SelectedInstructionKind::ReturnUnit,
                    &return_row,
                    &[],
                ),
                psi_return_edge: EdgeId::new(6).unwrap(),
            },
        });
    });
    assert_eq!(
        remove(&source, &environment).unwrap_err(),
        RedundantCompareError::UnsupportedUse
    );
}

/// Without an earlier in-block flag event there is no shadow at all.
#[test]
fn no_shadow_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        function.blocks[0].instructions.remove(2);
    });
    assert_eq!(
        remove(&source, &environment).unwrap_err(),
        RedundantCompareError::UnsupportedUse
    );
}

/// Boundary settlements positioned after the removed compare shift one
/// ordinal earlier; positions at or before it stay put.
#[test]
fn settlements_shift_over_the_removed_ordinal() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        // The redundant compare sits at body index 3: position 2 settles
        // before it and stays; position 3 settles at it and stays; position
        // 4 — after the body — shifts one ordinal earlier to 3.
        function.boundary_settlements =
            vec![settlement(2, 20), settlement(3, 21), settlement(4, 22)];
    });
    let result = remove(&source, &environment).unwrap();
    let positions: Vec<u32> = result.transformed().functions[0]
        .boundary_settlements
        .iter()
        .map(|settlement| settlement.instruction_index)
        .collect();
    assert_eq!(positions, vec![2, 3, 3]);
}

#[test]
fn source_identity_mismatches_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    assert_eq!(
        remove_redundant_compare(&source, 1, REDUNDANT, &environment, budget()).unwrap_err(),
        RedundantCompareError::SourceMismatch
    );
    assert_eq!(
        remove_redundant_compare(
            &source,
            0,
            SelectedInstructionId(99),
            &environment,
            budget()
        )
        .unwrap_err(),
        RedundantCompareError::SourceMismatch
    );
    let wrong_environment =
        baseline_target_register_environment(NativeTarget::macos_arm64()).unwrap();
    assert_eq!(
        remove_redundant_compare(&source, 0, REDUNDANT, &wrong_environment, budget()).unwrap_err(),
        RedundantCompareError::SourceMismatch
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
        remove_redundant_compare(&source, 0, LOAD, &environment, budget()).unwrap_err(),
        RedundantCompareError::UnsupportedInstruction
    );
    // A `Def` operand rides along on the compare.
    let defined = mutated(target, |function, _| {
        function.blocks[0].instructions[3].operands[1].access = RegisterOperandAccess::Def;
    });
    assert_eq!(
        remove(&defined, &environment).unwrap_err(),
        RedundantCompareError::UnsupportedInstruction
    );
    // A fixed view narrows the operand surface.
    let viewed = mutated(target, |function, _| {
        function.blocks[0].instructions[3].operands[0].fixed_view =
            Some(register_model::RegisterViewId(0));
    });
    assert_eq!(
        remove(&viewed, &environment).unwrap_err(),
        RedundantCompareError::UnsupportedInstruction
    );
    // An operand position other than its canonical index.
    let displaced = mutated(target, |function, _| {
        function.blocks[0].instructions[3].operands[1].operand = 3;
    });
    assert_eq!(
        remove(&displaced, &environment).unwrap_err(),
        RedundantCompareError::UnsupportedInstruction
    );
    // An implicit use the removal would silently drop.
    let implicit_read = mutated(target, |function, _| {
        let defs = function.blocks[0].instructions[3].implicit_defs.clone();
        function.blocks[0].instructions[3].implicit_uses = defs;
    });
    assert_eq!(
        remove(&implicit_read, &environment).unwrap_err(),
        RedundantCompareError::UnsupportedInstruction
    );
    // A compare publishing no condition state at all is malformed.
    let surfaceless = mutated(target, |function, _| {
        function.blocks[0].instructions[3].implicit_defs = Vec::new();
        function.blocks[0].instructions[3].clobbers = Vec::new();
    });
    assert_eq!(
        remove(&surfaceless, &environment).unwrap_err(),
        RedundantCompareError::UnsupportedInstruction
    );
}

/// A constraint row that disagrees with the instruction's shape — the wrong
/// row, an unknown key, or an operand the roster does not list — changes
/// what removal means and refuses before any admission reasoning.
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
        function.blocks[0].instructions[3].constraint = copy_row;
    });
    assert_eq!(
        remove(&wrong_row, &environment).unwrap_err(),
        RedundantCompareError::ConstraintMismatch
    );
    // A row key that resolves nowhere the environment knows.
    let unknown_row = mutated(target, |function, _| {
        function.blocks[0].instructions[3].constraint = register_model::RegisterConstraintKey {
            family: register_model::RegisterConstraintFamily::Instruction,
            variant: 65_000,
        };
    });
    assert_eq!(
        remove(&unknown_row, &environment).unwrap_err(),
        RedundantCompareError::ConstraintMismatch
    );
    // An operand register missing from the roster cannot class-check.
    let unrostered = mutated(target, |function, _| {
        function.blocks[0].instructions[3].operands[0].virtual_register = VirtualRegisterId(40);
    });
    assert_eq!(
        remove(&unrostered, &environment).unwrap_err(),
        RedundantCompareError::ConstraintMismatch
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
            instruction: REDUNDANT,
            origin: SelectedMemoryAccessOrigin::Operation(OperationId::new(11).unwrap()),
            place: PlaceId::new(1).unwrap(),
            byte_offset: 0,
            byte_count: 8,
            role: SelectedMemoryAccessRole::ReadPlace,
        });
    });
    assert_eq!(
        remove(&memory_row, &environment).unwrap_err(),
        RedundantCompareError::UnsupportedInstruction
    );
    let call_row = mutated(target, |function, _| {
        function.calls.push(SelectedCallContract {
            instruction: REDUNDANT,
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
        RedundantCompareError::UnsupportedInstruction
    );
}

/// The measured validation-step boundary: admission charges one step per
/// block plus one per instruction across the plan, repeats the scan over
/// the admitted function together with its register roster and call/access
/// rows, then charges the reaching walk's setup — every edge's target
/// resolution, the predecessor fill, and the cone mark — plus per
/// published unit the in-block prefix scan, the per-block last-event
/// table, and the bounded fixpoint propagation, and finally the backward
/// operand audit once per unit over stream positions and crossed edge
/// surfaces — so the exact count admits the removal on both the proposal
/// and the independent replay path while one step below rejects both, on
/// the single-block fixture and on a second whose acyclic block list
/// carries a nonzero edge count.
#[test]
fn measured_validation_step_boundary_admits_and_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A second, acyclic two-block form: the compare's block is a jump's
    // target, so the walk's setup resolves one real edge and the operand
    // audit crosses it backward.
    let two_block = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        let tail = function.blocks[0].instructions.split_off(1);
        function.blocks[0].terminator = SelectedTerminator::Jump {
            instruction: instruction(TERMINAL, SelectedInstructionKind::Jump, &jump_row, &[]),
            successor: successor(1, 2),
        };
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(1),
            origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
            instructions: tail,
            terminator: SelectedTerminator::Return {
                instruction: instruction(
                    SelectedInstructionId(10),
                    SelectedInstructionKind::ReturnUnit,
                    &environment
                        .constraint(environment.selected_keys().return_unit)
                        .unwrap()
                        .clone(),
                    &[],
                ),
                psi_return_edge: EdgeId::new(3).unwrap(),
            },
        });
    });
    for source in [fixture(target), two_block] {
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
        let block_count = function.blocks.len() as u64;
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
        let widest_out: u64 = function
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
            .max()
            .unwrap_or(0);
        let transport_rows: u64 = function
            .blocks
            .iter()
            .flat_map(|block| match &block.terminator {
                SelectedTerminator::Jump { successor, .. } => vec![successor],
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
                SelectedTerminator::HostedExitProcess { .. }
                | SelectedTerminator::Return { .. } => Vec::new(),
            })
            .map(|successor| {
                (successor.bindings.len()
                    + successor.structural_bindings.len()
                    + successor
                        .structural_case
                        .as_ref()
                        .map_or(0, |case| case.payloads.len())) as u64
            })
            .sum();
        let compare = function
            .blocks
            .iter()
            .flat_map(|block| block.instructions.iter())
            .find(|instruction| instruction.id == REDUNDANT)
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
        let elements = function_scan + 1;
        let pops = block_count * (elements + 1);
        let per_unit = function_scan + block_count + pops + pops * widest_out * elements;
        let walk_setup = edge_count * (block_count + 1) + block_count + edge_count;
        let audit = units * (function_scan + edge_count + transport_rows);
        let exact_steps = plan_scan
            + function_scan
            + registers
            + calls
            + accesses
            + walk_setup
            + units * per_unit
            + audit;
        let exact = OptimizationWorkBudget::new(1, 1, exact_steps, 1, 1).unwrap();
        let result = remove_redundant_compare(&source, 0, REDUNDANT, &environment, exact).unwrap();
        validate_redundant_compare(
            &source,
            0,
            REDUNDANT,
            &environment,
            exact,
            result.transformed().clone(),
        )
        .unwrap();
        let starved = OptimizationWorkBudget::new(1, 1, exact_steps - 1, 1, 1).unwrap();
        assert_eq!(
            remove_redundant_compare(&source, 0, REDUNDANT, &environment, starved).unwrap_err(),
            RedundantCompareError::WorkBudgetExceeded
        );
        assert_eq!(
            validate_redundant_compare(
                &source,
                0,
                REDUNDANT,
                &environment,
                starved,
                result.transformed().clone(),
            )
            .unwrap_err(),
            RedundantCompareError::WorkBudgetExceeded
        );
    }
}

/// Replay refuses anything but the exact removal: a proposal that keeps the
/// compare, that drifts an instruction the removal never touched, that
/// leaves the settlements unshifted, or that drifts elsewhere in the plan
/// all mismatch.
#[test]
fn replay_rejects_anything_but_the_removal() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // The proposal still carries the compare.
    assert_eq!(
        validate_redundant_compare(
            &source,
            0,
            REDUNDANT,
            &environment,
            budget(),
            source.transformed().clone(),
        )
        .unwrap_err(),
        RedundantCompareError::ReplayMismatch
    );
    let result = remove(&source, &environment).unwrap();
    // The surviving shadow was renamed — not the removal.
    let mut wrong_member = result.transformed().clone();
    wrong_member.functions[0].blocks[0].instructions[2].id = SelectedInstructionId(90);
    assert_eq!(
        validate_redundant_compare(&source, 0, REDUNDANT, &environment, budget(), wrong_member)
            .unwrap_err(),
        RedundantCompareError::ReplayMismatch
    );
    // Drift in an instruction the removal never touched.
    let mut drifted = result.transformed().clone();
    drifted.functions[0].blocks[0].instructions[0].kind =
        SelectedInstructionKind::Load8 { byte_offset: 8 };
    assert_eq!(
        validate_redundant_compare(&source, 0, REDUNDANT, &environment, budget(), drifted)
            .unwrap_err(),
        RedundantCompareError::ReplayMismatch
    );
    // A settlement the shift did not apply.
    let mut unshifted = result.transformed().clone();
    unshifted.functions[0]
        .boundary_settlements
        .push(settlement(4, 30));
    assert_eq!(
        validate_redundant_compare(&source, 0, REDUNDANT, &environment, budget(), unshifted)
            .unwrap_err(),
        RedundantCompareError::ReplayMismatch
    );
    // Drift outside the function under rewrite.
    let mut foreign = result.transformed().clone();
    foreign.functions[0].virtual_registers[0].scalar_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap());
    assert_eq!(
        validate_redundant_compare(&source, 0, REDUNDANT, &environment, budget(), foreign)
            .unwrap_err(),
        RedundantCompareError::ReplayMismatch
    );
}

/// The removal is deterministic — two runs publish identical plans — and
/// terminal: the published artifact feeds back through the sealed analysis
/// boundary as a legal second input, re-admission at the removed compare's
/// site refuses, and the surviving shadow — itself dead behind the return —
/// removes through the dead family on the published plan.
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
        remove_redundant_compare(&first, 0, REDUNDANT, &environment, budget()).unwrap_err(),
        RedundantCompareError::SourceMismatch
    );
    // The surviving compare's flags reach the return unobserved: the dead
    // family removes it, leaving the two loads — the families compose.
    let dead = remove_dead_compare(&first, 0, COMPARE, &environment, budget()).unwrap();
    let ids: Vec<SelectedInstructionId> = dead.transformed().functions[0].blocks[0]
        .instructions
        .iter()
        .map(|instruction| instruction.id)
        .collect();
    assert_eq!(ids, vec![LOAD, SECOND_LOAD]);
}
