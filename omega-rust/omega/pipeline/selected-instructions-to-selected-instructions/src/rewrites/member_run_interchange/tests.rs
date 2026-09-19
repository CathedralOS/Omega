use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget};
use optimization_unit::{EffectLink, ValueDefinitionSite};
use register_environment::baseline_target_register_environment;
use register_model::RegisterInstructionConstraint;
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedBoundarySettlement,
    SelectedBoundarySettlementPayload, SelectedCallContract, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan, SelectedMemoryAccess,
    SelectedMemoryAccessOrigin, SelectedMemoryAccessRole, SelectedOperand, SelectedSuccessor,
    SelectedSuccessorRole, SelectedTerminator, VirtualRegister, VirtualRegisterId,
    VirtualRegisterOrigin,
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

use super::{
    MemberRunInterchangeError, MemberRunInterchangeReceipt, ValidatedMemberRunInterchange,
    interchange_selected_member_and_run, validate_member_run_interchange,
};
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

const HEAD: SelectedInstructionId = SelectedInstructionId(2);
const MAT_A: SelectedInstructionId = SelectedInstructionId(3);
const SUM: SelectedInstructionId = SelectedInstructionId(4);
const SEP: SelectedInstructionId = SelectedInstructionId(5);
const MAT_C: SelectedInstructionId = SelectedInstructionId(6);
const DIFF: SelectedInstructionId = SelectedInstructionId(7);
const MAT_Z: SelectedInstructionId = SelectedInstructionId(8);
const TERMINAL: SelectedInstructionId = SelectedInstructionId(9);

const POINTER: VirtualRegisterId = VirtualRegisterId(0);
const FIRST: VirtualRegisterId = VirtualRegisterId(1);
const TOTAL: VirtualRegisterId = VirtualRegisterId(2);
const THIRD: VirtualRegisterId = VirtualRegisterId(3);
const FOURTH: VirtualRegisterId = VirtualRegisterId(4);
const FIFTH: VirtualRegisterId = VirtualRegisterId(5);
const SIXTH: VirtualRegisterId = VirtualRegisterId(6);
const SEVENTH: VirtualRegisterId = VirtualRegisterId(7);

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

fn access(
    instruction: SelectedInstructionId,
    place: PlaceId,
    role: SelectedMemoryAccessRole,
) -> SelectedMemoryAccess {
    SelectedMemoryAccess {
        instruction,
        origin: SelectedMemoryAccessOrigin::Operation(OperationId::new(31).unwrap()),
        place,
        byte_offset: 0,
        byte_count: 8,
        role,
    }
}

