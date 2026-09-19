use crate::LiteralArithmeticError;
use crate::LiteralArithmeticReceipt;
use crate::ValidatedLiteralArithmetic;
use crate::ValidatedSelectedAnalysis;
use crate::fold_selected_literal_arithmetic;
use crate::validate_literal_arithmetic_fold;
use optimization_core::{
    AcceptedObligationFactIdentity, OptimizationUnitIdentity, OptimizationWorkBudget,
};
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
    ObligationId, OperationId, PlaceId, ScalarType, ValueId,
};
use target::NativeTarget;
use target_operations_to_selected_instructions::selected_instruction_plan_identity;
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

fn budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(100, 100, 1000, 100, 100).unwrap()
}

fn obligation() -> ObligationId {
    ObligationId::new(11).unwrap()
}

fn accepted_fact() -> AcceptedObligationFactIdentity {
    AcceptedObligationFactIdentity::from_bytes([7; 32])
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
const OPERATION: SelectedInstructionId = SelectedInstructionId(4);
const COPY: SelectedInstructionId = SelectedInstructionId(5);
const POINTER: VirtualRegisterId = VirtualRegisterId(0);
const LEFT: VirtualRegisterId = VirtualRegisterId(1);
const LITERAL: VirtualRegisterId = VirtualRegisterId(2);
const RESULT: VirtualRegisterId = VirtualRegisterId(3);
const OUTPUT: VirtualRegisterId = VirtualRegisterId(4);
const SPARE: VirtualRegisterId = VirtualRegisterId(5);

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

fn operation_kind(add: bool) -> SelectedInstructionKind {
    if add {
        SelectedInstructionKind::ExactAddI64 {
            obligation: obligation(),
            accepted_fact: accepted_fact(),
        }
    } else {
        SelectedInstructionKind::ExactSubtractI64 {
            obligation: obligation(),
            accepted_fact: accepted_fact(),
        }
    }
}

/// A raw selected-stage unit fixture, not a source/Terminal admission claim:
/// `r1 = load8 r0; r2 = <producer>; r3 = <op> r1, r2; r4 = copy r3; return`.
fn fixture(
    target: NativeTarget,
    add: bool,
    producer_kind: SelectedInstructionKind,
    producer_key: register_model::RegisterConstraintKey,
    producer_registers: &[VirtualRegisterId],
) -> ValidatedLiteralArithmetic {
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let load = environment.constraint(keys.load8.unwrap()).unwrap();
    let operation_row = environment
        .constraint(if add { keys.add_i64 } else { keys.subtract_i64 })
        .unwrap();
    let copy = environment.constraint(keys.copy_i64).unwrap();
    let producer_row = environment.constraint(producer_key).unwrap();
    let terminal_row = environment.constraint(keys.return_unit).unwrap();
    let class = operation_row.operands[0].class;
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
            RESULT,
            scalar_type,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: OPERATION,
                source_value: ValueId::new(4).unwrap(),
            },
        ),
        register(
            OUTPUT,
            scalar_type,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: COPY,
                source_value: ValueId::new(5).unwrap(),
            },
        ),
    ];
    let mut operation = instruction(
        OPERATION,
        operation_kind(add),
        operation_row,
        &[LEFT, LITERAL, RESULT],
    );
    operation.provenance.operations = vec![OperationId::new(7).unwrap()];
    operation.provenance.values = vec![
        ValueId::new(2).unwrap(),
        ValueId::new(3).unwrap(),
        ValueId::new(4).unwrap(),
    ];
    operation.provenance.obligations = vec![obligation()];
    let instructions = vec![
        instruction(
            LOAD,
            SelectedInstructionKind::Load8 { byte_offset: 0 },
            load,
            &[POINTER, LEFT],
        ),
        instruction(MATERIALIZE, producer_kind, producer_row, producer_registers),
        operation,
        instruction(
            COPY,
            SelectedInstructionKind::CopyI64,
            copy,
            &[RESULT, OUTPUT],
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
    ValidatedLiteralArithmetic {
        receipt: LiteralArithmeticReceipt {
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

fn materialize_fixture(
    target: NativeTarget,
    add: bool,
    value: IntegerValue,
) -> ValidatedLiteralArithmetic {
    let environment = baseline_target_register_environment(target).unwrap();
    fixture(
        target,
        add,
        SelectedInstructionKind::MaterializeI64 { value },
        keys(&environment).materialize_i64,
        &[LITERAL],
    )
}

/// A literal right operand selects the immediate add form on every target.
#[test]
fn right_literal_selects_the_immediate_add_form() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = materialize_fixture(target, true, IntegerValue::Unsigned(9));
        let result =
            fold_selected_literal_arithmetic(&source, 0, OPERATION, &environment, budget())
                .unwrap();
        let rewritten = &result.transformed().functions[0].blocks[0].instructions[2];
        assert_eq!(rewritten.id, OPERATION);
        assert_eq!(
            rewritten.kind,
            SelectedInstructionKind::ExactAddI64Immediate {
                immediate: IntegerValue::Unsigned(9),
                obligation: obligation(),
                accepted_fact: accepted_fact(),
            }
        );
        assert_eq!(rewritten.constraint, keys(&environment).add_i64_immediate);
        assert_eq!(rewritten.operands.len(), 2);
        assert_eq!(rewritten.operands[0].operand, 0);
        assert_eq!(rewritten.operands[0].virtual_register, LEFT);
        assert_eq!(rewritten.operands[0].access, RegisterOperandAccess::Use);
        assert_eq!(rewritten.operands[1].operand, 1);
        assert_eq!(rewritten.operands[1].virtual_register, RESULT);
        assert_eq!(rewritten.operands[1].access, RegisterOperandAccess::Def);
        // The operation's provenance moves to the selected form.
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
        validate_literal_arithmetic_fold(
            &source,
            0,
            OPERATION,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
        // A detached, separately allocated proposal replays by content.
        let mut detached = result.transformed().clone();
        detached.functions = detached.functions.iter().cloned().collect();
        validate_literal_arithmetic_fold(&source, 0, OPERATION, &environment, budget(), detached)
            .unwrap();
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
        Result<SelectedInstructionKind, LiteralArithmeticError>,
    )] = &[
        (
            IntegerValue::Unsigned(0),
            Ok(SelectedInstructionKind::ExactAddI64Immediate {
                immediate: IntegerValue::Unsigned(0),
                obligation: obligation(),
                accepted_fact: accepted_fact(),
            }),
        ),
        (
            IntegerValue::Unsigned(4095),
            Ok(SelectedInstructionKind::ExactAddI64Immediate {
                immediate: IntegerValue::Unsigned(4095),
                obligation: obligation(),
                accepted_fact: accepted_fact(),
            }),
        ),
        (
            IntegerValue::Unsigned(4096),
            Err(LiteralArithmeticError::UnsupportedLiteral),
        ),
        (
            IntegerValue::Unsigned(u128::from(u64::MAX)),
            Err(LiteralArithmeticError::UnsupportedLiteral),
        ),
        (
            IntegerValue::Unsigned(u128::from(u64::MAX) + 1),
            Err(LiteralArithmeticError::UnsupportedLiteral),
        ),
        (
            IntegerValue::Unsigned(u128::MAX),
            Err(LiteralArithmeticError::UnsupportedLiteral),
        ),
        (
            IntegerValue::Signed(0),
            Ok(SelectedInstructionKind::ExactAddI64Immediate {
                immediate: IntegerValue::Unsigned(0),
                obligation: obligation(),
                accepted_fact: accepted_fact(),
            }),
        ),
        (
            IntegerValue::Signed(4095),
            Ok(SelectedInstructionKind::ExactAddI64Immediate {
                immediate: IntegerValue::Unsigned(4095),
                obligation: obligation(),
                accepted_fact: accepted_fact(),
            }),
        ),
        (
            IntegerValue::Signed(4096),
            Err(LiteralArithmeticError::UnsupportedLiteral),
        ),
        (
            IntegerValue::Signed(-1),
            Err(LiteralArithmeticError::UnsupportedLiteral),
        ),
        (
            IntegerValue::Signed(-4096),
            Err(LiteralArithmeticError::UnsupportedLiteral),
        ),
    ];
    for (value, expected) in cases {
        let source = materialize_fixture(target, true, *value);
        let result =
            fold_selected_literal_arithmetic(&source, 0, OPERATION, &environment, budget());
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

/// A literal in the left operand of an add still folds: exact addition
/// commutes, so `literal + x` selects the same `x + literal` immediate form.
#[test]
fn left_literal_add_commutes_into_the_immediate_form() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = mutated(target, true, |function, _| {
            function.blocks[0].instructions[2].operands[0].virtual_register = LITERAL;
            function.blocks[0].instructions[2].operands[1].virtual_register = LEFT;
        });
        let result = fold(&source, &environment).unwrap();
        let rewritten = &result.transformed().functions[0].blocks[0].instructions[2];
        assert_eq!(
            rewritten.kind,
            SelectedInstructionKind::ExactAddI64Immediate {
                immediate: IntegerValue::Unsigned(0),
                obligation: obligation(),
                accepted_fact: accepted_fact(),
            }
        );
        // The former right operand survives as the register input.
        assert_eq!(rewritten.operands[0].virtual_register, LEFT);
        assert_eq!(rewritten.operands[1].virtual_register, RESULT);
    }
}

/// A literal in the left operand of a subtract cannot fold: no
/// `literal - register` immediate form exists to select.
#[test]
fn left_literal_subtract_cannot_fold() {
    let target = NativeTarget::linux_arm64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, false, |function, _| {
        function.blocks[0].instructions[2].operands[0].virtual_register = LITERAL;
        function.blocks[0].instructions[2].operands[1].virtual_register = LEFT;
    });
    assert_eq!(
        fold(&source, &environment).unwrap_err(),
        LiteralArithmeticError::UnsupportedProducer
    );
}

/// The subtract fold fires where the target's two rows publish the same
/// implicit surface: aarch64's `sub` realizations write no flags on either
/// form. x86-64's register-register `sub` variants clobber `rflags` while the
/// `lea`-encoded immediate preserves them, so the surfaces differ and the
/// fold refuses there.
#[test]
fn subtract_fold_respects_each_target_implicit_surface() {
    for target in [NativeTarget::linux_arm64(), NativeTarget::macos_arm64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = materialize_fixture(target, false, IntegerValue::Unsigned(4));
        let result = fold(&source, &environment).unwrap();
        let rewritten = &result.transformed().functions[0].blocks[0].instructions[2];
        assert_eq!(
            rewritten.kind,
            SelectedInstructionKind::ExactSubtractI64Immediate {
                immediate: IntegerValue::Unsigned(4),
                obligation: obligation(),
                accepted_fact: accepted_fact(),
            }
        );
        assert_eq!(
            rewritten.constraint,
            keys(&environment).subtract_i64_immediate
        );
        assert_eq!(rewritten.operands[0].virtual_register, LEFT);
        assert_eq!(rewritten.operands[1].virtual_register, RESULT);
    }
    for target in [NativeTarget::linux_x64(), NativeTarget::windows_x64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = materialize_fixture(target, false, IntegerValue::Unsigned(4));
        assert_eq!(
            fold(&source, &environment).unwrap_err(),
            LiteralArithmeticError::ConstraintMismatch
        );
    }
}

/// A literal on both sides folds the right operand: `register - register`
/// and `register + register` read the literal register as the surviving
/// input while the immediate carries the same literal.
#[test]
fn same_register_on_both_sides_folds() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, true, |function, _| {
        function.blocks[0].instructions[2].operands[0].virtual_register = LITERAL;
    });
    let result = fold(&source, &environment).unwrap();
    let rewritten = &result.transformed().functions[0].blocks[0].instructions[2];
    assert_eq!(
        rewritten.kind,
        SelectedInstructionKind::ExactAddI64Immediate {
            immediate: IntegerValue::Unsigned(0),
            obligation: obligation(),
            accepted_fact: accepted_fact(),
        }
    );
    assert_eq!(rewritten.operands[0].virtual_register, LITERAL);
}

