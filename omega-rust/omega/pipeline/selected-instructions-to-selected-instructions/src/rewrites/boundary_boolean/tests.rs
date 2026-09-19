use crate::BoundaryBooleanError;
use crate::BoundaryBooleanReceipt;
use crate::ValidatedBoundaryBoolean;
use crate::ValidatedSelectedAnalysis;
use crate::fold_selected_boundary_boolean;
use crate::validate_boundary_boolean_fold;
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
    OperationId, ScalarType, ValueId,
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
/// `ra = materialize va; rb = materialize vb; compare <registers>;
/// `rout = boolean; return`. `compare_kind` selects the compare form and its
/// constraint row; `compare_registers` picks which register each operand
/// reads — `PARAM` is an entry parameter with no producing instruction, so
/// it stands for the unknown side, while `LITA`/`LITB` carry the emitted
/// literal materializations. Variants rebuild pieces through `mutated`.
fn fixture(
    target: NativeTarget,
    compare_kind: SelectedInstructionKind,
    compare_registers: &[VirtualRegisterId],
    left_value: IntegerValue,
    right_value: IntegerValue,
    boolean_kind: SelectedInstructionKind,
) -> ValidatedBoundaryBoolean {
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let materialize = environment.constraint(keys.materialize_i64).unwrap();
    let compare = environment
        .constraint(match compare_kind {
            SelectedInstructionKind::CompareI64 => keys.compare_i64,
            SelectedInstructionKind::CompareI64Immediate { .. } => keys.compare_i64_immediate,
            SelectedInstructionKind::CompareI64Zero => keys.compare_i64_zero,
            _ => panic!("fixture compare kind"),
        })
        .unwrap();
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
    let mut compare_instruction = instruction(COMPARE, compare_kind, compare, compare_registers);
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
    ValidatedBoundaryBoolean {
        receipt: BoundaryBooleanReceipt {
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

/// The mutation baseline: `compare PARAM, rb` where `rb` is materialized
/// zero — `x <u 0` never holds, so the `U64LessThan` reader folds to zero.
fn mutated(
    target: NativeTarget,
    edit: impl FnOnce(&mut SelectedFunction, &register_environment::ValidatedTargetRegisterEnvironment),
) -> ValidatedBoundaryBoolean {
    let environment = baseline_target_register_environment(target).unwrap();
    let mut source = fixture(
        target,
        SelectedInstructionKind::CompareI64,
        &[PARAM, LITB],
        IntegerValue::Unsigned(0),
        IntegerValue::Unsigned(0),
        SelectedInstructionKind::MaterializeBooleanU64LessThan,
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
    source: &ValidatedBoundaryBoolean,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> Result<ValidatedBoundaryBoolean, BoundaryBooleanError> {
    fold_selected_boundary_boolean(source, 0, BOOLEAN, environment, budget())
}

/// Every pole-decided predicate folds on every target: the strict
/// less-thans are contradictions at the far pole and the less-or-equals are
/// tautologies at the near pole, with the unknown side carrying no
/// producer at all.
#[test]
fn decided_predicates_fold_on_all_targets() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        // (compare operand registers, left literal, right literal, reader
        // kind, folded value). Registers not read by the compare still hold
        // their materializations; the value column for an unread side is a
        // fixture filler.
        let cases: &[(&[VirtualRegisterId], u64, u64, SelectedInstructionKind, u64)] = &[
            // `x <u 0` never holds.
            (
                &[PARAM, LITB],
                9,
                0,
                SelectedInstructionKind::MaterializeBooleanU64LessThan,
                0,
            ),
            // `u64::MAX <u x` never holds.
            (
                &[LITA, PARAM],
                u64::MAX,
                7,
                SelectedInstructionKind::MaterializeBooleanU64LessThan,
                0,
            ),
            // `x <s i64::MIN` never holds.
            (
                &[PARAM, LITB],
                9,
                i64::MIN as u64,
                SelectedInstructionKind::MaterializeBooleanI64LessThan,
                0,
            ),
            // `i64::MAX <s x` never holds.
            (
                &[LITA, PARAM],
                i64::MAX as u64,
                7,
                SelectedInstructionKind::MaterializeBooleanI64LessThan,
                0,
            ),
            // `0 <=u x` always holds.
            (
                &[LITA, PARAM],
                0,
                7,
                SelectedInstructionKind::MaterializeBooleanU64LessOrEqual,
                1,
            ),
            // `x <=u u64::MAX` always holds.
            (
                &[PARAM, LITB],
                9,
                u64::MAX,
                SelectedInstructionKind::MaterializeBooleanU64LessOrEqual,
                1,
            ),
            // `i64::MIN <=s x` always holds.
            (
                &[LITA, PARAM],
                i64::MIN as u64,
                7,
                SelectedInstructionKind::MaterializeBooleanI64LessOrEqual,
                1,
            ),
            // `x <=s i64::MAX` always holds.
            (
                &[PARAM, LITB],
                9,
                i64::MAX as u64,
                SelectedInstructionKind::MaterializeBooleanI64LessOrEqual,
                1,
            ),
        ];
        for (registers, left, right, kind, expected) in cases {
            let source = fixture(
                target,
                SelectedInstructionKind::CompareI64,
                registers,
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
                "{target:?} {kind:?} {registers:?}"
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
            validate_boundary_boolean_fold(
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
            validate_boundary_boolean_fold(&source, 0, BOOLEAN, &environment, budget(), detached)
                .unwrap();
        }
    }
}

/// The zero and immediate compare forms publish their known right operand
/// without a producer; the pole decision applies the same way.
#[test]
fn zero_and_immediate_compares_decide() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // `x - 0` with an unsigned less-than reader: `x <u 0` never holds,
    // whether `x` is open or materialized.
    for registers in [&[PARAM][..], &[LITA][..]] {
        let source = fixture(
            target,
            SelectedInstructionKind::CompareI64Zero,
            registers,
            IntegerValue::Unsigned(5),
            IntegerValue::Unsigned(0),
            SelectedInstructionKind::MaterializeBooleanU64LessThan,
        );
        let result = fold(&source, &environment).unwrap();
        assert_eq!(
            result.transformed().functions[0].blocks[0].instructions[3].kind,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(0)
            },
            "{registers:?}"
        );
    }
    // `x - immediate` at the unsigned maximum: `x <=u u64::MAX` always
    // holds.
    let source = fixture(
        target,
        SelectedInstructionKind::CompareI64Immediate {
            immediate: IntegerValue::Unsigned(u128::from(u64::MAX)),
        },
        &[PARAM],
        IntegerValue::Unsigned(5),
        IntegerValue::Unsigned(0),
        SelectedInstructionKind::MaterializeBooleanU64LessOrEqual,
    );
    let result = fold(&source, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[0].instructions[3].kind,
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(1)
        }
    );
    // A non-pole immediate decides nothing.
    let source = fixture(
        target,
        SelectedInstructionKind::CompareI64Immediate {
            immediate: IntegerValue::Unsigned(5),
        },
        &[PARAM],
        IntegerValue::Unsigned(5),
        IntegerValue::Unsigned(0),
        SelectedInstructionKind::MaterializeBooleanU64LessThan,
    );
    assert_eq!(
        fold(&source, &environment).unwrap_err(),
        BoundaryBooleanError::UnsupportedLiteral
    );
}

/// The far-pole less-or-equals hold exactly at equality: the reader keeps
/// its flag-reading shape — operands, implicit uses, constraint row, and
/// provenance — and only its kind becomes `MaterializeBooleanEqual`.
#[test]
fn equality_at_the_pole_rekinds_the_reader() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let cases: &[(
        SelectedInstructionKind,
        &[VirtualRegisterId],
        u64,
        u64,
        SelectedInstructionKind,
    )] = &[
        // `x <=u 0` is `x == 0`.
        (
            SelectedInstructionKind::CompareI64,
            &[PARAM, LITB],
            9,
            0,
            SelectedInstructionKind::MaterializeBooleanU64LessOrEqual,
        ),
        // `u64::MAX <=u x` is `x == u64::MAX`.
        (
            SelectedInstructionKind::CompareI64,
            &[LITA, PARAM],
            u64::MAX,
            7,
            SelectedInstructionKind::MaterializeBooleanU64LessOrEqual,
        ),
        // `x <=s i64::MIN` is `x == i64::MIN`.
        (
            SelectedInstructionKind::CompareI64,
            &[PARAM, LITB],
            9,
            i64::MIN as u64,
            SelectedInstructionKind::MaterializeBooleanI64LessOrEqual,
        ),
        // `i64::MAX <=s x` is `x == i64::MAX`.
        (
            SelectedInstructionKind::CompareI64,
            &[LITA, PARAM],
            i64::MAX as u64,
            7,
            SelectedInstructionKind::MaterializeBooleanI64LessOrEqual,
        ),
        // `x <=u 0` through the zero form is `x == 0`.
        (
            SelectedInstructionKind::CompareI64Zero,
            &[PARAM],
            9,
            0,
            SelectedInstructionKind::MaterializeBooleanU64LessOrEqual,
        ),
        // `x <=u 0` through the immediate form is `x == 0`.
        (
            SelectedInstructionKind::CompareI64Immediate {
                immediate: IntegerValue::Unsigned(0),
            },
            &[PARAM],
            9,
            0,
            SelectedInstructionKind::MaterializeBooleanU64LessOrEqual,
        ),
    ];
    for (compare_kind, registers, left, right, boolean_kind) in cases {
        let source = fixture(
            target,
            *compare_kind,
            registers,
            IntegerValue::Unsigned(u128::from(*left)),
            IntegerValue::Unsigned(u128::from(*right)),
            *boolean_kind,
        );
        let result = fold(&source, &environment).unwrap();
        let rewritten = &result.transformed().functions[0].blocks[0].instructions[3];
        let original = &source.transformed().functions[0].blocks[0].instructions[3];
        assert_eq!(
            rewritten.kind,
            SelectedInstructionKind::MaterializeBooleanEqual,
            "{compare_kind:?} {registers:?} {boolean_kind:?}"
        );
        // Only the kind field moves: the reader keeps its identity,
        // result register, flag uses, constraint row, and provenance.
        assert_eq!(rewritten.id, original.id);
        assert_eq!(rewritten.constraint, original.constraint);
        assert_eq!(rewritten.operands, original.operands);
        assert_eq!(rewritten.implicit_uses, original.implicit_uses);
        assert_eq!(rewritten.implicit_defs, original.implicit_defs);
        assert_eq!(rewritten.clobbers, original.clobbers);
        assert_eq!(rewritten.provenance, original.provenance);
        validate_boundary_boolean_fold(
            &source,
            0,
            BOOLEAN,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
    }
}

/// Poles whose collapse is a disequality — `0 <u x` is `x != 0`, `x <u
/// u64::MAX` is `x != u64::MAX` and the signed twins — decide no
/// materializable predicate, and non-pole or absent operands decide none at
/// all.
#[test]
fn undecided_predicates_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let cases: &[(&[VirtualRegisterId], u64, u64, SelectedInstructionKind)] = &[
        // `0 <u x` is `x != 0`: no boolean kind materializes the negation.
        (
            &[LITA, PARAM],
            0,
            7,
            SelectedInstructionKind::MaterializeBooleanU64LessThan,
        ),
        // `x <u u64::MAX` is `x != u64::MAX`.
        (
            &[PARAM, LITB],
            9,
            u64::MAX,
            SelectedInstructionKind::MaterializeBooleanU64LessThan,
        ),
        // `i64::MIN <s x` is `x != i64::MIN`.
        (
            &[LITA, PARAM],
            i64::MIN as u64,
            7,
            SelectedInstructionKind::MaterializeBooleanI64LessThan,
        ),
        // `x <s i64::MAX` is `x != i64::MAX`.
        (
            &[PARAM, LITB],
            9,
            i64::MAX as u64,
            SelectedInstructionKind::MaterializeBooleanI64LessThan,
        ),
        // A non-pole right operand decides no ordering predicate.
        (
            &[PARAM, LITB],
            9,
            5,
            SelectedInstructionKind::MaterializeBooleanU64LessThan,
        ),
        (
            &[PARAM, LITB],
            9,
            5,
            SelectedInstructionKind::MaterializeBooleanU64LessOrEqual,
        ),
        (
            &[LITA, PARAM],
            5,
            7,
            SelectedInstructionKind::MaterializeBooleanI64LessOrEqual,
        ),
        // `register - register` is the identity case the constant family
        // owns: the boundary audit reports no poles.
        (
            &[PARAM, PARAM],
            9,
            7,
            SelectedInstructionKind::MaterializeBooleanU64LessThan,
        ),
        (
            &[PARAM, PARAM],
            9,
            7,
            SelectedInstructionKind::MaterializeBooleanI64LessOrEqual,
        ),
    ];
    for (registers, left, right, kind) in cases {
        let source = fixture(
            target,
            SelectedInstructionKind::CompareI64,
            registers,
            IntegerValue::Unsigned(u128::from(*left)),
            IntegerValue::Unsigned(u128::from(*right)),
            *kind,
        );
        assert_eq!(
            fold(&source, &environment).unwrap_err(),
            BoundaryBooleanError::UnsupportedLiteral,
            "{registers:?} {kind:?}"
        );
    }
    // The equality reader has no pole-decidable predicate at all.
    let source = fixture(
        target,
        SelectedInstructionKind::CompareI64,
        &[PARAM, LITB],
        IntegerValue::Unsigned(9),
        IntegerValue::Unsigned(0),
        SelectedInstructionKind::MaterializeBooleanEqual,
    );
    assert_eq!(
        fold(&source, &environment).unwrap_err(),
        BoundaryBooleanError::UnsupportedInstruction
    );
}

/// Only the four ordering boolean kinds read here; any other named
/// instruction refuses before the operand audit.
#[test]
fn non_boolean_instructions_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(
        target,
        SelectedInstructionKind::CompareI64,
        &[PARAM, LITB],
        IntegerValue::Unsigned(9),
        IntegerValue::Unsigned(0),
        SelectedInstructionKind::MaterializeBooleanU64LessThan,
    );
    // The compare itself is not a boolean materialization.
    assert_eq!(
        fold_selected_boundary_boolean(&source, 0, COMPARE, &environment, budget()).unwrap_err(),
        BoundaryBooleanError::UnsupportedInstruction
    );
    // Neither is a plain materialization.
    assert_eq!(
        fold_selected_boundary_boolean(&source, 0, MATERIALIZE_A, &environment, budget())
            .unwrap_err(),
        BoundaryBooleanError::UnsupportedInstruction
    );
    // An absent instruction id cannot be located.
    assert_eq!(
        fold_selected_boundary_boolean(
            &source,
            0,
            SelectedInstructionId(99),
            &environment,
            budget()
        )
        .unwrap_err(),
        BoundaryBooleanError::SourceMismatch
    );
    // A flag-free boolean shape has no condition to evaluate.
    let flag_free = mutated(target, |function, _| {
        function.blocks[0].instructions[3].implicit_uses.clear();
    });
    assert_eq!(
        fold(&flag_free, &environment).unwrap_err(),
        BoundaryBooleanError::UnsupportedInstruction
    );
}

