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
    RunInterchangeError, RunInterchangeReceipt, ValidatedRunInterchange, interchange_selected_runs,
    validate_run_interchange,
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

const MAT_A: SelectedInstructionId = SelectedInstructionId(2);
const SUM: SelectedInstructionId = SelectedInstructionId(3);
const SEP: SelectedInstructionId = SelectedInstructionId(4);
const MAT_C: SelectedInstructionId = SelectedInstructionId(5);
const DIFF: SelectedInstructionId = SelectedInstructionId(6);
const TERMINAL: SelectedInstructionId = SelectedInstructionId(7);
const HEAD: SelectedInstructionId = SelectedInstructionId(8);
const MAT_Z: SelectedInstructionId = SelectedInstructionId(9);

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
/// return`. The runs the tests interchange are the internally coupled pairs
/// `MAT_A; SUM` at positions 1..=2 — the sum reads the materialization's
/// result — and `MAT_C; DIFF` at positions 4..=5, with `SEP` the interior
/// position between them, `HEAD` ahead of the window, and `MAT_Z` behind it;
/// `r0` is an entry parameter so each sum's second read has no in-body
/// producer.
fn fixture(target: NativeTarget) -> ValidatedRunInterchange {
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
    ValidatedRunInterchange {
        receipt: RunInterchangeReceipt {
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
) -> ValidatedRunInterchange {
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
    source: &ValidatedRunInterchange,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    earlier_first: SelectedInstructionId,
    earlier_last: SelectedInstructionId,
    later_first: SelectedInstructionId,
    later_last: SelectedInstructionId,
) -> Result<ValidatedRunInterchange, RunInterchangeError> {
    interchange_selected_runs(
        source,
        0,
        earlier_first,
        earlier_last,
        later_first,
        later_last,
        environment,
        budget(),
    )
}

/// Two internally coupled runs trade places around an inert interior on
/// every target: each member keeps its identity, kind, operands, and
/// provenance inside its run's own order, the interior instruction keeps
/// its relative order shifted by the length difference (here zero), and the
/// positions before and after the window stay put.
#[test]
fn independent_runs_interchange_on_every_target() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = fixture(target);
        let result = interchange(&source, &environment, MAT_A, SUM, MAT_C, DIFF).unwrap();
        let original = &source.transformed().functions[0].blocks[0].instructions;
        let body = &result.transformed().functions[0].blocks[0].instructions;
        assert_eq!(body.len(), original.len());
        assert_eq!(
            body.iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![HEAD, MAT_C, DIFF, SEP, MAT_A, SUM, MAT_Z]
        );
        // The members keep their own instruction content: the later run's
        // pair landed on the earlier run's span bit-identically.
        assert_eq!(body[1], original[4]);
        assert_eq!(body[2], original[5]);
        assert_eq!(body[3], original[3]);
        assert_eq!(body[4], original[1]);
        assert_eq!(body[5], original[2]);
        assert_eq!(
            result.receipt().source_selected(),
            source.selected_identity()
        );
        assert_eq!(
            result.receipt().transformed_selected(),
            selected_instruction_plan_identity(result.transformed())
        );
        validate_run_interchange(
            &source,
            0,
            MAT_A,
            SUM,
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
        validate_run_interchange(
            &source,
            0,
            MAT_A,
            SUM,
            MAT_C,
            DIFF,
            &environment,
            budget(),
            detached,
        )
        .unwrap();
    }
}

/// The runs' internal couplings are exactly why the pair interchange cannot
/// express this move: the sum reads the materialization it follows, so no
/// singleton crossing carries them together, while the run moves them as
/// one body with their order — and the hazard the sum's read makes —
/// preserved.
#[test]
fn internally_coupled_members_move_with_their_run() {
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
    let result = interchange(&source, &environment, MAT_A, SUM, MAT_C, DIFF).unwrap();
    let body = &result.transformed().functions[0].blocks[0].instructions;
    // The internally coupled members kept their in-run order: the sum still
    // follows its materialization, the difference still follows its own.
    assert!(
        body.windows(2)
            .any(|pair| pair[0].id == MAT_A && pair[1].id == SUM)
    );
    assert!(
        body.windows(2)
            .any(|pair| pair[0].id == MAT_C && pair[1].id == DIFF)
    );
}

/// Runs of different lengths interchange: the earlier run's three members
/// take the later pair's span and the interior keeps its relative order,
/// shifted by the one-instruction difference.
#[test]
fn unequal_runs_interchange_with_the_interior_shifted() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The earlier run grows to three members: [HEAD, MAT_A, SUM].
    let source = fixture(target);
    let result = interchange(&source, &environment, HEAD, SUM, MAT_C, DIFF).unwrap();
    let body = &result.transformed().functions[0].blocks[0].instructions;
    assert_eq!(
        body.iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![MAT_C, DIFF, SEP, HEAD, MAT_A, SUM, MAT_Z]
    );
    validate_run_interchange(
        &source,
        0,
        HEAD,
        SUM,
        MAT_C,
        DIFF,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
}

/// Adjacent runs are the empty-interior case: the two runs trade places
/// directly, coinciding with the rotation the sibling relocation proves.
#[test]
fn adjacent_runs_interchange() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        function.blocks[0].instructions.remove(3);
    });
    let result = interchange(&source, &environment, MAT_A, SUM, MAT_C, DIFF).unwrap();
    let body = &result.transformed().functions[0].blocks[0].instructions;
    assert_eq!(
        body.iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![HEAD, MAT_C, DIFF, MAT_A, SUM, MAT_Z]
    );
}

