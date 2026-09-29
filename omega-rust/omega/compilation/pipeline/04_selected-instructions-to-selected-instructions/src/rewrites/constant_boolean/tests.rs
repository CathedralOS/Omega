use super::{
    ConstantBooleanError, ConstantBooleanReceipt, ValidatedConstantBoolean,
    fold_selected_constant_boolean as produce_fold, validate_constant_boolean_fold as replay_fold,
};
use crate::ValidatedSelectedAnalysis;
use crate::analyses::validated_machine_effect_catalog;
use crate::rewrites::test_support::{budget, instruction, measured_step_budget};
use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget};
use optimization_unit::ValueDefinitionSite;
use register_environment::{
    ValidatedTargetRegisterEnvironment, baseline_target_register_environment,
};
use register_model::RegisterOperandAccess;
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan, SelectedSuccessor,
    SelectedSuccessorRole, SelectedTerminator, VirtualRegister, VirtualRegisterId,
    VirtualRegisterOrigin,
};
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, MachineId,
    OperationId, ScalarType, ValueId,
};
use target::NativeTarget;
use target_operations_to_selected_instructions::selected_instruction_plan_identity;
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

const MATERIALIZE_A: SelectedInstructionId = SelectedInstructionId(2);
const MATERIALIZE_B: SelectedInstructionId = SelectedInstructionId(3);
const COMPARE: SelectedInstructionId = SelectedInstructionId(4);
const BOOLEAN: SelectedInstructionId = SelectedInstructionId(5);
const EXTRA: SelectedInstructionId = SelectedInstructionId(7);
const LITA: VirtualRegisterId = VirtualRegisterId(0);
const LITB: VirtualRegisterId = VirtualRegisterId(1);
const PARAM: VirtualRegisterId = VirtualRegisterId(2);
const OUTPUT: VirtualRegisterId = VirtualRegisterId(3);

fn register(
    id: VirtualRegisterId,
    scalar_type: ScalarType,
    class: register_model::RegisterClassId,
    origin: VirtualRegisterOrigin,
) -> VirtualRegister {
    VirtualRegister {
        id,
        scalar_type,
        class,
        origin,
        definition_site: None,
        entry_fixed_view: None,
    }
}

