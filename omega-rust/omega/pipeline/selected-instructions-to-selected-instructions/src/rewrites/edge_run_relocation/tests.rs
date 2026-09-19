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
    EdgeRunRelocationError, EdgeRunRelocationReceipt, ValidatedEdgeRunRelocation,
    relocate_selected_run_across_edge, validate_edge_run_relocation,
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

const LEAD: SelectedInstructionId = SelectedInstructionId(2);
const RUN_A: SelectedInstructionId = SelectedInstructionId(3);
const RUN_B: SelectedInstructionId = SelectedInstructionId(4);
const TRAIL: SelectedInstructionId = SelectedInstructionId(5);
const HEAD: SelectedInstructionId = SelectedInstructionId(6);
const MID: SelectedInstructionId = SelectedInstructionId(7);
const TAIL: SelectedInstructionId = SelectedInstructionId(8);
const JUMP: SelectedInstructionId = SelectedInstructionId(9);
const RET: SelectedInstructionId = SelectedInstructionId(10);

const POINTER: VirtualRegisterId = VirtualRegisterId(0);
const R_LEAD: VirtualRegisterId = VirtualRegisterId(1);
const R_MOVE_A: VirtualRegisterId = VirtualRegisterId(2);
const R_TRAIL: VirtualRegisterId = VirtualRegisterId(3);
const R_HEAD: VirtualRegisterId = VirtualRegisterId(4);
const R_MID: VirtualRegisterId = VirtualRegisterId(5);
const R_TAIL: VirtualRegisterId = VirtualRegisterId(6);
const R_BOUND: VirtualRegisterId = VirtualRegisterId(7);
const R_MOVE_B: VirtualRegisterId = VirtualRegisterId(8);

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
/// `A = [LEAD; RUN_A; RUN_B; TRAIL] -> jump -> B = [HEAD; MID; TAIL] -> return`.
/// The default move relocates the run `RUN_A..=RUN_B` onto `HEAD`'s position —
/// the head of B's body — crossing `TRAIL`, the `Jump` instruction, and the
/// edge's (empty) transport roster while B's body keeps its order behind it.
fn fixture(target: NativeTarget) -> ValidatedEdgeRunRelocation {
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
        result_register(R_LEAD, LEAD, 2),
        result_register(R_MOVE_A, RUN_A, 3),
        result_register(R_MOVE_B, RUN_B, 8),
        result_register(R_TRAIL, TRAIL, 4),
        result_register(R_HEAD, HEAD, 5),
        result_register(R_MID, MID, 6),
        result_register(R_TAIL, TAIL, 7),
        result_register(R_BOUND, LEAD, 9),
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
                        materialization(LEAD, R_LEAD, 5),
                        materialization(RUN_A, R_MOVE_A, 7),
                        materialization(RUN_B, R_MOVE_B, 9),
                        materialization(TRAIL, R_TRAIL, 11),
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
                        materialization(HEAD, R_HEAD, 13),
                        materialization(MID, R_MID, 15),
                        materialization(TAIL, R_TAIL, 17),
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
    ValidatedEdgeRunRelocation {
        receipt: EdgeRunRelocationReceipt {
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
) -> ValidatedEdgeRunRelocation {
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
    source: &ValidatedEdgeRunRelocation,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    first: SelectedInstructionId,
    last: SelectedInstructionId,
    destination: SelectedInstructionId,
) -> Result<ValidatedEdgeRunRelocation, EdgeRunRelocationError> {
    relocate_selected_run_across_edge(source, 0, first, last, destination, environment, budget())
}

fn block_order(block: &SelectedBlock) -> Vec<SelectedInstructionId> {
    block
        .instructions
        .iter()
        .map(|instruction| instruction.id)
        .collect()
}

/// The run crosses its block's `Jump` edge onto the destination's position
/// on every target: `RUN_A` and `RUN_B` leave A's body in their own order,
/// `TRAIL` and the crossed instructions keep their positions, and the run
/// lands at B's head with the destination and every later position one
/// run-width later in their original order — identity, kind, operands, and
/// provenance intact. The replayed proposal restores the source
/// bit-identically.
#[test]
fn run_relocates_across_the_jump_edge() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = fixture(target);
        let result = relocate(&source, &environment, RUN_A, RUN_B, HEAD).unwrap();
        let original = &source.transformed().functions[0];
        let moved = &result.transformed().functions[0];
        assert_eq!(block_order(&moved.blocks[0]), vec![LEAD, TRAIL]);
        assert_eq!(
            block_order(&moved.blocks[1]),
            vec![RUN_A, RUN_B, HEAD, MID, TAIL]
        );
        // The members moved bit-identically; both terminators and every
        // roster stayed untouched.
        assert_eq!(
            moved.blocks[1].instructions[0],
            original.blocks[0].instructions[1]
        );
        assert_eq!(
            moved.blocks[1].instructions[1],
            original.blocks[0].instructions[2]
        );
        assert_eq!(moved.blocks[0].terminator, original.blocks[0].terminator);
        assert_eq!(moved.blocks[1].terminator, original.blocks[1].terminator);
        assert_eq!(moved.memory_accesses, original.memory_accesses);
        assert_eq!(moved.calls, original.calls);
        assert_eq!(moved.boundary_settlements, original.boundary_settlements);
        validate_edge_run_relocation(
            &source,
            0,
            RUN_A,
            RUN_B,
            HEAD,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
    }
}

/// The destination names the landing position directly: a body instruction
/// puts the run on its index, and the target's terminator-carried
/// instruction lands the run at the body end.
#[test]
fn run_lands_at_the_named_position() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let middle = relocate(&source, &environment, RUN_A, RUN_B, MID).unwrap();
    assert_eq!(
        block_order(&middle.transformed().functions[0].blocks[1]),
        vec![HEAD, RUN_A, RUN_B, MID, TAIL]
    );
    let last_body = relocate(&source, &environment, RUN_A, RUN_B, TAIL).unwrap();
    assert_eq!(
        block_order(&last_body.transformed().functions[0].blocks[1]),
        vec![HEAD, MID, RUN_A, RUN_B, TAIL]
    );
    let body_end = relocate(&source, &environment, RUN_A, RUN_B, RET).unwrap();
    assert_eq!(
        block_order(&body_end.transformed().functions[0].blocks[1]),
        vec![HEAD, MID, TAIL, RUN_A, RUN_B]
    );
}