/// An out-of-bound right literal does not block an in-bound left literal on
/// an add: the commute retries and the wide literal stays a register read.
#[test]
fn wide_right_literal_falls_back_to_the_left_operand() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, true, |function, environment| {
        // Rebind the materialization to a wide literal and put an in-bound
        // materialization on the left operand through a second producer.
        function.blocks[0].instructions[1].kind = SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(5000),
        };
        let materialize = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap()
            .clone();
        function.virtual_registers.push(register(
            SPARE,
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
            function.virtual_registers[0].class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(8),
                source_value: ValueId::new(6).unwrap(),
            },
        ));
        function.blocks[0].instructions.insert(
            2,
            instruction(
                SelectedInstructionId(8),
                SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(5),
                },
                &materialize,
                &[SPARE],
            ),
        );
        function.blocks[0].instructions[3].operands[0].virtual_register = SPARE;
    });
    let result = fold(&source, &environment).unwrap();
    let rewritten = &result.transformed().functions[0].blocks[0].instructions[3];
    assert_eq!(
        rewritten.kind,
        SelectedInstructionKind::ExactAddI64Immediate {
            immediate: IntegerValue::Unsigned(5),
            obligation: obligation(),
            accepted_fact: accepted_fact(),
        }
    );
    // The wide literal's register survives as the register input.
    assert_eq!(rewritten.operands[0].virtual_register, LITERAL);
    assert_eq!(rewritten.operands[1].virtual_register, RESULT);
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
    let source = mutated(target, true, |function, _| {
        function.virtual_registers.push(register(
            SPARE,
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
            function.virtual_registers[0].class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(7),
                source_value: ValueId::new(6).unwrap(),
            },
        ));
        let mut consumer = instruction(
            SelectedInstructionId(7),
            SelectedInstructionKind::CopyI64,
            &copy,
            &[LITERAL, SPARE],
        );
        consumer.provenance.values = vec![ValueId::new(7).unwrap()];
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
        SelectedInstructionKind::ExactAddI64Immediate {
            immediate: IntegerValue::Unsigned(0),
            obligation: obligation(),
            accepted_fact: accepted_fact(),
        }
    );
    assert_eq!(
        transformed.functions[0].blocks[0].instructions[3].kind,
        SelectedInstructionKind::CopyI64
    );
}

