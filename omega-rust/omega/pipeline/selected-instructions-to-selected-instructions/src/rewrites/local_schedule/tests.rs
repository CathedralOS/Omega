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
    LocalScheduleError, LocalScheduleReceipt, ValidatedLocalSchedule, schedule_selected_pair,
    validate_local_schedule,
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
fn fixture(target: NativeTarget) -> ValidatedLocalSchedule {
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
    ValidatedLocalSchedule {
        receipt: LocalScheduleReceipt {
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
) -> ValidatedLocalSchedule {
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

fn schedule(
    source: &ValidatedLocalSchedule,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    earlier: SelectedInstructionId,
    later: SelectedInstructionId,
) -> Result<ValidatedLocalSchedule, LocalScheduleError> {
    schedule_selected_pair(source, 0, earlier, later, environment, budget())
}

/// Two materializations writing different registers exchange places on every
/// target: each keeps its identity, kind, operands, and provenance while the
/// block vector carries them in the opposite order.
#[test]
fn independent_adjacent_pair_interchanges() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = fixture(target);
        let result = schedule(&source, &environment, MAT_A, MAT_B).unwrap();
        let body = &result.transformed().functions[0].blocks[0].instructions;
        assert_eq!(body[0].id, MAT_B);
        assert_eq!(body[1].id, MAT_A);
        assert_eq!(
            body[0],
            source.transformed().functions[0].blocks[0].instructions[1]
        );
        assert_eq!(
            body[1],
            source.transformed().functions[0].blocks[0].instructions[0]
        );
        assert_eq!(body.len(), 5);
        assert_eq!(
            result.receipt().source_selected(),
            source.selected_identity()
        );
        assert_eq!(
            result.receipt().transformed_selected(),
            selected_instruction_plan_identity(result.transformed())
        );
        validate_local_schedule(
            &source,
            0,
            MAT_A,
            MAT_B,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
        // A detached, separately allocated proposal replays by content.
        let mut detached = result.transformed().clone();
        detached.functions = detached.functions.iter().cloned().collect();
        validate_local_schedule(&source, 0, MAT_A, MAT_B, &environment, budget(), detached)
            .unwrap();
    }
}

/// A compare followed by a pure arithmetic instruction interchanges: the sum
/// reads the same registers and writes a third, and neither touches the
/// condition-state units the compare publishes.
#[test]
fn flag_publisher_crosses_pure_register_work() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let result = schedule(&source, &environment, SUM, COMPARE).unwrap();
    let body = &result.transformed().functions[0].blocks[0].instructions;
    assert_eq!(body[2].id, COMPARE);
    assert_eq!(body[3].id, SUM);
    assert_eq!(
        body[2].implicit_defs,
        source.transformed().functions[0].blocks[0].instructions[3].implicit_defs
    );
}

/// A use of the earlier instruction's result must stay behind it: the sum
/// reads the register the second materialization defines.
#[test]
fn raw_dependency_keeps_order() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    assert_eq!(
        schedule(&source, &environment, MAT_B, SUM).unwrap_err(),
        LocalScheduleError::UnsupportedPair
    );
    assert_eq!(
        schedule(&source, &environment, MAT_A, MAT_B)
            .unwrap()
            .receipt()
            .source_selected(),
        source.selected_identity()
    );
}

/// The later instruction may not overwrite a register the earlier one reads:
/// the compare's view of its inputs would change.
#[test]
fn war_dependency_keeps_order() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
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
            &[FIRST],
        );
    });
    assert_eq!(
        schedule(&source, &environment, COMPARE, BOOLEAN).unwrap_err(),
        LocalScheduleError::UnsupportedPair
    );
}

/// Two definitions of one register cannot exchange: positions after the pair
/// observe whichever write ran last.
#[test]
fn waw_dependency_keeps_order() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        function.blocks[0].instructions[1].operands[0].virtual_register = FIRST;
    });
    assert_eq!(
        schedule(&source, &environment, MAT_A, MAT_B).unwrap_err(),
        LocalScheduleError::UnsupportedPair
    );
}

