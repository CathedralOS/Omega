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
    LocalRelocationError, LocalRelocationReceipt, ValidatedLocalRelocation,
    relocate_selected_instruction, validate_local_relocation,
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
const MAT_B: SelectedInstructionId = SelectedInstructionId(3);
const SUM: SelectedInstructionId = SelectedInstructionId(4);
const COMPARE: SelectedInstructionId = SelectedInstructionId(5);
const BOOLEAN: SelectedInstructionId = SelectedInstructionId(6);
const TERMINAL: SelectedInstructionId = SelectedInstructionId(7);

const POINTER: VirtualRegisterId = VirtualRegisterId(0);
const FIRST: VirtualRegisterId = VirtualRegisterId(1);
const SECOND: VirtualRegisterId = VirtualRegisterId(2);
const TOTAL: VirtualRegisterId = VirtualRegisterId(3);
const OUTCOME: VirtualRegisterId = VirtualRegisterId(4);

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
/// `r1 = 5; r2 = 7; r3 = r1 + r2; compare r1, r2; r4 = boolean; return`.
/// The two materializations are independent, the sum reads both, the compare
/// publishes condition state, and the boolean materialization reads it.
fn fixture(target: NativeTarget) -> ValidatedLocalRelocation {
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let materialize = environment.constraint(keys.materialize_i64).unwrap();
    let add = environment.constraint(keys.add_i64).unwrap();
    let compare = environment.constraint(keys.compare_i64).unwrap();
    let boolean = environment.constraint(keys.materialize_boolean).unwrap();
    let terminal_row = environment.constraint(keys.return_unit).unwrap();
    let class = compare.operands[0].class;
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
            SECOND,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: MAT_B,
                source_value: ValueId::new(3).unwrap(),
            },
        ),
        register(
            TOTAL,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: SUM,
                source_value: ValueId::new(4).unwrap(),
            },
        ),
        register(
            OUTCOME,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: BOOLEAN,
                source_value: ValueId::new(5).unwrap(),
            },
        ),
    ];
    let instructions = vec![
        instruction(
            MAT_A,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(5),
            },
            materialize,
            &[FIRST],
        ),
        instruction(
            MAT_B,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(7),
            },
            materialize,
            &[SECOND],
        ),
        instruction(
            SUM,
            SelectedInstructionKind::WrappingAddI64,
            add,
            &[FIRST, SECOND, TOTAL],
        ),
        instruction(
            COMPARE,
            SelectedInstructionKind::CompareI64,
            compare,
            &[FIRST, SECOND],
        ),
        instruction(
            BOOLEAN,
            SelectedInstructionKind::MaterializeBooleanEqual,
            boolean,
            &[OUTCOME],
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
    ValidatedLocalRelocation {
        receipt: LocalRelocationReceipt {
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
) -> ValidatedLocalRelocation {
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
    source: &ValidatedLocalRelocation,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
) -> Result<ValidatedLocalRelocation, LocalRelocationError> {
    relocate_selected_instruction(source, 0, member, destination, environment, budget())
}

/// Two adjacent materializations writing different registers trade places
/// when either is relocated onto the other — the distance-one rotation is
/// the adjacent interchange — and each keeps its identity, kind, operands,
/// and provenance while the block vector carries them in the opposite
/// order.
#[test]
fn adjacent_destination_interchanges() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = fixture(target);
        let down = relocate(&source, &environment, MAT_A, MAT_B).unwrap();
        let body = &down.transformed().functions[0].blocks[0].instructions;
        assert_eq!(body[0].id, MAT_B);
        assert_eq!(body[1].id, MAT_A);
        assert_eq!(
            body[2..],
            source.transformed().functions[0].blocks[0].instructions[2..]
        );
        let up = relocate(&source, &environment, MAT_B, MAT_A).unwrap();
        assert_eq!(up.transformed(), down.transformed());
    }
}

/// A flag-defining member relocates across pure register work: the compare
/// publishes condition state and the sum observes neither it nor the
/// compare's registers, so the compare lands on the sum's position and the
/// sum shifts one slot earlier.
#[test]
fn flag_publisher_relocates_across_pure_register_work() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let result = relocate(&source, &environment, COMPARE, SUM).unwrap();
    let body = &result.transformed().functions[0].blocks[0].instructions;
    assert_eq!(body[2].id, COMPARE);
    assert_eq!(body[3].id, SUM);
    validate_local_relocation(
        &source,
        0,
        COMPARE,
        SUM,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
}

/// A member whose write feeds a crossed read keeps order in both
/// directions: relocating the producer after its consumer would starve the
/// consumer, and relocating the consumer ahead of its producer reads a
/// value that has not been defined yet.
#[test]
fn raw_dependency_keeps_order() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // MAT_B defines SECOND and the sum reads it: the producer cannot move
    // down past the consumer.
    assert_eq!(
        relocate(&source, &environment, MAT_B, SUM).unwrap_err(),
        LocalRelocationError::UnsupportedPair
    );
    // The consumer cannot move up ahead of the producer either.
    assert_eq!(
        relocate(&source, &environment, SUM, MAT_B).unwrap_err(),
        LocalRelocationError::UnsupportedPair
    );
    // A member reading a register defined inside the window observes the
    // same hazard at distance: the compare reads FIRST which MAT_A
    // defines, so MAT_A cannot move onto the compare.
    assert_eq!(
        relocate(&source, &environment, MAT_A, COMPARE).unwrap_err(),
        LocalRelocationError::UnsupportedPair
    );
}