/// A read of a register the other run writes keeps the order: the later
/// run's sum cannot observe the earlier run's materialization before it
/// runs.
#[test]
fn raw_dependency_between_runs_keeps_order() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // DIFF's first read moves from THIRD to FIRST — the earlier run's
    // materialization defines it, so the runs cannot trade.
    let source = mutated(target, |function, _| {
        function.blocks[0].instructions[5].operands[0].virtual_register = FIRST;
    });
    assert_eq!(
        interchange(&source, &environment, MAT_A, SUM, MAT_C, DIFF).unwrap_err(),
        RunInterchangeError::UnsupportedPair
    );
}

/// A write to a register the other run reads keeps the order: the later
/// run's materialization cannot overwrite the entry parameter the earlier
/// run's sums read.
#[test]
fn war_dependency_between_runs_keeps_order() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        function.blocks[0].instructions[4].operands[0].virtual_register = POINTER;
    });
    assert_eq!(
        interchange(&source, &environment, MAT_A, SUM, MAT_C, DIFF).unwrap_err(),
        RunInterchangeError::UnsupportedPair
    );
}

/// Two definitions of one register across the runs cannot exchange:
/// positions after the window observe whichever write ran last.
#[test]
fn waw_dependency_between_runs_keeps_order() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        function.blocks[0].instructions[4].operands[0].virtual_register = FIRST;
    });
    assert_eq!(
        interchange(&source, &environment, MAT_A, SUM, MAT_C, DIFF).unwrap_err(),
        RunInterchangeError::UnsupportedPair
    );
}

/// The interior participates in the hazard audit in both directions
/// against both runs: an interior reader of the earlier run's result keeps
/// the run behind it, and an interior writer of a register the later run
/// reads keeps the later run ahead of it.
#[test]
fn interior_hazards_keep_order() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The interior instruction becomes a sum reading TOTAL, which the
    // earlier run's SUM defines.
    let interior_reader = mutated(target, |function, environment| {
        let add = environment
            .constraint(environment.selected_keys().add_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[3] = instruction(
            SEP,
            SelectedInstructionKind::WrappingAddI64,
            &add,
            &[TOTAL, POINTER, FIFTH],
        );
    });
    assert_eq!(
        interchange(&interior_reader, &environment, MAT_A, SUM, MAT_C, DIFF).unwrap_err(),
        RunInterchangeError::UnsupportedPair
    );
    // The interior instruction becomes a sum writing THIRD, which the
    // later run's DIFF reads.
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
        interchange(&interior_writer, &environment, MAT_A, SUM, MAT_C, DIFF).unwrap_err(),
        RunInterchangeError::UnsupportedPair
    );
}

