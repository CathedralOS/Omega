use crate::DeadCompareError;
use crate::EquivalentCompareError;
use crate::EquivalentCompareReceipt;
use crate::ValidatedEquivalentCompare;
use crate::ValidatedSelectedAnalysis;
use crate::remove_dead_compare;
use crate::remove_equivalent_compare;
use crate::validate_equivalent_compare;
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
const MATERIALIZE: SelectedInstructionId = SelectedInstructionId(4);
const SECOND_MATERIALIZE: SelectedInstructionId = SelectedInstructionId(12);
const SHADOW: SelectedInstructionId = SelectedInstructionId(5);
const EQUIVALENT: SelectedInstructionId = SelectedInstructionId(6);
const TERMINAL: SelectedInstructionId = SelectedInstructionId(7);
const BOOLEAN: SelectedInstructionId = SelectedInstructionId(8);
const MOVED: SelectedInstructionId = SelectedInstructionId(9);
const THIRD_LOAD: SelectedInstructionId = SelectedInstructionId(10);

const POINTER: VirtualRegisterId = VirtualRegisterId(0);
const INPUT: VirtualRegisterId = VirtualRegisterId(1);
const LITERAL: VirtualRegisterId = VirtualRegisterId(2);
const OTHER: VirtualRegisterId = VirtualRegisterId(3);
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
/// `r1 = load8 r0; r3 = load8 r0; r2 = materialize 7; compare_imm r1, 7;
/// compare r1, r2; return`. The immediate-form shadow computes `r1 - 7`,
/// the register-form victim `r1 - r2` — different kinds and different
/// operand registers, but `r2`'s unique materialization pins it to the same
/// literal the shadow encodes, so the victim republishes flag state every
/// path already observes.
fn fixture(target: NativeTarget) -> ValidatedEquivalentCompare {
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let load = environment.constraint(keys.load8.unwrap()).unwrap();
    let materialize = environment.constraint(keys.materialize_i64).unwrap();
    let compare = environment.constraint(keys.compare_i64).unwrap();
    let immediate = environment.constraint(keys.compare_i64_immediate).unwrap();
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
            LITERAL,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: MATERIALIZE,
                source_value: ValueId::new(4).unwrap(),
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
                        MATERIALIZE,
                        SelectedInstructionKind::MaterializeI64 {
                            value: IntegerValue::Unsigned(7),
                        },
                        materialize,
                        &[LITERAL],
                    ),
                    instruction(
                        SHADOW,
                        SelectedInstructionKind::CompareI64Immediate {
                            immediate: IntegerValue::Unsigned(7),
                        },
                        immediate,
                        &[INPUT],
                    ),
                    instruction(
                        EQUIVALENT,
                        SelectedInstructionKind::CompareI64,
                        compare,
                        &[INPUT, LITERAL],
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
    ValidatedEquivalentCompare {
        receipt: EquivalentCompareReceipt {
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
) -> ValidatedEquivalentCompare {
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
    source: &ValidatedEquivalentCompare,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> Result<ValidatedEquivalentCompare, EquivalentCompareError> {
    remove_equivalent_compare(source, 0, EQUIVALENT, environment, budget())
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

/// The headline case: an immediate-form shadow republishes the register-form
/// victim's subtraction because the victim's right operand materializes the
/// encoded literal — on every target, and the result replays by content.
#[test]
fn cross_form_shadow_removes_on_every_target() {
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
        assert_eq!(block.instructions.len(), 4);
        assert_eq!(
            block
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![LOAD, SECOND_LOAD, MATERIALIZE, SHADOW]
        );
        assert_eq!(
            result.receipt().source_selected(),
            source.selected_identity()
        );
        assert_eq!(
            result.receipt().transformed_selected(),
            selected_instruction_plan_identity(result.transformed())
        );
        validate_equivalent_compare(
            &source,
            0,
            EQUIVALENT,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
        // A detached, separately allocated proposal replays by content.
        let mut detached = result.transformed().clone();
        detached.functions = detached.functions.iter().cloned().collect();
        validate_equivalent_compare(&source, 0, EQUIVALENT, &environment, budget(), detached)
            .unwrap();
    }
}

/// Equivalence is symmetric in the compare forms: a register-form shadow
/// whose right operand materializes the victim's encoded immediate serves
/// an immediate-form victim.
#[test]
fn register_shadow_serves_immediate_victim() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let compare = environment
            .constraint(environment.selected_keys().compare_i64)
            .unwrap()
            .clone();
        let immediate = environment
            .constraint(environment.selected_keys().compare_i64_immediate)
            .unwrap()
            .clone();
        function.blocks[0].instructions[3] = instruction(
            SHADOW,
            SelectedInstructionKind::CompareI64,
            &compare,
            &[INPUT, LITERAL],
        );
        function.blocks[0].instructions[4] = instruction(
            EQUIVALENT,
            SelectedInstructionKind::CompareI64Immediate {
                immediate: IntegerValue::Unsigned(7),
            },
            &immediate,
            &[INPUT],
        );
    });
    let result = remove(&source, &environment).unwrap();
    assert!(
        result.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .all(|instruction| instruction.id != EQUIVALENT)
    );
}

/// The zero form reads as `left - 0`: a register materializing zero pins
/// the victim's right side to the shadow's encoded literal.
#[test]
fn zero_shadow_serves_materialized_zero_victim() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let zero = environment
            .constraint(environment.selected_keys().compare_i64_zero)
            .unwrap()
            .clone();
        function.blocks[0].instructions[2].kind = SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(0),
        };
        function.blocks[0].instructions[3] = instruction(
            SHADOW,
            SelectedInstructionKind::CompareI64Zero,
            &zero,
            &[INPUT],
        );
    });
    let result = remove(&source, &environment).unwrap();
    assert!(
        result.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .all(|instruction| instruction.id != EQUIVALENT)
    );
}

