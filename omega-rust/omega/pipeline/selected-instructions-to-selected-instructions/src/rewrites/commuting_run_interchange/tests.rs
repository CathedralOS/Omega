use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget};
use optimization_unit::{EffectLink, ValueDefinitionSite};
use register_environment::baseline_target_register_environment;
use register_model::RegisterInstructionConstraint;
use selected_instructions::{
    LocalStorageSlotId, SelectedBlock, SelectedBlockId, SelectedBlockOrigin,
    SelectedBoundarySettlement, SelectedBoundarySettlementPayload, SelectedCallContract,
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionPlan, SelectedMemoryAccess, SelectedMemoryAccessOrigin,
    SelectedMemoryAccessRole, SelectedOperand, SelectedSuccessor, SelectedSuccessorRole,
    SelectedTerminator, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
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
    CommutingRunInterchangeError, CommutingRunInterchangeReceipt, ValidatedCommutingRunInterchange,
    interchange_selected_commuting_runs, validate_commuting_run_interchange,
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

const LOAD_A: SelectedInstructionId = SelectedInstructionId(2);
const SUM: SelectedInstructionId = SelectedInstructionId(3);
const SEP: SelectedInstructionId = SelectedInstructionId(4);
const LOAD_C: SelectedInstructionId = SelectedInstructionId(5);
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
    instruction: SelectedInstructionId,
    source_value: u64,
) -> VirtualRegister {
    VirtualRegister {
        id,
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        class,
        origin: VirtualRegisterOrigin::InstructionResult {
            instruction,
            source_value: ValueId::new(source_value).unwrap(),
        },
        definition_site: None,
        entry_fixed_view: None,
    }
}

fn access(
    instruction: SelectedInstructionId,
    place: u64,
    role: SelectedMemoryAccessRole,
    byte_offset: u32,
    byte_count: u32,
) -> SelectedMemoryAccess {
    SelectedMemoryAccess {
        instruction,
        origin: SelectedMemoryAccessOrigin::Operation(OperationId::new(31).unwrap()),
        place: PlaceId::new(place).unwrap(),
        byte_offset,
        byte_count,
        role,
    }
}

