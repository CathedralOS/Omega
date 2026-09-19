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
    RunRelocationError, RunRelocationReceipt, ValidatedRunRelocation, relocate_selected_run,
    validate_run_relocation,
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
const MAT_C: SelectedInstructionId = SelectedInstructionId(4);
const MAT_D: SelectedInstructionId = SelectedInstructionId(5);
const MAT_B: SelectedInstructionId = SelectedInstructionId(6);
const TERMINAL: SelectedInstructionId = SelectedInstructionId(7);

const POINTER: VirtualRegisterId = VirtualRegisterId(0);
const FIRST: VirtualRegisterId = VirtualRegisterId(1);
const TOTAL: VirtualRegisterId = VirtualRegisterId(2);
const THIRD: VirtualRegisterId = VirtualRegisterId(3);
const FOURTH: VirtualRegisterId = VirtualRegisterId(4);
const SECOND: VirtualRegisterId = VirtualRegisterId(5);

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
/// `r1 = 5; r2 = r1 + r0; r3 = 13; r4 = 14; r5 = 7; return`. The run the
/// tests relocate is the internally coupled `MAT_A; SUM` pair — the sum
/// reads the materialization's result — beside three pure
/// materializations, with `r0` an entry parameter so the sum's second read
/// has no in-body producer.
fn fixture(target: NativeTarget) -> ValidatedRunRelocation {
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
                instruction: MAT_D,
                source_value: ValueId::new(5).unwrap(),
            },
        ),
        register(
            SECOND,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction: MAT_B,
                source_value: ValueId::new(6).unwrap(),
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
            SUM,
            SelectedInstructionKind::WrappingAddI64,
            add,
            &[FIRST, POINTER, TOTAL],
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
            MAT_D,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(14),
            },
            materialize,
            &[FOURTH],
        ),
        instruction(
            MAT_B,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(7),
            },
            materialize,
            &[SECOND],
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
    ValidatedRunRelocation {
        receipt: RunRelocationReceipt {
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
) -> ValidatedRunRelocation {
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
    source: &ValidatedRunRelocation,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    first: SelectedInstructionId,
    last: SelectedInstructionId,
    destination: SelectedInstructionId,
) -> Result<ValidatedRunRelocation, RunRelocationError> {
    relocate_selected_run(source, 0, first, last, destination, environment, budget())
}

/// The internally coupled `MAT_A; SUM` run relocates down across the inert
/// materializations on every target: the run takes the window's trailing
/// edge with its members in their original order — the sum still reads the
/// materialization it followed — while every crossed instruction shifts
/// one run-width earlier keeping its own relative order, identity, kind,
/// operands, and provenance.
#[test]
fn run_relocates_down_across_inert_window() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = fixture(target);
        let result = relocate(&source, &environment, MAT_A, SUM, MAT_B).unwrap();
        let original = &source.transformed().functions[0].blocks[0].instructions;
        let body = &result.transformed().functions[0].blocks[0].instructions;
        assert_eq!(body.len(), original.len());
        // The crossed run shifts one run-width earlier in its original
        // order; the relocated run keeps both members adjacent and ordered.
        assert_eq!(body[0], original[2]);
        assert_eq!(body[1], original[3]);
        assert_eq!(body[2], original[4]);
        assert_eq!(body[3], original[0]);
        assert_eq!(body[4], original[1]);
        validate_run_relocation(
            &source,
            0,
            MAT_A,
            SUM,
            MAT_B,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
    }
}

/// The same window runs in the other direction: a later run relocates up
/// to an earlier destination's position, taking the window's leading edge
/// while the crossed instructions shift one run-width later in their
/// original order.
#[test]
fn run_relocates_up_across_inert_window() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let result = relocate(&source, &environment, MAT_C, MAT_D, MAT_A).unwrap();
    let original = &source.transformed().functions[0].blocks[0].instructions;
    let body = &result.transformed().functions[0].blocks[0].instructions;
    assert_eq!(
        body.iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![MAT_C, MAT_D, MAT_A, SUM, MAT_B]
    );
    assert_eq!(body[0], original[2]);
    assert_eq!(body[1], original[3]);
    // The crossed instructions MAT_A and SUM shifted one run-width later
    // in their original order; MAT_B outside the window stayed put.
    assert_eq!(body[2..4], original[0..2]);
    assert_eq!(body[4], original[4]);
}

