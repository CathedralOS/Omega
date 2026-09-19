use crate::LiteralMinuendError;
use crate::LiteralMinuendReceipt;
use crate::ValidatedLiteralMinuend;
use crate::ValidatedSelectedAnalysis;
use crate::fold_selected_literal_minuend;
use crate::validate_literal_minuend_fold;
use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget};
use optimization_unit::ValueDefinitionSite;
use register_environment::baseline_target_register_environment;
use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan, SelectedOperand,
    SelectedSuccessor, SelectedSuccessorRole, SelectedTerminator, VirtualRegister,
    VirtualRegisterId, VirtualRegisterOrigin,
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

const LOAD: SelectedInstructionId = SelectedInstructionId(2);
const MATERIALIZE: SelectedInstructionId = SelectedInstructionId(3);
const COMPARE: SelectedInstructionId = SelectedInstructionId(4);
const BOOLEAN: SelectedInstructionId = SelectedInstructionId(5);
const TERMINAL: SelectedInstructionId = SelectedInstructionId(6);
const POINTER: VirtualRegisterId = VirtualRegisterId(0);
const INPUT: VirtualRegisterId = VirtualRegisterId(1);
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

fn successor(block: SelectedBlockId, source_target: BlockId, edge: u64) -> SelectedSuccessor {
    SelectedSuccessor {
        role: SelectedSuccessorRole::Semantic,
        structural_case: None,
        structural_bindings: Vec::new(),
        psi_edge: EdgeId::new(edge).unwrap(),
        block,
        source_target,
        bindings: Vec::new(),
        fuel: Vec::new(),
    }
}

/// A raw selected-stage unit fixture, not a source/Terminal admission claim:
/// `r1 = load8 r0; r2 = <producer>; compare r2, r1; r3 = boolean; return`.
/// The literal sits in the minuend operand; the boolean materialization is
/// the equality-sensing consumer the fold's flag audit must reach.
fn fixture(
    target: NativeTarget,
    producer_kind: SelectedInstructionKind,
    producer_key: register_model::RegisterConstraintKey,
    producer_registers: &[VirtualRegisterId],
    consumer_kind: SelectedInstructionKind,
) -> ValidatedLiteralMinuend {
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
            INPUT,
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
        &[LITERAL, INPUT],
    );
    compare_instruction.provenance.operations = vec![OperationId::new(7).unwrap()];
    compare_instruction.provenance.values = vec![ValueId::new(3).unwrap()];
    let instructions = vec![
        instruction(
            LOAD,
            SelectedInstructionKind::Load8 { byte_offset: 0 },
            load,
            &[POINTER, INPUT],
        ),
        instruction(MATERIALIZE, producer_kind, producer_row, producer_registers),
        compare_instruction,
        instruction(BOOLEAN, consumer_kind, boolean, &[OUTPUT]),
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
    ValidatedLiteralMinuend {
        receipt: LiteralMinuendReceipt {
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

fn materialize_fixture(target: NativeTarget, value: IntegerValue) -> ValidatedLiteralMinuend {
    let environment = baseline_target_register_environment(target).unwrap();
    fixture(
        target,
        SelectedInstructionKind::MaterializeI64 { value },
        keys(&environment).materialize_i64,
        &[LITERAL],
        SelectedInstructionKind::MaterializeBooleanEqual,
    )
}

/// A literal of zero in the minuend operand selects the dedicated test form
/// on every target: `0 - r` and `r - 0` share the zero condition.
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
            fold_selected_literal_minuend(&source, 0, COMPARE, &environment, budget()).unwrap();
        let rewritten = &result.transformed().functions[0].blocks[0].instructions[2];
        assert_eq!(rewritten.id, COMPARE);
        assert_eq!(rewritten.kind, SelectedInstructionKind::CompareI64Zero);
        assert_eq!(rewritten.constraint, keys(&environment).compare_i64_zero);
        assert_eq!(rewritten.operands.len(), 1);
        assert_eq!(rewritten.operands[0].operand, 0);
        // The surviving input is the source's subtrahend register.
        assert_eq!(rewritten.operands[0].virtual_register, INPUT);
        assert_eq!(rewritten.operands[0].access, RegisterOperandAccess::Use);
        // The condition-state surface is preserved bit-identically: the zero
        // form publishes exactly the flag definitions the compare carried.
        assert_eq!(
            rewritten.implicit_defs,
            source.transformed().functions[0].blocks[0].instructions[2].implicit_defs
        );
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
        validate_literal_minuend_fold(
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
        validate_literal_minuend_fold(&source, 0, COMPARE, &environment, budget(), detached)
            .unwrap();
    }
}

/// A nonzero literal inside the shared U12 bound selects the immediate form
/// reading the subtrahend register.
#[test]
fn positive_literal_selects_the_immediate_form() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for value in [1u64, 5, 2048, 4095] {
        let source = materialize_fixture(target, IntegerValue::Unsigned(u128::from(value)));
        let result =
            fold_selected_literal_minuend(&source, 0, COMPARE, &environment, budget()).unwrap();
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
        assert_eq!(rewritten.operands[0].virtual_register, INPUT);
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
        Result<SelectedInstructionKind, LiteralMinuendError>,
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
            Err(LiteralMinuendError::UnsupportedLiteral),
        ),
        (
            IntegerValue::Unsigned(u128::from(u64::MAX)),
            Err(LiteralMinuendError::UnsupportedLiteral),
        ),
        (
            IntegerValue::Unsigned(u128::from(u64::MAX) + 1),
            Err(LiteralMinuendError::UnsupportedLiteral),
        ),
        (
            IntegerValue::Unsigned(u128::MAX),
            Err(LiteralMinuendError::UnsupportedLiteral),
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
            Err(LiteralMinuendError::UnsupportedLiteral),
        ),
        (
            IntegerValue::Signed(-1),
            Err(LiteralMinuendError::UnsupportedLiteral),
        ),
        (
            IntegerValue::Signed(-4096),
            Err(LiteralMinuendError::UnsupportedLiteral),
        ),
    ];
    for (value, expected) in cases {
        let source = materialize_fixture(target, *value);
        let result = fold_selected_literal_minuend(&source, 0, COMPARE, &environment, budget());
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

/// A literal in the subtrahend operand belongs to the sibling family: this
/// rule's producer scan runs on the minuend register, whose producer here is
/// the load, not a materialization.
#[test]
fn right_operand_literal_belongs_to_the_sibling_rule() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        function.blocks[0].instructions[2].operands[0].virtual_register = INPUT;
        function.blocks[0].instructions[2].operands[1].virtual_register = LITERAL;
    });
    assert_eq!(
        fold(&source, &environment).unwrap_err(),
        LiteralMinuendError::UnsupportedProducer
    );
}

