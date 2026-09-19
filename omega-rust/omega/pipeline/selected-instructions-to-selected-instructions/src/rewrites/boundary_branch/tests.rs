use crate::BoundaryBranchError;
use crate::BoundaryBranchReceipt;
use crate::ValidatedBoundaryBranch;
use crate::ValidatedSelectedAnalysis;
use crate::fold_selected_boundary_branch;
use crate::validate_boundary_branch_fold;
use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget};
use optimization_unit::{FuelSettlement, PsiProvenance, ValueDefinitionSite};
use register_environment::baseline_target_register_environment;
use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan, SelectedOperand,
    SelectedSuccessor, SelectedSuccessorRole, SelectedTerminator, SelectedValueBinding,
    SelectedValueTransport, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
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
const PARAM2: VirtualRegisterId = VirtualRegisterId(4);

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

/// A raw selected-stage unit fixture, not a source/Terminal admission
/// claim: `ra = materialize va; rb = materialize vb; compare lo, ro;` then
/// a conditional branch on the published flags to two returning blocks.
/// `None` for either side reads an entry parameter instead — the pole
/// audit leaves that side open. The `when_less`/`when_nonzero` arm carries
/// a register binding and a fuel settlement so the fold's verbatim
/// successor moves are observable. The materializations stay in place even
/// when the compare reads a parameter: they keep their producing
/// instruction count stable across variants.
fn fixture(
    target: NativeTarget,
    left: Option<u64>,
    right: Option<u64>,
    shape: BranchShape,
) -> ValidatedBoundaryBranch {
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let materialize = environment.constraint(keys.materialize_i64).unwrap();
    let compare = environment.constraint(keys.compare_i64).unwrap();
    let branch_row = environment.constraint(keys.conditional_branch).unwrap();
    let terminal_row = environment.constraint(keys.return_unit).unwrap();
    let class = compare.operands[0].class;
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let entry_parameter =
        |register_id: VirtualRegisterId, source: u64, index: usize| VirtualRegister {
            id: register_id,
            scalar_type,
            class,
            origin: VirtualRegisterOrigin::EntryParameter {
                source_value: ValueId::new(source).unwrap(),
                parameter_index: index,
            },
            definition_site: Some(ValueDefinitionSite::FunctionParameter(index as u32)),
            entry_fixed_view: None,
        };
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
        entry_parameter(PARAM, 4, 0),
        register(
            OUTPUT,
            scalar_type,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: EXTRA,
                source_value: ValueId::new(6).unwrap(),
            },
        ),
        entry_parameter(PARAM2, 9, 1),
    ];
    let mut compare_instruction = instruction(
        COMPARE,
        SelectedInstructionKind::CompareI64,
        compare,
        &[
            left.map(|_| LITA).unwrap_or(PARAM),
            right.map(|_| LITB).unwrap_or(PARAM),
        ],
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
            normalized_foreign_calls: Vec::new(),
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
                            SelectedInstructionKind::MaterializeI64 {
                                value: IntegerValue::Unsigned(u128::from(left.unwrap_or(9))),
                            },
                            materialize,
                            &[LITA],
                        ),
                        instruction(
                            MATERIALIZE_B,
                            SelectedInstructionKind::MaterializeI64 {
                                value: IntegerValue::Unsigned(u128::from(right.unwrap_or(9))),
                            },
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
    ValidatedBoundaryBranch {
        receipt: BoundaryBranchReceipt {
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

/// Edit the single fixture function, then refresh the receipt identities so
/// the mutated plan is a well-formed analysis source. The base is the
/// never-taken `x <u 0` fold: an unsigned strict less-than on an unknown
/// register against the domain minimum.
fn mutated(
    target: NativeTarget,
    edit: impl FnOnce(&mut SelectedFunction, &register_environment::ValidatedTargetRegisterEnvironment),
) -> ValidatedBoundaryBranch {
    let environment = baseline_target_register_environment(target).unwrap();
    let mut source = fixture(target, None, Some(0), BranchShape::U64LessThan);
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
    source: &ValidatedBoundaryBranch,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> Result<ValidatedBoundaryBranch, BoundaryBranchError> {
    fold_selected_boundary_branch(source, 0, BRANCH, environment, budget())
}

/// A jump terminator on `block_index` targeting `arm`, rebuilt from the
/// fixture's own jump row.
fn jump_terminator(
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
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

/// The terminator a decided fold publishes: `Jump` on the branch's own
/// instruction identity to `block`.
fn jumped_to(
    result: &ValidatedBoundaryBranch,
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

/// The terminator a near-pole collapse publishes: `ConditionalBranch`
/// carrying `ConditionalBranchNonZero` on the branch's own instruction,
/// with the source `when_less`/`when_not_less` records republished as
/// `when_nonzero`/`when_zero`.
fn collapsed_to_nonzero<'result>(
    result: &'result ValidatedBoundaryBranch,
    source: &ValidatedBoundaryBranch,
    block_index: usize,
) -> &'result SelectedTerminator {
    let terminator = &result.transformed().functions[0].blocks[block_index].terminator;
    let SelectedTerminator::ConditionalBranch {
        instruction,
        when_nonzero,
        when_zero,
    } = terminator
    else {
        panic!("expected a ConditionalBranch terminator, found {terminator:?}");
    };
    let source_terminator = &source.transformed().functions[0].blocks[block_index].terminator;
    let (source_instruction, when_less, when_not_less) = match source_terminator {
        SelectedTerminator::ConditionalBranchU64LessThan {
            instruction,
            when_less,
            when_not_less,
        }
        | SelectedTerminator::ConditionalBranchI64LessThan {
            instruction,
            when_less,
            when_not_less,
        } => (instruction, when_less, when_not_less),
        _ => unreachable!(),
    };
    // The collapse keeps the branch's own row and whole implicit surface:
    // only the kind field changes.
    assert_eq!(instruction.id, BRANCH);
    assert_eq!(
        instruction.kind,
        SelectedInstructionKind::ConditionalBranchNonZero
    );
    assert_eq!(instruction.constraint, source_instruction.constraint);
    assert_eq!(instruction.operands, source_instruction.operands);
    assert_eq!(instruction.implicit_uses, source_instruction.implicit_uses);
    assert_eq!(instruction.implicit_defs, source_instruction.implicit_defs);
    assert_eq!(instruction.clobbers, source_instruction.clobbers);
    assert_eq!(instruction.provenance, source_instruction.provenance);
    // The arms move verbatim: the ordering's taken leg becomes the nonzero
    // leg — `0 <u x`, `x <u MAX`, `MIN <s x`, `x <s MAX` each hold exactly
    // when the difference is nonzero.
    assert_eq!(when_nonzero, when_less);
    assert_eq!(when_zero, when_not_less);
    terminator
}

/// Far poles and pairs of known operands fold the strict-ordering branch
/// to a `Jump` on every target, moving the decided successor record —
/// bindings and fuel included — verbatim while the untaken edge leaves the
/// plan.
#[test]
fn decided_poles_fold_on_all_targets() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        // `(shape, left, right, decided block)`: `None` reads the entry
        // parameter, leaving the pole audit's side open. The unsigned
        // domain's poles are 0 and u64::MAX; the signed domain's are
        // i64::MIN and i64::MAX in their bit patterns.
        let cases: &[(BranchShape, Option<u64>, Option<u64>, u32)] = &[
            // `x <u 0` can never hold.
            (BranchShape::U64LessThan, None, Some(0), 2),
            // `u64::MAX <u x` can never hold.
            (BranchShape::U64LessThan, Some(u64::MAX), None, 2),
            // `x <s i64::MIN` can never hold.
            (BranchShape::I64LessThan, None, Some(i64::MIN as u64), 2),
            // `i64::MAX <s x` can never hold.
            (BranchShape::I64LessThan, Some(i64::MAX as u64), None, 2),
            // Two known operands decide outright — the constant family's
            // same table.
            (BranchShape::U64LessThan, Some(3), Some(5), 1),
            (BranchShape::U64LessThan, Some(9), Some(5), 2),
            (BranchShape::I64LessThan, Some((-3i64) as u64), Some(5), 1),
            (BranchShape::I64LessThan, Some(7), Some(i64::MIN as u64), 2),
            // Two known poles still decide outright, not collapse.
            (BranchShape::U64LessThan, Some(0), Some(0), 2),
            (BranchShape::U64LessThan, Some(0), Some(u64::MAX), 1),
        ];
        for (shape, left, right, expected) in cases {
            let source = fixture(target, *left, *right, *shape);
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
            // decided flag uses are dropped, the program-counter traffic
            // stays.
            let jump_row = environment.constraint(keys(&environment).jump).unwrap();
            assert_eq!(jumped_instruction.implicit_uses, jump_row.implicit_uses);
            assert_eq!(jumped_instruction.implicit_defs, jump_row.implicit_defs);
            let source_terminator = &source.transformed().functions[0].blocks[0].terminator;
            let (branch_instruction, when_less, when_not_less) = match source_terminator {
                SelectedTerminator::ConditionalBranchU64LessThan {
                    instruction,
                    when_less,
                    when_not_less,
                }
                | SelectedTerminator::ConditionalBranchI64LessThan {
                    instruction,
                    when_less,
                    when_not_less,
                } => (instruction, when_less, when_not_less),
                _ => unreachable!(),
            };
            assert_eq!(jumped_instruction.provenance, branch_instruction.provenance);
            // The decided record moved verbatim — binding and fuel
            // settlement included. Expected block 1 is the `when_less`
            // arm, block 2 the `when_not_less` arm.
            let expected_arm = if *expected == 1 {
                when_less
            } else {
                when_not_less
            };
            assert_eq!(jumped_arm, expected_arm, "{target:?} {left:?} {right:?}");
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
            validate_boundary_branch_fold(
                &source,
                0,
                BRANCH,
                &environment,
                budget(),
                result.transformed().clone(),
            )
            .unwrap();
            // A detached, separately allocated proposal replays by content.
            let mut detached = result.transformed().clone();
            detached.functions = detached.functions.iter().cloned().collect();
            validate_boundary_branch_fold(&source, 0, BRANCH, &environment, budget(), detached)
                .unwrap();
        }
    }
}

/// Near poles collapse the ordering to the nonzero condition on the
/// identical published flag state: `0 <u x` is `x != 0`, `x <u u64::MAX`
/// is `x != u64::MAX`, `i64::MIN <s x` is `x != i64::MIN`, and `x <
/// i64::MAX` is `x != i64::MAX` — so `when_less` republishes as
/// `when_nonzero`.
#[test]
fn near_poles_collapse_to_nonzero_on_all_targets() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let cases: &[(BranchShape, Option<u64>, Option<u64>)] = &[
            (BranchShape::U64LessThan, Some(0), None),
            (BranchShape::U64LessThan, None, Some(u64::MAX)),
            (BranchShape::I64LessThan, Some(i64::MIN as u64), None),
            (BranchShape::I64LessThan, None, Some(i64::MAX as u64)),
        ];
        for (shape, left, right) in cases {
            let source = fixture(target, *left, *right, *shape);
            let result = fold(&source, &environment).unwrap();
            collapsed_to_nonzero(&result, &source, 0);
            // The compare keeps publishing flag state for other readers.
            assert_eq!(
                result.transformed().functions[0].blocks[0].instructions[2].kind,
                SelectedInstructionKind::CompareI64
            );
            validate_boundary_branch_fold(
                &source,
                0,
                BRANCH,
                &environment,
                budget(),
                result.transformed().clone(),
            )
            .unwrap();
        }
    }
}