/// A three-member run relocates as one body: `MAT_A; SUM; MAT_C` takes the
/// destination's position while the two crossed instructions shift one
/// run-width earlier.
#[test]
fn three_member_run_relocates() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let result = relocate(&source, &environment, MAT_A, MAT_C, MAT_B).unwrap();
    let body = &result.transformed().functions[0].blocks[0].instructions;
    assert_eq!(
        body.iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![MAT_D, MAT_B, MAT_A, SUM, MAT_C]
    );
    validate_run_relocation(
        &source,
        0,
        MAT_A,
        MAT_C,
        MAT_B,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
}

/// An adjacent destination swaps the whole run with its single neighbor:
/// the crossed instruction trades sides with the run while every other
/// position keeps its instruction.
#[test]
fn adjacent_destination_swaps_the_run_with_its_neighbor() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let down = relocate(&source, &environment, MAT_A, SUM, MAT_C).unwrap();
    let body = &down.transformed().functions[0].blocks[0].instructions;
    assert_eq!(
        body.iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![MAT_C, MAT_A, SUM, MAT_D, MAT_B]
    );
    let up = relocate(&source, &environment, MAT_C, MAT_D, SUM).unwrap();
    let body = &up.transformed().functions[0].blocks[0].instructions;
    assert_eq!(
        body.iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![MAT_A, MAT_C, MAT_D, SUM, MAT_B]
    );
}

/// The run's own couplings never cross anything: the sum reads the
/// materialization's result, so neither member can trade places with the
/// other or cross it under the single-member relocation and the pairwise
/// interchange — yet the run lifts both across the window as one body.
#[test]
fn internally_coupled_members_move_as_one_body() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // The producer cannot cross its own consumer in either direction.
    assert_eq!(
        crate::relocate_selected_instruction(&source, 0, MAT_A, SUM, &environment, budget())
            .unwrap_err(),
        crate::LocalRelocationError::UnsupportedPair
    );
    assert_eq!(
        crate::relocate_selected_instruction(&source, 0, SUM, MAT_A, &environment, budget())
            .unwrap_err(),
        crate::LocalRelocationError::UnsupportedPair
    );
    // Nor can the pair interchange under the two-member primitive.
    assert_eq!(
        crate::schedule_selected_pair(&source, 0, MAT_A, SUM, &environment, budget()).unwrap_err(),
        crate::LocalScheduleError::UnsupportedPair
    );
    // And the producer cannot reach the far side of the window alone: the
    // consumer sits inside its window.
    assert_eq!(
        crate::relocate_selected_instruction(&source, 0, MAT_A, MAT_B, &environment, budget())
            .unwrap_err(),
        crate::LocalRelocationError::UnsupportedPair
    );
    let result = relocate(&source, &environment, MAT_A, SUM, MAT_B).unwrap();
    let body = &result.transformed().functions[0].blocks[0].instructions;
    assert_eq!(body[3].id, MAT_A);
    assert_eq!(body[4].id, SUM);
}

/// A member whose write feeds a crossed read keeps order in both run
/// directions: relocating the run behind the consumer would starve it, and
/// relocating it ahead of a crossed redefinition would hand the consumer
/// the wrong value.
#[test]
fn raw_hazard_against_a_crossed_keeps_order() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // MAT_C becomes a copy reading FIRST: the run's first member defines
    // FIRST, so the run cannot cross the copy in either direction.
    let source = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[2] = instruction(
            MAT_C,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[FIRST, THIRD],
        );
    });
    assert_eq!(
        relocate(&source, &environment, MAT_A, SUM, MAT_B).unwrap_err(),
        RunRelocationError::UnsupportedPair
    );
    // The same hazard binds a shorter window: run MAT_A..SUM onto the copy
    // itself.
    assert_eq!(
        relocate(&source, &environment, MAT_A, SUM, MAT_C).unwrap_err(),
        RunRelocationError::UnsupportedPair
    );
}

