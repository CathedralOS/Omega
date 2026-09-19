use crate::ConditionMaterializationError;
use crate::ConditionMaterializationReceipt;
use crate::ValidatedConditionMaterialization;
use crate::ValidatedSelectedAnalysis;
use crate::fold_selected_condition_materialization;
use crate::validate_condition_materialization_fold;
use crate::validated_machine_effect_catalog;
use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget};
use optimization_unit::ValueDefinitionSite;
use register_environment::{
    ValidatedTargetRegisterEnvironment, baseline_target_register_environment,
};
use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan, SelectedOperand,
    SelectedSuccessor, SelectedSuccessorRole, SelectedTerminator, ValidatedMachineEffectCatalog,
    VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, MachineId,
    OperationId, ScalarType, ValueId,
};
use target::NativeTarget;
use target_operations_to_selected_instructions::selected_instruction_plan_identity;
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

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

const MATERIALIZE_A: SelectedInstructionId = SelectedInstructionId(2);
const MATERIALIZE_B: SelectedInstructionId = SelectedInstructionId(3);
const COMPARE: SelectedInstructionId = SelectedInstructionId(4);
const BOOLEAN: SelectedInstructionId = SelectedInstructionId(5);
const EXTRA: SelectedInstructionId = SelectedInstructionId(7);
const LITA: VirtualRegisterId = VirtualRegisterId(0);
const LITB: VirtualRegisterId = VirtualRegisterId(1);
const PARAM: VirtualRegisterId = VirtualRegisterId(2);
const OUTPUT: VirtualRegisterId = VirtualRegisterId(3);

/// Which `MaterializeBoolean*` kind the fixture's consumer carries.
#[derive(Clone, Copy)]
enum MaterializeShape {
    Equal,
    U64LessThan,
    I64LessThan,
    U64LessOrEqual,
    I64LessOrEqual,
}

impl MaterializeShape {
    fn kind(self) -> SelectedInstructionKind {
        match self {
            Self::Equal => SelectedInstructionKind::MaterializeBooleanEqual,
            Self::U64LessThan => SelectedInstructionKind::MaterializeBooleanU64LessThan,
            Self::I64LessThan => SelectedInstructionKind::MaterializeBooleanI64LessThan,
            Self::U64LessOrEqual => SelectedInstructionKind::MaterializeBooleanU64LessOrEqual,
            Self::I64LessOrEqual => SelectedInstructionKind::MaterializeBooleanI64LessOrEqual,
        }
    }
}

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

/// A raw selected-stage unit fixture, not a source/Terminal admission claim:
/// `ra = materialize va; rb = materialize vb; compare ra, rb;
/// out = materialize_boolean_*;` then a return terminator — the consumer is
/// the flag-reading body instruction whose implicit uses observe the
/// compare's published condition state. Variants rebuild the consumer's
/// kind through `shape`.
fn fixture(
    target: NativeTarget,
    left_value: IntegerValue,
    right_value: IntegerValue,
    shape: MaterializeShape,
) -> ValidatedConditionMaterialization {
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
    let mut consumer = instruction(BOOLEAN, shape.kind(), boolean, &[OUTPUT]);
    consumer.provenance.operations = vec![OperationId::new(9).unwrap()];
    consumer.provenance.values = vec![ValueId::new(5).unwrap()];
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
                    consumer,
                ],
                terminator: SelectedTerminator::Return {
                    instruction: instruction(
                        SelectedInstructionId(6),
                        SelectedInstructionKind::ReturnUnit,
                        terminal_row,
                        &[],
                    ),
                    psi_return_edge: EdgeId::new(4).unwrap(),
                },
            }],
        }]
        .into(),
    };
    let identity = selected_instruction_plan_identity(&plan);
    ValidatedConditionMaterialization {
        receipt: ConditionMaterializationReceipt {
            source_selected: identity,
            transformed_selected: identity,
            optimization_unit: OptimizationUnitIdentity::from_bytes([2; 32]),
            fuel_schedule: plan.fuel_schedule,
        },
        transformed: std::sync::Arc::new(plan),
    }
}

fn keys(
    environment: &ValidatedTargetRegisterEnvironment,
) -> selected_instructions::SelectedConstraintKeys {
    environment.selected_keys()
}