/// A member reading a location a crossed instruction writes would observe
/// the new value after the move: relocating the compare onto a
/// materialization redefining its right operand refuses. The same pair in
/// the other direction is a member write feeding a crossed read, which the
/// same audit refuses.
#[test]
fn war_dependency_keeps_order() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The trailing instruction becomes a pure materialization writing
    // SECOND, which the compare reads; no flag use survives on the
    // materialization kind.
    let source = mutated(target, |function, environment| {
        let materialize = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[4] = instruction(
            BOOLEAN,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(9),
            },
            &materialize,
            &[SECOND],
        );
    });
    // Member reads SECOND, crossed writes SECOND: the compare landing after
    // the materialization would observe 9 where it observed 7.
    assert_eq!(
        relocate(&source, &environment, COMPARE, BOOLEAN).unwrap_err(),
        LocalRelocationError::UnsupportedPair
    );
    // Member writes SECOND, crossed reads SECOND: the materialization
    // landing ahead of the compare would change the compare's right
    // operand the same way.
    assert_eq!(
        relocate(&source, &environment, BOOLEAN, COMPARE).unwrap_err(),
        LocalRelocationError::UnsupportedPair
    );
}

/// A member and a crossed instruction writing the same register would
/// change which definition later positions observe.
#[test]
fn waw_dependency_keeps_order() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        function.blocks[0].instructions[1].operands[0].virtual_register = FIRST;
    });
    assert_eq!(
        relocate(&source, &environment, MAT_A, MAT_B).unwrap_err(),
        LocalRelocationError::UnsupportedPair
    );
    assert_eq!(
        relocate(&source, &environment, MAT_B, MAT_A).unwrap_err(),
        LocalRelocationError::UnsupportedPair
    );
}

/// Condition state couples like registers: a flag publisher cannot cross
/// the boolean materialization reading it in either direction, and a flag
/// reader cannot cross a later flag writer.
#[test]
fn condition_state_hazards_keep_order() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    assert_eq!(
        relocate(&source, &environment, COMPARE, BOOLEAN).unwrap_err(),
        LocalRelocationError::UnsupportedPair
    );
    assert_eq!(
        relocate(&source, &environment, BOOLEAN, COMPARE).unwrap_err(),
        LocalRelocationError::UnsupportedPair
    );
    // At distance the same hazard applies: an inert materialization
    // between the compare and the boolean reader does not weaken it.
    let window = mutated(target, |function, environment| {
        let materialize = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap()
            .clone();
        let class = function.virtual_registers[1].class;
        function.virtual_registers.push(register(
            THIRD,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: MAT_C,
                source_value: ValueId::new(6).unwrap(),
            },
        ));
        function.blocks[0].instructions.insert(
            4,
            instruction(
                MAT_C,
                SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(13),
                },
                &materialize,
                &[THIRD],
            ),
        );
    });
    assert_eq!(
        relocate(&window, &environment, COMPARE, BOOLEAN).unwrap_err(),
        LocalRelocationError::UnsupportedPair
    );
    assert_eq!(
        relocate(&window, &environment, BOOLEAN, MAT_A).unwrap_err(),
        LocalRelocationError::UnsupportedPair
    );
}