/// Two register-form compares with different operand registers admit when
/// each pair of distinct operands materializes the same literal: the
/// registers are different names for one value.
#[test]
fn literal_pinned_register_pair_admits() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let compare = environment
            .constraint(environment.selected_keys().compare_i64)
            .unwrap()
            .clone();
        let materialize = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap()
            .clone();
        let class = function.virtual_registers[1].class;
        function.virtual_registers.push(register(
            FRESH,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: SECOND_MATERIALIZE,
                source_value: ValueId::new(7).unwrap(),
            },
        ));
        function.blocks[0].instructions.insert(
            3,
            instruction(
                SECOND_MATERIALIZE,
                SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(7),
                },
                &materialize,
                &[FRESH],
            ),
        );
        // shadow: `compare INPUT, FRESH`; victim: `compare INPUT, LITERAL`
        // — different right registers, both pinned to 7.
        function.blocks[0].instructions[4] = instruction(
            SHADOW,
            SelectedInstructionKind::CompareI64,
            &compare,
            &[INPUT, FRESH],
        );
    });
    let result = remove(&source, &environment).unwrap();
    let ids: Vec<SelectedInstructionId> = result.transformed().functions[0].blocks[0]
        .instructions
        .iter()
        .map(|instruction| instruction.id)
        .collect();
    assert_eq!(
        ids,
        vec![LOAD, SECOND_LOAD, MATERIALIZE, SECOND_MATERIALIZE, SHADOW]
    );
}

/// Subtraction direction is part of the grammar: `compare LITERAL, INPUT`
/// is not `compare INPUT, 7` even though the literal matches — the left
/// side resolves to no common value.
#[test]
fn direction_mismatch_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let compare = environment
            .constraint(environment.selected_keys().compare_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[4] = instruction(
            EQUIVALENT,
            SelectedInstructionKind::CompareI64,
            &compare,
            &[LITERAL, INPUT],
        );
    });
    assert_eq!(
        remove(&source, &environment).unwrap_err(),
        EquivalentCompareError::UnsupportedUse
    );
}