/// A side without a literal guarantee is simply open — no pole means no
/// decision, whether the register carries a copy producer, a second
/// definition anywhere in the function, or a literal wider than the
/// register pattern.
#[test]
fn open_sides_hold_no_pole() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A copy producer leaves the right side open; `x <u <open>` decides
    // nothing.
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
        BoundaryBooleanError::UnsupportedLiteral
    );
    // A second definition anywhere in the function — including a
    // terminator-carried one — breaks the literal guarantee on the right
    // side the unsigned less-than needs.
    let second_definition = mutated(target, |function, environment| {
        let materialize = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap();
        let carried = instruction(
            SelectedInstructionId(6),
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(0),
            },
            materialize,
            &[LITB],
        );
        function.blocks[0].terminator = SelectedTerminator::Return {
            instruction: carried,
            psi_return_edge: EdgeId::new(1).unwrap(),
        };
    });
    assert_eq!(
        fold(&second_definition, &environment).unwrap_err(),
        BoundaryBooleanError::UnsupportedLiteral
    );
    // A literal wider than the sixty-four-bit pattern is no pole.
    let source = fixture(
        target,
        SelectedInstructionKind::CompareI64,
        &[PARAM, LITB],
        IntegerValue::Unsigned(9),
        IntegerValue::Unsigned(u128::from(u64::MAX) + 1),
        SelectedInstructionKind::MaterializeBooleanU64LessThan,
    );
    assert_eq!(
        fold(&source, &environment).unwrap_err(),
        BoundaryBooleanError::UnsupportedLiteral
    );
}