/// A clobber is a unit write: a flag-clobbering member cannot cross the
/// compare that publishes those flags nor the boolean reader that observes
/// them, and a flag reader cannot relocate across a clobbering writer.
#[test]
fn clobbers_couple_like_definitions() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let subtract_kind = SelectedInstructionKind::ExactSubtractI64 {
        obligation: ObligationId::new(11).unwrap(),
        accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes([7; 32]),
    };
    // The subtract clobbers the flag unit the compare also defines: two
    // writers of one location never reorder.
    let before_compare = mutated(target, |function, environment| {
        let subtract = environment
            .constraint(environment.selected_keys().subtract_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[2] =
            instruction(SUM, subtract_kind, &subtract, &[FIRST, SECOND, TOTAL]);
    });
    assert_eq!(
        relocate(&before_compare, &environment, SUM, COMPARE).unwrap_err(),
        LocalRelocationError::UnsupportedPair
    );
    // A clobbering member cannot cross the flag reader either: moving the
    // write ahead of the read would change the observed flags.
    let before_reader = mutated(target, |function, environment| {
        let subtract = environment
            .constraint(environment.selected_keys().subtract_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[3] =
            instruction(COMPARE, subtract_kind, &subtract, &[FIRST, SECOND, TOTAL]);
    });
    assert_eq!(
        relocate(&before_reader, &environment, COMPARE, BOOLEAN).unwrap_err(),
        LocalRelocationError::UnsupportedPair
    );
    // And the flag reader cannot relocate up across the clobbering writer.
    assert_eq!(
        relocate(&before_reader, &environment, BOOLEAN, COMPARE).unwrap_err(),
        LocalRelocationError::UnsupportedPair
    );
}

/// A boundary settlement inside the window's span observes a different
/// executed prefix once the member lands on the other side of the crossed
/// run; settlements at or outside the span admit.
#[test]
fn interior_settlement_bounds_the_relocation() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Window MAT_A@0 to MAT_B@1: a settlement at position 1 sits before the
    // destination's ordinal and sees the member run after it; positions 0
    // and 2 see the same executed set either way.
    let inside = mutated(target, |function, _| {
        function.boundary_settlements.push(settlement(1, 41));
    });
    assert_eq!(
        relocate(&inside, &environment, MAT_A, MAT_B).unwrap_err(),
        LocalRelocationError::UnsupportedPair
    );
    for position in [0, 2] {
        let boundary = mutated(target, |function, _| {
            function.boundary_settlements.push(settlement(position, 41));
        });
        relocate(&boundary, &environment, MAT_A, MAT_B).unwrap();
    }
    // Moving up binds the same span: the destination's ordinal still sees
    // the member on a different side.
    let up = mutated(target, |function, _| {
        function.boundary_settlements.push(settlement(1, 41));
    });
    assert_eq!(
        relocate(&up, &environment, MAT_B, MAT_A).unwrap_err(),
        LocalRelocationError::UnsupportedPair
    );
    for position in [0, 2] {
        let boundary = mutated(target, |function, _| {
            function.boundary_settlements.push(settlement(position, 41));
        });
        relocate(&boundary, &environment, MAT_B, MAT_A).unwrap();
    }
}

/// A roster-accounted member relocates across pure register work: the
/// crossed instructions cannot observe memory, so the member's recorded
/// access keeps its relative order in the program.
#[test]
fn accounted_member_relocates_across_pure_work() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[0] = instruction(
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
    let result = relocate(&source, &environment, MAT_A, MAT_B).unwrap();
    let body = &result.transformed().functions[0].blocks[0].instructions;
    assert_eq!(body[0].id, MAT_B);
    assert_eq!(body[1].id, MAT_A);
    assert_eq!(
        result.transformed().functions[0].memory_accesses,
        source.transformed().functions[0].memory_accesses
    );
}

/// A row-less member cannot observe memory at all under the roster's
/// completeness, so it relocates across a run of roster-carrying accesses
/// — including a destination that carries rows — while every recorded
/// access keeps its relative order. The pairwise interchange refuses this
/// same window because its later member carries rows beside an accounted
/// interior.
#[test]
fn row_less_member_crosses_an_accounted_run() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let keys = environment.selected_keys();
        let load = environment.constraint(keys.load8.unwrap()).unwrap().clone();
        let store = environment.constraint(keys.store.unwrap()).unwrap().clone();
        let body = &mut function.blocks[0].instructions;
        body[1] = instruction(
            MAT_B,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            &store,
            &[POINTER, SECOND],
        );
        body[2] = instruction(
            SUM,
            SelectedInstructionKind::Load8 { byte_offset: 0 },
            &load,
            &[POINTER, TOTAL],
        );
        function.memory_accesses.push(access(
            MAT_B,
            PlaceId::new(1).unwrap(),
            SelectedMemoryAccessRole::WritePlace,
        ));
        function.memory_accesses.push(access(
            SUM,
            PlaceId::new(2).unwrap(),
            SelectedMemoryAccessRole::ReadPlace,
        ));
    });
    // The member crosses two roster-carrying instructions and lands on the
    // accounted store's position: every recorded access keeps its relative
    // order while the memory-inert materialization moves behind them.
    let result = relocate(&source, &environment, MAT_A, SUM).unwrap();
    let body = &result.transformed().functions[0].blocks[0].instructions;
    assert_eq!(
        body.iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![MAT_B, SUM, MAT_A, COMPARE, BOOLEAN]
    );
    assert_eq!(
        result.transformed().functions[0].memory_accesses,
        source.transformed().functions[0].memory_accesses
    );
    // The pairwise primitive cannot take this window: its members would be
    // MAT_A and the accounted store beside the accounted load inside.
    assert_eq!(
        crate::schedule_selected_pair(&source, 0, MAT_A, SUM, &environment, budget()).unwrap_err(),
        crate::LocalScheduleError::UnsupportedPair
    );
}