/// A dynamic-extent row: the span and sequence roles carry a zero recorded
/// extent by contract.
fn dynamic_access(
    instruction: SelectedInstructionId,
    place: u64,
    role: SelectedMemoryAccessRole,
) -> SelectedMemoryAccess {
    access(instruction, place, role, 0, 0)
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

/// A raw selected-stage unit fixture, not a source/Terminal admission claim:
/// `r6 = 11; r1 = *r0+0; r2 = r1 + r0; r5 = 9; r3 = *r0+8; r4 = r3 + r0;
/// r7 = 3; return`. The runs the tests interchange are the internally
/// coupled pairs `LOAD_A; SUM` at positions 1..=2 — the sum reads the
/// load's result — and `LOAD_C; DIFF` at positions 4..=5, with `SEP` the
/// interior position between them, `HEAD` ahead of the window, and `MAT_Z`
/// behind it; `r0` is an entry parameter so each load's address operand and
/// each sum's second read have no in-body producer. Each run leads with a
/// load whose roster row reads a place the other run never names, so the
/// traded accesses commute.
fn fixture(target: NativeTarget) -> ValidatedCommutingRunInterchange {
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let materialize = environment.constraint(keys.materialize_i64).unwrap();
    let add = environment.constraint(keys.add_i64).unwrap();
    let load = environment.constraint(keys.load8.unwrap()).unwrap();
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
        register(FIRST, class, LOAD_A, 2),
        register(TOTAL, class, SUM, 3),
        register(THIRD, class, LOAD_C, 4),
        register(FOURTH, class, DIFF, 5),
        register(FIFTH, class, SEP, 6),
        register(SIXTH, class, HEAD, 7),
        register(SEVENTH, class, MAT_Z, 8),
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
            LOAD_A,
            SelectedInstructionKind::Load8 { byte_offset: 0 },
            load,
            &[POINTER, FIRST],
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
            LOAD_C,
            SelectedInstructionKind::Load8 { byte_offset: 8 },
            load,
            &[POINTER, THIRD],
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
            memory_accesses: vec![
                access(LOAD_A, 1, SelectedMemoryAccessRole::ReadPlace, 0, 8),
                access(LOAD_C, 2, SelectedMemoryAccessRole::ReadPlace, 8, 8),
            ],
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
    ValidatedCommutingRunInterchange {
        transformed: std::sync::Arc::new(plan),
        receipt: CommutingRunInterchangeReceipt {
            source_selected: identity,
            transformed_selected: identity,
            optimization_unit: OptimizationUnitIdentity::from_bytes([2; 32]),
            fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
        },
    }
}

fn mutated(
    target: NativeTarget,
    edit: impl FnOnce(&mut SelectedFunction, &register_environment::ValidatedTargetRegisterEnvironment),
) -> ValidatedCommutingRunInterchange {
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
    source: &ValidatedCommutingRunInterchange,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    earlier_first: SelectedInstructionId,
    earlier_last: SelectedInstructionId,
    later_first: SelectedInstructionId,
    later_last: SelectedInstructionId,
) -> Result<ValidatedCommutingRunInterchange, CommutingRunInterchangeError> {
    interchange_selected_commuting_runs(
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

/// Two internally coupled runs whose accounted loads read distinct places
/// trade places around an inert interior on every target: each member keeps
/// its identity, kind, operands, and provenance inside its run's own order,
/// the interior instruction keeps its relative order, and the roster's
/// window rows follow the new execution order — the later run's row, then
/// the earlier run's row — while the receipt binds both identities.
#[test]
fn commuting_runs_interchange_on_every_target() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = fixture(target);
        let result = interchange(&source, &environment, LOAD_A, SUM, LOAD_C, DIFF).unwrap();
        let original = &source.transformed().functions[0].blocks[0].instructions;
        let body = &result.transformed().functions[0].blocks[0].instructions;
        assert_eq!(body.len(), original.len());
        assert_eq!(
            body.iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![HEAD, LOAD_C, DIFF, SEP, LOAD_A, SUM, MAT_Z]
        );
        assert_eq!(body[1], original[4]);
        assert_eq!(body[2], original[5]);
        assert_eq!(body[3], original[3]);
        assert_eq!(body[4], original[1]);
        assert_eq!(body[5], original[2]);
        // The roster followed the new execution order: LOAD_C's row now
        // leads the window and LOAD_A's closes it, each row itself
        // bit-identical.
        let roster = &result.transformed().functions[0].memory_accesses;
        let source_roster = &source.transformed().functions[0].memory_accesses;
        assert_eq!(roster.len(), source_roster.len());
        assert_eq!(roster[0], source_roster[1]);
        assert_eq!(roster[1], source_roster[0]);
        assert_eq!(
            result.receipt().source_selected(),
            source.selected_identity()
        );
        assert_eq!(
            result.receipt().transformed_selected(),
            selected_instruction_plan_identity(result.transformed())
        );
        assert_eq!(
            result.receipt().optimization_unit(),
            source.optimization_unit_identity()
        );
        assert_eq!(
            result.receipt().fuel_schedule(),
            source.fuel_schedule_identity()
        );
        validate_commuting_run_interchange(
            &source,
            0,
            LOAD_A,
            SUM,
            LOAD_C,
            DIFF,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
    }
}

/// The family's accounting rule admits every commuting row shape the run
/// interchange refused: two accounted runs on disjoint extents of one
/// place, a run whose store stages into a boundary slot no place shares, a
/// dynamic-extent read against a write on a different place, and a rowed
/// interior whose own read commutes with both runs' rows.
#[test]
fn commuting_access_shapes_admit() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Two writes on disjoint fixed extents of one place: the runs' heads
    // store disjoint halves of the same place, so neither observes the
    // other's bytes.
    let disjoint = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[1] = instruction(
            LOAD_A,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            &store,
            &[POINTER, FIRST],
        );
        function.blocks[0].instructions[4] = instruction(
            LOAD_C,
            SelectedInstructionKind::Store {
                byte_offset: 24,
                byte_size: 8,
            },
            &store,
            &[POINTER, THIRD],
        );
        function.memory_accesses = vec![
            access(LOAD_A, 1, SelectedMemoryAccessRole::WritePlace, 0, 8),
            access(LOAD_C, 1, SelectedMemoryAccessRole::WritePlace, 24, 8),
        ];
    });
    let result = interchange(&disjoint, &environment, LOAD_A, SUM, LOAD_C, DIFF).unwrap();
    assert_eq!(
        result.transformed().functions[0].memory_accesses[0].instruction,
        LOAD_C
    );
    // A write into a boundary slot is storage distinct from any place: the
    // slot is never a place's own storage, so the staged bytes never reach
    // the place the other run reads.
    let staged = mutated(target, |function, _| {
        function.memory_accesses = vec![
            access(LOAD_A, 1, SelectedMemoryAccessRole::ReadPlace, 0, 8),
            access(
                LOAD_C,
                1,
                SelectedMemoryAccessRole::WriteLocal {
                    slot: LocalStorageSlotId::Boundary {
                        operation: OperationId::new(61).unwrap(),
                    },
                },
                0,
                8,
            ),
        ];
    });
    interchange(&staged, &environment, LOAD_A, SUM, LOAD_C, DIFF).unwrap();
    // A dynamic-extent read commutes with a write whose reach it provably
    // never shares — here a different place entirely.
    let span = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[4] = instruction(
            LOAD_C,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            &store,
            &[POINTER, THIRD],
        );
        function.memory_accesses = vec![
            dynamic_access(
                LOAD_A,
                1,
                SelectedMemoryAccessRole::ReadByteSpan {
                    length: ValueId::new(11).unwrap(),
                    obligation: ObligationId::new(12).unwrap(),
                    accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                        [7; 32],
                    ),
                },
            ),
            access(LOAD_C, 2, SelectedMemoryAccessRole::WritePlace, 0, 8),
        ];
    });
    interchange(&span, &environment, LOAD_A, SUM, LOAD_C, DIFF).unwrap();
    // A rowed interior position joins the commutation audit: its read of a
    // third place trades order with both runs and commutes with each.
    let rowed_interior = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[3] = instruction(
            SEP,
            SelectedInstructionKind::Load8 { byte_offset: 16 },
            &load,
            &[POINTER, FIFTH],
        );
        function.memory_accesses = vec![
            access(LOAD_A, 1, SelectedMemoryAccessRole::ReadPlace, 0, 8),
            access(SEP, 3, SelectedMemoryAccessRole::ReadPlace, 16, 8),
            access(LOAD_C, 2, SelectedMemoryAccessRole::ReadPlace, 8, 8),
        ];
    });
    let result = interchange(&rowed_interior, &environment, LOAD_A, SUM, LOAD_C, DIFF).unwrap();
    let roster = &result.transformed().functions[0].memory_accesses;
    assert_eq!(roster[0].instruction, LOAD_C);
    assert_eq!(roster[1].instruction, SEP);
    assert_eq!(roster[2].instruction, LOAD_A);
}