/// The machine-effect catalog bound to `environment` for the plan's target —
/// the catalog the pair declarations resolve their effect surface in.
fn catalog(
    source: &ValidatedConditionMaterialization,
    environment: &ValidatedTargetRegisterEnvironment,
) -> ValidatedMachineEffectCatalog {
    validated_machine_effect_catalog(source.transformed().target, environment.constraints())
        .unwrap()
}

/// Edit the single fixture function, then refresh the receipt identities so
/// the mutated plan is a well-formed analysis source.
fn mutated(
    target: NativeTarget,
    edit: impl FnOnce(&mut SelectedFunction, &ValidatedTargetRegisterEnvironment),
) -> ValidatedConditionMaterialization {
    let environment = baseline_target_register_environment(target).unwrap();
    let mut source = fixture(
        target,
        IntegerValue::Unsigned(3),
        IntegerValue::Unsigned(5),
        MaterializeShape::Equal,
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
    source: &ValidatedConditionMaterialization,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<ValidatedConditionMaterialization, ConditionMaterializationError> {
    let effect_catalog = catalog(source, environment);
    fold_selected_condition_materialization(
        source,
        0,
        BOOLEAN,
        environment,
        &effect_catalog,
        budget(),
    )
}

fn replay(
    source: &ValidatedConditionMaterialization,
    environment: &ValidatedTargetRegisterEnvironment,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedConditionMaterialization, ConditionMaterializationError> {
    let effect_catalog = catalog(source, environment);
    validate_condition_materialization_fold(
        source,
        0,
        BOOLEAN,
        environment,
        &effect_catalog,
        budget(),
        proposed,
    )
}

/// The instruction a successful fold publishes: `MaterializeI64` on the
/// consumer's own instruction identity carrying `value`, at `position` in
/// `block_index`'s body.
fn materialized(
    result: &ValidatedConditionMaterialization,
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

/// Every declared (producer, consumer) pair folds on every target: the
/// consumer's own identity, provenance, and `Def` operand carry onto the
/// `MaterializeI64` under the target's own row while the compare stays for
/// other readers, and the independent replay re-derives all of it without
/// the descriptor table.
#[test]
fn predicate_table_folds_on_all_targets() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let cases: &[(MaterializeShape, u64, u64, u64)] = &[
            (MaterializeShape::Equal, 7, 7, 1),
            (MaterializeShape::Equal, 7, 8, 0),
            (MaterializeShape::U64LessThan, 3, 9, 1),
            (MaterializeShape::U64LessThan, 9, 3, 0),
            // Unsigned ordering reads the published bit patterns: the
            // all-ones literal is the u64 maximum, not a negative.
            (MaterializeShape::U64LessThan, u64::MAX, 0, 0),
            (MaterializeShape::I64LessThan, u64::MAX, 0, 1),
            (MaterializeShape::I64LessThan, 0, u64::MAX, 0),
            (MaterializeShape::I64LessThan, i64::MIN as u64, 0, 1),
            (MaterializeShape::U64LessOrEqual, 4, 4, 1),
            (MaterializeShape::U64LessOrEqual, 9, 3, 0),
            (MaterializeShape::I64LessOrEqual, u64::MAX, 0, 1),
            (MaterializeShape::I64LessOrEqual, 1, 0, 0),
        ];
        for (shape, left, right, expected) in cases {
            let source = fixture(
                target,
                IntegerValue::Unsigned(u128::from(*left)),
                IntegerValue::Unsigned(u128::from(*right)),
                *shape,
            );
            let result = fold(&source, &environment).unwrap();
            let folded = materialized(&result, 0, 3, *expected);
            assert_eq!(
                folded.constraint,
                keys(&environment).materialize_i64,
                "{target:?} {left} {right}"
            );
            let materialize_row = environment
                .constraint(keys(&environment).materialize_i64)
                .unwrap();
            // The materialize row's implicit surface replaces the
            // consumer's: the decided flag uses retire entirely.
            assert_eq!(folded.implicit_uses, materialize_row.implicit_uses);
            assert_eq!(folded.implicit_defs, materialize_row.implicit_defs);
            assert_eq!(folded.clobbers, materialize_row.clobbers);
            // The consumer's `Def` operand record and provenance carry
            // verbatim: the decided value lands in the same register home.
            let consumer = &source.transformed().functions[0].blocks[0].instructions[3];
            assert_eq!(folded.operands, consumer.operands);
            assert_eq!(folded.provenance, consumer.provenance);
            // The compare keeps publishing flag state for other readers.
            assert_eq!(
                result.transformed().functions[0].blocks[0].instructions[2].kind,
                SelectedInstructionKind::CompareI64
            );
            assert_eq!(
                result.receipt().source_selected(),
                source.selected_identity()
            );
            assert_eq!(
                result.receipt().transformed_selected(),
                selected_instruction_plan_identity(result.transformed())
            );
            replay(&source, &environment, result.transformed().clone()).unwrap();
            // A detached, separately allocated proposal replays by content.
            let mut detached = result.transformed().clone();
            detached.functions = detached.functions.iter().cloned().collect();
            replay(&source, &environment, detached).unwrap();
        }
    }
}

/// The declared `CompareI64Immediate` producer pairs fold the same grammar:
/// `literal - immediate` needs no second producer.
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

/// The declared `CompareI64Zero` producer pairs fold the same grammar:
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
        materialized(&result, 0, 3, expected);
    }
}