/// Every bounded span relocates: the block-tail run crosses only the
/// terminator and edge, while the block-head run crosses its whole trailing
/// body before the edge.
#[test]
fn tail_and_head_runs_relocate() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let tail = relocate(&source, &environment, RUN_B, TRAIL, HEAD).unwrap();
    assert_eq!(
        block_order(&tail.transformed().functions[0].blocks[0]),
        vec![LEAD, RUN_A]
    );
    assert_eq!(
        block_order(&tail.transformed().functions[0].blocks[1]),
        vec![RUN_B, TRAIL, HEAD, MID, TAIL]
    );
    let head = relocate(&source, &environment, LEAD, RUN_A, HEAD).unwrap();
    assert_eq!(
        block_order(&head.transformed().functions[0].blocks[0]),
        vec![RUN_B, TRAIL]
    );
    assert_eq!(
        block_order(&head.transformed().functions[0].blocks[1]),
        vec![LEAD, RUN_A, HEAD, MID, TAIL]
    );
}

/// The run moves as one body: a producer whose only crossed reader is the
/// run's own next member cannot leave its block alone — the member move
/// would starve the consumer it leaves behind — while the run carries the
/// consumer with it.
#[test]
fn internally_coupled_run_moves_as_one_body() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[2] = instruction(
            RUN_B,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[R_MOVE_A, R_MOVE_B],
        );
    });
    // The run admits: `RUN_B` reads `RUN_A`'s result inside the run, where
    // internal coupling never trades order.
    let moved = relocate(&source, &environment, RUN_A, RUN_B, HEAD).unwrap();
    assert_eq!(
        block_order(&moved.transformed().functions[0].blocks[1]),
        vec![RUN_A, RUN_B, HEAD, MID, TAIL]
    );
    // `RUN_A` alone cannot cross `RUN_B`, whose read of its result sits in
    // the member move's window — the single-member family refuses where the
    // run admits.
    assert_eq!(
        crate::relocate_selected_instruction_across_edge(
            &source,
            0,
            RUN_A,
            HEAD,
            &environment,
            budget(),
        )
        .unwrap_err(),
        crate::EdgeRelocationError::UnsupportedPair
    );
}

