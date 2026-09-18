use crate::TerminatorPairError;
use crate::TerminatorPairReceipt;
use crate::ValidatedSelectedAnalysis;
use crate::ValidatedTerminatorPair;
use crate::fold_selected_terminator_pair;
use crate::validate_terminator_pair_fold;
use crate::validated_machine_effect_catalog;
use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget};
use optimization_unit::{FuelSettlement, PsiProvenance, ValueDefinitionSite};
use register_environment::{
    ValidatedTargetRegisterEnvironment, baseline_target_register_environment,
};
use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan, SelectedOperand,
    SelectedSuccessor, SelectedSuccessorRole, SelectedTerminator, SelectedValueBinding,
    SelectedValueTransport, ValidatedMachineEffectCatalog, VirtualRegister, VirtualRegisterId,
    VirtualRegisterOrigin,
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
const BRANCH: SelectedInstructionId = SelectedInstructionId(5);
const EXTRA: SelectedInstructionId = SelectedInstructionId(7);
const LITA: VirtualRegisterId = VirtualRegisterId(0);
const LITB: VirtualRegisterId = VirtualRegisterId(1);
const PARAM: VirtualRegisterId = VirtualRegisterId(2);
const OUTPUT: VirtualRegisterId = VirtualRegisterId(3);

/// Which conditional-branch terminator and instruction kind the fixture's
/// entry block carries.
#[derive(Clone, Copy)]
enum BranchShape {
    NonZero,
    U64LessThan,
    I64LessThan,
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

/// The terminator record the branch carries, rebuilt onto `instruction`
/// with the two named arms.
fn branch_terminator(
    shape: BranchShape,
    branch_instruction: SelectedInstruction,
    taken: SelectedSuccessor,
    untaken: SelectedSuccessor,
) -> SelectedTerminator {
    match shape {
        BranchShape::NonZero => SelectedTerminator::ConditionalBranch {
            instruction: branch_instruction,
            when_nonzero: taken,
            when_zero: untaken,
        },
        BranchShape::U64LessThan => SelectedTerminator::ConditionalBranchU64LessThan {
            instruction: branch_instruction,
            when_less: taken,
            when_not_less: untaken,
        },
        BranchShape::I64LessThan => SelectedTerminator::ConditionalBranchI64LessThan {
            instruction: branch_instruction,
            when_less: taken,
            when_not_less: untaken,
        },
    }
}

/// A raw selected-stage unit fixture, not a source/Terminal admission claim:
/// `ra = materialize va; rb = materialize vb; compare ra, rb;` then a
/// conditional branch on the published flags to two returning blocks. The
/// `when_nonzero`/`when_less` arm carries a register binding and a fuel
/// settlement so the fold's verbatim successor move is observable.
/// Variants rebuild the terminator's variant/kind pair through `mutated`.
fn fixture(
    target: NativeTarget,
    left_value: IntegerValue,
    right_value: IntegerValue,
    shape: BranchShape,
) -> ValidatedTerminatorPair {
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let materialize = environment.constraint(keys.materialize_i64).unwrap();
    let compare = environment.constraint(keys.compare_i64).unwrap();
    let branch_row = environment.constraint(keys.conditional_branch).unwrap();
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
                instruction: EXTRA,
                source_value: ValueId::new(6).unwrap(),
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
    // The decided arm's record carries cargo the fold must move verbatim.
    let mut taken = arm(SelectedBlockId(1), 2, 2);
    taken.bindings.push(SelectedValueBinding {
        semantic: abstract_operations::ValueBinding {
            parameter: ValueId::new(8).unwrap(),
            argument: ValueId::new(4).unwrap(),
            scalar_type,
        },
        transport: SelectedValueTransport::Registers {
            argument: PARAM,
            parameter: PARAM,
        },
    });
    taken.fuel.push(FuelSettlement {
        site: PsiProvenance::Edge(EdgeId::new(2).unwrap()),
        units: 3,
    });
    let untaken = arm(SelectedBlockId(2), 3, 3);
    let mut branch_instruction = instruction(
        BRANCH,
        match shape {
            BranchShape::NonZero => SelectedInstructionKind::ConditionalBranchNonZero,
            BranchShape::U64LessThan => SelectedInstructionKind::ConditionalBranchU64LessThan,
            BranchShape::I64LessThan => SelectedInstructionKind::ConditionalBranchI64LessThan,
        },
        branch_row,
        &[],
    );
    branch_instruction.provenance.edges = vec![EdgeId::new(2).unwrap(), EdgeId::new(3).unwrap()];
    let terminator = branch_terminator(shape, branch_instruction, taken, untaken);
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
            blocks: vec![
                SelectedBlock {
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
                    ],
                    terminator,
                },
                returning(1, 8, 4, terminal_row),
                returning(2, 9, 5, terminal_row),
            ],
        }]
        .into(),
    };
    let identity = selected_instruction_plan_identity(&plan);
    ValidatedTerminatorPair {
        receipt: TerminatorPairReceipt {
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
/// the catalog the pair declarations resolve their control surface in.
fn catalog(
    source: &ValidatedTerminatorPair,
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
) -> ValidatedTerminatorPair {
    let environment = baseline_target_register_environment(target).unwrap();
    let mut source = fixture(
        target,
        IntegerValue::Unsigned(3),
        IntegerValue::Unsigned(5),
        BranchShape::NonZero,
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
    source: &ValidatedTerminatorPair,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<ValidatedTerminatorPair, TerminatorPairError> {
    let effect_catalog = catalog(source, environment);
    fold_selected_terminator_pair(source, 0, BRANCH, environment, &effect_catalog, budget())
}

fn replay(
    source: &ValidatedTerminatorPair,
    environment: &ValidatedTargetRegisterEnvironment,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedTerminatorPair, TerminatorPairError> {
    let effect_catalog = catalog(source, environment);
    validate_terminator_pair_fold(
        source,
        0,
        BRANCH,
        environment,
        &effect_catalog,
        budget(),
        proposed,
    )
}

/// A jump terminator on `block_index` targeting `arm`, rebuilt from the
/// fixture's own jump row.
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

/// The terminator a successful fold publishes: `Jump` on the branch's own
/// instruction identity to `block`.
fn jumped_to(
    result: &ValidatedTerminatorPair,
    block_index: usize,
    block: u32,
) -> &SelectedTerminator {
    let terminator = &result.transformed().functions[0].blocks[block_index].terminator;
    let SelectedTerminator::Jump {
        instruction,
        successor,
    } = terminator
    else {
        panic!("expected a Jump terminator, found {terminator:?}");
    };
    assert_eq!(instruction.id, BRANCH);
    assert_eq!(instruction.kind, SelectedInstructionKind::Jump);
    assert_eq!(successor.block, SelectedBlockId(block));
    terminator
}

/// The record the branch would select for `(left, right)`, read off the
/// source terminator — the reference the moved successor must equal.
fn expected_arm(terminator: &SelectedTerminator, left: u64, right: u64) -> &SelectedSuccessor {
    match terminator {
        SelectedTerminator::ConditionalBranch {
            when_nonzero,
            when_zero,
            ..
        } => {
            if left != right {
                when_nonzero
            } else {
                when_zero
            }
        }
        SelectedTerminator::ConditionalBranchU64LessThan {
            when_less,
            when_not_less,
            ..
        }
        | SelectedTerminator::ConditionalBranchI64LessThan {
            when_less,
            when_not_less,
            ..
        } => {
            let holds = if matches!(
                terminator,
                SelectedTerminator::ConditionalBranchI64LessThan { .. }
            ) {
                (left as i64) < (right as i64)
            } else {
                left < right
            };
            if holds { when_less } else { when_not_less }
        }
        _ => unreachable!(),
    }
}

/// Every declared (producer, consumer) pair folds on every target, moving
/// the decided successor record — bindings and fuel included — onto the
/// `Jump` verbatim while the untaken edge leaves the plan, and the
/// independent replay re-derives all of it without the descriptor table.
#[test]
fn predicate_table_folds_on_all_targets() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let cases: &[(BranchShape, u64, u64, u32)] = &[
            (BranchShape::NonZero, 7, 7, 2),
            (BranchShape::NonZero, 7, 8, 1),
            (BranchShape::U64LessThan, 3, 9, 1),
            (BranchShape::U64LessThan, 9, 3, 2),
            // Unsigned ordering reads the published bit patterns: the
            // all-ones literal is the u64 maximum, not a negative.
            (BranchShape::U64LessThan, u64::MAX, 0, 2),
            (BranchShape::I64LessThan, u64::MAX, 0, 1),
            (BranchShape::I64LessThan, 0, u64::MAX, 2),
            (BranchShape::I64LessThan, i64::MIN as u64, 0, 1),
        ];
        for (shape, left, right, expected) in cases {
            let source = fixture(
                target,
                IntegerValue::Unsigned(u128::from(*left)),
                IntegerValue::Unsigned(u128::from(*right)),
                *shape,
            );
            let result = fold(&source, &environment).unwrap();
            let terminator = jumped_to(&result, 0, *expected);
            let SelectedTerminator::Jump {
                instruction: jumped_instruction,
                successor: jumped_arm,
            } = terminator
            else {
                unreachable!()
            };
            assert_eq!(jumped_instruction.constraint, keys(&environment).jump);
            assert!(jumped_instruction.operands.is_empty());
            // The jump row's implicit surface replaces the branch's: the
            // decided flag uses retire, the program-counter traffic stays.
            let jump_row = environment.constraint(keys(&environment).jump).unwrap();
            assert_eq!(jumped_instruction.implicit_uses, jump_row.implicit_uses);
            assert_eq!(jumped_instruction.implicit_defs, jump_row.implicit_defs);
            let source_terminator = &source.transformed().functions[0].blocks[0].terminator;
            let branch_instruction = match source_terminator {
                SelectedTerminator::ConditionalBranch { instruction, .. }
                | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
                | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. } => {
                    instruction
                }
                _ => unreachable!(),
            };
            assert_eq!(jumped_instruction.provenance, branch_instruction.provenance);
            // The decided record moved verbatim — binding and fuel
            // settlement included.
            assert_eq!(
                jumped_arm,
                expected_arm(source_terminator, *left, *right),
                "{target:?} {left} {right}"
            );
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
                immediate: IntegerValue::Unsigned(9),
            },
            row,
            &[LITA],
        );
        compare.provenance.operations = vec![OperationId::new(7).unwrap()];
        function.blocks[0].instructions[2] = compare;
    });
    // LITA is materialized 3; `3 - 9` is nonzero, so the nonzero arm wins.
    let result = fold(&source, &environment).unwrap();
    jumped_to(&result, 0, 1);
    replay(&source, &environment, result.transformed().clone()).unwrap();
}