/// `register - register` is zero on every lane: the flag state is constant
/// whatever the register holds, so no literal producer is needed at all —
/// here the compared register's producer is a copy of an entry parameter,
/// which could never witness a literal.
#[test]
fn same_register_compare_folds_without_literals() {
    let target = NativeTarget::linux_arm64();
    let environment = baseline_target_register_environment(target).unwrap();
    for (shape, expected) in [
        (MaterializeShape::Equal, 1u64),
        (MaterializeShape::U64LessThan, 0),
        (MaterializeShape::I64LessThan, 0),
        (MaterializeShape::U64LessOrEqual, 1),
        (MaterializeShape::I64LessOrEqual, 1),
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
            function.blocks[0].instructions[3] =
                instruction(BOOLEAN, shape.kind(), boolean, &[OUTPUT]);
        });
        // `r - r` is zero: equality and both less-or-equals publish one,
        // both strict less-thans publish zero.
        let result = fold(&source, &environment).unwrap();
        materialized(&result, 0, 3, expected);
    }
}

/// A flag unit reaching in through a predecessor edge resolves to the
/// compare when every path to the consumer last observed the same
/// condition-state event — here the compare lives in the entry block and
/// the materialization in its jump target.
#[test]
fn cross_block_flag_reaching_folds() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let terminal_row = environment
            .constraint(environment.selected_keys().return_unit)
            .unwrap()
            .clone();
        // The entry block keeps its materializations and compare, then
        // jumps to a new block carrying the consumer.
        let consumer = function.blocks[0].instructions.remove(3);
        function.blocks[0].terminator = jump_terminator(environment, 6, 1, 2);
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
    let result = fold(&source, &environment).unwrap();
    // `3 - 5` is nonzero: equality does not hold.
    materialized(&result, 1, 0, 0);
    replay(&source, &environment, result.transformed().clone()).unwrap();
}

/// A join carries the same constant flag state: both event-free
/// predecessors pass the compare's event through, so the materialization on
/// the joined block resolves to the one compare and folds.
#[test]
fn cross_block_join_reaching_folds() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let terminal_row = environment
            .constraint(environment.selected_keys().return_unit)
            .unwrap()
            .clone();
        let branch_row = environment
            .constraint(environment.selected_keys().conditional_branch)
            .unwrap()
            .clone();
        let consumer = function.blocks[0].instructions.remove(3);
        // The entry block branches on the same flags to two intermediate
        // blocks; both jump to the consumer's join block.
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
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(1),
            origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
            instructions: Vec::new(),
            terminator: jump_terminator(environment, 8, 3, 4),
        });
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(2),
            origin: SelectedBlockOrigin::Source(BlockId::new(3).unwrap()),
            instructions: Vec::new(),
            terminator: jump_terminator(environment, 9, 3, 5),
        });
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(3),
            origin: SelectedBlockOrigin::Source(BlockId::new(4).unwrap()),
            instructions: vec![consumer],
            terminator: SelectedTerminator::Return {
                instruction: instruction(
                    SelectedInstructionId(10),
                    SelectedInstructionKind::ReturnUnit,
                    &terminal_row,
                    &[],
                ),
                psi_return_edge: EdgeId::new(6).unwrap(),
            },
        });
    });
    let result = fold(&source, &environment).unwrap();
    materialized(&result, 3, 0, 0);
    // The entry block's own conditional branch is untouched.
    assert!(matches!(
        result.transformed().functions[0].blocks[0].terminator,
        SelectedTerminator::ConditionalBranch { .. }
    ));
}