/// A raw selected-stage unit fixture, not a source/Terminal admission claim:
/// `ra = materialize va; rb = materialize vb; compare ra, rb;
/// `rout = boolean; return`. Variants rebuild the compare's kind and operand
/// roster or the boolean's kind through `mutated`.
fn fixture(
    target: NativeTarget,
    left_value: IntegerValue,
    right_value: IntegerValue,
    boolean_kind: SelectedInstructionKind,
) -> ValidatedConstantBoolean {
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let materialize = environment.constraint(keys.materialize_i64).unwrap();
    let compare = environment.constraint(keys.compare_i64).unwrap();
    let boolean = environment.constraint(keys.materialize_boolean).unwrap();
    let terminal_row = environment.constraint(keys.return_unit).unwrap();
    let class = compare.operands[0].class;
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let registers = vec![
        register(
            LITA,
            scalar_type,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: MATERIALIZE_A,
                source_value: ValueId::new(2).unwrap(),
            },
        ),
        register(
            LITB,
            scalar_type,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: MATERIALIZE_B,
                source_value: ValueId::new(3).unwrap(),
            },
        ),
        VirtualRegister {
            id: PARAM,
            scalar_type,
            class,
            origin: VirtualRegisterOrigin::EntryParameter {
                source_value: ValueId::new(4).unwrap(),
                parameter_index: 0,
            },
            definition_site: Some(ValueDefinitionSite::FunctionParameter(0)),
            entry_fixed_view: None,
        },
        register(
            OUTPUT,
            scalar_type,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: BOOLEAN,
                source_value: ValueId::new(5).unwrap(),
            },
        ),
    ];
    let mut compare_instruction = instruction(
        COMPARE,
        SelectedInstructionKind::CompareI64,
        compare,
        &[LITA, LITB],
    );
    compare_instruction.provenance.operations = vec![OperationId::new(7).unwrap()];
    compare_instruction.provenance.values = vec![ValueId::new(3).unwrap()];
    let mut boolean_instruction = instruction(BOOLEAN, boolean_kind, boolean, &[OUTPUT]);
    boolean_instruction.provenance.values = vec![ValueId::new(5).unwrap()];
    let instructions = vec![
        instruction(
            MATERIALIZE_A,
            SelectedInstructionKind::MaterializeI64 { value: left_value },
            materialize,
            &[LITA],
        ),
        instruction(
            MATERIALIZE_B,
            SelectedInstructionKind::MaterializeI64 { value: right_value },
            materialize,
            &[LITB],
        ),
        compare_instruction,
        boolean_instruction,
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
                instructions,
                terminator: SelectedTerminator::Return {
                    instruction: instruction(
                        SelectedInstructionId(6),
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
    ValidatedConstantBoolean {
        receipt: ConstantBooleanReceipt {
            source_selected: identity,
            transformed_selected: identity,
            optimization_unit: OptimizationUnitIdentity::from_bytes([2; 32]),
            fuel_schedule: plan.fuel_schedule,
            function_index: 0,
            materialization: BOOLEAN,
        },
        transformed: std::sync::Arc::new(plan),
    }
}

fn keys(
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> selected_instructions::SelectedConstraintKeys {
    environment.selected_keys()
}

/// The machine-effect catalog the stage binds for `environment` — the
/// declarations the condition-materialization pair descriptors resolve
/// their effect surfaces in.
fn catalog(
    environment: &ValidatedTargetRegisterEnvironment,
) -> std::sync::Arc<selected_instructions::ValidatedMachineEffectCatalog> {
    validated_machine_effect_catalog(environment.target(), environment.constraints()).unwrap()
}

/// The stage-bound fold: the environment's own validated catalog, exactly
/// as the pre-allocation pass binds it. Tests that exercise a foreign or
/// corrupted catalog call [`produce_fold`] directly.
fn fold_selected_constant_boolean(
    source: &ValidatedConstantBoolean,
    materialization: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedConstantBoolean, ConstantBooleanError> {
    produce_fold(
        source,
        0,
        materialization,
        environment,
        &catalog(environment),
        budget,
    )
}

/// The stage-bound independent replay, binding the environment's catalog.
fn validate_constant_boolean_fold(
    source: &ValidatedConstantBoolean,
    materialization: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedConstantBoolean, ConstantBooleanError> {
    replay_fold(
        source,
        0,
        materialization,
        environment,
        &catalog(environment),
        budget,
        proposed,
    )
}

/// Edit the single fixture function, then refresh the receipt identities so
/// the mutated plan is a well-formed analysis source.
fn mutated(
    target: NativeTarget,
    edit: impl FnOnce(&mut SelectedFunction, &register_environment::ValidatedTargetRegisterEnvironment),
) -> ValidatedConstantBoolean {
    let environment = baseline_target_register_environment(target).unwrap();
    let mut source = fixture(
        target,
        IntegerValue::Unsigned(3),
        IntegerValue::Unsigned(5),
        SelectedInstructionKind::MaterializeBooleanEqual,
    );
    edit(
        &mut std::sync::Arc::make_mut(&mut source.transformed).functions[0],
        &environment,
    );
    let identity = selected_instruction_plan_identity(&source.transformed);
    source.receipt.source_selected = identity;
    source.receipt.transformed_selected = identity;
    source
}

fn fold(
    source: &ValidatedConstantBoolean,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> Result<ValidatedConstantBoolean, ConstantBooleanError> {
    fold_selected_constant_boolean(source, BOOLEAN, environment, budget())
}

fn replay(
    source: &ValidatedConstantBoolean,
    environment: &ValidatedTargetRegisterEnvironment,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedConstantBoolean, ConstantBooleanError> {
    validate_constant_boolean_fold(source, BOOLEAN, environment, budget(), proposed)
}

fn arm(block: SelectedBlockId, target: u64, edge: u64) -> SelectedSuccessor {
    SelectedSuccessor {
        role: SelectedSuccessorRole::Semantic,
        psi_edge: EdgeId::new(edge).unwrap(),
        block,
        source_target: BlockId::new(target).unwrap(),
        bindings: Vec::new(),
        structural_bindings: Vec::new(),
        structural_case: None,
        fuel: Vec::new(),
    }
}

fn returning(
    block: u32,
    instruction_id: u32,
    edge: u64,
    row: &register_model::RegisterInstructionConstraint,
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

/// A jump terminator on `block_index`-independent id targeting `arm`,
/// rebuilt from the fixture's own jump row.
fn jump_terminator(
    environment: &ValidatedTargetRegisterEnvironment,
    instruction_id: u32,
    arm_block: u32,
    edge: u64,
) -> SelectedTerminator {
    let jump_row = environment
        .constraint(environment.selected_keys().jump)
        .unwrap();
    SelectedTerminator::Jump {
        instruction: instruction(
            SelectedInstructionId(instruction_id),
            SelectedInstructionKind::Jump,
            jump_row,
            &[],
        ),
        successor: arm(SelectedBlockId(arm_block), u64::from(arm_block) + 1, edge),
    }
}

/// The instruction a successful fold publishes: `MaterializeI64` on the
/// boolean's own instruction identity carrying `value`, at `position` in
/// `block_index`'s body.
fn materialized(
    result: &ValidatedConstantBoolean,
    block_index: usize,
    position: usize,
    value: u64,
) -> &SelectedInstruction {
    let instruction = &result.transformed().functions[0].blocks[block_index].instructions[position];
    assert_eq!(instruction.id, BOOLEAN);
    assert_eq!(
        instruction.kind,
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(u128::from(value))
        }
    );
    instruction
}

/// Every boolean kind folds on every target, carrying the predicate outcome
/// the compare publishes for its two literal operands.
#[test]
fn predicate_table_folds_on_all_targets() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let cases: &[(SelectedInstructionKind, u64, u64, u64)] = &[
            (SelectedInstructionKind::MaterializeBooleanEqual, 7, 7, 1),
            (SelectedInstructionKind::MaterializeBooleanEqual, 7, 8, 0),
            (
                SelectedInstructionKind::MaterializeBooleanU64LessThan,
                3,
                9,
                1,
            ),
            (
                SelectedInstructionKind::MaterializeBooleanU64LessThan,
                9,
                3,
                0,
            ),
            // Unsigned ordering reads the published bit patterns: the
            // all-ones literal is the u64 maximum, not a negative.
            (
                SelectedInstructionKind::MaterializeBooleanU64LessThan,
                u64::MAX,
                0,
                0,
            ),
            (
                SelectedInstructionKind::MaterializeBooleanI64LessThan,
                u64::MAX,
                0,
                1,
            ),
            (
                SelectedInstructionKind::MaterializeBooleanI64LessThan,
                0,
                u64::MAX,
                0,
            ),
            (
                SelectedInstructionKind::MaterializeBooleanU64LessOrEqual,
                5,
                5,
                1,
            ),
            (
                SelectedInstructionKind::MaterializeBooleanU64LessOrEqual,
                6,
                5,
                0,
            ),
            (
                SelectedInstructionKind::MaterializeBooleanI64LessOrEqual,
                i64::MIN as u64,
                0,
                1,
            ),
            (
                SelectedInstructionKind::MaterializeBooleanI64LessOrEqual,
                0,
                i64::MIN as u64,
                0,
            ),
        ];
        for (kind, left, right, expected) in cases {
            let source = fixture(
                target,
                IntegerValue::Unsigned(u128::from(*left)),
                IntegerValue::Unsigned(u128::from(*right)),
                *kind,
            );
            let result = fold(&source, &environment).unwrap();
            let rewritten = &result.transformed().functions[0].blocks[0].instructions[3];
            assert_eq!(rewritten.id, BOOLEAN, "{target:?} {kind:?}");
            assert_eq!(
                rewritten.kind,
                SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(u128::from(*expected)),
                },
                "{target:?} {kind:?} {left} {right}"
            );
            assert_eq!(rewritten.constraint, keys(&environment).materialize_i64);
            assert_eq!(rewritten.operands.len(), 1);
            assert_eq!(rewritten.operands[0].operand, 0);
            assert_eq!(rewritten.operands[0].virtual_register, OUTPUT);
            assert_eq!(rewritten.operands[0].access, RegisterOperandAccess::Def);
            // The folded materialization drops the flag uses the boolean
            // carried; the compare keeps publishing them.
            assert!(rewritten.implicit_uses.is_empty());
            assert!(rewritten.implicit_defs.is_empty());
            assert!(rewritten.clobbers.is_empty());
            assert_eq!(
                result.transformed().functions[0].blocks[0].instructions[2].kind,
                SelectedInstructionKind::CompareI64
            );
            assert_eq!(
                rewritten.provenance,
                source.transformed().functions[0].blocks[0].instructions[3].provenance
            );
            assert_eq!(
                result.receipt().source_selected(),
                source.selected_identity()
            );
            assert_eq!(
                result.receipt().transformed_selected(),
                selected_instruction_plan_identity(result.transformed())
            );
            validate_constant_boolean_fold(
                &source,
                BOOLEAN,
                &environment,
                budget(),
                result.transformed().clone(),
            )
            .unwrap();
            // A detached, separately allocated proposal replays by content.
            let mut detached = result.transformed().clone();
            detached.functions = detached.functions.iter().cloned().collect();
            validate_constant_boolean_fold(&source, BOOLEAN, &environment, budget(), detached)
                .unwrap();
        }
    }
}

/// The zero compare form on a materialized register is constant:
/// `literal - 0` decides equality by the literal alone.
#[test]
fn zero_compare_folds() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for (literal, expected) in [(0u64, 1u64), (9, 0)] {
        let source = mutated(target, |function, environment| {
            let row = environment
                .constraint(environment.selected_keys().compare_i64_zero)
                .unwrap();
            function.blocks[0].instructions[0].kind = SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(u128::from(literal)),
            };
            function.blocks[0].instructions[2] = instruction(
                COMPARE,
                SelectedInstructionKind::CompareI64Zero,
                row,
                &[LITA],
            );
        });
        let result = fold(&source, &environment).unwrap();
        let rewritten = &result.transformed().functions[0].blocks[0].instructions[3];
        assert_eq!(
            rewritten.kind,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(u128::from(expected))
            },
            "{literal}"
        );
    }
}

/// Only the five flag-reading boolean kinds fold; any other named
/// instruction refuses.
#[test]
fn non_boolean_instructions_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(
        target,
        IntegerValue::Unsigned(3),
        IntegerValue::Unsigned(5),
        SelectedInstructionKind::MaterializeBooleanEqual,
    );
    // The compare itself is not a boolean materialization.
    assert_eq!(
        fold_selected_constant_boolean(&source, COMPARE, &environment, budget()).unwrap_err(),
        ConstantBooleanError::UnsupportedInstruction
    );
    // Neither is a plain materialization.
    assert_eq!(
        fold_selected_constant_boolean(&source, MATERIALIZE_A, &environment, budget()).unwrap_err(),
        ConstantBooleanError::UnsupportedInstruction
    );
    // An absent instruction id cannot be located.
    assert_eq!(
        fold_selected_constant_boolean(&source, SelectedInstructionId(99), &environment, budget())
            .unwrap_err(),
        ConstantBooleanError::SourceMismatch
    );
    // A flag-free boolean shape has no condition to evaluate.
    let flag_free = mutated(target, |function, _| {
        function.blocks[0].instructions[3].implicit_uses.clear();
    });
    assert_eq!(
        fold(&flag_free, &environment).unwrap_err(),
        ConstantBooleanError::UnsupportedInstruction
    );
    // A body instruction kind the family never declares — here a copy —
    // refuses even where its record carries flag uses.
    let wrong_kind = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        let boolean = environment
            .constraint(environment.selected_keys().materialize_boolean)
            .unwrap();
        let mut copy_record = instruction(
            BOOLEAN,
            SelectedInstructionKind::CopyI64,
            copy,
            &[PARAM, OUTPUT],
        );
        copy_record.implicit_uses = boolean.implicit_uses.clone();
        copy_record.operands.pop();
        function.blocks[0].instructions[3] = copy_record;
    });
    assert_eq!(
        fold(&wrong_kind, &environment).unwrap_err(),
        ConstantBooleanError::UnsupportedInstruction
    );
}

