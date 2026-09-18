use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget};
use optimization_unit::{EffectLink, ValueDefinitionSite};
use register_environment::baseline_target_register_environment;
use register_model::RegisterInstructionConstraint;
use selected_instructions::{
    LocalStorageSlotId, SelectedBlock, SelectedBlockId, SelectedBlockOrigin,
    SelectedBoundarySettlement, SelectedBoundarySettlementPayload, SelectedCallContract,
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionPlan, SelectedMemoryAccess, SelectedMemoryAccessOrigin,
    SelectedMemoryAccessRole, SelectedOperand, SelectedTerminator, VirtualRegister,
    VirtualRegisterId, VirtualRegisterOrigin,
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
    CommutingRelocationError, CommutingRelocationReceipt, ValidatedCommutingRelocation,
    relocate_selected_commuting_member, validate_commuting_relocation,
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

const STORE_A: SelectedInstructionId = SelectedInstructionId(2);
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

/// A raw selected-stage unit fixture, not a source/Terminal admission claim:
/// `*r0+0 = r1; r2 = 5; r3 = *r0+16; r4 = 9; return`. The named pair the
/// tests relocate are the store `STORE_A` at position 0 and the load
/// `LOAD_C` at position 2 around the inert interior `MAT_B`, with `MAT_D`
/// after the window and `r0` an entry parameter so both address operands
/// have no in-body producer. The store and the load reach distinct places.
fn fixture(target: NativeTarget) -> ValidatedCommutingRelocation {
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let materialize = environment.constraint(keys.materialize_i64).unwrap();
    let store = environment.constraint(keys.store.unwrap()).unwrap();
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
        register(FIRST, class, STORE_A, 2),
        register(SECOND, class, MAT_B, 3),
        register(THIRD, class, LOAD_C, 4),
        register(FOURTH, class, MAT_D, 5),
    ];
    let instructions = vec![
        instruction(
            STORE_A,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            store,
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
            SelectedInstructionKind::Load8 { byte_offset: 16 },
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
            memory_accesses: vec![
                access(STORE_A, 1, SelectedMemoryAccessRole::WritePlace, 0, 8),
                access(LOAD_C, 2, SelectedMemoryAccessRole::ReadPlace, 16, 8),
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
    ValidatedCommutingRelocation {
        transformed: std::sync::Arc::new(plan),
        receipt: CommutingRelocationReceipt {
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
) -> ValidatedCommutingRelocation {
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

fn relocate(
    source: &ValidatedCommutingRelocation,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
) -> Result<ValidatedCommutingRelocation, CommutingRelocationError> {
    relocate_selected_commuting_member(source, 0, member, destination, environment, budget())
}

/// An accounted store relocates past an inert position and an accounted
/// load on every target — downward to the destination's index and upward
/// back again: the member keeps its identity, kind, operands, and
/// provenance, the crossed run keeps its relative order shifted one slot,
/// and the roster's window rows follow the new execution order while the
/// receipt binds both identities.
#[test]
fn commuting_member_relocates_on_every_target() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = fixture(target);
        let result = relocate(&source, &environment, STORE_A, LOAD_C).unwrap();
        let original = &source.transformed().functions[0].blocks[0].instructions;
        let body = &result.transformed().functions[0].blocks[0].instructions;
        assert_eq!(
            body.iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![MAT_B, LOAD_C, STORE_A, MAT_D]
        );
        assert_eq!(body[0], original[1]);
        assert_eq!(body[1], original[2]);
        assert_eq!(body[2], original[0]);
        assert_eq!(body[3], original[3]);
        // The roster followed the new execution order: LOAD_C's row now
        // leads the window and STORE_A's closes it, each row itself
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
        // The upward direction relocates the load onto the store's index.
        let upward = relocate(&source, &environment, LOAD_C, STORE_A).unwrap();
        let body = &upward.transformed().functions[0].blocks[0].instructions;
        assert_eq!(
            body.iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![LOAD_C, STORE_A, MAT_B, MAT_D]
        );
        let roster = &upward.transformed().functions[0].memory_accesses;
        assert_eq!(roster[0].instruction, LOAD_C);
        assert_eq!(roster[1].instruction, STORE_A);
    }
}

/// The family's accounting rule admits every commuting row shape the local
/// relocation refused: two reads of one place, a read against a write on a
/// disjoint extent of one place, a write into staging storage distinct
/// from any place, and a dynamic-extent read against a write on a
/// different place — each with the roster permuted into the new execution
/// order.
#[test]
fn commuting_access_shapes_admit() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A store against a read of the same place's disjoint bytes.
    let disjoint = mutated(target, |function, _| {
        function.memory_accesses = vec![
            access(STORE_A, 1, SelectedMemoryAccessRole::WritePlace, 0, 8),
            access(LOAD_C, 1, SelectedMemoryAccessRole::ReadPlace, 16, 8),
        ];
    });
    relocate(&disjoint, &environment, STORE_A, LOAD_C).unwrap();
    // A write into a boundary slot is storage distinct from any place:
    // the slot is never a place's own storage, so the staged bytes never
    // reach the place's bytes.
    let staged = mutated(target, |function, _| {
        function.memory_accesses = vec![
            access(
                STORE_A,
                1,
                SelectedMemoryAccessRole::WriteLocal {
                    slot: LocalStorageSlotId::Boundary {
                        operation: OperationId::new(61).unwrap(),
                    },
                },
                0,
                8,
            ),
            access(LOAD_C, 1, SelectedMemoryAccessRole::ReadPlace, 16, 8),
        ];
    });
    relocate(&staged, &environment, STORE_A, LOAD_C).unwrap();
    // A dynamic-extent read commutes with a write whose reach it provably
    // never shares — here a different place entirely.
    let span = mutated(target, |function, _| {
        function.memory_accesses = vec![
            access(STORE_A, 1, SelectedMemoryAccessRole::WritePlace, 0, 8),
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
    relocate(&span, &environment, STORE_A, LOAD_C).unwrap();
    // Two reads of one place: neither changes what the other observes.
    let reads = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[0] = instruction(
            STORE_A,
            SelectedInstructionKind::Load8 { byte_offset: 0 },
            &load,
            &[POINTER, FIRST],
        );
        function.memory_accesses = vec![
            access(STORE_A, 1, SelectedMemoryAccessRole::ReadPlace, 0, 8),
            access(LOAD_C, 1, SelectedMemoryAccessRole::ReadPlace, 0, 8),
        ];
    });
    let result = relocate(&reads, &environment, STORE_A, LOAD_C).unwrap();
    assert_eq!(
        result.transformed().functions[0].memory_accesses[0].instruction,
        LOAD_C
    );
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
    // Overlapping fixed extents on one place: the member's write against
    // the crossed read of the same bytes.
    let overlapping = mutated(target, |function, _| {
        function.memory_accesses = vec![
            access(STORE_A, 1, SelectedMemoryAccessRole::WritePlace, 4, 8),
            access(LOAD_C, 1, SelectedMemoryAccessRole::ReadPlace, 0, 8),
        ];
    });
    assert_eq!(
        relocate(&overlapping, &environment, STORE_A, LOAD_C).unwrap_err(),
        CommutingRelocationError::UnsupportedPair
    );
    // Two writes on one place's shared extent: either order changes the
    // bytes both leave behind.
    let writes = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
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
            access(STORE_A, 1, SelectedMemoryAccessRole::WritePlace, 0, 16),
            access(LOAD_C, 1, SelectedMemoryAccessRole::WritePlace, 8, 8),
        ];
    });
    assert_eq!(
        relocate(&writes, &environment, STORE_A, LOAD_C).unwrap_err(),
        CommutingRelocationError::UnsupportedPair
    );
    // A dynamic-extent span cannot be bounded away from the member's write
    // on the same place even though its own recorded extent is empty.
    let span = mutated(target, |function, _| {
        function.memory_accesses = vec![
            access(STORE_A, 1, SelectedMemoryAccessRole::WritePlace, 64, 8),
            dynamic_access(
                LOAD_C,
                1,
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
    assert_eq!(
        relocate(&span, &environment, STORE_A, LOAD_C).unwrap_err(),
        CommutingRelocationError::UnsupportedPair
    );
    // The member's row against an accounted interior position refuses
    // under the same rule: the write trades order with the interior's
    // read of the same bytes.
    let interior = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[1] = instruction(
            MAT_B,
            SelectedInstructionKind::Load8 { byte_offset: 0 },
            &load,
            &[POINTER, SECOND],
        );
        function.memory_accesses = vec![
            access(STORE_A, 1, SelectedMemoryAccessRole::WritePlace, 0, 8),
            access(MAT_B, 1, SelectedMemoryAccessRole::ReadPlace, 0, 8),
            access(LOAD_C, 2, SelectedMemoryAccessRole::ReadPlace, 16, 8),
        ];
    });
    assert_eq!(
        relocate(&interior, &environment, STORE_A, LOAD_C).unwrap_err(),
        CommutingRelocationError::UnsupportedPair
    );
}

/// The local relocation's own accounting cases stay with it: a window
/// whose trading pairs carry no rowed-vs-rowed trade — no rows at all, or
/// rows on only one side — is `UnsupportedPair` here even though the
/// commutation audit is vacuously satisfied.
#[test]
fn rowless_windows_belong_to_the_local_relocation() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // No rows at all: the plain relocation case. The member keeps an
    // unaccounted-capable kind out of the window — a row-less `Store`
    // would fail the schedulable bar before the accounting rule runs.
    let bare = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[0] = instruction(
            STORE_A,
            SelectedInstructionKind::Load8 { byte_offset: 0 },
            &load,
            &[POINTER, FIRST],
        );
        function.memory_accesses = Vec::new();
    });
    assert_eq!(
        relocate(&bare, &environment, STORE_A, LOAD_C).unwrap_err(),
        CommutingRelocationError::UnsupportedPair
    );
    // A rowed member against row-less positions: the single-actor case.
    let one_side = mutated(target, |function, _| {
        function.memory_accesses = vec![access(
            STORE_A,
            1,
            SelectedMemoryAccessRole::WritePlace,
            0,
            8,
        )];
    });
    assert_eq!(
        relocate(&one_side, &environment, STORE_A, LOAD_C).unwrap_err(),
        CommutingRelocationError::UnsupportedPair
    );
    // A rowed crossed run alone admits in the plain family when the member
    // is row-less; here the member rows but every crossed position is
    // row-less.
    let crossed_only = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[0] = instruction(
            STORE_A,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(3),
            },
            &environment
                .constraint(environment.selected_keys().materialize_i64)
                .unwrap()
                .clone(),
            &[FIRST],
        );
        function.blocks[0].instructions[1] = instruction(
            MAT_B,
            SelectedInstructionKind::Load8 { byte_offset: 24 },
            &load,
            &[POINTER, SECOND],
        );
        function.memory_accesses = vec![
            access(MAT_B, 1, SelectedMemoryAccessRole::ReadPlace, 24, 8),
            access(LOAD_C, 2, SelectedMemoryAccessRole::ReadPlace, 16, 8),
        ];
    });
    assert_eq!(
        relocate(&crossed_only, &environment, STORE_A, LOAD_C).unwrap_err(),
        CommutingRelocationError::UnsupportedPair
    );
}