/// The immediate and zero compare forms supply their own pole: an
/// immediate bound or the implicit zero decides — or collapses — without
/// any literal producer on the other side.
#[test]
fn immediate_and_zero_compares_decide() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // `x <u 0` through the immediate form is never taken — no producer on
    // the register side is needed at all.
    let never = mutated(target, |function, environment| {
        let row = environment
            .constraint(environment.selected_keys().compare_i64_immediate)
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            COMPARE,
            SelectedInstructionKind::CompareI64Immediate {
                immediate: IntegerValue::Unsigned(0),
            },
            row,
            &[PARAM],
        );
    });
    let result = fold(&never, &environment).unwrap();
    jumped_to(&result, 0, 2);
    // `x <u u64::MAX` through the immediate form is `x != u64::MAX`.
    let collapse = mutated(target, |function, environment| {
        let row = environment
            .constraint(environment.selected_keys().compare_i64_immediate)
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            COMPARE,
            SelectedInstructionKind::CompareI64Immediate {
                immediate: IntegerValue::Unsigned(u128::from(u64::MAX)),
            },
            row,
            &[PARAM],
        );
    });
    let result = fold(&collapse, &environment).unwrap();
    collapsed_to_nonzero(&result, &collapse, 0);
    // The zero compare reads as `x - 0`: `x <u 0` never holds.
    let zero = mutated(target, |function, environment| {
        let row = environment
            .constraint(environment.selected_keys().compare_i64_zero)
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            COMPARE,
            SelectedInstructionKind::CompareI64Zero,
            row,
            &[PARAM],
        );
    });
    let result = fold(&zero, &environment).unwrap();
    jumped_to(&result, 0, 2);
    // The same zero compare under the signed ordering is `x <s 0` — a
    // sign test, not a pole — and refuses.
    let signed_zero = mutated(target, |function, environment| {
        let row = environment
            .constraint(environment.selected_keys().compare_i64_zero)
            .unwrap()
            .clone();
        function.blocks[0].instructions[2] = instruction(
            COMPARE,
            SelectedInstructionKind::CompareI64Zero,
            &row,
            &[PARAM],
        );
        let branch_instruction = match &function.blocks[0].terminator {
            SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. } => {
                instruction.clone()
            }
            _ => unreachable!(),
        };
        function.blocks[0].terminator = SelectedTerminator::ConditionalBranchI64LessThan {
            instruction: SelectedInstruction {
                kind: SelectedInstructionKind::ConditionalBranchI64LessThan,
                ..branch_instruction
            },
            when_less: arm(SelectedBlockId(1), 2, 2),
            when_not_less: arm(SelectedBlockId(2), 3, 3),
        };
    });
    assert_eq!(
        fold(&signed_zero, &environment).unwrap_err(),
        BoundaryBranchError::UnsupportedLiteral
    );
}