/// An intervening flag event — a second compare or a clobber — makes the
/// observed state the later instruction's, not the constant compare's.
#[test]
fn intervening_flag_events_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A second compare on a non-materialized operand is the reaching
    // definition; its right side has no literal producer.
    let second_compare = mutated(target, |function, environment| {
        let compare = environment
            .constraint(environment.selected_keys().compare_i64)
            .unwrap();
        function.blocks[0].instructions.insert(
            3,
            instruction(
                EXTRA,
                SelectedInstructionKind::CompareI64,
                compare,
                &[LITA, PARAM],
            ),
        );
    });
    assert_eq!(
        fold(&second_compare, &environment).unwrap_err(),
        ConstantBooleanError::UnsupportedProducer
    );
    // A clobber of the flag unit between compare and materialization leaves
    // the observed state unknown.
    let clobbered = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        let flags = function.blocks[0].instructions[2].implicit_defs.clone();
        let mut wiper = instruction(
            EXTRA,
            SelectedInstructionKind::CopyI64,
            copy,
            &[PARAM, OUTPUT],
        );
        wiper.clobbers = flags;
        function.blocks[0].instructions.insert(3, wiper);
    });
    assert_eq!(
        fold(&clobbered, &environment).unwrap_err(),
        ConstantBooleanError::UnsupportedUse
    );
    // A used unit no in-block instruction defines or clobbers reaches in
    // from outside the block and refuses.
    let outsider = mutated(target, |function, _| {
        let spare = register_model::RegisterUnitId(999);
        function.blocks[0].instructions[3].implicit_uses.push(spare);
    });
    assert_eq!(
        fold(&outsider, &environment).unwrap_err(),
        ConstantBooleanError::UnsupportedUse
    );
}

