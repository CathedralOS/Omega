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
    CommutingInterchangeError, CommutingInterchangeReceipt, ValidatedCommutingInterchange,
    interchange_selected_commuting_pair, validate_commuting_interchange,
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
const MAT_B: SelectedInstructionId = SelectedInstructionId(3);
const LOAD_C: SelectedInstructionId = SelectedInstructionId(4);
const MAT_D: SelectedInstructionId = SelectedInstructionId(5);
const TERMINAL: SelectedInstructionId = SelectedInstructionId(6);

const POINTER: VirtualRegisterId = VirtualRegisterId(0);
const FIRST: VirtualRegisterId = VirtualRegisterId(1);
const SECOND: VirtualRegisterId = VirtualRegisterId(2);
const THIRD: VirtualRegisterId = VirtualRegisterId(3);
const FOURTH: VirtualRegisterId = VirtualRegisterId(4);

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
/// `r1 = *r0+0; r2 = 5; r3 = *r0+8; r4 = 9; return`. The named pair the
/// tests interchange are the two loads `LOAD_A` at position 0 and `LOAD_C`
/// at position 2 around the inert interior `MAT_B`, with `MAT_D` after the
/// window and `r0` an entry parameter so the loads' address operand has no
/// in-body producer. The two loads read distinct places.
fn fixture(target: NativeTarget) -> ValidatedCommutingInterchange {
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let materialize = environment.constraint(keys.materialize_i64).unwrap();
    let load = environment.constraint(keys.load8.unwrap()).unwrap();
    let terminal_row = environment.constraint(keys.return_unit).unwrap();
    let class = materialize.operands[0].class;
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
        register(SECOND, class, MAT_B, 3),
        register(THIRD, class, LOAD_C, 4),
        register(FOURTH, class, MAT_D, 5),
    ];
    let instructions = vec![
        instruction(
            LOAD_A,
            SelectedInstructionKind::Load8 { byte_offset: 0 },
            load,
            &[POINTER, FIRST],
        ),
        instruction(
            MAT_B,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(5),
            },
            materialize,
            &[SECOND],
        ),
        instruction(
            LOAD_C,
            SelectedInstructionKind::Load8 { byte_offset: 8 },
            load,
            &[POINTER, THIRD],
        ),
        instruction(
            MAT_D,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(9),
            },
            materialize,
            &[FOURTH],
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
    ValidatedCommutingInterchange {
        transformed: std::sync::Arc::new(plan),
        receipt: CommutingInterchangeReceipt {
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
) -> ValidatedCommutingInterchange {
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
    source: &ValidatedCommutingInterchange,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    earlier: SelectedInstructionId,
    later: SelectedInstructionId,
) -> Result<ValidatedCommutingInterchange, CommutingInterchangeError> {
    interchange_selected_commuting_pair(source, 0, earlier, later, environment, budget())
}

/// Two accounted loads on distinct places trade places around an inert
/// interior on every target: each member keeps its identity, kind,
/// operands, and provenance, the interior keeps its position, and the
/// roster's window rows follow the new execution order — the later load's
/// row, then the earlier load's row — while the receipt binds both
/// identities.
#[test]
fn commuting_pair_interchanges_on_every_target() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = fixture(target);
        let result = interchange(&source, &environment, LOAD_A, LOAD_C).unwrap();
        let original = &source.transformed().functions[0].blocks[0].instructions;
        let body = &result.transformed().functions[0].blocks[0].instructions;
        assert_eq!(
            body.iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![LOAD_C, MAT_B, LOAD_A, MAT_D]
        );
        assert_eq!(body[0], original[2]);
        assert_eq!(body[1], original[1]);
        assert_eq!(body[2], original[0]);
        assert_eq!(body[3], original[3]);
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
    }
}