/// Condition-state units couple exactly like registers: a flag reader never
/// crosses its publisher, a reader never crosses a later publisher, and two
/// flag writers never exchange — while two readers of the same flag state
/// interchange freely.
#[test]
fn condition_state_hazards_keep_order() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // RAW: the boolean materialization reads the flags the compare defines.
    let source = fixture(target);
    assert_eq!(
        schedule(&source, &environment, COMPARE, BOOLEAN).unwrap_err(),
        LocalScheduleError::UnsupportedPair
    );
    // WAW on the flag unit: a second compare cannot cross the first.
    let second_compare = mutated(target, |function, environment| {
        let compare = environment
            .constraint(environment.selected_keys().compare_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[4] = instruction(
            BOOLEAN,
            SelectedInstructionKind::CompareI64,
            &compare,
            &[FIRST, SECOND],
        );
    });
    assert_eq!(
        schedule(&second_compare, &environment, COMPARE, BOOLEAN).unwrap_err(),
        LocalScheduleError::UnsupportedPair
    );
    // WAR on the flag unit: a flag reader cannot cross a later flag writer.
    let reader_first = mutated(target, |function, _| {
        let body = &mut function.blocks[0].instructions;
        body.swap(3, 4);
    });
    assert_eq!(
        schedule(&reader_first, &environment, BOOLEAN, COMPARE).unwrap_err(),
        LocalScheduleError::UnsupportedPair
    );
    // Two flag readers share no written location and interchange.
    let two_readers = mutated(target, |function, environment| {
        let boolean = environment
            .constraint(environment.selected_keys().materialize_boolean)
            .unwrap()
            .clone();
        function.blocks[0].instructions[3] = instruction(
            COMPARE,
            SelectedInstructionKind::MaterializeBooleanU64LessThan,
            &boolean,
            &[TOTAL],
        );
    });
    let result = schedule(&two_readers, &environment, COMPARE, BOOLEAN).unwrap();
    let body = &result.transformed().functions[0].blocks[0].instructions;
    assert_eq!(body[3].id, BOOLEAN);
    assert_eq!(body[4].id, COMPARE);
}

/// A clobber is a unit write: a flag-clobbering subtraction cannot cross the
/// compare that publishes those flags, nor can it cross a flag reader that
/// must observe the compare's publication.
#[test]
fn clobbers_couple_like_definitions() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let subtract_kind = SelectedInstructionKind::ExactSubtractI64 {
        obligation: ObligationId::new(11).unwrap(),
        accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes([7; 32]),
    };
    // The subtract clobbers the flag unit the compare also defines: two
    // writers of one location never exchange.
    let before_compare = mutated(target, |function, environment| {
        let subtract = environment
            .constraint(environment.selected_keys().subtract_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[2] =
            instruction(SUM, subtract_kind, &subtract, &[FIRST, SECOND, TOTAL]);
    });
    assert_eq!(
        schedule(&before_compare, &environment, SUM, COMPARE).unwrap_err(),
        LocalScheduleError::UnsupportedPair
    );
    // The subtract clobbers the flag unit the boolean materialization reads:
    // moving the write ahead of the read would change the observed flags.
    let before_reader = mutated(target, |function, environment| {
        let subtract = environment
            .constraint(environment.selected_keys().subtract_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[3] =
            instruction(COMPARE, subtract_kind, &subtract, &[FIRST, SECOND, TOTAL]);
    });
    assert_eq!(
        schedule(&before_reader, &environment, COMPARE, BOOLEAN).unwrap_err(),
        LocalScheduleError::UnsupportedPair
    );
}

/// A boundary settlement at the interior index observes the order between
/// the pair and refuses; positions at or outside the pair's span keep the
/// same executed set and admit.
#[test]
fn interior_settlement_bounds_the_interchange() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let interior = mutated(target, |function, _| {
        function.boundary_settlements.push(settlement(1, 41));
    });
    assert_eq!(
        schedule(&interior, &environment, MAT_A, MAT_B).unwrap_err(),
        LocalScheduleError::UnsupportedPair
    );
    for position in [0, 2, 4] {
        let outside = mutated(target, |function, _| {
            function.boundary_settlements.push(settlement(position, 41));
        });
        schedule(&outside, &environment, MAT_A, MAT_B).unwrap();
    }
    // A settlement at the interior index of a different pair does not bound
    // this one: position 3 sits between the compare and the boolean, not
    // between the two materializations.
    let other = mutated(target, |function, _| {
        function.boundary_settlements.push(settlement(3, 41));
    });
    schedule(&other, &environment, MAT_A, MAT_B).unwrap();
}