/// The same register on both sides still folds — `v - v` and the surviving
/// `v - v` are the identical subtraction, so even an ordering consumer
/// observes unchanged flags and the consumer audit does not apply.
#[test]
fn same_register_on_both_sides_folds_without_consumer_audit() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        function.blocks[0].instructions[2].operands[1].virtual_register = LITERAL;
        function.blocks[0].instructions[3].kind =
            SelectedInstructionKind::MaterializeBooleanU64LessThan;
    });
    let result = fold(&source, &environment).unwrap();
    let rewritten = &result.transformed().functions[0].blocks[0].instructions[2];
    assert_eq!(rewritten.kind, SelectedInstructionKind::CompareI64Zero);
    assert_eq!(rewritten.operands[0].virtual_register, LITERAL);
}

/// Every ordering predicate reads flag bits the swapped subtraction inverts:
/// the boolean materializations in the body and the predicate-aware
/// terminators all refuse the fold.
#[test]
fn ordering_consumers_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    use SelectedInstructionKind::*;
    for kind in [
        MaterializeBooleanU64LessThan,
        MaterializeBooleanI64LessThan,
        MaterializeBooleanU64LessOrEqual,
        MaterializeBooleanI64LessOrEqual,
    ] {
        let source = fixture(
            target,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(3),
            },
            keys(&environment).materialize_i64,
            &[LITERAL],
            kind,
        );
        assert_eq!(
            fold(&source, &environment).unwrap_err(),
            LiteralMinuendError::UnsupportedUse,
            "{kind:?}"
        );
    }
    // A predicate-aware terminator reads the same inverted ordering.
    for branch in [ConditionalBranchU64LessThan, ConditionalBranchI64LessThan] {
        let source = mutated(target, |function, environment| {
            let row = environment
                .constraint(environment.selected_keys().conditional_branch)
                .unwrap()
                .clone();
            let branch_instruction = instruction(TERMINAL, branch, &row, &[]);
            function.blocks[0].terminator = match branch {
                ConditionalBranchU64LessThan => SelectedTerminator::ConditionalBranchU64LessThan {
                    instruction: branch_instruction,
                    when_less: successor(SelectedBlockId(0), BlockId::new(1).unwrap(), 2),
                    when_not_less: successor(SelectedBlockId(0), BlockId::new(1).unwrap(), 3),
                },
                _ => SelectedTerminator::ConditionalBranchI64LessThan {
                    instruction: branch_instruction,
                    when_less: successor(SelectedBlockId(0), BlockId::new(1).unwrap(), 2),
                    when_not_less: successor(SelectedBlockId(0), BlockId::new(1).unwrap(), 3),
                },
            };
        });
        assert_eq!(
            fold(&source, &environment).unwrap_err(),
            LiteralMinuendError::UnsupportedUse,
            "{branch:?}"
        );
    }
}