/// A member reading a location a crossed instruction writes would observe
/// the new value after the move: the sum reads the entry-parameter
/// register, so a crossed redefinition of it keeps the run in place.
#[test]
fn war_hazard_against_a_crossed_keeps_order() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let materialize = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[2] = instruction(
            MAT_C,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(9),
            },
            &materialize,
            &[POINTER],
        );
    });
    assert_eq!(
        relocate(&source, &environment, MAT_A, SUM, MAT_B).unwrap_err(),
        RunRelocationError::UnsupportedPair
    );
    // Moving the POINTER-writing materialization inside the run couples it
    // the other way: member writes meet the crossed sum's read.
    assert_eq!(
        relocate(&source, &environment, MAT_C, MAT_D, MAT_A).unwrap_err(),
        RunRelocationError::UnsupportedPair
    );
    // On the untouched fixture the same window admits.
    let source = fixture(target);
    relocate(&source, &environment, MAT_C, MAT_D, MAT_A).unwrap();
}

/// A member and a crossed instruction writing the same register would
/// change which definition later positions observe.
#[test]
fn waw_hazard_against_a_crossed_keeps_order() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let materialize = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[2] = instruction(
            MAT_C,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(9),
            },
            &materialize,
            &[FIRST],
        );
    });
    assert_eq!(
        relocate(&source, &environment, MAT_A, SUM, MAT_B).unwrap_err(),
        RunRelocationError::UnsupportedPair
    );
}

/// Condition state couples like registers: a flag-clobbering member cannot
/// cross the compare that publishes those flags, and a flag-publishing
/// member cannot cross the materialization reading them.
#[test]
fn condition_state_hazards_keep_order() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let subtract_kind = SelectedInstructionKind::ExactSubtractI64 {
        obligation: ObligationId::new(11).unwrap(),
        accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes([7; 32]),
    };
    // The member clobbers the flag unit the crossed compare defines: two
    // writers of one location never reorder.
    let flag_crossed = mutated(target, |function, environment| {
        let subtract = environment
            .constraint(environment.selected_keys().subtract_i64)
            .unwrap()
            .clone();
        let compare = environment
            .constraint(environment.selected_keys().compare_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[1] =
            instruction(SUM, subtract_kind, &subtract, &[FIRST, POINTER, TOTAL]);
        function.blocks[0].instructions[2] = instruction(
            MAT_C,
            SelectedInstructionKind::CompareI64,
            &compare,
            &[THIRD, FOURTH],
        );
    });
    assert_eq!(
        relocate(&flag_crossed, &environment, MAT_A, SUM, MAT_B).unwrap_err(),
        RunRelocationError::UnsupportedPair
    );
    // A flag-reading crossed materialization cannot be crossed by the
    // clobbering member either.
    let flag_reader = mutated(target, |function, environment| {
        let subtract = environment
            .constraint(environment.selected_keys().subtract_i64)
            .unwrap()
            .clone();
        let boolean = environment
            .constraint(environment.selected_keys().materialize_boolean)
            .unwrap()
            .clone();
        function.blocks[0].instructions[1] =
            instruction(SUM, subtract_kind, &subtract, &[FIRST, POINTER, TOTAL]);
        function.blocks[0].instructions[2] = instruction(
            MAT_C,
            SelectedInstructionKind::MaterializeBooleanEqual,
            &boolean,
            &[THIRD],
        );
    });
    assert_eq!(
        relocate(&flag_reader, &environment, MAT_A, SUM, MAT_B).unwrap_err(),
        RunRelocationError::UnsupportedPair
    );
}

