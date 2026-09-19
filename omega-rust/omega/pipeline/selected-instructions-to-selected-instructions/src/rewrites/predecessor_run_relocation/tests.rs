use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget};
use optimization_unit::{EffectLink, ValueDefinitionSite};
use register_environment::baseline_target_register_environment;
use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedBoundarySettlement,
    SelectedBoundarySettlementPayload, SelectedCallContract, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan, SelectedMemoryAccess,
    SelectedMemoryAccessOrigin, SelectedMemoryAccessRole, SelectedOperand,
    SelectedStructuralBinding, SelectedStructuralTransport, SelectedSuccessor,
    SelectedSuccessorRole, SelectedTerminator, SelectedValueBinding, SelectedValueTransport,
    VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType,
    IntegerValue, MachineId, ObligationId, OperationId, PlaceId, ScalarType, StructuralCaseId,
    ValueId,
};
use target::NativeTarget;
use target_operations_to_selected_instructions::selected_instruction_plan_identity;
use terminal_psi::{
    CrashCause, CrashRouteBucket, CrashRouteGuard, SemanticFingerprint, TerminalPsiIdentity,
    VocabularyMarker,
};

use super::{
    PredecessorRunRelocationError, PredecessorRunRelocationReceipt,
    ValidatedPredecessorRunRelocation, relocate_selected_run_into_predecessor,
    validate_predecessor_run_relocation,
};
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
const MID: SelectedInstructionId = SelectedInstructionId(3);
const TAIL: SelectedInstructionId = SelectedInstructionId(4);
const JUMP: SelectedInstructionId = SelectedInstructionId(5);
const LEAD: SelectedInstructionId = SelectedInstructionId(6);
const RUN_A: SelectedInstructionId = SelectedInstructionId(7);
const RUN_B: SelectedInstructionId = SelectedInstructionId(8);
const TRAIL: SelectedInstructionId = SelectedInstructionId(9);
const RET: SelectedInstructionId = SelectedInstructionId(10);

const POINTER: VirtualRegisterId = VirtualRegisterId(0);
const R_HEAD: VirtualRegisterId = VirtualRegisterId(1);
const R_MID: VirtualRegisterId = VirtualRegisterId(2);
const R_TAIL: VirtualRegisterId = VirtualRegisterId(3);
const R_LEAD: VirtualRegisterId = VirtualRegisterId(4);
const R_MOVE_A: VirtualRegisterId = VirtualRegisterId(5);
const R_MOVE_B: VirtualRegisterId = VirtualRegisterId(6);
const R_TRAIL: VirtualRegisterId = VirtualRegisterId(7);
const R_BOUND: VirtualRegisterId = VirtualRegisterId(8);

const BLOCK_A: SelectedBlockId = SelectedBlockId(0);
const BLOCK_B: SelectedBlockId = SelectedBlockId(1);
const EDGE_AB: u64 = 10;

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

fn settlement(block: SelectedBlockId, position: u32, operation: u64) -> SelectedBoundarySettlement {
    SelectedBoundarySettlement {
        block,
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

fn edge_access(instruction: SelectedInstructionId, edge: u64) -> SelectedMemoryAccess {
    SelectedMemoryAccess {
        instruction,
        origin: SelectedMemoryAccessOrigin::Edge(EdgeId::new(edge).unwrap()),
        place: PlaceId::new(3).unwrap(),
        byte_offset: 0,
        byte_count: 8,
        role: SelectedMemoryAccessRole::WritePlace,
    }
}

fn jump_terminator(jump: SelectedInstruction, successor: SelectedSuccessor) -> SelectedTerminator {
    SelectedTerminator::Jump {
        instruction: jump,
        successor,
    }
}

/// A raw selected-stage unit fixture, not a source/Terminal admission claim.
/// Block A is the entry and ends in a semantic `Jump` to block B, whose only
/// predecessor is that edge and whose terminator is a plain return:
/// `A = [HEAD; MID; TAIL] -> jump -> B = [LEAD; RUN_A; RUN_B; TRAIL] -> return`.
/// The default move relocates the run `RUN_A..=RUN_B` onto `MID`'s position —
/// inside A's body — crossing `MID`, `TAIL`, the `Jump` instruction, the
/// edge's (empty) transport roster, and `LEAD` while B's tail keeps its
/// order behind it.
fn fixture(target: NativeTarget) -> ValidatedPredecessorRunRelocation {
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let materialize = environment.constraint(keys.materialize_i64).unwrap();
    let jump_row = environment.constraint(keys.jump).unwrap();
    let return_row = environment.constraint(keys.return_unit).unwrap();
    let class = materialize.operands[0].class;
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let result_register = |id: VirtualRegisterId, instruction: SelectedInstructionId, value| {
        register(
            id,
            class,
            VirtualRegisterOrigin::InstructionResult {
                instruction,
                source_value: ValueId::new(value).unwrap(),
            },
        )
    };
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
        result_register(R_HEAD, HEAD, 11),
        result_register(R_MID, MID, 13),
        result_register(R_TAIL, TAIL, 15),
        result_register(R_LEAD, LEAD, 5),
        result_register(R_MOVE_A, RUN_A, 7),
        result_register(R_MOVE_B, RUN_B, 8),
        result_register(R_TRAIL, TRAIL, 9),
        result_register(R_BOUND, LEAD, 2),
    ];
    let materialization = |id, register: VirtualRegisterId, value| {
        instruction(
            id,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(value),
            },
            materialize,
            &[register],
        )
    };
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
            entry_block: BLOCK_A,
            virtual_registers: registers,
            blocks: vec![
                SelectedBlock {
                    id: BLOCK_A,
                    origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
                    instructions: vec![
                        materialization(HEAD, R_HEAD, 11),
                        materialization(MID, R_MID, 13),
                        materialization(TAIL, R_TAIL, 15),
                    ],
                    terminator: jump_terminator(
                        instruction(JUMP, SelectedInstructionKind::Jump, jump_row, &[]),
                        successor(BLOCK_B, BlockId::new(2).unwrap(), EDGE_AB),
                    ),
                },
                SelectedBlock {
                    id: BLOCK_B,
                    origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
                    instructions: vec![
                        materialization(LEAD, R_LEAD, 5),
                        materialization(RUN_A, R_MOVE_A, 7),
                        materialization(RUN_B, R_MOVE_B, 9),
                        materialization(TRAIL, R_TRAIL, 17),
                    ],
                    terminator: SelectedTerminator::Return {
                        instruction: instruction(
                            RET,
                            SelectedInstructionKind::ReturnUnit,
                            return_row,
                            &[],
                        ),
                        psi_return_edge: EdgeId::new(12).unwrap(),
                    },
                },
            ],
        }]
        .into(),
    };
    let identity = selected_instruction_plan_identity(&plan);
    ValidatedPredecessorRunRelocation {
        receipt: PredecessorRunRelocationReceipt {
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
) -> ValidatedPredecessorRunRelocation {
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
    source: &ValidatedPredecessorRunRelocation,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    first: SelectedInstructionId,
    last: SelectedInstructionId,
    destination: SelectedInstructionId,
) -> Result<ValidatedPredecessorRunRelocation, PredecessorRunRelocationError> {
    relocate_selected_run_into_predecessor(
        source,
        0,
        first,
        last,
        destination,
        environment,
        budget(),
    )
}

fn block_order(block: &SelectedBlock) -> Vec<SelectedInstructionId> {
    block
        .instructions
        .iter()
        .map(|instruction| instruction.id)
        .collect()
}

/// The run crosses its block's incoming `Jump` edge onto the
/// destination's position on every target: `RUN_A` and `RUN_B` leave B's
/// body in their own order, `LEAD` and the crossed instructions keep
/// their positions, and the run lands at `MID`'s index in A with the
/// destination and every later position one run-width later in their
/// original order — identity, kind, operands, and provenance intact. The
/// replayed proposal restores the source bit-identically.
#[test]
fn run_relocates_across_the_edge_into_the_predecessor() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = fixture(target);
        let result = relocate(&source, &environment, RUN_A, RUN_B, MID).unwrap();
        let original = &source.transformed().functions[0];
        let moved = &result.transformed().functions[0];
        assert_eq!(block_order(&moved.blocks[1]), vec![LEAD, TRAIL]);
        assert_eq!(
            block_order(&moved.blocks[0]),
            vec![HEAD, RUN_A, RUN_B, MID, TAIL]
        );
        // The members moved bit-identically; both terminators and every
        // roster stayed untouched.
        assert_eq!(
            moved.blocks[0].instructions[1],
            original.blocks[1].instructions[1]
        );
        assert_eq!(
            moved.blocks[0].instructions[2],
            original.blocks[1].instructions[2]
        );
        assert_eq!(moved.blocks[0].terminator, original.blocks[0].terminator);
        assert_eq!(moved.blocks[1].terminator, original.blocks[1].terminator);
        assert_eq!(moved.memory_accesses, original.memory_accesses);
        assert_eq!(moved.calls, original.calls);
        assert_eq!(moved.boundary_settlements, original.boundary_settlements);
        validate_predecessor_run_relocation(
            &source,
            0,
            RUN_A,
            RUN_B,
            MID,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
    }
}