/// The commutation bound: a write whose reach cannot be bounded away from
/// a crossed access refuses — overlapping fixed extents on one place
/// between the runs, two writes on a shared extent, a dynamic-extent span
/// on the shared place, and a rowed interior position whose read conflicts
/// with a run's write. The traded order of conflicting accesses is never
/// speculative.
#[test]
fn noncommuting_accesses_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Overlapping fixed extents on one place, one run reading and the
    // other writing.
    let overlapping = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[4] = instruction(
            LOAD_C,
            SelectedInstructionKind::Store {
                byte_offset: 4,
                byte_size: 8,
            },
            &store,
            &[POINTER, THIRD],
        );
        function.memory_accesses = vec![
            access(LOAD_A, 1, SelectedMemoryAccessRole::ReadPlace, 0, 8),
            access(LOAD_C, 1, SelectedMemoryAccessRole::WritePlace, 4, 8),
        ];
    });
    assert_eq!(
        interchange(&overlapping, &environment, LOAD_A, SUM, LOAD_C, DIFF).unwrap_err(),
        CommutingRunInterchangeError::UnsupportedPair
    );
    // Two writes on one place's shared extent: either order changes the
    // bytes both leave behind.
    let writes = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[1] = instruction(
            LOAD_A,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            &store,
            &[POINTER, FIRST],
        );
        function.blocks[0].instructions[4] = instruction(
            LOAD_C,
            SelectedInstructionKind::Store {
                byte_offset: 8,
                byte_size: 8,
            },
            &store,
            &[POINTER, THIRD],
        );
        function.memory_accesses = vec![
            access(LOAD_A, 1, SelectedMemoryAccessRole::WritePlace, 0, 16),
            access(LOAD_C, 1, SelectedMemoryAccessRole::WritePlace, 8, 8),
        ];
    });
    assert_eq!(
        interchange(&writes, &environment, LOAD_A, SUM, LOAD_C, DIFF).unwrap_err(),
        CommutingRunInterchangeError::UnsupportedPair
    );
    // A dynamic-extent span cannot be bounded away from the crossed write
    // on the same place even though its own recorded extent is empty.
    let span = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[4] = instruction(
            LOAD_C,
            SelectedInstructionKind::Store {
                byte_offset: 64,
                byte_size: 8,
            },
            &store,
            &[POINTER, THIRD],
        );
        function.memory_accesses = vec![
            dynamic_access(
                LOAD_A,
                1,
                SelectedMemoryAccessRole::ReadByteSpan {
                    length: ValueId::new(11).unwrap(),
                    obligation: ObligationId::new(12).unwrap(),
                    accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                        [7; 32],
                    ),
                },
            ),
            access(LOAD_C, 1, SelectedMemoryAccessRole::WritePlace, 64, 8),
        ];
    });
    assert_eq!(
        interchange(&span, &environment, LOAD_A, SUM, LOAD_C, DIFF).unwrap_err(),
        CommutingRunInterchangeError::UnsupportedPair
    );
    // A run member's row against an accounted interior position refuses
    // under the same rule: the write trades order with the interior's read
    // of the same bytes.
    let interior = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap()
            .clone();
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[1] = instruction(
            LOAD_A,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            &store,
            &[POINTER, FIRST],
        );
        function.blocks[0].instructions[3] = instruction(
            SEP,
            SelectedInstructionKind::Load8 { byte_offset: 0 },
            &load,
            &[POINTER, FIFTH],
        );
        function.memory_accesses = vec![
            access(LOAD_A, 1, SelectedMemoryAccessRole::WritePlace, 0, 8),
            access(SEP, 1, SelectedMemoryAccessRole::ReadPlace, 0, 8),
            access(LOAD_C, 2, SelectedMemoryAccessRole::ReadPlace, 8, 8),
        ];
    });
    assert_eq!(
        interchange(&interior, &environment, LOAD_A, SUM, LOAD_C, DIFF).unwrap_err(),
        CommutingRunInterchangeError::UnsupportedPair
    );
}