/// A non-pole operand on either side, two unknown sides, and the identity
/// `register - register` case all leave the predicate undecidable.
#[test]
fn undecided_predicates_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for (shape, left, right) in [
        // `5 <u x` and `x <u 5` are genuine orderings, not poles.
        (BranchShape::U64LessThan, Some(5), None),
        (BranchShape::U64LessThan, None, Some(5)),
        // `7 <s x` and `x <s 7` likewise.
        (BranchShape::I64LessThan, Some(7), None),
        (BranchShape::I64LessThan, None, Some(7)),
        // Two unknown sides.
        (BranchShape::U64LessThan, None, None),
        (BranchShape::I64LessThan, None, None),
    ] {
        let source = fixture(target, left, right, shape);
        assert_eq!(
            fold(&source, &environment).unwrap_err(),
            BoundaryBranchError::UnsupportedLiteral,
            "{left:?} {right:?}"
        );
    }
    // `r - r` is zero on every lane — the flag state is constant whatever
    // the register holds — but that is the identity case the constant
    // family owns, not a pole decision.
    let identity = mutated(target, |function, _| {
        function.blocks[0].instructions[2].operands[0].virtual_register = LITA;
        function.blocks[0].instructions[2].operands[1].virtual_register = LITA;
    });
    assert_eq!(
        fold(&identity, &environment).unwrap_err(),
        BoundaryBranchError::UnsupportedLiteral
    );
    // Two distinct unknown registers decide nothing either.
    let two_unknown = mutated(target, |function, _| {
        function.blocks[0].instructions[2].operands[0].virtual_register = PARAM;
        function.blocks[0].instructions[2].operands[1].virtual_register = PARAM2;
    });
    assert_eq!(
        fold(&two_unknown, &environment).unwrap_err(),
        BoundaryBranchError::UnsupportedLiteral
    );
}