/// A roster-accounted load interchanges with a pure materialization: the
/// materialization cannot observe memory, so the recorded access order is
/// preserved.
#[test]
fn accounted_memory_actor_crosses_pure_work() {
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
    let result = schedule(&source, &environment, MAT_A, MAT_B).unwrap();
    let body = &result.transformed().functions[0].blocks[0].instructions;
    assert_eq!(body[0].id, MAT_B);
    assert_eq!(body[1].id, MAT_A);
    assert_eq!(
        result.transformed().functions[0].memory_accesses,
        source.transformed().functions[0].memory_accesses
    );
}

/// Two roster-carrying accesses would need a place-alias decision this step
/// does not take: the pair refuses even with disjoint roles.
#[test]
fn two_memory_actors_keep_order() {
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
        schedule(&source, &environment, MAT_A, MAT_B).unwrap_err(),
        LocalScheduleError::UnsupportedPair
    );
}

/// A memory-capable kind without a roster row is an unaccounted access: the
/// interchange cannot prove what it reaches, so it refuses outright. Adding
/// the row makes it the pair's single accounted actor and admits.
#[test]
fn unaccounted_memory_kind_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let bare = mutated(target, |function, environment| {
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
        schedule(&bare, &environment, MAT_A, MAT_B).unwrap_err(),
        LocalScheduleError::UnsupportedInstruction
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
    schedule(&accounted, &environment, MAT_A, MAT_B).unwrap();
}

/// Calls, hosted effects, and terminator kinds are barriers on either member
/// of the pair, and a call-roster row makes the member a barrier even when
/// its kind is register-pure.
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
        let source = mutated(target, |function, _| {
            function.blocks[0].instructions[0].kind = kind;
        });
        assert_eq!(
            schedule(&source, &environment, MAT_A, MAT_B).unwrap_err(),
            LocalScheduleError::UnsupportedInstruction,
            "{kind:?}"
        );
    }
    // The barrier binds the later member identically.
    let later_barrier = mutated(target, |function, _| {
        function.blocks[0].instructions[1].kind = SelectedInstructionKind::Jump;
    });
    assert_eq!(
        schedule(&later_barrier, &environment, MAT_A, MAT_B).unwrap_err(),
        LocalScheduleError::UnsupportedInstruction
    );
    let call_row =
        mutated(target, |function, _| {
            function.calls.push(SelectedCallContract {
            instruction: MAT_A,
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
        schedule(&call_row, &environment, MAT_A, MAT_B).unwrap_err(),
        LocalScheduleError::UnsupportedInstruction
    );
}

/// The pair must be two neighboring body instructions in the named order: a
/// non-neighbor, the reversed order, one repeated id, a trailing member with
/// no successor, and an unknown id all refuse.
#[test]
fn only_the_adjacent_named_pair_interchanges() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // Not neighbors.
    assert_eq!(
        schedule(&source, &environment, MAT_A, SUM).unwrap_err(),
        LocalScheduleError::UnsupportedPair
    );
    // Reversed order: MAT_B's successor is the sum, not MAT_A.
    assert_eq!(
        schedule(&source, &environment, MAT_B, MAT_A).unwrap_err(),
        LocalScheduleError::UnsupportedPair
    );
    // The last body instruction has no successor to interchange with.
    assert_eq!(
        schedule(&source, &environment, BOOLEAN, MAT_A).unwrap_err(),
        LocalScheduleError::UnsupportedPair
    );
    // One id cannot name both members.
    assert_eq!(
        schedule(&source, &environment, MAT_A, MAT_A).unwrap_err(),
        LocalScheduleError::UnsupportedPair
    );
    // An unknown id or function index never locates the pair.
    assert_eq!(
        schedule(&source, &environment, SelectedInstructionId(99), MAT_B).unwrap_err(),
        LocalScheduleError::SourceMismatch
    );
    assert_eq!(
        schedule_selected_pair(&source, 1, MAT_A, MAT_B, &environment, budget()).unwrap_err(),
        LocalScheduleError::SourceMismatch
    );
    // A terminator-carried instruction is not a body member.
    assert_eq!(
        schedule(&source, &environment, TERMINAL, MAT_A).unwrap_err(),
        LocalScheduleError::SourceMismatch
    );
}

/// The interchange is not confined to the entry block: the same pair
/// exchanges in a later block, found by the function-wide scan, while the
/// crossed block stays bit-identical.
#[test]
fn pair_in_a_later_block_interchanges() {
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
        // The entry block keeps one instruction and jumps to a second block
        // carrying the whole original body and the return.
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
    let result = schedule(&source, &environment, MAT_A, MAT_B).unwrap();
    let moved = &result.transformed().functions[0].blocks[1].instructions;
    assert_eq!(moved[0].id, MAT_B);
    assert_eq!(moved[1].id, MAT_A);
    // The crossed entry block stays bit-identical.
    assert_eq!(
        result.transformed().functions[0].blocks[0],
        source.transformed().functions[0].blocks[0]
    );
}

/// A flag-reading terminator still observes the same publisher when the pair
/// ending the body interchanges: neither member writes condition state, so
/// the compare remains the last flag definition before the branch.
#[test]
fn pair_before_a_flag_reading_terminator_interchanges() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        // [mat_a, mat_b, compare, boolean, sum] with a flag-reading branch.
        let body = &mut function.blocks[0].instructions;
        body.swap(2, 4);
        body.swap(2, 3);
        let branch = environment
            .constraint(environment.selected_keys().conditional_branch)
            .unwrap()
            .clone();
        function.blocks[0].terminator = SelectedTerminator::ConditionalBranch {
            instruction: instruction(
                TERMINAL,
                SelectedInstructionKind::ConditionalBranchNonZero,
                &branch,
                &[],
            ),
            when_nonzero: successor(SelectedBlockId(0), BlockId::new(1).unwrap(), 2),
            when_zero: successor(SelectedBlockId(0), BlockId::new(1).unwrap(), 3),
        };
    });
    let result = schedule(&source, &environment, BOOLEAN, SUM).unwrap();
    let body = &result.transformed().functions[0].blocks[0].instructions;
    assert_eq!(body[3].id, SUM);
    assert_eq!(body[4].id, BOOLEAN);
}

