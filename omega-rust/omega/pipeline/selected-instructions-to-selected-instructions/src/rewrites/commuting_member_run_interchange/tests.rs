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
    CommutingMemberRunInterchangeError, CommutingMemberRunInterchangeReceipt,
    ValidatedCommutingMemberRunInterchange, interchange_selected_commuting_member_and_run,
    validate_commuting_member_run_interchange,
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

const MEMBER: SelectedInstructionId = SelectedInstructionId(2);
const MAT_A: SelectedInstructionId = SelectedInstructionId(3);
const SUM: SelectedInstructionId = SelectedInstructionId(4);
const SEP: SelectedInstructionId = SelectedInstructionId(5);
const LOAD_C: SelectedInstructionId = SelectedInstructionId(6);
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

/// A raw selected-stage unit fixture, not a source/Terminal admission claim:
/// `r6 = *r0+0; r1 = 5; r2 = r1 + r0; r5 = 9; r3 = *r0+8; r4 = r3 + r0;
/// r7 = 3; return`. The sides the tests interchange are the member
/// `MEMBER` at position 0 — an accounted load reading one place — and the
/// internally coupled run `LOAD_C; DIFF` at positions 4..=5, with `SEP`
/// the single interior position inside the coupled pair `MAT_A; SUM`,
/// `MAT_Z` behind the window, and `r0` an entry parameter so each load's
/// address operand and each sum's second read have no in-body producer.
/// The member's and the run head's roster rows read distinct places, so
/// the traded accesses commute.
fn fixture(target: NativeTarget) -> ValidatedCommutingMemberRunInterchange {
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
        register(FIRST, class, MAT_A, 2),
        register(TOTAL, class, SUM, 3),
        register(THIRD, class, LOAD_C, 4),
        register(FOURTH, class, DIFF, 5),
        register(FIFTH, class, SEP, 6),
        register(SIXTH, class, MEMBER, 7),
        register(SEVENTH, class, MAT_Z, 8),
    ];
    let instructions = vec![
        instruction(
            MEMBER,
            SelectedInstructionKind::Load8 { byte_offset: 0 },
            load,
            &[POINTER, SIXTH],
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
                access(MEMBER, 1, SelectedMemoryAccessRole::ReadPlace, 0, 8),
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
    ValidatedCommutingMemberRunInterchange {
        transformed: std::sync::Arc::new(plan),
        receipt: CommutingMemberRunInterchangeReceipt {
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
) -> ValidatedCommutingMemberRunInterchange {
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
    source: &ValidatedCommutingMemberRunInterchange,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    member: SelectedInstructionId,
    run_first: SelectedInstructionId,
    run_last: SelectedInstructionId,
) -> Result<ValidatedCommutingMemberRunInterchange, CommutingMemberRunInterchangeError> {
    interchange_selected_commuting_member_and_run(
        source,
        0,
        member,
        run_first,
        run_last,
        environment,
        budget(),
    )
}

/// An accounted member before an internally coupled run trades places
/// around an inert interior on every target: each side's members keep
/// their identity, kind, operands, and provenance inside their own order,
/// the interior keeps its relative order shifted by one less than the
/// run's length, the roster's window rows follow the new execution order —
/// the run's row, then the member's — and the receipt binds both
/// identities.
#[test]
fn commuting_member_and_run_interchange_on_every_target() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = fixture(target);
        let result = interchange(&source, &environment, MEMBER, LOAD_C, DIFF).unwrap();
        let original = &source.transformed().functions[0].blocks[0].instructions;
        let body = &result.transformed().functions[0].blocks[0].instructions;
        assert_eq!(body.len(), original.len());
        assert_eq!(
            body.iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![LOAD_C, DIFF, MAT_A, SUM, SEP, MEMBER, MAT_Z]
        );
        assert_eq!(body[0], original[4]);
        assert_eq!(body[1], original[5]);
        assert_eq!(body[2], original[1]);
        assert_eq!(body[3], original[2]);
        assert_eq!(body[4], original[3]);
        assert_eq!(body[5], original[0]);
        // The roster followed the new execution order: LOAD_C's row now
        // leads the window and MEMBER's closes it, each row itself
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
        validate_commuting_member_run_interchange(
            &source,
            0,
            MEMBER,
            LOAD_C,
            DIFF,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
        // A detached, separately allocated proposal replays by content.
        let mut detached = result.transformed().clone();
        detached.functions = detached.functions.iter().cloned().collect();
        validate_commuting_member_run_interchange(
            &source,
            0,
            MEMBER,
            LOAD_C,
            DIFF,
            &environment,
            budget(),
            detached,
        )
        .unwrap();
    }
}

/// The member may sit after the run: `MAT_Z`, itself an accounted load,
/// follows the `MAT_A; SUM` run and takes its leading edge while the run
/// takes the member's position — and the member's row trades against the
/// interior's accounted load on the way.
#[test]
fn member_later_interchanges_onto_the_runs_span() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[6] = instruction(
            MAT_Z,
            SelectedInstructionKind::Load8 { byte_offset: 16 },
            &load,
            &[POINTER, SEVENTH],
        );
        function
            .memory_accesses
            .push(access(MAT_Z, 3, SelectedMemoryAccessRole::ReadPlace, 16, 8));
    });
    let result = interchange(&source, &environment, MAT_Z, MAT_A, SUM).unwrap();
    let body = &result.transformed().functions[0].blocks[0].instructions;
    assert_eq!(
        body.iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![MEMBER, MAT_Z, SEP, LOAD_C, DIFF, MAT_A, SUM]
    );
    // The window's rows followed the move: the member's row leads, the
    // interior's kept its order inside the window, and MEMBER's own row
    // outside the window never moved.
    let roster = &result.transformed().functions[0].memory_accesses;
    assert_eq!(roster[0].instruction, MEMBER);
    assert_eq!(roster[1].instruction, MAT_Z);
    assert_eq!(roster[2].instruction, LOAD_C);
    validate_commuting_member_run_interchange(
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

/// The family's accounting rule admits every commuting row shape the
/// member-against-run interchange refused: an accounted member against an
/// accounted run on disjoint extents of one place, a run whose store
/// stages into a boundary slot no place shares, a dynamic-extent read
/// against a write on a different place, a second rostered member inside
/// the run keeping the recorded order, and a rowed interior whose own
/// read commutes with both sides' rows.
#[test]
fn commuting_access_shapes_admit() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Two writes on disjoint fixed extents of one place: the member and
    // the run's head store disjoint halves of the same place, so neither
    // observes the other's bytes.
    let disjoint = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[0] = instruction(
            MEMBER,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            &store,
            &[POINTER, SIXTH],
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
            access(MEMBER, 1, SelectedMemoryAccessRole::WritePlace, 0, 8),
            access(LOAD_C, 1, SelectedMemoryAccessRole::WritePlace, 24, 8),
        ];
    });
    let result = interchange(&disjoint, &environment, MEMBER, LOAD_C, DIFF).unwrap();
    assert_eq!(
        result.transformed().functions[0].memory_accesses[0].instruction,
        LOAD_C
    );
    // A write into a boundary slot is storage distinct from any place: the
    // slot is never a place's own storage, so the staged bytes never reach
    // the place the member reads.
    let staged = mutated(target, |function, _| {
        function.memory_accesses = vec![
            access(MEMBER, 1, SelectedMemoryAccessRole::ReadPlace, 0, 8),
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
    interchange(&staged, &environment, MEMBER, LOAD_C, DIFF).unwrap();
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
                MEMBER,
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
    interchange(&span, &environment, MEMBER, LOAD_C, DIFF).unwrap();
    // Two rostered members inside the run keep their recorded order — the
    // run's rows move as one body — while each trades against the member's
    // own row.
    let rowed_run = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[5] = instruction(
            DIFF,
            SelectedInstructionKind::Load8 { byte_offset: 16 },
            &load,
            &[POINTER, FOURTH],
        );
        function
            .memory_accesses
            .push(access(DIFF, 4, SelectedMemoryAccessRole::ReadPlace, 16, 8));
    });
    let result = interchange(&rowed_run, &environment, MEMBER, LOAD_C, DIFF).unwrap();
    let roster = &result.transformed().functions[0].memory_accesses;
    assert_eq!(roster[0].instruction, LOAD_C);
    assert_eq!(roster[1].instruction, DIFF);
    assert_eq!(roster[2].instruction, MEMBER);
    // A rowed interior position joins the commutation audit: its read of a
    // third place trades order with both sides and commutes with each.
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
            access(MEMBER, 1, SelectedMemoryAccessRole::ReadPlace, 0, 8),
            access(SEP, 3, SelectedMemoryAccessRole::ReadPlace, 16, 8),
            access(LOAD_C, 2, SelectedMemoryAccessRole::ReadPlace, 8, 8),
        ];
    });
    let result = interchange(&rowed_interior, &environment, MEMBER, LOAD_C, DIFF).unwrap();
    let roster = &result.transformed().functions[0].memory_accesses;
    assert_eq!(roster[0].instruction, LOAD_C);
    assert_eq!(roster[1].instruction, SEP);
    assert_eq!(roster[2].instruction, MEMBER);
}