/// A flag unit reaching in through a predecessor edge resolves to the
/// compare when every path to the materialization last observed the same
/// condition-state event — the pole decision then applies exactly as in
/// the single-block form.
#[test]
fn cross_block_flag_reaching_folds() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        let terminal_row = environment
            .constraint(environment.selected_keys().return_unit)
            .unwrap();
        let boolean = function.blocks[0].instructions.pop().unwrap();
        function.blocks[0].terminator = SelectedTerminator::Jump {
            instruction: instruction(
                SelectedInstructionId(6),
                SelectedInstructionKind::Jump,
                jump_row,
                &[],
            ),
            successor: SelectedSuccessor {
                role: SelectedSuccessorRole::Semantic,
                psi_edge: EdgeId::new(2).unwrap(),
                block: SelectedBlockId(1),
                source_target: BlockId::new(2).unwrap(),
                bindings: Vec::new(),
                structural_bindings: Vec::new(),
                structural_case: None,
                fuel: Vec::new(),
            },
        };
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(1),
            origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
            instructions: vec![boolean],
            terminator: SelectedTerminator::Return {
                instruction: instruction(
                    SelectedInstructionId(8),
                    SelectedInstructionKind::ReturnUnit,
                    terminal_row,
                    &[],
                ),
                psi_return_edge: EdgeId::new(3).unwrap(),
            },
        });
    });
    let result = fold(&source, &environment).unwrap();
    let rewritten = &result.transformed().functions[0].blocks[1].instructions[0];
    assert_eq!(rewritten.id, BOOLEAN);
    // `x <u 0` decided across the edge: the folded materialization carries
    // zero.
    assert_eq!(
        rewritten.kind,
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(0)
        }
    );
    assert!(rewritten.implicit_uses.is_empty());
    assert_eq!(
        result.transformed().functions[0].blocks[0].instructions[2].kind,
        SelectedInstructionKind::CompareI64
    );
    validate_boundary_boolean_fold(
        &source,
        0,
        BOOLEAN,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
}

