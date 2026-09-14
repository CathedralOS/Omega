use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget};
use optimization_unit::ValueDefinitionSite;
use register_environment::baseline_target_register_environment;
use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan, SelectedOperand,
    SelectedTerminator, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, MachineId,
    OperationId, PlaceId, ScalarType, ValueId,
};
use target::NativeTarget;
use target_operations_to_selected_instructions::selected_instruction_plan_identity;
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use super::*;
use crate::ValidatedSelectedAnalysis;

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

const LOAD: SelectedInstructionId = SelectedInstructionId(2);
const MATERIALIZE: SelectedInstructionId = SelectedInstructionId(3);
const COMPARE: SelectedInstructionId = SelectedInstructionId(4);
const BOOLEAN: SelectedInstructionId = SelectedInstructionId(5);
const POINTER: VirtualRegisterId = VirtualRegisterId(0);
const LEFT: VirtualRegisterId = VirtualRegisterId(1);
const LITERAL: VirtualRegisterId = VirtualRegisterId(2);
const OUTPUT: VirtualRegisterId = VirtualRegisterId(3);
const SPARE: VirtualRegisterId = VirtualRegisterId(4);

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
/// `r1 = load8 r0; r2 = <producer>; compare r1, r2; r3 = boolean; return`.
fn fixture(
    target: NativeTarget,
    producer_kind: SelectedInstructionKind,
    producer_key: register_model::RegisterConstraintKey,
    producer_registers: &[VirtualRegisterId],
) -> ValidatedLiteralCompare {
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let load = environment.constraint(keys.load8.unwrap()).unwrap();
    let compare = environment.constraint(keys.compare_i64).unwrap();
    let boolean = environment.constraint(keys.materialize_boolean).unwrap();
    let producer_row = environment.constraint(producer_key).unwrap();
    let terminal_row = environment.constraint(keys.return_unit).unwrap();
    let class = compare.operands[0].class;
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let registers = vec![
        VirtualRegister {
            id: POINTER,
            scalar_type,
            class,
            origin: VirtualRegisterOrigin::EntryParameter {
                source_value: ValueId::new(1).unwrap(),
                parameter_index: 0,
            },
            definition_site: Some(ValueDefinitionSite::FunctionParameter(0)),
            entry_fixed_view: None,
        },
        register(
            LEFT,
            scalar_type,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: LOAD,
                source_value: ValueId::new(2).unwrap(),
            },
        ),
        register(
            LITERAL,
            scalar_type,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: MATERIALIZE,
                source_value: ValueId::new(3).unwrap(),
            },
        ),
        register(
            OUTPUT,
            scalar_type,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: BOOLEAN,
                source_value: ValueId::new(4).unwrap(),
            },
        ),
    ];
    let mut compare_instruction = instruction(
        COMPARE,
        SelectedInstructionKind::CompareI64,
        compare,
        &[LEFT, LITERAL],
    );
    compare_instruction.provenance.operations = vec![OperationId::new(7).unwrap()];
    compare_instruction.provenance.values = vec![ValueId::new(3).unwrap()];
    let instructions = vec![
        instruction(
            LOAD,
            SelectedInstructionKind::Load8 { byte_offset: 0 },
            load,
            &[POINTER, LEFT],
        ),
        instruction(MATERIALIZE, producer_kind, producer_row, producer_registers),
        compare_instruction,
        instruction(
            BOOLEAN,
            SelectedInstructionKind::MaterializeBooleanEqual,
            boolean,
            &[OUTPUT],
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
    ValidatedLiteralCompare {
        receipt: LiteralCompareReceipt {
            source_selected: identity,
            transformed_selected: identity,
            optimization_unit: OptimizationUnitIdentity::from_bytes([2; 32]),
            fuel_schedule: plan.fuel_schedule,
        },
        transformed: std::sync::Arc::new(plan),
    }
}

fn keys(
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> selected_instructions::SelectedConstraintKeys {
    environment.selected_keys()
}

fn materialize_fixture(target: NativeTarget, value: IntegerValue) -> ValidatedLiteralCompare {
    let environment = baseline_target_register_environment(target).unwrap();
    fixture(
        target,
        SelectedInstructionKind::MaterializeI64 { value },
        keys(&environment).materialize_i64,
        &[LITERAL],
    )
}

/// A literal of zero selects the dedicated test form on every target.
#[test]
fn zero_literal_selects_the_test_form() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = materialize_fixture(target, IntegerValue::Unsigned(0));
        let result =
            fold_selected_literal_compare(&source, 0, COMPARE, &environment, budget()).unwrap();
        let rewritten = &result.transformed().functions[0].blocks[0].instructions[2];
        assert_eq!(rewritten.id, COMPARE);
        assert_eq!(rewritten.kind, SelectedInstructionKind::CompareI64Zero);
        assert_eq!(rewritten.constraint, keys(&environment).compare_i64_zero);
        assert_eq!(rewritten.operands.len(), 1);
        assert_eq!(rewritten.operands[0].operand, 0);
        assert_eq!(rewritten.operands[0].virtual_register, LEFT);
        assert_eq!(rewritten.operands[0].access, RegisterOperandAccess::Use);
        // The condition-state surface is preserved bit-identically: the zero
        // form publishes exactly the flag definitions the compare carried.
        assert_eq!(
            rewritten.implicit_defs,
            source.transformed().functions[0].blocks[0].instructions[2].implicit_defs
        );
        // The compare's provenance moves to the selected form.
        assert_eq!(
            rewritten.provenance,
            source.transformed().functions[0].blocks[0].instructions[2].provenance
        );
        assert_eq!(
            result.receipt().source_selected(),
            source.selected_identity()
        );
        assert_eq!(
            result.receipt().transformed_selected(),
            selected_instruction_plan_identity(result.transformed())
        );
        validate_literal_compare_fold(
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
        validate_literal_compare_fold(&source, 0, COMPARE, &environment, budget(), detached)
            .unwrap();
    }
}

/// A nonzero literal inside the shared U12 bound selects the immediate form.
#[test]
fn positive_literal_selects_the_immediate_form() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for value in [1u64, 5, 2048, 4095] {
        let source = materialize_fixture(target, IntegerValue::Unsigned(u128::from(value)));
        let result =
            fold_selected_literal_compare(&source, 0, COMPARE, &environment, budget()).unwrap();
        let rewritten = &result.transformed().functions[0].blocks[0].instructions[2];
        assert_eq!(
            rewritten.kind,
            SelectedInstructionKind::CompareI64Immediate {
                immediate: IntegerValue::Unsigned(u128::from(value)),
            }
        );
        assert_eq!(
            rewritten.constraint,
            keys(&environment).compare_i64_immediate
        );
        assert_eq!(rewritten.operands.len(), 1);
        assert_eq!(rewritten.operands[0].virtual_register, LEFT);
    }
}