/// The commutation bound: a write whose reach cannot be bounded away from
/// a crossed access refuses — overlapping fixed extents on one place
/// between the sides, two writes on a shared extent, a dynamic-extent span
/// on the shared place, and a rowed interior position whose read conflicts
/// with the member's write. The traded order of conflicting accesses is
/// never speculative.
#[test]
fn noncommuting_accesses_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Overlapping fixed extents on one place, the member reading and the
    // run's head writing.
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
            access(MEMBER, 1, SelectedMemoryAccessRole::ReadPlace, 0, 8),
            access(LOAD_C, 1, SelectedMemoryAccessRole::WritePlace, 4, 8),
        ];
    });
    assert_eq!(
        interchange(&overlapping, &environment, MEMBER, LOAD_C, DIFF).unwrap_err(),
        CommutingMemberRunInterchangeError::UnsupportedPair
    );
    // Two writes on one place's shared extent: either order changes the
    // bytes both leave behind.
    let writes = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[0] = instruction(
            MEMBER,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            &store,
            &[POINTER, SIXTH],
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
            access(MEMBER, 1, SelectedMemoryAccessRole::WritePlace, 0, 16),
            access(LOAD_C, 1, SelectedMemoryAccessRole::WritePlace, 8, 8),
        ];
    });
    assert_eq!(
        interchange(&writes, &environment, MEMBER, LOAD_C, DIFF).unwrap_err(),
        CommutingMemberRunInterchangeError::UnsupportedPair
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
                MEMBER,
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
        interchange(&span, &environment, MEMBER, LOAD_C, DIFF).unwrap_err(),
        CommutingMemberRunInterchangeError::UnsupportedPair
    );
    // A member's row against an accounted interior position refuses under
    // the same rule: the write trades order with the interior's read of
    // the same bytes.
    let interior = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap()
            .clone();
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[0] = instruction(
            MEMBER,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            &store,
            &[POINTER, SIXTH],
        );
        function.blocks[0].instructions[3] = instruction(
            SEP,
            SelectedInstructionKind::Load8 { byte_offset: 0 },
            &load,
            &[POINTER, FIFTH],
        );
        function.memory_accesses = vec![
            access(MEMBER, 1, SelectedMemoryAccessRole::WritePlace, 0, 8),
            access(SEP, 1, SelectedMemoryAccessRole::ReadPlace, 0, 8),
            access(LOAD_C, 2, SelectedMemoryAccessRole::ReadPlace, 8, 8),
        ];
    });
    assert_eq!(
        interchange(&interior, &environment, MEMBER, LOAD_C, DIFF).unwrap_err(),
        CommutingMemberRunInterchangeError::UnsupportedPair
    );
}