/// Literals must fit the sixty-four-bit register pattern they publish;
/// wider values are malformed materializations, not foldable constants.
#[test]
fn out_of_range_literals_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for value in [
        IntegerValue::Unsigned(u128::from(u64::MAX) + 1),
        IntegerValue::Signed(i128::from(i64::MIN) - 1),
        IntegerValue::Signed(i128::from(i64::MAX) + 1),
    ] {
        let source = fixture(
            target,
            value,
            IntegerValue::Unsigned(5),
            SelectedInstructionKind::MaterializeBooleanEqual,
        );
        assert_eq!(
            fold(&source, &environment).unwrap_err(),
            ConstantBooleanError::UnsupportedLiteral,
            "{value:?}"
        );
    }
}

#[test]
fn source_identity_mismatches_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(
        target,
        IntegerValue::Unsigned(3),
        IntegerValue::Unsigned(5),
        SelectedInstructionKind::MaterializeBooleanEqual,
    );
    // A function index outside the plan.
    assert_eq!(
        produce_fold(
            &source,
            1,
            BOOLEAN,
            &environment,
            &catalog(&environment),
            budget()
        )
        .unwrap_err(),
        ConstantBooleanError::SourceMismatch
    );
    // An environment for a different target than the plan's.
    let wrong_environment =
        baseline_target_register_environment(NativeTarget::macos_arm64()).unwrap();
    assert_eq!(
        produce_fold(
            &source,
            0,
            BOOLEAN,
            &wrong_environment,
            &catalog(&environment),
            budget()
        )
        .unwrap_err(),
        ConstantBooleanError::SourceMismatch
    );
    // A catalog bound to a foreign target cannot attest this plan's
    // effect surface even where the constraint keys coincide.
    let foreign_environment =
        baseline_target_register_environment(NativeTarget::linux_arm64()).unwrap();
    assert_eq!(
        produce_fold(
            &source,
            0,
            BOOLEAN,
            &environment,
            &catalog(&foreign_environment),
            budget()
        )
        .unwrap_err(),
        ConstantBooleanError::EffectSurfaceMismatch
    );
}