/// A swapped pair still admits when every side independently resolves to
/// the same literal: `compare LITERAL, FRESH` and `compare FRESH, LITERAL`
/// both compute `7 - 7`.
#[test]
fn materialized_swapped_operands_admit() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let compare = environment
            .constraint(environment.selected_keys().compare_i64)
            .unwrap()
            .clone();
        let materialize = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap()
            .clone();
        let class = function.virtual_registers[1].class;
        function.virtual_registers.push(register(
            FRESH,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: SECOND_MATERIALIZE,
                source_value: ValueId::new(7).unwrap(),
            },
        ));
        function.blocks[0].instructions.insert(
            3,
            instruction(
                SECOND_MATERIALIZE,
                SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(7),
                },
                &materialize,
                &[FRESH],
            ),
        );
        function.blocks[0].instructions[4] = instruction(
            SHADOW,
            SelectedInstructionKind::CompareI64,
            &compare,
            &[FRESH, LITERAL],
        );
        function.blocks[0].instructions[5] = instruction(
            EQUIVALENT,
            SelectedInstructionKind::CompareI64,
            &compare,
            &[LITERAL, FRESH],
        );
    });
    let result = remove(&source, &environment).unwrap();
    assert!(
        result.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .all(|instruction| instruction.id != EQUIVALENT)
    );
}

/// A different literal is a different subtraction: the shadow encodes 8,
/// the victim's register materializes 7.
#[test]
fn different_literal_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let immediate = environment
            .constraint(environment.selected_keys().compare_i64_immediate)
            .unwrap()
            .clone();
        function.blocks[0].instructions[3] = instruction(
            SHADOW,
            SelectedInstructionKind::CompareI64Immediate {
                immediate: IntegerValue::Unsigned(8),
            },
            &immediate,
            &[INPUT],
        );
    });
    assert_eq!(
        remove(&source, &environment).unwrap_err(),
        EquivalentCompareError::UnsupportedUse
    );
}

/// A right side with two producers is not literal-pinned: the register may
/// read a different value than the materialization's.
#[test]
fn ambiguous_producer_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        // A second definition of LITERAL arrives before the shadow: the
        // unique-producer audit no longer pins it to 7.
        function.blocks[0].instructions.insert(
            3,
            instruction(
                MOVED,
                SelectedInstructionKind::CopyI64,
                &copy,
                &[OTHER, LITERAL],
            ),
        );
    });
    assert_eq!(
        remove(&source, &environment).unwrap_err(),
        EquivalentCompareError::UnsupportedUse
    );
}

/// A flag reader after the victim does not block removal — the reader
/// observes the shadow's identical publication. The dead family refuses
/// this same plan.
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
            FRESH,
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
            &[FRESH],
        ));
    });
    let result = remove(&source, &environment).unwrap();
    let ids: Vec<SelectedInstructionId> = result.transformed().functions[0].blocks[0]
        .instructions
        .iter()
        .map(|instruction| instruction.id)
        .collect();
    assert_eq!(ids, vec![LOAD, SECOND_LOAD, MATERIALIZE, SHADOW, BOOLEAN]);
    // The dead family must refuse the same removal: the reader observes the
    // victim's publication — the families answer different questions about
    // the same surface.
    assert_eq!(
        remove_dead_compare(&source, 0, EQUIVALENT, &environment, budget()).unwrap_err(),
        DeadCompareError::UnsupportedUse
    );
}

/// A write to the shared left register between the shadow and the victim
/// refuses: the victim may compute flags from a different value than the
/// shadow observed. A write to the literal-pinned right side can never
/// exist — the unique-producer guarantee is function-wide — so only the
/// shared register's drift is audited.
#[test]
fn shared_register_rewrite_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let copy_row = environment
        .constraint(environment.selected_keys().copy_i64)
        .unwrap()
        .clone();
    let source = mutated(target, |function, _| {
        function.blocks[0].instructions.insert(
            4,
            instruction(
                MOVED,
                SelectedInstructionKind::CopyI64,
                &copy_row,
                &[OTHER, INPUT],
            ),
        );
    });
    assert_eq!(
        remove(&source, &environment).unwrap_err(),
        EquivalentCompareError::UnsupportedUse
    );
}