/// Validation re-derives the admission and compares by content: a proposal
/// that leaves the pair in place, interchanges a different pair, or carries
/// an unrelated edit each fails replay even though each is a complete plan.
#[test]
fn replay_rejects_anything_but_the_interchange() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // The source itself is not a proposal: the pair was never exchanged.
    assert_eq!(
        validate_local_schedule(
            &source,
            0,
            MAT_A,
            MAT_B,
            &environment,
            budget(),
            source.transformed().clone(),
        )
        .unwrap_err(),
        LocalScheduleError::ReplayMismatch
    );
    // A different interchange is not this one.
    let mut other = source.transformed().clone();
    other.functions[0].blocks[0].instructions.swap(2, 3);
    assert_eq!(
        validate_local_schedule(&source, 0, MAT_A, MAT_B, &environment, budget(), other)
            .unwrap_err(),
        LocalScheduleError::ReplayMismatch
    );
    // The right interchange plus an unrelated extra edit still fails restore.
    let mut extra = source.transformed().clone();
    extra.functions[0].blocks[0].instructions.swap(0, 1);
    extra.functions[0].blocks[0].instructions[4]
        .provenance
        .operations = vec![OperationId::new(77).unwrap()];
    assert_eq!(
        validate_local_schedule(&source, 0, MAT_A, MAT_B, &environment, budget(), extra)
            .unwrap_err(),
        LocalScheduleError::ReplayMismatch
    );
    // The honest interchange replays.
    let result = schedule(&source, &environment, MAT_A, MAT_B).unwrap();
    validate_local_schedule(
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

/// The admission walk is bounded by the validation budget: a plan whose scan
/// cost exceeds it refuses rather than running unbounded.
#[test]
fn work_budget_bounds_the_scan() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    assert_eq!(
        schedule_selected_pair(
            &source,
            0,
            MAT_A,
            MAT_B,
            &environment,
            OptimizationWorkBudget::new(100, 100, 1, 100, 100).unwrap(),
        )
        .unwrap_err(),
        LocalScheduleError::WorkBudgetExceeded
    );
}

/// The environment's target must be the plan's target: a schedule proven for
/// one target's constraints is not evidence on another.
#[test]
fn target_mismatch_rejects() {
    let source = fixture(NativeTarget::linux_x64());
    let environment = baseline_target_register_environment(NativeTarget::linux_arm64()).unwrap();
    assert_eq!(
        schedule(&source, &environment, MAT_A, MAT_B).unwrap_err(),
        LocalScheduleError::SourceMismatch
    );
}