/// The family's accounting rule admits every commuting row shape the pair
/// interchange refused: two reads of one place, a read against a write on
/// a disjoint extent of one place, a write into staging storage distinct
/// from any place, and a dynamic-extent read against a write on a
/// different place — each with the roster permuted into the new execution
/// order.
#[test]
fn commuting_access_shapes_admit() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Two reads of one place: neither changes what the other observes.
    let reads = mutated(target, |function, _| {
        function.memory_accesses = vec![
            access(LOAD_A, 1, SelectedMemoryAccessRole::ReadPlace, 0, 8),
            access(LOAD_C, 1, SelectedMemoryAccessRole::ReadPlace, 0, 8),
        ];
    });
    let result = interchange(&reads, &environment, LOAD_A, LOAD_C).unwrap();
    assert_eq!(
        result.transformed().functions[0].memory_accesses[0].instruction,
        LOAD_C
    );
    // Read against write, disjoint extents of one place.
    let disjoint = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[2] = instruction(
            LOAD_C,
            SelectedInstructionKind::Store {
                byte_offset: 24,
                byte_size: 8,
            },
            &store,
            &[POINTER, THIRD],
        );
        function.memory_accesses = vec![
            access(LOAD_A, 1, SelectedMemoryAccessRole::ReadPlace, 0, 8),
            access(LOAD_C, 1, SelectedMemoryAccessRole::WritePlace, 24, 8),
        ];
    });
    interchange(&disjoint, &environment, LOAD_A, LOAD_C).unwrap();
    // A write into a boundary slot is storage distinct from any place:
    // the slot is never a place's own storage, so the staged bytes never
    // reach the place's bytes.
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
    interchange(&staged, &environment, LOAD_A, LOAD_C).unwrap();
    // A dynamic-extent read commutes with a write whose reach it provably
    // never shares — here a different place entirely.
    let span = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[0] = instruction(
            LOAD_A,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            &store,
            &[POINTER, FIRST],
        );
        function.memory_accesses = vec![
            access(LOAD_A, 1, SelectedMemoryAccessRole::WritePlace, 0, 8),
            dynamic_access(
                LOAD_C,
                2,
                SelectedMemoryAccessRole::ReadByteSpan {
                    length: ValueId::new(11).unwrap(),
                    obligation: ObligationId::new(12).unwrap(),
                    accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                        [7; 32],
                    ),
                },
            ),
        ];
    });
    interchange(&span, &environment, LOAD_A, LOAD_C).unwrap();
}

/// The commutation bound: a write whose reach cannot be bounded away from
/// a crossed access refuses — same place with overlapping fixed extents,
/// two writes on a shared extent, and a dynamic-extent span on the shared
/// place whose reach no recorded extent bounds. The traded order of
/// conflicting accesses is never speculative.
#[test]
fn noncommuting_accesses_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Overlapping fixed extents on one place.
    let overlapping = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[2] = instruction(
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
        interchange(&overlapping, &environment, LOAD_A, LOAD_C).unwrap_err(),
        CommutingInterchangeError::UnsupportedPair
    );
    // Two writes on one place's shared extent: either order changes the
    // bytes both leave behind.
    let writes = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[0] = instruction(
            LOAD_A,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            &store,
            &[POINTER, FIRST],
        );
        function.blocks[0].instructions[2] = instruction(
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
        interchange(&writes, &environment, LOAD_A, LOAD_C).unwrap_err(),
        CommutingInterchangeError::UnsupportedPair
    );
    // A dynamic-extent span cannot be bounded away from the crossed write
    // on the same place even though its own recorded extent is empty.
    let span = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[2] = instruction(
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
        interchange(&span, &environment, LOAD_A, LOAD_C).unwrap_err(),
        CommutingInterchangeError::UnsupportedPair
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
            LOAD_A,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            &store,
            &[POINTER, FIRST],
        );
        function.blocks[0].instructions[1] = instruction(
            MAT_B,
            SelectedInstructionKind::Load8 { byte_offset: 16 },
            &load,
            &[POINTER, SECOND],
        );
        function.memory_accesses = vec![
            access(LOAD_A, 1, SelectedMemoryAccessRole::WritePlace, 0, 8),
            access(MAT_B, 1, SelectedMemoryAccessRole::ReadPlace, 0, 8),
            access(LOAD_C, 2, SelectedMemoryAccessRole::ReadPlace, 8, 8),
        ];
    });
    assert_eq!(
        interchange(&interior, &environment, LOAD_A, LOAD_C).unwrap_err(),
        CommutingInterchangeError::UnsupportedPair
    );
}