/// The run interchange's own accounting cases stay with it: a window whose
/// trading pairs carry no rowed-vs-rowed trade — no rows at all, rows only
/// on one run, or a rowed interior alone against memory-inert runs — is
/// `UnsupportedPair` here even though the commutation audit is vacuously
/// satisfied, and the plain run family admits each.
#[test]
fn single_actor_windows_belong_to_the_run_interchange() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // No rows at all: the plain run-interchange case.
    let bare = mutated(target, |function, _| {
        function.memory_accesses = Vec::new();
    });
    assert_eq!(
        interchange(&bare, &environment, LOAD_A, SUM, LOAD_C, DIFF).unwrap_err(),
        CommutingRunInterchangeError::UnsupportedPair
    );
    crate::interchange_selected_runs(&bare, 0, LOAD_A, SUM, LOAD_C, DIFF, &environment, budget())
        .unwrap();
    // Rows on one run only: the single-actor case.
    let one_side = mutated(target, |function, _| {
        function
            .memory_accesses
            .retain(|access| access.instruction == LOAD_A);
    });
    assert_eq!(
        interchange(&one_side, &environment, LOAD_A, SUM, LOAD_C, DIFF).unwrap_err(),
        CommutingRunInterchangeError::UnsupportedPair
    );
    crate::interchange_selected_runs(
        &one_side,
        0,
        LOAD_A,
        SUM,
        LOAD_C,
        DIFF,
        &environment,
        budget(),
    )
    .unwrap();
    // A rowed interior alone admits in the run family; here no trading
    // pair is rowed on both sides either.
    let interior_only = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[3] = instruction(
            SEP,
            SelectedInstructionKind::Load8 { byte_offset: 16 },
            &load,
            &[POINTER, FIFTH],
        );
        function.memory_accesses = vec![access(SEP, 1, SelectedMemoryAccessRole::ReadPlace, 16, 8)];
    });
    assert_eq!(
        interchange(&interior_only, &environment, LOAD_A, SUM, LOAD_C, DIFF).unwrap_err(),
        CommutingRunInterchangeError::UnsupportedPair
    );
    crate::interchange_selected_runs(
        &interior_only,
        0,
        LOAD_A,
        SUM,
        LOAD_C,
        DIFF,
        &environment,
        budget(),
    )
    .unwrap();
}