/// A roster-carrying member may cross only row-less positions: a second
/// accounted actor anywhere in the window would reorder recorded accesses,
/// which needs a place-alias decision this step does not take.
#[test]
fn accounted_member_sharing_an_accounted_window_keeps_order() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let keys = environment.selected_keys();
        let load = environment.constraint(keys.load8.unwrap()).unwrap().clone();
        let store = environment.constraint(keys.store.unwrap()).unwrap().clone();
        let body = &mut function.blocks[0].instructions;
        body[0] = instruction(
            MAT_A,
            SelectedInstructionKind::Load8 { byte_offset: 0 },
            &load,
            &[POINTER, FIRST],
        );
        body[1] = instruction(
            MAT_B,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            &store,
            &[POINTER, SECOND],
        );
        function.memory_accesses.push(access(
            MAT_A,
            PlaceId::new(1).unwrap(),
            SelectedMemoryAccessRole::ReadPlace,
        ));
        function.memory_accesses.push(access(
            MAT_B,
            PlaceId::new(2).unwrap(),
            SelectedMemoryAccessRole::WritePlace,
        ));
    });
    assert_eq!(
        relocate(&source, &environment, MAT_A, MAT_B).unwrap_err(),
        LocalRelocationError::UnsupportedPair
    );
    assert_eq!(
        relocate(&source, &environment, MAT_B, MAT_A).unwrap_err(),
        LocalRelocationError::UnsupportedPair
    );
}

/// A memory-capable kind without a roster row is an unaccounted access: the
/// relocation cannot prove what it reaches, so it refuses outright whether
/// the bare access is the member or a crossed instruction. Adding the row
/// makes it the window's single accounted actor and admits.
#[test]
fn unaccounted_memory_kind_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let bare_member = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[0] = instruction(
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
        relocate(&bare_member, &environment, MAT_A, MAT_B).unwrap_err(),
        LocalRelocationError::UnsupportedInstruction
    );
    let bare_crossed = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[1] = instruction(
            MAT_B,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            &store,
            &[POINTER, SECOND],
        );
    });
    assert_eq!(
        relocate(&bare_crossed, &environment, MAT_A, MAT_B).unwrap_err(),
        LocalRelocationError::UnsupportedInstruction
    );
    let accounted = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[0] = instruction(
            MAT_A,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            &store,
            &[POINTER, FIRST],
        );
        function.memory_accesses.push(access(
            MAT_A,
            PlaceId::new(1).unwrap(),
            SelectedMemoryAccessRole::WritePlace,
        ));
    });
    relocate(&accounted, &environment, MAT_A, MAT_B).unwrap();
}

/// Calls, hosted effects, and terminator kinds are barriers as the member
/// or anywhere inside the crossed run, and a call-roster row makes an
/// instruction a barrier even when its kind is register-pure.
#[test]
fn barrier_kinds_and_call_roster_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for kind in [
        SelectedInstructionKind::Jump,
        SelectedInstructionKind::ReturnUnit,
        SelectedInstructionKind::ConditionalBranchNonZero,
        SelectedInstructionKind::CallUnit {
            callee: MachineId::new(9).unwrap(),
        },
        SelectedInstructionKind::HostedExitProcessI32,
    ] {
        let member_barrier = mutated(target, |function, _| {
            function.blocks[0].instructions[0].kind = kind;
        });
        assert_eq!(
            relocate(&member_barrier, &environment, MAT_A, MAT_B).unwrap_err(),
            LocalRelocationError::UnsupportedInstruction,
            "member {kind:?}"
        );
        let crossed_barrier = mutated(target, |function, _| {
            function.blocks[0].instructions[1].kind = kind;
        });
        assert_eq!(
            relocate(&crossed_barrier, &environment, MAT_A, MAT_B).unwrap_err(),
            LocalRelocationError::UnsupportedInstruction,
            "crossed {kind:?}"
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
        relocate(&call_row, &environment, MAT_A, MAT_B).unwrap_err(),
        LocalRelocationError::UnsupportedInstruction
    );
}

/// The member and the destination must be distinct body instructions in
/// one block: one repeated id, a destination behind a terminator, an
/// unknown id, and a wrong function index all refuse.
#[test]
fn only_distinct_in_block_positions_relocate() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // One id cannot name both positions.
    assert_eq!(
        relocate(&source, &environment, MAT_A, MAT_A).unwrap_err(),
        LocalRelocationError::UnsupportedPair
    );
    // An unknown member or function index never locates the window.
    assert_eq!(
        relocate(&source, &environment, SelectedInstructionId(99), MAT_B).unwrap_err(),
        LocalRelocationError::SourceMismatch
    );
    assert_eq!(
        relocate_selected_instruction(&source, 1, MAT_A, MAT_B, &environment, budget())
            .unwrap_err(),
        LocalRelocationError::SourceMismatch
    );
    // A terminator-carried instruction is not a body position.
    assert_eq!(
        relocate(&source, &environment, MAT_A, TERMINAL).unwrap_err(),
        LocalRelocationError::UnsupportedPair
    );
    assert_eq!(
        relocate(&source, &environment, TERMINAL, MAT_A).unwrap_err(),
        LocalRelocationError::SourceMismatch
    );
    // A destination in a different block names no in-block window.
    let other_block = mutated(target, |function, environment| {
        let keys = environment.selected_keys();
        let jump_row = environment.constraint(keys.jump).unwrap().clone();
        let return_row = environment.constraint(keys.return_unit).unwrap().clone();
        let moved = function.blocks[0].instructions.remove(1);
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
            instructions: vec![moved],
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
        relocate(&other_block, &environment, MAT_A, MAT_B).unwrap_err(),
        LocalRelocationError::UnsupportedPair
    );
}