/// Calls, hosted effects, and terminator kinds are barriers as a member or
/// anywhere inside the crossed run — destination included — and a
/// call-roster row makes an instruction a barrier even when its kind is
/// register-pure.
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
            function.blocks[0].instructions[1].kind = kind;
        });
        assert_eq!(
            relocate(&member_barrier, &environment, MAT_A, SUM, MAT_B).unwrap_err(),
            RunRelocationError::UnsupportedInstruction,
            "member {kind:?}"
        );
        let crossed_barrier = mutated(target, |function, _| {
            function.blocks[0].instructions[3].kind = kind;
        });
        assert_eq!(
            relocate(&crossed_barrier, &environment, MAT_A, SUM, MAT_B).unwrap_err(),
            RunRelocationError::UnsupportedInstruction,
            "crossed {kind:?}"
        );
        let destination_barrier = mutated(target, |function, _| {
            function.blocks[0].instructions[4].kind = kind;
        });
        assert_eq!(
            relocate(&destination_barrier, &environment, MAT_A, SUM, MAT_B).unwrap_err(),
            RunRelocationError::UnsupportedInstruction,
            "destination {kind:?}"
        );
    }
    let call_row =
        mutated(target, |function, _| {
            function.calls.push(SelectedCallContract {
            instruction: MAT_D,
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
    // The call-contracted instruction sits inside the crossed run.
    assert_eq!(
        relocate(&call_row, &environment, MAT_A, SUM, MAT_B).unwrap_err(),
        RunRelocationError::UnsupportedInstruction
    );
    // And as a member.
    let call_member =
        mutated(target, |function, _| {
            function.calls.push(SelectedCallContract {
            instruction: SUM,
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
        relocate(&call_member, &environment, MAT_A, SUM, MAT_B).unwrap_err(),
        RunRelocationError::UnsupportedInstruction
    );
}

/// The run is the contiguous span the two named members bound in one
/// block, of at least two members, with the destination outside it: a
/// repeated id, a last member that does not follow the first, a
/// destination inside the run or behind a terminator, an unknown id, and
/// a wrong function index all refuse.
#[test]
fn only_distinct_in_block_runs_relocate() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // A repeated member names a one-instruction run — the single-member
    // relocation's case — not a run.
    assert_eq!(
        relocate(&source, &environment, MAT_A, MAT_A, MAT_B).unwrap_err(),
        RunRelocationError::UnsupportedPair
    );
    // The last member must follow the first in the same block.
    assert_eq!(
        relocate(&source, &environment, MAT_B, MAT_A, MAT_D).unwrap_err(),
        RunRelocationError::UnsupportedPair
    );
    // The destination cannot name a member's own position.
    assert_eq!(
        relocate(&source, &environment, MAT_A, MAT_C, SUM).unwrap_err(),
        RunRelocationError::UnsupportedPair
    );
    // An unknown first member or function index never locates the run.
    assert_eq!(
        relocate(&source, &environment, SelectedInstructionId(99), SUM, MAT_B).unwrap_err(),
        RunRelocationError::SourceMismatch
    );
    assert_eq!(
        relocate_selected_run(&source, 1, MAT_A, SUM, MAT_B, &environment, budget()).unwrap_err(),
        RunRelocationError::SourceMismatch
    );
    // An unknown last member or destination names no in-block window.
    assert_eq!(
        relocate(
            &source,
            &environment,
            MAT_A,
            SelectedInstructionId(99),
            MAT_B
        )
        .unwrap_err(),
        RunRelocationError::UnsupportedPair
    );
    assert_eq!(
        relocate(&source, &environment, MAT_A, SUM, SelectedInstructionId(99)).unwrap_err(),
        RunRelocationError::UnsupportedPair
    );
    // A terminator-carried instruction is not a body position.
    assert_eq!(
        relocate(&source, &environment, MAT_A, SUM, TERMINAL).unwrap_err(),
        RunRelocationError::UnsupportedPair
    );
    assert_eq!(
        relocate(&source, &environment, TERMINAL, SUM, MAT_B).unwrap_err(),
        RunRelocationError::SourceMismatch
    );
    // A last member or destination in a different block names no in-block
    // window.
    let other_block = mutated(target, |function, environment| {
        let keys = environment.selected_keys();
        let jump_row = environment.constraint(keys.jump).unwrap().clone();
        let return_row = environment.constraint(keys.return_unit).unwrap().clone();
        let moved = function.blocks[0].instructions.remove(4);
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
        relocate(&other_block, &environment, MAT_A, SUM, MAT_B).unwrap_err(),
        RunRelocationError::UnsupportedPair
    );
    assert_eq!(
        relocate(&other_block, &environment, MAT_A, MAT_B, MAT_C).unwrap_err(),
        RunRelocationError::UnsupportedPair
    );
}

/// A boundary settlement inside the window's span observes a different
/// executed prefix once the run lands on the other side of the crossed
/// positions; settlements at or outside the span admit.
#[test]
fn interior_settlements_bound_the_window() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Run MAT_A..SUM to MAT_B: window positions 0..=4 — every position
    // after the first through the last sees the run on a different side.
    for position in 1..=4 {
        let source = mutated(target, |function, _| {
            function.boundary_settlements.push(settlement(position, 41));
        });
        assert_eq!(
            relocate(&source, &environment, MAT_A, SUM, MAT_B).unwrap_err(),
            RunRelocationError::UnsupportedPair,
            "settlement at {position}"
        );
    }
    for position in [0, 5] {
        let boundary = mutated(target, |function, _| {
            function.boundary_settlements.push(settlement(position, 41));
        });
        relocate(&boundary, &environment, MAT_A, SUM, MAT_B).unwrap();
    }
    // Moving up binds the same span the other way: run MAT_C..MAT_D to
    // MAT_A crosses positions 0..=3.
    for position in 1..=3 {
        let source = mutated(target, |function, _| {
            function.boundary_settlements.push(settlement(position, 41));
        });
        assert_eq!(
            relocate(&source, &environment, MAT_C, MAT_D, MAT_A).unwrap_err(),
            RunRelocationError::UnsupportedPair,
            "upward settlement at {position}"
        );
    }
    for position in [0, 4] {
        let boundary = mutated(target, |function, _| {
            function.boundary_settlements.push(settlement(position, 41));
        });
        relocate(&boundary, &environment, MAT_C, MAT_D, MAT_A).unwrap();
    }
}

/// A row-less run cannot observe memory under the roster's completeness,
/// so it relocates across any mix of roster-carrying positions while every
/// recorded access keeps its relative order.
#[test]
fn row_less_run_crosses_accounted_positions() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
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
            SUM,
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
            SelectedMemoryAccessRole::ReadPlace,
        ));
        function.memory_accesses.push(access(
            SUM,
            PlaceId::new(2).unwrap(),
            SelectedMemoryAccessRole::WritePlace,
        ));
    });
    // The row-less MAT_C..MAT_D run relocates up across two roster-carrying
    // positions: every recorded access keeps its relative order.
    let result = relocate(&source, &environment, MAT_C, MAT_D, MAT_A).unwrap();
    let body = &result.transformed().functions[0].blocks[0].instructions;
    assert_eq!(
        body.iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![MAT_C, MAT_D, MAT_A, SUM, MAT_B]
    );
    assert_eq!(
        result.transformed().functions[0].memory_accesses,
        source.transformed().functions[0].memory_accesses
    );
}