/// The destination names the landing position directly: a body
/// instruction puts the run on its index, and the predecessor's
/// terminator-carried `Jump` instruction lands the run at the body end.
#[test]
fn run_lands_at_the_named_position() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let head = relocate(&source, &environment, RUN_A, RUN_B, HEAD).unwrap();
    assert_eq!(
        block_order(&head.transformed().functions[0].blocks[0]),
        vec![RUN_A, RUN_B, HEAD, MID, TAIL]
    );
    let tail = relocate(&source, &environment, RUN_A, RUN_B, TAIL).unwrap();
    assert_eq!(
        block_order(&tail.transformed().functions[0].blocks[0]),
        vec![HEAD, MID, RUN_A, RUN_B, TAIL]
    );
    let body_end = relocate(&source, &environment, RUN_A, RUN_B, JUMP).unwrap();
    assert_eq!(
        block_order(&body_end.transformed().functions[0].blocks[0]),
        vec![HEAD, MID, TAIL, RUN_A, RUN_B]
    );
}

/// Every bounded span relocates: the block-head run crosses only the
/// predecessor tail, the terminator, and the edge, while the block-tail
/// run crosses its whole leading body before the edge as well.
#[test]
fn head_and_tail_runs_relocate() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let head = relocate(&source, &environment, LEAD, RUN_A, MID).unwrap();
    assert_eq!(
        block_order(&head.transformed().functions[0].blocks[1]),
        vec![RUN_B, TRAIL]
    );
    assert_eq!(
        block_order(&head.transformed().functions[0].blocks[0]),
        vec![HEAD, LEAD, RUN_A, MID, TAIL]
    );
    let tail = relocate(&source, &environment, RUN_B, TRAIL, MID).unwrap();
    assert_eq!(
        block_order(&tail.transformed().functions[0].blocks[1]),
        vec![LEAD, RUN_A]
    );
    assert_eq!(
        block_order(&tail.transformed().functions[0].blocks[0]),
        vec![HEAD, RUN_B, TRAIL, MID, TAIL]
    );
}

/// The run moves as one body: a member whose only crossed hazard meets
/// the run's own earlier member cannot leave its block alone — the
/// member move would cross the producer it reads — while the run carries
/// the producer with it.
#[test]
fn internally_coupled_run_moves_as_one_body() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[1].instructions[2] = instruction(
            RUN_B,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[R_MOVE_A, R_MOVE_B],
        );
    });
    // The run admits: `RUN_B` reads `RUN_A`'s result inside the run,
    // where internal coupling never trades order.
    let moved = relocate(&source, &environment, RUN_A, RUN_B, MID).unwrap();
    assert_eq!(
        block_order(&moved.transformed().functions[0].blocks[0]),
        vec![HEAD, RUN_A, RUN_B, MID, TAIL]
    );
    // `RUN_B` alone cannot cross `RUN_A`, whose write it reads — the
    // producer sits in the member move's own-block prefix, so the
    // single-member family refuses where the run admits.
    assert_eq!(
        crate::relocate_selected_instruction_into_predecessor(
            &source,
            0,
            RUN_B,
            MID,
            &environment,
            budget(),
        )
        .unwrap_err(),
        crate::PredecessorRelocationError::UnsupportedPair
    );
}

/// A member whose write feeds a crossed read keeps order: the crossed
/// `TAIL` reading a member's result refuses, and the crossed `LEAD`
/// reading it refuses at every landing since the run's own-block prefix
/// always crosses.
#[test]
fn raw_hazard_keeps_order_across_the_edge() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let tail_reads = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[2] = instruction(
            TAIL,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[R_MOVE_A, R_TAIL],
        );
    });
    // Landing on `TAIL` crosses it and the member's write would feed the
    // crossed read early; landing at the body end keeps `TAIL` ahead of
    // the run as before.
    assert_eq!(
        relocate(&tail_reads, &environment, RUN_A, RUN_B, TAIL).unwrap_err(),
        PredecessorRunRelocationError::UnsupportedPair
    );
    relocate(&tail_reads, &environment, RUN_A, RUN_B, JUMP).unwrap();
    let lead_reads = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[1].instructions[0] = instruction(
            LEAD,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[R_MOVE_A, R_LEAD],
        );
    });
    // `LEAD` sits before the run in its own block: every landing crosses
    // it, so the member's write would newly feed it.
    assert_eq!(
        relocate(&lead_reads, &environment, RUN_A, RUN_B, MID).unwrap_err(),
        PredecessorRunRelocationError::UnsupportedPair
    );
    assert_eq!(
        relocate(&lead_reads, &environment, RUN_A, RUN_B, JUMP).unwrap_err(),
        PredecessorRunRelocationError::UnsupportedPair
    );
}