/// Condition-state units couple across runs exactly like registers: a flag
/// reader in the later run never crosses the earlier run's publisher,
/// while a flag publisher and its reader inside one run move together.
#[test]
fn condition_state_couples_across_runs_but_not_within() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The later run's tail becomes a flag reader and the earlier run's tail
    // a flag publisher: RAW on the condition-state units refuses.
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
        function.blocks[0].instructions[5] = instruction(
            DIFF,
            SelectedInstructionKind::MaterializeBooleanEqual,
            &boolean,
            &[FOURTH],
        );
    });
    assert_eq!(
        interchange(&flags_cross, &environment, MAT_A, SUM, MAT_C, DIFF).unwrap_err(),
        RunInterchangeError::UnsupportedPair
    );
    // The same publisher/reader pair inside the earlier run moves as one
    // body: their order is preserved, so no hazard the interchange audits
    // can form between them.
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
    interchange(&flags_within, &environment, MAT_A, SUM, MAT_C, DIFF).unwrap();
}

/// A roster-carrying run may only cross row-less positions: an accounted
/// actor in the other run or in the interior refuses, while two accounted
/// members inside one run keep their recorded order and admit, and a
/// row-carrying interior admits when both runs are memory-inert.
#[test]
fn memory_roster_accounting_bounds_the_interchange() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let load_kind = |offset| SelectedInstructionKind::Load8 {
        byte_offset: offset,
    };
    // One accounted member in each run: the runs would trade the recorded
    // accesses' order.
    let both_runs = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[1] =
            instruction(MAT_A, load_kind(0), &load, &[POINTER, FIRST]);
        function.blocks[0].instructions[4] =
            instruction(MAT_C, load_kind(8), &load, &[POINTER, THIRD]);
        function.memory_accesses.push(access(
            MAT_A,
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
        interchange(&both_runs, &environment, MAT_A, SUM, MAT_C, DIFF).unwrap_err(),
        RunInterchangeError::UnsupportedPair
    );
    // An accounted member and an accounted interior position refuse for
    // the same reason.
    let interior_actor = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[1] =
            instruction(MAT_A, load_kind(0), &load, &[POINTER, FIRST]);
        function.blocks[0].instructions[3] =
            instruction(SEP, load_kind(8), &load, &[POINTER, FIFTH]);
        function.memory_accesses.push(access(
            MAT_A,
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
        interchange(&interior_actor, &environment, MAT_A, SUM, MAT_C, DIFF).unwrap_err(),
        RunInterchangeError::UnsupportedPair
    );
    // Two accounted members inside one run keep their relative order: the
    // run's recorded accesses move together past row-less positions.
    let one_run = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[1] =
            instruction(MAT_A, load_kind(0), &load, &[POINTER, FIRST]);
        function.blocks[0].instructions[2] =
            instruction(SUM, load_kind(8), &load, &[POINTER, TOTAL]);
        function.memory_accesses.push(access(
            MAT_A,
            PlaceId::new(1).unwrap(),
            SelectedMemoryAccessRole::ReadPlace,
        ));
        function.memory_accesses.push(access(
            SUM,
            PlaceId::new(2).unwrap(),
            SelectedMemoryAccessRole::ReadPlace,
        ));
    });
    let result = interchange(&one_run, &environment, MAT_A, SUM, MAT_C, DIFF).unwrap();
    let body = &result.transformed().functions[0].blocks[0].instructions;
    assert_eq!(body[4].id, MAT_A);
    assert_eq!(body[5].id, SUM);
    assert_eq!(
        result.transformed().functions[0].memory_accesses,
        one_run.transformed().functions[0].memory_accesses
    );
    // A row-carrying interior admits while neither run carries rows: its
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
    interchange(&interior_only, &environment, MAT_A, SUM, MAT_C, DIFF).unwrap();
}

/// A memory-capable kind without a roster row is an unaccounted access:
/// the interchange cannot prove what it reaches as a member or as a
/// crossed interior position, so both refuse outright.
#[test]
fn unaccounted_memory_kinds_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let bare_member = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[1] = instruction(
            MAT_A,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            &store,
            &[POINTER, FIRST],
        );
    });
    assert_eq!(
        interchange(&bare_member, &environment, MAT_A, SUM, MAT_C, DIFF).unwrap_err(),
        RunInterchangeError::UnsupportedInstruction
    );
    let bare_interior = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[3] = instruction(
            SEP,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            &store,
            &[POINTER, FIFTH],
        );
    });
    assert_eq!(
        interchange(&bare_interior, &environment, MAT_A, SUM, MAT_C, DIFF).unwrap_err(),
        RunInterchangeError::UnsupportedInstruction
    );
}