/// Register and condition-state hazards keep the local relocation's audit
/// unchanged: a crossed position writing a register the member reads — or
/// reading one the member writes — refuses the move.
#[test]
fn register_hazards_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The interior now defines POINTER, which both the store and the load
    // read as their address: relocating the member past it would observe
    // the wrong address.
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
        relocate(&hazard, &environment, STORE_A, LOAD_C).unwrap_err(),
        CommutingRelocationError::UnsupportedPair
    );
    // The destination writes the register the member reads as its stored
    // value: relocating the member past it stores the wrong bytes.
    let reader = mutated(target, |function, environment| {
        let add = environment
            .constraint(environment.selected_keys().add_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[2] = instruction(
            LOAD_C,
            SelectedInstructionKind::WrappingAddI64,
            &add,
            &[THIRD, FOURTH, FIRST],
        );
        function
            .memory_accesses
            .retain(|access| access.instruction != LOAD_C);
        function
            .memory_accesses
            .push(access(MAT_B, 1, SelectedMemoryAccessRole::ReadPlace, 24, 8));
    });
    assert_eq!(
        relocate(&reader, &environment, STORE_A, LOAD_C).unwrap_err(),
        CommutingRelocationError::UnsupportedPair
    );
}

/// A boundary settlement inside the window's span observes a different
/// executed prefix once the member relocates and refuses; a settlement at
/// the window's own first index or past its end sees the same executed
/// set and admits.
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
            relocate(&source, &environment, STORE_A, LOAD_C).unwrap_err(),
            CommutingRelocationError::UnsupportedPair,
            "settlement at {position}"
        );
    }
    let outside = mutated(target, |function, _| {
        function.boundary_settlements.push(settlement(0));
        function.boundary_settlements.push(settlement(3));
    });
    relocate(&outside, &environment, STORE_A, LOAD_C).unwrap();
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
            relocate(&bare, &environment, STORE_A, LOAD_C).unwrap_err(),
            CommutingRelocationError::UnsupportedInstruction,
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
            relocate(&source, &environment, STORE_A, LOAD_C).unwrap_err(),
            CommutingRelocationError::UnsupportedInstruction,
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
        relocate(&call_row, &environment, STORE_A, LOAD_C).unwrap_err(),
        CommutingRelocationError::UnsupportedInstruction
    );
}