/// The member relocates inside a later block: window lookup scopes to the
/// block holding the member, and the untouched entry block and terminator
/// structure survive bit-identical.
#[test]
fn member_in_a_later_block_relocates() {
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
                value: IntegerValue::Unsigned(11),
            },
            &materialize,
            &[OUTCOME],
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
    let result = relocate(&source, &environment, MAT_A, MAT_B).unwrap();
    assert_eq!(result.transformed().functions[0].blocks.len(), 2);
    let entry = &result.transformed().functions[0].blocks[0];
    assert_eq!(entry, &source.transformed().functions[0].blocks[0]);
    let body = &result.transformed().functions[0].blocks[1].instructions;
    assert_eq!(body[0].id, MAT_B);
    assert_eq!(body[1].id, MAT_A);
}

/// Replay consumes only the exact rotation: a proposed block that drops
/// the member, keeps the destination in place, permutes the crossed run,
/// or carries an unrelated edit all reject.
#[test]
fn replay_rejects_anything_but_the_rotation() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = windowed(target, 2);
    let result = relocate(&source, &environment, MAT_A, MAT_B).unwrap();
    // The honest proposal replays.
    validate_local_relocation(
        &source,
        0,
        MAT_A,
        MAT_B,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // The member at the right slot with the crossed run permuted rejects:
    // the run must shift one slot, not reorder.
    let mut permuted = result.transformed().clone();
    permuted.functions[0].blocks[0].instructions.swap(0, 1);
    assert_eq!(
        validate_local_relocation(&source, 0, MAT_A, MAT_B, &environment, budget(), permuted)
            .unwrap_err(),
        LocalRelocationError::ReplayMismatch
    );
    // The plain interchange is not the rotation: interior positions must
    // shift, not keep the endpoints' trade.
    let mut transposed = source.transformed().clone();
    transposed.functions[0].blocks[0].instructions.swap(0, 3);
    assert_eq!(
        validate_local_relocation(&source, 0, MAT_A, MAT_B, &environment, budget(), transposed)
            .unwrap_err(),
        LocalRelocationError::ReplayMismatch
    );
    // A proposal missing an instruction rejects.
    let mut dropped = result.transformed().clone();
    dropped.functions[0].blocks[0].instructions.pop();
    assert_eq!(
        validate_local_relocation(&source, 0, MAT_A, MAT_B, &environment, budget(), dropped)
            .unwrap_err(),
        LocalRelocationError::ReplayMismatch
    );
    // An unrelated literal edit inside the relocated block rejects.
    let mut edited = result.transformed().clone();
    edited.functions[0].blocks[0].instructions[0].kind = SelectedInstructionKind::MaterializeI64 {
        value: IntegerValue::Unsigned(12),
    };
    assert_eq!(
        validate_local_relocation(&source, 0, MAT_A, MAT_B, &environment, budget(), edited)
            .unwrap_err(),
        LocalRelocationError::ReplayMismatch
    );
    // Naming the stale order or a different destination on the same
    // proposal re-derives a different window and rejects.
    assert_eq!(
        validate_local_relocation(
            &source,
            0,
            MAT_B,
            MAT_A,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap_err(),
        LocalRelocationError::ReplayMismatch
    );
}

/// Replay corruption in a block the relocation never touched still rejects:
/// the restore-by-content check compares the complete plan, not just the
/// block carrying the move.
#[test]
fn replay_rejects_drift_outside_the_relocated_block() {
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
                value: IntegerValue::Unsigned(11),
            },
            &materialize,
            &[OUTCOME],
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
    let result = relocate(&source, &environment, MAT_A, MAT_B).unwrap();
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
            &[FIRST, SECOND],
        ));
    assert_eq!(
        validate_local_relocation(&source, 0, MAT_A, MAT_B, &environment, budget(), proposed)
            .unwrap_err(),
        LocalRelocationError::ReplayMismatch
    );
    // A changed literal in the untouched entry block rejects.
    let mut proposed = result.transformed().clone();
    proposed.functions[0].blocks[0].instructions[0].kind =
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(12),
        };
    assert_eq!(
        validate_local_relocation(&source, 0, MAT_A, MAT_B, &environment, budget(), proposed)
            .unwrap_err(),
        LocalRelocationError::ReplayMismatch
    );
}

/// The measured validation step count bounds both the proposal path and
/// the independent replay: a starved budget refuses before any position
/// changes.
#[test]
fn work_budget_bounds_the_scan() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    assert_eq!(
        relocate_selected_instruction(
            &source,
            0,
            MAT_A,
            MAT_B,
            &environment,
            OptimizationWorkBudget::new(100, 100, 1, 100, 100).unwrap(),
        )
        .unwrap_err(),
        LocalRelocationError::WorkBudgetExceeded
    );
}