/// A raw selected-stage unit fixture, not a source/Terminal admission claim:
/// `r6 = 11; r1 = 5; r2 = r1 + r0; r5 = 9; r3 = 13; r4 = r3 + r0; r7 = 3;
/// return`. The named sides the tests interchange are the member `HEAD` at
/// position 0 or `MAT_Z` at position 6 and the internally coupled runs
/// `MAT_A; SUM` at positions 1..=2 or `MAT_C; DIFF` at positions 4..=5 —
/// each sum reads the materialization's result — with `SEP` a plain
/// interior position and `r0` an entry parameter so each sum's second read
/// has no in-body producer.
fn fixture(target: NativeTarget) -> ValidatedMemberRunInterchange {
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let materialize = environment.constraint(keys.materialize_i64).unwrap();
    let add = environment.constraint(keys.add_i64).unwrap();
    let terminal_row = environment.constraint(keys.return_unit).unwrap();
    let class = add.operands[0].class;
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
            FIRST,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: MAT_A,
                source_value: ValueId::new(2).unwrap(),
            },
        ),
        register(
            TOTAL,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: SUM,
                source_value: ValueId::new(3).unwrap(),
            },
        ),
        register(
            THIRD,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: MAT_C,
                source_value: ValueId::new(4).unwrap(),
            },
        ),
        register(
            FOURTH,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: DIFF,
                source_value: ValueId::new(5).unwrap(),
            },
        ),
        register(
            FIFTH,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: SEP,
                source_value: ValueId::new(6).unwrap(),
            },
        ),
        register(
            SIXTH,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: HEAD,
                source_value: ValueId::new(7).unwrap(),
            },
        ),
        register(
            SEVENTH,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: MAT_Z,
                source_value: ValueId::new(8).unwrap(),
            },
        ),
    ];
    let instructions = vec![
        instruction(
            HEAD,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(11),
            },
            materialize,
            &[SIXTH],
        ),
        instruction(
            MAT_A,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(5),
            },
            materialize,
            &[FIRST],
        ),
        instruction(
            SUM,
            SelectedInstructionKind::WrappingAddI64,
            add,
            &[FIRST, POINTER, TOTAL],
        ),
        instruction(
            SEP,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(9),
            },
            materialize,
            &[FIFTH],
        ),
        instruction(
            MAT_C,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(13),
            },
            materialize,
            &[THIRD],
        ),
        instruction(
            DIFF,
            SelectedInstructionKind::WrappingAddI64,
            add,
            &[THIRD, POINTER, FOURTH],
        ),
        instruction(
            MAT_Z,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(3),
            },
            materialize,
            &[SEVENTH],
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
    ValidatedMemberRunInterchange {
        receipt: MemberRunInterchangeReceipt {
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
) -> ValidatedMemberRunInterchange {
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

fn interchange(
    source: &ValidatedMemberRunInterchange,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    member: SelectedInstructionId,
    run_first: SelectedInstructionId,
    run_last: SelectedInstructionId,
) -> Result<ValidatedMemberRunInterchange, MemberRunInterchangeError> {
    interchange_selected_member_and_run(
        source,
        0,
        member,
        run_first,
        run_last,
        environment,
        budget(),
    )
}

/// A member before a run and the run trade places around an inert interior
/// on every target: each side's members keep their identity, kind,
/// operands, and provenance inside their own order, the interior keeps its
/// relative order shifted by one less than the run's length, and the
/// positions before and after the window stay put.
#[test]
fn member_and_run_interchange_on_every_target() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = fixture(target);
        let result = interchange(&source, &environment, HEAD, MAT_C, DIFF).unwrap();
        let original = &source.transformed().functions[0].blocks[0].instructions;
        let body = &result.transformed().functions[0].blocks[0].instructions;
        assert_eq!(body.len(), original.len());
        assert_eq!(
            body.iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![MAT_C, DIFF, MAT_A, SUM, SEP, HEAD, MAT_Z]
        );
        // Both sides keep their own instruction content: the run landed on
        // the member's position bit-identically and the member on the
        // run's trailing edge.
        assert_eq!(body[0], original[4]);
        assert_eq!(body[1], original[5]);
        assert_eq!(body[2], original[1]);
        assert_eq!(body[3], original[2]);
        assert_eq!(body[4], original[3]);
        assert_eq!(body[5], original[0]);
        assert_eq!(
            result.receipt().source_selected(),
            source.selected_identity()
        );
        assert_eq!(
            result.receipt().transformed_selected(),
            selected_instruction_plan_identity(result.transformed())
        );
        validate_member_run_interchange(
            &source,
            0,
            HEAD,
            MAT_C,
            DIFF,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
        // A detached, separately allocated proposal replays by content.
        let mut detached = result.transformed().clone();
        detached.functions = detached.functions.iter().cloned().collect();
        validate_member_run_interchange(
            &source,
            0,
            HEAD,
            MAT_C,
            DIFF,
            &environment,
            budget(),
            detached,
        )
        .unwrap();
    }
}

/// The member may sit after the run: `MAT_Z` follows the `MAT_A; SUM` run
/// and takes its leading edge while the run takes the member's position
/// and the interior keeps its relative order between them.
#[test]
fn member_later_interchanges_onto_the_runs_span() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let result = interchange(&source, &environment, MAT_Z, MAT_A, SUM).unwrap();
    let body = &result.transformed().functions[0].blocks[0].instructions;
    assert_eq!(
        body.iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![HEAD, MAT_Z, SEP, MAT_C, DIFF, MAT_A, SUM]
    );
    validate_member_run_interchange(
        &source,
        0,
        MAT_Z,
        MAT_A,
        SUM,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
}

/// The interchange is not the rotation either relocation proves: moving
/// the run onto the member's position rotates the crossed window so the
/// interior lands on the member's far side, while the interchange keeps
/// the interior between the traded sides — two different orders, both
/// admitted by their own families.
#[test]
fn interchange_is_not_the_rotation_relocation_proves() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let interchanged = interchange(&source, &environment, HEAD, MAT_C, DIFF).unwrap();
    let relocated =
        crate::relocate_selected_run(&source, 0, MAT_C, DIFF, HEAD, &environment, budget())
            .unwrap();
    let interchanged_body = &interchanged.transformed().functions[0].blocks[0].instructions;
    let relocated_body = &relocated.transformed().functions[0].blocks[0].instructions;
    assert_eq!(
        interchanged_body
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![MAT_C, DIFF, MAT_A, SUM, SEP, HEAD, MAT_Z]
    );
    assert_eq!(
        relocated_body
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![MAT_C, DIFF, HEAD, MAT_A, SUM, SEP, MAT_Z]
    );
}

/// The run's internal couplings are exactly why the pair interchange
/// cannot express this move: the sum reads the materialization it follows,
/// so no singleton crossing carries them together, while the member moves
/// the whole run as one body with their order — and the hazard the sum's
/// read makes — preserved.
#[test]
fn internally_coupled_run_moves_as_one_body() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // The coupled pair cannot interchange at all — the sum reads the
    // register the materialization defines — yet both move together inside
    // the run.
    assert_eq!(
        crate::schedule_selected_pair(&source, 0, MAT_A, SUM, &environment, budget()).unwrap_err(),
        crate::LocalScheduleError::UnsupportedPair
    );
    // A run needs a second run to trade against: naming the run and a
    // one-member span is not the run interchange's shape.
    assert_eq!(
        crate::interchange_selected_runs(
            &source,
            0,
            MAT_A,
            SUM,
            MAT_Z,
            MAT_Z,
            &environment,
            budget()
        )
        .unwrap_err(),
        crate::RunInterchangeError::UnsupportedPair
    );
    let result = interchange(&source, &environment, MAT_Z, MAT_A, SUM).unwrap();
    let body = &result.transformed().functions[0].blocks[0].instructions;
    // The internally coupled members kept their in-run order: the sum
    // still follows its materialization.
    assert!(
        body.windows(2)
            .any(|pair| pair[0].id == MAT_A && pair[1].id == SUM)
    );
}