/// Calls, hosted effects, and terminator kinds are barriers as members and
/// as crossed interior positions, and a call-roster row makes an
/// instruction a barrier even when its kind is register-pure.
#[test]
fn barrier_kinds_and_call_roster_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for position in [1, 3, 5] {
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
                interchange(&source, &environment, MAT_A, SUM, MAT_C, DIFF).unwrap_err(),
                RunInterchangeError::UnsupportedInstruction,
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
        interchange(&call_row, &environment, MAT_A, SUM, MAT_C, DIFF).unwrap_err(),
        RunInterchangeError::UnsupportedInstruction
    );
}

/// Every boundary settlement inside the window's span — after the earlier
/// run's first index through the later run's last — observes a different
/// executed prefix once the runs trade places, while settlements at or
/// outside the span admit.
#[test]
fn interior_settlements_bound_the_window() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for position in 2..=5 {
        let settled = mutated(target, |function, _| {
            function.boundary_settlements.push(settlement(position, 41));
        });
        assert_eq!(
            interchange(&settled, &environment, MAT_A, SUM, MAT_C, DIFF).unwrap_err(),
            RunInterchangeError::UnsupportedPair,
            "settlement at {position}"
        );
    }
    for position in [0, 1, 6] {
        let settled = mutated(target, |function, _| {
            function.boundary_settlements.push(settlement(position, 41));
        });
        interchange(&settled, &environment, MAT_A, SUM, MAT_C, DIFF).unwrap();
    }
}

/// The runs must be two disjoint multi-member spans in the named order
/// inside one block: a one-member run, the reversed order, an overlapping
/// span, and an unknown or foreign-block id all refuse.
#[test]
fn only_ordered_multi_member_runs_interchange() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // A one-member earlier run is the pair interchange's granularity.
    assert_eq!(
        interchange(&source, &environment, MAT_A, MAT_A, MAT_C, DIFF).unwrap_err(),
        RunInterchangeError::UnsupportedPair
    );
    // A one-member later run likewise.
    assert_eq!(
        interchange(&source, &environment, MAT_A, SUM, MAT_C, MAT_C).unwrap_err(),
        RunInterchangeError::UnsupportedPair
    );
    // The reversed order names no ordered pair of runs.
    assert_eq!(
        interchange(&source, &environment, MAT_C, DIFF, MAT_A, SUM).unwrap_err(),
        RunInterchangeError::UnsupportedPair
    );
    // An overlapping span: the later run's first member sits inside the
    // earlier run.
    assert_eq!(
        interchange(&source, &environment, MAT_A, MAT_C, SUM, DIFF).unwrap_err(),
        RunInterchangeError::UnsupportedPair
    );
    // An unknown leading id never locates the window; an unknown trailing
    // id names no run.
    assert_eq!(
        interchange(
            &source,
            &environment,
            SelectedInstructionId(99),
            SUM,
            MAT_C,
            DIFF
        )
        .unwrap_err(),
        RunInterchangeError::SourceMismatch
    );
    assert_eq!(
        interchange(
            &source,
            &environment,
            MAT_A,
            SUM,
            MAT_C,
            SelectedInstructionId(99)
        )
        .unwrap_err(),
        RunInterchangeError::UnsupportedPair
    );
    // A terminator-carried instruction is not a body member.
    assert_eq!(
        interchange(&source, &environment, TERMINAL, MAT_A, MAT_C, DIFF).unwrap_err(),
        RunInterchangeError::SourceMismatch
    );
    // Runs split across blocks name no in-block window.
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
        interchange(&split, &environment, MAT_A, SUM, MAT_C, DIFF).unwrap_err(),
        RunInterchangeError::UnsupportedPair
    );
    // A wrong function index never locates the runs.
    assert_eq!(
        interchange_selected_runs(&source, 1, MAT_A, SUM, MAT_C, DIFF, &environment, budget())
            .unwrap_err(),
        RunInterchangeError::SourceMismatch
    );
}