/// A path last touched by a different flag event — here a second compare
/// on a non-literal operand behind the sibling branch — refuses: the
/// observed condition state is not provably the constant compare's.
#[test]
fn cross_block_divergent_reaching_refuses() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let terminal_row = environment
            .constraint(environment.selected_keys().return_unit)
            .unwrap()
            .clone();
        let branch_row = environment
            .constraint(environment.selected_keys().conditional_branch)
            .unwrap()
            .clone();
        let compare_row = environment
            .constraint(environment.selected_keys().compare_i64)
            .unwrap()
            .clone();
        let consumer = function.blocks[0].instructions.remove(3);
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
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(1),
            origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
            instructions: Vec::new(),
            terminator: jump_terminator(environment, 8, 3, 4),
        });
        // The other predecessor republishes the flag units from a different
        // instruction before reaching the consumer's join.
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(2),
            origin: SelectedBlockOrigin::Source(BlockId::new(3).unwrap()),
            instructions: vec![instruction(
                EXTRA,
                SelectedInstructionKind::CompareI64,
                &compare_row,
                &[LITA, PARAM],
            )],
            terminator: jump_terminator(environment, 9, 3, 5),
        });
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(3),
            origin: SelectedBlockOrigin::Source(BlockId::new(4).unwrap()),
            instructions: vec![consumer],
            terminator: SelectedTerminator::Return {
                instruction: instruction(
                    SelectedInstructionId(10),
                    SelectedInstructionKind::ReturnUnit,
                    &terminal_row,
                    &[],
                ),
                psi_return_edge: EdgeId::new(6).unwrap(),
            },
        });
    });
    assert_eq!(
        fold(&source, &environment).unwrap_err(),
        ConditionMaterializationError::UnsupportedUse
    );
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
        ConditionMaterializationError::UnsupportedUse
    );
}

/// An intervening flag event — a second compare or a clobber between the
/// compare and the consumer in the same block — makes the observed state
/// the later instruction's, not the constant compare's.
#[test]
fn intervening_flag_events_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A second compare on a non-materialized operand is the reaching
    // producer; its right side has no literal producer, so the declared
    // operand grammar cannot decide the state.
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
        ConditionMaterializationError::UndecidedOperands
    );
    // A clobber of the flag unit between compare and consumer leaves the
    // observed state unknown.
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
        ConditionMaterializationError::UnsupportedUse
    );
    // A clobber carried by the terminator between blocks ends the reach
    // the same way.
    let terminator_clobbered = mutated(target, |function, environment| {
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
        fold(&terminator_clobbered, &environment).unwrap_err(),
        ConditionMaterializationError::UnsupportedUse
    );
}

/// A used unit outside the flag universe must lie in the materialize row's
/// implicit surface: `MaterializeI64` reads nothing at all, so any non-flag
/// observation refuses under the declared unit-flow axis.
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
        ConditionMaterializationError::UnsupportedUse
    );
}

/// A materialization whose implicit definitions or clobbers the
/// `MaterializeI64` row cannot republish refuses: the rebuilt instruction
/// would change the unit events downstream readers observe at this site.
#[test]
fn unpublishable_unit_traffic_refuses() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A defined unit is not republished by the materialize row.
    let extra_def = mutated(target, |function, _| {
        function.blocks[0].instructions[3]
            .implicit_defs
            .push(register_model::RegisterUnitId(997));
    });
    assert_eq!(
        fold(&extra_def, &environment).unwrap_err(),
        ConditionMaterializationError::UnsupportedUse
    );
    // A clobber the materialize row lacks would lose the event entirely.
    let clobbering = mutated(target, |function, _| {
        function.blocks[0].instructions[3].clobbers = vec![register_model::RegisterUnitId(996)];
    });
    assert_eq!(
        fold(&clobbering, &environment).unwrap_err(),
        ConditionMaterializationError::UnsupportedUse
    );
}