/// A member adjacent to the run is the empty-interior case on each side:
/// the two sides trade places directly, coinciding with the rotation the
/// sibling relocations prove.
#[test]
fn adjacent_member_and_run_interchange() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let earlier = interchange(&source, &environment, HEAD, MAT_A, SUM).unwrap();
    assert_eq!(
        earlier.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![MAT_A, SUM, HEAD, SEP, MAT_C, DIFF, MAT_Z]
    );
    let later = interchange(&source, &environment, MAT_Z, MAT_C, DIFF).unwrap();
    assert_eq!(
        later.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![HEAD, MAT_A, SUM, SEP, MAT_Z, MAT_C, DIFF]
    );
}

/// A read of a register the other side writes keeps the order: the run's
/// sum cannot observe the member's materialization before it runs, and a
/// trailing member cannot observe the run's sum early.
#[test]
fn raw_dependency_between_sides_keeps_order() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // DIFF's first read moves from THIRD to SIXTH — the member HEAD
    // defines it, so the sides cannot trade.
    let member_read = mutated(target, |function, _| {
        function.blocks[0].instructions[5].operands[0].virtual_register = SIXTH;
    });
    assert_eq!(
        interchange(&member_read, &environment, HEAD, MAT_C, DIFF).unwrap_err(),
        MemberRunInterchangeError::UnsupportedPair
    );
    // The member MAT_Z becomes a sum reading TOTAL, which the run's SUM
    // defines.
    let run_read = mutated(target, |function, environment| {
        let add = environment
            .constraint(environment.selected_keys().add_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[6] = instruction(
            MAT_Z,
            SelectedInstructionKind::WrappingAddI64,
            &add,
            &[TOTAL, POINTER, SEVENTH],
        );
    });
    assert_eq!(
        interchange(&run_read, &environment, MAT_Z, MAT_A, SUM).unwrap_err(),
        MemberRunInterchangeError::UnsupportedPair
    );
}

/// A write to a register the other side reads keeps the order: the member
/// cannot overwrite the entry parameter the run's sums read, and a
/// trailing member cannot overwrite the run's first input.
#[test]
fn war_dependency_between_sides_keeps_order() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // HEAD's definition moves from SIXTH to POINTER — the run's DIFF and
    // the interior's SUM both read it.
    let member_writes = mutated(target, |function, _| {
        function.blocks[0].instructions[0].operands[0].virtual_register = POINTER;
    });
    assert_eq!(
        interchange(&member_writes, &environment, HEAD, MAT_C, DIFF).unwrap_err(),
        MemberRunInterchangeError::UnsupportedPair
    );
    // MAT_Z's definition moves from SEVENTH to FIRST — the run's SUM reads
    // it.
    let run_reads = mutated(target, |function, _| {
        function.blocks[0].instructions[6].operands[0].virtual_register = FIRST;
    });
    assert_eq!(
        interchange(&run_reads, &environment, MAT_Z, MAT_A, SUM).unwrap_err(),
        MemberRunInterchangeError::UnsupportedPair
    );
}

/// Two definitions of one register across the sides cannot exchange:
/// positions after the window observe whichever write ran last.
#[test]
fn waw_dependency_between_sides_keeps_order() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // HEAD and the run's MAT_C both define THIRD.
    let member_first = mutated(target, |function, _| {
        function.blocks[0].instructions[0].operands[0].virtual_register = THIRD;
    });
    assert_eq!(
        interchange(&member_first, &environment, HEAD, MAT_C, DIFF).unwrap_err(),
        MemberRunInterchangeError::UnsupportedPair
    );
    // MAT_Z and the run's SUM both define TOTAL.
    let member_last = mutated(target, |function, _| {
        function.blocks[0].instructions[6].operands[0].virtual_register = TOTAL;
    });
    assert_eq!(
        interchange(&member_last, &environment, MAT_Z, MAT_A, SUM).unwrap_err(),
        MemberRunInterchangeError::UnsupportedPair
    );
}