/// The environment's target must be the plan's target: a relocation proven
/// for one target's constraints is not evidence on another.
#[test]
fn target_mismatch_rejects() {
    let source = fixture(NativeTarget::linux_x64());
    let environment = baseline_target_register_environment(NativeTarget::linux_arm64()).unwrap();
    assert_eq!(
        relocate(&source, &environment, MAT_A, MAT_B).unwrap_err(),
        LocalRelocationError::SourceMismatch
    );
}

/// The measured validation-step boundary: admission charges one step per
/// block plus one per instruction across the plan, then one per operand,
/// implicit use, implicit definition, and clobber row on each window
/// instruction, then the admitted function's memory, call, and settlement
/// roster lengths — so the exact count admits the relocation on both the
/// proposal and the independent replay path while one step below rejects
/// both, at three fixture sizes that each grow a different term of the
/// charge.
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
                value: IntegerValue::Unsigned(11),
            },
            &materialize,
            &[OUTCOME],
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
    // the window-surface term and the memory-access row joins the roster
    // term.
    let roster_actor = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[0] = instruction(
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
    for (source, member, destination, exact_steps) in [
        // (1 block + 5 instructions) + (1 operand per materialization) = 8.
        (fixture(target), MAT_A, MAT_B, 8u64),
        // (2 blocks + 6 instructions) + (1 operand per materialization) = 10.
        (later_block, MAT_A, MAT_B, 10u64),
        // (1 block + 5 instructions) + (2 + 1 window operands) + (1 roster
        // row) = 10.
        (roster_actor, MAT_A, MAT_B, 10u64),
    ] {
        let exact = OptimizationWorkBudget::new(1, 1, exact_steps, 1, 1).unwrap();
        let result =
            relocate_selected_instruction(&source, 0, member, destination, &environment, exact)
                .unwrap();
        validate_local_relocation(
            &source,
            0,
            member,
            destination,
            &environment,
            exact,
            result.transformed().clone(),
        )
        .unwrap();
        let starved = OptimizationWorkBudget::new(1, 1, exact_steps - 1, 1, 1).unwrap();
        assert_eq!(
            relocate_selected_instruction(&source, 0, member, destination, &environment, starved)
                .unwrap_err(),
            LocalRelocationError::WorkBudgetExceeded
        );
        assert_eq!(
            validate_local_relocation(
                &source,
                0,
                member,
                destination,
                &environment,
                starved,
                result.transformed().clone(),
            )
            .unwrap_err(),
            LocalRelocationError::WorkBudgetExceeded
        );
    }
}

const MAT_C: SelectedInstructionId = SelectedInstructionId(8);
const MAT_D: SelectedInstructionId = SelectedInstructionId(9);
const THIRD: VirtualRegisterId = VirtualRegisterId(5);
const FOURTH: VirtualRegisterId = VirtualRegisterId(6);

/// The fixture with `middle` independent materializations inserted between
/// the first materialization and the second: the member sits at position 0
/// and every inserted instruction is pure register work observing neither
/// of the members' registers.
fn windowed(target: NativeTarget, middle: usize) -> ValidatedLocalRelocation {
    mutated(target, |function, environment| {
        let materialize = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap()
            .clone();
        let class = function.virtual_registers[1].class;
        for (offset, (id, member_register)) in [(MAT_C, THIRD), (MAT_D, FOURTH)]
            .iter()
            .take(middle)
            .enumerate()
        {
            function.virtual_registers.push(register(
                *member_register,
                class,
                VirtualRegisterOrigin::InstructionResult {
                    instruction: *id,
                    source_value: ValueId::new(6 + offset as u64).unwrap(),
                },
            ));
            function.blocks[0].instructions.insert(
                1 + offset,
                instruction(
                    *id,
                    SelectedInstructionKind::MaterializeI64 {
                        value: IntegerValue::Unsigned(13 + offset as u128),
                    },
                    &materialize,
                    &[*member_register],
                ),
            );
        }
    })
}

/// The member relocates to the far end of an inert run on every target:
/// each crossed instruction shifts one slot toward the vacated position
/// while keeping its identity, kind, operands, provenance, and relative
/// order — the window's interior does not stay put as it would under the
/// interchange.
#[test]
fn member_relocates_across_inert_run() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        for middle in 1..=2usize {
            let source = windowed(target, middle);
            let result = relocate(&source, &environment, MAT_A, MAT_B).unwrap();
            let original = &source.transformed().functions[0].blocks[0].instructions;
            let body = &result.transformed().functions[0].blocks[0].instructions;
            assert_eq!(body.len(), original.len());
            // The member lands at the destination's index; the crossed run
            // shifts one slot earlier in its original order.
            assert_eq!(body[middle + 1].id, MAT_A);
            for position in 0..=middle {
                assert_eq!(body[position], original[position + 1]);
            }
            assert_eq!(&body[middle + 2..], &original[middle + 2..]);
            validate_local_relocation(
                &source,
                0,
                MAT_A,
                MAT_B,
                &environment,
                budget(),
                result.transformed().clone(),
            )
            .unwrap();
        }
    }
}