/// The equality-condition terminator stays refused: `left != right`
/// varies with every unknown side, so no single pole decides it — even
/// with a pole operand present.
#[test]
fn equality_condition_branches_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for (left, right) in [(None, Some(0)), (Some(0), None), (Some(3), Some(5))] {
        let source = fixture(target, left, right, BranchShape::NonZero);
        assert_eq!(
            fold(&source, &environment).unwrap_err(),
            BoundaryBranchError::UnsupportedInstruction
        );
    }
}

/// Only the emitted variant/kind pairings fold: a mismatched pair, an
/// explicit operand, a flag-free use roster, a non-branch terminator, and
/// non-terminator members all refuse.
#[test]
fn non_branch_shapes_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target, None, Some(0), BranchShape::U64LessThan);
    // The compare itself is not a terminator-carried branch.
    assert_eq!(
        fold_selected_boundary_branch(&source, 0, COMPARE, &environment, budget()).unwrap_err(),
        BoundaryBranchError::SourceMismatch
    );
    // Neither is a body materialization.
    assert_eq!(
        fold_selected_boundary_branch(&source, 0, MATERIALIZE_A, &environment, budget())
            .unwrap_err(),
        BoundaryBranchError::SourceMismatch
    );
    // An absent instruction id cannot be located.
    assert_eq!(
        fold_selected_boundary_branch(
            &source,
            0,
            SelectedInstructionId(99),
            &environment,
            budget()
        )
        .unwrap_err(),
        BoundaryBranchError::SourceMismatch
    );
    // A variant/kind pairing selection never emits refuses.
    let mismatched = mutated(target, |function, _| {
        if let SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. } =
            &mut function.blocks[0].terminator
        {
            instruction.kind = SelectedInstructionKind::ConditionalBranchNonZero;
        }
    });
    assert_eq!(
        fold(&mismatched, &environment).unwrap_err(),
        BoundaryBranchError::UnsupportedInstruction
    );
    // An explicit operand is not the emitted branch shape.
    let operandful = mutated(target, |function, _| {
        let class = function.blocks[0].instructions[2].operands[0].class;
        let instruction = match &mut function.blocks[0].terminator {
            SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. } => instruction,
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
        BoundaryBranchError::UnsupportedInstruction
    );
    // A flag-free branch shape has no condition to evaluate.
    let flag_free = mutated(target, |function, _| {
        let instruction = match &mut function.blocks[0].terminator {
            SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. } => instruction,
            _ => unreachable!(),
        };
        instruction.implicit_uses.clear();
    });
    assert_eq!(
        fold(&flag_free, &environment).unwrap_err(),
        BoundaryBranchError::UnsupportedInstruction
    );
    // A use roster of only non-flag units — the program-counter read alone
    // — still has no condition to evaluate.
    let non_flag_only = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        let instruction = match &mut function.blocks[0].terminator {
            SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. } => instruction,
            _ => unreachable!(),
        };
        instruction.implicit_uses = jump_row.implicit_uses.clone();
    });
    assert_eq!(
        fold(&non_flag_only, &environment).unwrap_err(),
        BoundaryBranchError::UnsupportedInstruction
    );
    // A return terminator is not a branch.
    let returning = mutated(target, |function, environment| {
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
        fold(&returning, &environment).unwrap_err(),
        BoundaryBranchError::UnsupportedInstruction
    );
}