/// A member reading a location a crossed instruction writes would
/// observe the new value before the move's old position.
#[test]
fn war_hazard_keeps_order_across_the_edge() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        let materialize = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap()
            .clone();
        function.blocks[1].instructions[2] = instruction(
            RUN_B,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[POINTER, R_MOVE_B],
        );
        function.blocks[0].instructions[1] = instruction(
            MID,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(9),
            },
            &materialize,
            &[POINTER],
        );
    });
    assert_eq!(
        relocate(&source, &environment, RUN_A, RUN_B, MID).unwrap_err(),
        PredecessorRunRelocationError::UnsupportedPair
    );
    // Landing at the body end keeps the run behind `MID`'s write, as the
    // source had it.
    relocate(&source, &environment, RUN_A, RUN_B, JUMP).unwrap();
}

/// A member and a crossed instruction writing the same register would
/// change which definition later positions observe.
#[test]
fn waw_hazard_keeps_order_across_the_edge() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let materialize = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[1] = instruction(
            MID,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(9),
            },
            &materialize,
            &[R_MOVE_A],
        );
    });
    assert_eq!(
        relocate(&source, &environment, RUN_A, RUN_B, MID).unwrap_err(),
        PredecessorRunRelocationError::UnsupportedPair
    );
    relocate(&source, &environment, RUN_A, RUN_B, JUMP).unwrap();
}

/// Condition state couples like registers across the edge: a
/// flag-clobbering member cannot cross the compare publishing those
/// flags, and cannot cross the materialization reading them.
#[test]
fn condition_state_couples_across_the_edge() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let subtract_kind = SelectedInstructionKind::ExactSubtractI64 {
        obligation: ObligationId::new(11).unwrap(),
        accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes([7; 32]),
    };
    let flag_writer = mutated(target, |function, environment| {
        let subtract = environment
            .constraint(environment.selected_keys().subtract_i64)
            .unwrap()
            .clone();
        let compare = environment
            .constraint(environment.selected_keys().compare_i64)
            .unwrap()
            .clone();
        function.blocks[1].instructions[1] = instruction(
            RUN_A,
            subtract_kind,
            &subtract,
            &[POINTER, R_MOVE_A, R_MOVE_A],
        );
        function.blocks[0].instructions[1] = instruction(
            MID,
            SelectedInstructionKind::CompareI64,
            &compare,
            &[R_HEAD, R_MID],
        );
    });
    assert_eq!(
        relocate(&flag_writer, &environment, RUN_A, RUN_B, MID).unwrap_err(),
        PredecessorRunRelocationError::UnsupportedPair
    );
    relocate(&flag_writer, &environment, RUN_A, RUN_B, JUMP).unwrap();
    let flag_reader = mutated(target, |function, environment| {
        let subtract = environment
            .constraint(environment.selected_keys().subtract_i64)
            .unwrap()
            .clone();
        let boolean = environment
            .constraint(environment.selected_keys().materialize_boolean)
            .unwrap()
            .clone();
        function.blocks[1].instructions[1] = instruction(
            RUN_A,
            subtract_kind,
            &subtract,
            &[POINTER, R_MOVE_A, R_MOVE_A],
        );
        function.blocks[0].instructions[1] = instruction(
            MID,
            SelectedInstructionKind::MaterializeBooleanEqual,
            &boolean,
            &[R_HEAD],
        );
    });
    assert_eq!(
        relocate(&flag_reader, &environment, RUN_A, RUN_B, MID).unwrap_err(),
        PredecessorRunRelocationError::UnsupportedPair
    );
}

/// The `Jump` terminator is a crossed position, not a window barrier: a
/// register it reads couples like any crossed read, while a read it
/// shares with a member admits.
#[test]
fn terminator_position_couples() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The terminator reads a member's result: the member's write would
    // land before the read instead of after it.
    let terminator_reads = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        let class = function.blocks[0].instructions[0].operands[0].class;
        let mut jump = instruction(JUMP, SelectedInstructionKind::Jump, &jump_row, &[]);
        jump.operands.push(SelectedOperand {
            operand: 0,
            virtual_register: R_MOVE_B,
            access: RegisterOperandAccess::Use,
            class,
            fixed_view: None,
            tied_to: None,
            early_clobber: false,
        });
        let successor = match &function.blocks[0].terminator {
            SelectedTerminator::Jump { successor, .. } => successor.clone(),
            _ => unreachable!(),
        };
        function.blocks[0].terminator = jump_terminator(jump, successor);
    });
    assert_eq!(
        relocate(&terminator_reads, &environment, RUN_A, RUN_B, MID).unwrap_err(),
        PredecessorRunRelocationError::UnsupportedPair
    );
    // The terminator reading only the entry parameter a member also
    // reads shares no written location: the run still crosses.
    let shared_read = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[1].instructions[2] = instruction(
            RUN_B,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[POINTER, R_MOVE_B],
        );
        let class = function.blocks[0].instructions[0].operands[0].class;
        let mut jump = instruction(JUMP, SelectedInstructionKind::Jump, &jump_row, &[]);
        jump.operands.push(SelectedOperand {
            operand: 0,
            virtual_register: POINTER,
            access: RegisterOperandAccess::Use,
            class,
            fixed_view: None,
            tied_to: None,
            early_clobber: false,
        });
        let successor = match &function.blocks[0].terminator {
            SelectedTerminator::Jump { successor, .. } => successor.clone(),
            _ => unreachable!(),
        };
        function.blocks[0].terminator = jump_terminator(jump, successor);
    });
    relocate(&shared_read, &environment, RUN_A, RUN_B, MID).unwrap();
}