/// The same window runs in the other direction: a later member relocates
/// up to an earlier destination's position and the crossed run shifts one
/// slot later.
#[test]
fn member_relocates_up_across_inert_run() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = windowed(target, 2);
    let result = relocate(&source, &environment, MAT_B, MAT_A).unwrap();
    let original = &source.transformed().functions[0].blocks[0].instructions;
    let body = &result.transformed().functions[0].blocks[0].instructions;
    assert_eq!(body[0].id, MAT_B);
    for position in 1..=3 {
        assert_eq!(body[position], original[position - 1]);
    }
    assert_eq!(&body[4..], &original[4..]);
}

/// A `windowed` source with the settlement at `position` added: the
/// fixture helper's `mutated` wrapper cannot push settlements after
/// building the window, so this adds the row and refreshes the plan
/// identity directly.
fn windowed_settled(
    target: NativeTarget,
    middle: usize,
    position: u32,
) -> ValidatedLocalRelocation {
    let mut source = windowed(target, middle);
    std::sync::Arc::make_mut(&mut source.transformed).functions[0]
        .boundary_settlements
        .push(settlement(position, 41));
    let identity = selected_instruction_plan_identity(&source.transformed);
    source.receipt.source_selected = identity;
    source.receipt.transformed_selected = identity;
    source
}

/// At distance the settlement boundary is the window's span: every
/// position after the window's first index through its last sees the
/// member on a different side, while positions at or outside the span
/// observe the same executed set on either order.
#[test]
fn windowed_settlements_bound_the_span() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Window MAT_A@0 to MAT_B@3: positions 1..=3 reject, position 0 and 4
    // admit.
    for position in 1..=3 {
        let source = windowed_settled(target, 2, position);
        assert_eq!(
            relocate(&source, &environment, MAT_A, MAT_B).unwrap_err(),
            LocalRelocationError::UnsupportedPair,
            "settlement at {position}"
        );
    }
    for position in [0, 4] {
        let source = windowed_settled(target, 2, position);
        relocate(&source, &environment, MAT_A, MAT_B).unwrap();
    }
    // Moving up binds the same span: positions after the destination
    // through the member's own index reject.
    for position in 1..=3 {
        let source = windowed_settled(target, 2, position);
        assert_eq!(
            relocate(&source, &environment, MAT_B, MAT_A).unwrap_err(),
            LocalRelocationError::UnsupportedPair,
            "upward settlement at {position}"
        );
    }
    let source = windowed_settled(target, 2, 4);
    relocate(&source, &environment, MAT_B, MAT_A).unwrap();
}

/// At distance the window still refuses barrier kinds and unaccounted
/// memory anywhere inside it, while a row-less member crosses any number
/// of roster-carrying positions.
#[test]
fn windowed_barriers_and_memory_actors_bound_the_span() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A barrier inside the window rejects even though the named pair is
    // clear of it.
    let barrier = windowed(target, 2);
    let mut barrier_source = barrier.clone();
    std::sync::Arc::make_mut(&mut barrier_source.transformed).functions[0].blocks[0].instructions
        [1]
    .kind = SelectedInstructionKind::Jump;
    let identity = selected_instruction_plan_identity(&barrier_source.transformed);
    barrier_source.receipt.source_selected = identity;
    barrier_source.receipt.transformed_selected = identity;
    assert_eq!(
        relocate(&barrier_source, &environment, MAT_A, MAT_B).unwrap_err(),
        LocalRelocationError::UnsupportedInstruction
    );
    // Two roster-carrying interior instructions still admit under a
    // row-less member: the recorded accesses keep their relative order as
    // the run shifts.
    let accounted = mutated(target, |function, environment| {
        let keys = environment.selected_keys();
        let load = environment.constraint(keys.load8.unwrap()).unwrap().clone();
        let store = environment.constraint(keys.store.unwrap()).unwrap().clone();
        let class = function.virtual_registers[1].class;
        for (offset, (id, member_register)) in [(MAT_C, THIRD), (MAT_D, FOURTH)].iter().enumerate()
        {
            function.virtual_registers.push(register(
                *member_register,
                class,
                VirtualRegisterOrigin::InstructionResult {
                    instruction: *id,
                    source_value: ValueId::new(6 + offset as u64).unwrap(),
                },
            ));
        }
        let body = &mut function.blocks[0].instructions;
        body.insert(
            1,
            instruction(
                MAT_C,
                SelectedInstructionKind::Load8 { byte_offset: 0 },
                &load,
                &[POINTER, THIRD],
            ),
        );
        body.insert(
            2,
            instruction(
                MAT_D,
                SelectedInstructionKind::Store {
                    byte_offset: 0,
                    byte_size: 8,
                },
                &store,
                &[POINTER, FOURTH],
            ),
        );
        function.memory_accesses.push(access(
            MAT_C,
            PlaceId::new(1).unwrap(),
            SelectedMemoryAccessRole::ReadPlace,
        ));
        function.memory_accesses.push(access(
            MAT_D,
            PlaceId::new(2).unwrap(),
            SelectedMemoryAccessRole::WritePlace,
        ));
    });
    let result = relocate(&accounted, &environment, MAT_A, MAT_B).unwrap();
    let body = &result.transformed().functions[0].blocks[0].instructions;
    assert_eq!(
        body.iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![MAT_C, MAT_D, MAT_B, MAT_A, SUM, COMPARE, BOOLEAN]
    );
    // An accounted member crossing the same accounted run refuses: its
    // recorded access would pass the interior's.
    let member_actor = mutated(target, |function, environment| {
        let keys = environment.selected_keys();
        let load = environment.constraint(keys.load8.unwrap()).unwrap().clone();
        let store = environment.constraint(keys.store.unwrap()).unwrap().clone();
        function.blocks[0].instructions[0] = instruction(
            MAT_A,
            SelectedInstructionKind::Load8 { byte_offset: 0 },
            &load,
            &[POINTER, FIRST],
        );
        function.blocks[0].instructions[1] = instruction(
            MAT_B,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            &store,
            &[POINTER, SECOND],
        );
        function.memory_accesses.push(access(
            MAT_A,
            PlaceId::new(1).unwrap(),
            SelectedMemoryAccessRole::ReadPlace,
        ));
        function.memory_accesses.push(access(
            MAT_B,
            PlaceId::new(2).unwrap(),
            SelectedMemoryAccessRole::WritePlace,
        ));
    });
    assert_eq!(
        relocate(&member_actor, &environment, MAT_A, SUM).unwrap_err(),
        LocalRelocationError::UnsupportedPair
    );
}