/// The measured validation-step boundary: admission charges one step per
/// block plus one per instruction across the plan, then one per operand,
/// implicit use, implicit definition, and clobber row on each member, then
/// the admitted function's memory, call, and settlement roster lengths — so
/// the exact count admits the interchange on both the proposal and the
/// independent replay path while one step below rejects both, at three
/// fixture sizes that each grow a different term of the charge.
#[test]
fn measured_validation_step_boundary_admits_and_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The pair moves to a second block behind a one-instruction entry: the
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
    // The earlier member becomes a roster-carrying load: its second operand
    // joins the member-surface term and the memory-access row joins the
    // roster term.
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
    for (source, exact_steps) in [
        // (1 block + 5 instructions) + (1 operand per materialization) = 8.
        (fixture(target), 8u64),
        // (2 blocks + 6 instructions) + (1 operand per materialization) = 10.
        (later_block, 10u64),
        // (1 block + 5 instructions) + (2 + 1 member operands) + (1 roster
        // row) = 10.
        (roster_actor, 10u64),
    ] {
        let exact = OptimizationWorkBudget::new(1, 1, exact_steps, 1, 1).unwrap();
        let result = schedule_selected_pair(&source, 0, MAT_A, MAT_B, &environment, exact).unwrap();
        validate_local_schedule(
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
            schedule_selected_pair(&source, 0, MAT_A, MAT_B, &environment, starved).unwrap_err(),
            LocalScheduleError::WorkBudgetExceeded
        );
        assert_eq!(
            validate_local_schedule(
                &source,
                0,
                MAT_A,
                MAT_B,
                &environment,
                starved,
                result.transformed().clone(),
            )
            .unwrap_err(),
            LocalScheduleError::WorkBudgetExceeded
        );
    }
}

/// Two runs over the identical source produce the identical validated
/// result, and the published plan is a legal second input through the sealed
/// analysis boundary: on the transformed plan the pair's order is flipped,
/// so naming the stale order refuses, the pair in its new order interchanges
/// back to restore the source plan bit-identically, a hazard-coupled pair
/// still declines, and a different independent pair still admits.
#[test]
fn interchange_is_deterministic_and_re_admitted() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let first = schedule(&source, &environment, MAT_A, MAT_B).unwrap();
    let second = schedule(&source, &environment, MAT_A, MAT_B).unwrap();
    assert_eq!(first, second);
    // The validated output carries the sealed analysis boundary, so it is a
    // legal second input — not merely a reconstruction of one. The pair's
    // order flipped there, so the stale ordering no longer names an
    // adjacent pair in that order.
    assert_eq!(
        schedule_selected_pair(&first, 0, MAT_A, MAT_B, &environment, budget()).unwrap_err(),
        LocalScheduleError::UnsupportedPair
    );
    // The pair in its new order interchanges back through the same
    // admission and replay, restoring the published source bit-identically.
    let restored = schedule_selected_pair(&first, 0, MAT_B, MAT_A, &environment, budget()).unwrap();
    assert_eq!(restored.transformed(), source.transformed());
    assert_eq!(
        restored.receipt().transformed_selected(),
        source.selected_identity()
    );
    validate_local_schedule(
        &first,
        0,
        MAT_B,
        MAT_A,
        &environment,
        budget(),
        restored.transformed().clone(),
    )
    .unwrap();
    // A hazard-coupled pair still declines on the second input: the sum
    // reads the register the swapped materialization defines.
    assert_eq!(
        schedule_selected_pair(&first, 0, MAT_A, SUM, &environment, budget()).unwrap_err(),
        LocalScheduleError::UnsupportedPair
    );
    // And a different independent pair still admits on the second input.
    let composed = schedule_selected_pair(&first, 0, SUM, COMPARE, &environment, budget()).unwrap();
    let body = &composed.transformed().functions[0].blocks[0].instructions;
    assert_eq!(body[2].id, COMPARE);
    assert_eq!(body[3].id, SUM);
}