/// The declared `CompareI64Zero` producer pairs fold the same grammar:
/// `literal - 0` decides equality by the literal alone.
#[test]
fn zero_compare_folds() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for (literal, expected) in [(0u64, 2u32), (9, 1)] {
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
        jumped_to(&result, 0, expected);
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
        (BranchShape::NonZero, 2u32),
        (BranchShape::U64LessThan, 2),
        (BranchShape::I64LessThan, 2),
    ] {
        let source = mutated(target, |function, environment| {
            let copy = environment
                .constraint(environment.selected_keys().copy_i64)
                .unwrap();
            function.blocks[0].instructions[0] = instruction(
                MATERIALIZE_A,
                SelectedInstructionKind::CopyI64,
                copy,
                &[PARAM, LITA],
            );
            function.blocks[0].instructions[2].operands[0].virtual_register = LITA;
            function.blocks[0].instructions[2].operands[1].virtual_register = LITA;
            let branch_instruction = match &function.blocks[0].terminator {
                SelectedTerminator::ConditionalBranch { instruction, .. } => instruction.clone(),
                _ => unreachable!(),
            };
            function.blocks[0].terminator = branch_terminator(
                shape,
                SelectedInstruction {
                    kind: match shape {
                        BranchShape::NonZero => SelectedInstructionKind::ConditionalBranchNonZero,
                        BranchShape::U64LessThan => {
                            SelectedInstructionKind::ConditionalBranchU64LessThan
                        }
                        BranchShape::I64LessThan => {
                            SelectedInstructionKind::ConditionalBranchI64LessThan
                        }
                    },
                    ..branch_instruction
                },
                arm(SelectedBlockId(1), 2, 2),
                arm(SelectedBlockId(2), 3, 3),
            );
        });
        // `r - r` is zero: nonzero refuses, both less-thans refuse.
        let result = fold(&source, &environment).unwrap();
        jumped_to(&result, 0, expected);
    }
}