/// A path last touched by a different flag event — here a second compare
/// behind the sibling branch — refuses: the observed condition state is
/// not provably the pole compare's.
#[test]
fn cross_block_divergent_reaching_refuses() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let branch_row = environment
            .constraint(environment.selected_keys().conditional_branch)
            .unwrap();
        let compare_row = environment
            .constraint(environment.selected_keys().compare_i64)
            .unwrap();
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        let terminal_row = environment
            .constraint(environment.selected_keys().return_unit)
            .unwrap();
        let boolean = function.blocks[0].instructions.pop().unwrap();
        let successor = |block: SelectedBlockId, target: u64, edge: u64| SelectedSuccessor {
            role: SelectedSuccessorRole::Semantic,
            psi_edge: EdgeId::new(edge).unwrap(),
            block,
            source_target: BlockId::new(target).unwrap(),
            bindings: Vec::new(),
            structural_bindings: Vec::new(),
            structural_case: None,
            fuel: Vec::new(),
        };
        let jump = |id: u32, edge: u64| SelectedTerminator::Jump {
            instruction: instruction(
                SelectedInstructionId(id),
                SelectedInstructionKind::Jump,
                jump_row,
                &[],
            ),
            successor: successor(SelectedBlockId(3), 4, edge),
        };
        function.blocks[0].terminator = SelectedTerminator::ConditionalBranch {
            instruction: instruction(
                SelectedInstructionId(6),
                SelectedInstructionKind::ConditionalBranchNonZero,
                branch_row,
                &[],
            ),
            when_nonzero: successor(SelectedBlockId(1), 2, 2),
            when_zero: successor(SelectedBlockId(2), 3, 3),
        };
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(1),
            origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
            instructions: Vec::new(),
            terminator: jump(10, 4),
        });
        // The sibling path republishes the flag units from a different
        // instruction before the join.
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(2),
            origin: SelectedBlockOrigin::Source(BlockId::new(3).unwrap()),
            instructions: vec![instruction(
                EXTRA,
                SelectedInstructionKind::CompareI64,
                compare_row,
                &[LITA, PARAM],
            )],
            terminator: jump(11, 5),
        });
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(3),
            origin: SelectedBlockOrigin::Source(BlockId::new(4).unwrap()),
            instructions: vec![boolean],
            terminator: SelectedTerminator::Return {
                instruction: instruction(
                    SelectedInstructionId(8),
                    SelectedInstructionKind::ReturnUnit,
                    terminal_row,
                    &[],
                ),
                psi_return_edge: EdgeId::new(20).unwrap(),
            },
        });
    });
    assert_eq!(
        fold(&source, &environment).unwrap_err(),
        BoundaryBooleanError::UnsupportedUse
    );
}