/// The pair interchange's own accounting cases stay with it: a window
/// whose trading pairs carry no rowed-vs-rowed trade — no rows at all, or
/// rows only on one side — is `UnsupportedPair` here even though the
/// commutation audit is vacuously satisfied.
#[test]
fn rowless_windows_belong_to_the_pair_interchange() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // No rows at all: the plain pair case.
    let bare = mutated(target, |function, _| {
        function.memory_accesses = Vec::new();
    });
    assert_eq!(
        interchange(&bare, &environment, LOAD_A, LOAD_C).unwrap_err(),
        CommutingInterchangeError::UnsupportedPair
    );
    // One rowed member against row-less positions: the single-actor case.
    let one_side = mutated(target, |function, _| {
        function.memory_accesses =
            vec![access(LOAD_A, 1, SelectedMemoryAccessRole::ReadPlace, 0, 8)];
    });
    assert_eq!(
        interchange(&one_side, &environment, LOAD_A, LOAD_C).unwrap_err(),
        CommutingInterchangeError::UnsupportedPair
    );
    // A rowed interior alone admits in the pair family; here no trading
    // pair is rowed on both sides either.
    let interior_only = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[1] = instruction(
            MAT_B,
            SelectedInstructionKind::Load8 { byte_offset: 16 },
            &load,
            &[POINTER, SECOND],
        );
        function.memory_accesses =
            vec![access(MAT_B, 1, SelectedMemoryAccessRole::ReadPlace, 16, 8)];
    });
    assert_eq!(
        interchange(&interior_only, &environment, LOAD_A, LOAD_C).unwrap_err(),
        CommutingInterchangeError::UnsupportedPair
    );
}

/// Register and condition-state hazards keep the pair interchange's audit
/// unchanged: a crossed position writing a register a member reads refuses
/// the trade.
#[test]
fn register_hazards_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The interior now defines POINTER, which both loads read as their
    // address: scheduling the later load ahead of it would observe the
    // wrong address.
    let hazard = mutated(target, |function, environment| {
        let add = environment
            .constraint(environment.selected_keys().add_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[1] = instruction(
            MAT_B,
            SelectedInstructionKind::WrappingAddI64,
            &add,
            &[SECOND, FIRST, POINTER],
        );
    });
    assert_eq!(
        interchange(&hazard, &environment, LOAD_A, LOAD_C).unwrap_err(),
        CommutingInterchangeError::UnsupportedPair
    );
}

/// A boundary settlement inside the window's span observes a different
/// executed prefix once the pair trades places and refuses; a settlement
/// at the window's own earlier index or past its end sees the same
/// executed set and admits.
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
    for position in [1, 2] {
        let source = mutated(target, |function, _| {
            function.boundary_settlements.push(settlement(position));
        });
        assert_eq!(
            interchange(&source, &environment, LOAD_A, LOAD_C).unwrap_err(),
            CommutingInterchangeError::UnsupportedPair,
            "settlement at {position}"
        );
    }
    let outside = mutated(target, |function, _| {
        function.boundary_settlements.push(settlement(0));
        function.boundary_settlements.push(settlement(3));
    });
    interchange(&outside, &environment, LOAD_A, LOAD_C).unwrap();
}

