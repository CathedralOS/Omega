use super::{
    ConstantBooleanError, ConstantBooleanReceipt, ValidatedConstantBoolean,
    fold_selected_constant_boolean, validate_constant_boolean_fold,
};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::test_support::{budget, instruction, measured_step_budget};
use optimization_core::OptimizationUnitIdentity;
use optimization_unit::ValueDefinitionSite;
use register_environment::baseline_target_register_environment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedFunction, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionPlan, SelectedTerminator, VirtualRegister,
    VirtualRegisterId, VirtualRegisterOrigin,
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
    fold_selected_constant_boolean(source, 0, BOOLEAN, environment, budget())
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
                0,
                BOOLEAN,
                &environment,
                budget(),
                result.transformed().clone(),
            )
            .unwrap();
            // A detached, separately allocated proposal replays by content.
            let mut detached = result.transformed().clone();
            detached.functions = detached.functions.iter().cloned().collect();
            validate_constant_boolean_fold(&source, 0, BOOLEAN, &environment, budget(), detached)
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
        fold_selected_constant_boolean(&source, 0, COMPARE, &environment, budget()).unwrap_err(),
        ConstantBooleanError::UnsupportedInstruction
    );
    // Neither is a plain materialization.
    assert_eq!(
        fold_selected_constant_boolean(&source, 0, MATERIALIZE_A, &environment, budget())
            .unwrap_err(),
        ConstantBooleanError::UnsupportedInstruction
    );
    // An absent instruction id cannot be located.
    assert_eq!(
        fold_selected_constant_boolean(
            &source,
            0,
            SelectedInstructionId(99),
            &environment,
            budget()
        )
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
        fold_selected_constant_boolean(&source, 1, BOOLEAN, &environment, budget()).unwrap_err(),
        ConstantBooleanError::SourceMismatch
    );
    // An environment for a different target than the plan's.
    let wrong_environment =
        baseline_target_register_environment(NativeTarget::macos_arm64()).unwrap();
    assert_eq!(
        fold_selected_constant_boolean(&source, 0, BOOLEAN, &wrong_environment, budget())
            .unwrap_err(),
        ConstantBooleanError::SourceMismatch
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
/// the adjacency setup plus, per used unit, the block-prefix and
/// last-event scans and the bounded entry-set propagation — twenty-nine
/// for the four-instruction fixture on x86-64, thirty-four once a fifth
/// instruction trails the boolean — so the exact count admits the fold on
/// both the proposal and the independent replay path while one step below
/// rejects both.
#[test]
fn measured_validation_step_boundary_admits_and_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A fifth body instruction extends the plan-wide and producer scans
    // and every term the flag walk derives from the function scan: (1
    // block + 5 instructions) once for the plan, twice for the producers,
    // and the walk bound of setup 1 plus a per-unit 15 — thirty-four
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
    for (source, exact_steps) in [
        (
            fixture(
                target,
                IntegerValue::Unsigned(3),
                IntegerValue::Unsigned(5),
                SelectedInstructionKind::MaterializeBooleanEqual,
            ),
            29u64,
        ),
        (wider, 34u64),
    ] {
        let exact = measured_step_budget(exact_steps);
        let result =
            fold_selected_constant_boolean(&source, 0, BOOLEAN, &environment, exact).unwrap();
        validate_constant_boolean_fold(
            &source,
            0,
            BOOLEAN,
            &environment,
            exact,
            result.transformed().clone(),
        )
        .unwrap();
        let starved = measured_step_budget(exact_steps - 1);
        assert_eq!(
            fold_selected_constant_boolean(&source, 0, BOOLEAN, &environment, starved).unwrap_err(),
            ConstantBooleanError::WorkBudgetExceeded
        );
        assert_eq!(
            validate_constant_boolean_fold(
                &source,
                0,
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
            validate_constant_boolean_fold(&source, 0, BOOLEAN, &environment, budget(), proposed)
                .is_err(),
            "mutation {mutation}"
        );
    }
}