/// The interchange is not confined to the entry block: the same runs
/// exchange in a later block, found by the function-wide scan, while the
/// crossed entry block stays bit-identical.
#[test]
fn runs_in_a_later_block_interchange() {
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
    let result = interchange(&source, &environment, MAT_A, SUM, MAT_C, DIFF).unwrap();
    let body = &result.transformed().functions[0].blocks[1].instructions;
    assert_eq!(
        body.iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![HEAD, MAT_C, DIFF, SEP, MAT_A, SUM, MAT_Z]
    );
    assert_eq!(
        result.transformed().functions[0].blocks[0],
        source.transformed().functions[0].blocks[0]
    );
}

/// Validation re-derives the admission and compares by content: a proposal
/// that leaves the runs in place, permutes the window differently, or
/// carries an unrelated edit each fails replay even though each is a
/// complete plan.
#[test]
fn replay_rejects_anything_but_the_interchange() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // The source itself is not a proposal: the runs were never exchanged.
    assert_eq!(
        validate_run_interchange(
            &source,
            0,
            MAT_A,
            SUM,
            MAT_C,
            DIFF,
            &environment,
            budget(),
            source.transformed().clone(),
        )
        .unwrap_err(),
        RunInterchangeError::ReplayMismatch
    );
    // A rotation of the same window is not the interchange.
    let mut rotated = source.transformed().clone();
    rotated.functions[0].blocks[0]
        .instructions
        .get_mut(1..=5)
        .unwrap()
        .rotate_left(1);
    assert_eq!(
        validate_run_interchange(
            &source,
            0,
            MAT_A,
            SUM,
            MAT_C,
            DIFF,
            &environment,
            budget(),
            rotated
        )
        .unwrap_err(),
        RunInterchangeError::ReplayMismatch
    );
    // The right interchange plus an unrelated extra edit still fails
    // restore.
    let mut extra = source.transformed().clone();
    {
        let instructions = &mut extra.functions[0].blocks[0].instructions;
        let window: Vec<_> = instructions.drain(1..=5).collect();
        let rearranged: Vec<_> = window[3..]
            .iter()
            .chain(&window[2..3])
            .chain(&window[..2])
            .cloned()
            .collect();
        instructions.splice(1..1, rearranged);
        instructions[6].provenance.operations = vec![OperationId::new(77).unwrap()];
    }
    assert_eq!(
        validate_run_interchange(
            &source,
            0,
            MAT_A,
            SUM,
            MAT_C,
            DIFF,
            &environment,
            budget(),
            extra
        )
        .unwrap_err(),
        RunInterchangeError::ReplayMismatch
    );
    // The honest interchange replays.
    let result = interchange(&source, &environment, MAT_A, SUM, MAT_C, DIFF).unwrap();
    validate_run_interchange(
        &source,
        0,
        MAT_A,
        SUM,
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
/// just the block carrying the traded runs.
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
    let result = interchange(&source, &environment, MAT_A, SUM, MAT_C, DIFF).unwrap();
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
        validate_run_interchange(
            &source,
            0,
            MAT_A,
            SUM,
            MAT_C,
            DIFF,
            &environment,
            budget(),
            proposed
        )
        .unwrap_err(),
        RunInterchangeError::ReplayMismatch
    );
    // A changed literal in the untouched entry block rejects.
    let mut proposed = result.transformed().clone();
    proposed.functions[0].blocks[0].instructions[0].kind =
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(12),
        };
    assert_eq!(
        validate_run_interchange(
            &source,
            0,
            MAT_A,
            SUM,
            MAT_C,
            DIFF,
            &environment,
            budget(),
            proposed
        )
        .unwrap_err(),
        RunInterchangeError::ReplayMismatch
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
        interchange_selected_runs(
            &source,
            0,
            MAT_A,
            SUM,
            MAT_C,
            DIFF,
            &environment,
            OptimizationWorkBudget::new(100, 100, 1, 100, 100).unwrap(),
        )
        .unwrap_err(),
        RunInterchangeError::WorkBudgetExceeded
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
    // The runs move to a second block behind a one-instruction entry: the
    // plan scan grows by the entry block's body instruction and terminator.
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
    // The earlier run's lead becomes a roster-carrying load: its second
    // operand joins the member-surface term of each pair it audits and the
    // memory-access row joins the roster term.
    let roster_actor = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[1] = instruction(
            MAT_A,
            SelectedInstructionKind::Load8 { byte_offset: 0 },
            &load,
            &[POINTER, FIRST],
        );
        function.memory_accesses.push(access(
            MAT_A,
            PlaceId::new(1).unwrap(),
            SelectedMemoryAccessRole::ReadPlace,
        ));
    });
    for (source, exact_steps) in [
        // (7 instructions + 1 per block) + member-against-crossed surfaces
        // (the earlier run's 2 members against SEP and the later run, the
        // later run's 2 against SEP and the earlier run: 12 pairs on
        // materialization surfaces of 1 and add surfaces of 3 = 44) = 52.
        (fixture(target), 52u64),
        // (8 instructions + 1 per block over 2 blocks) + the same 44 = 54.
        (later_block, 54u64),
        // The fixture's 52 plus the load's extra operand surface on each of
        // the five pairs MAT_A audits or is audited by, plus one roster
        // row = 58.
        (roster_actor, 58u64),
    ] {
        let exact = OptimizationWorkBudget::new(1, 1, exact_steps, 1, 1).unwrap();
        let result =
            interchange_selected_runs(&source, 0, MAT_A, SUM, MAT_C, DIFF, &environment, exact)
                .unwrap();
        validate_run_interchange(
            &source,
            0,
            MAT_A,
            SUM,
            MAT_C,
            DIFF,
            &environment,
            exact,
            result.transformed().clone(),
        )
        .unwrap();
        let starved = OptimizationWorkBudget::new(1, 1, exact_steps - 1, 1, 1).unwrap();
        assert_eq!(
            interchange_selected_runs(&source, 0, MAT_A, SUM, MAT_C, DIFF, &environment, starved)
                .unwrap_err(),
            RunInterchangeError::WorkBudgetExceeded
        );
        assert_eq!(
            validate_run_interchange(
                &source,
                0,
                MAT_A,
                SUM,
                MAT_C,
                DIFF,
                &environment,
                starved,
                result.transformed().clone(),
            )
            .unwrap_err(),
            RunInterchangeError::WorkBudgetExceeded
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
        interchange(&source, &environment, MAT_A, SUM, MAT_C, DIFF).unwrap_err(),
        RunInterchangeError::SourceMismatch
    );
}