/// A unit whose reaching walk finds no compare at all — the flag state
/// flows in from the function entry's unknown condition state — refuses.
#[test]
fn cross_block_entry_reaching_refuses() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        let terminal_row = environment
            .constraint(environment.selected_keys().return_unit)
            .unwrap();
        let boolean = function.blocks[0].instructions.pop().unwrap();
        // The entry block keeps the literal materializations but publishes
        // no flag event at all before jumping to the boolean's block.
        function.blocks[0].instructions.remove(2);
        function.blocks[0].terminator = SelectedTerminator::Jump {
            instruction: instruction(
                SelectedInstructionId(6),
                SelectedInstructionKind::Jump,
                jump_row,
                &[],
            ),
            successor: SelectedSuccessor {
                role: SelectedSuccessorRole::Semantic,
                psi_edge: EdgeId::new(2).unwrap(),
                block: SelectedBlockId(1),
                source_target: BlockId::new(2).unwrap(),
                bindings: Vec::new(),
                structural_bindings: Vec::new(),
                structural_case: None,
                fuel: Vec::new(),
            },
        };
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(1),
            origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
            instructions: vec![boolean],
            terminator: SelectedTerminator::Return {
                instruction: instruction(
                    SelectedInstructionId(8),
                    SelectedInstructionKind::ReturnUnit,
                    terminal_row,
                    &[],
                ),
                psi_return_edge: EdgeId::new(3).unwrap(),
            },
        });
    });
    assert_eq!(
        fold(&source, &environment).unwrap_err(),
        BoundaryBooleanError::UnsupportedUse
    );
}

/// A terminator-carried clobber ends the reach like any other event: the
/// predecessor's last event for the flag unit is the jump instruction's
/// own wipe, not the compare, so the materialization refuses.
#[test]
fn cross_block_terminator_event_refuses() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        let terminal_row = environment
            .constraint(environment.selected_keys().return_unit)
            .unwrap();
        let flags = function.blocks[0].instructions[3].implicit_uses.clone();
        let boolean = function.blocks[0].instructions.pop().unwrap();
        let mut jump_instruction = instruction(
            SelectedInstructionId(6),
            SelectedInstructionKind::Jump,
            jump_row,
            &[],
        );
        jump_instruction.clobbers = flags;
        function.blocks[0].terminator = SelectedTerminator::Jump {
            instruction: jump_instruction,
            successor: SelectedSuccessor {
                role: SelectedSuccessorRole::Semantic,
                psi_edge: EdgeId::new(2).unwrap(),
                block: SelectedBlockId(1),
                source_target: BlockId::new(2).unwrap(),
                bindings: Vec::new(),
                structural_bindings: Vec::new(),
                structural_case: None,
                fuel: Vec::new(),
            },
        };
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(1),
            origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
            instructions: vec![boolean],
            terminator: SelectedTerminator::Return {
                instruction: instruction(
                    SelectedInstructionId(8),
                    SelectedInstructionKind::ReturnUnit,
                    terminal_row,
                    &[],
                ),
                psi_return_edge: EdgeId::new(3).unwrap(),
            },
        });
    });
    assert_eq!(
        fold(&source, &environment).unwrap_err(),
        BoundaryBooleanError::UnsupportedUse
    );
}