/// The literal admission table: the shared twelve-bit unsigned bound both
/// encoders enforce, with signed literals folding only at nonnegative values.
#[test]
fn literal_admission_table() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let cases: &[(
        IntegerValue,
        Result<SelectedInstructionKind, LiteralCompareError>,
    )] = &[
        (
            IntegerValue::Unsigned(0),
            Ok(SelectedInstructionKind::CompareI64Zero),
        ),
        (
            IntegerValue::Unsigned(4095),
            Ok(SelectedInstructionKind::CompareI64Immediate {
                immediate: IntegerValue::Unsigned(4095),
            }),
        ),
        (
            IntegerValue::Unsigned(4096),
            Err(LiteralCompareError::UnsupportedLiteral),
        ),
        (
            IntegerValue::Unsigned(u128::from(u64::MAX)),
            Err(LiteralCompareError::UnsupportedLiteral),
        ),
        (
            IntegerValue::Unsigned(u128::from(u64::MAX) + 1),
            Err(LiteralCompareError::UnsupportedLiteral),
        ),
        (
            IntegerValue::Unsigned(u128::MAX),
            Err(LiteralCompareError::UnsupportedLiteral),
        ),
        (
            IntegerValue::Signed(0),
            Ok(SelectedInstructionKind::CompareI64Zero),
        ),
        (
            IntegerValue::Signed(4095),
            Ok(SelectedInstructionKind::CompareI64Immediate {
                immediate: IntegerValue::Unsigned(4095),
            }),
        ),
        (
            IntegerValue::Signed(4096),
            Err(LiteralCompareError::UnsupportedLiteral),
        ),
        (
            IntegerValue::Signed(-1),
            Err(LiteralCompareError::UnsupportedLiteral),
        ),
        (
            IntegerValue::Signed(-4096),
            Err(LiteralCompareError::UnsupportedLiteral),
        ),
    ];
    for (value, expected) in cases {
        let source = materialize_fixture(target, *value);
        let result = fold_selected_literal_compare(&source, 0, COMPARE, &environment, budget());
        match expected {
            Ok(kind) => {
                let rewritten =
                    &result.as_ref().unwrap().transformed().functions[0].blocks[0].instructions[2];
                assert_eq!(&rewritten.kind, kind, "{value:?}");
            }
            Err(error) => assert_eq!(result.unwrap_err(), *error, "{value:?}"),
        }
    }
}