/// Replay corruption in a block the interchange never touched still rejects:
/// the restore-by-content check compares the complete plan, not just the
/// block carrying the swapped pair.
#[test]
fn replay_rejects_drift_outside_the_interchanged_block() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Stretch the fixture across one edge: a one-instruction entry block
    // jumps to the block carrying the pair and the return.
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
    let result = schedule(&source, &environment, MAT_A, MAT_B).unwrap();
    assert_eq!(result.transformed().functions[0].blocks.len(), 2);
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
        validate_local_schedule(&source, 0, MAT_A, MAT_B, &environment, budget(), proposed)
            .unwrap_err(),
        LocalScheduleError::ReplayMismatch
    );
    // A changed literal in the untouched entry block rejects.
    let mut proposed = result.transformed().clone();
    proposed.functions[0].blocks[0].instructions[0].kind =
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(12),
        };
    assert_eq!(
        validate_local_schedule(&source, 0, MAT_A, MAT_B, &environment, budget(), proposed)
            .unwrap_err(),
        LocalScheduleError::ReplayMismatch
    );
}

const MAT_C: SelectedInstructionId = SelectedInstructionId(8);
const MAT_D: SelectedInstructionId = SelectedInstructionId(9);
const THIRD: VirtualRegisterId = VirtualRegisterId(5);
const FOURTH: VirtualRegisterId = VirtualRegisterId(6);