/// The interior participates in the hazard audit in both directions
/// against both sides: an interior reader of the member's result keeps
/// the member ahead of it, and an interior writer of a register a run
/// member reads keeps the run behind it.
#[test]
fn interior_hazards_keep_order() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The interior instruction becomes a sum reading SIXTH, which the
    // member HEAD defines.
    let interior_reader = mutated(target, |function, environment| {
        let add = environment
            .constraint(environment.selected_keys().add_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[3] = instruction(
            SEP,
            SelectedInstructionKind::WrappingAddI64,
            &add,
            &[SIXTH, POINTER, FIFTH],
        );
    });
    assert_eq!(
        interchange(&interior_reader, &environment, HEAD, MAT_C, DIFF).unwrap_err(),
        MemberRunInterchangeError::UnsupportedPair
    );
    // The interior instruction becomes a sum writing THIRD, which the
    // run's DIFF reads and MAT_C defines.
    let interior_writer = mutated(target, |function, environment| {
        let add = environment
            .constraint(environment.selected_keys().add_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[3] = instruction(
            SEP,
            SelectedInstructionKind::WrappingAddI64,
            &add,
            &[FIRST, POINTER, THIRD],
        );
    });
    assert_eq!(
        interchange(&interior_writer, &environment, HEAD, MAT_C, DIFF).unwrap_err(),
        MemberRunInterchangeError::UnsupportedPair
    );
}

/// Condition-state units couple across the sides exactly like registers: a
/// flag-reading member never crosses the run's publisher, while a flag
/// publisher and its reader inside one run move together.
#[test]
fn condition_state_couples_across_sides_but_not_within_the_run() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The member MAT_Z becomes a flag reader and the run's tail a flag
    // publisher: RAW on the condition-state units refuses.
    let flags_cross = mutated(target, |function, environment| {
        let keys = environment.selected_keys();
        let compare = environment.constraint(keys.compare_i64).unwrap().clone();
        let boolean = environment
            .constraint(keys.materialize_boolean)
            .unwrap()
            .clone();
        function.blocks[0].instructions[2] = instruction(
            SUM,
            SelectedInstructionKind::CompareI64,
            &compare,
            &[FIRST, POINTER],
        );
        function.blocks[0].instructions[6] = instruction(
            MAT_Z,
            SelectedInstructionKind::MaterializeBooleanEqual,
            &boolean,
            &[SEVENTH],
        );
    });
    assert_eq!(
        interchange(&flags_cross, &environment, MAT_Z, MAT_A, SUM).unwrap_err(),
        MemberRunInterchangeError::UnsupportedPair
    );
    // The same publisher/reader pair inside the run moves as one body:
    // their order is preserved, so no hazard the interchange audits can
    // form between them.
    let flags_within = mutated(target, |function, environment| {
        let keys = environment.selected_keys();
        let compare = environment.constraint(keys.compare_i64).unwrap().clone();
        let boolean = environment
            .constraint(keys.materialize_boolean)
            .unwrap()
            .clone();
        function.blocks[0].instructions[1] = instruction(
            MAT_A,
            SelectedInstructionKind::CompareI64,
            &compare,
            &[FIRST, POINTER],
        );
        function.blocks[0].instructions[2] = instruction(
            SUM,
            SelectedInstructionKind::MaterializeBooleanEqual,
            &boolean,
            &[TOTAL],
        );
    });
    interchange(&flags_within, &environment, MAT_Z, MAT_A, SUM).unwrap();
}

/// A roster-carrying side may only cross row-less positions: an accounted
/// actor on the other side or in the interior refuses, while two accounted
/// members inside the run keep their recorded order and admit, an
/// accounted member crossing a row-less run and interior admits, and a
/// row-carrying interior admits when both sides are memory-inert.
#[test]
fn memory_roster_accounting_bounds_the_interchange() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let load_kind = |offset| SelectedInstructionKind::Load8 {
        byte_offset: offset,
    };
    // One accounted member on each side: the sides would trade the
    // recorded accesses' order.
    let both_sides = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[0] =
            instruction(HEAD, load_kind(0), &load, &[POINTER, SIXTH]);
        function.blocks[0].instructions[4] =
            instruction(MAT_C, load_kind(8), &load, &[POINTER, THIRD]);
        function.memory_accesses.push(access(
            HEAD,
            PlaceId::new(1).unwrap(),
            SelectedMemoryAccessRole::ReadPlace,
        ));
        function.memory_accesses.push(access(
            MAT_C,
            PlaceId::new(2).unwrap(),
            SelectedMemoryAccessRole::ReadPlace,
        ));
    });
    assert_eq!(
        interchange(&both_sides, &environment, HEAD, MAT_C, DIFF).unwrap_err(),
        MemberRunInterchangeError::UnsupportedPair
    );
    // An accounted member and an accounted interior position refuse for
    // the same reason.
    let interior_actor = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[0] =
            instruction(HEAD, load_kind(0), &load, &[POINTER, SIXTH]);
        function.blocks[0].instructions[3] =
            instruction(SEP, load_kind(8), &load, &[POINTER, FIFTH]);
        function.memory_accesses.push(access(
            HEAD,
            PlaceId::new(1).unwrap(),
            SelectedMemoryAccessRole::ReadPlace,
        ));
        function.memory_accesses.push(access(
            SEP,
            PlaceId::new(2).unwrap(),
            SelectedMemoryAccessRole::ReadPlace,
        ));
    });
    assert_eq!(
        interchange(&interior_actor, &environment, HEAD, MAT_C, DIFF).unwrap_err(),
        MemberRunInterchangeError::UnsupportedPair
    );
    // Two accounted members inside the run keep their relative order: the
    // run's recorded accesses move together past row-less positions.
    let accounted_run = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[4] =
            instruction(MAT_C, load_kind(0), &load, &[POINTER, THIRD]);
        function.blocks[0].instructions[5] =
            instruction(DIFF, load_kind(8), &load, &[POINTER, FOURTH]);
        function.memory_accesses.push(access(
            MAT_C,
            PlaceId::new(1).unwrap(),
            SelectedMemoryAccessRole::ReadPlace,
        ));
        function.memory_accesses.push(access(
            DIFF,
            PlaceId::new(2).unwrap(),
            SelectedMemoryAccessRole::ReadPlace,
        ));
    });
    let result = interchange(&accounted_run, &environment, HEAD, MAT_C, DIFF).unwrap();
    let body = &result.transformed().functions[0].blocks[0].instructions;
    assert_eq!(body[0].id, MAT_C);
    assert_eq!(body[1].id, DIFF);
    assert_eq!(
        result.transformed().functions[0].memory_accesses,
        accounted_run.transformed().functions[0].memory_accesses
    );
    // An accounted member crosses a row-less run and interior: its
    // recorded access keeps its position relative to every other recorded
    // access.
    let accounted_member = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[0] =
            instruction(HEAD, load_kind(0), &load, &[POINTER, SIXTH]);
        function.memory_accesses.push(access(
            HEAD,
            PlaceId::new(1).unwrap(),
            SelectedMemoryAccessRole::ReadPlace,
        ));
    });
    interchange(&accounted_member, &environment, HEAD, MAT_C, DIFF).unwrap();
    // A row-carrying interior admits while neither side carries rows: its
    // recorded access keeps its relative order inside the window.
    let interior_only = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[3] =
            instruction(SEP, load_kind(0), &load, &[POINTER, FIFTH]);
        function.memory_accesses.push(access(
            SEP,
            PlaceId::new(1).unwrap(),
            SelectedMemoryAccessRole::ReadPlace,
        ));
    });
    interchange(&interior_only, &environment, HEAD, MAT_C, DIFF).unwrap();
}