/// A flag unit reaching in through a predecessor edge resolves to the
/// compare when every path to the branch last observed the same
/// condition-state event — here the compare lives in the entry block and
/// the branch in its jump target.
#[test]
fn cross_block_flag_reaching_folds() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        // The entry block keeps its materializations and compare, then
        // jumps to a new block carrying the branch; its two arms keep the
        // fixture's returning blocks.
        let terminator = std::mem::replace(
            &mut function.blocks[0].terminator,
            jump_terminator(environment, 6, 3, 6),
        );
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(3),
            origin: SelectedBlockOrigin::Source(BlockId::new(4).unwrap()),
            instructions: Vec::new(),
            terminator,
        });
    });
    let result = fold(&source, &environment).unwrap();
    // `3 - 5` is nonzero: the nonzero arm is decided.
    jumped_to(&result, 3, 1);
    replay(&source, &environment, result.transformed().clone()).unwrap();
}

/// A join carries the same constant flag state: both event-free
/// predecessors pass the compare's event through, so the branch on the
/// joined block resolves to the one compare and folds — while the entry
/// block's own branch keeps reading the retained definitions.
#[test]
fn cross_block_join_reaching_folds() {
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
        // The entry block branches to the fixture's arms on a fresh
        // instruction; the arms jump to a new join block carrying the
        // folded branch, which selects between two fresh returns.
        let branch_instruction = match &function.blocks[0].terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. } => instruction.clone(),
            _ => unreachable!(),
        };
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
        function.blocks[1].terminator = jump_terminator(environment, 8, 3, 7);
        function.blocks[2].terminator = jump_terminator(environment, 9, 3, 8);
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(3),
            origin: SelectedBlockOrigin::Source(BlockId::new(4).unwrap()),
            instructions: Vec::new(),
            terminator: SelectedTerminator::ConditionalBranch {
                instruction: branch_instruction,
                when_nonzero: arm(SelectedBlockId(4), 5, 9),
                when_zero: arm(SelectedBlockId(5), 6, 10),
            },
        });
        function.blocks.push(returning(4, 10, 11, &terminal_row));
        function.blocks.push(returning(5, 11, 12, &terminal_row));
    });
    let result = fold(&source, &environment).unwrap();
    // `3 - 5` is nonzero: the join's branch always takes its nonzero arm.
    jumped_to(&result, 3, 4);
    // The entry block's own conditional branch is untouched.
    assert!(matches!(
        result.transformed().functions[0].blocks[0].terminator,
        SelectedTerminator::ConditionalBranch { .. }
    ));
}