/// A literal in the left operand cannot fold: the immediate forms always
/// subtract the encoded literal from the register operand.
#[test]
fn left_operand_literal_cannot_fold() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        function.blocks[0].instructions[2].operands[0].virtual_register = LITERAL;
        function.blocks[0].instructions[2].operands[1].virtual_register = LEFT;
    });
    assert_eq!(
        fold(&source, &environment).unwrap_err(),
        LiteralCompareError::UnsupportedProducer
    );
}

/// The same register on both sides still folds: `v - v` and the surviving
/// `register - v` compute the identical condition state.
#[test]
fn same_register_on_both_sides_folds() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        function.blocks[0].instructions[2].operands[0].virtual_register = LITERAL;
    });
    let result = fold(&source, &environment).unwrap();
    let rewritten = &result.transformed().functions[0].blocks[0].instructions[2];
    assert_eq!(rewritten.kind, SelectedInstructionKind::CompareI64Zero);
    assert_eq!(rewritten.operands[0].virtual_register, LITERAL);
}

/// Other readers of the literal register do not block the fold; the
/// materialization is retained for them.
#[test]
fn other_uses_of_the_literal_survive() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let copy = environment
        .constraint(keys(&environment).copy_i64)
        .unwrap()
        .clone();
    let source = mutated(target, |function, _| {
        function.virtual_registers.push(register(
            SPARE,
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
            function.virtual_registers[0].class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(7),
                source_value: ValueId::new(5).unwrap(),
            },
        ));
        let mut consumer = instruction(
            SelectedInstructionId(7),
            SelectedInstructionKind::CopyI64,
            &copy,
            &[LITERAL, SPARE],
        );
        consumer.provenance.values = vec![ValueId::new(6).unwrap()];
        function.blocks[0].instructions.insert(3, consumer);
    });
    let result = fold(&source, &environment).unwrap();
    let transformed = result.transformed();
    // The materialization remains for the surviving reader.
    assert_eq!(
        transformed.functions[0].blocks[0].instructions[1].kind,
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(0)
        }
    );
    assert_eq!(
        transformed.functions[0].blocks[0].instructions[2].kind,
        SelectedInstructionKind::CompareI64Zero
    );
    assert_eq!(
        transformed.functions[0].blocks[0].instructions[3].kind,
        SelectedInstructionKind::CopyI64
    );
}

/// A validated fold is itself a sealed analysis source: a second literal
/// compare folds on the transformed program without re-entering the stage.
#[test]
fn validated_results_compose() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let compare = environment
        .constraint(keys(&environment).compare_i64)
        .unwrap()
        .clone();
    let source = mutated(target, |function, _| {
        let mut second = instruction(
            SelectedInstructionId(7),
            SelectedInstructionKind::CompareI64,
            &compare,
            &[LEFT, LITERAL],
        );
        second.provenance.operations = vec![OperationId::new(8).unwrap()];
        function.blocks[0].instructions.insert(3, second);
    });
    let first = fold(&source, &environment).unwrap();
    let second =
        fold_selected_literal_compare(&first, 0, SelectedInstructionId(7), &environment, budget())
            .unwrap();
    let instructions = &second.transformed().functions[0].blocks[0].instructions;
    assert_eq!(
        instructions[2].kind,
        SelectedInstructionKind::CompareI64Zero
    );
    assert_eq!(
        instructions[3].kind,
        SelectedInstructionKind::CompareI64Zero
    );
    assert_eq!(
        second.receipt().source_selected(),
        first.receipt().transformed_selected()
    );
}