/// A source the admission cannot locate — an absent member or
/// destination, the destination at the member's own position, a missing
/// function, a different target — reports its own reason, and an
/// exhausted work budget reports `WorkBudgetExceeded`.
#[test]
fn admission_reports_its_own_reasons() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    assert_eq!(
        relocate(&source, &environment, STORE_A, TERMINAL).unwrap_err(),
        CommutingRelocationError::UnsupportedPair
    );
    assert_eq!(
        relocate(&source, &environment, STORE_A, STORE_A).unwrap_err(),
        CommutingRelocationError::UnsupportedPair
    );
    assert_eq!(
        relocate(&source, &environment, SelectedInstructionId(77), LOAD_C).unwrap_err(),
        CommutingRelocationError::SourceMismatch
    );
    assert_eq!(
        relocate_selected_commuting_member(&source, 7, STORE_A, LOAD_C, &environment, budget())
            .unwrap_err(),
        CommutingRelocationError::SourceMismatch
    );
    let arm64 = baseline_target_register_environment(NativeTarget::linux_arm64()).unwrap();
    assert_eq!(
        relocate_selected_commuting_member(&source, 0, STORE_A, LOAD_C, &arm64, budget())
            .unwrap_err(),
        CommutingRelocationError::SourceMismatch
    );
    let tight = OptimizationWorkBudget::new(1, 1, 1, 1, 1).unwrap();
    assert_eq!(
        relocate_selected_commuting_member(&source, 0, STORE_A, LOAD_C, &environment, tight)
            .unwrap_err(),
        CommutingRelocationError::WorkBudgetExceeded
    );
}