/// The member-against-run interchange's own accounting cases stay with
/// it: a window whose trading pairs carry no rowed-vs-rowed trade — no
/// rows at all, rows only on one side, or a rowed interior alone against
/// memory-inert sides — is `UnsupportedPair` here even though the
/// commutation audit is vacuously satisfied, and the plain
/// member-against-run family admits each.
#[test]
fn single_actor_windows_belong_to_the_member_run_interchange() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // No rows at all: the plain member-against-run case.
    let bare = mutated(target, |function, _| {
        function.memory_accesses = Vec::new();
    });
    assert_eq!(
        interchange(&bare, &environment, MEMBER, LOAD_C, DIFF).unwrap_err(),
        CommutingMemberRunInterchangeError::UnsupportedPair
    );
    crate::interchange_selected_member_and_run(
        &bare,
        0,
        MEMBER,
        LOAD_C,
        DIFF,
        &environment,
        budget(),
    )
    .unwrap();
    // Rows on one side only: the single-actor case.
    let one_side = mutated(target, |function, _| {
        function
            .memory_accesses
            .retain(|access| access.instruction == MEMBER);
    });
    assert_eq!(
        interchange(&one_side, &environment, MEMBER, LOAD_C, DIFF).unwrap_err(),
        CommutingMemberRunInterchangeError::UnsupportedPair
    );
    crate::interchange_selected_member_and_run(
        &one_side,
        0,
        MEMBER,
        LOAD_C,
        DIFF,
        &environment,
        budget(),
    )
    .unwrap();
    // A rowed interior alone admits in the plain family; here no trading
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
        interchange(&interior_only, &environment, MEMBER, LOAD_C, DIFF).unwrap_err(),
        CommutingMemberRunInterchangeError::UnsupportedPair
    );
    crate::interchange_selected_member_and_run(
        &interior_only,
        0,
        MEMBER,
        LOAD_C,
        DIFF,
        &environment,
        budget(),
    )
    .unwrap();
}