/// The edge's register transports sit between the run's new and old
/// positions: a member defining the transported argument would hand the
/// binding a new value where the source bound the old, a member defining
/// or reading the parameter would be overwritten or observe the
/// pre-transport value, while a member merely reading the argument
/// crosses freely.
#[test]
fn register_transports_bind_the_boundary() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let with_binding = |edit: &mut dyn FnMut(
        &mut SelectedFunction,
        &register_environment::ValidatedTargetRegisterEnvironment,
    )| {
        mutated(target, |function, environment| {
            let successor = match &mut function.blocks[0].terminator {
                SelectedTerminator::Jump { successor, .. } => successor,
                _ => unreachable!(),
            };
            successor.bindings.push(SelectedValueBinding {
                semantic: abstract_operations::ValueBinding {
                    parameter: ValueId::new(20).unwrap(),
                    argument: ValueId::new(1).unwrap(),
                    scalar_type: ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                    ),
                },
                transport: SelectedValueTransport::Registers {
                    argument: POINTER,
                    parameter: R_BOUND,
                },
            });
            edit(function, environment);
        })
    };
    // An unrelated run crosses the transport freely.
    relocate(
        &with_binding(&mut |_, _| {}),
        &environment,
        RUN_A,
        RUN_B,
        MID,
    )
    .unwrap();
    // A member reading the transported argument reads the same register
    // on either side of the binding.
    relocate(
        &with_binding(&mut |function, environment| {
            let copy = environment
                .constraint(environment.selected_keys().copy_i64)
                .unwrap()
                .clone();
            function.blocks[1].instructions[2] = instruction(
                RUN_B,
                SelectedInstructionKind::CopyI64,
                &copy,
                &[POINTER, R_MOVE_B],
            );
        }),
        &environment,
        RUN_A,
        RUN_B,
        MID,
    )
    .unwrap();
    // A member defining the transported argument would hand the binding
    // a new value where the source bound the old.
    assert_eq!(
        relocate(
            &with_binding(&mut |function, environment| {
                let materialize = environment
                    .constraint(environment.selected_keys().materialize_i64)
                    .unwrap()
                    .clone();
                function.blocks[1].instructions[1] = instruction(
                    RUN_A,
                    SelectedInstructionKind::MaterializeI64 {
                        value: IntegerValue::Unsigned(9),
                    },
                    &materialize,
                    &[POINTER],
                );
            }),
            &environment,
            RUN_A,
            RUN_B,
            MID,
        )
        .unwrap_err(),
        PredecessorRunRelocationError::UnsupportedPair
    );
    // A member defining the parameter would be overwritten by the
    // transport before the run's block observes it.
    assert_eq!(
        relocate(
            &with_binding(&mut |function, environment| {
                let copy = environment
                    .constraint(environment.selected_keys().copy_i64)
                    .unwrap()
                    .clone();
                function.blocks[1].instructions[2] = instruction(
                    RUN_B,
                    SelectedInstructionKind::CopyI64,
                    &copy,
                    &[POINTER, R_BOUND],
                );
            }),
            &environment,
            RUN_A,
            RUN_B,
            MID,
        )
        .unwrap_err(),
        PredecessorRunRelocationError::UnsupportedPair
    );
    // A member reading the parameter would observe the pre-transport
    // value after the move.
    assert_eq!(
        relocate(
            &with_binding(&mut |function, environment| {
                let copy = environment
                    .constraint(environment.selected_keys().copy_i64)
                    .unwrap()
                    .clone();
                function.blocks[1].instructions[1] = instruction(
                    RUN_A,
                    SelectedInstructionKind::CopyI64,
                    &copy,
                    &[R_BOUND, R_MOVE_A],
                );
            }),
            &environment,
            RUN_A,
            RUN_B,
            MID,
        )
        .unwrap_err(),
        PredecessorRunRelocationError::UnsupportedPair
    );
}

/// The run's block must be reached by the crossed edge alone: a second
/// predecessor gives the block a path the run would stop executing on,
/// the entry block is reached with no predecessor at all, and a
/// self-edge is the in-block family's case.
#[test]
fn run_block_needs_the_crossed_edge_alone() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A second predecessor edge into B refuses.
    let two_predecessors = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(2),
            origin: SelectedBlockOrigin::Source(BlockId::new(3).unwrap()),
            instructions: Vec::new(),
            terminator: jump_terminator(
                instruction(
                    SelectedInstructionId(20),
                    SelectedInstructionKind::Jump,
                    &jump_row,
                    &[],
                ),
                successor(BLOCK_B, BlockId::new(2).unwrap(), 13),
            ),
        });
    });
    assert_eq!(
        relocate(&two_predecessors, &environment, RUN_A, RUN_B, MID).unwrap_err(),
        PredecessorRunRelocationError::UnsupportedPair
    );
    // The run's own block as the entry block refuses even with the
    // single edge naming it.
    let entry_member = mutated(target, |function, _| {
        function.entry_block = BLOCK_B;
    });
    assert_eq!(
        relocate(&entry_member, &environment, RUN_A, RUN_B, MID).unwrap_err(),
        PredecessorRunRelocationError::UnsupportedPair
    );
    // A self-edge is the in-block family's case, not a cross-edge window:
    // with A's `Jump` retargeted at A itself, `HEAD..=MID` sits in a
    // block whose sole incoming edge is its own.
    let self_edge = mutated(target, |function, _| {
        let successor = match &mut function.blocks[0].terminator {
            SelectedTerminator::Jump { successor, .. } => successor,
            _ => unreachable!(),
        };
        successor.block = BLOCK_A;
    });
    assert_eq!(
        relocate(&self_edge, &environment, HEAD, MID, MID).unwrap_err(),
        PredecessorRunRelocationError::UnsupportedPair
    );
    // An implementation block never receives the run: edge-transfer
    // bridges and case dispatch carry boundary work this step does not
    // cross into.
    for origin in [
        SelectedBlockOrigin::EdgeTransfer {
            edge: EdgeId::new(EDGE_AB).unwrap(),
            target: BlockId::new(2).unwrap(),
        },
        SelectedBlockOrigin::CaseDispatch {
            source: BlockId::new(2).unwrap(),
            case_ordinal: 0,
        },
    ] {
        let implementation_target = mutated(target, |function, _| {
            function.blocks[0].origin = origin;
        });
        assert_eq!(
            relocate(&implementation_target, &environment, RUN_A, RUN_B, MID).unwrap_err(),
            PredecessorRunRelocationError::UnsupportedPair
        );
    }
}