/// A memory-capable kind without a roster row is an unaccounted access:
/// the interchange cannot prove what it reaches as a member, a run member,
/// or a crossed interior position, so all three refuse outright.
#[test]
fn unaccounted_memory_kinds_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for position in [0, 3, 5] {
        let bare = mutated(target, |function, environment| {
            let store = environment
                .constraint(environment.selected_keys().store.unwrap())
                .unwrap()
                .clone();
            let id = function.blocks[0].instructions[position].id;
            function.blocks[0].instructions[position] = instruction(
                id,
                SelectedInstructionKind::Store {
                    byte_offset: 0,
                    byte_size: 8,
                },
                &store,
                &[POINTER, SIXTH],
            );
        });
        assert_eq!(
            interchange(&bare, &environment, HEAD, MAT_C, DIFF).unwrap_err(),
            MemberRunInterchangeError::UnsupportedInstruction,
            "position {position}"
        );
    }
}

/// Calls, hosted effects, and terminator kinds are barriers as the member,
/// as a run member, and as a crossed interior position, and a call-roster
/// row makes an instruction a barrier even when its kind is register-pure.
#[test]
fn barrier_kinds_and_call_roster_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for position in [0, 3, 5] {
        for kind in [
            SelectedInstructionKind::Jump,
            SelectedInstructionKind::CallUnit {
                callee: MachineId::new(9).unwrap(),
            },
            SelectedInstructionKind::HostedExitProcessI32,
        ] {
            let source = mutated(target, |function, _| {
                function.blocks[0].instructions[position].kind = kind;
            });
            assert_eq!(
                interchange(&source, &environment, HEAD, MAT_C, DIFF).unwrap_err(),
                MemberRunInterchangeError::UnsupportedInstruction,
                "position {position} kind {kind:?}"
            );
        }
    }
    let call_row =
        mutated(target, |function, _| {
            function.calls.push(SelectedCallContract {
            instruction: MAT_C,
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
            effect: EffectLink { input: 0, output: 0 },
            ownership: Vec::new(),
        });
        });
    assert_eq!(
        interchange(&call_row, &environment, HEAD, MAT_C, DIFF).unwrap_err(),
        MemberRunInterchangeError::UnsupportedInstruction
    );
}

/// Every boundary settlement inside the window's span — after the window's
/// first index through its last, in either direction of the member —
/// observes a different executed prefix once the sides trade places, while
/// settlements at or outside the span admit.
#[test]
fn interior_settlements_bound_the_window() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Member earlier: the window is positions 0..=5.
    for position in 1..=5 {
        let settled = mutated(target, |function, _| {
            function.boundary_settlements.push(settlement(position, 41));
        });
        assert_eq!(
            interchange(&settled, &environment, HEAD, MAT_C, DIFF).unwrap_err(),
            MemberRunInterchangeError::UnsupportedPair,
            "settlement at {position}"
        );
    }
    for position in [0, 6] {
        let settled = mutated(target, |function, _| {
            function.boundary_settlements.push(settlement(position, 41));
        });
        interchange(&settled, &environment, HEAD, MAT_C, DIFF).unwrap();
    }
    // Member later: the window is positions 1..=6.
    for position in 2..=6 {
        let settled = mutated(target, |function, _| {
            function.boundary_settlements.push(settlement(position, 41));
        });
        assert_eq!(
            interchange(&settled, &environment, MAT_Z, MAT_A, SUM).unwrap_err(),
            MemberRunInterchangeError::UnsupportedPair,
            "settlement at {position}"
        );
    }
    for position in [0, 1] {
        let settled = mutated(target, |function, _| {
            function.boundary_settlements.push(settlement(position, 41));
        });
        interchange(&settled, &environment, MAT_Z, MAT_A, SUM).unwrap();
    }
}