/// A roster-carrying run may cross only row-less positions: a second
/// accounted actor anywhere in the window would reorder recorded accesses,
/// which needs a place-alias decision this step does not take. Several
/// roster-carrying members inside one run keep their internal order, so a
/// multi-actor run is one moving sequence — not a second actor.
#[test]
fn roster_carrying_run_crosses_only_row_less_positions() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Both members carry rows: the load reads one place and the store
    // writes another, and the run crosses only inert materializations.
    let multi_actor = mutated(target, |function, environment| {
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
            SUM,
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
            SelectedMemoryAccessRole::ReadPlace,
        ));
        function.memory_accesses.push(access(
            SUM,
            PlaceId::new(2).unwrap(),
            SelectedMemoryAccessRole::WritePlace,
        ));
    });
    let result = relocate(&multi_actor, &environment, MAT_A, SUM, MAT_B).unwrap();
    let body = &result.transformed().functions[0].blocks[0].instructions;
    assert_eq!(
        body.iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![MAT_C, MAT_D, MAT_B, MAT_A, SUM]
    );
    assert_eq!(
        result.transformed().functions[0].memory_accesses,
        multi_actor.transformed().functions[0].memory_accesses
    );
    // A second accounted actor inside the crossed run rejects: the store's
    // recorded write would trade order with the member's recorded read.
    let accounted_crossed = mutated(target, |function, environment| {
        let keys = environment.selected_keys();
        let load = environment.constraint(keys.load8.unwrap()).unwrap().clone();
        let store = environment.constraint(keys.store.unwrap()).unwrap().clone();
        function.blocks[0].instructions[0] = instruction(
            MAT_A,
            SelectedInstructionKind::Load8 { byte_offset: 0 },
            &load,
            &[POINTER, FIRST],
        );
        function.blocks[0].instructions[2] = instruction(
            MAT_C,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            &store,
            &[POINTER, THIRD],
        );
        function.memory_accesses.push(access(
            MAT_A,
            PlaceId::new(1).unwrap(),
            SelectedMemoryAccessRole::ReadPlace,
        ));
        function.memory_accesses.push(access(
            MAT_C,
            PlaceId::new(2).unwrap(),
            SelectedMemoryAccessRole::WritePlace,
        ));
    });
    assert_eq!(
        relocate(&accounted_crossed, &environment, MAT_A, SUM, MAT_B).unwrap_err(),
        RunRelocationError::UnsupportedPair
    );
    assert_eq!(
        relocate(&accounted_crossed, &environment, MAT_C, MAT_D, MAT_A).unwrap_err(),
        RunRelocationError::UnsupportedPair
    );
}