/// The sides must be one member and one disjoint multi-member run inside
/// one block: a member inside the run's span shares the side it trades
/// against, a one-member run is the commuting pair's granularity, the
/// reversed bounds name no ordered run, and an unknown or foreign-block id
/// never locates the window.
#[test]
fn only_a_disjoint_member_and_multi_member_run_interchange() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // A member inside the run's span shares the side it trades against.
    assert_eq!(
        interchange(&source, &environment, SEP, LOAD_C, DIFF).unwrap_err(),
        CommutingMemberRunInterchangeError::UnsupportedPair
    );
    // A member equal to a run bound names no disjoint side.
    assert_eq!(
        interchange(&source, &environment, LOAD_C, LOAD_C, DIFF).unwrap_err(),
        CommutingMemberRunInterchangeError::UnsupportedPair
    );
    // A one-member run is the commuting pair's granularity.
    assert_eq!(
        interchange(&source, &environment, MEMBER, LOAD_C, LOAD_C).unwrap_err(),
        CommutingMemberRunInterchangeError::UnsupportedPair
    );
    // The reversed bounds name no ordered run.
    assert_eq!(
        interchange(&source, &environment, MEMBER, DIFF, LOAD_C).unwrap_err(),
        CommutingMemberRunInterchangeError::UnsupportedPair
    );
    // An unknown member never locates the window; an unknown run bound
    // names no run.
    assert_eq!(
        interchange(
            &source,
            &environment,
            SelectedInstructionId(99),
            LOAD_C,
            DIFF
        )
        .unwrap_err(),
        CommutingMemberRunInterchangeError::SourceMismatch
    );
    assert_eq!(
        interchange(
            &source,
            &environment,
            MEMBER,
            SelectedInstructionId(99),
            DIFF
        )
        .unwrap_err(),
        CommutingMemberRunInterchangeError::UnsupportedPair
    );
    assert_eq!(
        interchange(
            &source,
            &environment,
            MEMBER,
            LOAD_C,
            SelectedInstructionId(99)
        )
        .unwrap_err(),
        CommutingMemberRunInterchangeError::UnsupportedPair
    );
    // A terminator-carried instruction is not a body member.
    assert_eq!(
        interchange(&source, &environment, TERMINAL, LOAD_C, DIFF).unwrap_err(),
        CommutingMemberRunInterchangeError::SourceMismatch
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
        interchange(&split, &environment, MEMBER, LOAD_C, DIFF).unwrap_err(),
        CommutingMemberRunInterchangeError::UnsupportedPair
    );
    // A wrong function index never locates the sides.
    assert_eq!(
        interchange_selected_commuting_member_and_run(
            &source,
            1,
            MEMBER,
            LOAD_C,
            DIFF,
            &environment,
            budget()
        )
        .unwrap_err(),
        CommutingMemberRunInterchangeError::SourceMismatch
    );
}