/// The fixture with `middle` independent materializations inserted between
/// the two named members: the pair sits at positions 0 and `middle + 1` and
/// every interior instruction is pure register work observing neither of
/// the members' registers.
fn windowed(target: NativeTarget, middle: usize) -> ValidatedLocalSchedule {
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

/// Two members with independent instructions between them exchange places
/// on every target: each crossed instruction keeps its own position,
/// identity, kind, operands, and provenance while the named pair trades the
/// window's endpoints.
#[test]
fn windowed_pair_interchanges_across_inert_interior() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        for middle in 1..=2usize {
            let source = windowed(target, middle);
            let result = schedule(&source, &environment, MAT_A, MAT_B).unwrap();
            let original = &source.transformed().functions[0].blocks[0].instructions;
            let body = &result.transformed().functions[0].blocks[0].instructions;
            assert_eq!(body.len(), original.len());
            assert_eq!(body[0].id, MAT_B);
            assert_eq!(body[middle + 1].id, MAT_A);
            for position in 1..=middle {
                assert_eq!(body[position], original[position]);
            }
            validate_local_schedule(
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

/// An interior instruction participates in the hazard audit in both
/// directions: a member cannot cross an interior instruction reading the
/// register it defines, an interior reader cannot jump its publisher, and
/// the named members' own hazards apply unchanged at any distance.
#[test]
fn windowed_hazards_keep_order() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The earlier member's write feeds an interior read: MAT_B defines
    // SECOND and the sum between it and the compare reads it.
    let source = fixture(target);
    assert_eq!(
        schedule(&source, &environment, MAT_B, COMPARE).unwrap_err(),
        LocalScheduleError::UnsupportedPair
    );
    // The later member's read observes the earlier member's write across
    // the same interior: the compare reads FIRST which MAT_A defines.
    assert_eq!(
        schedule(&source, &environment, MAT_A, COMPARE).unwrap_err(),
        LocalScheduleError::UnsupportedPair
    );
    // Interior readers bind identically: the compare and the sum inside
    // this window read registers MAT_B defines, so MAT_B cannot trade
    // places with the boolean materialization across them.
    let interior_reader = mutated(target, |function, environment| {
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
            3,
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
        schedule(&interior_reader, &environment, MAT_B, BOOLEAN).unwrap_err(),
        LocalScheduleError::UnsupportedPair
    );
    // The members' own hazard applies at distance: an inert materialization
    // between the compare and the boolean reader does not weaken the flag
    // hazard between the named pair.
    let flag_window = mutated(target, |function, environment| {
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
        schedule(&flag_window, &environment, COMPARE, BOOLEAN).unwrap_err(),
        LocalScheduleError::UnsupportedPair
    );
    // The named later instruction behind the earlier one names no
    // in-block window in that order.
    assert_eq!(
        schedule(&flag_window, &environment, MAT_B, MAT_A).unwrap_err(),
        LocalScheduleError::UnsupportedPair
    );
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
        schedule(&other_block, &environment, MAT_A, MAT_B).unwrap_err(),
        LocalScheduleError::UnsupportedPair
    );
}

/// A `windowed` source with the settlement at `position` added: the fixture
/// helper's `mutated` wrapper cannot push settlements after building the
/// window, so this adds the row and refreshes the plan identity directly.
fn windowed_settled(target: NativeTarget, middle: usize, position: u32) -> ValidatedLocalSchedule {
    let mut source = windowed(target, middle);
    std::sync::Arc::make_mut(&mut source.transformed).functions[0]
        .boundary_settlements
        .push(settlement(position, 41));
    let identity = selected_instruction_plan_identity(&source.transformed);
    source.receipt.source_selected = identity;
    source.receipt.transformed_selected = identity;
    source
}

/// Every boundary settlement inside the window's span — between the earlier
/// member and any crossed position or before the later member itself —
/// observes a different executed set once the pair trades places, while
/// settlements at or outside the span admit.
#[test]
fn windowed_settlements_bound_the_span() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for position in [1, 2] {
        assert_eq!(
            schedule(
                &windowed_settled(target, 1, position),
                &environment,
                MAT_A,
                MAT_B
            )
            .unwrap_err(),
            LocalScheduleError::UnsupportedPair,
            "settlement at {position}"
        );
    }
    for position in [0, 3] {
        schedule(
            &windowed_settled(target, 1, position),
            &environment,
            MAT_A,
            MAT_B,
        )
        .unwrap();
    }
}

/// The window's schedulable bar binds the interior too: a call, hosted
/// effect, or call-roster row between the members refuses, a roster-
/// carrying interior refuses once a member carries rows, and an interior
/// access stays accounted while two memory-inert members trade places
/// around it.
#[test]
fn windowed_barriers_and_memory_actors_bound_the_span() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A barrier kind inside the window refuses outright.
    let interior_call = mutated(target, |function, environment| {
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
        let mut interior = instruction(
            MAT_C,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(13),
            },
            &materialize,
            &[THIRD],
        );
        interior.kind = SelectedInstructionKind::CallUnit {
            callee: MachineId::new(9).unwrap(),
        };
        function.blocks[0].instructions.insert(1, interior);
    });
    assert_eq!(
        schedule(&interior_call, &environment, MAT_A, MAT_B).unwrap_err(),
        LocalScheduleError::UnsupportedInstruction
    );
    // A roster-carrying member cannot cross a roster-carrying interior:
    // the earlier load's recorded access would trade order with the
    // interior load's.
    let two_actors = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
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
        function.blocks[0].instructions[0] = instruction(
            MAT_A,
            SelectedInstructionKind::Load8 { byte_offset: 0 },
            &load,
            &[POINTER, FIRST],
        );
        function.blocks[0].instructions.insert(
            1,
            instruction(
                MAT_C,
                SelectedInstructionKind::Load8 { byte_offset: 8 },
                &load,
                &[POINTER, THIRD],
            ),
        );
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
        schedule(&two_actors, &environment, MAT_A, MAT_B).unwrap_err(),
        LocalScheduleError::UnsupportedPair
    );
    // Two memory-inert members may trade places around an interior access
    // that keeps its position and roster rows.
    let interior_actor = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
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
            1,
            instruction(
                MAT_C,
                SelectedInstructionKind::Load8 { byte_offset: 0 },
                &load,
                &[POINTER, THIRD],
            ),
        );
        function.memory_accesses.push(access(
            MAT_C,
            PlaceId::new(1).unwrap(),
            SelectedMemoryAccessRole::ReadPlace,
        ));
    });
    let result = schedule(&interior_actor, &environment, MAT_A, MAT_B).unwrap();
    let body = &result.transformed().functions[0].blocks[0].instructions;
    assert_eq!(body[0].id, MAT_B);
    assert_eq!(body[1].id, MAT_C);
    assert_eq!(body[2].id, MAT_A);
    assert_eq!(
        result.transformed().functions[0].memory_accesses,
        interior_actor.transformed().functions[0].memory_accesses
    );
    validate_local_schedule(
        &interior_actor,
        0,
        MAT_A,
        MAT_B,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
}