/// Replay independently re-derives the relocation: the proposed program
/// validates again through the public validator, an unpermuted roster is
/// rejected as not the relocation admission proved, an unrotated body is
/// rejected, and any unrelated mutation fails the restore-by-content
/// comparison.
#[test]
fn replay_restores_the_source_by_content() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let result = relocate(&source, &environment, STORE_A, LOAD_C).unwrap();
    let reproposed = validate_commuting_relocation(
        &source,
        0,
        STORE_A,
        LOAD_C,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    assert_eq!(reproposed.receipt(), result.receipt());
    // Instructions rotated but the roster left in source order is not the
    // relocation: the recorded accesses must follow the new execution
    // order.
    let mut stale_roster = result.transformed().clone();
    stale_roster.functions[0].memory_accesses =
        source.transformed().functions[0].memory_accesses.clone();
    assert_eq!(
        validate_commuting_relocation(
            &source,
            0,
            STORE_A,
            LOAD_C,
            &environment,
            budget(),
            stale_roster
        )
        .unwrap_err(),
        CommutingRelocationError::ReplayMismatch
    );
    // Roster permuted but instructions left in source order.
    let mut stale_body = source.transformed().clone();
    stale_body.functions[0].memory_accesses =
        result.transformed().functions[0].memory_accesses.clone();
    assert_eq!(
        validate_commuting_relocation(
            &source,
            0,
            STORE_A,
            LOAD_C,
            &environment,
            budget(),
            stale_body
        )
        .unwrap_err(),
        CommutingRelocationError::ReplayMismatch
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
        validate_commuting_relocation(&source, 0, STORE_A, LOAD_C, &environment, budget(), extra)
            .unwrap_err(),
        CommutingRelocationError::ReplayMismatch
    );
}

/// An adjacent destination is the distance-one case: the member and the
/// accounted position it lands on trade places directly with an empty
/// interior, and the roster rows of positions outside the window keep
/// their places.
#[test]
fn adjacent_destination_relocates() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[1] = instruction(
            MAT_B,
            SelectedInstructionKind::Load8 { byte_offset: 24 },
            &load,
            &[POINTER, SECOND],
        );
        function.memory_accesses = vec![
            access(STORE_A, 1, SelectedMemoryAccessRole::WritePlace, 0, 8),
            access(MAT_B, 2, SelectedMemoryAccessRole::ReadPlace, 24, 8),
            access(LOAD_C, 3, SelectedMemoryAccessRole::ReadPlace, 16, 8),
        ];
    });
    let result = relocate(&source, &environment, STORE_A, MAT_B).unwrap();
    let body = &result.transformed().functions[0].blocks[0].instructions;
    assert_eq!(
        body.iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![MAT_B, STORE_A, LOAD_C, MAT_D]
    );
    let roster = &result.transformed().functions[0].memory_accesses;
    assert_eq!(roster[0].instruction, MAT_B);
    assert_eq!(roster[1].instruction, STORE_A);
    assert_eq!(roster[2].instruction, LOAD_C);
}