/// The emitted boolean shape is a single plain `[def]` with no unit traffic
/// of its own beyond the flag uses the fold consumes.
#[test]
fn malformed_boolean_shapes_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A second operand is not the emitted `[def]` shape.
    let extra_operand = mutated(target, |function, _| {
        let operand = function.blocks[0].instructions[3].operands[0];
        function.blocks[0].instructions[3].operands.push(operand);
    });
    assert_eq!(
        fold(&extra_operand, &environment).unwrap_err(),
        ConstantBooleanError::UnsupportedInstruction
    );
    // A read operand is not the result definition.
    let reading = mutated(target, |function, _| {
        function.blocks[0].instructions[3].operands[0].access = RegisterOperandAccess::Use;
    });
    assert_eq!(
        fold(&reading, &environment).unwrap_err(),
        ConstantBooleanError::UnsupportedInstruction
    );
    // A pinned result stays an allocation-shaped instruction.
    let pinned = mutated(target, |function, _| {
        function.blocks[0].instructions[3].operands[0].fixed_view =
            Some(register_model::RegisterViewId(0));
    });
    assert_eq!(
        fold(&pinned, &environment).unwrap_err(),
        ConstantBooleanError::UnsupportedInstruction
    );
    // The boolean cannot carry implicit definitions the materialize row
    // would drop.
    let defining = mutated(target, |function, _| {
        function.blocks[0].instructions[3].implicit_defs =
            vec![register_model::RegisterUnitId(u16::MAX)];
    });
    assert_eq!(
        fold(&defining, &environment).unwrap_err(),
        ConstantBooleanError::UnsupportedInstruction
    );
    // Nor clobbers.
    let clobbering = mutated(target, |function, _| {
        function.blocks[0].instructions[3].clobbers =
            vec![register_model::RegisterUnitId(u16::MAX)];
    });
    assert_eq!(
        fold(&clobbering, &environment).unwrap_err(),
        ConstantBooleanError::UnsupportedInstruction
    );
    // The result register must exist in the roster.
    let missing = mutated(target, |function, _| {
        function
            .virtual_registers
            .retain(|entry| entry.id != OUTPUT);
    });
    assert_eq!(
        fold(&missing, &environment).unwrap_err(),
        ConstantBooleanError::UnsupportedUse
    );
}

/// The measured validation-step boundary: admission charges one step per
/// block plus one per instruction across the plan, then the admitted
/// function's scan twice for the producer lookups, then the flag walk —
/// the adjacency and partition-surface setup plus, per used unit, the
/// block-prefix and last-event scans, the bounded entry-set propagation,
/// and the flag-universe, materialize-surface, and producer-definition
/// lookups — thirty-two for the four-instruction fixture on x86-64,
/// thirty-seven once a fifth instruction trails the boolean — so the
/// exact count admits the fold on both the proposal and the independent
/// replay path while one step below rejects both.
#[test]
fn measured_validation_step_boundary_admits_and_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A fifth body instruction extends the plan-wide and producer scans
    // and every term the flag walk derives from the function scan: (1
    // block + 5 instructions) once for the plan, twice for the producers,
    // and the walk bound of setup 2 plus a per-unit 17 — thirty-seven
    // steps.
    let wider = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions.push(instruction(
            EXTRA,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[PARAM, OUTPUT],
        ));
    });
    // x86-64 publishes one flag unit, so the walk adds the flag-universe,
    // materialize-surface, and producer-definition lookups the descriptor
    // binding charges: the fixture costs 15 for the plan and producer
    // scans plus a per-unit 15 (prefix scan 5, block count 1, pops 7,
    // surfaces 1 + 0 + 1) and a walk setup of 2 — 32; the wider plan's
    // per-unit 17 and setup 2 on scans of 18 give 37.
    for (source, exact_steps) in [
        (
            fixture(
                target,
                IntegerValue::Unsigned(3),
                IntegerValue::Unsigned(5),
                SelectedInstructionKind::MaterializeBooleanEqual,
            ),
            32u64,
        ),
        (wider, 37u64),
    ] {
        let exact = measured_step_budget(exact_steps);
        let result = fold_selected_constant_boolean(&source, BOOLEAN, &environment, exact).unwrap();
        validate_constant_boolean_fold(
            &source,
            BOOLEAN,
            &environment,
            exact,
            result.transformed().clone(),
        )
        .unwrap();
        let starved = measured_step_budget(exact_steps - 1);
        assert_eq!(
            fold_selected_constant_boolean(&source, BOOLEAN, &environment, starved).unwrap_err(),
            ConstantBooleanError::WorkBudgetExceeded
        );
        assert_eq!(
            validate_constant_boolean_fold(
                &source,
                BOOLEAN,
                &environment,
                starved,
                result.transformed().clone(),
            )
            .unwrap_err(),
            ConstantBooleanError::WorkBudgetExceeded
        );
    }
}