/// A loop back into the branch's own block is still the compare's flag
/// state: the self-edge contributes the block's own entry set, which the
/// fixpoint resolves to the one reaching event.
#[test]
fn cross_block_self_loop_reaching_folds() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        // The entry block jumps to a new block that carries the branch;
        // the nonzero arm loops back to the branch block itself, the zero
        // arm keeps a returning block.
        let terminator = std::mem::replace(
            &mut function.blocks[0].terminator,
            jump_terminator(environment, 6, 3, 6),
        );
        let SelectedTerminator::ConditionalBranch {
            instruction: branch_instruction,
            ..
        } = terminator
        else {
            unreachable!()
        };
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(3),
            origin: SelectedBlockOrigin::Source(BlockId::new(4).unwrap()),
            instructions: Vec::new(),
            terminator: SelectedTerminator::ConditionalBranch {
                instruction: branch_instruction,
                when_nonzero: arm(SelectedBlockId(3), 4, 2),
                when_zero: arm(SelectedBlockId(2), 3, 3),
            },
        });
    });
    let result = fold(&source, &environment).unwrap();
    // `3 - 5` is nonzero: the loop arm is decided, so the surviving jump
    // targets the branch block itself.
    jumped_to(&result, 3, 3);
}

/// A path last touched by a different flag event — here a second compare
/// on a non-literal operand behind the sibling branch — refuses: the
/// observed condition state is not provably the constant compare's.
#[test]
fn cross_block_divergent_reaching_refuses() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let branch_row = environment
            .constraint(environment.selected_keys().conditional_branch)
            .unwrap()
            .clone();
        let compare_row = environment
            .constraint(environment.selected_keys().compare_i64)
            .unwrap()
            .clone();
        let terminal_row = environment
            .constraint(environment.selected_keys().return_unit)
            .unwrap()
            .clone();
        let branch_instruction = match &function.blocks[0].terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. } => instruction.clone(),
            _ => unreachable!(),
        };
        // The entry block keeps a fresh conditional branch to the two
        // intermediate blocks; one jumps to the folded branch's join, the
        // other republishes the flag units from a different instruction
        // first.
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
        function.blocks[1].terminator = jump_terminator(environment, 8, 3, 7);
        function.blocks[2].instructions.push(instruction(
            EXTRA,
            SelectedInstructionKind::CompareI64,
            &compare_row,
            &[LITA, PARAM],
        ));
        function.blocks[2].terminator = jump_terminator(environment, 9, 3, 8);
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(3),
            origin: SelectedBlockOrigin::Source(BlockId::new(4).unwrap()),
            instructions: Vec::new(),
            terminator: SelectedTerminator::ConditionalBranch {
                instruction: branch_instruction,
                when_nonzero: arm(SelectedBlockId(4), 5, 9),
                when_zero: arm(SelectedBlockId(5), 6, 10),
            },
        });
        function.blocks.push(returning(4, 10, 11, &terminal_row));
        function.blocks.push(returning(5, 11, 12, &terminal_row));
    });
    assert_eq!(
        fold(&source, &environment).unwrap_err(),
        TerminatorPairError::UnsupportedUse
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
        // branch anywhere.
        function.blocks[0].instructions.remove(2);
    });
    assert_eq!(
        fold(&source, &environment).unwrap_err(),
        TerminatorPairError::UnsupportedUse
    );
}

/// A terminator-carried clobber ends the reach like any other event: the
/// predecessor's last event for the flag unit is the jump instruction's
/// own wipe, not the compare, so the branch refuses.
#[test]
fn cross_block_terminator_event_refuses() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let flags = match &function.blocks[0].terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. } => instruction
                .implicit_uses
                .iter()
                .copied()
                .filter(|unit| {
                    environment
                        .constraint(environment.selected_keys().compare_i64)
                        .unwrap()
                        .implicit_defs
                        .contains(unit)
                })
                .collect::<Vec<_>>(),
            _ => unreachable!(),
        };
        let mut jump = jump_terminator(environment, 6, 3, 6);
        let SelectedTerminator::Jump { instruction, .. } = &mut jump else {
            unreachable!()
        };
        instruction.clobbers = flags;
        let terminator = std::mem::replace(&mut function.blocks[0].terminator, jump);
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(3),
            origin: SelectedBlockOrigin::Source(BlockId::new(4).unwrap()),
            instructions: Vec::new(),
            terminator,
        });
    });
    assert_eq!(
        fold(&source, &environment).unwrap_err(),
        TerminatorPairError::UnsupportedUse
    );
}