/// An instruction between the shadow and the victim that writes no shared
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
            4,
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
    assert_eq!(
        ids,
        vec![LOAD, SECOND_LOAD, MATERIALIZE, SHADOW, THIRD_LOAD]
    );
}

/// A nearer flag event is the resolved shadow: when the nearer compare is
/// not value-equivalent — here a plain swapped pair whose sides resolve to
/// no common value — admission refuses even though an earlier equivalent
/// shadow exists behind it.
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
            4,
            instruction(
                THIRD_LOAD,
                SelectedInstructionKind::CompareI64,
                &compare,
                &[LITERAL, INPUT],
            ),
        );
    });
    assert_eq!(
        remove(&source, &environment).unwrap_err(),
        EquivalentCompareError::UnsupportedUse
    );
}

/// A shadow that clobbers a unit the victim defines is not equivalent:
/// removing the victim would leave readers an unspecified unit where the
/// compare published real flags.
#[test]
fn role_mismatch_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        let shadow = &mut function.blocks[0].instructions[3];
        shadow.clobbers = shadow.implicit_defs.clone();
        shadow.implicit_defs = Vec::new();
    });
    assert_eq!(
        remove(&source, &environment).unwrap_err(),
        EquivalentCompareError::UnsupportedUse
    );
}

/// A cross-block shadow admits the same way: the join's victim resolves
/// its reaching set to the predecessor's immediate-form shadow, and the
/// exposed interval crossing the edge holds no write to the shared
/// register.
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
        let victim = function.blocks[0].instructions.remove(4);
        function.blocks[0].terminator = SelectedTerminator::Jump {
            instruction: instruction(TERMINAL, SelectedInstructionKind::Jump, &jump_row, &[]),
            successor: successor(1, 2),
        };
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(1),
            origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
            instructions: vec![victim],
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
    validate_equivalent_compare(
        &source,
        0,
        EQUIVALENT,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
}

/// A write to the shared left register inside the compare's own block —
/// exposed between the cross-block shadow and the victim — refuses.
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
        let victim = function.blocks[0].instructions.remove(4);
        function.blocks[0].terminator = SelectedTerminator::Jump {
            instruction: instruction(TERMINAL, SelectedInstructionKind::Jump, &jump_row, &[]),
            successor: successor(1, 2),
        };
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(1),
            origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
            instructions: vec![
                instruction(
                    MOVED,
                    SelectedInstructionKind::CopyI64,
                    &copy_row,
                    &[OTHER, INPUT],
                ),
                victim,
            ],
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
        EquivalentCompareError::UnsupportedUse
    );
}

/// An edge transport defining the shared left register on the crossed edge
/// refuses: the victim would read the transported value, not the one the
/// shadow computed flags from.
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
        let victim = function.blocks[0].instructions.remove(4);
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
            instructions: vec![victim],
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
        EquivalentCompareError::UnsupportedUse
    );
}