/// A memory-capable kind without a roster row is an unaccounted access: the
/// relocation cannot prove what it reaches, so it refuses outright whether
/// the bare access is a member or a crossed instruction. Adding the row
/// makes it the window's only accounted side and admits.
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
        relocate(&bare_member, &environment, MAT_A, SUM, MAT_B).unwrap_err(),
        RunRelocationError::UnsupportedInstruction
    );
    let bare_crossed = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[3] = instruction(
            MAT_D,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            &store,
            &[POINTER, FOURTH],
        );
    });
    assert_eq!(
        relocate(&bare_crossed, &environment, MAT_A, SUM, MAT_B).unwrap_err(),
        RunRelocationError::UnsupportedInstruction
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
    relocate(&accounted, &environment, MAT_A, SUM, MAT_B).unwrap();
}

/// The run relocates inside a later block: window lookup scopes to the
/// block holding the first member, and the untouched entry block and
/// terminator structure survive bit-identical.
#[test]
fn run_in_a_later_block_relocates() {
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
    let result = relocate(&source, &environment, MAT_A, SUM, MAT_B).unwrap();
    assert_eq!(result.transformed().functions[0].blocks.len(), 2);
    let entry = &result.transformed().functions[0].blocks[0];
    assert_eq!(entry, &source.transformed().functions[0].blocks[0]);
    let body = &result.transformed().functions[0].blocks[1].instructions;
    assert_eq!(
        body.iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![MAT_C, MAT_D, MAT_B, MAT_A, SUM]
    );
}