/// A validated fold is itself a sealed analysis source: a second literal
/// arithmetic fold runs on the transformed program without re-entering the
/// stage.
#[test]
fn validated_results_compose() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let add = environment
        .constraint(keys(&environment).add_i64)
        .unwrap()
        .clone();
    let source = mutated(target, true, |function, _| {
        function.virtual_registers.push(register(
            SPARE,
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
            function.virtual_registers[0].class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(8),
                source_value: ValueId::new(6).unwrap(),
            },
        ));
        let mut second = instruction(
            SelectedInstructionId(8),
            SelectedInstructionKind::ExactAddI64 {
                obligation: obligation(),
                accepted_fact: accepted_fact(),
            },
            &add,
            &[OUTPUT, LITERAL, SPARE],
        );
        second.provenance.operations = vec![OperationId::new(8).unwrap()];
        function.blocks[0].instructions.insert(3, second);
    });
    let first = fold(&source, &environment).unwrap();
    let second = fold_selected_literal_arithmetic(
        &first,
        0,
        SelectedInstructionId(8),
        &environment,
        budget(),
    )
    .unwrap();
    let instructions = &second.transformed().functions[0].blocks[0].instructions;
    assert_eq!(
        instructions[2].kind,
        SelectedInstructionKind::ExactAddI64Immediate {
            immediate: IntegerValue::Unsigned(0),
            obligation: obligation(),
            accepted_fact: accepted_fact(),
        }
    );
    assert_eq!(
        instructions[3].kind,
        SelectedInstructionKind::ExactAddI64Immediate {
            immediate: IntegerValue::Unsigned(0),
            obligation: obligation(),
            accepted_fact: accepted_fact(),
        }
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
    add: bool,
    edit: impl FnOnce(&mut SelectedFunction, &register_environment::ValidatedTargetRegisterEnvironment),
) -> ValidatedLiteralArithmetic {
    let environment = baseline_target_register_environment(target).unwrap();
    let mut source = materialize_fixture(target, add, IntegerValue::Unsigned(0));
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
    source: &ValidatedLiteralArithmetic,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> Result<ValidatedLiteralArithmetic, LiteralArithmeticError> {
    fold_selected_literal_arithmetic(source, 0, OPERATION, environment, budget())
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
        let source = fixture(
            target,
            true,
            *producer_kind,
            *producer_key,
            producer_registers,
        );
        assert_eq!(
            fold(&source, &environment).unwrap_err(),
            LiteralArithmeticError::UnsupportedProducer,
            "{producer_kind:?}"
        );
    }
    // A second definition of the literal register — even through a UseDef
    // rewrite — breaks the wherever-read guarantee.
    let second_definition = mutated(target, true, |function, environment| {
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
        LiteralArithmeticError::UnsupportedProducer
    );
    // A definition carried by a terminator's instruction still counts.
    let terminator_definition = mutated(target, true, |function, environment| {
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
        LiteralArithmeticError::UnsupportedProducer
    );
    // A block-parameter or entry register has no producing instruction.
    let no_producer = mutated(target, true, |function, _| {
        function.blocks[0].instructions.remove(1);
        function.virtual_registers[2].origin = VirtualRegisterOrigin::BlockParameter {
            source_value: ValueId::new(3).unwrap(),
            block: SelectedBlockId(0),
            parameter_index: 0,
        };
    });
    assert_eq!(
        fold(&no_producer, &environment).unwrap_err(),
        LiteralArithmeticError::UnsupportedProducer
    );
    // A literal register absent from the roster is malformed.
    let missing_register = mutated(target, true, |function, _| {
        function
            .virtual_registers
            .retain(|entry| entry.id != LITERAL);
    });
    assert_eq!(
        fold(&missing_register, &environment).unwrap_err(),
        LiteralArithmeticError::UnsupportedProducer
    );
}

#[test]
fn malformed_operation_shapes_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A pinned use cannot ride into the rebuilt operand row.
    let pinned_use = mutated(target, true, |function, _| {
        function.blocks[0].instructions[2].operands[0].fixed_view =
            Some(register_model::RegisterViewId(0));
    });
    assert_eq!(
        fold(&pinned_use, &environment).unwrap_err(),
        LiteralArithmeticError::UnsupportedInstruction
    );
    // A tied operand stays an allocation-shaped instruction.
    let tied_use = mutated(target, true, |function, _| {
        function.blocks[0].instructions[2].operands[1].tied_to = Some(0);
    });
    assert_eq!(
        fold(&tied_use, &environment).unwrap_err(),
        LiteralArithmeticError::UnsupportedInstruction
    );
    // An implicit unit use would be silently dropped by the immediate form.
    let implicit = mutated(target, true, |function, environment| {
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
        LiteralArithmeticError::UnsupportedInstruction
    );
    // The kind check rejects non-arithmetic instructions.
    let not_arithmetic = mutated(target, true, |function, _| {
        function.blocks[0].instructions[2].kind = SelectedInstructionKind::CopyI64;
    });
    assert_eq!(
        fold(&not_arithmetic, &environment).unwrap_err(),
        LiteralArithmeticError::UnsupportedInstruction
    );
    // A fourth operand is not the emitted `[use, use, def]` shape.
    let extra_operand = mutated(target, true, |function, _| {
        let operand = function.blocks[0].instructions[2].operands[1];
        function.blocks[0].instructions[2].operands.push(operand);
    });
    assert_eq!(
        fold(&extra_operand, &environment).unwrap_err(),
        LiteralArithmeticError::UnsupportedInstruction
    );
    // A write operand is not an arithmetic use.
    let writing = mutated(target, true, |function, _| {
        function.blocks[0].instructions[2].operands[1].access = RegisterOperandAccess::Def;
    });
    assert_eq!(
        fold(&writing, &environment).unwrap_err(),
        LiteralArithmeticError::UnsupportedInstruction
    );
    // A result operand that only reads is not the emitted def.
    let reading_result = mutated(target, true, |function, _| {
        function.blocks[0].instructions[2].operands[2].access = RegisterOperandAccess::Use;
    });
    assert_eq!(
        fold(&reading_result, &environment).unwrap_err(),
        LiteralArithmeticError::UnsupportedInstruction
    );
    // The surviving input must exist in the register roster.
    let missing_input = mutated(target, true, |function, _| {
        function.virtual_registers.retain(|entry| entry.id != LEFT);
    });
    assert_eq!(
        fold(&missing_input, &environment).unwrap_err(),
        LiteralArithmeticError::UnsupportedUse
    );
    // The result register must exist in the register roster.
    let missing_result = mutated(target, true, |function, _| {
        function
            .virtual_registers
            .retain(|entry| entry.id != RESULT);
    });
    assert_eq!(
        fold(&missing_result, &environment).unwrap_err(),
        LiteralArithmeticError::ConstraintMismatch
    );
}

/// The selected row must publish exactly the implicit surface the operation
/// carried, and the operation's declared row must match its operand shape and
/// carried surface.
#[test]
fn constraint_and_effect_surfaces_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // An extra implicit definition the declared row does not publish cannot
    // be preserved.
    let extra_def = mutated(target, true, |function, _| {
        function.blocks[0].instructions[2]
            .implicit_defs
            .push(register_model::RegisterUnitId(u16::MAX));
    });
    assert_eq!(
        fold(&extra_def, &environment).unwrap_err(),
        LiteralArithmeticError::ConstraintMismatch
    );
    // An extra clobber the declared row does not carry cannot be preserved.
    let extra_clobber = mutated(target, true, |function, _| {
        function.blocks[0].instructions[2]
            .clobbers
            .push(register_model::RegisterUnitId(u16::MAX));
    });
    assert_eq!(
        fold(&extra_clobber, &environment).unwrap_err(),
        LiteralArithmeticError::ConstraintMismatch
    );
    // A declared row that does not declare the `[use, use, def]` shape the
    // operands carry is malformed.
    let wrong_row = mutated(target, true, |function, environment| {
        function.blocks[0].instructions[2].constraint = environment.selected_keys().copy_i64;
    });
    assert_eq!(
        fold(&wrong_row, &environment).unwrap_err(),
        LiteralArithmeticError::ConstraintMismatch
    );
    // A roster class the row does not declare for operand zero rejects.
    let wrong_class = mutated(target, true, |function, _| {
        function.virtual_registers[1].class = register_model::RegisterClassId(u16::MAX);
    });
    assert_eq!(
        fold(&wrong_class, &environment).unwrap_err(),
        LiteralArithmeticError::ConstraintMismatch
    );
}

#[test]
fn source_identity_mismatches_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = materialize_fixture(target, true, IntegerValue::Unsigned(0));
    // A function index outside the plan.
    assert_eq!(
        fold_selected_literal_arithmetic(&source, 1, OPERATION, &environment, budget())
            .unwrap_err(),
        LiteralArithmeticError::SourceMismatch
    );
    // An instruction id the function does not contain.
    assert_eq!(
        fold_selected_literal_arithmetic(
            &source,
            0,
            SelectedInstructionId(99),
            &environment,
            budget()
        )
        .unwrap_err(),
        LiteralArithmeticError::SourceMismatch
    );
    // An environment for a different target than the plan's.
    let wrong_environment =
        baseline_target_register_environment(NativeTarget::macos_arm64()).unwrap();
    assert_eq!(
        fold_selected_literal_arithmetic(&source, 0, OPERATION, &wrong_environment, budget())
            .unwrap_err(),
        LiteralArithmeticError::SourceMismatch
    );
}