/// The sides must be one member and one disjoint multi-member run inside
/// one block: a member inside or equal to the run's span, a one-member
/// run, the reversed bounds, and an unknown or foreign-block id all
/// refuse.
#[test]
fn only_a_disjoint_member_and_multi_member_run_interchange() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // A member inside the run's span shares the side it trades against.
    assert_eq!(
        interchange(&source, &environment, SEP, MAT_A, DIFF).unwrap_err(),
        MemberRunInterchangeError::UnsupportedPair
    );
    // A member equal to a run bound names no disjoint side.
    assert_eq!(
        interchange(&source, &environment, MAT_C, MAT_C, DIFF).unwrap_err(),
        MemberRunInterchangeError::UnsupportedPair
    );
    // A one-member run is the pair interchange's granularity.
    assert_eq!(
        interchange(&source, &environment, MAT_Z, MAT_C, MAT_C).unwrap_err(),
        MemberRunInterchangeError::UnsupportedPair
    );
    // The reversed bounds name no ordered run.
    assert_eq!(
        interchange(&source, &environment, MAT_Z, DIFF, MAT_C).unwrap_err(),
        MemberRunInterchangeError::UnsupportedPair
    );
    // An unknown member never locates the window; an unknown run bound
    // names no run.
    assert_eq!(
        interchange(
            &source,
            &environment,
            SelectedInstructionId(99),
            MAT_C,
            DIFF
        )
        .unwrap_err(),
        MemberRunInterchangeError::SourceMismatch
    );
    assert_eq!(
        interchange(
            &source,
            &environment,
            MAT_Z,
            SelectedInstructionId(99),
            DIFF
        )
        .unwrap_err(),
        MemberRunInterchangeError::UnsupportedPair
    );
    assert_eq!(
        interchange(
            &source,
            &environment,
            MAT_Z,
            MAT_C,
            SelectedInstructionId(99)
        )
        .unwrap_err(),
        MemberRunInterchangeError::UnsupportedPair
    );
    // A terminator-carried instruction is not a body member.
    assert_eq!(
        interchange(&source, &environment, TERMINAL, MAT_C, DIFF).unwrap_err(),
        MemberRunInterchangeError::SourceMismatch
    );
    // A member and a run split across blocks name no in-block window.
    let split = mutated(target, |function, environment| {
        let keys = environment.selected_keys();
        let jump_row = environment.constraint(keys.jump).unwrap().clone();
        let return_row = environment.constraint(keys.return_unit).unwrap().clone();
        let moved: Vec<_> = function.blocks[0].instructions.drain(4..).collect();
        function.blocks[0].terminator = SelectedTerminator::Jump {
            instruction: instruction(
                SelectedInstructionId(21),
                SelectedInstructionKind::Jump,
                &jump_row,
                &[],
            ),
            successor: successor(SelectedBlockId(1), BlockId::new(2).unwrap(), 2),
        };
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(1),
            origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
            instructions: moved,
            terminator: SelectedTerminator::Return {
                instruction: instruction(
                    SelectedInstructionId(22),
                    SelectedInstructionKind::ReturnUnit,
                    &return_row,
                    &[],
                ),
                psi_return_edge: EdgeId::new(3).unwrap(),
            },
        });
    });
    assert_eq!(
        interchange(&split, &environment, HEAD, MAT_C, DIFF).unwrap_err(),
        MemberRunInterchangeError::UnsupportedPair
    );
    // A wrong function index never locates the sides.
    assert_eq!(
        interchange_selected_member_and_run(&source, 1, HEAD, MAT_C, DIFF, &environment, budget())
            .unwrap_err(),
        MemberRunInterchangeError::SourceMismatch
    );
}

/// The interchange is not confined to the entry block: the same member and
/// run exchange in a later block, found by the function-wide scan, while
/// the crossed entry block stays bit-identical.
#[test]
fn member_and_run_in_a_later_block_interchange() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let keys = environment.selected_keys();
        let jump_row = environment.constraint(keys.jump).unwrap().clone();
        let return_row = environment.constraint(keys.return_unit).unwrap().clone();
        let materialize = environment
            .constraint(keys.materialize_i64)
            .unwrap()
            .clone();
        let moved = std::mem::take(&mut function.blocks[0].instructions);
        function.blocks[0].instructions = vec![instruction(
            SelectedInstructionId(20),
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(17),
            },
            &materialize,
            &[SIXTH],
        )];
        function.blocks[0].terminator = SelectedTerminator::Jump {
            instruction: instruction(
                SelectedInstructionId(21),
                SelectedInstructionKind::Jump,
                &jump_row,
                &[],
            ),
            successor: successor(SelectedBlockId(1), BlockId::new(2).unwrap(), 2),
        };
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(1),
            origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
            instructions: moved,
            terminator: SelectedTerminator::Return {
                instruction: instruction(
                    SelectedInstructionId(22),
                    SelectedInstructionKind::ReturnUnit,
                    &return_row,
                    &[],
                ),
                psi_return_edge: EdgeId::new(3).unwrap(),
            },
        });
    });
    let result = interchange(&source, &environment, HEAD, MAT_C, DIFF).unwrap();
    let body = &result.transformed().functions[0].blocks[1].instructions;
    assert_eq!(
        body.iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![MAT_C, DIFF, MAT_A, SUM, SEP, HEAD, MAT_Z]
    );
    assert_eq!(
        result.transformed().functions[0].blocks[0],
        source.transformed().functions[0].blocks[0]
    );
}