/// A used unit outside the flag universe must lie in the jump row's
/// implicit surface when the fold lands on `Jump`: a foreign non-flag
/// observation the jump cannot carry refuses — while the collapse, which
/// retains the branch's own row and surface verbatim, admits it.
#[test]
fn non_flag_surface_partition_is_outcome_scoped() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let foreign_use = |function: &mut SelectedFunction| {
        // Unit 998 is neither a compare-published flag unit nor in the
        // jump row's program-counter-only surface.
        let spare = register_model::RegisterUnitId(998);
        let instruction = match &mut function.blocks[0].terminator {
            SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. } => instruction,
            _ => unreachable!(),
        };
        instruction.implicit_uses.push(spare);
    };
    // The decided fold rebuilds on the jump row: the foreign use is lost.
    let decided = mutated(target, |function, _| foreign_use(function));
    assert_eq!(
        fold(&decided, &environment).unwrap_err(),
        BoundaryBranchError::UnsupportedUse
    );
    // The collapse keeps the instruction's own surface, so the same
    // observation survives verbatim.
    let collapsing = mutated(target, |function, _| {
        foreign_use(function);
        function.blocks[0].instructions[0].kind = SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(0),
        };
        function.blocks[0].instructions[2].operands[0].virtual_register = LITA;
        function.blocks[0].instructions[2].operands[1].virtual_register = PARAM;
    });
    let result = fold(&collapsing, &environment).unwrap();
    collapsed_to_nonzero(&result, &collapsing, 0);
}