/// The generic conditional branch reads the zero condition alone, so it is
/// an equality-sensing consumer: the compare folds and the flag unit keeps
/// flowing through its edges without finding an ordering reader.
#[test]
fn generic_conditional_branch_is_equality_sensing() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let branch_row = environment
            .constraint(environment.selected_keys().conditional_branch)
            .unwrap()
            .clone();
        let return_row = environment
            .constraint(environment.selected_keys().return_unit)
            .unwrap()
            .clone();
        function.blocks[0].terminator = SelectedTerminator::ConditionalBranch {
            instruction: instruction(
                TERMINAL,
                SelectedInstructionKind::ConditionalBranchNonZero,
                &branch_row,
                &[],
            ),
            when_nonzero: successor(SelectedBlockId(1), BlockId::new(2).unwrap(), 2),
            when_zero: successor(SelectedBlockId(1), BlockId::new(2).unwrap(), 3),
        };
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(1),
            origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
            instructions: Vec::new(),
            terminator: SelectedTerminator::Return {
                instruction: instruction(
                    SelectedInstructionId(10),
                    SelectedInstructionKind::ReturnUnit,
                    &return_row,
                    &[],
                ),
                psi_return_edge: EdgeId::new(4).unwrap(),
            },
        });
    });
    let result = fold(&source, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[0].instructions[2].kind,
        SelectedInstructionKind::CompareI64Zero
    );
}

/// A flag reader reached through a successor edge is audited like a body
/// reader: an ordering materialization at the head of the jump target
/// refuses, while an equality one admits.
#[test]
fn flag_consumers_reached_through_edges() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let jump_target = |kind: SelectedInstructionKind| {
        mutated(target, move |function, environment| {
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
            // The boolean consumer moves out of the compare's block into the
            // jump target; the unit stays live across the edge.
            function.blocks[0].instructions.remove(3);
            function.blocks[0].terminator = SelectedTerminator::Jump {
                instruction: instruction(TERMINAL, SelectedInstructionKind::Jump, &jump_row, &[]),
                successor: successor(SelectedBlockId(1), BlockId::new(2).unwrap(), 2),
            };
            function.blocks.push(SelectedBlock {
                id: SelectedBlockId(1),
                origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
                instructions: vec![instruction(BOOLEAN, kind, &boolean_row, &[OUTPUT])],
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
        })
    };
    let ordering = jump_target(SelectedInstructionKind::MaterializeBooleanU64LessThan);
    assert_eq!(
        fold(&ordering, &environment).unwrap_err(),
        LiteralMinuendError::UnsupportedUse
    );
    let equality = jump_target(SelectedInstructionKind::MaterializeBooleanEqual);
    fold(&equality, &environment).unwrap();
}

/// A second compare redefines the flag units before the ordering reader, so
/// the reader consumes the later comparison, not this one's — the fold is
/// free to proceed.
#[test]
fn flag_redefinition_ends_the_audit() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let compare_row = environment
            .constraint(environment.selected_keys().compare_i64)
            .unwrap()
            .clone();
        let mut second = instruction(
            SelectedInstructionId(8),
            SelectedInstructionKind::CompareI64,
            &compare_row,
            &[INPUT, LITERAL],
        );
        second.provenance.operations = vec![OperationId::new(9).unwrap()];
        function.blocks[0].instructions.insert(3, second);
        // Now ordering after the second compare reads its flags, not ours.
        function.blocks[0].instructions[4].kind =
            SelectedInstructionKind::MaterializeBooleanU64LessThan;
    });
    let result = fold(&source, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[0].instructions[2].kind,
        SelectedInstructionKind::CompareI64Zero
    );
}