/// Only a plain semantic edge carries the run: continuation roles, case
/// custody, edge fuel, and a live structural transport are all boundary
/// work the window does not cross.
#[test]
fn only_a_plain_semantic_edge_carries_the_run() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for role in [
        SelectedSuccessorRole::EdgeTransferContinuation,
        SelectedSuccessorRole::CaseDispatchContinuation,
    ] {
        let continued = mutated(target, |function, _| {
            let successor = match &mut function.blocks[0].terminator {
                SelectedTerminator::Jump { successor, .. } => successor,
                _ => unreachable!(),
            };
            successor.role = role;
        });
        assert_eq!(
            relocate(&continued, &environment, RUN_A, RUN_B, MID).unwrap_err(),
            PredecessorRunRelocationError::UnsupportedPair
        );
    }
    let case_edge = mutated(target, |function, _| {
        let successor = match &mut function.blocks[0].terminator {
            SelectedTerminator::Jump { successor, .. } => successor,
            _ => unreachable!(),
        };
        successor.structural_case = Some(selected_instructions::SelectedStructuralCaseEdge {
            slot: selected_instructions::LocalStorageSlotId::Spill { register: R_BOUND },
            case: StructuralCaseId::new(1).unwrap(),
            case_tag: 0,
            payloads: Vec::new(),
            trivial_affine_discards: Vec::new(),
        });
    });
    assert_eq!(
        relocate(&case_edge, &environment, RUN_A, RUN_B, MID).unwrap_err(),
        PredecessorRunRelocationError::UnsupportedPair
    );
    let fueled = mutated(target, |function, _| {
        let successor = match &mut function.blocks[0].terminator {
            SelectedTerminator::Jump { successor, .. } => successor,
            _ => unreachable!(),
        };
        successor.fuel.push(optimization_unit::FuelSettlement {
            site: optimization_unit::PsiProvenance::Operation(OperationId::new(30).unwrap()),
            units: 1,
        });
    });
    assert_eq!(
        relocate(&fueled, &environment, RUN_A, RUN_B, MID).unwrap_err(),
        PredecessorRunRelocationError::UnsupportedPair
    );
    // A live structural transport moves a stored value at the boundary.
    let structural = mutated(target, |function, _| {
        let successor = match &mut function.blocks[0].terminator {
            SelectedTerminator::Jump { successor, .. } => successor,
            _ => unreachable!(),
        };
        successor
            .structural_bindings
            .push(SelectedStructuralBinding {
                semantic: abstract_operations::AbstractStructuralBinding {
                    parameter: PlaceId::new(4).unwrap(),
                    argument: terminal_psi::StructuralArgument {
                        place: PlaceId::new(5).unwrap(),
                        path: Vec::new(),
                        access: terminal_psi::StructuralAccess::MutableBorrow,
                    },
                },
                transport: SelectedStructuralTransport::WholeValue {
                    argument: POINTER,
                    destination: selected_instructions::LocalStorageSlotId::Spill {
                        register: R_BOUND,
                    },
                    byte_size: 8,
                    alignment: 8,
                },
            });
    });
    assert_eq!(
        relocate(&structural, &environment, RUN_A, RUN_B, MID).unwrap_err(),
        PredecessorRunRelocationError::UnsupportedPair
    );
    // An `Unused` structural transport moves nothing and admits.
    let inert_structural = mutated(target, |function, _| {
        let successor = match &mut function.blocks[0].terminator {
            SelectedTerminator::Jump { successor, .. } => successor,
            _ => unreachable!(),
        };
        successor
            .structural_bindings
            .push(SelectedStructuralBinding {
                semantic: abstract_operations::AbstractStructuralBinding {
                    parameter: PlaceId::new(4).unwrap(),
                    argument: terminal_psi::StructuralArgument {
                        place: PlaceId::new(5).unwrap(),
                        path: Vec::new(),
                        access: terminal_psi::StructuralAccess::MutableBorrow,
                    },
                },
                transport: SelectedStructuralTransport::Unused,
            });
    });
    relocate(&inert_structural, &environment, RUN_A, RUN_B, MID).unwrap();
}

/// The sole edge into the run's block must leave an unconditional
/// `Jump`: a conditional predecessor keeps a second exit the run would
/// newly execute on after relocating, whether it names the run's block
/// on one edge or both. A predecessor without a jump edge names no
/// window at all.
#[test]
fn only_a_jump_edge_into_the_run_block() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // One conditional edge reaches the run's block; the other leaves for
    // a third block, so a predecessor traversal does not always reach
    // the run.
    let conditional = mutated(target, |function, environment| {
        let branch_row = environment
            .constraint(environment.selected_keys().conditional_branch)
            .unwrap()
            .clone();
        let return_row = environment
            .constraint(environment.selected_keys().return_unit)
            .unwrap()
            .clone();
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(2),
            origin: SelectedBlockOrigin::Source(BlockId::new(3).unwrap()),
            instructions: Vec::new(),
            terminator: SelectedTerminator::Return {
                instruction: instruction(
                    SelectedInstructionId(20),
                    SelectedInstructionKind::ReturnUnit,
                    &return_row,
                    &[],
                ),
                psi_return_edge: EdgeId::new(13).unwrap(),
            },
        });
        function.blocks[0].terminator = SelectedTerminator::ConditionalBranch {
            instruction: instruction(
                JUMP,
                SelectedInstructionKind::ConditionalBranchNonZero,
                &branch_row,
                &[],
            ),
            when_nonzero: successor(BLOCK_B, BlockId::new(2).unwrap(), EDGE_AB),
            when_zero: successor(SelectedBlockId(2), BlockId::new(3).unwrap(), 11),
        };
    });
    assert_eq!(
        relocate(&conditional, &environment, RUN_A, RUN_B, MID).unwrap_err(),
        PredecessorRunRelocationError::UnsupportedPair
    );
    // Both conditional edges naming the run's block give it a second
    // incoming edge even though both leave the same predecessor.
    let twice = mutated(target, |function, environment| {
        let branch_row = environment
            .constraint(environment.selected_keys().conditional_branch)
            .unwrap()
            .clone();
        function.blocks[0].terminator = SelectedTerminator::ConditionalBranch {
            instruction: instruction(
                JUMP,
                SelectedInstructionKind::ConditionalBranchNonZero,
                &branch_row,
                &[],
            ),
            when_nonzero: successor(BLOCK_B, BlockId::new(2).unwrap(), EDGE_AB),
            when_zero: successor(BLOCK_B, BlockId::new(2).unwrap(), 11),
        };
    });
    assert_eq!(
        relocate(&twice, &environment, RUN_A, RUN_B, MID).unwrap_err(),
        PredecessorRunRelocationError::UnsupportedPair
    );
    // A predecessor whose terminator does not reach the run's block at
    // all leaves it with no incoming edge.
    let returned = mutated(target, |function, environment| {
        let return_row = environment
            .constraint(environment.selected_keys().return_unit)
            .unwrap()
            .clone();
        function.blocks[0].terminator = SelectedTerminator::Return {
            instruction: instruction(JUMP, SelectedInstructionKind::ReturnUnit, &return_row, &[]),
            psi_return_edge: EdgeId::new(EDGE_AB).unwrap(),
        };
    });
    assert_eq!(
        relocate(&returned, &environment, RUN_A, RUN_B, MID).unwrap_err(),
        PredecessorRunRelocationError::UnsupportedPair
    );
    // The run's own terminator is unconstrained: B ends in a return in
    // the base fixture, and the run still relocates.
    relocate(&fixture(target), &environment, RUN_A, RUN_B, MID).unwrap();
}