#[test]
fn validation_budget_covers_the_producer_scan() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = materialize_fixture(target, true, IntegerValue::Unsigned(0));
    let tiny = OptimizationWorkBudget::new(100, 100, 4, 100, 100).unwrap();
    assert_eq!(
        fold_selected_literal_arithmetic(&source, 0, OPERATION, &environment, tiny).unwrap_err(),
        LiteralArithmeticError::WorkBudgetExceeded
    );
}

/// The measured validation-step boundary: admission charges one step per
/// block plus one per instruction across the plan, then repeats the same
/// scan over the admitted function to locate the literal's producers —
/// ten steps for the four-instruction fixture, twelve once a fifth
/// instruction joins the block — so the exact count admits the fold on
/// both the proposal and the independent replay path while one step below
/// rejects both.
#[test]
fn measured_validation_step_boundary_admits_and_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A fifth body instruction extends both scans: (1 block) + (5
    // instructions) charged twice measures twelve steps.
    let wider = mutated(target, true, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.virtual_registers.push(register(
            SPARE,
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
            function.virtual_registers[0].class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(7),
                source_value: ValueId::new(6).unwrap(),
            },
        ));
        function.blocks[0].instructions.push(instruction(
            SelectedInstructionId(7),
            SelectedInstructionKind::CopyI64,
            &copy,
            &[POINTER, SPARE],
        ));
    });
    for (source, exact_steps) in [
        (
            materialize_fixture(target, true, IntegerValue::Unsigned(9)),
            10u64,
        ),
        (wider, 12u64),
    ] {
        let exact = OptimizationWorkBudget::new(1, 1, exact_steps, 1, 1).unwrap();
        let result =
            fold_selected_literal_arithmetic(&source, 0, OPERATION, &environment, exact).unwrap();
        validate_literal_arithmetic_fold(
            &source,
            0,
            OPERATION,
            &environment,
            exact,
            result.transformed().clone(),
        )
        .unwrap();
        let starved = OptimizationWorkBudget::new(1, 1, exact_steps - 1, 1, 1).unwrap();
        assert_eq!(
            fold_selected_literal_arithmetic(&source, 0, OPERATION, &environment, starved)
                .unwrap_err(),
            LiteralArithmeticError::WorkBudgetExceeded
        );
        assert_eq!(
            validate_literal_arithmetic_fold(
                &source,
                0,
                OPERATION,
                &environment,
                starved,
                result.transformed().clone(),
            )
            .unwrap_err(),
            LiteralArithmeticError::WorkBudgetExceeded
        );
    }
}