/// The family's reason to exist: a diamond whose arms publish the same
/// subtraction through different compare forms — register-form on one arm,
/// immediate-form on the other — resolves to one value-equivalent reaching
/// set, and the join's compare removes.
#[test]
fn diamond_mixed_forms_admit() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let keys = environment.selected_keys();
        let compare_row = environment.constraint(keys.compare_i64).unwrap().clone();
        let immediate_row = environment
            .constraint(keys.compare_i64_immediate)
            .unwrap()
            .clone();
        let branch_row = environment
            .constraint(keys.conditional_branch)
            .unwrap()
            .clone();
        let jump_row = environment.constraint(keys.jump).unwrap().clone();
        let return_row = environment.constraint(keys.return_unit).unwrap().clone();
        let victim = function.blocks[0].instructions.remove(4);
        function.blocks[0].instructions.remove(3);
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
        // Arm 1 shadows with the register form, arm 2 with the immediate
        // form — the same subtraction through different kinds.
        let arm_one = instruction(
            SelectedInstructionId(12),
            SelectedInstructionKind::CompareI64,
            &compare_row,
            &[INPUT, LITERAL],
        );
        let arm_two = instruction(
            SelectedInstructionId(13),
            SelectedInstructionKind::CompareI64Immediate {
                immediate: IntegerValue::Unsigned(7),
            },
            &immediate_row,
            &[INPUT],
        );
        for (block, edge, arm_shadow) in [(1u32, 4u64, arm_one), (2u32, 5u64, arm_two)] {
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
            instructions: vec![victim],
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
    validate_equivalent_compare(
        &source,
        0,
        EQUIVALENT,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
}

/// One divergent arm refuses: the leg whose last event encodes a different
/// literal publishes a different subtraction, so the reaching set does not
/// resolve to value-equivalent sites.
#[test]
fn divergent_leg_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let keys = environment.selected_keys();
        let compare_row = environment.constraint(keys.compare_i64).unwrap().clone();
        let immediate_row = environment
            .constraint(keys.compare_i64_immediate)
            .unwrap()
            .clone();
        let branch_row = environment
            .constraint(keys.conditional_branch)
            .unwrap()
            .clone();
        let jump_row = environment.constraint(keys.jump).unwrap().clone();
        let return_row = environment.constraint(keys.return_unit).unwrap().clone();
        let victim = function.blocks[0].instructions.remove(4);
        function.blocks[0].instructions.remove(3);
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
        let arm_one = instruction(
            SelectedInstructionId(12),
            SelectedInstructionKind::CompareI64,
            &compare_row,
            &[INPUT, LITERAL],
        );
        let arm_two = instruction(
            SelectedInstructionId(13),
            SelectedInstructionKind::CompareI64Immediate {
                immediate: IntegerValue::Unsigned(8),
            },
            &immediate_row,
            &[INPUT],
        );
        for (block, edge, arm_shadow) in [(1u32, 4u64, arm_one), (2u32, 5u64, arm_two)] {
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
            instructions: vec![victim],
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
        EquivalentCompareError::UnsupportedUse
    );
}

/// Without an earlier flag event there is no shadow at all: the entry's
/// condition state is unknown.
#[test]
fn no_shadow_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        function.blocks[0].instructions.remove(3);
    });
    assert_eq!(
        remove(&source, &environment).unwrap_err(),
        EquivalentCompareError::UnsupportedUse
    );
}

/// Boundary settlements positioned after the removed compare shift one
/// ordinal earlier; positions at or before it stay put.
#[test]
fn settlements_shift_over_the_removed_ordinal() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        // The victim sits at body index 4: position 3 settles before it and
        // stays; position 4 settles at it and stays; position 5 — after the
        // body — shifts one ordinal earlier to 4.
        function.boundary_settlements =
            vec![settlement(3, 20), settlement(4, 21), settlement(5, 22)];
    });
    let result = remove(&source, &environment).unwrap();
    let positions: Vec<u32> = result.transformed().functions[0]
        .boundary_settlements
        .iter()
        .map(|settlement| settlement.instruction_index)
        .collect();
    assert_eq!(positions, vec![3, 4, 4]);
}