/// Runs of one member are the commuting pair's and the member-against-run
/// interchange's granularity, not this family's: a repeated or adjacent
/// bound names no run.
#[test]
fn one_member_runs_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    for (earlier_first, earlier_last, later_first, later_last) in [
        (LOAD_A, LOAD_A, LOAD_C, DIFF),
        (LOAD_A, SUM, LOAD_C, LOAD_C),
        (LOAD_A, LOAD_A, LOAD_C, LOAD_C),
        (SUM, LOAD_A, LOAD_C, DIFF),
        (LOAD_A, SUM, DIFF, LOAD_C),
    ] {
        assert_eq!(
            interchange(
                &source,
                &environment,
                earlier_first,
                earlier_last,
                later_first,
                later_last
            )
            .unwrap_err(),
            CommutingRunInterchangeError::UnsupportedPair,
            "{earlier_first:?}..={earlier_last:?} vs {later_first:?}..={later_last:?}"
        );
    }
}

/// Register and condition-state hazards keep the run interchange's audit
/// unchanged: a crossed position writing a register a member reads refuses
/// the trade, in either direction across the runs and through the
/// interior.
#[test]
fn register_hazards_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // DIFF's first read moves to FIRST — the earlier run's load defines
    // it, so the runs cannot trade.
    let raw = mutated(target, |function, _| {
        function.blocks[0].instructions[5].operands[0].virtual_register = FIRST;
    });
    assert_eq!(
        interchange(&raw, &environment, LOAD_A, SUM, LOAD_C, DIFF).unwrap_err(),
        CommutingRunInterchangeError::UnsupportedPair
    );
    // The later run's head overwrites POINTER, which the earlier run's
    // load and sum both read.
    let war = mutated(target, |function, environment| {
        let add = environment
            .constraint(environment.selected_keys().add_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[4] = instruction(
            LOAD_C,
            SelectedInstructionKind::WrappingAddI64,
            &add,
            &[FIRST, POINTER, POINTER],
        );
    });
    assert_eq!(
        interchange(&war, &environment, LOAD_A, SUM, LOAD_C, DIFF).unwrap_err(),
        CommutingRunInterchangeError::UnsupportedPair
    );
    // The interior instruction writes TOTAL, which the earlier run's sum
    // defines: a crossed WAW against the earlier run.
    let interior_waw = mutated(target, |function, environment| {
        let add = environment
            .constraint(environment.selected_keys().add_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[3] = instruction(
            SEP,
            SelectedInstructionKind::WrappingAddI64,
            &add,
            &[FIRST, POINTER, TOTAL],
        );
    });
    assert_eq!(
        interchange(&interior_waw, &environment, LOAD_A, SUM, LOAD_C, DIFF).unwrap_err(),
        CommutingRunInterchangeError::UnsupportedPair
    );
    // The later run's tail becomes a flag reader and the earlier run's
    // tail a flag publisher: the condition-state RAW refuses identically.
    let flags = mutated(target, |function, environment| {
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
        interchange(&flags, &environment, LOAD_A, SUM, LOAD_C, DIFF).unwrap_err(),
        CommutingRunInterchangeError::UnsupportedPair
    );
}

/// A boundary settlement inside the window's span observes a different
/// executed prefix once the runs trade places and refuses; a settlement at
/// the window's own first index or past its end sees the same executed set
/// and admits.
#[test]
fn interior_settlements_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let settlement = |position: u32| SelectedBoundarySettlement {
        block: SelectedBlockId(0),
        instruction_index: position,
        settlement: SelectedBoundarySettlementPayload::HostedWriteByteI32 {
            operation: OperationId::new(51).unwrap(),
            boundary: BoundaryMachineId::new(1).unwrap(),
            source: ValueId::new(9).unwrap(),
        },
    };
    for position in [2, 3, 4, 5] {
        let source = mutated(target, |function, _| {
            function.boundary_settlements.push(settlement(position));
        });
        assert_eq!(
            interchange(&source, &environment, LOAD_A, SUM, LOAD_C, DIFF).unwrap_err(),
            CommutingRunInterchangeError::UnsupportedPair,
            "settlement at {position}"
        );
    }
    let outside = mutated(target, |function, _| {
        function.boundary_settlements.push(settlement(1));
        function.boundary_settlements.push(settlement(6));
    });
    interchange(&outside, &environment, LOAD_A, SUM, LOAD_C, DIFF).unwrap();
}

/// A memory-capable kind without a roster row is an unaccounted access
/// anywhere in the window; barrier kinds and call-roster entries never
/// trade order.
#[test]
fn unaccounted_and_barrier_positions_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for position in [1, 3, 4] {
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
                &[POINTER, FIRST],
            );
            // The kind's reach is now unaccounted: drop its roster rows.
            function
                .memory_accesses
                .retain(|access| access.instruction != id);
        });
        assert_eq!(
            interchange(&bare, &environment, LOAD_A, SUM, LOAD_C, DIFF).unwrap_err(),
            CommutingRunInterchangeError::UnsupportedInstruction,
            "position {position}"
        );
    }
    for kind in [
        SelectedInstructionKind::Jump,
        SelectedInstructionKind::CallUnit {
            callee: MachineId::new(9).unwrap(),
        },
        SelectedInstructionKind::HostedExitProcessI32,
    ] {
        let source = mutated(target, |function, _| {
            function.blocks[0].instructions[3].kind = kind;
        });
        assert_eq!(
            interchange(&source, &environment, LOAD_A, SUM, LOAD_C, DIFF).unwrap_err(),
            CommutingRunInterchangeError::UnsupportedInstruction,
            "kind {kind:?}"
        );
    }
    let call_row =
        mutated(target, |function, _| {
            function.calls.push(SelectedCallContract {
            instruction: SEP,
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
        interchange(&call_row, &environment, LOAD_A, SUM, LOAD_C, DIFF).unwrap_err(),
        CommutingRunInterchangeError::UnsupportedInstruction
    );
}

/// A source the admission cannot locate — an absent earlier bound, a
/// missing function, a different target — reports its own reason, and an
/// exhausted work budget reports `WorkBudgetExceeded`.
#[test]
fn admission_reports_its_own_reasons() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    assert_eq!(
        interchange(&source, &environment, TERMINAL, SUM, LOAD_C, DIFF).unwrap_err(),
        CommutingRunInterchangeError::SourceMismatch
    );
    assert_eq!(
        interchange_selected_commuting_runs(
            &source,
            7,
            LOAD_A,
            SUM,
            LOAD_C,
            DIFF,
            &environment,
            budget()
        )
        .unwrap_err(),
        CommutingRunInterchangeError::SourceMismatch
    );
    let arm64 = baseline_target_register_environment(NativeTarget::linux_arm64()).unwrap();
    assert_eq!(
        interchange_selected_commuting_runs(
            &source,
            0,
            LOAD_A,
            SUM,
            LOAD_C,
            DIFF,
            &arm64,
            budget()
        )
        .unwrap_err(),
        CommutingRunInterchangeError::SourceMismatch
    );
    let tight = OptimizationWorkBudget::new(1, 1, 1, 1, 1).unwrap();
    assert_eq!(
        interchange_selected_commuting_runs(
            &source,
            0,
            LOAD_A,
            SUM,
            LOAD_C,
            DIFF,
            &environment,
            tight
        )
        .unwrap_err(),
        CommutingRunInterchangeError::WorkBudgetExceeded
    );
}

/// Replay independently re-derives the interchange: the proposed program
/// validates again through the public validator, an unpermuted roster is
/// rejected as not the interchange admission proved, an unreordered body
/// is rejected, and any unrelated mutation fails the restore-by-content
/// comparison.
#[test]
fn replay_restores_the_source_by_content() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let result = interchange(&source, &environment, LOAD_A, SUM, LOAD_C, DIFF).unwrap();
    let reproposed = validate_commuting_run_interchange(
        &source,
        0,
        LOAD_A,
        SUM,
        LOAD_C,
        DIFF,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    assert_eq!(reproposed.receipt(), result.receipt());
    // Instructions reordered but the roster left in source order is not
    // the interchange: the recorded accesses must follow the new execution
    // order.
    let mut stale_roster = result.transformed().clone();
    stale_roster.functions[0].memory_accesses =
        source.transformed().functions[0].memory_accesses.clone();
    assert_eq!(
        validate_commuting_run_interchange(
            &source,
            0,
            LOAD_A,
            SUM,
            LOAD_C,
            DIFF,
            &environment,
            budget(),
            stale_roster
        )
        .unwrap_err(),
        CommutingRunInterchangeError::ReplayMismatch
    );
    // Roster permuted but instructions left in source order.
    let mut stale_body = source.transformed().clone();
    stale_body.functions[0].memory_accesses =
        result.transformed().functions[0].memory_accesses.clone();
    assert_eq!(
        validate_commuting_run_interchange(
            &source,
            0,
            LOAD_A,
            SUM,
            LOAD_C,
            DIFF,
            &environment,
            budget(),
            stale_body
        )
        .unwrap_err(),
        CommutingRunInterchangeError::ReplayMismatch
    );
    // Any unrelated mutation — here a third roster row — fails the
    // restore-by-content comparison.
    let mut extra = result.transformed().clone();
    extra.functions[0].memory_accesses.push(access(
        MAT_Z,
        3,
        SelectedMemoryAccessRole::ReadPlace,
        0,
        8,
    ));
    assert_eq!(
        validate_commuting_run_interchange(
            &source,
            0,
            LOAD_A,
            SUM,
            LOAD_C,
            DIFF,
            &environment,
            budget(),
            extra
        )
        .unwrap_err(),
        CommutingRunInterchangeError::ReplayMismatch
    );
}

/// Adjacent runs are the empty-interior case: the two runs trade places
/// directly while their commuting rows still permute in the roster.
#[test]
fn adjacent_runs_interchange() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        function.blocks[0].instructions.remove(3);
    });
    let result = interchange(&source, &environment, LOAD_A, SUM, LOAD_C, DIFF).unwrap();
    let body = &result.transformed().functions[0].blocks[0].instructions;
    assert_eq!(
        body.iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![HEAD, LOAD_C, DIFF, LOAD_A, SUM, MAT_Z]
    );
    let roster = &result.transformed().functions[0].memory_accesses;
    assert_eq!(roster[0].instruction, LOAD_C);
    assert_eq!(roster[1].instruction, LOAD_A);
}