/// Only the declared materialization consumers fold; anything else — a
/// non-materialization kind, a flag-free record, a `Use` operand, or an
/// absent id — refuses.
#[test]
fn non_materialization_consumers_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(
        target,
        IntegerValue::Unsigned(3),
        IntegerValue::Unsigned(5),
        MaterializeShape::Equal,
    );
    let effect_catalog = catalog(&source, &environment);
    // The compare itself is not a flag-reading materialization.
    assert_eq!(
        fold_selected_condition_materialization(
            &source,
            0,
            COMPARE,
            &environment,
            &effect_catalog,
            budget()
        )
        .unwrap_err(),
        ConditionMaterializationError::UnsupportedConsumer
    );
    // Neither is the producer materialization.
    assert_eq!(
        fold_selected_condition_materialization(
            &source,
            0,
            MATERIALIZE_A,
            &environment,
            &effect_catalog,
            budget()
        )
        .unwrap_err(),
        ConditionMaterializationError::UnsupportedConsumer
    );
    // An absent instruction id cannot be located.
    assert_eq!(
        fold_selected_condition_materialization(
            &source,
            0,
            SelectedInstructionId(99),
            &environment,
            &effect_catalog,
            budget()
        )
        .unwrap_err(),
        ConditionMaterializationError::UnsupportedConsumer
    );
    // A flag-free materialization has no condition to evaluate.
    let flag_free = mutated(target, |function, _| {
        function.blocks[0].instructions[3].implicit_uses.clear();
    });
    assert_eq!(
        fold(&flag_free, &environment).unwrap_err(),
        ConditionMaterializationError::UnsupportedUse
    );
    // A `Use` operand is not the declared single-`Def` shape.
    let operandful = mutated(target, |function, _| {
        let class = function.blocks[0].instructions[3].operands[0].class;
        function.blocks[0].instructions[3]
            .operands
            .push(SelectedOperand {
                operand: 1,
                virtual_register: PARAM,
                access: RegisterOperandAccess::Use,
                class,
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            });
    });
    assert_eq!(
        fold(&operandful, &environment).unwrap_err(),
        ConditionMaterializationError::UnsupportedConsumer
    );
    // A body instruction kind the family never declares — here a copy —
    // refuses even where its operand shape coincides.
    let wrong_kind = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        let boolean = environment
            .constraint(environment.selected_keys().materialize_boolean)
            .unwrap();
        let mut instruction = instruction(
            BOOLEAN,
            SelectedInstructionKind::CopyI64,
            copy,
            &[PARAM, OUTPUT],
        );
        instruction.implicit_uses = boolean.implicit_uses.clone();
        function.blocks[0].instructions[3] = instruction;
    });
    assert_eq!(
        fold(&wrong_kind, &environment).unwrap_err(),
        ConditionMaterializationError::UnsupportedConsumer
    );
}

/// The producer the flag uses resolve to must be a declared condition-state
/// producer: a non-compare instruction that publishes the same flag units
/// — a saturating divide defines flags as a side channel — refuses.
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
        ConditionMaterializationError::UnsupportedProducer
    );
}

/// Non-literal and multiply-defined operand registers refuse: the constant
/// guarantee must hold wherever the compare could read the register.
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
        ConditionMaterializationError::UndecidedOperands
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
        ConditionMaterializationError::UndecidedOperands
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
        ConditionMaterializationError::UndecidedOperands
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
            MaterializeShape::Equal,
        );
        assert_eq!(
            fold(&source, &environment).unwrap_err(),
            ConditionMaterializationError::UndecidedOperands,
            "{value:?}"
        );
    }
}