/// A branch whose implicit definitions or clobbers the jump row cannot
/// republish refuses the decided fold: the rebuilt terminator would change
/// the unit events downstream readers observe at this site.
#[test]
fn unpublishable_unit_traffic_refuses() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A second defined unit is not republished by the jump row.
    let extra_def = mutated(target, |function, _| {
        let instruction = match &mut function.blocks[0].terminator {
            SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. } => instruction,
            _ => unreachable!(),
        };
        instruction
            .implicit_defs
            .push(register_model::RegisterUnitId(997));
    });
    assert_eq!(
        fold(&extra_def, &environment).unwrap_err(),
        BoundaryBranchError::ConstraintMismatch
    );
    // A clobber the jump row lacks would lose the event entirely.
    let clobbering = mutated(target, |function, _| {
        let instruction = match &mut function.blocks[0].terminator {
            SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. } => instruction,
            _ => unreachable!(),
        };
        instruction.clobbers = vec![register_model::RegisterUnitId(996)];
    });
    assert_eq!(
        fold(&clobbering, &environment).unwrap_err(),
        BoundaryBranchError::ConstraintMismatch
    );
}

/// The collapse rebuilds on the shared conditional-branch row: a branch
/// instruction carrying a different constraint refuses.
#[test]
fn collapse_requires_the_branch_row() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let off_row = mutated(target, |function, environment| {
        let jump = environment.selected_keys().jump;
        function.blocks[0].instructions[0].kind = SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(0),
        };
        function.blocks[0].instructions[2].operands[0].virtual_register = LITA;
        function.blocks[0].instructions[2].operands[1].virtual_register = PARAM;
        let instruction = match &mut function.blocks[0].terminator {
            SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. } => instruction,
            _ => unreachable!(),
        };
        instruction.constraint = jump;
    });
    assert_eq!(
        fold(&off_row, &environment).unwrap_err(),
        BoundaryBranchError::ConstraintMismatch
    );
}

/// A flag unit reaching in through a predecessor edge resolves to the
/// compare when every path to the branch last observed the same
/// condition-state event — here the compare lives in the entry block and
/// the branch in its jump target — for the decided fold and the collapse
/// alike.
#[test]
fn cross_block_flag_reaching_folds() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let cases: &[(Option<u64>, Option<u64>, bool)] = &[
        (None, Some(0), false),
        (Some(0), None, true),
        (None, Some(u64::MAX), true),
    ];
    for &(left, right, collapsed) in cases {
        let source = mutated(target, |function, environment| {
            function.blocks[0].instructions[0].kind = SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(u128::from(left.unwrap_or(9))),
            };
            function.blocks[0].instructions[1].kind = SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(u128::from(right.unwrap_or(9))),
            };
            function.blocks[0].instructions[2].operands[0].virtual_register =
                left.map(|_| LITA).unwrap_or(PARAM);
            function.blocks[0].instructions[2].operands[1].virtual_register =
                right.map(|_| LITB).unwrap_or(PARAM);
            // The entry block keeps its materializations and compare, then
            // jumps to a new block carrying the branch; its two arms keep
            // the fixture's returning blocks.
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
        if collapsed {
            collapsed_to_nonzero(&result, &source, 3);
        } else {
            // `x <u 0` never holds: the not-taken arm is decided.
            jumped_to(&result, 3, 2);
        }
        validate_boundary_branch_fold(
            &source,
            0,
            BRANCH,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
    }
}