/// Runs of different lengths interchange under the same rule: the earlier
/// run's three members take the later pair's span and the interior keeps
/// its relative order, shifted by the one-instruction difference, while
/// the rowed trades keep commuting pairwise.
#[test]
fn unequal_runs_interchange_with_the_interior_shifted() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let result = interchange(&source, &environment, HEAD, SUM, LOAD_C, DIFF).unwrap();
    let body = &result.transformed().functions[0].blocks[0].instructions;
    assert_eq!(
        body.iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![LOAD_C, DIFF, SEP, HEAD, LOAD_A, SUM, MAT_Z]
    );
    validate_commuting_run_interchange(
        &source,
        0,
        HEAD,
        SUM,
        LOAD_C,
        DIFF,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
}

/// The admission walk is bounded by the validation budget: a plan whose
/// scan cost exceeds it refuses rather than running unbounded.
#[test]
fn work_budget_bounds_the_scan() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    assert_eq!(
        interchange_selected_commuting_runs(
            &source,
            0,
            LOAD_A,
            SUM,
            LOAD_C,
            DIFF,
            &environment,
            OptimizationWorkBudget::new(100, 100, 1, 100, 100).unwrap(),
        )
        .unwrap_err(),
        CommutingRunInterchangeError::WorkBudgetExceeded
    );
}