/// Two runs over the identical windowed source produce the identical
/// validated result, and the published plan is a legal second input: naming
/// the stale order refuses, the flipped order interchanges back through the
/// same admission and replay to restore the source bit-identically, and a
/// different independent pair still admits on the transformed plan.
#[test]
fn windowed_interchange_is_deterministic_and_an_involution() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = windowed(target, 2);
    let first = schedule(&source, &environment, MAT_A, MAT_B).unwrap();
    let second = schedule(&source, &environment, MAT_A, MAT_B).unwrap();
    assert_eq!(first, second);
    // The window's endpoints traded places: the stale order names no
    // in-block window in the published plan.
    assert_eq!(
        schedule_selected_pair(&first, 0, MAT_A, MAT_B, &environment, budget()).unwrap_err(),
        LocalScheduleError::UnsupportedPair
    );
    // The flipped order interchanges back, restoring the source
    // bit-identically and replaying through independent validation.
    let restored = schedule_selected_pair(&first, 0, MAT_B, MAT_A, &environment, budget()).unwrap();
    assert_eq!(restored.transformed(), source.transformed());
    assert_eq!(
        restored.receipt().transformed_selected(),
        source.selected_identity()
    );
    validate_local_schedule(
        &first,
        0,
        MAT_B,
        MAT_A,
        &environment,
        budget(),
        restored.transformed().clone(),
    )
    .unwrap();
    // A different pair still admits on the second input: the interior
    // materializations remained in place and independent.
    let composed = schedule_selected_pair(&first, 0, MAT_C, MAT_D, &environment, budget()).unwrap();
    let body = &composed.transformed().functions[0].blocks[0].instructions;
    assert_eq!(body[1].id, MAT_D);
    assert_eq!(body[2].id, MAT_C);
}

/// Replay binds the whole window, not only its endpoints: a proposal that
/// swaps the named pair but reorders, edits, or drops an interior
/// instruction fails the restore-by-content check.
#[test]
fn windowed_replay_rejects_interior_drift() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = windowed(target, 2);
    let result = schedule(&source, &environment, MAT_A, MAT_B).unwrap();
    // The endpoints are exactly the admitted interchange, but the interior
    // materializations traded places — not the proven proposal.
    let mut permuted = result.transformed().clone();
    permuted.functions[0].blocks[0].instructions.swap(1, 2);
    assert_eq!(
        validate_local_schedule(&source, 0, MAT_A, MAT_B, &environment, budget(), permuted)
            .unwrap_err(),
        LocalScheduleError::ReplayMismatch
    );
    // A drifted literal on an interior member rejects identically.
    let mut edited = result.transformed().clone();
    edited.functions[0].blocks[0].instructions[1].kind = SelectedInstructionKind::MaterializeI64 {
        value: IntegerValue::Unsigned(99),
    };
    assert_eq!(
        validate_local_schedule(&source, 0, MAT_A, MAT_B, &environment, budget(), edited)
            .unwrap_err(),
        LocalScheduleError::ReplayMismatch
    );
    // An interior member removed outright rejects.
    let mut dropped = result.transformed().clone();
    dropped.functions[0].blocks[0].instructions.remove(1);
    assert_eq!(
        validate_local_schedule(&source, 0, MAT_A, MAT_B, &environment, budget(), dropped)
            .unwrap_err(),
        LocalScheduleError::ReplayMismatch
    );
}

/// The measured validation-step boundary charges the whole window's member
/// surfaces, not just the named pair: the plan scan plus one step per
/// operand, implicit use, implicit definition, and clobber row on every
/// crossed instruction, then the function's roster lengths — so the exact
/// count admits the interchange on both the proposal and the independent
/// replay path while one step below rejects both, at two window sizes.
#[test]
fn windowed_measured_validation_step_boundary_admits_and_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for (source, exact_steps) in [
        // (1 block + 6 instructions) + (3 window materializations) = 10.
        (windowed(target, 1), 10u64),
        // (1 block + 7 instructions) + (4 window materializations) = 12.
        (windowed(target, 2), 12u64),
    ] {
        let exact = OptimizationWorkBudget::new(1, 1, exact_steps, 1, 1).unwrap();
        let result = schedule_selected_pair(&source, 0, MAT_A, MAT_B, &environment, exact).unwrap();
        validate_local_schedule(
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
            schedule_selected_pair(&source, 0, MAT_A, MAT_B, &environment, starved).unwrap_err(),
            LocalScheduleError::WorkBudgetExceeded
        );
        assert_eq!(
            validate_local_schedule(
                &source,
                0,
                MAT_A,
                MAT_B,
                &environment,
                starved,
                result.transformed().clone(),
            )
            .unwrap_err(),
            LocalScheduleError::WorkBudgetExceeded
        );
    }
}