/// A path last touched by a different flag event — here a second compare
/// on a non-pole operand behind the sibling branch — refuses: the observed
/// condition state is not provably the pole compare's.
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
            SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. } => {
                instruction.clone()
            }
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
            terminator: SelectedTerminator::ConditionalBranchU64LessThan {
                instruction: branch_instruction,
                when_less: arm(SelectedBlockId(4), 5, 9),
                when_not_less: arm(SelectedBlockId(5), 6, 10),
            },
        });
        function.blocks.push(returning(4, 10, 11, &terminal_row));
        function.blocks.push(returning(5, 11, 12, &terminal_row));
    });
    assert_eq!(
        fold(&source, &environment).unwrap_err(),
        BoundaryBranchError::UnsupportedUse
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
        BoundaryBranchError::UnsupportedUse
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
            SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. } => instruction
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
        BoundaryBranchError::UnsupportedUse
    );
}

/// An intervening flag event — a second compare or a clobber in the
/// branch's own block — makes the observed state the later instruction's,
/// not the pole compare's: the second compare's own operands carry no
/// pole, and the clobber leaves the observed state unknown.
#[test]
fn intervening_flag_events_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A second compare on ordinary operands is the reaching definition;
    // neither side sits at a pole.
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
        BoundaryBranchError::UnsupportedLiteral
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
        BoundaryBranchError::UnsupportedUse
    );
}

/// A boolean materialization on the decided arm and the branch fold
/// compose: the pole twin folds the reader first, then the branch fold
/// consumes the same compare from the transformed plan.
#[test]
fn validated_results_compose_with_boundary_boolean() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // `x <u 0` through the immediate form feeds both a
    // `MaterializeBooleanU64LessThan` reader on the taken arm and the
    // never-taken branch.
    let source = mutated(target, |function, environment| {
        let immediate_row = environment
            .constraint(environment.selected_keys().compare_i64_immediate)
            .unwrap()
            .clone();
        let boolean = environment
            .constraint(environment.selected_keys().materialize_boolean)
            .unwrap()
            .clone();
        function.blocks[0].instructions[2] = instruction(
            COMPARE,
            SelectedInstructionKind::CompareI64Immediate {
                immediate: IntegerValue::Unsigned(0),
            },
            &immediate_row,
            &[PARAM],
        );
        let mut materialization = instruction(
            EXTRA,
            SelectedInstructionKind::MaterializeBooleanU64LessThan,
            &boolean,
            &[OUTPUT],
        );
        materialization.provenance.values = vec![ValueId::new(5).unwrap()];
        function.blocks[1].instructions.push(materialization);
    });
    // The boolean on the taken arm folds first — `x <u 0` materializes
    // false — then the branch fold consumes the same compare from the
    // transformed plan.
    let boolean_fold =
        crate::fold_selected_boundary_boolean(&source, 0, EXTRA, &environment, budget()).unwrap();
    let result =
        fold_selected_boundary_branch(&boolean_fold, 0, BRANCH, &environment, budget()).unwrap();
    jumped_to(&result, 0, 2);
    assert_eq!(
        result.receipt().source_selected(),
        boolean_fold.receipt().transformed_selected()
    );
}