/// The measured validation-step boundary: admission charges one step per
/// block plus one per instruction across the plan, then the member and
/// crossed surfaces plus the roster-row product for each
/// member-against-crossed pair outside the member's own run, then the
/// admitted function's memory, call, and settlement roster lengths — so
/// the exact count admits the interchange on both the proposal and the
/// independent replay path while one step below rejects both.
#[test]
fn measured_validation_step_boundary_admits_and_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The window moves to a second block behind a one-instruction entry:
    // the plan scan grows by the entry block's body instruction and
    // terminator.
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
            &[FIFTH],
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
    // A boundary settlement ahead of the window's first member observes
    // the same executed prefix on either order, so it admits — and
    // charges the settlement roster term.
    let settled = mutated(target, |function, _| {
        function.boundary_settlements.push(settlement(0, 71));
    });
    for (source, exact_steps) in [
        // (7 instructions + 1 block) + member-against-crossed surfaces
        // (each earlier-run member against the interior and both later-run
        // members, each later-run member against both earlier-run members
        // and the interior: load 2, add 3, materialize 1 surfaces — 13 +
        // 15 + 13 + 15, including the load rows' product counted once per
        // direction) + 2 roster rows = 66.
        (fixture(target), 66u64),
        // (8 instructions + 1 per block over 2 blocks) + the same 56 + 2 =
        // 68.
        (later_block, 68u64),
        // The base charge plus one settlement roster row = 67.
        (settled, 67u64),
    ] {
        let exact = OptimizationWorkBudget::new(1, 1, exact_steps, 1, 1).unwrap();
        let result = interchange_selected_commuting_runs(
            &source,
            0,
            LOAD_A,
            SUM,
            LOAD_C,
            DIFF,
            &environment,
            exact,
        )
        .unwrap();
        validate_commuting_run_interchange(
            &source,
            0,
            LOAD_A,
            SUM,
            LOAD_C,
            DIFF,
            &environment,
            exact,
            result.transformed().clone(),
        )
        .unwrap();
        let starved = OptimizationWorkBudget::new(1, 1, exact_steps - 1, 1, 1).unwrap();
        assert_eq!(
            interchange_selected_commuting_runs(
                &source,
                0,
                LOAD_A,
                SUM,
                LOAD_C,
                DIFF,
                &environment,
                starved
            )
            .unwrap_err(),
            CommutingRunInterchangeError::WorkBudgetExceeded
        );
        assert_eq!(
            validate_commuting_run_interchange(
                &source,
                0,
                LOAD_A,
                SUM,
                LOAD_C,
                DIFF,
                &environment,
                starved,
                result.transformed().clone(),
            )
            .unwrap_err(),
            CommutingRunInterchangeError::WorkBudgetExceeded
        );
    }
}