/// A memory-capable kind without a roster row is an unaccounted access
/// anywhere in the window; barrier kinds and call-roster entries never
/// trade order.
#[test]
fn unaccounted_and_barrier_positions_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for position in [0, 1, 2] {
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
            interchange(&bare, &environment, LOAD_A, LOAD_C).unwrap_err(),
            CommutingInterchangeError::UnsupportedInstruction,
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
            function.blocks[0].instructions[1].kind = kind;
        });
        assert_eq!(
            interchange(&source, &environment, LOAD_A, LOAD_C).unwrap_err(),
            CommutingInterchangeError::UnsupportedInstruction,
            "kind {kind:?}"
        );
    }
    let call_row =
        mutated(target, |function, _| {
            function.calls.push(SelectedCallContract {
            instruction: MAT_B,
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
        interchange(&call_row, &environment, LOAD_A, LOAD_C).unwrap_err(),
        CommutingInterchangeError::UnsupportedInstruction
    );
}

/// A source the admission cannot locate — an absent instruction, a
/// misordered pair, a missing function, a different target — reports its
/// own reason, and an exhausted work budget reports `WorkBudgetExceeded`.
#[test]
fn admission_reports_its_own_reasons() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    assert_eq!(
        interchange(&source, &environment, LOAD_A, TERMINAL).unwrap_err(),
        CommutingInterchangeError::UnsupportedPair
    );
    assert_eq!(
        interchange(&source, &environment, LOAD_C, LOAD_A).unwrap_err(),
        CommutingInterchangeError::UnsupportedPair
    );
    assert_eq!(
        interchange_selected_commuting_pair(&source, 7, LOAD_A, LOAD_C, &environment, budget())
            .unwrap_err(),
        CommutingInterchangeError::SourceMismatch
    );
    let arm64 = baseline_target_register_environment(NativeTarget::linux_arm64()).unwrap();
    assert_eq!(
        interchange_selected_commuting_pair(&source, 0, LOAD_A, LOAD_C, &arm64, budget())
            .unwrap_err(),
        CommutingInterchangeError::SourceMismatch
    );
    let tight = OptimizationWorkBudget::new(1, 1, 1, 1, 1).unwrap();
    assert_eq!(
        interchange_selected_commuting_pair(&source, 0, LOAD_A, LOAD_C, &environment, tight)
            .unwrap_err(),
        CommutingInterchangeError::WorkBudgetExceeded
    );
}

/// Replay independently re-derives the interchange: the proposed program
/// validates again through the public validator, an unpermuted roster is
/// rejected as not the interchange admission proved, an unswapped body is
/// rejected, and any unrelated mutation fails the restore-by-content
/// comparison.
#[test]
fn replay_restores_the_source_by_content() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let result = interchange(&source, &environment, LOAD_A, LOAD_C).unwrap();
    let reproposed = validate_commuting_interchange(
        &source,
        0,
        LOAD_A,
        LOAD_C,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    assert_eq!(reproposed.receipt(), result.receipt());
    // Instructions swapped but the roster left in source order is not the
    // interchange: the recorded accesses must follow the new execution
    // order.
    let mut stale_roster = result.transformed().clone();
    stale_roster.functions[0].memory_accesses =
        source.transformed().functions[0].memory_accesses.clone();
    assert_eq!(
        validate_commuting_interchange(
            &source,
            0,
            LOAD_A,
            LOAD_C,
            &environment,
            budget(),
            stale_roster
        )
        .unwrap_err(),
        CommutingInterchangeError::ReplayMismatch
    );
    // Roster permuted but instructions left in source order.
    let mut stale_body = source.transformed().clone();
    stale_body.functions[0].memory_accesses =
        result.transformed().functions[0].memory_accesses.clone();
    assert_eq!(
        validate_commuting_interchange(
            &source,
            0,
            LOAD_A,
            LOAD_C,
            &environment,
            budget(),
            stale_body
        )
        .unwrap_err(),
        CommutingInterchangeError::ReplayMismatch
    );
    // Any unrelated mutation — here a third roster row — fails the
    // restore-by-content comparison.
    let mut extra = result.transformed().clone();
    extra.functions[0].memory_accesses.push(access(
        MAT_D,
        3,
        SelectedMemoryAccessRole::ReadPlace,
        0,
        8,
    ));
    assert_eq!(
        validate_commuting_interchange(&source, 0, LOAD_A, LOAD_C, &environment, budget(), extra)
            .unwrap_err(),
        CommutingInterchangeError::ReplayMismatch
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
        interchange_selected_commuting_pair(
            &source,
            0,
            LOAD_A,
            LOAD_C,
            &environment,
            OptimizationWorkBudget::new(100, 100, 1, 100, 100).unwrap(),
        )
        .unwrap_err(),
        CommutingInterchangeError::WorkBudgetExceeded
    );
}

/// The measured validation-step boundary: admission charges one step per
/// block plus one per instruction across the plan, then the member and
/// crossed surfaces plus the roster-row product for each
/// member-against-crossed pair, then the admitted function's memory,
/// call, and settlement roster lengths — so the exact count admits the
/// interchange on both the proposal and the independent replay path while
/// one step below rejects both.
#[test]
fn measured_validation_step_boundary_admits_and_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The pair moves to a second block behind a one-instruction entry: the
    // plan scan grows by the entry block's body instruction and
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
            &[SECOND],
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
    // A boundary settlement at the window's own earlier index observes the
    // same executed prefix on either order, so it admits — and charges the
    // settlement roster term.
    let settled = mutated(target, |function, _| {
        function.boundary_settlements.push(settlement(0, 71));
    });
    for (source, exact_steps) in [
        // (4 instructions + 1 block) + member-against-crossed surfaces
        // (each member against the interior and the other member: load 2,
        // materialize 1 — 3 + 5 + 5 + 3, including the members' shared row
        // product counted once per direction) + 2 roster rows = 23.
        (fixture(target), 23u64),
        // (5 instructions + 1 per block over 2 blocks) + the same 16 + 2 =
        // 25.
        (later_block, 25u64),
        // The base charge plus one settlement roster row = 24.
        (settled, 24u64),
    ] {
        let exact = OptimizationWorkBudget::new(1, 1, exact_steps, 1, 1).unwrap();
        let result =
            interchange_selected_commuting_pair(&source, 0, LOAD_A, LOAD_C, &environment, exact)
                .unwrap();
        validate_commuting_interchange(
            &source,
            0,
            LOAD_A,
            LOAD_C,
            &environment,
            exact,
            result.transformed().clone(),
        )
        .unwrap();
        let starved = OptimizationWorkBudget::new(1, 1, exact_steps - 1, 1, 1).unwrap();
        assert_eq!(
            interchange_selected_commuting_pair(&source, 0, LOAD_A, LOAD_C, &environment, starved)
                .unwrap_err(),
            CommutingInterchangeError::WorkBudgetExceeded
        );
        assert_eq!(
            validate_commuting_interchange(
                &source,
                0,
                LOAD_A,
                LOAD_C,
                &environment,
                starved,
                result.transformed().clone(),
            )
            .unwrap_err(),
            CommutingInterchangeError::WorkBudgetExceeded
        );
    }
}