/// An intervening flag event — a second compare or a clobber in the
/// branch's own block — makes the observed state the later instruction's,
/// not the constant compare's.
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
        function.blocks[0].instructions.push(instruction(
            EXTRA,
            SelectedInstructionKind::CompareI64,
            compare,
            &[LITA, PARAM],
        ));
    });
    assert_eq!(
        fold(&second_compare, &environment).unwrap_err(),
        TerminatorPairError::UndecidedOperands
    );
    // A clobber of the flag unit between compare and branch leaves the
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
        function.blocks[0].instructions.push(wiper);
    });
    assert_eq!(
        fold(&clobbered, &environment).unwrap_err(),
        TerminatorPairError::UnsupportedUse
    );
}

/// A used unit outside the flag universe must lie in the jump row's
/// implicit surface: a foreign non-flag observation the jump cannot carry
/// refuses under the declared unit-flow axis.
#[test]
fn non_flag_use_outside_jump_surface_refuses() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let foreign = mutated(target, |function, _| {
        // Unit 998 is neither a compare-published flag unit nor in the
        // jump row's program-counter-only surface.
        let spare = register_model::RegisterUnitId(998);
        let instruction = match &mut function.blocks[0].terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. } => instruction,
            _ => unreachable!(),
        };
        instruction.implicit_uses.push(spare);
    });
    assert_eq!(
        fold(&foreign, &environment).unwrap_err(),
        TerminatorPairError::UnsupportedUse
    );
}

/// A branch whose implicit definitions or clobbers the jump row cannot
/// republish refuses: the rebuilt terminator would change the unit events
/// downstream readers observe at this site.
#[test]
fn unpublishable_unit_traffic_refuses() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A second defined unit is not republished by the jump row.
    let extra_def = mutated(target, |function, _| {
        let instruction = match &mut function.blocks[0].terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. } => instruction,
            _ => unreachable!(),
        };
        instruction
            .implicit_defs
            .push(register_model::RegisterUnitId(997));
    });
    assert_eq!(
        fold(&extra_def, &environment).unwrap_err(),
        TerminatorPairError::UnsupportedUse
    );
    // A clobber the jump row lacks would lose the event entirely.
    let clobbering = mutated(target, |function, _| {
        let instruction = match &mut function.blocks[0].terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. } => instruction,
            _ => unreachable!(),
        };
        instruction.clobbers = vec![register_model::RegisterUnitId(996)];
    });
    assert_eq!(
        fold(&clobbering, &environment).unwrap_err(),
        TerminatorPairError::UnsupportedUse
    );
}