/// An intervening flag event makes the observed state the later
/// instruction's: a second compare without a pole operand resolves and
/// then fails the boundary audit, while a clobber never resolves.
#[test]
fn intervening_flag_events_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A second compare on a non-pole operand is the reaching definition;
    // `x <u 5` is not pole-decided.
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
                &[PARAM, LITA],
            ),
        );
        function.blocks[0].instructions[0].kind = SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(5),
        };
    });
    assert_eq!(
        fold(&second_compare, &environment).unwrap_err(),
        BoundaryBooleanError::UnsupportedLiteral
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
        BoundaryBooleanError::UnsupportedUse
    );
    // A used unit no in-block instruction defines or clobbers reaches in
    // from outside the block and refuses.
    let outsider = mutated(target, |function, _| {
        let spare = register_model::RegisterUnitId(999);
        function.blocks[0].instructions[3].implicit_uses.push(spare);
    });
    assert_eq!(
        fold(&outsider, &environment).unwrap_err(),
        BoundaryBooleanError::UnsupportedUse
    );
}

/// A second reader of the same pole compare folds independently on the
/// transformed plan: the decided materialization and the re-kinded
/// equality reader coexist.
#[test]
fn validated_results_compose() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // `x <u 0` folds to zero and `x <=u 0` re-kinds to equality off the
    // same published compare state.
    let source = mutated(target, |function, environment| {
        let boolean = environment
            .constraint(environment.selected_keys().materialize_boolean)
            .unwrap();
        let mut second = instruction(
            EXTRA,
            SelectedInstructionKind::MaterializeBooleanU64LessOrEqual,
            boolean,
            &[PARAM],
        );
        second.provenance.values = vec![ValueId::new(6).unwrap()];
        function.blocks[0].instructions.push(second);
    });
    let first = fold(&source, &environment).unwrap();
    let second = fold_selected_boundary_boolean(&first, 0, EXTRA, &environment, budget()).unwrap();
    let instructions = &second.transformed().functions[0].blocks[0].instructions;
    assert_eq!(
        instructions[3].kind,
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(0)
        }
    );
    assert_eq!(
        instructions[4].kind,
        SelectedInstructionKind::MaterializeBooleanEqual
    );
    assert_eq!(
        second.receipt().source_selected(),
        first.receipt().transformed_selected()
    );
}

/// A proposal that does not match the reconstructed form — or that retains
/// the source instruction — fails replay.
#[test]
fn replay_mismatches_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(
        target,
        SelectedInstructionKind::CompareI64,
        &[PARAM, LITB],
        IntegerValue::Unsigned(9),
        IntegerValue::Unsigned(0),
        SelectedInstructionKind::MaterializeBooleanU64LessThan,
    );
    // The unchanged source is not a rewritten proposal.
    assert_eq!(
        validate_boundary_boolean_fold(
            &source,
            0,
            BOOLEAN,
            &environment,
            budget(),
            source.transformed().clone()
        )
        .unwrap_err(),
        BoundaryBooleanError::ReplayMismatch
    );
    // The wrong constant is not the reconstructed form.
    let mut wrong = source.transformed().clone();
    let materialize = environment
        .constraint(environment.selected_keys().materialize_i64)
        .unwrap();
    wrong.functions[0].blocks[0].instructions[3] = instruction(
        BOOLEAN,
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(1),
        },
        materialize,
        &[OUTPUT],
    );
    assert_eq!(
        validate_boundary_boolean_fold(&source, 0, BOOLEAN, &environment, budget(), wrong)
            .unwrap_err(),
        BoundaryBooleanError::ReplayMismatch
    );
    // For the equality collapse, proposing the constant materialize — or
    // any kind but `MaterializeBooleanEqual` — is not the reconstructed
    // form.
    let source = fixture(
        target,
        SelectedInstructionKind::CompareI64,
        &[PARAM, LITB],
        IntegerValue::Unsigned(9),
        IntegerValue::Unsigned(0),
        SelectedInstructionKind::MaterializeBooleanU64LessOrEqual,
    );
    let mut wrong = source.transformed().clone();
    wrong.functions[0].blocks[0].instructions[3] = instruction(
        BOOLEAN,
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(1),
        },
        materialize,
        &[OUTPUT],
    );
    assert_eq!(
        validate_boundary_boolean_fold(&source, 0, BOOLEAN, &environment, budget(), wrong)
            .unwrap_err(),
        BoundaryBooleanError::ReplayMismatch
    );
}