/// The bound catalog must describe the plan's target and the environment's
/// constraint inventory: a catalog built for another target — or a record
/// whose constraint key no declaration binds to its semantic — cannot
/// attest the effect relationship.
#[test]
fn effect_surface_mismatches_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(
        target,
        IntegerValue::Unsigned(3),
        IntegerValue::Unsigned(5),
        MaterializeShape::Equal,
    );
    // A catalog bound to a different target and constraint inventory.
    let foreign_environment =
        baseline_target_register_environment(NativeTarget::linux_arm64()).unwrap();
    let foreign_catalog = validated_machine_effect_catalog(
        NativeTarget::linux_arm64(),
        foreign_environment.constraints(),
    )
    .unwrap();
    assert_eq!(
        fold_selected_condition_materialization(
            &source,
            0,
            BOOLEAN,
            &environment,
            &foreign_catalog,
            budget()
        )
        .unwrap_err(),
        ConditionMaterializationError::EffectSurfaceMismatch
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
        ConditionMaterializationError::EffectSurfaceMismatch
    );
    // The same defects refuse on the replay path: the proposed
    // materialization cannot carry the fold the bound catalog never
    // declared.
    let result = fold(&source, &environment).unwrap();
    assert_eq!(
        validate_condition_materialization_fold(
            &source,
            0,
            BOOLEAN,
            &environment,
            &foreign_catalog,
            budget(),
            result.transformed().clone()
        )
        .unwrap_err(),
        ConditionMaterializationError::EffectSurfaceMismatch
    );
    let mut detached = result.transformed().clone();
    detached.functions = detached.functions.iter().cloned().collect();
    let effect_catalog = catalog(&wrong_row, &environment);
    assert_eq!(
        validate_condition_materialization_fold(
            &wrong_row,
            0,
            BOOLEAN,
            &environment,
            &effect_catalog,
            budget(),
            detached
        )
        .unwrap_err(),
        ConditionMaterializationError::EffectSurfaceMismatch
    );
}

/// The two flag consumers compose on the same published compare: the
/// materialization folds to its literal while a conditional-branch
/// terminator keeps reading the compare's flags — and the terminator fold
/// then accepts this fold's validated output.
#[test]
fn validated_results_compose_with_terminator_pair() {
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
    // The boolean materialization folds first; the terminator pair then
    // consumes the same compare from the transformed plan.
    let materialization_fold = fold(&source, &environment).unwrap();
    materialized(&materialization_fold, 0, 3, 0);
    let effect_catalog = catalog(&source, &environment);
    let result = crate::fold_selected_terminator_pair(
        &materialization_fold,
        0,
        SelectedInstructionId(6),
        &environment,
        &effect_catalog,
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

/// A proposal that does not match the reconstructed `MaterializeI64` — or
/// that retains the source instruction — fails the independent replay.
#[test]
fn replay_mismatches_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(
        target,
        IntegerValue::Unsigned(3),
        IntegerValue::Unsigned(5),
        MaterializeShape::Equal,
    );
    // The unchanged source is not a rewritten proposal.
    assert_eq!(
        replay(&source, &environment, source.transformed().clone()).unwrap_err(),
        ConditionMaterializationError::ReplayMismatch
    );
    // The wrong decided value is not the reconstruction.
    let mut wrong = source.transformed().clone();
    let materialize_row = environment
        .constraint(keys(&environment).materialize_i64)
        .unwrap()
        .clone();
    let consumer = source.transformed().functions[0].blocks[0].instructions[3].clone();
    wrong.functions[0].blocks[0].instructions[3] = SelectedInstruction {
        kind: SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(1),
        },
        constraint: materialize_row.key,
        operands: consumer.operands.clone(),
        implicit_uses: materialize_row.implicit_uses.clone(),
        implicit_defs: materialize_row.implicit_defs.clone(),
        clobbers: materialize_row.clobbers.clone(),
        ..consumer
    };
    assert_eq!(
        replay(&source, &environment, wrong).unwrap_err(),
        ConditionMaterializationError::ReplayMismatch
    );
    // A drifted instruction identity is not the reconstruction either.
    let mut drifted = source.transformed().clone();
    let mut expected = source.transformed().functions[0].blocks[0].instructions[3].clone();
    expected.id = BOOLEAN;
    expected.kind = SelectedInstructionKind::MaterializeI64 {
        value: IntegerValue::Unsigned(0),
    };
    expected.constraint = materialize_row.key;
    expected.implicit_uses = materialize_row.implicit_uses.clone();
    expected.implicit_defs = materialize_row.implicit_defs.clone();
    expected.clobbers = materialize_row.clobbers.clone();
    expected.id = SelectedInstructionId(42);
    drifted.functions[0].blocks[0].instructions[3] = expected;
    assert_eq!(
        replay(&source, &environment, drifted).unwrap_err(),
        ConditionMaterializationError::ReplayMismatch
    );
}