#[test]
fn source_identity_mismatches_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    assert_eq!(
        remove_equivalent_compare(&source, 1, EQUIVALENT, &environment, budget()).unwrap_err(),
        EquivalentCompareError::SourceMismatch
    );
    assert_eq!(
        remove_equivalent_compare(
            &source,
            0,
            SelectedInstructionId(99),
            &environment,
            budget()
        )
        .unwrap_err(),
        EquivalentCompareError::SourceMismatch
    );
    let wrong_environment =
        baseline_target_register_environment(NativeTarget::macos_arm64()).unwrap();
    assert_eq!(
        remove_equivalent_compare(&source, 0, EQUIVALENT, &wrong_environment, budget())
            .unwrap_err(),
        EquivalentCompareError::SourceMismatch
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
        remove_equivalent_compare(&source, 0, LOAD, &environment, budget()).unwrap_err(),
        EquivalentCompareError::UnsupportedInstruction
    );
    // A `Def` operand rides along on the compare.
    let defined = mutated(target, |function, _| {
        function.blocks[0].instructions[4].operands[1].access = RegisterOperandAccess::Def;
    });
    assert_eq!(
        remove(&defined, &environment).unwrap_err(),
        EquivalentCompareError::UnsupportedInstruction
    );
    // A fixed view narrows the operand surface.
    let viewed = mutated(target, |function, _| {
        function.blocks[0].instructions[4].operands[0].fixed_view =
            Some(register_model::RegisterViewId(0));
    });
    assert_eq!(
        remove(&viewed, &environment).unwrap_err(),
        EquivalentCompareError::UnsupportedInstruction
    );
    // An operand position other than its canonical index.
    let displaced = mutated(target, |function, _| {
        function.blocks[0].instructions[4].operands[1].operand = 3;
    });
    assert_eq!(
        remove(&displaced, &environment).unwrap_err(),
        EquivalentCompareError::UnsupportedInstruction
    );
    // An implicit use the removal would silently drop.
    let implicit_read = mutated(target, |function, _| {
        let defs = function.blocks[0].instructions[4].implicit_defs.clone();
        function.blocks[0].instructions[4].implicit_uses = defs;
    });
    assert_eq!(
        remove(&implicit_read, &environment).unwrap_err(),
        EquivalentCompareError::UnsupportedInstruction
    );
    // A compare publishing no condition state at all is malformed.
    let surfaceless = mutated(target, |function, _| {
        function.blocks[0].instructions[4].implicit_defs = Vec::new();
        function.blocks[0].instructions[4].clobbers = Vec::new();
    });
    assert_eq!(
        remove(&surfaceless, &environment).unwrap_err(),
        EquivalentCompareError::UnsupportedInstruction
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
        function.blocks[0].instructions[4].constraint = copy_row;
    });
    assert_eq!(
        remove(&wrong_row, &environment).unwrap_err(),
        EquivalentCompareError::ConstraintMismatch
    );
    // A row key that resolves nowhere the environment knows.
    let unknown_row = mutated(target, |function, _| {
        function.blocks[0].instructions[4].constraint = register_model::RegisterConstraintKey {
            family: register_model::RegisterConstraintFamily::Instruction,
            variant: 65_000,
        };
    });
    assert_eq!(
        remove(&unknown_row, &environment).unwrap_err(),
        EquivalentCompareError::ConstraintMismatch
    );
    // An operand register missing from the roster cannot class-check.
    let unrostered = mutated(target, |function, _| {
        function.blocks[0].instructions[4].operands[0].virtual_register = VirtualRegisterId(40);
    });
    assert_eq!(
        remove(&unrostered, &environment).unwrap_err(),
        EquivalentCompareError::ConstraintMismatch
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
            instruction: EQUIVALENT,
            origin: SelectedMemoryAccessOrigin::Operation(OperationId::new(11).unwrap()),
            place: PlaceId::new(1).unwrap(),
            byte_offset: 0,
            byte_count: 8,
            role: SelectedMemoryAccessRole::ReadPlace,
        });
    });
    assert_eq!(
        remove(&memory_row, &environment).unwrap_err(),
        EquivalentCompareError::UnsupportedInstruction
    );
    let call_row = mutated(target, |function, _| {
        function.calls.push(SelectedCallContract {
            instruction: EQUIVALENT,
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
        EquivalentCompareError::UnsupportedInstruction
    );
}

/// The measured validation-step boundary: admission charges one step per
/// block plus one per instruction across the plan, repeats the scan over
/// the admitted function together with its register roster and call/access
/// rows, then charges the reaching walk's setup — every edge's target
/// resolution, the predecessor fill, and the cone mark — plus per
/// published unit the in-block prefix scan, the per-block last-event
/// table, the bounded fixpoint propagation, and the per-site value
/// equivalence checks' bounded unique-producer scans, and finally the
/// backward operand audit once per unit over stream positions and crossed
/// edge surfaces — so the exact count admits the removal on both the
/// proposal and the independent replay path while one step below rejects
/// both, on the single-block fixture and on a second whose acyclic block
/// list carries a nonzero edge count.
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
            .find(|instruction| instruction.id == EQUIVALENT)
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
        let per_unit = function_scan
            + block_count
            + pops
            + pops * widest_out * elements
            + elements * function_scan * 4;
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
        let result =
            remove_equivalent_compare(&source, 0, EQUIVALENT, &environment, exact).unwrap();
        validate_equivalent_compare(
            &source,
            0,
            EQUIVALENT,
            &environment,
            exact,
            result.transformed().clone(),
        )
        .unwrap();
        let starved = OptimizationWorkBudget::new(1, 1, exact_steps - 1, 1, 1).unwrap();
        assert_eq!(
            remove_equivalent_compare(&source, 0, EQUIVALENT, &environment, starved).unwrap_err(),
            EquivalentCompareError::WorkBudgetExceeded
        );
        assert_eq!(
            validate_equivalent_compare(
                &source,
                0,
                EQUIVALENT,
                &environment,
                starved,
                result.transformed().clone(),
            )
            .unwrap_err(),
            EquivalentCompareError::WorkBudgetExceeded
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
        validate_equivalent_compare(
            &source,
            0,
            EQUIVALENT,
            &environment,
            budget(),
            source.transformed().clone(),
        )
        .unwrap_err(),
        EquivalentCompareError::ReplayMismatch
    );
    let result = remove(&source, &environment).unwrap();
    // The surviving shadow was renamed — not the removal.
    let mut wrong_member = result.transformed().clone();
    wrong_member.functions[0].blocks[0].instructions[3].id = SelectedInstructionId(90);
    assert_eq!(
        validate_equivalent_compare(&source, 0, EQUIVALENT, &environment, budget(), wrong_member)
            .unwrap_err(),
        EquivalentCompareError::ReplayMismatch
    );
    // Drift in an instruction the removal never touched.
    let mut drifted = result.transformed().clone();
    drifted.functions[0].blocks[0].instructions[0].kind =
        SelectedInstructionKind::Load8 { byte_offset: 8 };
    assert_eq!(
        validate_equivalent_compare(&source, 0, EQUIVALENT, &environment, budget(), drifted)
            .unwrap_err(),
        EquivalentCompareError::ReplayMismatch
    );
    // A settlement the shift did not apply.
    let mut unshifted = result.transformed().clone();
    unshifted.functions[0]
        .boundary_settlements
        .push(settlement(5, 30));
    assert_eq!(
        validate_equivalent_compare(&source, 0, EQUIVALENT, &environment, budget(), unshifted)
            .unwrap_err(),
        EquivalentCompareError::ReplayMismatch
    );
    // Drift outside the function under rewrite.
    let mut foreign = result.transformed().clone();
    foreign.functions[0].virtual_registers[0].scalar_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap());
    assert_eq!(
        validate_equivalent_compare(&source, 0, EQUIVALENT, &environment, budget(), foreign)
            .unwrap_err(),
        EquivalentCompareError::ReplayMismatch
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
        remove_equivalent_compare(&first, 0, EQUIVALENT, &environment, budget()).unwrap_err(),
        EquivalentCompareError::SourceMismatch
    );
    // The surviving compare's flags reach the return unobserved: the dead
    // family removes it — the families compose.
    let dead = remove_dead_compare(&first, 0, SHADOW, &environment, budget()).unwrap();
    let ids: Vec<SelectedInstructionId> = dead.transformed().functions[0].blocks[0]
        .instructions
        .iter()
        .map(|instruction| instruction.id)
        .collect();
    assert_eq!(ids, vec![LOAD, SECOND_LOAD, MATERIALIZE]);
}