/// Edit the single fixture function, then refresh the receipt identities so
/// the mutated plan is a well-formed analysis source.
fn mutated(
    target: NativeTarget,
    edit: impl FnOnce(&mut SelectedFunction, &register_environment::ValidatedTargetRegisterEnvironment),
) -> ValidatedLiteralCompare {
    let environment = baseline_target_register_environment(target).unwrap();
    let mut source = materialize_fixture(target, IntegerValue::Unsigned(0));
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
    source: &ValidatedLiteralCompare,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> Result<ValidatedLiteralCompare, LiteralCompareError> {
    fold_selected_literal_compare(source, 0, COMPARE, environment, budget())
}

/// Only a `MaterializeI64` may witness the literal; every other producer
/// kind, and every extra definition site, refuses.
#[test]
fn producer_admission_table() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = keys(&environment);
    use SelectedInstructionKind::*;
    let cases: &[(
        SelectedInstructionKind,
        register_model::RegisterConstraintKey,
        &[VirtualRegisterId],
    )] = &[
        (CopyI64, keys.copy_i64, &[POINTER, LITERAL]),
        (
            Load8 { byte_offset: 0 },
            keys.load8.unwrap(),
            &[POINTER, LITERAL],
        ),
        (ZeroExtendU8, keys.copy_i64, &[POINTER, LITERAL]),
        (
            CompareI64Immediate {
                immediate: IntegerValue::Unsigned(3),
            },
            keys.compare_i64,
            &[POINTER, LITERAL],
        ),
        // A boolean materialization carries an implicit flag use the fold
        // would leave behind; it is not the integer literal producer.
        (
            MaterializeBooleanEqual,
            keys.materialize_boolean,
            &[LITERAL],
        ),
    ];
    for (producer_kind, producer_key, producer_registers) in cases {
        let source = fixture(target, *producer_kind, *producer_key, producer_registers);
        assert_eq!(
            fold(&source, &environment).unwrap_err(),
            LiteralCompareError::UnsupportedProducer,
            "{producer_kind:?}"
        );
    }
    // A second definition of the literal register — even through a UseDef
    // rewrite — breaks the wherever-read guarantee.
    let second_definition = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        let mut extra = instruction(
            SelectedInstructionId(8),
            SelectedInstructionKind::CopyI64,
            copy,
            &[POINTER, LITERAL],
        );
        extra.operands[1].access = RegisterOperandAccess::UseDef;
        function.blocks[0].instructions.insert(2, extra);
    });
    assert_eq!(
        fold(&second_definition, &environment).unwrap_err(),
        LiteralCompareError::UnsupportedProducer
    );
    // A definition carried by a terminator's instruction still counts.
    let terminator_definition = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        let carried = instruction(
            SelectedInstructionId(6),
            SelectedInstructionKind::CopyI64,
            copy,
            &[POINTER, LITERAL],
        );
        function.blocks[0].terminator = SelectedTerminator::Return {
            instruction: carried,
            psi_return_edge: EdgeId::new(1).unwrap(),
        };
    });
    assert_eq!(
        fold(&terminator_definition, &environment).unwrap_err(),
        LiteralCompareError::UnsupportedProducer
    );
    // A block-parameter or entry register has no producing instruction.
    let no_producer = mutated(target, |function, _| {
        function.blocks[0].instructions.remove(1);
        function.virtual_registers[2].origin = VirtualRegisterOrigin::BlockParameter {
            source_value: ValueId::new(3).unwrap(),
            block: SelectedBlockId(0),
            parameter_index: 0,
        };
    });
    assert_eq!(
        fold(&no_producer, &environment).unwrap_err(),
        LiteralCompareError::UnsupportedProducer
    );
    // A literal register absent from the roster is malformed.
    let missing_register = mutated(target, |function, _| {
        function
            .virtual_registers
            .retain(|entry| entry.id != LITERAL);
    });
    assert_eq!(
        fold(&missing_register, &environment).unwrap_err(),
        LiteralCompareError::UnsupportedProducer
    );
}