/// Replay consumes only the exact rotation: a proposed block that drops a
/// member, keeps the destination in place, permutes the run or the crossed
/// positions, or carries an unrelated edit all reject.
#[test]
fn replay_rejects_anything_but_the_rotation() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let result = relocate(&source, &environment, MAT_A, SUM, MAT_B).unwrap();
    // The honest proposal replays.
    validate_run_relocation(
        &source,
        0,
        MAT_A,
        SUM,
        MAT_B,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // The run at the right slots with its members permuted rejects: the
    // run must keep its internal order, not reorder.
    let mut permuted = result.transformed().clone();
    permuted.functions[0].blocks[0].instructions.swap(3, 4);
    assert_eq!(
        validate_run_relocation(
            &source,
            0,
            MAT_A,
            SUM,
            MAT_B,
            &environment,
            budget(),
            permuted
        )
        .unwrap_err(),
        RunRelocationError::ReplayMismatch
    );
    // The crossed run permuted rejects the same way.
    let mut permuted = result.transformed().clone();
    permuted.functions[0].blocks[0].instructions.swap(0, 1);
    assert_eq!(
        validate_run_relocation(
            &source,
            0,
            MAT_A,
            SUM,
            MAT_B,
            &environment,
            budget(),
            permuted
        )
        .unwrap_err(),
        RunRelocationError::ReplayMismatch
    );
    // A pairwise interchange is not the rotation: interior positions must
    // shift, not keep the endpoints' trade.
    let mut transposed = source.transformed().clone();
    transposed.functions[0].blocks[0].instructions.swap(0, 4);
    assert_eq!(
        validate_run_relocation(
            &source,
            0,
            MAT_A,
            SUM,
            MAT_B,
            &environment,
            budget(),
            transposed
        )
        .unwrap_err(),
        RunRelocationError::ReplayMismatch
    );
    // A proposal missing an instruction rejects.
    let mut dropped = result.transformed().clone();
    dropped.functions[0].blocks[0].instructions.pop();
    assert_eq!(
        validate_run_relocation(
            &source,
            0,
            MAT_A,
            SUM,
            MAT_B,
            &environment,
            budget(),
            dropped
        )
        .unwrap_err(),
        RunRelocationError::ReplayMismatch
    );
    // An unrelated literal edit inside the relocated block rejects.
    let mut edited = result.transformed().clone();
    edited.functions[0].blocks[0].instructions[0].kind = SelectedInstructionKind::MaterializeI64 {
        value: IntegerValue::Unsigned(12),
    };
    assert_eq!(
        validate_run_relocation(
            &source,
            0,
            MAT_A,
            SUM,
            MAT_B,
            &environment,
            budget(),
            edited
        )
        .unwrap_err(),
        RunRelocationError::ReplayMismatch
    );
    // Naming the stale order or a different destination on the same
    // proposal re-derives a different window and rejects.
    assert_eq!(
        validate_run_relocation(
            &source,
            0,
            MAT_C,
            MAT_D,
            MAT_A,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap_err(),
        RunRelocationError::ReplayMismatch
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
    let result = relocate(&source, &environment, MAT_A, SUM, MAT_B).unwrap();
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
        validate_run_relocation(
            &source,
            0,
            MAT_A,
            SUM,
            MAT_B,
            &environment,
            budget(),
            proposed
        )
        .unwrap_err(),
        RunRelocationError::ReplayMismatch
    );
    // A changed literal in the untouched entry block rejects.
    let mut proposed = result.transformed().clone();
    proposed.functions[0].blocks[0].instructions[0].kind =
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(12),
        };
    assert_eq!(
        validate_run_relocation(
            &source,
            0,
            MAT_A,
            SUM,
            MAT_B,
            &environment,
            budget(),
            proposed
        )
        .unwrap_err(),
        RunRelocationError::ReplayMismatch
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
        relocate_selected_run(
            &source,
            0,
            MAT_A,
            SUM,
            MAT_B,
            &environment,
            OptimizationWorkBudget::new(100, 100, 1, 100, 100).unwrap(),
        )
        .unwrap_err(),
        RunRelocationError::WorkBudgetExceeded
    );
}

/// The environment's target must be the plan's target: a relocation proven
/// for one target's constraints is not evidence on another.
#[test]
fn target_mismatch_rejects() {
    let source = fixture(NativeTarget::linux_x64());
    let environment = baseline_target_register_environment(NativeTarget::linux_arm64()).unwrap();
    assert_eq!(
        relocate(&source, &environment, MAT_A, SUM, MAT_B).unwrap_err(),
        RunRelocationError::SourceMismatch
    );
}