/// Replay rejects any drift from the reconstructed materialize form, in the
/// rewritten slot or anywhere else in the plan.
#[test]
fn replay_rejects_anything_but_the_exact_form() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(
        target,
        IntegerValue::Unsigned(3),
        IntegerValue::Unsigned(5),
        SelectedInstructionKind::MaterializeBooleanEqual,
    );
    let result = fold(&source, &environment).unwrap();
    for mutation in 0..10 {
        let mut proposed = result.transformed().clone();
        let function = &mut proposed.functions[0];
        match mutation {
            // The wrong result register.
            0 => {
                function.blocks[0].instructions[3].operands[0].virtual_register = PARAM;
            }
            // The wrong folded constant.
            1 => {
                function.blocks[0].instructions[3].kind = SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(1),
                };
            }
            // The boolean form must not survive in the slot.
            2 => {
                function.blocks[0].instructions[3].kind =
                    SelectedInstructionKind::MaterializeBooleanEqual;
            }
            // A different instruction kind carrying the constant.
            3 => {
                function.blocks[0].instructions[3].kind = SelectedInstructionKind::CopyI64;
            }
            // A different instruction id on the materialization.
            4 => function.blocks[0].instructions[3].id = COMPARE,
            // The boolean's provenance must survive intact.
            5 => {
                function.blocks[0].instructions[3]
                    .provenance
                    .values
                    .push(ValueId::new(9).unwrap());
            }
            // The retained compare must remain untouched.
            6 => {
                function.blocks[0].instructions[2].kind = SelectedInstructionKind::CompareI64Zero;
            }
            // A phantom flag use must not linger on the materialize.
            7 => {
                function.blocks[0].instructions[3]
                    .implicit_uses
                    .push(register_model::RegisterUnitId(0));
            }
            // An unrelated register must stay identical.
            8 => function.virtual_registers[3].scalar_type = ScalarType::Boolean,
            // A fabricated roster row has no source counterpart.
            9 => {
                function
                    .memory_accesses
                    .push(selected_instructions::SelectedMemoryAccess {
                        instruction: BOOLEAN,
                        origin: selected_instructions::SelectedMemoryAccessOrigin::Operation(
                            OperationId::new(4).unwrap(),
                        ),
                        place: semantic_vocabulary::PlaceId::new(1).unwrap(),
                        byte_offset: 0,
                        byte_count: 8,
                        role: selected_instructions::SelectedMemoryAccessRole::ReadPlace,
                    });
            }
            _ => unreachable!(),
        }
        assert!(
            validate_constant_boolean_fold(&source, BOOLEAN, &environment, budget(), proposed)
                .is_err(),
            "mutation {mutation}"
        );
    }
}

/// The declared `CompareI64Immediate` producer pairs fold the same
/// grammar: `literal - immediate` needs no second producer.
#[test]
fn immediate_compare_folds() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let row = environment
            .constraint(environment.selected_keys().compare_i64_immediate)
            .unwrap();
        let mut compare = instruction(
            COMPARE,
            SelectedInstructionKind::CompareI64Immediate {
                immediate: IntegerValue::Unsigned(3),
            },
            row,
            &[LITA],
        );
        compare.provenance.operations = vec![OperationId::new(7).unwrap()];
        function.blocks[0].instructions[2] = compare;
        // `3 - 3` is zero: equality holds.
        function.blocks[0].instructions[0].kind = SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(3),
        };
    });
    let result = fold(&source, &environment).unwrap();
    materialized(&result, 0, 3, 1);
    replay(&source, &environment, result.transformed().clone()).unwrap();
}

/// `register - register` is zero on every lane: the flag state is
/// constant whatever the register holds, so no literal producer is needed
/// at all — here the compared register's producer is a copy of an entry
/// parameter, which could never witness a literal.
#[test]
fn same_register_compare_folds_without_literals() {
    let target = NativeTarget::linux_arm64();
    let environment = baseline_target_register_environment(target).unwrap();
    for (kind, expected) in [
        (SelectedInstructionKind::MaterializeBooleanEqual, 1u64),
        (SelectedInstructionKind::MaterializeBooleanU64LessThan, 0),
        (SelectedInstructionKind::MaterializeBooleanI64LessThan, 0),
        (SelectedInstructionKind::MaterializeBooleanU64LessOrEqual, 1),
        (SelectedInstructionKind::MaterializeBooleanI64LessOrEqual, 1),
    ] {
        let source = mutated(target, |function, environment| {
            let copy = environment
                .constraint(environment.selected_keys().copy_i64)
                .unwrap();
            let boolean = environment
                .constraint(environment.selected_keys().materialize_boolean)
                .unwrap();
            function.blocks[0].instructions[0] = instruction(
                MATERIALIZE_A,
                SelectedInstructionKind::CopyI64,
                copy,
                &[PARAM, LITA],
            );
            function.blocks[0].instructions[2].operands[0].virtual_register = LITA;
            function.blocks[0].instructions[2].operands[1].virtual_register = LITA;
            function.blocks[0].instructions[3] = instruction(BOOLEAN, kind, boolean, &[OUTPUT]);
        });
        // `r - r` is zero: equality and both less-or-equals publish one,
        // both strict less-thans publish zero.
        let result = fold(&source, &environment).unwrap();
        materialized(&result, 0, 3, expected);
    }
}