#[test]
fn malformed_compare_shapes_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A pinned use cannot ride into the rebuilt operand row.
    let pinned_use = mutated(target, |function, _| {
        function.blocks[0].instructions[2].operands[0].fixed_view =
            Some(register_model::RegisterViewId(0));
    });
    assert_eq!(
        fold(&pinned_use, &environment).unwrap_err(),
        LiteralCompareError::UnsupportedInstruction
    );
    // A tied operand stays an allocation-shaped instruction.
    let tied_use = mutated(target, |function, _| {
        function.blocks[0].instructions[2].operands[1].tied_to = Some(0);
    });
    assert_eq!(
        fold(&tied_use, &environment).unwrap_err(),
        LiteralCompareError::UnsupportedInstruction
    );
    // An implicit unit use would be silently dropped by the immediate form.
    let implicit = mutated(target, |function, environment| {
        let unit = environment
            .constraint(environment.selected_keys().materialize_boolean)
            .unwrap()
            .implicit_uses
            .first()
            .copied()
            .unwrap_or(register_model::RegisterUnitId(0));
        function.blocks[0].instructions[2].implicit_uses = vec![unit];
    });
    assert_eq!(
        fold(&implicit, &environment).unwrap_err(),
        LiteralCompareError::UnsupportedInstruction
    );
    // A clobber the immediate form does not declare cannot be preserved.
    let clobber = mutated(target, |function, _| {
        function.blocks[0].instructions[2].clobbers = vec![register_model::RegisterUnitId(0)];
    });
    assert_eq!(
        fold(&clobber, &environment).unwrap_err(),
        LiteralCompareError::UnsupportedInstruction
    );
    // The kind check rejects non-compare instructions.
    let not_compare = mutated(target, |function, _| {
        function.blocks[0].instructions[2].kind = SelectedInstructionKind::CopyI64;
    });
    assert_eq!(
        fold(&not_compare, &environment).unwrap_err(),
        LiteralCompareError::UnsupportedInstruction
    );
    // A third operand is not the emitted `[use, use]` shape.
    let extra_operand = mutated(target, |function, _| {
        let operand = function.blocks[0].instructions[2].operands[1];
        function.blocks[0].instructions[2].operands.push(operand);
    });
    assert_eq!(
        fold(&extra_operand, &environment).unwrap_err(),
        LiteralCompareError::UnsupportedInstruction
    );
    // A write operand is not a compare use.
    let writing = mutated(target, |function, _| {
        function.blocks[0].instructions[2].operands[1].access = RegisterOperandAccess::Def;
    });
    assert_eq!(
        fold(&writing, &environment).unwrap_err(),
        LiteralCompareError::UnsupportedInstruction
    );
    // The surviving input must exist in the register roster.
    let missing_input = mutated(target, |function, _| {
        function.virtual_registers.retain(|entry| entry.id != LEFT);
    });
    assert_eq!(
        fold(&missing_input, &environment).unwrap_err(),
        LiteralCompareError::UnsupportedUse
    );
}

/// The selected row must publish exactly the implicit surface the compare
/// carried, and the compare's declared row must match its operand shape.
#[test]
fn constraint_and_effect_surfaces_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A compare that published no flag definitions cannot gain them.
    let no_defs = mutated(target, |function, _| {
        function.blocks[0].instructions[2].implicit_defs = Vec::new();
    });
    assert_eq!(
        fold(&no_defs, &environment).unwrap_err(),
        LiteralCompareError::ConstraintMismatch
    );
    // An extra implicit definition the selected row does not publish cannot
    // be preserved.
    let extra_def = mutated(target, |function, _| {
        function.blocks[0].instructions[2]
            .implicit_defs
            .push(register_model::RegisterUnitId(u16::MAX));
    });
    assert_eq!(
        fold(&extra_def, &environment).unwrap_err(),
        LiteralCompareError::ConstraintMismatch
    );
    // A declared row that does not declare the `[use, use]` shape the
    // operands carry is malformed.
    let wrong_row = mutated(target, |function, environment| {
        function.blocks[0].instructions[2].constraint = environment.selected_keys().copy_i64;
    });
    assert_eq!(
        fold(&wrong_row, &environment).unwrap_err(),
        LiteralCompareError::ConstraintMismatch
    );
    // A roster class the row does not declare for operand zero rejects.
    let wrong_class = mutated(target, |function, _| {
        function.virtual_registers[1].class = register_model::RegisterClassId(u16::MAX);
    });
    assert_eq!(
        fold(&wrong_class, &environment).unwrap_err(),
        LiteralCompareError::ConstraintMismatch
    );
}