#[test]
fn source_identity_mismatches_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(
        target,
        SelectedInstructionKind::CompareI64,
        &[PARAM, LITB],
        IntegerValue::Unsigned(9),
        IntegerValue::Unsigned(0),
        SelectedInstructionKind::MaterializeBooleanU64LessThan,
    );
    // A function index outside the plan.
    assert_eq!(
        fold_selected_boundary_boolean(&source, 1, BOOLEAN, &environment, budget()).unwrap_err(),
        BoundaryBooleanError::SourceMismatch
    );
    // An environment for a different target than the plan's.
    let wrong_environment =
        baseline_target_register_environment(NativeTarget::macos_arm64()).unwrap();
    assert_eq!(
        fold_selected_boundary_boolean(&source, 0, BOOLEAN, &wrong_environment, budget())
            .unwrap_err(),
        BoundaryBooleanError::SourceMismatch
    );
}

/// The emitted boolean shape is a single plain `[def]` with no unit traffic
/// of its own beyond the flag uses the fold may drop.
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
        BoundaryBooleanError::UnsupportedInstruction
    );
    // A read operand is not the result definition.
    let reading = mutated(target, |function, _| {
        function.blocks[0].instructions[3].operands[0].access = RegisterOperandAccess::Use;
    });
    assert_eq!(
        fold(&reading, &environment).unwrap_err(),
        BoundaryBooleanError::UnsupportedInstruction
    );
    // A pinned result stays an allocation-shaped instruction.
    let pinned = mutated(target, |function, _| {
        function.blocks[0].instructions[3].operands[0].fixed_view =
            Some(register_model::RegisterViewId(0));
    });
    assert_eq!(
        fold(&pinned, &environment).unwrap_err(),
        BoundaryBooleanError::UnsupportedInstruction
    );
    // The boolean cannot carry implicit definitions the materialize row
    // would drop — and the equality collapse would propagate them.
    let defining = mutated(target, |function, _| {
        function.blocks[0].instructions[3].implicit_defs =
            vec![register_model::RegisterUnitId(u16::MAX)];
    });
    assert_eq!(
        fold(&defining, &environment).unwrap_err(),
        BoundaryBooleanError::UnsupportedInstruction
    );
    // Nor clobbers.
    let clobbering = mutated(target, |function, _| {
        function.blocks[0].instructions[3].clobbers =
            vec![register_model::RegisterUnitId(u16::MAX)];
    });
    assert_eq!(
        fold(&clobbering, &environment).unwrap_err(),
        BoundaryBooleanError::UnsupportedInstruction
    );
    // The result register must exist in the roster.
    let missing = mutated(target, |function, _| {
        function
            .virtual_registers
            .retain(|entry| entry.id != OUTPUT);
    });
    assert_eq!(
        fold(&missing, &environment).unwrap_err(),
        BoundaryBooleanError::UnsupportedUse
    );
}

/// The strict-pole collapse also fires when both operands are materialized:
/// the pole decides before the second literal is even consulted, and the
/// outcome agrees with the constant fold the same compare admits there.
#[test]
fn both_literal_poles_fold_the_same() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // `9 <u 0` — the pole decides false before `9` is read.
    let source = fixture(
        target,
        SelectedInstructionKind::CompareI64,
        &[LITA, LITB],
        IntegerValue::Unsigned(9),
        IntegerValue::Unsigned(0),
        SelectedInstructionKind::MaterializeBooleanU64LessThan,
    );
    let result = fold(&source, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[0].instructions[3].kind,
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(0)
        }
    );
    // `0 <=u 0` — the tautology pole takes precedence over the equality
    // collapse the far pole would admit on the same operands.
    let source = fixture(
        target,
        SelectedInstructionKind::CompareI64,
        &[LITA, LITB],
        IntegerValue::Unsigned(0),
        IntegerValue::Unsigned(0),
        SelectedInstructionKind::MaterializeBooleanU64LessOrEqual,
    );
    let result = fold(&source, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[0].instructions[3].kind,
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(1)
        }
    );
}

#[test]
fn validation_budget_covers_the_flag_scan() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(
        target,
        SelectedInstructionKind::CompareI64,
        &[PARAM, LITB],
        IntegerValue::Unsigned(9),
        IntegerValue::Unsigned(0),
        SelectedInstructionKind::MaterializeBooleanU64LessThan,
    );
    let tiny = OptimizationWorkBudget::new(100, 100, 4, 100, 100).unwrap();
    assert_eq!(
        fold_selected_boundary_boolean(&source, 0, BOOLEAN, &environment, tiny).unwrap_err(),
        BoundaryBooleanError::WorkBudgetExceeded
    );
}