#[test]
fn replay_rejects_anything_but_the_exact_form() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = materialize_fixture(target, true, IntegerValue::Unsigned(9));
    let result = fold(&source, &environment).unwrap();
    for mutation in 0..11 {
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
                    SelectedInstructionKind::ExactAddI64Immediate {
                        immediate: IntegerValue::Unsigned(8),
                        obligation: obligation(),
                        accepted_fact: accepted_fact(),
                    };
            }
            // A substituted obligation is not the operation's own.
            2 => {
                function.blocks[0].instructions[2].kind =
                    SelectedInstructionKind::ExactAddI64Immediate {
                        immediate: IntegerValue::Unsigned(9),
                        obligation: ObligationId::new(12).unwrap(),
                        accepted_fact: accepted_fact(),
                    };
            }
            // The register-register form must not survive in the slot.
            3 => {
                function.blocks[0].instructions[2].kind = SelectedInstructionKind::ExactAddI64 {
                    obligation: obligation(),
                    accepted_fact: accepted_fact(),
                };
            }
            // A different instruction id on the selected form.
            4 => function.blocks[0].instructions[2].id = LOAD,
            // The operation's provenance must survive intact.
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
            // The result register must stay the defined operand.
            8 => {
                function.blocks[0].instructions[2].operands[1].virtual_register = OUTPUT;
            }
            // An unrelated register must stay identical.
            9 => function.virtual_registers[3].scalar_type = ScalarType::Boolean,
            // A fabricated roster row has no source counterpart.
            10 => {
                function
                    .memory_accesses
                    .push(selected_instructions::SelectedMemoryAccess {
                        instruction: OPERATION,
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
            validate_literal_arithmetic_fold(
                &source,
                0,
                OPERATION,
                &environment,
                budget(),
                proposed
            )
            .is_err(),
            "mutation {mutation}"
        );
    }
}