/// A flag unit carried around a back edge re-enters the compare's block at
/// its head: readers between the head and the compare still observe this
/// compare's state, so an ordering reader there refuses and an equality one
/// — ended by the compare's own redefinition — admits.
#[test]
fn loop_carried_flag_consumers() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let cyclic = |kind: SelectedInstructionKind| {
        mutated(target, move |function, environment| {
            let boolean_row = environment
                .constraint(environment.selected_keys().materialize_boolean)
                .unwrap()
                .clone();
            let branch_row = environment
                .constraint(environment.selected_keys().conditional_branch)
                .unwrap()
                .clone();
            let return_row = environment
                .constraint(environment.selected_keys().return_unit)
                .unwrap()
                .clone();
            // The in-block consumer moves to the block head, where a
            // back-edge can still deliver this compare's flag state.
            function.blocks[0].instructions.remove(3);
            let mut head = instruction(BOOLEAN, kind, &boolean_row, &[OUTPUT]);
            head.provenance.values = vec![ValueId::new(4).unwrap()];
            function.blocks[0].instructions.insert(0, head);
            function.blocks[0].terminator = SelectedTerminator::ConditionalBranch {
                instruction: instruction(
                    TERMINAL,
                    SelectedInstructionKind::ConditionalBranchNonZero,
                    &branch_row,
                    &[],
                ),
                when_nonzero: successor(SelectedBlockId(0), BlockId::new(1).unwrap(), 2),
                when_zero: successor(SelectedBlockId(1), BlockId::new(2).unwrap(), 3),
            };
            function.blocks.push(SelectedBlock {
                id: SelectedBlockId(1),
                origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
                instructions: Vec::new(),
                terminator: SelectedTerminator::Return {
                    instruction: instruction(
                        SelectedInstructionId(10),
                        SelectedInstructionKind::ReturnUnit,
                        &return_row,
                        &[],
                    ),
                    psi_return_edge: EdgeId::new(4).unwrap(),
                },
            });
        })
    };
    let ordering = cyclic(SelectedInstructionKind::MaterializeBooleanI64LessOrEqual);
    assert_eq!(
        fold(&ordering, &environment).unwrap_err(),
        LiteralMinuendError::UnsupportedUse
    );
    let equality = cyclic(SelectedInstructionKind::MaterializeBooleanEqual);
    fold(&equality, &environment).unwrap();
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
/// minuend folds on the transformed program without re-entering the stage.
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
            &[LITERAL, INPUT],
        );
        second.provenance.operations = vec![OperationId::new(8).unwrap()];
        function.blocks[0].instructions.insert(3, second);
    });
    let first = fold(&source, &environment).unwrap();
    let second =
        fold_selected_literal_minuend(&first, 0, SelectedInstructionId(7), &environment, budget())
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
) -> ValidatedLiteralMinuend {
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
            *producer_kind,
            *producer_key,
            producer_registers,
            MaterializeBooleanEqual,
        );
        assert_eq!(
            fold(&source, &environment).unwrap_err(),
            LiteralMinuendError::UnsupportedProducer,
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
        LiteralMinuendError::UnsupportedProducer
    );
    // A definition carried by a terminator's instruction still counts.
    let terminator_definition = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        let carried = instruction(
            TERMINAL,
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
        LiteralMinuendError::UnsupportedProducer
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
        LiteralMinuendError::UnsupportedProducer
    );
    // A literal register absent from the roster is malformed.
    let missing_register = mutated(target, |function, _| {
        function
            .virtual_registers
            .retain(|entry| entry.id != LITERAL);
    });
    assert_eq!(
        fold(&missing_register, &environment).unwrap_err(),
        LiteralMinuendError::UnsupportedProducer
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
        LiteralMinuendError::UnsupportedInstruction
    );
    // A tied operand stays an allocation-shaped instruction.
    let tied_use = mutated(target, |function, _| {
        function.blocks[0].instructions[2].operands[1].tied_to = Some(0);
    });
    assert_eq!(
        fold(&tied_use, &environment).unwrap_err(),
        LiteralMinuendError::UnsupportedInstruction
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
        LiteralMinuendError::UnsupportedInstruction
    );
    // A clobber the immediate form does not declare cannot be preserved.
    let clobber = mutated(target, |function, _| {
        function.blocks[0].instructions[2].clobbers = vec![register_model::RegisterUnitId(0)];
    });
    assert_eq!(
        fold(&clobber, &environment).unwrap_err(),
        LiteralMinuendError::UnsupportedInstruction
    );
    // The kind check rejects non-compare instructions.
    let not_compare = mutated(target, |function, _| {
        function.blocks[0].instructions[2].kind = SelectedInstructionKind::CopyI64;
    });
    assert_eq!(
        fold(&not_compare, &environment).unwrap_err(),
        LiteralMinuendError::UnsupportedInstruction
    );
    // A third operand is not the emitted `[use, use]` shape.
    let extra_operand = mutated(target, |function, _| {
        let operand = function.blocks[0].instructions[2].operands[1];
        function.blocks[0].instructions[2].operands.push(operand);
    });
    assert_eq!(
        fold(&extra_operand, &environment).unwrap_err(),
        LiteralMinuendError::UnsupportedInstruction
    );
    // A write operand is not a compare use.
    let writing = mutated(target, |function, _| {
        function.blocks[0].instructions[2].operands[1].access = RegisterOperandAccess::Def;
    });
    assert_eq!(
        fold(&writing, &environment).unwrap_err(),
        LiteralMinuendError::UnsupportedInstruction
    );
    // The surviving input must exist in the register roster.
    let missing_input = mutated(target, |function, _| {
        function.virtual_registers.retain(|entry| entry.id != INPUT);
    });
    assert_eq!(
        fold(&missing_input, &environment).unwrap_err(),
        LiteralMinuendError::UnsupportedUse
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
        LiteralMinuendError::ConstraintMismatch
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
        LiteralMinuendError::ConstraintMismatch
    );
    // A declared row that does not declare the `[use, use]` shape the
    // operands carry is malformed.
    let wrong_row = mutated(target, |function, environment| {
        function.blocks[0].instructions[2].constraint = environment.selected_keys().copy_i64;
    });
    assert_eq!(
        fold(&wrong_row, &environment).unwrap_err(),
        LiteralMinuendError::ConstraintMismatch
    );
    // A roster class the row does not declare for operand zero rejects.
    let wrong_class = mutated(target, |function, _| {
        function.virtual_registers[2].class = register_model::RegisterClassId(u16::MAX);
    });
    assert_eq!(
        fold(&wrong_class, &environment).unwrap_err(),
        LiteralMinuendError::ConstraintMismatch
    );
}