/// Calls, hosted effects, and terminator kinds are barriers as a run
/// member or anywhere inside the crossed window, and a call-roster row
/// makes an instruction a barrier even when its kind is register-pure —
/// the `Jump` terminator's own id included. Positions outside the window
/// keep their barriers without blocking the move.
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
            function.blocks[1].instructions[1].kind = kind;
        });
        assert_eq!(
            relocate(&member_barrier, &environment, RUN_A, RUN_B, MID).unwrap_err(),
            PredecessorRunRelocationError::UnsupportedInstruction,
            "member {kind:?}"
        );
        let tail_barrier = mutated(target, |function, _| {
            function.blocks[0].instructions[2].kind = kind;
        });
        assert_eq!(
            relocate(&tail_barrier, &environment, RUN_A, RUN_B, MID).unwrap_err(),
            PredecessorRunRelocationError::UnsupportedInstruction,
            "crossed predecessor tail {kind:?}"
        );
        let lead_barrier = mutated(target, |function, _| {
            function.blocks[1].instructions[0].kind = kind;
        });
        assert_eq!(
            relocate(&lead_barrier, &environment, RUN_A, RUN_B, MID).unwrap_err(),
            PredecessorRunRelocationError::UnsupportedInstruction,
            "crossed lead {kind:?}"
        );
        // Barriers outside the window — before the landing index in the
        // predecessor or after the run in its own block — are never
        // crossed and the move still admits.
        let head_barrier = mutated(target, |function, _| {
            function.blocks[0].instructions[0].kind = kind;
        });
        relocate(&head_barrier, &environment, RUN_A, RUN_B, MID).unwrap();
        let trail_barrier = mutated(target, |function, _| {
            function.blocks[1].instructions[3].kind = kind;
        });
        relocate(&trail_barrier, &environment, RUN_A, RUN_B, MID).unwrap();
    }
    let call_contract = |instruction: SelectedInstructionId| SelectedCallContract {
        instruction,
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
        effect: EffectLink {
            input: 0,
            output: 0,
        },
        ownership: Vec::new(),
    };
    for instruction_id in [RUN_A, RUN_B, MID, TAIL, LEAD, JUMP] {
        let contract = mutated(target, |function, _| {
            function.calls.push(call_contract(instruction_id));
        });
        assert_eq!(
            relocate(&contract, &environment, RUN_A, RUN_B, MID).unwrap_err(),
            PredecessorRunRelocationError::UnsupportedInstruction,
            "call roster {instruction_id:?}"
        );
    }
    // A call-roster row outside the crossed window never blocks the
    // move.
    for instruction_id in [HEAD, TRAIL] {
        let contract = mutated(target, |function, _| {
            function.calls.push(call_contract(instruction_id));
        });
        relocate(&contract, &environment, RUN_A, RUN_B, MID).unwrap();
    }
}

/// A roster-carrying run may cross only row-less positions — the `Jump`
/// terminator and the edge's own recorded accesses included — while a
/// row-less run crosses any accounted mix because no recorded access
/// changes order.
#[test]
fn memory_roster_binds_the_window() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let load_member =
        |function: &mut SelectedFunction,
         environment: &register_environment::ValidatedTargetRegisterEnvironment| {
            let load = environment
                .constraint(environment.selected_keys().load8.unwrap())
                .unwrap()
                .clone();
            function.blocks[1].instructions[1] = instruction(
                RUN_A,
                SelectedInstructionKind::Load8 { byte_offset: 0 },
                &load,
                &[POINTER, R_MOVE_A],
            );
            function.memory_accesses.push(access(
                RUN_A,
                PlaceId::new(1).unwrap(),
                SelectedMemoryAccessRole::ReadPlace,
            ));
        };
    // The roster-carrying run crosses row-less positions: the
    // predecessor tail materializations, the jump, the edge, and the
    // lead materialization in its own block.
    relocate(
        &mutated(target, |function, environment| {
            load_member(function, environment)
        }),
        &environment,
        RUN_A,
        RUN_B,
        MID,
    )
    .unwrap();
    // Two roster-carrying members may share the run — their recorded
    // order never changes against each other — while the window still
    // admits only row-less positions.
    relocate(
        &mutated(target, |function, environment| {
            load_member(function, environment);
            let load = environment
                .constraint(environment.selected_keys().load8.unwrap())
                .unwrap()
                .clone();
            function.blocks[1].instructions[2] = instruction(
                RUN_B,
                SelectedInstructionKind::Load8 { byte_offset: 0 },
                &load,
                &[POINTER, R_MOVE_B],
            );
            function.memory_accesses.push(access(
                RUN_B,
                PlaceId::new(2).unwrap(),
                SelectedMemoryAccessRole::ReadPlace,
            ));
        }),
        &environment,
        RUN_A,
        RUN_B,
        MID,
    )
    .unwrap();
    // A second accounted actor anywhere in the window refuses: the run's
    // recorded read would trade order with the crossed write.
    let accounted_tail = mutated(target, |function, environment| {
        load_member(function, environment);
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[2] = instruction(
            TAIL,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            &store,
            &[POINTER, R_TAIL],
        );
        function.memory_accesses.push(access(
            TAIL,
            PlaceId::new(2).unwrap(),
            SelectedMemoryAccessRole::WritePlace,
        ));
    });
    assert_eq!(
        relocate(&accounted_tail, &environment, RUN_A, RUN_B, MID).unwrap_err(),
        PredecessorRunRelocationError::UnsupportedPair
    );
    let accounted_lead = mutated(target, |function, environment| {
        load_member(function, environment);
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
        function.blocks[1].instructions[0] = instruction(
            LEAD,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            &store,
            &[POINTER, R_LEAD],
        );
        function.memory_accesses.push(access(
            LEAD,
            PlaceId::new(2).unwrap(),
            SelectedMemoryAccessRole::WritePlace,
        ));
    });
    assert_eq!(
        relocate(&accounted_lead, &environment, RUN_A, RUN_B, MID).unwrap_err(),
        PredecessorRunRelocationError::UnsupportedPair
    );
    // A row recorded against the crossed edge itself counts as the edge
    // position's memory surface.
    let accounted_edge = mutated(target, |function, environment| {
        load_member(function, environment);
        function.memory_accesses.push(edge_access(RET, EDGE_AB));
    });
    assert_eq!(
        relocate(&accounted_edge, &environment, RUN_A, RUN_B, MID).unwrap_err(),
        PredecessorRunRelocationError::UnsupportedPair
    );
    // A row recorded against the `Jump` instruction is the terminator
    // position's memory surface.
    let accounted_jump = mutated(target, |function, environment| {
        load_member(function, environment);
        function.memory_accesses.push(access(
            JUMP,
            PlaceId::new(2).unwrap(),
            SelectedMemoryAccessRole::WritePlace,
        ));
    });
    assert_eq!(
        relocate(&accounted_jump, &environment, RUN_A, RUN_B, MID).unwrap_err(),
        PredecessorRunRelocationError::UnsupportedPair
    );
    // A row on another edge is not crossed: it names a position outside
    // the window — recorded here against the non-crossed `HEAD`.
    let foreign_edge = mutated(target, |function, environment| {
        load_member(function, environment);
        function.memory_accesses.push(edge_access(HEAD, 99));
    });
    relocate(&foreign_edge, &environment, RUN_A, RUN_B, MID).unwrap();
    // A row-less run crosses any accounted mix.
    let rowless = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[2] = instruction(
            TAIL,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            &store,
            &[POINTER, R_TAIL],
        );
        function.blocks[1].instructions[0] = instruction(
            LEAD,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            &store,
            &[POINTER, R_LEAD],
        );
        function.memory_accesses.push(access(
            TAIL,
            PlaceId::new(1).unwrap(),
            SelectedMemoryAccessRole::WritePlace,
        ));
        function.memory_accesses.push(access(
            LEAD,
            PlaceId::new(2).unwrap(),
            SelectedMemoryAccessRole::WritePlace,
        ));
    });
    let result = relocate(&rowless, &environment, RUN_A, RUN_B, MID).unwrap();
    assert_eq!(
        result.transformed().functions[0].memory_accesses,
        rowless.transformed().functions[0].memory_accesses
    );
    // A memory-capable kind without a roster row is an unaccounted
    // access: the relocation cannot prove what it reaches, so it refuses
    // outright whether the bare access is a member or a crossed
    // position.
    let bare_member = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
        function.blocks[1].instructions[1] = instruction(
            RUN_A,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            &store,
            &[POINTER, R_MOVE_A],
        );
    });
    assert_eq!(
        relocate(&bare_member, &environment, RUN_A, RUN_B, MID).unwrap_err(),
        PredecessorRunRelocationError::UnsupportedInstruction
    );
    let bare_crossed = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[2] = instruction(
            TAIL,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            &store,
            &[POINTER, R_TAIL],
        );
    });
    assert_eq!(
        relocate(&bare_crossed, &environment, RUN_A, RUN_B, MID).unwrap_err(),
        PredecessorRunRelocationError::UnsupportedInstruction
    );
}