/// Two runs over the identical source produce the identical validated result,
/// and the published plan is a legal second input: the sealed transformed
/// program already sits at the rule's fixed point — the operation keeps its
/// instruction identity as the selected immediate form, so a second fold at
/// the same site finds no register-register arithmetic to admit.
#[test]
fn fold_is_deterministic_and_terminal() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let first = fold(
        &materialize_fixture(target, true, IntegerValue::Unsigned(9)),
        &environment,
    )
    .unwrap();
    let second = fold(
        &materialize_fixture(target, true, IntegerValue::Unsigned(9)),
        &environment,
    )
    .unwrap();
    assert_eq!(first, second);
    // The validated output carries the sealed analysis boundary, so it is a
    // legal second input — not merely a reconstruction of one. Re-running on
    // it is terminal: instruction OPERATION is the emitted
    // ExactAddI64Immediate now, not a two-register arithmetic operation.
    assert_eq!(
        fold_selected_literal_arithmetic(&first, 0, OPERATION, &environment, budget()).unwrap_err(),
        LiteralArithmeticError::UnsupportedInstruction
    );
}

/// Replay corruption in a block the fold never touched still rejects: the
/// restore-by-content check compares the complete plan, not just the block
/// carrying the rewritten operation.
#[test]
fn replay_rejects_drift_outside_the_rewritten_block() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Stretch the fixture across one edge: the operation's block folds and
    // jumps to a second block that returns.
    let source = mutated(target, true, |function, environment| {
        let jump = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        let tail = std::mem::replace(
            &mut function.blocks[0].terminator,
            SelectedTerminator::Jump {
                instruction: instruction(
                    SelectedInstructionId(10),
                    SelectedInstructionKind::Jump,
                    jump,
                    &[],
                ),
                successor: selected_instructions::SelectedSuccessor {
                    role: selected_instructions::SelectedSuccessorRole::Semantic,
                    structural_case: None,
                    structural_bindings: Vec::new(),
                    psi_edge: EdgeId::new(2).unwrap(),
                    block: SelectedBlockId(1),
                    source_target: BlockId::new(2).unwrap(),
                    bindings: Vec::new(),
                    fuel: Vec::new(),
                },
            },
        );
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(1),
            origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
            instructions: Vec::new(),
            terminator: tail,
        });
    });
    let result = fold(&source, &environment).unwrap();
    assert_eq!(result.transformed().functions[0].blocks.len(), 2);
    // An extra instruction in the untouched landing block rejects.
    let mut proposed = result.transformed().clone();
    let copy = environment
        .constraint(keys(&environment).copy_i64)
        .unwrap()
        .clone();
    proposed.functions[0].blocks[1]
        .instructions
        .push(instruction(
            SelectedInstructionId(11),
            SelectedInstructionKind::CopyI64,
            &copy,
            &[POINTER, OUTPUT],
        ));
    assert_eq!(
        validate_literal_arithmetic_fold(&source, 0, OPERATION, &environment, budget(), proposed)
            .unwrap_err(),
        LiteralArithmeticError::ReplayMismatch
    );
    // Drift in the untouched block's terminator rejects.
    let mut proposed = result.transformed().clone();
    let SelectedTerminator::Return {
        instruction: return_instruction,
        ..
    } = &mut proposed.functions[0].blocks[1].terminator
    else {
        unreachable!()
    };
    return_instruction.id = SelectedInstructionId(12);
    assert_eq!(
        validate_literal_arithmetic_fold(&source, 0, OPERATION, &environment, budget(), proposed)
            .unwrap_err(),
        LiteralArithmeticError::ReplayMismatch
    );
    // A phantom trailing block rejects.
    let mut proposed = result.transformed().clone();
    let return_row = environment
        .constraint(keys(&environment).return_unit)
        .unwrap()
        .clone();
    proposed.functions[0].blocks.push(SelectedBlock {
        id: SelectedBlockId(2),
        origin: SelectedBlockOrigin::Source(BlockId::new(3).unwrap()),
        instructions: Vec::new(),
        terminator: SelectedTerminator::Return {
            instruction: instruction(
                SelectedInstructionId(13),
                SelectedInstructionKind::ReturnUnit,
                &return_row,
                &[],
            ),
            psi_return_edge: EdgeId::new(3).unwrap(),
        },
    });
    assert_eq!(
        validate_literal_arithmetic_fold(&source, 0, OPERATION, &environment, budget(), proposed)
            .unwrap_err(),
        LiteralArithmeticError::ReplayMismatch
    );
}