#[test]
fn source_identity_mismatches_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = materialize_fixture(target, IntegerValue::Unsigned(0));
    // A function index outside the plan.
    assert_eq!(
        fold_selected_literal_compare(&source, 1, COMPARE, &environment, budget()).unwrap_err(),
        LiteralCompareError::SourceMismatch
    );
    // An instruction id the function does not contain.
    assert_eq!(
        fold_selected_literal_compare(
            &source,
            0,
            SelectedInstructionId(99),
            &environment,
            budget()
        )
        .unwrap_err(),
        LiteralCompareError::SourceMismatch
    );
    // An environment for a different target than the plan's.
    let wrong_environment =
        baseline_target_register_environment(NativeTarget::macos_arm64()).unwrap();
    assert_eq!(
        fold_selected_literal_compare(&source, 0, COMPARE, &wrong_environment, budget())
            .unwrap_err(),
        LiteralCompareError::SourceMismatch
    );
}

#[test]
fn validation_budget_covers_the_producer_scan() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = materialize_fixture(target, IntegerValue::Unsigned(0));
    let tiny = OptimizationWorkBudget::new(100, 100, 4, 100, 100).unwrap();
    assert_eq!(
        fold_selected_literal_compare(&source, 0, COMPARE, &environment, tiny).unwrap_err(),
        LiteralCompareError::WorkBudgetExceeded
    );
}

#[test]
fn replay_rejects_anything_but_the_exact_form() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = materialize_fixture(target, IntegerValue::Unsigned(9));
    let result = fold(&source, &environment).unwrap();
    for mutation in 0..10 {
        let mut proposed = result.transformed().clone();
        let function = &mut proposed.functions[0];
        match mutation {
            // The wrong surviving input register.
            0 => {
                function.blocks[0].instructions[2].operands[0].virtual_register = POINTER;
            }
            // A different immediate than the materialized literal.
            1 => {
                function.blocks[0].instructions[2].kind =
                    SelectedInstructionKind::CompareI64Immediate {
                        immediate: IntegerValue::Unsigned(8),
                    };
            }
            // The dedicated test form is not the nine-literal's selection.
            2 => {
                function.blocks[0].instructions[2].kind = SelectedInstructionKind::CompareI64Zero;
            }
            // The register-register form must not survive in the slot.
            3 => {
                function.blocks[0].instructions[2].kind = SelectedInstructionKind::CompareI64;
            }
            // A different instruction id on the selected form.
            4 => function.blocks[0].instructions[2].id = LOAD,
            // The compare's provenance must survive intact.
            5 => {
                function.blocks[0].instructions[2]
                    .provenance
                    .values
                    .push(ValueId::new(9).unwrap());
            }
            // The retained producer must remain untouched.
            6 => {
                function.blocks[0].instructions[1].kind =
                    SelectedInstructionKind::Load16 { byte_offset: 0 };
            }
            // The dropped operand must not linger.
            7 => {
                let operand = function.blocks[0].instructions[2].operands[0];
                function.blocks[0].instructions[2].operands.push(operand);
            }
            // An unrelated register must stay identical.
            8 => function.virtual_registers[3].scalar_type = ScalarType::Boolean,
            // A fabricated roster row has no source counterpart.
            9 => {
                function
                    .memory_accesses
                    .push(selected_instructions::SelectedMemoryAccess {
                        instruction: COMPARE,
                        origin: selected_instructions::SelectedMemoryAccessOrigin::Operation(
                            OperationId::new(4).unwrap(),
                        ),
                        place: PlaceId::new(1).unwrap(),
                        byte_offset: 0,
                        byte_count: 8,
                        role: selected_instructions::SelectedMemoryAccessRole::ReadPlace,
                    });
            }
            _ => unreachable!(),
        }
        assert!(
            validate_literal_compare_fold(&source, 0, COMPARE, &environment, budget(), proposed)
                .is_err(),
            "mutation {mutation}"
        );
    }
}