/// Only the declared conditional-branch pairs fold; anything else — a
/// non-terminator id, a flag-free or operandful record, a variant/kind
/// mismatch, or a non-branch terminator — refuses.
#[test]
fn non_branch_terminators_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(
        target,
        IntegerValue::Unsigned(3),
        IntegerValue::Unsigned(5),
        BranchShape::NonZero,
    );
    let effect_catalog = catalog(&source, &environment);
    // The compare itself is not a terminator-carried branch.
    assert_eq!(
        fold_selected_terminator_pair(&source, 0, COMPARE, &environment, &effect_catalog, budget())
            .unwrap_err(),
        TerminatorPairError::UnsupportedTerminator
    );
    // Neither is a body materialization.
    assert_eq!(
        fold_selected_terminator_pair(
            &source,
            0,
            MATERIALIZE_A,
            &environment,
            &effect_catalog,
            budget()
        )
        .unwrap_err(),
        TerminatorPairError::UnsupportedTerminator
    );
    // An absent instruction id cannot be located.
    assert_eq!(
        fold_selected_terminator_pair(
            &source,
            0,
            SelectedInstructionId(99),
            &environment,
            &effect_catalog,
            budget()
        )
        .unwrap_err(),
        TerminatorPairError::UnsupportedTerminator
    );
    // A flag-free branch shape has no condition to evaluate.
    let flag_free = mutated(target, |function, _| {
        let instruction = match &mut function.blocks[0].terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. } => instruction,
            _ => unreachable!(),
        };
        instruction.implicit_uses.clear();
    });
    assert_eq!(
        fold(&flag_free, &environment).unwrap_err(),
        TerminatorPairError::UnsupportedUse
    );
    // A use roster of only non-flag units — the program-counter read alone
    // — still has no condition to evaluate.
    let non_flag_only = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        let instruction = match &mut function.blocks[0].terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. } => instruction,
            _ => unreachable!(),
        };
        instruction.implicit_uses = jump_row.implicit_uses.clone();
    });
    assert_eq!(
        fold(&non_flag_only, &environment).unwrap_err(),
        TerminatorPairError::UnsupportedUse
    );
    // An explicit operand is not the declared branch shape.
    let operandful = mutated(target, |function, _| {
        let class = function.blocks[0].instructions[2].operands[0].class;
        let instruction = match &mut function.blocks[0].terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. } => instruction,
            _ => unreachable!(),
        };
        instruction.operands.push(SelectedOperand {
            operand: 0,
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
        TerminatorPairError::UnsupportedTerminator
    );
    // A variant/kind pairing no declared rule emits refuses.
    let mismatched = mutated(target, |function, _| {
        if let SelectedTerminator::ConditionalBranch { instruction, .. } =
            &mut function.blocks[0].terminator
        {
            instruction.kind = SelectedInstructionKind::ConditionalBranchU64LessThan;
        }
    });
    assert_eq!(
        fold(&mismatched, &environment).unwrap_err(),
        TerminatorPairError::UnsupportedTerminator
    );
    // A return terminator is not a branch.
    let returning_source = mutated(target, |function, environment| {
        let terminal_row = environment
            .constraint(environment.selected_keys().return_unit)
            .unwrap()
            .clone();
        function.blocks[0].terminator = SelectedTerminator::Return {
            instruction: instruction(
                BRANCH,
                SelectedInstructionKind::ReturnUnit,
                &terminal_row,
                &[],
            ),
            psi_return_edge: EdgeId::new(9).unwrap(),
        };
    });
    assert_eq!(
        fold(&returning_source, &environment).unwrap_err(),
        TerminatorPairError::UnsupportedTerminator
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
        TerminatorPairError::UndecidedOperands
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
        TerminatorPairError::UndecidedOperands
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
        } = &mut function.blocks[1].terminator
        else {
            unreachable!()
        };
        *slot = carried;
    });
    assert_eq!(
        fold(&second_definition, &environment).unwrap_err(),
        TerminatorPairError::UndecidedOperands
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
            BranchShape::NonZero,
        );
        assert_eq!(
            fold(&source, &environment).unwrap_err(),
            TerminatorPairError::UndecidedOperands,
            "{value:?}"
        );
    }
}

/// The bound catalog must describe the plan's target and the environment's
/// constraint inventory: a catalog built for another target — or a record
/// whose constraint key no declaration binds to its semantic — cannot
/// attest the control-flow relationship.
#[test]
fn effect_surface_mismatches_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(
        target,
        IntegerValue::Unsigned(3),
        IntegerValue::Unsigned(5),
        BranchShape::NonZero,
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
        fold_selected_terminator_pair(&source, 0, BRANCH, &environment, &foreign_catalog, budget())
            .unwrap_err(),
        TerminatorPairError::EffectSurfaceMismatch
    );
    // A record bound to the jump row's constraint key has no catalog
    // declaration pairing it with the conditional-branch semantic.
    let wrong_row = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        let instruction = match &mut function.blocks[0].terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. } => instruction,
            _ => unreachable!(),
        };
        instruction.constraint = jump_row.key;
    });
    assert_eq!(
        fold(&wrong_row, &environment).unwrap_err(),
        TerminatorPairError::EffectSurfaceMismatch
    );
    // The same defects refuse on the replay path: the proposed jump cannot
    // carry the fold the bound catalog never declared.
    let result = fold(&source, &environment).unwrap();
    assert_eq!(
        validate_terminator_pair_fold(
            &source,
            0,
            BRANCH,
            &environment,
            &foreign_catalog,
            budget(),
            result.transformed().clone()
        )
        .unwrap_err(),
        TerminatorPairError::EffectSurfaceMismatch
    );
    let mut detached = result.transformed().clone();
    detached.functions = detached.functions.iter().cloned().collect();
    let effect_catalog = catalog(&wrong_row, &environment);
    assert_eq!(
        validate_terminator_pair_fold(
            &wrong_row,
            0,
            BRANCH,
            &environment,
            &effect_catalog,
            budget(),
            detached
        )
        .unwrap_err(),
        TerminatorPairError::EffectSurfaceMismatch
    );
}