/// Two interchanges over the identical source produce the identical
/// validated result, and the published plan is a legal second input
/// through the sealed analysis boundary: on the transformed plan the runs
/// have traded places, so the same names interchange them back to restore
/// the source plan bit-identically, while the stale run order still
/// declines.
#[test]
fn interchange_is_deterministic_and_re_admitted() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let first = interchange(&source, &environment, LOAD_A, SUM, LOAD_C, DIFF).unwrap();
    let second = interchange(&source, &environment, LOAD_A, SUM, LOAD_C, DIFF).unwrap();
    assert_eq!(first, second);
    // The validated output carries the sealed analysis boundary, so it is
    // a legal second input — not merely a reconstruction of one. On the
    // transformed plan the `LOAD_C; DIFF` run leads and `LOAD_A; SUM`
    // follows, so naming them in their new order admits the reverse
    // interchange back to the published source.
    let restored = interchange_selected_commuting_runs(
        &first,
        0,
        LOAD_C,
        DIFF,
        LOAD_A,
        SUM,
        &environment,
        budget(),
    )
    .unwrap();
    assert_eq!(restored.transformed(), source.transformed());
    assert_eq!(
        restored.receipt().transformed_selected(),
        source.selected_identity()
    );
    validate_commuting_run_interchange(
        &first,
        0,
        LOAD_C,
        DIFF,
        LOAD_A,
        SUM,
        &environment,
        budget(),
        restored.transformed().clone(),
    )
    .unwrap();
    // The stale run order declines on the second input: `LOAD_A; SUM` now
    // closes the window, so naming it as the earlier run leaves no later
    // run to trade with.
    assert_eq!(
        interchange_selected_commuting_runs(
            &first,
            0,
            LOAD_A,
            SUM,
            LOAD_C,
            DIFF,
            &environment,
            budget()
        )
        .unwrap_err(),
        CommutingRunInterchangeError::UnsupportedPair
    );
}

/// Replay drift beyond the admitted window still fails the
/// restore-by-content comparison: an instruction the interchange never
/// touched carries mutated content, so restoring the runs' order and the
/// window's rows cannot reproduce the source.
#[test]
fn replay_rejects_drift_outside_the_window() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let result = interchange(&source, &environment, LOAD_A, SUM, LOAD_C, DIFF).unwrap();
    let mut drifted = result.transformed().clone();
    drifted.functions[0].blocks[0].instructions[6].kind = SelectedInstructionKind::MaterializeI64 {
        value: IntegerValue::Unsigned(10),
    };
    assert_eq!(
        validate_commuting_run_interchange(
            &source,
            0,
            LOAD_A,
            SUM,
            LOAD_C,
            DIFF,
            &environment,
            budget(),
            drifted
        )
        .unwrap_err(),
        CommutingRunInterchangeError::ReplayMismatch
    );
}