/// Two runs over the identical source produce the identical validated
/// result, and the published plan is a legal second input through the
/// sealed analysis boundary: on the transformed plan the runs' order is
/// flipped, so naming the stale order refuses, the runs in their new order
/// interchange back to restore the source plan bit-identically, and a
/// hazard-coupled proposal still declines.
#[test]
fn interchange_is_deterministic_and_re_admitted() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let first = interchange(&source, &environment, MAT_A, SUM, MAT_C, DIFF).unwrap();
    let second = interchange(&source, &environment, MAT_A, SUM, MAT_C, DIFF).unwrap();
    assert_eq!(first, second);
    // The validated output carries the sealed analysis boundary, so it is
    // a legal second input — not merely a reconstruction of one. The
    // runs' order flipped there, so the stale ordering no longer names
    // ordered runs in the block.
    assert_eq!(
        interchange_selected_runs(&first, 0, MAT_A, SUM, MAT_C, DIFF, &environment, budget())
            .unwrap_err(),
        RunInterchangeError::UnsupportedPair
    );
    // The runs in their new order interchange back through the same
    // admission and replay, restoring the published source bit-identically.
    let restored =
        interchange_selected_runs(&first, 0, MAT_C, DIFF, MAT_A, SUM, &environment, budget())
            .unwrap();
    assert_eq!(restored.transformed(), source.transformed());
    assert_eq!(
        restored.receipt().transformed_selected(),
        source.selected_identity()
    );
    validate_run_interchange(
        &first,
        0,
        MAT_C,
        DIFF,
        MAT_A,
        SUM,
        &environment,
        budget(),
        restored.transformed().clone(),
    )
    .unwrap();
    // A hazard-coupled proposal still declines on the second input: on the
    // transformed plan MAT_C and DIFF head the body, so naming the runs
    // [HEAD, MAT_C] and [DIFF, SEP] would move the difference ahead of the
    // materialization whose result it reads.
    assert_eq!(
        interchange_selected_runs(&first, 0, HEAD, MAT_C, DIFF, SEP, &environment, budget())
            .unwrap_err(),
        RunInterchangeError::UnsupportedPair
    );
}