/// A boundary settlement positioned past the landing index observes the
/// run inside the predecessor block's executed prefix, and one
/// positioned past the run's first index observed a member inside its
/// own block's — both refuse, while positions at or before either
/// boundary keep the executed set they always had.
#[test]
fn boundary_settlements_bound_the_window() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // In the run's block, settlements at or before the run's first index
    // admit; positions past it observed a member inside the prefix.
    for (position, admits) in [(0u32, true), (1, true), (2, false), (3, false), (4, false)] {
        let settled = mutated(target, |function, _| {
            function
                .boundary_settlements
                .push(settlement(BLOCK_B, position, 50));
        });
        let result = relocate(&settled, &environment, RUN_A, RUN_B, MID);
        assert_eq!(
            result.is_ok(),
            admits,
            "member-block settlement at {position}"
        );
    }
    // In the predecessor block the bound is the landing index: at or
    // before it the executed prefix is unchanged; past it the run joins
    // the prefix.
    for (position, admits) in [(0u32, true), (1, true), (2, false), (3, false)] {
        let settled = mutated(target, |function, _| {
            function
                .boundary_settlements
                .push(settlement(BLOCK_A, position, 51));
        });
        let result = relocate(&settled, &environment, RUN_A, RUN_B, MID);
        assert_eq!(
            result.is_ok(),
            admits,
            "predecessor-block settlement at {position}"
        );
    }
    // Landing at the body end keeps every predecessor settlement: none
    // sits past the run's new span.
    let settled = mutated(target, |function, _| {
        function
            .boundary_settlements
            .push(settlement(BLOCK_A, 3, 52));
    });
    relocate(&settled, &environment, RUN_A, RUN_B, JUMP).unwrap();
    // A settlement in an untouched block never observes the move.
    let elsewhere = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(2),
            origin: SelectedBlockOrigin::Source(BlockId::new(3).unwrap()),
            instructions: Vec::new(),
            terminator: jump_terminator(
                instruction(
                    SelectedInstructionId(20),
                    SelectedInstructionKind::Jump,
                    &jump_row,
                    &[],
                ),
                successor(BLOCK_A, BlockId::new(1).unwrap(), 13),
            ),
        });
        function
            .boundary_settlements
            .push(settlement(SelectedBlockId(2), 0, 53));
    });
    relocate(&elsewhere, &environment, RUN_A, RUN_B, MID).unwrap();
}

/// The triple must name one contiguous run in a block reached by the
/// crossed edge alone and one position in the edge's predecessor: a
/// repeated or misordered member, an unknown member, a wrong function, a
/// destination in another block, and a destination that is no
/// instruction at all refuse.
#[test]
fn only_the_named_run_relocates() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // An unknown member or function index never locates the window.
    assert_eq!(
        relocate_selected_run_into_predecessor(
            &source,
            0,
            SelectedInstructionId(99),
            RUN_B,
            MID,
            &environment,
            budget(),
        )
        .unwrap_err(),
        PredecessorRunRelocationError::SourceMismatch
    );
    assert_eq!(
        relocate_selected_run_into_predecessor(
            &source,
            9,
            RUN_A,
            RUN_B,
            MID,
            &environment,
            budget(),
        )
        .unwrap_err(),
        PredecessorRunRelocationError::SourceMismatch
    );
    // A repeated member bounds no multi-member run, and a last member
    // that does not follow the first names none either — a single member
    // is the one-instruction family's case.
    assert_eq!(
        relocate(&source, &environment, RUN_A, RUN_A, MID).unwrap_err(),
        PredecessorRunRelocationError::UnsupportedPair
    );
    assert_eq!(
        relocate(&source, &environment, RUN_B, RUN_A, MID).unwrap_err(),
        PredecessorRunRelocationError::UnsupportedPair
    );
    // The destination must name a position in the predecessor block — a
    // member of the run's own block and a dangling id both refuse.
    for destination in [TRAIL, SelectedInstructionId(99)] {
        assert_eq!(
            relocate(&source, &environment, RUN_A, RUN_B, destination).unwrap_err(),
            PredecessorRunRelocationError::UnsupportedPair
        );
    }
    // A run in a block with no incoming edge names no window: the entry
    // block's own `HEAD..=MID` span has no predecessor at all.
    assert_eq!(
        relocate(&source, &environment, HEAD, MID, TAIL).unwrap_err(),
        PredecessorRunRelocationError::UnsupportedPair
    );
    // Naming the run block's terminator as member is not a body
    // position.
    assert_eq!(
        relocate(&source, &environment, RET, TRAIL, MID).unwrap_err(),
        PredecessorRunRelocationError::SourceMismatch
    );
    // Naming the run block's terminator as destination names a position
    // in the wrong block.
    assert_eq!(
        relocate(&source, &environment, RUN_A, RUN_B, RET).unwrap_err(),
        PredecessorRunRelocationError::UnsupportedPair
    );
}