/// The measured validation-step boundary: admission charges one step per
/// block plus one per instruction across the plan, then the admitted
/// function's scan twice for the operand audits, then the flag walk — the
/// adjacency setup plus, per used unit, the block-prefix and last-event
/// scans and the bounded entry-set propagation — twenty-nine for the
/// four-instruction fixture on x86-64, thirty-four once a fifth
/// instruction trails the boolean — so the exact count admits the fold on
/// both the proposal and the independent replay path while one step below
/// rejects both.
#[test]
fn measured_validation_step_boundary_admits_and_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A fifth body instruction extends the plan-wide and operand scans and
    // every term the flag walk derives from the function scan: (1 block +
    // 5 instructions) once for the plan, twice for the operand audit, and
    // the walk bound of setup 1 plus a per-unit 15 — thirty-four steps.
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
                SelectedInstructionKind::CompareI64,
                &[PARAM, LITB],
                IntegerValue::Unsigned(9),
                IntegerValue::Unsigned(0),
                SelectedInstructionKind::MaterializeBooleanU64LessThan,
            ),
            29u64,
        ),
        (wider, 34u64),
    ] {
        let exact = OptimizationWorkBudget::new(1, 1, exact_steps, 1, 1).unwrap();
        let result =
            fold_selected_boundary_boolean(&source, 0, BOOLEAN, &environment, exact).unwrap();
        validate_boundary_boolean_fold(
            &source,
            0,
            BOOLEAN,
            &environment,
            exact,
            result.transformed().clone(),
        )
        .unwrap();
        let starved = OptimizationWorkBudget::new(1, 1, exact_steps - 1, 1, 1).unwrap();
        assert_eq!(
            fold_selected_boundary_boolean(&source, 0, BOOLEAN, &environment, starved).unwrap_err(),
            BoundaryBooleanError::WorkBudgetExceeded
        );
        assert_eq!(
            validate_boundary_boolean_fold(
                &source,
                0,
                BOOLEAN,
                &environment,
                starved,
                result.transformed().clone(),
            )
            .unwrap_err(),
            BoundaryBooleanError::WorkBudgetExceeded
        );
    }
}

/// Two runs over the identical source produce the identical validated
/// result, and the published plan is a legal second input: re-running at
/// the same site is terminal because the slot now carries a materialize —
/// or, for the equality collapse, an equality reader no pole decides.
#[test]
fn fold_is_deterministic_and_terminal() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for boolean_kind in [
        SelectedInstructionKind::MaterializeBooleanU64LessThan,
        SelectedInstructionKind::MaterializeBooleanU64LessOrEqual,
    ] {
        let build = || {
            fixture(
                target,
                SelectedInstructionKind::CompareI64,
                &[PARAM, LITB],
                IntegerValue::Unsigned(9),
                IntegerValue::Unsigned(0),
                boolean_kind,
            )
        };
        let first = fold(&build(), &environment).unwrap();
        let second = fold(&build(), &environment).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            fold_selected_boundary_boolean(&first, 0, BOOLEAN, &environment, budget()).unwrap_err(),
            BoundaryBooleanError::UnsupportedInstruction,
            "{boolean_kind:?}"
        );
    }
}

/// Replay rejects any drift from the reconstructed form, in the rewritten
/// slot or anywhere else in the plan.
#[test]
fn replay_rejects_anything_but_the_exact_form() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(
        target,
        SelectedInstructionKind::CompareI64,
        &[PARAM, LITB],
        IntegerValue::Unsigned(9),
        IntegerValue::Unsigned(0),
        SelectedInstructionKind::MaterializeBooleanU64LessThan,
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
                    SelectedInstructionKind::MaterializeBooleanU64LessThan;
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
            validate_boundary_boolean_fold(&source, 0, BOOLEAN, &environment, budget(), proposed)
                .is_err(),
            "mutation {mutation}"
        );
    }
}

/// Replay corruption in a block the fold never touched still rejects: the
/// restore-by-content check compares the complete plan, not just the block
/// carrying the rewritten materialization.
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
                successor: SelectedSuccessor {
                    role: SelectedSuccessorRole::Semantic,
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
            &[PARAM, OUTPUT],
        ));
    assert_eq!(
        validate_boundary_boolean_fold(&source, 0, BOOLEAN, &environment, budget(), proposed)
            .unwrap_err(),
        BoundaryBooleanError::ReplayMismatch
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
        validate_boundary_boolean_fold(&source, 0, BOOLEAN, &environment, budget(), proposed)
            .unwrap_err(),
        BoundaryBooleanError::ReplayMismatch
    );
}