/// A boolean materialization on the decided arm and the branch fold
/// compose: each consumes the same published compare independently, and
/// the terminator pair accepts the prior rewrite's validated output.
#[test]
fn validated_results_compose_with_constant_boolean() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let boolean = environment
            .constraint(environment.selected_keys().materialize_boolean)
            .unwrap()
            .clone();
        let mut materialization = instruction(
            EXTRA,
            SelectedInstructionKind::MaterializeBooleanEqual,
            &boolean,
            &[OUTPUT],
        );
        materialization.provenance.values = vec![ValueId::new(5).unwrap()];
        function.blocks[1].instructions.push(materialization);
    });
    // The boolean on the decided (nonzero) arm folds first; the branch
    // fold then consumes the same compare from the transformed plan.
    let boolean_fold =
        crate::fold_selected_constant_boolean(&source, 0, EXTRA, &environment, budget()).unwrap();
    let effect_catalog = catalog(&source, &environment);
    let result = fold_selected_terminator_pair(
        &boolean_fold,
        0,
        BRANCH,
        &environment,
        &effect_catalog,
        budget(),
    )
    .unwrap();
    jumped_to(&result, 0, 1);
    assert_eq!(
        result.receipt().source_selected(),
        boolean_fold.receipt().transformed_selected()
    );
}

/// A proposal that does not match the reconstructed `Jump` — or that
/// retains the source terminator — fails the independent replay.
#[test]
fn replay_mismatches_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(
        target,
        IntegerValue::Unsigned(3),
        IntegerValue::Unsigned(5),
        BranchShape::NonZero,
    );
    // The unchanged source is not a rewritten proposal.
    assert_eq!(
        replay(&source, &environment, source.transformed().clone()).unwrap_err(),
        TerminatorPairError::ReplayMismatch
    );
    // The wrong surviving successor is not the decided record.
    let mut wrong = source.transformed().clone();
    let jump_row = environment
        .constraint(keys(&environment).jump)
        .unwrap()
        .clone();
    let branch_instruction = match &source.transformed().functions[0].blocks[0].terminator {
        SelectedTerminator::ConditionalBranch { instruction, .. } => instruction.clone(),
        _ => unreachable!(),
    };
    wrong.functions[0].blocks[0].terminator = SelectedTerminator::Jump {
        instruction: SelectedInstruction {
            kind: SelectedInstructionKind::Jump,
            constraint: jump_row.key,
            operands: Vec::new(),
            implicit_uses: jump_row.implicit_uses.clone(),
            implicit_defs: jump_row.implicit_defs.clone(),
            clobbers: jump_row.clobbers.clone(),
            ..branch_instruction
        },
        successor: arm(SelectedBlockId(1), 2, 2),
    };
    assert_eq!(
        replay(&source, &environment, wrong).unwrap_err(),
        TerminatorPairError::ReplayMismatch
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
        BranchShape::NonZero,
    );
    let effect_catalog = catalog(&source, &environment);
    // A function index outside the plan.
    assert_eq!(
        fold_selected_terminator_pair(&source, 1, BRANCH, &environment, &effect_catalog, budget())
            .unwrap_err(),
        TerminatorPairError::SourceMismatch
    );
    // An environment for a different target than the plan's.
    let wrong_environment =
        baseline_target_register_environment(NativeTarget::macos_arm64()).unwrap();
    assert_eq!(
        fold_selected_terminator_pair(
            &source,
            0,
            BRANCH,
            &wrong_environment,
            &effect_catalog,
            budget()
        )
        .unwrap_err(),
        TerminatorPairError::SourceMismatch
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
        BranchShape::NonZero,
    );
    let tiny = OptimizationWorkBudget::new(100, 100, 4, 100, 100).unwrap();
    let effect_catalog = catalog(&source, &environment);
    assert_eq!(
        fold_selected_terminator_pair(&source, 0, BRANCH, &environment, &effect_catalog, tiny)
            .unwrap_err(),
        TerminatorPairError::WorkBudgetExceeded
    );
}

/// The measured validation-step boundary: admission charges one step per
/// block plus one per instruction across the plan, then the admitted
/// function's scan twice for the producer lookups, then the flag walk —
/// the adjacency and partition-surface setup plus, per used unit, the
/// block-prefix and last-event scans and the bounded entry-set
/// propagation — seven hundred seventy-seven for the six-element fixture
/// on x86-64, nine hundred eighty once a seventh element trails the
/// compare — so the exact count admits the fold on both the proposal and
/// the independent replay path while one step below rejects both.
#[test]
fn measured_validation_step_boundary_admits_and_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A seventh element extends the plan-wide and producer scans and every
    // term the flag walk derives from the function scan.
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
                BranchShape::NonZero,
            ),
            777u64,
        ),
        (wider, 980u64),
    ] {
        let effect_catalog = catalog(&source, &environment);
        let exact = OptimizationWorkBudget::new(1, 1, exact_steps, 1, 1).unwrap();
        let result =
            fold_selected_terminator_pair(&source, 0, BRANCH, &environment, &effect_catalog, exact)
                .unwrap();
        validate_terminator_pair_fold(
            &source,
            0,
            BRANCH,
            &environment,
            &effect_catalog,
            exact,
            result.transformed().clone(),
        )
        .unwrap();
        let starved = OptimizationWorkBudget::new(1, 1, exact_steps - 1, 1, 1).unwrap();
        assert_eq!(
            fold_selected_terminator_pair(
                &source,
                0,
                BRANCH,
                &environment,
                &effect_catalog,
                starved
            )
            .unwrap_err(),
            TerminatorPairError::WorkBudgetExceeded
        );
        assert_eq!(
            validate_terminator_pair_fold(
                &source,
                0,
                BRANCH,
                &environment,
                &effect_catalog,
                starved,
                result.transformed().clone(),
            )
            .unwrap_err(),
            TerminatorPairError::WorkBudgetExceeded
        );
    }
}