/// Replay consumes only the exact move: a proposal that drops a member,
/// lands the run anywhere else, permutes its members or the crossed
/// positions, or carries an unrelated edit all reject.
#[test]
fn replay_rejects_anything_but_the_move() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let result = relocate(&source, &environment, RUN_A, RUN_B, MID).unwrap();
    // The honest proposal replays.
    validate_predecessor_run_relocation(
        &source,
        0,
        RUN_A,
        RUN_B,
        MID,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // The run at the wrong index rejects.
    let mut displaced = result.transformed().clone();
    let run: Vec<_> = displaced.functions[0].blocks[0]
        .instructions
        .drain(1..=2)
        .collect();
    displaced.functions[0].blocks[0]
        .instructions
        .splice(3..3, run);
    assert_eq!(
        validate_predecessor_run_relocation(
            &source,
            0,
            RUN_A,
            RUN_B,
            MID,
            &environment,
            budget(),
            displaced
        )
        .unwrap_err(),
        PredecessorRunRelocationError::ReplayMismatch
    );
    // A permuted run rejects: the members keep their order or stay
    // home.
    let mut permuted = result.transformed().clone();
    permuted.functions[0].blocks[0].instructions.swap(1, 2);
    assert_eq!(
        validate_predecessor_run_relocation(
            &source,
            0,
            RUN_A,
            RUN_B,
            MID,
            &environment,
            budget(),
            permuted
        )
        .unwrap_err(),
        PredecessorRunRelocationError::ReplayMismatch
    );
    // The run left in its own block rejects.
    let mut unmoved = result.transformed().clone();
    let run: Vec<_> = unmoved.functions[0].blocks[0]
        .instructions
        .drain(1..=2)
        .collect();
    unmoved.functions[0].blocks[1]
        .instructions
        .splice(1..1, run);
    assert_eq!(
        validate_predecessor_run_relocation(
            &source,
            0,
            RUN_A,
            RUN_B,
            MID,
            &environment,
            budget(),
            unmoved
        )
        .unwrap_err(),
        PredecessorRunRelocationError::ReplayMismatch
    );
    // A dropped instruction anywhere rejects.
    let mut dropped = result.transformed().clone();
    dropped.functions[0].blocks[1].instructions.pop();
    assert_eq!(
        validate_predecessor_run_relocation(
            &source,
            0,
            RUN_A,
            RUN_B,
            MID,
            &environment,
            budget(),
            dropped
        )
        .unwrap_err(),
        PredecessorRunRelocationError::ReplayMismatch
    );
    // An unrelated literal edit inside the run's block rejects.
    let mut edited = result.transformed().clone();
    edited.functions[0].blocks[1].instructions[0].kind = SelectedInstructionKind::MaterializeI64 {
        value: IntegerValue::Unsigned(12),
    };
    assert_eq!(
        validate_predecessor_run_relocation(
            &source,
            0,
            RUN_A,
            RUN_B,
            MID,
            &environment,
            budget(),
            edited
        )
        .unwrap_err(),
        PredecessorRunRelocationError::ReplayMismatch
    );
    // Naming a different window on the same proposal re-derives a
    // different landing and rejects.
    assert_eq!(
        validate_predecessor_run_relocation(
            &source,
            0,
            RUN_A,
            RUN_B,
            TAIL,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap_err(),
        PredecessorRunRelocationError::ReplayMismatch
    );
}

/// The measured validation step count bounds both the proposal path and
/// the independent replay: a starved budget refuses before any position
/// changes, and the exact count admits both.
#[test]
fn measured_validation_step_boundary_admits_and_rejects() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The jump row's implicit RIP use and definition join its surface on
    // x86-64: with the members materializations and `MID` the
    // destination, the charge is (2 blocks + 7 body instructions) for
    // the member scan + (7 body + 2 terminator instructions) for the
    // predecessor scan + member-against-crossed surfaces for each member
    // against `MID` (1+1), `TAIL` (1+1), `LEAD` (1+1), and the jump
    // (1+2) + empty rosters = 9 + 9 + 2*9 = 36. Destination `HEAD` adds
    // the crossed `HEAD` pair (1+1) per member for 40.
    for (destination, exact_steps) in [(MID, 36u64), (HEAD, 40u64)] {
        let source = fixture(target);
        let exact = OptimizationWorkBudget::new(1, 1, exact_steps, 1, 1).unwrap();
        let result = relocate_selected_run_into_predecessor(
            &source,
            0,
            RUN_A,
            RUN_B,
            destination,
            &environment,
            exact,
        )
        .unwrap();
        validate_predecessor_run_relocation(
            &source,
            0,
            RUN_A,
            RUN_B,
            destination,
            &environment,
            exact,
            result.transformed().clone(),
        )
        .unwrap();
        let starved = OptimizationWorkBudget::new(1, 1, exact_steps - 1, 1, 1).unwrap();
        assert_eq!(
            relocate_selected_run_into_predecessor(
                &source,
                0,
                RUN_A,
                RUN_B,
                destination,
                &environment,
                starved,
            )
            .unwrap_err(),
            PredecessorRunRelocationError::WorkBudgetExceeded
        );
        assert_eq!(
            validate_predecessor_run_relocation(
                &source,
                0,
                RUN_A,
                RUN_B,
                destination,
                &environment,
                starved,
                result.transformed().clone(),
            )
            .unwrap_err(),
            PredecessorRunRelocationError::WorkBudgetExceeded
        );
    }
}

/// The environment's target must be the plan's target: a relocation
/// proven for one target's constraints is not evidence on another.
#[test]
fn target_mismatch_rejects() {
    let source = fixture(NativeTarget::linux_x64());
    let environment = baseline_target_register_environment(NativeTarget::linux_arm64()).unwrap();
    assert_eq!(
        relocate(&source, &environment, RUN_A, RUN_B, MID).unwrap_err(),
        PredecessorRunRelocationError::SourceMismatch
    );
}

/// Two runs over the identical source produce the identical validated
/// result, and the published plan is a legal second input through the
/// sealed analysis boundary: the trailing member crosses the same edge
/// onto it, a hazard-coupled member still declines, and an in-block
/// family still admits.
#[test]
fn predecessor_run_relocation_is_deterministic_and_re_admitted() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let first = relocate(&source, &environment, RUN_A, RUN_B, MID).unwrap();
    let second = relocate(&source, &environment, RUN_A, RUN_B, MID).unwrap();
    assert_eq!(first, second);
    // The validated output carries the sealed analysis boundary, so it
    // is a legal second input — not merely a reconstruction of one. The
    // tail member crosses the same edge onto it.
    let again = crate::relocate_selected_instruction_into_predecessor(
        &first,
        0,
        TRAIL,
        MID,
        &environment,
        budget(),
    )
    .unwrap();
    assert_eq!(
        block_order(&again.transformed().functions[0].blocks[0]),
        vec![HEAD, RUN_A, RUN_B, TRAIL, MID, TAIL]
    );
    assert_eq!(
        again.receipt().source_selected(),
        first.receipt().transformed_selected()
    );
    // A hazard-coupled member still declines on the second input: once
    // the run sits before `MID` in A, `MID` reading `R_TRAIL` keeps
    // `TRAIL` from crossing it to land on `MID`'s position.
    let coupled_source = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[1] = instruction(
            MID,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[R_TRAIL, R_MID],
        );
    });
    let moved = relocate(&coupled_source, &environment, RUN_A, RUN_B, MID).unwrap();
    assert_eq!(
        crate::relocate_selected_instruction_into_predecessor(
            &moved,
            0,
            TRAIL,
            MID,
            &environment,
            budget(),
        )
        .unwrap_err(),
        crate::PredecessorRelocationError::UnsupportedPair
    );
    // A different scheduling family still admits on the second input.
    let swapped =
        crate::relocate_selected_instruction(&first, 0, MID, TAIL, &environment, budget()).unwrap();
    assert_eq!(
        block_order(&swapped.transformed().functions[0].blocks[0]),
        vec![HEAD, RUN_A, RUN_B, TAIL, MID]
    );
}