/// A unit whose reaching walk finds no compare at all — the flag state
/// flows in from the function entry's unknown condition state — refuses.
#[test]
fn cross_block_entry_reaching_refuses() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        // The entry block drops its compare: no flag event precedes the
        // consumer anywhere.
        function.blocks[0].instructions.remove(2);
    });
    assert_eq!(
        fold(&source, &environment).unwrap_err(),
        ConstantBooleanError::UnsupportedUse
    );
}

/// A clobber carried by the terminator between blocks ends the flag reach
/// the same way an in-block clobber does.
#[test]
fn terminator_flag_clobber_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let flags = function.blocks[0].instructions[2].implicit_defs.clone();
        let consumer = function.blocks[0].instructions.remove(3);
        let terminal_row = environment
            .constraint(environment.selected_keys().return_unit)
            .unwrap()
            .clone();
        let mut jump = jump_terminator(environment, 6, 1, 2);
        let SelectedTerminator::Jump {
            instruction: jumped,
            ..
        } = &mut jump
        else {
            unreachable!()
        };
        jumped.clobbers = flags;
        function.blocks[0].terminator = jump;
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(1),
            origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
            instructions: vec![consumer],
            terminator: SelectedTerminator::Return {
                instruction: instruction(
                    SelectedInstructionId(8),
                    SelectedInstructionKind::ReturnUnit,
                    &terminal_row,
                    &[],
                ),
                psi_return_edge: EdgeId::new(3).unwrap(),
            },
        });
    });
    assert_eq!(
        fold(&source, &environment).unwrap_err(),
        ConstantBooleanError::UnsupportedUse
    );
}

/// A used unit outside the flag universe must lie in the materialize
/// row's implicit surface: `MaterializeI64` reads nothing at all, so any
/// non-flag observation refuses under the declared unit-flow axis.
#[test]
fn non_flag_use_outside_materialize_surface_refuses() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let foreign = mutated(target, |function, _| {
        // Unit 998 is neither a compare-published flag unit nor anything
        // the `MaterializeI64` row reads.
        function.blocks[0].instructions[3]
            .implicit_uses
            .push(register_model::RegisterUnitId(998));
    });
    assert_eq!(
        fold(&foreign, &environment).unwrap_err(),
        ConstantBooleanError::UnsupportedUse
    );
}

/// The producer the flag uses resolve to must be a declared
/// condition-state producer: a non-compare instruction that publishes the
/// same flag units — here a materialize carrying flag definitions as a
/// forged side channel — refuses.
#[test]
fn non_compare_producer_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let materialize_row = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap()
            .clone();
        // A hand-built non-producer record that publishes the compare's
        // flag units but names no declared producer kind.
        let mut imposter = instruction(
            COMPARE,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(0),
            },
            &materialize_row,
            &[LITB],
        );
        imposter.implicit_defs = function.blocks[0].instructions[2].implicit_defs.clone();
        function.blocks[0].instructions[2] = imposter;
    });
    assert_eq!(
        fold(&source, &environment).unwrap_err(),
        ConstantBooleanError::UnsupportedProducer
    );
}

/// Non-literal and multiply-defined operand registers refuse: the
/// constant guarantee must hold wherever the compare could read the
/// register.
#[test]
fn operand_producers_refuse() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // An entry parameter has no producing instruction.
    let parameter = mutated(target, |function, _| {
        function.blocks[0].instructions[2].operands[1].virtual_register = PARAM;
    });
    assert_eq!(
        fold(&parameter, &environment).unwrap_err(),
        ConstantBooleanError::UnsupportedProducer
    );
    // A copy producer is not the literal materialization.
    let copy_producer = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        function.blocks[0].instructions[1] = instruction(
            MATERIALIZE_B,
            SelectedInstructionKind::CopyI64,
            copy,
            &[PARAM, LITB],
        );
    });
    assert_eq!(
        fold(&copy_producer, &environment).unwrap_err(),
        ConstantBooleanError::UnsupportedProducer
    );
    // A second definition anywhere in the function — including a
    // terminator-carried one — breaks the unique-producer guarantee.
    let second_definition = mutated(target, |function, environment| {
        let materialize_row = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap()
            .clone();
        let carried = instruction(
            SelectedInstructionId(10),
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(0),
            },
            &materialize_row,
            &[LITB],
        );
        let SelectedTerminator::Return {
            instruction: slot, ..
        } = &mut function.blocks[0].terminator
        else {
            unreachable!()
        };
        *slot = carried;
    });
    assert_eq!(
        fold(&second_definition, &environment).unwrap_err(),
        ConstantBooleanError::UnsupportedProducer
    );
}