/// The measured validation-step boundary: admission charges one step per
/// block plus one per instruction across the plan, then every
/// member-against-crossed pair's operand and unit surface, then the
/// admitted function's memory, call, and settlement roster lengths — so
/// the exact count admits the relocation on both the proposal and the
/// independent replay path while one step below rejects both, at fixture
/// sizes that each grow a different term of the charge.
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
    // The run's first member becomes a roster-carrying load: its second
    // operand joins the window-surface term of every pair it sits in, and
    // the memory-access row joins the roster term.
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
    for (source, first, last, destination, exact_steps) in [
        // (1 block + 5 instructions) + (two members × three crossed: the
        // materialization surfaces 1+1+1 against the add's 3 and the two
        // materializations' 1+1) + no rosters = 6 + 18 = 24.
        (fixture(target), MAT_A, SUM, MAT_B, 24u64),
        // The three-member window: (1 block + 5 instructions) + (three
        // members × two crossed: 2 × (1+3+1) member surfaces + 3 × (1+1)
        // crossed surfaces) = 6 + 16 = 22.
        (fixture(target), MAT_A, MAT_C, MAT_B, 22u64),
        // (2 blocks + 6 instructions) + (two members × three crossed) +
        // no rosters = 8 + 18 = 26.
        (later_block, MAT_A, SUM, MAT_B, 26u64),
        // (1 block + 5 instructions) + (load's 2 and add's 3 against the
        // three materializations' 1 each) + (1 roster row) = 6 + 21 + 1 =
        // 28.
        (roster_actor, MAT_A, SUM, MAT_B, 28u64),
    ] {
        let exact = OptimizationWorkBudget::new(1, 1, exact_steps, 1, 1).unwrap();
        let result =
            relocate_selected_run(&source, 0, first, last, destination, &environment, exact)
                .unwrap();
        validate_run_relocation(
            &source,
            0,
            first,
            last,
            destination,
            &environment,
            exact,
            result.transformed().clone(),
        )
        .unwrap();
        let starved = OptimizationWorkBudget::new(1, 1, exact_steps - 1, 1, 1).unwrap();
        assert_eq!(
            relocate_selected_run(&source, 0, first, last, destination, &environment, starved)
                .unwrap_err(),
            RunRelocationError::WorkBudgetExceeded
        );
        assert_eq!(
            validate_run_relocation(
                &source,
                0,
                first,
                last,
                destination,
                &environment,
                starved,
                result.transformed().clone(),
            )
            .unwrap_err(),
            RunRelocationError::WorkBudgetExceeded
        );
    }
}

/// Two runs over the identical source produce the identical validated
/// result, and the published plan is a legal second input through the
/// sealed analysis boundary: on the transformed plan the window's order
/// rotated, so the inverse run relocation restores the published source
/// bit-identically, a hazard-coupled run still declines, and a different
/// relocation family still admits.
#[test]
fn run_relocation_is_deterministic_and_re_admitted() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let first = relocate(&source, &environment, MAT_A, SUM, MAT_B).unwrap();
    let second = relocate(&source, &environment, MAT_A, SUM, MAT_B).unwrap();
    assert_eq!(first, second);
    // The validated output carries the sealed analysis boundary, so it is
    // a legal second input — not merely a reconstruction of one. The run
    // now sits at the window's trailing edge; relocating it back onto the
    // crossed run's head restores the source bit-identically.
    let restored =
        relocate_selected_run(&first, 0, MAT_A, SUM, MAT_C, &environment, budget()).unwrap();
    assert_eq!(restored.transformed(), source.transformed());
    assert_eq!(
        restored.receipt().transformed_selected(),
        source.selected_identity()
    );
    validate_run_relocation(
        &first,
        0,
        MAT_A,
        SUM,
        MAT_C,
        &environment,
        budget(),
        restored.transformed().clone(),
    )
    .unwrap();
    // A hazard-coupled run still declines on the second input: a member
    // reading a crossed redefinition keeps order.
    let coupled = mutated(target, |function, environment| {
        let materialize = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[2] = instruction(
            MAT_C,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(9),
            },
            &materialize,
            &[POINTER],
        );
    });
    let moved = relocate(&coupled, &environment, MAT_A, MAT_C, MAT_B).unwrap();
    // On the moved plan the POINTER writer trails the sum: relocating the
    // MAT_A..SUM run onto it would hand the sum the redefined value.
    assert_eq!(
        relocate_selected_run(&moved, 0, MAT_A, SUM, MAT_C, &environment, budget()).unwrap_err(),
        RunRelocationError::UnsupportedPair
    );
    // A different scheduling family still admits on the second input.
    let swapped =
        crate::schedule_selected_pair(&first, 0, MAT_C, MAT_D, &environment, budget()).unwrap();
    let body = &swapped.transformed().functions[0].blocks[0].instructions;
    assert_eq!(body[0].id, MAT_D);
    assert_eq!(body[1].id, MAT_C);
}