/// Register and condition-state hazards keep the member-against-run
/// interchange's audit unchanged: a crossed position writing a register a
/// member reads refuses the trade, in either direction across the sides
/// and through the interior.
#[test]
fn register_hazards_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // DIFF's first read moves to SIXTH — the member defines it, so the
    // sides cannot trade.
    let raw = mutated(target, |function, _| {
        function.blocks[0].instructions[5].operands[0].virtual_register = SIXTH;
    });
    assert_eq!(
        interchange(&raw, &environment, MEMBER, LOAD_C, DIFF).unwrap_err(),
        CommutingMemberRunInterchangeError::UnsupportedPair
    );
    // The run's head overwrites POINTER, which the member's load reads.
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
        interchange(&war, &environment, MEMBER, LOAD_C, DIFF).unwrap_err(),
        CommutingMemberRunInterchangeError::UnsupportedPair
    );
    // The interior instruction writes TOTAL, which the interior's own sum
    // defines — a crossed WAW between the interior positions trades order
    // with no one, but SEP writing SIXTH collides with the member's own
    // definition: a crossed WAW against the member.
    let interior_waw = mutated(target, |function, environment| {
        let add = environment
            .constraint(environment.selected_keys().add_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[3] = instruction(
            SEP,
            SelectedInstructionKind::WrappingAddI64,
            &add,
            &[FIRST, POINTER, SIXTH],
        );
    });
    assert_eq!(
        interchange(&interior_waw, &environment, MEMBER, LOAD_C, DIFF).unwrap_err(),
        CommutingMemberRunInterchangeError::UnsupportedPair
    );
    // The run's tail becomes a flag reader and the member a flag
    // publisher: the condition-state RAW refuses identically.
    let flags = mutated(target, |function, environment| {
        let keys = environment.selected_keys();
        let compare = environment.constraint(keys.compare_i64).unwrap().clone();
        let boolean = environment
            .constraint(keys.materialize_boolean)
            .unwrap()
            .clone();
        function.blocks[0].instructions[0] = instruction(
            MEMBER,
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
        interchange(&flags, &environment, MEMBER, LOAD_C, DIFF).unwrap_err(),
        CommutingMemberRunInterchangeError::UnsupportedPair
    );
}

/// A boundary settlement inside the window's span observes a different
/// executed prefix once the sides trade places and refuses; a settlement
/// at the window's own first index or past its end sees the same executed
/// set and admits — in either direction of the member.
#[test]
fn interior_settlements_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Member earlier: the window is positions 0..=5.
    for position in 1..=5 {
        let settled = mutated(target, |function, _| {
            function.boundary_settlements.push(settlement(position, 41));
        });
        assert_eq!(
            interchange(&settled, &environment, MEMBER, LOAD_C, DIFF).unwrap_err(),
            CommutingMemberRunInterchangeError::UnsupportedPair,
            "settlement at {position}"
        );
    }
    for position in [0, 6] {
        let settled = mutated(target, |function, _| {
            function.boundary_settlements.push(settlement(position, 41));
        });
        interchange(&settled, &environment, MEMBER, LOAD_C, DIFF).unwrap();
    }
    // Member later: the member becomes an accounted load at position 6 and
    // the run is `MAT_A; SUM`, so the window is positions 1..=6.
    let member_later = |position: u32| {
        mutated(target, |function, environment| {
            let load = environment
                .constraint(environment.selected_keys().load8.unwrap())
                .unwrap()
                .clone();
            function.blocks[0].instructions[6] = instruction(
                MAT_Z,
                SelectedInstructionKind::Load8 { byte_offset: 16 },
                &load,
                &[POINTER, SEVENTH],
            );
            function.memory_accesses.push(access(
                MAT_Z,
                3,
                SelectedMemoryAccessRole::ReadPlace,
                16,
                8,
            ));
            function.boundary_settlements.push(settlement(position, 41));
        })
    };
    for position in 2..=6 {
        assert_eq!(
            interchange(&member_later(position), &environment, MAT_Z, MAT_A, SUM).unwrap_err(),
            CommutingMemberRunInterchangeError::UnsupportedPair,
            "settlement at {position}"
        );
    }
    for position in [0, 1] {
        interchange(&member_later(position), &environment, MAT_Z, MAT_A, SUM).unwrap();
    }
}

/// A memory-capable kind without a roster row is an unaccounted access
/// anywhere in the window; barrier kinds and call-roster entries never
/// trade order.
#[test]
fn unaccounted_and_barrier_positions_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for position in [0, 3, 4] {
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
            // The kind's reach is now unaccounted: drop its roster rows.
            function
                .memory_accesses
                .retain(|access| access.instruction != id);
        });
        assert_eq!(
            interchange(&bare, &environment, MEMBER, LOAD_C, DIFF).unwrap_err(),
            CommutingMemberRunInterchangeError::UnsupportedInstruction,
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
            interchange(&source, &environment, MEMBER, LOAD_C, DIFF).unwrap_err(),
            CommutingMemberRunInterchangeError::UnsupportedInstruction,
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
        interchange(&call_row, &environment, MEMBER, LOAD_C, DIFF).unwrap_err(),
        CommutingMemberRunInterchangeError::UnsupportedInstruction
    );
}