#[test]
fn source_identity_mismatches_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = materialize_fixture(target, IntegerValue::Unsigned(0));
    // A function index outside the plan.
    assert_eq!(
        fold_selected_literal_minuend(&source, 1, COMPARE, &environment, budget()).unwrap_err(),
        LiteralMinuendError::SourceMismatch
    );
    // An instruction id the function does not contain.
    assert_eq!(
        fold_selected_literal_minuend(
            &source,
            0,
            SelectedInstructionId(99),
            &environment,
            budget()
        )
        .unwrap_err(),
        LiteralMinuendError::SourceMismatch
    );
    // An environment for a different target than the plan's.
    let wrong_environment =
        baseline_target_register_environment(NativeTarget::macos_arm64()).unwrap();
    assert_eq!(
        fold_selected_literal_minuend(&source, 0, COMPARE, &wrong_environment, budget())
            .unwrap_err(),
        LiteralMinuendError::SourceMismatch
    );
}

#[test]
fn validation_budget_covers_the_flag_audit() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = materialize_fixture(target, IntegerValue::Unsigned(0));
    let tiny = OptimizationWorkBudget::new(100, 100, 4, 100, 100).unwrap();
    assert_eq!(
        fold_selected_literal_minuend(&source, 0, COMPARE, &environment, tiny).unwrap_err(),
        LiteralMinuendError::WorkBudgetExceeded
    );
}