/// Drift outside the rewritten block — any other instruction, register,
/// terminator, or roster entry — restores to a source that is not the
/// input plan and refuses.
#[test]
fn replay_rejects_drift_outside_the_rewritten_block() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(
        target,
        IntegerValue::Unsigned(3),
        IntegerValue::Unsigned(5),
        MaterializeShape::Equal,
    );
    let result = fold(&source, &environment).unwrap();
    // A different literal in the untouched producer materialization.
    let mut drifted = result.transformed().clone();
    drifted.functions[0].blocks[0].instructions[0].kind = SelectedInstructionKind::MaterializeI64 {
        value: IntegerValue::Unsigned(11),
    };
    assert_eq!(
        replay(&source, &environment, drifted).unwrap_err(),
        ConditionMaterializationError::ReplayMismatch
    );
    // A changed register roster row.
    let mut drifted = result.transformed().clone();
    drifted.functions[0].virtual_registers[0].scalar_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
    assert_eq!(
        replay(&source, &environment, drifted).unwrap_err(),
        ConditionMaterializationError::ReplayMismatch
    );
    // A changed provenance on the folded instruction's sibling.
    let mut drifted = result.transformed().clone();
    drifted.functions[0].blocks[0].instructions[2]
        .provenance
        .operations = vec![OperationId::new(77).unwrap()];
    assert_eq!(
        replay(&source, &environment, drifted).unwrap_err(),
        ConditionMaterializationError::ReplayMismatch
    );
}

#[test]
fn source_identity_mismatches_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(
        target,
        IntegerValue::Unsigned(3),
        IntegerValue::Unsigned(5),
        MaterializeShape::Equal,
    );
    let effect_catalog = catalog(&source, &environment);
    // A function index outside the plan.
    assert_eq!(
        fold_selected_condition_materialization(
            &source,
            1,
            BOOLEAN,
            &environment,
            &effect_catalog,
            budget()
        )
        .unwrap_err(),
        ConditionMaterializationError::SourceMismatch
    );
    // An environment for a different target than the plan's.
    let wrong_environment =
        baseline_target_register_environment(NativeTarget::macos_arm64()).unwrap();
    assert_eq!(
        fold_selected_condition_materialization(
            &source,
            0,
            BOOLEAN,
            &wrong_environment,
            &effect_catalog,
            budget()
        )
        .unwrap_err(),
        ConditionMaterializationError::SourceMismatch
    );
}

#[test]
fn validation_budget_covers_the_flag_scan() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(
        target,
        IntegerValue::Unsigned(3),
        IntegerValue::Unsigned(5),
        MaterializeShape::Equal,
    );
    let tiny = OptimizationWorkBudget::new(100, 100, 4, 100, 100).unwrap();
    let effect_catalog = catalog(&source, &environment);
    assert_eq!(
        fold_selected_condition_materialization(
            &source,
            0,
            BOOLEAN,
            &environment,
            &effect_catalog,
            tiny
        )
        .unwrap_err(),
        ConditionMaterializationError::WorkBudgetExceeded
    );
}

/// Two runs over the identical source produce the identical validated
/// result, and the published plan is a legal second input: re-running at
/// the same site is terminal because the instruction is now a
/// `MaterializeI64`, not a flag-reading materialization.
#[test]
fn fold_is_deterministic_and_terminal() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let first = fold(
        &fixture(
            target,
            IntegerValue::Unsigned(3),
            IntegerValue::Unsigned(5),
            MaterializeShape::Equal,
        ),
        &environment,
    )
    .unwrap();
    let second = fold(
        &fixture(
            target,
            IntegerValue::Unsigned(3),
            IntegerValue::Unsigned(5),
            MaterializeShape::Equal,
        ),
        &environment,
    )
    .unwrap();
    assert_eq!(first, second);
    let effect_catalog = catalog(&first, &environment);
    assert_eq!(
        fold_selected_condition_materialization(
            &first,
            0,
            BOOLEAN,
            &environment,
            &effect_catalog,
            budget()
        )
        .unwrap_err(),
        ConditionMaterializationError::UnsupportedConsumer
    );
}