/// A source the admission cannot locate — an absent member, a missing
/// function, a different target — reports its own reason, and an
/// exhausted work budget reports `WorkBudgetExceeded`.
#[test]
fn admission_reports_its_own_reasons() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    assert_eq!(
        interchange(&source, &environment, TERMINAL, LOAD_C, DIFF).unwrap_err(),
        CommutingMemberRunInterchangeError::SourceMismatch
    );
    assert_eq!(
        interchange_selected_commuting_member_and_run(
            &source,
            7,
            MEMBER,
            LOAD_C,
            DIFF,
            &environment,
            budget()
        )
        .unwrap_err(),
        CommutingMemberRunInterchangeError::SourceMismatch
    );
    let arm64 = baseline_target_register_environment(NativeTarget::linux_arm64()).unwrap();
    assert_eq!(
        interchange_selected_commuting_member_and_run(
            &source,
            0,
            MEMBER,
            LOAD_C,
            DIFF,
            &arm64,
            budget()
        )
        .unwrap_err(),
        CommutingMemberRunInterchangeError::SourceMismatch
    );
    let tight = OptimizationWorkBudget::new(1, 1, 1, 1, 1).unwrap();
    assert_eq!(
        interchange_selected_commuting_member_and_run(
            &source,
            0,
            MEMBER,
            LOAD_C,
            DIFF,
            &environment,
            tight
        )
        .unwrap_err(),
        CommutingMemberRunInterchangeError::WorkBudgetExceeded
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
    let result = interchange(&source, &environment, MEMBER, LOAD_C, DIFF).unwrap();
    let reproposed = validate_commuting_member_run_interchange(
        &source,
        0,
        MEMBER,
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
        validate_commuting_member_run_interchange(
            &source,
            0,
            MEMBER,
            LOAD_C,
            DIFF,
            &environment,
            budget(),
            stale_roster
        )
        .unwrap_err(),
        CommutingMemberRunInterchangeError::ReplayMismatch
    );
    // Roster permuted but instructions left in source order.
    let mut stale_body = source.transformed().clone();
    stale_body.functions[0].memory_accesses =
        result.transformed().functions[0].memory_accesses.clone();
    assert_eq!(
        validate_commuting_member_run_interchange(
            &source,
            0,
            MEMBER,
            LOAD_C,
            DIFF,
            &environment,
            budget(),
            stale_body
        )
        .unwrap_err(),
        CommutingMemberRunInterchangeError::ReplayMismatch
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
        validate_commuting_member_run_interchange(
            &source,
            0,
            MEMBER,
            LOAD_C,
            DIFF,
            &environment,
            budget(),
            extra
        )
        .unwrap_err(),
        CommutingMemberRunInterchangeError::ReplayMismatch
    );
}

/// A member adjacent to the run is the empty-interior case on each side:
/// the two sides trade places directly while their commuting rows still
/// permute in the roster.
#[test]
fn adjacent_member_and_run_interchange() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Member earlier and adjacent: the run's head must carry a row for the
    // commutation audit to see a rowed trade, so `MAT_A` becomes an
    // accounted load itself.
    let earlier = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[1] = instruction(
            MAT_A,
            SelectedInstructionKind::Load8 { byte_offset: 24 },
            &load,
            &[POINTER, FIRST],
        );
        function
            .memory_accesses
            .push(access(MAT_A, 4, SelectedMemoryAccessRole::ReadPlace, 24, 8));
    });
    let result = interchange(&earlier, &environment, MEMBER, MAT_A, SUM).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![MAT_A, SUM, MEMBER, SEP, LOAD_C, DIFF, MAT_Z]
    );
    // Member later and adjacent: `MAT_Z` as an accounted load trades
    // directly against the `LOAD_C; DIFF` run.
    let later = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[6] = instruction(
            MAT_Z,
            SelectedInstructionKind::Load8 { byte_offset: 16 },
            &load,
            &[POINTER, SEVENTH],
        );
        function
            .memory_accesses
            .push(access(MAT_Z, 3, SelectedMemoryAccessRole::ReadPlace, 16, 8));
    });
    let result = interchange(&later, &environment, MAT_Z, LOAD_C, DIFF).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![MEMBER, MAT_A, SUM, SEP, MAT_Z, LOAD_C, DIFF]
    );
    let roster = &result.transformed().functions[0].memory_accesses;
    assert_eq!(roster[1].instruction, MAT_Z);
    assert_eq!(roster[2].instruction, LOAD_C);
}