/// Validation re-derives the admission and compares by content: a proposal
/// that leaves the sides in place, permutes the window differently, or
/// carries an unrelated edit each fails replay even though each is a
/// complete plan.
#[test]
fn replay_rejects_anything_but_the_interchange() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // The source itself is not a proposal: the member and the run were
    // never exchanged.
    assert_eq!(
        validate_member_run_interchange(
            &source,
            0,
            HEAD,
            MAT_C,
            DIFF,
            &environment,
            budget(),
            source.transformed().clone(),
        )
        .unwrap_err(),
        MemberRunInterchangeError::ReplayMismatch
    );
    // A rotation of the same window — the relocation's outcome, not the
    // interchange's — is not the interchange.
    let mut rotated = source.transformed().clone();
    rotated.functions[0].blocks[0]
        .instructions
        .get_mut(0..=5)
        .unwrap()
        .rotate_left(1);
    assert_eq!(
        validate_member_run_interchange(
            &source,
            0,
            HEAD,
            MAT_C,
            DIFF,
            &environment,
            budget(),
            rotated
        )
        .unwrap_err(),
        MemberRunInterchangeError::ReplayMismatch
    );
    // The right interchange plus an unrelated extra edit still fails
    // restore.
    let mut extra = source.transformed().clone();
    {
        let instructions = &mut extra.functions[0].blocks[0].instructions;
        let window: Vec<_> = instructions.drain(0..=5).collect();
        let rearranged: Vec<_> = window[4..]
            .iter()
            .chain(&window[1..4])
            .chain(&window[..1])
            .cloned()
            .collect();
        instructions.splice(0..0, rearranged);
        instructions[6].provenance.operations = vec![OperationId::new(77).unwrap()];
    }
    assert_eq!(
        validate_member_run_interchange(
            &source,
            0,
            HEAD,
            MAT_C,
            DIFF,
            &environment,
            budget(),
            extra
        )
        .unwrap_err(),
        MemberRunInterchangeError::ReplayMismatch
    );
    // The honest interchange replays.
    let result = interchange(&source, &environment, HEAD, MAT_C, DIFF).unwrap();
    validate_member_run_interchange(
        &source,
        0,
        HEAD,
        MAT_C,
        DIFF,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
}

/// Replay corruption in a block the interchange never touched still
/// rejects: the restore-by-content check compares the complete plan, not
/// just the block carrying the traded sides.
#[test]
fn replay_rejects_drift_outside_the_interchanged_block() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let keys = environment.selected_keys();
        let jump_row = environment.constraint(keys.jump).unwrap().clone();
        let return_row = environment.constraint(keys.return_unit).unwrap().clone();
        let materialize = environment
            .constraint(keys.materialize_i64)
            .unwrap()
            .clone();
        let moved = std::mem::take(&mut function.blocks[0].instructions);
        function.blocks[0].instructions = vec![instruction(
            SelectedInstructionId(20),
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(17),
            },
            &materialize,
            &[SIXTH],
        )];
        function.blocks[0].terminator = SelectedTerminator::Jump {
            instruction: instruction(
                SelectedInstructionId(21),
                SelectedInstructionKind::Jump,
                &jump_row,
                &[],
            ),
            successor: successor(SelectedBlockId(1), BlockId::new(2).unwrap(), 2),
        };
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(1),
            origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
            instructions: moved,
            terminator: SelectedTerminator::Return {
                instruction: instruction(
                    SelectedInstructionId(22),
                    SelectedInstructionKind::ReturnUnit,
                    &return_row,
                    &[],
                ),
                psi_return_edge: EdgeId::new(3).unwrap(),
            },
        });
    });
    let result = interchange(&source, &environment, HEAD, MAT_C, DIFF).unwrap();
    // An extra instruction in the untouched entry block rejects.
    let mut proposed = result.transformed().clone();
    let copy = environment
        .constraint(environment.selected_keys().copy_i64)
        .unwrap()
        .clone();
    proposed.functions[0].blocks[0]
        .instructions
        .push(instruction(
            SelectedInstructionId(23),
            SelectedInstructionKind::CopyI64,
            &copy,
            &[FIRST, TOTAL],
        ));
    assert_eq!(
        validate_member_run_interchange(
            &source,
            0,
            HEAD,
            MAT_C,
            DIFF,
            &environment,
            budget(),
            proposed
        )
        .unwrap_err(),
        MemberRunInterchangeError::ReplayMismatch
    );
    // A changed literal in the untouched entry block rejects.
    let mut proposed = result.transformed().clone();
    proposed.functions[0].blocks[0].instructions[0].kind =
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(12),
        };
    assert_eq!(
        validate_member_run_interchange(
            &source,
            0,
            HEAD,
            MAT_C,
            DIFF,
            &environment,
            budget(),
            proposed
        )
        .unwrap_err(),
        MemberRunInterchangeError::ReplayMismatch
    );
}

/// The admission walk is bounded by the validation budget: a plan whose
/// scan cost exceeds it refuses rather than running unbounded.
#[test]
fn work_budget_bounds_the_scan() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    assert_eq!(
        interchange_selected_member_and_run(
            &source,
            0,
            HEAD,
            MAT_C,
            DIFF,
            &environment,
            OptimizationWorkBudget::new(100, 100, 1, 100, 100).unwrap(),
        )
        .unwrap_err(),
        MemberRunInterchangeError::WorkBudgetExceeded
    );
}