/// The measured validation-step boundary: admission charges one step per
/// block plus one per instruction across the plan, repeats the same scan
/// over the admitted function to locate the literal's producers, then
/// charges the flag audit at two block traversals plus the successor edges
/// per defined flag unit — so the exact count admits the fold on both the
/// proposal and the independent replay path while one step below rejects
/// both, on the single-block fixture and on a second whose flag audit
/// crosses an edge into the consumer's block.
#[test]
fn measured_validation_step_boundary_admits_and_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The equality consumer moves into a jump target: the flag unit stays
    // live across the edge, so the audit's edge term is nonzero.
    let edge_crossing = mutated(target, |function, environment| {
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
        function.blocks[0].instructions.remove(3);
        function.blocks[0].terminator = SelectedTerminator::Jump {
            instruction: instruction(TERMINAL, SelectedInstructionKind::Jump, &jump_row, &[]),
            successor: successor(SelectedBlockId(1), BlockId::new(2).unwrap(), 2),
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
    for (source, plan_scan, function_scan, edge_count) in [
        // One block carrying four body instructions plus the terminator;
        // the flag unit dies at the return, so the audit crosses no edge.
        (
            materialize_fixture(target, IntegerValue::Unsigned(9)),
            5u64,
            5u64,
            0u64,
        ),
        // Three body instructions plus the jump in the compare's block and
        // one body instruction plus the return in the target; the flag unit
        // reaches the consumer across the jump's one successor edge.
        (edge_crossing, 6u64, 6u64, 1u64),
    ] {
        let compare = &source.transformed().functions[0].blocks[0].instructions[2];
        let exact_steps = plan_scan
            + function_scan
            + compare.implicit_defs.len() as u64 * (2 * function_scan + edge_count);
        let exact = OptimizationWorkBudget::new(1, 1, exact_steps, 1, 1).unwrap();
        let result =
            fold_selected_literal_minuend(&source, 0, COMPARE, &environment, exact).unwrap();
        validate_literal_minuend_fold(
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
            fold_selected_literal_minuend(&source, 0, COMPARE, &environment, starved).unwrap_err(),
            LiteralMinuendError::WorkBudgetExceeded
        );
        assert_eq!(
            validate_literal_minuend_fold(
                &source,
                0,
                COMPARE,
                &environment,
                starved,
                result.transformed().clone(),
            )
            .unwrap_err(),
            LiteralMinuendError::WorkBudgetExceeded
        );
    }
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
            validate_literal_minuend_fold(&source, 0, COMPARE, &environment, budget(), proposed)
                .is_err(),
            "mutation {mutation}"
        );
    }
}

/// Two runs over the identical source produce the identical validated result,
/// and the published plan is a legal second input: the sealed transformed
/// program already sits at the rule's fixed point — the compare keeps its
/// instruction identity as the selected immediate form, so a second fold at
/// the same site finds no register-register compare to admit.
#[test]
fn fold_is_deterministic_and_terminal() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let first = fold(
        &materialize_fixture(target, IntegerValue::Unsigned(9)),
        &environment,
    )
    .unwrap();
    let second = fold(
        &materialize_fixture(target, IntegerValue::Unsigned(9)),
        &environment,
    )
    .unwrap();
    assert_eq!(first, second);
    // The validated output carries the sealed analysis boundary, so it is a
    // legal second input — not merely a reconstruction of one. Re-running on
    // it is terminal: instruction COMPARE is the emitted
    // CompareI64Immediate now, not a two-register compare.
    assert_eq!(
        fold_selected_literal_minuend(&first, 0, COMPARE, &environment, budget()).unwrap_err(),
        LiteralMinuendError::UnsupportedInstruction
    );
}

/// Replay corruption in a block the fold never touched still rejects: the
/// restore-by-content check compares the complete plan, not just the block
/// carrying the rewritten compare.
#[test]
fn replay_rejects_drift_outside_the_rewritten_block() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Stretch the fixture across one edge: the compare's block folds and
    // jumps to a second block that returns.
    let source = mutated(target, |function, environment| {
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
                successor: successor(SelectedBlockId(1), BlockId::new(2).unwrap(), 2),
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
        .constraint(environment.selected_keys().copy_i64)
        .unwrap()
        .clone();
    proposed.functions[0].blocks[1]
        .instructions
        .push(instruction(
            SelectedInstructionId(11),
            SelectedInstructionKind::CopyI64,
            &copy,
            &[INPUT, SPARE],
        ));
    assert_eq!(
        validate_literal_minuend_fold(&source, 0, COMPARE, &environment, budget(), proposed)
            .unwrap_err(),
        LiteralMinuendError::ReplayMismatch
    );
}

fn fold(
    source: &ValidatedLiteralMinuend,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> Result<ValidatedLiteralMinuend, LiteralMinuendError> {
    fold_selected_literal_minuend(source, 0, COMPARE, environment, budget())
}