/// Two runs over the identical source produce the identical validated
/// result, and the published plan is a legal second input: re-running at
/// the same site is terminal because the terminator is now a jump, not a
/// flag-reading branch.
#[test]
fn fold_is_deterministic_and_terminal() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let first = fold(
        &fixture(
            target,
            IntegerValue::Unsigned(3),
            IntegerValue::Unsigned(5),
            BranchShape::NonZero,
        ),
        &environment,
    )
    .unwrap();
    let second = fold(
        &fixture(
            target,
            IntegerValue::Unsigned(3),
            IntegerValue::Unsigned(5),
            BranchShape::NonZero,
        ),
        &environment,
    )
    .unwrap();
    assert_eq!(first, second);
    let effect_catalog = catalog(&first, &environment);
    assert_eq!(
        fold_selected_terminator_pair(&first, 0, BRANCH, &environment, &effect_catalog, budget())
            .unwrap_err(),
        TerminatorPairError::UnsupportedTerminator
    );
}

/// Replay rejects any drift from the reconstructed `Jump`, in the
/// rewritten terminator or anywhere else in the plan.
#[test]
fn replay_rejects_anything_but_the_exact_form() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(
        target,
        IntegerValue::Unsigned(3),
        IntegerValue::Unsigned(5),
        BranchShape::NonZero,
    );
    let result = fold(&source, &environment).unwrap();
    for mutation in 0..10 {
        let mut proposed = result.transformed().clone();
        let function = &mut proposed.functions[0];
        match mutation {
            // The wrong successor survived.
            0 => {
                let SelectedTerminator::Jump { successor, .. } = &mut function.blocks[0].terminator
                else {
                    unreachable!()
                };
                *successor = arm(SelectedBlockId(2), 3, 3);
            }
            // The decided record's bindings must survive verbatim.
            1 => {
                let SelectedTerminator::Jump { successor, .. } = &mut function.blocks[0].terminator
                else {
                    unreachable!()
                };
                successor.bindings.clear();
            }
            // The branch form must not survive in the terminator.
            2 => {
                function.blocks[0].terminator = source.transformed().functions[0].blocks[0]
                    .terminator
                    .clone();
            }
            // A different instruction kind on the terminator.
            3 => {
                let SelectedTerminator::Jump { instruction, .. } =
                    &mut function.blocks[0].terminator
                else {
                    unreachable!()
                };
                instruction.kind = SelectedInstructionKind::ConditionalBranchNonZero;
            }
            // A different instruction id on the terminator.
            4 => {
                let SelectedTerminator::Jump { instruction, .. } =
                    &mut function.blocks[0].terminator
                else {
                    unreachable!()
                };
                instruction.id = COMPARE;
            }
            // The branch's provenance must survive intact.
            5 => {
                let SelectedTerminator::Jump { instruction, .. } =
                    &mut function.blocks[0].terminator
                else {
                    unreachable!()
                };
                instruction.provenance.edges.push(EdgeId::new(9).unwrap());
            }
            // The retained compare must remain untouched.
            6 => {
                function.blocks[0].instructions[2].kind = SelectedInstructionKind::CompareI64Zero;
            }
            // A phantom flag use must not linger on the jump.
            7 => {
                let SelectedTerminator::Jump { instruction, .. } =
                    &mut function.blocks[0].terminator
                else {
                    unreachable!()
                };
                instruction
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
                        instruction: BRANCH,
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
            replay(&source, &environment, proposed).is_err(),
            "mutation {mutation}"
        );
    }
}

/// Replay corruption in a block the fold never touched still rejects: the
/// restore-by-content check compares the complete plan, not just the block
/// carrying the rewritten terminator.
#[test]
fn replay_rejects_drift_outside_the_rewritten_block() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(
        target,
        IntegerValue::Unsigned(3),
        IntegerValue::Unsigned(5),
        BranchShape::NonZero,
    );
    let result = fold(&source, &environment).unwrap();
    // An extra instruction in the untouched arm block rejects.
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
            &[PARAM, OUTPUT],
        ));
    assert_eq!(
        replay(&source, &environment, proposed).unwrap_err(),
        TerminatorPairError::ReplayMismatch
    );
    // A missing arm block likewise rejects.
    let mut proposed = result.transformed().clone();
    proposed.functions[0].blocks.pop();
    assert_eq!(
        replay(&source, &environment, proposed).unwrap_err(),
        TerminatorPairError::ReplayMismatch
    );
}