/// Two runs over the identical source produce the identical validated
/// result, and the published plan is a legal second input through the
/// sealed analysis boundary: on the transformed plan the window's order
/// rotated, so the inverse relocation restores the published source
/// bit-identically, a hazard-coupled move still declines, and a different
/// relocation or an interchange still admits.
#[test]
fn relocation_is_deterministic_and_re_admitted() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = windowed(target, 2);
    let first = relocate(&source, &environment, MAT_A, MAT_B).unwrap();
    let second = relocate(&source, &environment, MAT_A, MAT_B).unwrap();
    assert_eq!(first, second);
    // The validated output carries the sealed analysis boundary, so it is
    // a legal second input — not merely a reconstruction of one. The
    // member now sits at the destination's old index; relocating it back
    // onto the head of the crossed run restores the source bit-identically.
    let restored =
        relocate_selected_instruction(&first, 0, MAT_A, MAT_C, &environment, budget()).unwrap();
    assert_eq!(restored.transformed(), source.transformed());
    assert_eq!(
        restored.receipt().transformed_selected(),
        source.selected_identity()
    );
    validate_local_relocation(
        &first,
        0,
        MAT_A,
        MAT_C,
        &environment,
        budget(),
        restored.transformed().clone(),
    )
    .unwrap();
    // A hazard-coupled move still declines on the second input: the sum
    // reads the register the moved materialization defines.
    assert_eq!(
        relocate_selected_instruction(&first, 0, MAT_A, SUM, &environment, budget()).unwrap_err(),
        LocalRelocationError::UnsupportedPair
    );
    // A different relocation still admits on the second input.
    let composed =
        relocate_selected_instruction(&first, 0, SUM, COMPARE, &environment, budget()).unwrap();
    let body = &composed.transformed().functions[0].blocks[0].instructions;
    assert_eq!(body[4].id, COMPARE);
    assert_eq!(body[5].id, SUM);
    // And the pairwise interchange composes through the same boundary.
    let swapped =
        crate::schedule_selected_pair(&first, 0, SUM, COMPARE, &environment, budget()).unwrap();
    let body = &swapped.transformed().functions[0].blocks[0].instructions;
    assert_eq!(body[4].id, COMPARE);
    assert_eq!(body[5].id, SUM);
}

/// The windowed relocation's measured step boundary at distance: the plan
/// scan, the window's operand surface, and the roster term all charge, so
/// the exact count admits while one below rejects.
#[test]
fn windowed_measured_validation_step_boundary_admits_and_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = windowed(target, 2);
    // (1 block + 7 instructions) + (1 operand per window materialization
    // over a four-instruction window) = 12.
    let exact_steps = 12u64;
    let exact = OptimizationWorkBudget::new(1, 1, exact_steps, 1, 1).unwrap();
    let result =
        relocate_selected_instruction(&source, 0, MAT_A, MAT_B, &environment, exact).unwrap();
    validate_local_relocation(
        &source,
        0,
        MAT_A,
        MAT_B,
        &environment,
        exact,
        result.transformed().clone(),
    )
    .unwrap();
    let starved = OptimizationWorkBudget::new(1, 1, exact_steps - 1, 1, 1).unwrap();
    assert_eq!(
        relocate_selected_instruction(&source, 0, MAT_A, MAT_B, &environment, starved).unwrap_err(),
        LocalRelocationError::WorkBudgetExceeded
    );
    assert_eq!(
        validate_local_relocation(
            &source,
            0,
            MAT_A,
            MAT_B,
            &environment,
            starved,
            result.transformed().clone(),
        )
        .unwrap_err(),
        LocalRelocationError::WorkBudgetExceeded
    );
}