/// A member whose write feeds a crossed read keeps order: the crossed
/// `TRAIL` reading `RUN_A`'s result refuses, and a crossed `HEAD` reading
/// it refuses only once the window actually reaches it.
#[test]
fn raw_hazard_keeps_order_across_the_edge() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let tail_reads = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[3] = instruction(
            TRAIL,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[R_MOVE_A, R_TRAIL],
        );
    });
    assert_eq!(
        relocate(&tail_reads, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        EdgeRunRelocationError::UnsupportedPair
    );
    let head_reads = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[1].instructions[0] = instruction(
            HEAD,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[R_MOVE_A, R_HEAD],
        );
    });
    // Landing at the head does not cross `HEAD`; landing at `MID` crosses
    // it and the member's write would starve the crossed read.
    relocate(&head_reads, &environment, RUN_A, RUN_B, HEAD).unwrap();
    assert_eq!(
        relocate(&head_reads, &environment, RUN_A, RUN_B, MID).unwrap_err(),
        EdgeRunRelocationError::UnsupportedPair
    );
}

/// A member reading a location a crossed instruction writes would observe
/// the new value after the move.
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
        function.blocks[0].instructions[2] = instruction(
            RUN_B,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[POINTER, R_MOVE_B],
        );
        function.blocks[1].instructions[0] = instruction(
            HEAD,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(9),
            },
            &materialize,
            &[POINTER],
        );
    });
    relocate(&source, &environment, RUN_A, RUN_B, HEAD).unwrap();
    assert_eq!(
        relocate(&source, &environment, RUN_A, RUN_B, MID).unwrap_err(),
        EdgeRunRelocationError::UnsupportedPair
    );
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
        function.blocks[1].instructions[0] = instruction(
            HEAD,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(9),
            },
            &materialize,
            &[R_MOVE_A],
        );
    });
    assert_eq!(
        relocate(&source, &environment, RUN_A, RUN_B, MID).unwrap_err(),
        EdgeRunRelocationError::UnsupportedPair
    );
    relocate(&source, &environment, RUN_A, RUN_B, HEAD).unwrap();
}

/// Condition state couples like registers across the edge: a
/// flag-clobbering member cannot cross the compare publishing those flags,
/// and cannot cross the materialization reading them.
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
        function.blocks[0].instructions[1] = instruction(
            RUN_A,
            subtract_kind,
            &subtract,
            &[POINTER, R_MOVE_A, R_MOVE_A],
        );
        function.blocks[1].instructions[0] = instruction(
            HEAD,
            SelectedInstructionKind::CompareI64,
            &compare,
            &[R_HEAD, R_MID],
        );
    });
    assert_eq!(
        relocate(&flag_writer, &environment, RUN_A, RUN_B, MID).unwrap_err(),
        EdgeRunRelocationError::UnsupportedPair
    );
    let flag_reader = mutated(target, |function, environment| {
        let subtract = environment
            .constraint(environment.selected_keys().subtract_i64)
            .unwrap()
            .clone();
        let boolean = environment
            .constraint(environment.selected_keys().materialize_boolean)
            .unwrap()
            .clone();
        function.blocks[0].instructions[1] = instruction(
            RUN_A,
            subtract_kind,
            &subtract,
            &[POINTER, R_MOVE_A, R_MOVE_A],
        );
        function.blocks[1].instructions[0] = instruction(
            HEAD,
            SelectedInstructionKind::MaterializeBooleanEqual,
            &boolean,
            &[R_HEAD],
        );
    });
    assert_eq!(
        relocate(&flag_reader, &environment, RUN_A, RUN_B, MID).unwrap_err(),
        EdgeRunRelocationError::UnsupportedPair
    );
}