/// The measured validation-step boundary: admission charges one step per
/// block plus one per instruction across the plan, then one per operand,
/// implicit use, implicit definition, and clobber row on each
/// member-against-crossed pair, then the admitted function's memory, call,
/// and settlement roster lengths — so the exact count admits the
/// interchange on both the proposal and the independent replay path while
/// one step below rejects both.
#[test]
fn measured_validation_step_boundary_admits_and_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The member and the run move to a second block behind a
    // one-instruction entry: the plan scan grows by the entry block's body
    // instruction and terminator.
    let later_block = mutated(target, |function, environment| {
        let keys = environment.selected_keys();
        let jump_row = environment.constraint(keys.jump).unwrap().clone();
        let return_row = environment.constraint(keys.return_unit).unwrap().clone();
        let materialize = environment
            .constraint(keys.materialize_i64)
            .unwrap()
            .clone();
        let moved = std::mem::take(&mut function.blocks[0].instructions);
        function.blocks[0].instructions = vec![instruction(
            SelectedInstructionId(20),
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(17),
            },
            &materialize,
            &[SIXTH],
        )];
        function.blocks[0].terminator = SelectedTerminator::Jump {
            instruction: instruction(
                SelectedInstructionId(21),
                SelectedInstructionKind::Jump,
                &jump_row,
                &[],
            ),
            successor: successor(SelectedBlockId(1), BlockId::new(2).unwrap(), 2),
        };
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(1),
            origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
            instructions: moved,
            terminator: SelectedTerminator::Return {
                instruction: instruction(
                    SelectedInstructionId(22),
                    SelectedInstructionKind::ReturnUnit,
                    &return_row,
                    &[],
                ),
                psi_return_edge: EdgeId::new(3).unwrap(),
            },
        });
    });
    // The member becomes a roster-carrying load: its second operand joins
    // the member-surface term of each pair it audits and the memory-access
    // row joins the roster term.
    let roster_actor = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[0] = instruction(
            HEAD,
            SelectedInstructionKind::Load8 { byte_offset: 0 },
            &load,
            &[POINTER, SIXTH],
        );
        function.memory_accesses.push(access(
            HEAD,
            PlaceId::new(1).unwrap(),
            SelectedMemoryAccessRole::ReadPlace,
        ));
    });
    for (source, exact_steps) in [
        // (7 instructions + 1 per block) + member-against-crossed surfaces
        // (the member against the 3 interior and 2 run positions, each run
        // member against the 3 interior and the member: 13 pairs on
        // materialization surfaces of 1 and add surfaces of 3 = 42) = 50.
        (fixture(target), 50u64),
        // (8 instructions + 1 per block over 2 blocks) + the same 42 = 52.
        (later_block, 52u64),
        // The fixture's 50 plus the load's extra operand surface on each of
        // the seven pairs the member audits or is audited by, plus one
        // roster row = 58.
        (roster_actor, 58u64),
    ] {
        let exact = OptimizationWorkBudget::new(1, 1, exact_steps, 1, 1).unwrap();
        let result =
            interchange_selected_member_and_run(&source, 0, HEAD, MAT_C, DIFF, &environment, exact)
                .unwrap();
        validate_member_run_interchange(
            &source,
            0,
            HEAD,
            MAT_C,
            DIFF,
            &environment,
            exact,
            result.transformed().clone(),
        )
        .unwrap();
        let starved = OptimizationWorkBudget::new(1, 1, exact_steps - 1, 1, 1).unwrap();
        assert_eq!(
            interchange_selected_member_and_run(
                &source,
                0,
                HEAD,
                MAT_C,
                DIFF,
                &environment,
                starved
            )
            .unwrap_err(),
            MemberRunInterchangeError::WorkBudgetExceeded
        );
        assert_eq!(
            validate_member_run_interchange(
                &source,
                0,
                HEAD,
                MAT_C,
                DIFF,
                &environment,
                starved,
                result.transformed().clone(),
            )
            .unwrap_err(),
            MemberRunInterchangeError::WorkBudgetExceeded
        );
    }
}

/// The environment's target must be the plan's target: an interchange
/// proven for one target's constraints is not evidence on another.
#[test]
fn target_mismatch_rejects() {
    let source = fixture(NativeTarget::linux_x64());
    let environment = baseline_target_register_environment(NativeTarget::linux_arm64()).unwrap();
    assert_eq!(
        interchange(&source, &environment, HEAD, MAT_C, DIFF).unwrap_err(),
        MemberRunInterchangeError::SourceMismatch
    );
}

/// Two interchanges over the identical source produce the identical
/// validated result, and the published plan is a legal second input
/// through the sealed analysis boundary: on the transformed plan the
/// member sits on the run's far side, so the same names interchange back
/// to restore the source plan bit-identically, and a hazard-coupled
/// proposal still declines.
#[test]
fn interchange_is_deterministic_and_re_admitted() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let first = interchange(&source, &environment, HEAD, MAT_C, DIFF).unwrap();
    let second = interchange(&source, &environment, HEAD, MAT_C, DIFF).unwrap();
    assert_eq!(first, second);
    // The validated output carries the sealed analysis boundary, so it is
    // a legal second input — not merely a reconstruction of one. On the
    // transformed plan the member follows the run, so the same names admit
    // the member-later interchange back to the published source.
    let restored =
        interchange_selected_member_and_run(&first, 0, HEAD, MAT_C, DIFF, &environment, budget())
            .unwrap();
    assert_eq!(restored.transformed(), source.transformed());
    assert_eq!(
        restored.receipt().transformed_selected(),
        source.selected_identity()
    );
    validate_member_run_interchange(
        &first,
        0,
        HEAD,
        MAT_C,
        DIFF,
        &environment,
        budget(),
        restored.transformed().clone(),
    )
    .unwrap();
    // A hazard-coupled proposal still declines on the second input: on the
    // transformed plan SUM sits at index 3 behind the run at 0..=1 with
    // MAT_A between them, so naming member SUM against run [MAT_C, DIFF]
    // would move the sum ahead of the materialization whose result it
    // reads.
    assert_eq!(
        interchange_selected_member_and_run(&first, 0, SUM, MAT_C, DIFF, &environment, budget())
            .unwrap_err(),
        MemberRunInterchangeError::UnsupportedPair
    );
}