/// A proposal that does not match the reconstructed terminator — or that
/// retains the source terminator — fails replay.
#[test]
fn replay_mismatches_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target, None, Some(0), BranchShape::U64LessThan);
    // The unchanged source is not a rewritten proposal.
    assert_eq!(
        validate_boundary_branch_fold(
            &source,
            0,
            BRANCH,
            &environment,
            budget(),
            source.transformed().clone()
        )
        .unwrap_err(),
        BoundaryBranchError::ReplayMismatch
    );
    // The wrong surviving successor is not the decided record.
    let mut wrong = source.transformed().clone();
    let jump_row = environment
        .constraint(keys(&environment).jump)
        .unwrap()
        .clone();
    let branch_instruction = match &source.transformed().functions[0].blocks[0].terminator {
        SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. } => instruction.clone(),
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
        validate_boundary_branch_fold(&source, 0, BRANCH, &environment, budget(), wrong)
            .unwrap_err(),
        BoundaryBranchError::ReplayMismatch
    );
    // A `Jump` proposal where admission decided the collapse fails replay
    // the same way.
    let collapse_source = fixture(target, Some(0), None, BranchShape::U64LessThan);
    let mut jumped = collapse_source.transformed().clone();
    let collapse_instruction = match &collapse_source.transformed().functions[0].blocks[0]
        .terminator
    {
        SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. } => instruction.clone(),
        _ => unreachable!(),
    };
    jumped.functions[0].blocks[0].terminator = SelectedTerminator::Jump {
        instruction: SelectedInstruction {
            kind: SelectedInstructionKind::Jump,
            constraint: jump_row.key,
            operands: Vec::new(),
            implicit_uses: jump_row.implicit_uses.clone(),
            implicit_defs: jump_row.implicit_defs.clone(),
            clobbers: jump_row.clobbers.clone(),
            ..collapse_instruction
        },
        successor: arm(SelectedBlockId(2), 3, 3),
    };
    assert_eq!(
        validate_boundary_branch_fold(&collapse_source, 0, BRANCH, &environment, budget(), jumped)
            .unwrap_err(),
        BoundaryBranchError::ReplayMismatch
    );
}

#[test]
fn source_identity_mismatches_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target, None, Some(0), BranchShape::U64LessThan);
    // A function index outside the plan.
    assert_eq!(
        fold_selected_boundary_branch(&source, 1, BRANCH, &environment, budget()).unwrap_err(),
        BoundaryBranchError::SourceMismatch
    );
    // An environment for a different target than the plan's.
    let wrong_environment =
        baseline_target_register_environment(NativeTarget::macos_arm64()).unwrap();
    assert_eq!(
        fold_selected_boundary_branch(&source, 0, BRANCH, &wrong_environment, budget())
            .unwrap_err(),
        BoundaryBranchError::SourceMismatch
    );
}

#[test]
fn validation_budget_covers_the_flag_scan() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target, None, Some(0), BranchShape::U64LessThan);
    let tiny = OptimizationWorkBudget::new(100, 100, 4, 100, 100).unwrap();
    assert_eq!(
        fold_selected_boundary_branch(&source, 0, BRANCH, &environment, tiny).unwrap_err(),
        BoundaryBranchError::WorkBudgetExceeded
    );
}

/// The measured validation-step boundary: admission charges one step per
/// block plus one per instruction across the plan, the admitted function's
/// scan twice for the producer lookups, then the flag walk — the
/// adjacency, cone, and partition-surface setup plus, per walked unit, the
/// block-prefix and last-event scans and the bounded entry-set
/// propagation. The bound prices the collapse's whole flag-universe walk
/// alongside the branch's use roster, so both outcomes share the count:
/// one thousand one hundred forty-nine for the six-element fixture on
/// x86-64. The exact count admits the fold on both the proposal and the
/// independent replay path while one step below rejects both.
#[test]
fn measured_validation_step_boundary_admits_and_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for (left, right) in [(None, Some(0)), (Some(0), None)] {
        let source = fixture(target, left, right, BranchShape::U64LessThan);
        let exact = OptimizationWorkBudget::new(100, 100, 1149, 100, 100).unwrap();
        let folded =
            fold_selected_boundary_branch(&source, 0, BRANCH, &environment, exact).unwrap();
        validate_boundary_branch_fold(
            &source,
            0,
            BRANCH,
            &environment,
            exact,
            folded.transformed().clone(),
        )
        .unwrap();
        let short = OptimizationWorkBudget::new(100, 100, 1148, 100, 100).unwrap();
        assert_eq!(
            fold_selected_boundary_branch(&source, 0, BRANCH, &environment, short).unwrap_err(),
            BoundaryBranchError::WorkBudgetExceeded,
            "{left:?} {right:?}"
        );
        assert_eq!(
            validate_boundary_branch_fold(
                &source,
                0,
                BRANCH,
                &environment,
                short,
                folded.transformed().clone()
            )
            .unwrap_err(),
            BoundaryBranchError::WorkBudgetExceeded,
            "{left:?} {right:?}"
        );
    }
}