/// Two interchanges over the identical source produce the identical
/// validated result, and the published plan is a legal second input
/// through the sealed analysis boundary: on the transformed plan the
/// pair's order is reversed, so the same names interchange back to
/// restore the source plan bit-identically, while the stale order and a
/// rowless window still decline.
#[test]
fn interchange_is_deterministic_and_re_admitted() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let first = interchange(&source, &environment, LOAD_A, LOAD_C).unwrap();
    let second = interchange(&source, &environment, LOAD_A, LOAD_C).unwrap();
    assert_eq!(first, second);
    // The validated output carries the sealed analysis boundary, so it is
    // a legal second input — not merely a reconstruction of one. On the
    // transformed plan LOAD_C leads the window and LOAD_A closes it, so
    // the same names admit the reverse interchange back to the published
    // source.
    let restored =
        interchange_selected_commuting_pair(&first, 0, LOAD_C, LOAD_A, &environment, budget())
            .unwrap();
    assert_eq!(restored.transformed(), source.transformed());
    assert_eq!(
        restored.receipt().transformed_selected(),
        source.selected_identity()
    );
    validate_commuting_interchange(
        &first,
        0,
        LOAD_C,
        LOAD_A,
        &environment,
        budget(),
        restored.transformed().clone(),
    )
    .unwrap();
    // The stale order declines on the second input: on the transformed
    // plan LOAD_A already follows LOAD_C, so naming it earlier again finds
    // no later position.
    assert_eq!(
        interchange_selected_commuting_pair(&first, 0, LOAD_A, LOAD_C, &environment, budget())
            .unwrap_err(),
        CommutingInterchangeError::UnsupportedPair
    );
    // A rowless window still declines: MAT_B and MAT_D carry no roster
    // rows, so no trading pair is rowed on both sides.
    assert_eq!(
        interchange_selected_commuting_pair(&first, 0, MAT_B, MAT_D, &environment, budget())
            .unwrap_err(),
        CommutingInterchangeError::UnsupportedPair
    );
}

/// Replay drift beyond the admitted window still fails the
/// restore-by-content comparison: an instruction the interchange never
/// touched carries mutated content, so restoring the pair's order and the
/// window's rows cannot reproduce the source.
#[test]
fn replay_rejects_drift_outside_the_window() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let result = interchange(&source, &environment, LOAD_A, LOAD_C).unwrap();
    let mut drifted = result.transformed().clone();
    drifted.functions[0].blocks[0].instructions[3].kind = SelectedInstructionKind::MaterializeI64 {
        value: IntegerValue::Unsigned(10),
    };
    assert_eq!(
        validate_commuting_interchange(&source, 0, LOAD_A, LOAD_C, &environment, budget(), drifted)
            .unwrap_err(),
        CommutingInterchangeError::ReplayMismatch
    );
}

/// An adjacent pair is the distance-one case: two accounted members trade
/// places directly with an empty interior, and the roster rows of
/// positions outside the window keep their places.
#[test]
fn adjacent_pair_interchanges() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[1] = instruction(
            MAT_B,
            SelectedInstructionKind::Load8 { byte_offset: 16 },
            &load,
            &[POINTER, SECOND],
        );
        function.memory_accesses = vec![
            access(LOAD_A, 1, SelectedMemoryAccessRole::ReadPlace, 0, 8),
            access(MAT_B, 2, SelectedMemoryAccessRole::ReadPlace, 16, 8),
            access(LOAD_C, 3, SelectedMemoryAccessRole::ReadPlace, 24, 8),
        ];
    });
    let result = interchange(&source, &environment, LOAD_A, MAT_B).unwrap();
    let body = &result.transformed().functions[0].blocks[0].instructions;
    assert_eq!(
        body.iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![MAT_B, LOAD_A, LOAD_C, MAT_D]
    );
    let roster = &result.transformed().functions[0].memory_accesses;
    assert_eq!(roster[0].instruction, MAT_B);
    assert_eq!(roster[1].instruction, LOAD_A);
    assert_eq!(roster[2].instruction, LOAD_C);
}