/// The admission walk is bounded by the validation budget: a plan whose
/// scan cost exceeds it refuses rather than running unbounded.
#[test]
fn work_budget_bounds_the_scan() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    assert_eq!(
        interchange_selected_commuting_member_and_run(
            &source,
            0,
            MEMBER,
            LOAD_C,
            DIFF,
            &environment,
            OptimizationWorkBudget::new(100, 100, 1, 100, 100).unwrap(),
        )
        .unwrap_err(),
        CommutingMemberRunInterchangeError::WorkBudgetExceeded
    );
}

/// The measured validation-step boundary: admission charges one step per
/// block plus one per instruction across the plan, then one per operand,
/// implicit use, implicit definition, and clobber row on each
/// member-against-crossed pair plus the row product of each pair, then the
/// admitted function's memory, call, and settlement roster lengths — so
/// the exact count admits the interchange on both the proposal and the
/// independent replay path while one step below rejects both.
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
    for (source, exact_steps) in [
        // (7 instructions + 1 per block) + member-against-crossed surfaces
        // (the member against the 3 interior and 2 run positions, each run
        // member against the 3 interior and the member: 13 pairs — load 2,
        // materialize 1, add 3 surfaces — plus the member-against-run-head
        // row product counted twice) + 2 roster rows = 66.
        (fixture(target), 66u64),
        // (8 instructions + 1 per block over 2 blocks) + the same 56 + 2 =
        // 68.
        (later_block, 68u64),
    ] {
        let exact = OptimizationWorkBudget::new(1, 1, exact_steps, 1, 1).unwrap();
        let result = interchange_selected_commuting_member_and_run(
            &source,
            0,
            MEMBER,
            LOAD_C,
            DIFF,
            &environment,
            exact,
        )
        .unwrap();
        validate_commuting_member_run_interchange(
            &source,
            0,
            MEMBER,
            LOAD_C,
            DIFF,
            &environment,
            exact,
            result.transformed().clone(),
        )
        .unwrap();
        let starved = OptimizationWorkBudget::new(1, 1, exact_steps - 1, 1, 1).unwrap();
        assert_eq!(
            interchange_selected_commuting_member_and_run(
                &source,
                0,
                MEMBER,
                LOAD_C,
                DIFF,
                &environment,
                starved
            )
            .unwrap_err(),
            CommutingMemberRunInterchangeError::WorkBudgetExceeded
        );
        assert_eq!(
            validate_commuting_member_run_interchange(
                &source,
                0,
                MEMBER,
                LOAD_C,
                DIFF,
                &environment,
                starved,
                result.transformed().clone(),
            )
            .unwrap_err(),
            CommutingMemberRunInterchangeError::WorkBudgetExceeded
        );
    }
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
    let first = interchange(&source, &environment, MEMBER, LOAD_C, DIFF).unwrap();
    let second = interchange(&source, &environment, MEMBER, LOAD_C, DIFF).unwrap();
    assert_eq!(first, second);
    // The validated output carries the sealed analysis boundary, so it is
    // a legal second input — not merely a reconstruction of one. On the
    // transformed plan the member follows the run, so the same names admit
    // the member-later interchange back to the published source.
    let restored = interchange_selected_commuting_member_and_run(
        &first,
        0,
        MEMBER,
        LOAD_C,
        DIFF,
        &environment,
        budget(),
    )
    .unwrap();
    assert_eq!(restored.transformed(), source.transformed());
    assert_eq!(
        restored.receipt().transformed_selected(),
        source.selected_identity()
    );
    validate_commuting_member_run_interchange(
        &first,
        0,
        MEMBER,
        LOAD_C,
        DIFF,
        &environment,
        budget(),
        restored.transformed().clone(),
    )
    .unwrap();
    // A hazard-coupled proposal still declines on the second input: on the
    // transformed plan SUM sits at index 3 behind the run at 0..=1 with
    // MAT_A between them, so naming member SUM against run [LOAD_C, DIFF]
    // would move the sum ahead of the materialization whose result it
    // reads.
    assert_eq!(
        interchange_selected_commuting_member_and_run(
            &first,
            0,
            SUM,
            LOAD_C,
            DIFF,
            &environment,
            budget()
        )
        .unwrap_err(),
        CommutingMemberRunInterchangeError::UnsupportedPair
    );
}