/// The `Jump` terminator is a crossed position, not a window barrier: a
/// register it reads couples like any crossed read, while a read it shares
/// with a member admits.
#[test]
fn terminator_position_couples() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The terminator reads a member's result: the member's write would
    // land after the read.
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
        relocate(&terminator_reads, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        EdgeRunRelocationError::UnsupportedPair
    );
    // The terminator reading only the entry parameter a member also reads
    // shares no written location: the run still crosses.
    let shared_read = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[2] = instruction(
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
    relocate(&shared_read, &environment, RUN_A, RUN_B, HEAD).unwrap();
}

/// The edge's register transports sit between the run's old and new
/// positions: a member defining the transported argument would hand the
/// binding a stale value, a member defining or reading the parameter would
/// be overwritten or observe the transported value, while a member merely
/// reading the argument crosses freely.
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
        HEAD,
    )
    .unwrap();
    // A member reading the transported argument reads the same register on
    // either side of the binding.
    relocate(
        &with_binding(&mut |function, environment| {
            let copy = environment
                .constraint(environment.selected_keys().copy_i64)
                .unwrap()
                .clone();
            function.blocks[0].instructions[2] = instruction(
                RUN_B,
                SelectedInstructionKind::CopyI64,
                &copy,
                &[POINTER, R_MOVE_B],
            );
        }),
        &environment,
        RUN_A,
        RUN_B,
        HEAD,
    )
    .unwrap();
    // A member defining the transported argument would hand the binding a
    // stale value once it landed past the transport.
    assert_eq!(
        relocate(
            &with_binding(&mut |function, environment| {
                let materialize = environment
                    .constraint(environment.selected_keys().materialize_i64)
                    .unwrap()
                    .clone();
                function.blocks[0].instructions[1] = instruction(
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
            HEAD,
        )
        .unwrap_err(),
        EdgeRunRelocationError::UnsupportedPair
    );
    // A member defining the parameter would be overwritten by the
    // transport on the far side.
    assert_eq!(
        relocate(
            &with_binding(&mut |function, environment| {
                let copy = environment
                    .constraint(environment.selected_keys().copy_i64)
                    .unwrap()
                    .clone();
                function.blocks[0].instructions[2] = instruction(
                    RUN_B,
                    SelectedInstructionKind::CopyI64,
                    &copy,
                    &[POINTER, R_BOUND],
                );
            }),
            &environment,
            RUN_A,
            RUN_B,
            HEAD,
        )
        .unwrap_err(),
        EdgeRunRelocationError::UnsupportedPair
    );
    // A member reading the parameter would observe the transported value
    // only after the move.
    assert_eq!(
        relocate(
            &with_binding(&mut |function, environment| {
                let copy = environment
                    .constraint(environment.selected_keys().copy_i64)
                    .unwrap()
                    .clone();
                function.blocks[0].instructions[1] = instruction(
                    RUN_A,
                    SelectedInstructionKind::CopyI64,
                    &copy,
                    &[R_BOUND, R_MOVE_A],
                );
            }),
            &environment,
            RUN_A,
            RUN_B,
            HEAD,
        )
        .unwrap_err(),
        EdgeRunRelocationError::UnsupportedPair
    );
}

/// The destination block must be reached by the crossed edge alone: a
/// second predecessor hands the run to a path that never executed it, the
/// entry block needs no predecessor at all, and a self-edge is the
/// in-block family's case.
#[test]
fn destination_block_needs_the_crossed_edge_alone() {
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
        relocate(&two_predecessors, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        EdgeRunRelocationError::UnsupportedPair
    );
    // The destination as entry block refuses even with the single edge.
    let entry_target = mutated(target, |function, _| {
        function.entry_block = BLOCK_B;
    });
    assert_eq!(
        relocate(&entry_target, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        EdgeRunRelocationError::UnsupportedPair
    );
    // A self-edge is the in-block family's case, not a cross-edge window.
    let self_edge = mutated(target, |function, _| {
        let successor = match &mut function.blocks[0].terminator {
            SelectedTerminator::Jump { successor, .. } => successor,
            _ => unreachable!(),
        };
        successor.block = BLOCK_A;
    });
    assert_eq!(
        relocate(&self_edge, &environment, RUN_A, RUN_B, LEAD).unwrap_err(),
        EdgeRunRelocationError::UnsupportedPair
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
            function.blocks[1].origin = origin;
        });
        assert_eq!(
            relocate(&implementation_target, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
            EdgeRunRelocationError::UnsupportedPair
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
            relocate(&continued, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
            EdgeRunRelocationError::UnsupportedPair
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
        relocate(&case_edge, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        EdgeRunRelocationError::UnsupportedPair
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
        relocate(&fueled, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        EdgeRunRelocationError::UnsupportedPair
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
        relocate(&structural, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        EdgeRunRelocationError::UnsupportedPair
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
    relocate(&inert_structural, &environment, RUN_A, RUN_B, HEAD).unwrap();
}

/// The run's block must end in the crossed `Jump`: a conditional
/// terminator keeps a second exit the run would still execute on after
/// relocating, and a return has no successor to land on.
#[test]
fn only_a_jump_edge_carries_the_run() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let conditional = mutated(target, |function, environment| {
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
        relocate(&conditional, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        EdgeRunRelocationError::UnsupportedPair
    );
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
        relocate(&returned, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        EdgeRunRelocationError::UnsupportedPair
    );
}

/// Calls, hosted effects, and terminator kinds are barriers as a run
/// member or anywhere inside the crossed window, and a call-roster row
/// makes an instruction a barrier even when its kind is register-pure —
/// the `Jump` terminator's own id included.
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
            relocate(&member_barrier, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
            EdgeRunRelocationError::UnsupportedInstruction,
            "member {kind:?}"
        );
        let tail_barrier = mutated(target, |function, _| {
            function.blocks[0].instructions[3].kind = kind;
        });
        assert_eq!(
            relocate(&tail_barrier, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
            EdgeRunRelocationError::UnsupportedInstruction,
            "crossed tail {kind:?}"
        );
        let head_barrier = mutated(target, |function, _| {
            function.blocks[1].instructions[0].kind = kind;
        });
        assert_eq!(
            relocate(&head_barrier, &environment, RUN_A, RUN_B, MID).unwrap_err(),
            EdgeRunRelocationError::UnsupportedInstruction,
            "crossed head {kind:?}"
        );
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
    for (instruction_id, destination) in [
        (RUN_A, HEAD),
        (RUN_B, HEAD),
        (TRAIL, HEAD),
        (HEAD, MID),
        (JUMP, HEAD),
    ] {
        let contract = mutated(target, |function, _| {
            function.calls.push(call_contract(instruction_id));
        });
        assert_eq!(
            relocate(&contract, &environment, RUN_A, RUN_B, destination).unwrap_err(),
            EdgeRunRelocationError::UnsupportedInstruction,
            "call roster {instruction_id:?}"
        );
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
            function.blocks[0].instructions[1] = instruction(
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
    // The roster-carrying run crosses row-less positions: the tail
    // materialization, the jump, and the empty edge.
    relocate(
        &mutated(target, |function, environment| {
            load_member(function, environment)
        }),
        &environment,
        RUN_A,
        RUN_B,
        HEAD,
    )
    .unwrap();
    // Two roster-carrying members may share the run — their recorded order
    // never changes against each other — while the window still admits
    // only row-less positions.
    relocate(
        &mutated(target, |function, environment| {
            load_member(function, environment);
            let load = environment
                .constraint(environment.selected_keys().load8.unwrap())
                .unwrap()
                .clone();
            function.blocks[0].instructions[2] = instruction(
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
        HEAD,
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
        function.blocks[0].instructions[3] = instruction(
            TRAIL,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            &store,
            &[POINTER, R_TRAIL],
        );
        function.memory_accesses.push(access(
            TRAIL,
            PlaceId::new(2).unwrap(),
            SelectedMemoryAccessRole::WritePlace,
        ));
    });
    assert_eq!(
        relocate(&accounted_tail, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        EdgeRunRelocationError::UnsupportedPair
    );
    let accounted_head = mutated(target, |function, environment| {
        load_member(function, environment);
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
        function.blocks[1].instructions[0] = instruction(
            HEAD,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            &store,
            &[POINTER, R_HEAD],
        );
        function.memory_accesses.push(access(
            HEAD,
            PlaceId::new(2).unwrap(),
            SelectedMemoryAccessRole::WritePlace,
        ));
    });
    relocate(&accounted_head, &environment, RUN_A, RUN_B, HEAD).unwrap();
    assert_eq!(
        relocate(&accounted_head, &environment, RUN_A, RUN_B, MID).unwrap_err(),
        EdgeRunRelocationError::UnsupportedPair
    );
    // A row recorded against the crossed edge itself counts as the edge
    // position's memory surface.
    let accounted_edge = mutated(target, |function, environment| {
        load_member(function, environment);
        function.memory_accesses.push(edge_access(RET, EDGE_AB));
    });
    assert_eq!(
        relocate(&accounted_edge, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        EdgeRunRelocationError::UnsupportedPair
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
        relocate(&accounted_jump, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        EdgeRunRelocationError::UnsupportedPair
    );
    // A row on another edge is not crossed: it names a position outside
    // the window.
    let foreign_edge = mutated(target, |function, environment| {
        load_member(function, environment);
        function.memory_accesses.push(edge_access(TAIL, 99));
    });
    relocate(&foreign_edge, &environment, RUN_A, RUN_B, HEAD).unwrap();
    // A row-less run crosses any accounted mix.
    let rowless = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[3] = instruction(
            TRAIL,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            &store,
            &[POINTER, R_TRAIL],
        );
        function.blocks[1].instructions[0] = instruction(
            HEAD,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            &store,
            &[POINTER, R_HEAD],
        );
        function.memory_accesses.push(access(
            TRAIL,
            PlaceId::new(1).unwrap(),
            SelectedMemoryAccessRole::WritePlace,
        ));
        function.memory_accesses.push(access(
            HEAD,
            PlaceId::new(2).unwrap(),
            SelectedMemoryAccessRole::WritePlace,
        ));
    });
    let result = relocate(&rowless, &environment, RUN_A, RUN_B, MID).unwrap();
    assert_eq!(
        result.transformed().functions[0].memory_accesses,
        rowless.transformed().functions[0].memory_accesses
    );
    // A memory-capable kind without a roster row is an unaccounted access:
    // the relocation cannot prove what it reaches, so it refuses outright
    // whether the bare access is a member or a crossed position.
    let bare_member = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[1] = instruction(
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
        relocate(&bare_member, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        EdgeRunRelocationError::UnsupportedInstruction
    );
    let bare_crossed = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[3] = instruction(
            TRAIL,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            &store,
            &[POINTER, R_TRAIL],
        );
    });
    assert_eq!(
        relocate(&bare_crossed, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        EdgeRunRelocationError::UnsupportedInstruction
    );
}

/// A boundary settlement positioned past the run's first index observed a
/// member inside the source block's executed prefix, and one positioned
/// past the landing index observes the run inside the destination's —
/// both refuse, while positions at or before either boundary keep the
/// executed set they always had.
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
                .push(settlement(BLOCK_A, position, 50));
        });
        let result = relocate(&settled, &environment, RUN_A, RUN_B, HEAD);
        assert_eq!(
            result.is_ok(),
            admits,
            "source-block settlement at {position}"
        );
    }
    // In the destination block the bound is the landing index: at or
    // before it the executed prefix is unchanged; past it the run joins
    // the prefix.
    for (position, admits) in [(0u32, true), (1, true), (2, false), (3, false)] {
        let settled = mutated(target, |function, _| {
            function
                .boundary_settlements
                .push(settlement(BLOCK_B, position, 51));
        });
        let result = relocate(&settled, &environment, RUN_A, RUN_B, MID);
        assert_eq!(
            result.is_ok(),
            admits,
            "target-block settlement at {position}"
        );
    }
    // Landing at the body end keeps every destination settlement: none
    // sits past the run's new span.
    let settled = mutated(target, |function, _| {
        function
            .boundary_settlements
            .push(settlement(BLOCK_B, 3, 52));
    });
    relocate(&settled, &environment, RUN_A, RUN_B, RET).unwrap();
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
    relocate(&elsewhere, &environment, RUN_A, RUN_B, HEAD).unwrap();
}

/// The triple must name one contiguous run in a block ending in the
/// crossed `Jump` and one position in the edge's target: a repeated or
/// misordered member, an unknown member, a wrong function, a destination
/// in another block, and a destination that is no instruction at all
/// refuse.
#[test]
fn only_the_named_run_relocates() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // An unknown member or function index never locates the window.
    assert_eq!(
        relocate_selected_run_across_edge(
            &source,
            0,
            SelectedInstructionId(99),
            RUN_B,
            HEAD,
            &environment,
            budget(),
        )
        .unwrap_err(),
        EdgeRunRelocationError::SourceMismatch
    );
    assert_eq!(
        relocate_selected_run_across_edge(&source, 9, RUN_A, RUN_B, HEAD, &environment, budget(),)
            .unwrap_err(),
        EdgeRunRelocationError::SourceMismatch
    );
    // A repeated member bounds no multi-member run, and a last member that
    // does not follow the first names none either — a single member is
    // the one-instruction family's case.
    assert_eq!(
        relocate(&source, &environment, RUN_A, RUN_A, HEAD).unwrap_err(),
        EdgeRunRelocationError::UnsupportedPair
    );
    assert_eq!(
        relocate(&source, &environment, RUN_B, RUN_A, HEAD).unwrap_err(),
        EdgeRunRelocationError::UnsupportedPair
    );
    // The destination must name a position in the target block — a member
    // of the run's own block and a dangling id both refuse.
    for destination in [LEAD, SelectedInstructionId(99)] {
        assert_eq!(
            relocate(&source, &environment, RUN_A, RUN_B, destination).unwrap_err(),
            EdgeRunRelocationError::UnsupportedPair
        );
    }
    // A run in a block without the jump edge names no window: B has only
    // a return terminator.
    assert_eq!(
        relocate(&source, &environment, HEAD, TAIL, MID).unwrap_err(),
        EdgeRunRelocationError::UnsupportedPair
    );
    // Naming the run's own block terminator as member is not a body
    // position.
    assert_eq!(
        relocate(&source, &environment, JUMP, TRAIL, HEAD).unwrap_err(),
        EdgeRunRelocationError::SourceMismatch
    );
    // Naming the source block's terminator as destination names a position
    // in the wrong block.
    assert_eq!(
        relocate(&source, &environment, RUN_A, RUN_B, JUMP).unwrap_err(),
        EdgeRunRelocationError::UnsupportedPair
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
    validate_edge_run_relocation(
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
    let run: Vec<_> = displaced.functions[0].blocks[1]
        .instructions
        .drain(1..=2)
        .collect();
    displaced.functions[0].blocks[1]
        .instructions
        .splice(3..3, run);
    assert_eq!(
        validate_edge_run_relocation(
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
        EdgeRunRelocationError::ReplayMismatch
    );
    // A permuted run rejects: the members keep their order or stay home.
    let mut permuted = result.transformed().clone();
    permuted.functions[0].blocks[1].instructions.swap(1, 2);
    assert_eq!(
        validate_edge_run_relocation(
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
        EdgeRunRelocationError::ReplayMismatch
    );
    // The run left in its own block rejects.
    let mut unmoved = result.transformed().clone();
    let run: Vec<_> = unmoved.functions[0].blocks[1]
        .instructions
        .drain(1..=2)
        .collect();
    unmoved.functions[0].blocks[0]
        .instructions
        .splice(1..1, run);
    assert_eq!(
        validate_edge_run_relocation(
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
        EdgeRunRelocationError::ReplayMismatch
    );
    // A dropped instruction anywhere rejects.
    let mut dropped = result.transformed().clone();
    dropped.functions[0].blocks[0].instructions.pop();
    assert_eq!(
        validate_edge_run_relocation(
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
        EdgeRunRelocationError::ReplayMismatch
    );
    // An unrelated literal edit inside the source block rejects.
    let mut edited = result.transformed().clone();
    edited.functions[0].blocks[0].instructions[0].kind = SelectedInstructionKind::MaterializeI64 {
        value: IntegerValue::Unsigned(12),
    };
    assert_eq!(
        validate_edge_run_relocation(
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
        EdgeRunRelocationError::ReplayMismatch
    );
    // Naming a different window on the same proposal re-derives a
    // different landing and rejects.
    assert_eq!(
        validate_edge_run_relocation(
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
        EdgeRunRelocationError::ReplayMismatch
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
    // x86-64: with the members materializations and `HEAD` the
    // destination, the charge is (2 blocks + 7 body instructions) for the
    // member scan + (7 body + 2 terminator instructions) for the
    // predecessor scan + member-against-crossed surfaces for each member
    // against `TRAIL` (1+1) and the jump (1+2) + empty rosters = 9 + 9 +
    // 2*5 = 28. Destination `MID` adds the crossed `HEAD` pair (1+1) per
    // member for 32.
    for (destination, exact_steps) in [(HEAD, 28u64), (MID, 32u64)] {
        let source = fixture(target);
        let exact = OptimizationWorkBudget::new(1, 1, exact_steps, 1, 1).unwrap();
        let result = relocate_selected_run_across_edge(
            &source,
            0,
            RUN_A,
            RUN_B,
            destination,
            &environment,
            exact,
        )
        .unwrap();
        validate_edge_run_relocation(
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
            relocate_selected_run_across_edge(
                &source,
                0,
                RUN_A,
                RUN_B,
                destination,
                &environment,
                starved,
            )
            .unwrap_err(),
            EdgeRunRelocationError::WorkBudgetExceeded
        );
        assert_eq!(
            validate_edge_run_relocation(
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
            EdgeRunRelocationError::WorkBudgetExceeded
        );
    }
}

/// The environment's target must be the plan's target: a relocation proven
/// for one target's constraints is not evidence on another.
#[test]
fn target_mismatch_rejects() {
    let source = fixture(NativeTarget::linux_x64());
    let environment = baseline_target_register_environment(NativeTarget::linux_arm64()).unwrap();
    assert_eq!(
        relocate(&source, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        EdgeRunRelocationError::SourceMismatch
    );
}

/// Two runs over the identical source produce the identical validated
/// result, and the published plan is a legal second input through the
/// sealed analysis boundary: the trailing member crosses the same edge
/// onto it, a hazard-coupled member still declines, and an in-block
/// family still admits.
#[test]
fn edge_run_relocation_is_deterministic_and_re_admitted() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let first = relocate(&source, &environment, RUN_A, RUN_B, HEAD).unwrap();
    let second = relocate(&source, &environment, RUN_A, RUN_B, HEAD).unwrap();
    assert_eq!(first, second);
    // The validated output carries the sealed analysis boundary, so it is
    // a legal second input — not merely a reconstruction of one. The tail
    // member crosses the same edge onto it.
    let again = crate::relocate_selected_instruction_across_edge(
        &first,
        0,
        TRAIL,
        HEAD,
        &environment,
        budget(),
    )
    .unwrap();
    assert_eq!(
        block_order(&again.transformed().functions[0].blocks[1]),
        vec![RUN_A, RUN_B, TRAIL, HEAD, MID, TAIL]
    );
    assert_eq!(
        again.receipt().source_selected(),
        first.receipt().transformed_selected()
    );
    // A hazard-coupled member still declines on the second input: once
    // the run sits at B's head, `HEAD` reading `R_TRAIL` ahead of `MID`
    // keeps `TRAIL` from crossing it to land on `MID`'s position.
    let coupled_source = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[1].instructions[0] = instruction(
            HEAD,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[R_TRAIL, R_HEAD],
        );
    });
    let moved = relocate(&coupled_source, &environment, RUN_A, RUN_B, HEAD).unwrap();
    assert_eq!(
        crate::relocate_selected_instruction_across_edge(
            &moved,
            0,
            TRAIL,
            MID,
            &environment,
            budget(),
        )
        .unwrap_err(),
        crate::EdgeRelocationError::UnsupportedPair
    );
    // A different scheduling family still admits on the second input.
    let swapped =
        crate::relocate_selected_run(&first, 0, MID, TAIL, HEAD, &environment, budget()).unwrap();
    assert_eq!(
        block_order(&swapped.transformed().functions[0].blocks[1]),
        vec![RUN_A, RUN_B, MID, TAIL, HEAD]
    );
}