/// The bound catalog must describe the plan's target and the
/// environment's constraint inventory: a catalog built for another
/// target — or a record whose constraint key no declaration binds to its
/// semantic — cannot attest the effect relationship.
#[test]
fn effect_surface_mismatches_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(
        target,
        IntegerValue::Unsigned(3),
        IntegerValue::Unsigned(5),
        SelectedInstructionKind::MaterializeBooleanEqual,
    );
    // A catalog bound to a different target and constraint inventory.
    let foreign_environment =
        baseline_target_register_environment(NativeTarget::linux_arm64()).unwrap();
    let foreign_catalog = catalog(&foreign_environment);
    assert_eq!(
        produce_fold(
            &source,
            0,
            BOOLEAN,
            &environment,
            &foreign_catalog,
            budget()
        )
        .unwrap_err(),
        ConstantBooleanError::EffectSurfaceMismatch
    );
    // A record bound to the materialize row's constraint key has no
    // catalog declaration pairing it with the boolean semantic.
    let wrong_row = mutated(target, |function, environment| {
        let materialize_row = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap();
        function.blocks[0].instructions[3].constraint = materialize_row.key;
    });
    assert_eq!(
        fold(&wrong_row, &environment).unwrap_err(),
        ConstantBooleanError::EffectSurfaceMismatch
    );
    // The same defects refuse on the replay path: the proposed
    // materialization cannot carry the fold the bound catalog never
    // declared.
    let result = fold(&source, &environment).unwrap();
    assert_eq!(
        replay_fold(
            &source,
            0,
            BOOLEAN,
            &environment,
            &foreign_catalog,
            budget(),
            result.transformed().clone()
        )
        .unwrap_err(),
        ConstantBooleanError::EffectSurfaceMismatch
    );
    let mut detached = result.transformed().clone();
    detached.functions = detached.functions.iter().cloned().collect();
    let effect_catalog = catalog(&environment);
    assert_eq!(
        replay_fold(
            &wrong_row,
            0,
            BOOLEAN,
            &environment,
            &effect_catalog,
            budget(),
            detached
        )
        .unwrap_err(),
        ConstantBooleanError::EffectSurfaceMismatch
    );
}

/// The two flag consumers compose on the same published compare: the
/// materialization folds to its literal while a conditional-branch
/// terminator keeps reading the compare's flags — and the executed
/// constant-branch fold then accepts this fold's validated output.
#[test]
fn validated_results_compose_with_constant_branch() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let branch_row = environment
            .constraint(environment.selected_keys().conditional_branch)
            .unwrap()
            .clone();
        let terminal_row = environment
            .constraint(environment.selected_keys().return_unit)
            .unwrap()
            .clone();
        // The consumer stays in the entry block; the terminator becomes a
        // conditional branch on the same flags to two returning blocks.
        function.blocks[0].terminator = SelectedTerminator::ConditionalBranch {
            instruction: instruction(
                SelectedInstructionId(6),
                SelectedInstructionKind::ConditionalBranchNonZero,
                &branch_row,
                &[],
            ),
            when_nonzero: arm(SelectedBlockId(1), 2, 2),
            when_zero: arm(SelectedBlockId(2), 3, 3),
        };
        function.blocks.push(returning(1, 8, 4, &terminal_row));
        function.blocks.push(returning(2, 9, 5, &terminal_row));
    });
    // The boolean materialization folds first; the constant-branch fold
    // then consumes the same compare from the transformed plan.
    let materialization_fold = fold(&source, &environment).unwrap();
    materialized(&materialization_fold, 0, 3, 0);
    let result = crate::rewrites::fold_selected_constant_branch(
        &materialization_fold,
        0,
        SelectedInstructionId(6),
        &environment,
        &catalog(&environment),
        budget(),
    )
    .unwrap();
    assert_eq!(
        result.receipt().source_selected(),
        materialization_fold.receipt().transformed_selected()
    );
    // `3 - 5` is nonzero: the branch decided to the nonzero arm.
    let SelectedTerminator::Jump { successor, .. } =
        &result.transformed().functions[0].blocks[0].terminator
    else {
        panic!("expected a Jump terminator");
    };
    assert_eq!(successor.block, SelectedBlockId(1));
}

/// A proposal that retains the source instruction is not a rewritten
/// fold; replay refuses it alongside the drifted proposals the other
/// replay test exercises.
#[test]
fn replay_rejects_the_unmodified_source() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(
        target,
        IntegerValue::Unsigned(3),
        IntegerValue::Unsigned(5),
        SelectedInstructionKind::MaterializeBooleanEqual,
    );
    assert_eq!(
        replay(&source, &environment, source.transformed().clone()).unwrap_err(),
        ConstantBooleanError::ReplayMismatch
    );
}
