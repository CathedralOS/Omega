use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget};
use optimization_unit::{EffectLink, FuelSettlement, PsiProvenance, ValueDefinitionSite};
use register_environment::baseline_target_register_environment;
use register_model::RegisterInstructionConstraint;
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedBoundarySettlement,
    SelectedBoundarySettlementPayload, SelectedCallContract, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan, SelectedMemoryAccess,
    SelectedMemoryAccessOrigin, SelectedMemoryAccessRole, SelectedSuccessor, SelectedSuccessorRole,
    SelectedTerminator, SelectedValueBinding, SelectedValueTransport, VirtualRegister,
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
    ScheduledRelocationError, ScheduledRelocationReceipt, ValidatedScheduledRelocation,
    relocate_scheduled_run, validate_scheduled_relocation,
};
use crate::rewrites::test_support::{budget, instruction};

const LEAD: SelectedInstructionId = SelectedInstructionId(2);
const MOVING: SelectedInstructionId = SelectedInstructionId(3);
const SECOND: SelectedInstructionId = SelectedInstructionId(4);
const TRAIL: SelectedInstructionId = SelectedInstructionId(5);
const C_HEAD: SelectedInstructionId = SelectedInstructionId(6);
const C_TAIL: SelectedInstructionId = SelectedInstructionId(7);
const T_HEAD: SelectedInstructionId = SelectedInstructionId(8);
const T_TAIL: SelectedInstructionId = SelectedInstructionId(9);
const J_HEAD: SelectedInstructionId = SelectedInstructionId(10);
const J_TAIL: SelectedInstructionId = SelectedInstructionId(11);
const JUMP_B: SelectedInstructionId = SelectedInstructionId(12);
const JUMP_C: SelectedInstructionId = SelectedInstructionId(13);
const JUMP_T: SelectedInstructionId = SelectedInstructionId(14);
const RET: SelectedInstructionId = SelectedInstructionId(15);
const BRANCH: SelectedInstructionId = SelectedInstructionId(16);
const A_HEAD: SelectedInstructionId = SelectedInstructionId(17);
const A_TAIL: SelectedInstructionId = SelectedInstructionId(18);
const A_JUMP: SelectedInstructionId = SelectedInstructionId(19);
const F_HEAD: SelectedInstructionId = SelectedInstructionId(20);
const F_TAIL: SelectedInstructionId = SelectedInstructionId(21);
const F_JUMP: SelectedInstructionId = SelectedInstructionId(22);
const T_JUMP: SelectedInstructionId = SelectedInstructionId(23);
const HEAD: SelectedInstructionId = SelectedInstructionId(24);
const LOOP_J: SelectedInstructionId = SelectedInstructionId(26);
const F_INSERT: SelectedInstructionId = SelectedInstructionId(27);

const POINTER: VirtualRegisterId = VirtualRegisterId(0);
const R_LEAD: VirtualRegisterId = VirtualRegisterId(1);
const R_MOVE: VirtualRegisterId = VirtualRegisterId(2);
const R_SECOND: VirtualRegisterId = VirtualRegisterId(3);
const R_TRAIL: VirtualRegisterId = VirtualRegisterId(4);
const R_CHEAD: VirtualRegisterId = VirtualRegisterId(5);
const R_CTAIL: VirtualRegisterId = VirtualRegisterId(6);
const R_THEAD: VirtualRegisterId = VirtualRegisterId(7);
const R_TTAIL: VirtualRegisterId = VirtualRegisterId(8);
const R_JHEAD: VirtualRegisterId = VirtualRegisterId(9);
const R_JTAIL: VirtualRegisterId = VirtualRegisterId(10);
const R_AHEAD: VirtualRegisterId = VirtualRegisterId(11);
const R_ATAIL: VirtualRegisterId = VirtualRegisterId(12);
const R_FHEAD: VirtualRegisterId = VirtualRegisterId(13);
const R_FTAIL: VirtualRegisterId = VirtualRegisterId(14);
const R_HEAD: VirtualRegisterId = VirtualRegisterId(15);

const BLOCK_B: SelectedBlockId = SelectedBlockId(0);
const BLOCK_C: SelectedBlockId = SelectedBlockId(1);
const BLOCK_T: SelectedBlockId = SelectedBlockId(2);
const BLOCK_J: SelectedBlockId = SelectedBlockId(3);
const BLOCK_A: SelectedBlockId = SelectedBlockId(4);
const BLOCK_F: SelectedBlockId = SelectedBlockId(5);
const EDGE_BC: u64 = 20;
const EDGE_CT: u64 = 21;
const EDGE_TJ: u64 = 22;
const EDGE_BA: u64 = 30;
const EDGE_BF: u64 = 31;
const EDGE_AT: u64 = 32;
const EDGE_FJ: u64 = 33;

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

fn jump_terminator(jump: SelectedInstruction, successor: SelectedSuccessor) -> SelectedTerminator {
    SelectedTerminator::Jump {
        instruction: jump,
        successor,
    }
}

struct FixtureParts {
    materialize: RegisterInstructionConstraint,
    branch_row: RegisterInstructionConstraint,
    jump_row: RegisterInstructionConstraint,
    return_row: RegisterInstructionConstraint,
    class: register_model::RegisterClassId,
    scalar_type: ScalarType,
    machine: MachineId,
}

fn parts(target: NativeTarget) -> FixtureParts {
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let materialize = environment
        .constraint(keys.materialize_i64)
        .unwrap()
        .clone();
    let branch_row = environment
        .constraint(keys.conditional_branch)
        .unwrap()
        .clone();
    let jump_row = environment.constraint(keys.jump).unwrap().clone();
    let return_row = environment.constraint(keys.return_unit).unwrap().clone();
    let class = materialize.operands[0].class;
    FixtureParts {
        materialize,
        branch_row,
        jump_row,
        return_row,
        class,
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        machine: MachineId::new(1).unwrap(),
    }
}

fn result_register(
    class: register_model::RegisterClassId,
    id: VirtualRegisterId,
    instruction: SelectedInstructionId,
    value: u64,
) -> VirtualRegister {
    register(
        id,
        class,
        VirtualRegisterOrigin::InstructionResult {
            instruction,
            source_value: ValueId::new(value).unwrap(),
        },
    )
}

fn materialization(
    parts: &FixtureParts,
    id: SelectedInstructionId,
    register: VirtualRegisterId,
    value: u64,
) -> SelectedInstruction {
    instruction(
        id,
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(value.into()),
        },
        &parts.materialize,
        &[register],
    )
}

fn wrap(target: NativeTarget, function: SelectedFunction) -> ValidatedScheduledRelocation {
    let plan = SelectedInstructionPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([1; 32]),
        },
        fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
        target,
        entry: function.machine,
        functions: vec![function].into(),
    };
    let identity = selected_instruction_plan_identity(&plan);
    ValidatedScheduledRelocation {
        receipt: ScheduledRelocationReceipt {
            source_selected: identity,
            transformed_selected: identity,
            optimization_unit: OptimizationUnitIdentity::from_bytes([2; 32]),
            fuel_schedule: plan.fuel_schedule,
        },
        transformed: std::sync::Arc::new(plan),
    }
}

/// A raw selected-stage unit fixture, not a source/Terminal admission
/// claim. A sole-successor chain of four blocks:
/// `B = [LEAD; MOVING; SECOND; TRAIL] -> jump -> C = [C_HEAD; C_TAIL]
/// -> jump -> T = [T_HEAD; T_TAIL] -> jump -> J = [J_HEAD; J_TAIL]
/// -> return`. The default move relocates the run `[MOVING; SECOND]`
/// onto `T_HEAD`'s position, crossing `TRAIL`, B's terminator, C's whole
/// stream, and the two chain edges — a window none of the per-shape
/// families enumerates, since it crosses two block boundaries.
fn chain_fixture(target: NativeTarget) -> ValidatedScheduledRelocation {
    let parts = parts(target);
    let registers = vec![
        VirtualRegister {
            id: POINTER,
            scalar_type: parts.scalar_type,
            class: parts.class,
            origin: VirtualRegisterOrigin::EntryParameter {
                source_value: ValueId::new(1).unwrap(),
                parameter_index: 0,
            },
            definition_site: Some(ValueDefinitionSite::FunctionParameter(0)),
            entry_fixed_view: None,
        },
        result_register(parts.class, R_LEAD, LEAD, 2),
        result_register(parts.class, R_MOVE, MOVING, 3),
        result_register(parts.class, R_SECOND, SECOND, 4),
        result_register(parts.class, R_TRAIL, TRAIL, 5),
        result_register(parts.class, R_CHEAD, C_HEAD, 6),
        result_register(parts.class, R_CTAIL, C_TAIL, 7),
        result_register(parts.class, R_THEAD, T_HEAD, 8),
        result_register(parts.class, R_TTAIL, T_TAIL, 9),
        result_register(parts.class, R_JHEAD, J_HEAD, 10),
        result_register(parts.class, R_JTAIL, J_TAIL, 11),
    ];
    let materialize = |id, register, value| materialization(&parts, id, register, value);
    let function = SelectedFunction {
        machine: parts.machine,
        attachment: None,
        provenance: Default::default(),
        structural: None,
        local_storage_slots: Vec::new(),
        outgoing_arguments: Vec::new(),
        calls: Vec::new(),
        normalized_foreign_calls: Vec::new(),
        memory_accesses: Vec::new(),
        boundary_settlements: Vec::new(),
        entry_block: BLOCK_B,
        virtual_registers: registers,
        blocks: vec![
            SelectedBlock {
                id: BLOCK_B,
                origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
                instructions: vec![
                    materialize(LEAD, R_LEAD, 5),
                    materialize(MOVING, R_MOVE, 7),
                    materialize(SECOND, R_SECOND, 8),
                    materialize(TRAIL, R_TRAIL, 9),
                ],
                terminator: jump_terminator(
                    instruction(JUMP_B, SelectedInstructionKind::Jump, &parts.jump_row, &[]),
                    successor(BLOCK_C, BlockId::new(2).unwrap(), EDGE_BC),
                ),
            },
            SelectedBlock {
                id: BLOCK_C,
                origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
                instructions: vec![
                    materialize(C_HEAD, R_CHEAD, 11),
                    materialize(C_TAIL, R_CTAIL, 13),
                ],
                terminator: jump_terminator(
                    instruction(JUMP_C, SelectedInstructionKind::Jump, &parts.jump_row, &[]),
                    successor(BLOCK_T, BlockId::new(3).unwrap(), EDGE_CT),
                ),
            },
            SelectedBlock {
                id: BLOCK_T,
                origin: SelectedBlockOrigin::Source(BlockId::new(3).unwrap()),
                instructions: vec![
                    materialize(T_HEAD, R_THEAD, 15),
                    materialize(T_TAIL, R_TTAIL, 17),
                ],
                terminator: jump_terminator(
                    instruction(JUMP_T, SelectedInstructionKind::Jump, &parts.jump_row, &[]),
                    successor(BLOCK_J, BlockId::new(4).unwrap(), EDGE_TJ),
                ),
            },
            SelectedBlock {
                id: BLOCK_J,
                origin: SelectedBlockOrigin::Source(BlockId::new(4).unwrap()),
                instructions: vec![
                    materialize(J_HEAD, R_JHEAD, 19),
                    materialize(J_TAIL, R_JTAIL, 21),
                ],
                terminator: SelectedTerminator::Return {
                    instruction: instruction(
                        RET,
                        SelectedInstructionKind::ReturnUnit,
                        &parts.return_row,
                        &[],
                    ),
                    psi_return_edge: EdgeId::new(25).unwrap(),
                },
            },
        ],
    };
    wrap(target, function)
}

/// A second fixture: B's terminator is a two-edge conditional branch whose
/// nonzero edge enters arm A and whose zero edge enters arm F; A jumps to
/// T and F jumps to J:
/// `B -> branch -> A = [A_HEAD; A_TAIL] -> jump -> T = [T_HEAD] -> jump -> J`
/// and `F = [F_HEAD; F_TAIL] -> jump -> J = [HEAD] -> return`. Moving
/// `[MOVING]` onto `T_HEAD` sinks it through the fork's nonzero edge and
/// through A's whole stream — a window the fork family cannot name, since
/// its landing arm must be the branch's immediate target. The zero edge
/// is the skipped path: `R_MOVE` must die on F's traversal of J.
fn fork_deep_fixture(target: NativeTarget) -> ValidatedScheduledRelocation {
    let parts = parts(target);
    let registers = vec![
        VirtualRegister {
            id: POINTER,
            scalar_type: parts.scalar_type,
            class: parts.class,
            origin: VirtualRegisterOrigin::EntryParameter {
                source_value: ValueId::new(1).unwrap(),
                parameter_index: 0,
            },
            definition_site: Some(ValueDefinitionSite::FunctionParameter(0)),
            entry_fixed_view: None,
        },
        result_register(parts.class, R_LEAD, LEAD, 2),
        result_register(parts.class, R_MOVE, MOVING, 3),
        result_register(parts.class, R_TRAIL, TRAIL, 5),
        result_register(parts.class, R_AHEAD, A_HEAD, 6),
        result_register(parts.class, R_ATAIL, A_TAIL, 7),
        result_register(parts.class, R_FHEAD, F_HEAD, 8),
        result_register(parts.class, R_FTAIL, F_TAIL, 9),
        result_register(parts.class, R_THEAD, T_HEAD, 10),
        result_register(parts.class, R_HEAD, HEAD, 11),
    ];
    let materialize = |id, register, value| materialization(&parts, id, register, value);
    let function = SelectedFunction {
        machine: parts.machine,
        attachment: None,
        provenance: Default::default(),
        structural: None,
        local_storage_slots: Vec::new(),
        outgoing_arguments: Vec::new(),
        calls: Vec::new(),
        normalized_foreign_calls: Vec::new(),
        memory_accesses: Vec::new(),
        boundary_settlements: Vec::new(),
        entry_block: BLOCK_B,
        virtual_registers: registers,
        blocks: vec![
            SelectedBlock {
                id: BLOCK_B,
                origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
                instructions: vec![
                    materialize(LEAD, R_LEAD, 5),
                    materialize(MOVING, R_MOVE, 7),
                    materialize(TRAIL, R_TRAIL, 9),
                ],
                terminator: SelectedTerminator::ConditionalBranch {
                    instruction: instruction(
                        BRANCH,
                        SelectedInstructionKind::ConditionalBranchNonZero,
                        &parts.branch_row,
                        &[],
                    ),
                    when_nonzero: successor(BLOCK_A, BlockId::new(2).unwrap(), EDGE_BA),
                    when_zero: successor(BLOCK_F, BlockId::new(3).unwrap(), EDGE_BF),
                },
            },
            SelectedBlock {
                id: BLOCK_A,
                origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
                instructions: vec![
                    materialize(A_HEAD, R_AHEAD, 11),
                    materialize(A_TAIL, R_ATAIL, 13),
                ],
                terminator: jump_terminator(
                    instruction(A_JUMP, SelectedInstructionKind::Jump, &parts.jump_row, &[]),
                    successor(BLOCK_T, BlockId::new(4).unwrap(), EDGE_AT),
                ),
            },
            SelectedBlock {
                id: BLOCK_F,
                origin: SelectedBlockOrigin::Source(BlockId::new(3).unwrap()),
                instructions: vec![
                    materialize(F_HEAD, R_FHEAD, 15),
                    materialize(F_TAIL, R_FTAIL, 17),
                ],
                terminator: jump_terminator(
                    instruction(F_JUMP, SelectedInstructionKind::Jump, &parts.jump_row, &[]),
                    successor(BLOCK_J, BlockId::new(5).unwrap(), EDGE_FJ),
                ),
            },
            SelectedBlock {
                id: BLOCK_T,
                origin: SelectedBlockOrigin::Source(BlockId::new(4).unwrap()),
                instructions: vec![materialize(T_HEAD, R_THEAD, 19)],
                terminator: jump_terminator(
                    instruction(T_JUMP, SelectedInstructionKind::Jump, &parts.jump_row, &[]),
                    successor(BLOCK_J, BlockId::new(5).unwrap(), 34),
                ),
            },
            SelectedBlock {
                id: BLOCK_J,
                origin: SelectedBlockOrigin::Source(BlockId::new(5).unwrap()),
                instructions: vec![materialize(HEAD, R_HEAD, 21)],
                terminator: SelectedTerminator::Return {
                    instruction: instruction(
                        RET,
                        SelectedInstructionKind::ReturnUnit,
                        &parts.return_row,
                        &[],
                    ),
                    psi_return_edge: EdgeId::new(35).unwrap(),
                },
            },
        ],
    };
    wrap(target, function)
}

fn mutated(
    target: NativeTarget,
    forked: bool,
    edit: impl FnOnce(&mut SelectedFunction, &register_environment::ValidatedTargetRegisterEnvironment),
) -> ValidatedScheduledRelocation {
    let environment = baseline_target_register_environment(target).unwrap();
    let mut source = if forked {
        fork_deep_fixture(target)
    } else {
        chain_fixture(target)
    };
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
    source: &ValidatedScheduledRelocation,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    members: &[SelectedInstructionId],
    destination: SelectedInstructionId,
) -> Result<ValidatedScheduledRelocation, ScheduledRelocationError> {
    relocate_scheduled_run(source, 0, members, destination, environment, budget())
}

/// The run sinks through the chain onto the destination's position on
/// every target: `[MOVING; SECOND]` leave B's body contiguous, cross
/// `TRAIL`, both chain edges, and C's whole stream, and land at T's head —
/// `T = [MOVING, SECOND, T_HEAD, T_TAIL]` — with every other position
/// keeping its order and the replayed proposal restoring the source
/// bit-identically.
#[test]
fn run_relocates_through_a_sole_successor_chain() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = chain_fixture(target);
        let result = relocate(&source, &environment, &[MOVING, SECOND], T_HEAD).unwrap();
        let original = &source.transformed().functions[0];
        let moved = &result.transformed().functions[0];
        assert_eq!(
            moved.blocks[0]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![LEAD, TRAIL]
        );
        assert_eq!(
            moved.blocks[2]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![MOVING, SECOND, T_HEAD, T_TAIL]
        );
        assert_eq!(
            moved.blocks[1].instructions,
            original.blocks[1].instructions
        );
        assert_eq!(
            moved.blocks[3].instructions,
            original.blocks[3].instructions
        );
        assert_eq!(
            moved.blocks[2].instructions[0],
            original.blocks[0].instructions[1]
        );
        assert_eq!(
            moved.blocks[2].instructions[1],
            original.blocks[0].instructions[2]
        );
        for index in 0..4 {
            assert_eq!(
                moved.blocks[index].terminator,
                original.blocks[index].terminator
            );
        }
        validate_scheduled_relocation(
            &source,
            0,
            &[MOVING, SECOND],
            T_HEAD,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
    }
}

/// The destination names the landing position directly: a body
/// instruction's index, the body end for a terminator-carried
/// instruction, or a position mid-body in a block the path crosses.
#[test]
fn run_lands_at_the_named_position() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = chain_fixture(target);
    let middle = relocate(&source, &environment, &[MOVING], T_TAIL).unwrap();
    assert_eq!(
        middle.transformed().functions[0].blocks[2]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![T_HEAD, MOVING, T_TAIL]
    );
    let body_end = relocate(&source, &environment, &[MOVING], JUMP_T).unwrap();
    assert_eq!(
        body_end.transformed().functions[0].blocks[2]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![T_HEAD, T_TAIL, MOVING]
    );
    // Landing in C is also admitted: the run then crosses only its own
    // block's tail, terminator, and the first chain edge.
    let earlier = relocate(&source, &environment, &[MOVING], C_TAIL).unwrap();
    assert_eq!(
        earlier.transformed().functions[0].blocks[1]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![C_HEAD, MOVING, C_TAIL]
    );
}

/// A degenerate fork whose both edges reach one arm still sinks the run:
/// every traversal of the branch block still executes it exactly once, so
/// the skipped-edge set is empty and the dead-path audit has nothing to
/// clear. This is the boundary between the sink that abandons traversals
/// and the one that does not.
#[test]
fn a_degenerate_single_arm_fork_sinks_with_no_skipped_path() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let single = mutated(target, true, |function, _| {
        let successor = match &mut function.blocks[0].terminator {
            SelectedTerminator::ConditionalBranch { when_zero, .. } => when_zero,
            _ => unreachable!(),
        };
        successor.block = BLOCK_A;
        successor.source_target = BlockId::new(2).unwrap();
    });
    let result = relocate(&single, &environment, &[MOVING], A_HEAD).unwrap();
    let moved = &result.transformed().functions[0];
    assert_eq!(
        moved.blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![LEAD, TRAIL]
    );
    assert_eq!(
        moved.blocks[1]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![MOVING, A_HEAD, A_TAIL]
    );
    validate_scheduled_relocation(
        &single,
        0,
        &[MOVING],
        A_HEAD,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
}

/// The same fork with the landing in the branch's immediate target: the
/// member leaves the fork head, crosses only the nonzero edge, and takes
/// `A_HEAD`'s position in the arm A itself. That is the window the per-shape
/// fork family named, and it is the shallow case of the derived region — the
/// zero edge is still the skipped path the dead-path audit clears.
#[test]
fn member_sinks_into_the_immediate_fork_arm() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fork_deep_fixture(target);
    let result = relocate(&source, &environment, &[MOVING], A_HEAD).unwrap();
    let moved = &result.transformed().functions[0];
    assert_eq!(
        moved.blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![LEAD, TRAIL]
    );
    assert_eq!(
        moved.blocks[1]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![MOVING, A_HEAD, A_TAIL]
    );
    validate_scheduled_relocation(
        &source,
        0,
        &[MOVING],
        A_HEAD,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
}

/// A conditional sink: the member leaves the fork head, crosses the
/// nonzero edge and A's whole stream, and lands in the deeper dominated
/// block T — the zero edge's path loses the member, whose write must die
/// before J.
#[test]
fn member_sinks_beneath_a_fork_arm() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fork_deep_fixture(target);
    let result = relocate(&source, &environment, &[MOVING], T_HEAD).unwrap();
    let original = &source.transformed().functions[0];
    let moved = &result.transformed().functions[0];
    assert_eq!(
        moved.blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![LEAD, TRAIL]
    );
    assert_eq!(
        moved.blocks[3]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![MOVING, T_HEAD]
    );
    validate_scheduled_relocation(
        &source,
        0,
        &[MOVING],
        T_HEAD,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    assert_eq!(
        moved.blocks[3].instructions[0],
        original.blocks[0].instructions[1]
    );
}

/// The run must occupy one contiguous body span in its own order.
#[test]
fn run_must_be_contiguous() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = chain_fixture(target);
    assert_eq!(
        relocate(&source, &environment, &[MOVING, TRAIL], T_HEAD).unwrap_err(),
        ScheduledRelocationError::UnsupportedPair
    );
    assert_eq!(
        relocate(&source, &environment, &[SECOND, MOVING], T_HEAD).unwrap_err(),
        ScheduledRelocationError::UnsupportedPair
    );
    assert_eq!(
        relocate(&source, &environment, &[], T_HEAD).unwrap_err(),
        ScheduledRelocationError::UnsupportedPair
    );
    assert_eq!(
        relocate(&source, &environment, &[SelectedInstructionId(900)], T_HEAD).unwrap_err(),
        ScheduledRelocationError::SourceMismatch
    );
}

/// Only a cross-block window bounds the move: a destination inside the
/// run's own block stays with the in-block families.
#[test]
fn same_block_destinations_refuse() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = chain_fixture(target);
    assert_eq!(
        relocate(&source, &environment, &[MOVING], TRAIL).unwrap_err(),
        ScheduledRelocationError::UnsupportedPair
    );
    assert_eq!(
        relocate(&source, &environment, &[MOVING], JUMP_B).unwrap_err(),
        ScheduledRelocationError::UnsupportedPair
    );
}

/// The destination's block must be dominated by the run's: an inflow
/// beside the window would gain a run that never executed on that path.
#[test]
fn undominated_destinations_refuse() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A member in arm A cannot sink into the join J: F's edge reaches J
    // without ever running A, so those traversals would gain the member.
    let source = fork_deep_fixture(target);
    assert_eq!(
        relocate(&source, &environment, &[A_HEAD], HEAD).unwrap_err(),
        ScheduledRelocationError::UnsupportedPair
    );
    // A destination unreachable from the run's block has no crossing path.
    let chained = chain_fixture(target);
    assert_eq!(
        relocate(
            &chained,
            &environment,
            &[MOVING],
            SelectedInstructionId(900)
        )
        .unwrap_err(),
        ScheduledRelocationError::UnsupportedPair
    );
}

/// A destination re-enterable from its own successors would run the run
/// again where the source ran it once.
#[test]
fn reenterable_destinations_refuse() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Loop J back into T: the landing block now belongs to a cycle.
    let looped = mutated(target, false, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        function.blocks[3].terminator = jump_terminator(
            instruction(LOOP_J, SelectedInstructionKind::Jump, &jump_row, &[]),
            successor(BLOCK_T, BlockId::new(3).unwrap(), 41),
        );
    });
    assert_eq!(
        relocate(&looped, &environment, &[MOVING], T_HEAD).unwrap_err(),
        ScheduledRelocationError::UnsupportedPair
    );
}

/// The window's crossed positions keep the order they can observe: a
/// crossed instruction reading or writing a member's location refuses.
#[test]
fn crossed_hazards_keep_order() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A crossed position writing the member's register (WAW).
    let crossed_waw = mutated(target, false, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[1].instructions[0] = instruction(
            C_HEAD,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[R_JHEAD, R_MOVE],
        );
    });
    assert_eq!(
        relocate(&crossed_waw, &environment, &[MOVING], T_HEAD).unwrap_err(),
        ScheduledRelocationError::UnsupportedPair
    );
    // A crossed position reading the member's register (RAW).
    let crossed_raw = mutated(target, false, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[1].instructions[0] = instruction(
            C_HEAD,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[R_MOVE, R_CHEAD],
        );
    });
    assert_eq!(
        relocate(&crossed_raw, &environment, &[MOVING], T_HEAD).unwrap_err(),
        ScheduledRelocationError::UnsupportedPair
    );
    // Landing below the reading position keeps it uncrossed: T_TAIL reads
    // R_MOVE but sits after the landing index at T_HEAD.
    let late_read = mutated(target, false, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[2].instructions[1] = instruction(
            T_TAIL,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[R_MOVE, R_TTAIL],
        );
    });
    relocate(&late_read, &environment, &[MOVING], T_HEAD).unwrap();
    assert_eq!(
        relocate(&late_read, &environment, &[MOVING], JUMP_T).unwrap_err(),
        ScheduledRelocationError::UnsupportedPair
    );
}

/// Calls and barrier kinds may never sit inside a crossed window, and the
/// run's members must be pure register work.
#[test]
fn calls_barriers_and_memory_bound_the_window() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A call contract on a crossed body position.
    let called = mutated(target, false, |function, _environment| {
        function.calls.push(SelectedCallContract {
            instruction: C_TAIL,
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
        });
    });
    assert_eq!(
        relocate(&called, &environment, &[MOVING], T_HEAD).unwrap_err(),
        ScheduledRelocationError::UnsupportedInstruction
    );
    // A member carrying a memory roster row.
    let rostered = mutated(target, false, |function, _environment| {
        function.memory_accesses.push(access(
            MOVING,
            PlaceId::new(2).unwrap(),
            SelectedMemoryAccessRole::ReadPlace,
        ));
    });
    assert_eq!(
        relocate(&rostered, &environment, &[MOVING], T_HEAD).unwrap_err(),
        ScheduledRelocationError::UnsupportedInstruction
    );
}

/// A crossed edge must be a plain semantic successor the run's transports
/// do not interfere with.
#[test]
fn only_plain_edges_carry_the_run() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Fuel on the crossed B->C edge.
    let fueled = mutated(target, false, |function, environment| {
        let mut edge = successor(BLOCK_C, BlockId::new(2).unwrap(), EDGE_BC);
        edge.fuel.push(FuelSettlement {
            site: PsiProvenance::Operation(OperationId::new(30).unwrap()),
            units: 1,
        });
        let row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        function.blocks[0].terminator = jump_terminator(
            instruction(JUMP_B, SelectedInstructionKind::Jump, &row, &[]),
            edge,
        );
    });
    assert_eq!(
        relocate(&fueled, &environment, &[MOVING], T_HEAD).unwrap_err(),
        ScheduledRelocationError::UnsupportedPair
    );
    // A register transport binding on the crossed edge the member writes.
    let transported = mutated(target, false, |function, environment| {
        let mut edge = successor(BLOCK_C, BlockId::new(2).unwrap(), EDGE_BC);
        edge.bindings.push(SelectedValueBinding {
            semantic: abstract_operations::ValueBinding {
                parameter: ValueId::new(20).unwrap(),
                argument: ValueId::new(1).unwrap(),
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                ),
            },
            transport: SelectedValueTransport::Registers {
                argument: R_MOVE,
                parameter: R_CHEAD,
            },
        });
        let row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        function.blocks[0].terminator = jump_terminator(
            instruction(JUMP_B, SelectedInstructionKind::Jump, &row, &[]),
            edge,
        );
    });
    assert_eq!(
        relocate(&transported, &environment, &[MOVING], T_HEAD).unwrap_err(),
        ScheduledRelocationError::UnsupportedPair
    );
}

/// A boundary settlement observing a changed executed prefix refuses; one
/// outside the window's span keeps its own executed set.
#[test]
fn boundary_settlements_bound_the_window() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A settlement inside the destination past the landing index observes
    // the member inside the prefix.
    let inside_target = mutated(target, false, |function, _environment| {
        function
            .boundary_settlements
            .push(settlement(BLOCK_T, 2, 42));
    });
    assert_eq!(
        relocate(&inside_target, &environment, &[MOVING], T_HEAD).unwrap_err(),
        ScheduledRelocationError::UnsupportedPair
    );
    // A settlement inside the run's block past the first member index saw
    // the run inside the source block's prefix.
    let inside_source = mutated(target, false, |function, _environment| {
        function
            .boundary_settlements
            .push(settlement(BLOCK_B, 3, 42));
    });
    assert_eq!(
        relocate(&inside_source, &environment, &[MOVING, SECOND], T_HEAD).unwrap_err(),
        ScheduledRelocationError::UnsupportedPair
    );
    // Settlements at the landing index and on uncrossed paths stay.
    let outside = mutated(target, false, |function, _environment| {
        function
            .boundary_settlements
            .push(settlement(BLOCK_T, 0, 42));
        function
            .boundary_settlements
            .push(settlement(BLOCK_B, 1, 43));
        function
            .boundary_settlements
            .push(settlement(BLOCK_J, 2, 44));
    });
    relocate(&outside, &environment, &[MOVING], T_HEAD).unwrap();
}

/// Every location the run writes must be dead on the skipped paths: a
/// read of `R_MOVE` inside F, or inside J reachable from F, refuses.
#[test]
fn member_writes_must_die_on_the_skipped_paths() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let f_reads = mutated(target, true, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[2].instructions[0] = instruction(
            F_HEAD,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[R_MOVE, R_FHEAD],
        );
    });
    assert_eq!(
        relocate(&f_reads, &environment, &[MOVING], T_HEAD).unwrap_err(),
        ScheduledRelocationError::UnsupportedPair
    );
    let j_reads = mutated(target, true, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[4].instructions[0] = instruction(
            HEAD,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[R_MOVE, R_HEAD],
        );
    });
    assert_eq!(
        relocate(&j_reads, &environment, &[MOVING], T_HEAD).unwrap_err(),
        ScheduledRelocationError::UnsupportedPair
    );
    // A rewrite inside F retires the member's write: the moved program is
    // admitted.
    let f_rewrites = mutated(target, true, |function, environment| {
        let materialize = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap()
            .clone();
        function.blocks[2].instructions.insert(
            0,
            instruction(
                F_INSERT,
                SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(3),
                },
                &materialize,
                &[R_MOVE],
            ),
        );
    });
    relocate(&f_rewrites, &environment, &[MOVING], T_HEAD).unwrap();
}

/// Replay rejects anything but the exact admitted move.
#[test]
fn replay_rejects_anything_but_the_move() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = chain_fixture(target);
    let result = relocate(&source, &environment, &[MOVING], T_HEAD).unwrap();
    // The right member at the wrong index.
    let mut wrong_index = result.transformed().clone();
    let moved = wrong_index.functions[0].blocks[2].instructions.remove(0);
    wrong_index.functions[0].blocks[2]
        .instructions
        .insert(2, moved);
    assert_eq!(
        validate_scheduled_relocation(
            &source,
            0,
            &[MOVING],
            T_HEAD,
            &environment,
            budget(),
            wrong_index,
        )
        .unwrap_err(),
        ScheduledRelocationError::ReplayMismatch
    );
    // The right index, a different member.
    let mut wrong_member = result.transformed().clone();
    wrong_member.functions[0].blocks[2].instructions[0] =
        result.transformed().functions[0].blocks[2].instructions[1].clone();
    assert_eq!(
        validate_scheduled_relocation(
            &source,
            0,
            &[MOVING],
            T_HEAD,
            &environment,
            budget(),
            wrong_member,
        )
        .unwrap_err(),
        ScheduledRelocationError::ReplayMismatch
    );
}

/// The measured validation-step boundary refuses an over-budget plan.
#[test]
fn measured_validation_step_boundary() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = chain_fixture(target);
    let admitted = relocate(&source, &environment, &[MOVING], T_HEAD).unwrap();
    // A budget that cannot pay for the derivation refuses.
    let starved = relocate_scheduled_run(
        &source,
        0,
        &[MOVING],
        T_HEAD,
        &environment,
        OptimizationWorkBudget::new(100, 100, 1, 100, 100).unwrap(),
    );
    assert_eq!(
        starved.unwrap_err(),
        ScheduledRelocationError::WorkBudgetExceeded
    );
    // The admitted proposal replays under the admitted budget.
    validate_scheduled_relocation(
        &source,
        0,
        &[MOVING],
        T_HEAD,
        &environment,
        budget(),
        admitted.transformed().clone(),
    )
    .unwrap();
}

/// The plan's target must match the register environment's.
#[test]
fn target_mismatch_rejects() {
    let environment = baseline_target_register_environment(NativeTarget::linux_x64()).unwrap();
    let source = chain_fixture(NativeTarget::linux_arm64());
    assert_eq!(
        relocate(&source, &environment, &[MOVING], T_HEAD).unwrap_err(),
        ScheduledRelocationError::SourceMismatch
    );
}

/// The same window admits the same move twice, and the moved program is
/// itself a valid source for a follow-up relocation.
#[test]
fn scheduled_relocation_is_deterministic_and_re_admitted() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = chain_fixture(target);
    let first = relocate(&source, &environment, &[MOVING], T_HEAD).unwrap();
    let second = relocate(&source, &environment, &[MOVING], T_HEAD).unwrap();
    assert_eq!(first.transformed(), second.transformed());
    // The transformed program is a source too: TRAIL can follow the run
    // into T.
    let chained = relocate(&first, &environment, &[TRAIL], T_TAIL).unwrap();
    assert_eq!(
        chained.transformed().functions[0].blocks[2]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![MOVING, T_HEAD, TRAIL, T_TAIL]
    );
}

/// The mirror image: the run hoists out of a downstream block onto a
/// dominating destination — `[T_HEAD; T_TAIL]` leave T's head, cross C's
/// tail and terminator and the C->T edge, and land inside C —
/// `C = [C_HEAD, T_HEAD, T_TAIL, C_TAIL]` — with every other position
/// keeping its order and the replayed proposal restoring the source
/// bit-identically.
#[test]
fn run_hoists_through_a_sole_successor_chain() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = chain_fixture(target);
        let result = relocate(&source, &environment, &[T_HEAD, T_TAIL], C_TAIL).unwrap();
        let original = &source.transformed().functions[0];
        let moved = &result.transformed().functions[0];
        assert_eq!(
            moved.blocks[1]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![C_HEAD, T_HEAD, T_TAIL, C_TAIL]
        );
        assert!(moved.blocks[2].instructions.is_empty());
        assert_eq!(
            moved.blocks[0].instructions,
            original.blocks[0].instructions
        );
        assert_eq!(
            moved.blocks[3].instructions,
            original.blocks[3].instructions
        );
        for index in 0..4 {
            assert_eq!(
                moved.blocks[index].terminator,
                original.blocks[index].terminator
            );
        }
        validate_scheduled_relocation(
            &source,
            0,
            &[T_HEAD, T_TAIL],
            C_TAIL,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
    }
}

/// A hoist out of a join crosses every arm path back to the dominating
/// fork head: `[HEAD]` leaves J, crosses A's, T's, and F's streams, both
/// fork edges, and B's branch, and lands on TRAIL's position in B —
/// `B = [LEAD, MOVING, HEAD, TRAIL]`.
#[test]
fn run_hoists_out_of_a_join() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fork_deep_fixture(target);
    let result = relocate(&source, &environment, &[HEAD], TRAIL).unwrap();
    let original = &source.transformed().functions[0];
    let moved = &result.transformed().functions[0];
    assert_eq!(
        moved.blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![LEAD, MOVING, HEAD, TRAIL]
    );
    assert!(moved.blocks[4].instructions.is_empty());
    assert_eq!(
        moved.blocks[1].instructions,
        original.blocks[1].instructions
    );
    assert_eq!(
        moved.blocks[2].instructions,
        original.blocks[2].instructions
    );
    assert_eq!(
        moved.blocks[3].instructions,
        original.blocks[3].instructions
    );
    validate_scheduled_relocation(
        &source,
        0,
        &[HEAD],
        TRAIL,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
}

/// A hoist naming the terminator-carried instruction lands at the body
/// end, and one naming a mid-body position lands there.
#[test]
fn hoist_lands_at_the_named_position() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = chain_fixture(target);
    let body_end = relocate(&source, &environment, &[C_HEAD], JUMP_B).unwrap();
    assert_eq!(
        body_end.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![LEAD, MOVING, SECOND, TRAIL, C_HEAD]
    );
    let mid = relocate(&source, &environment, &[C_HEAD], MOVING).unwrap();
    assert_eq!(
        mid.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![LEAD, C_HEAD, MOVING, SECOND, TRAIL]
    );
}

/// A hoist's destination must dominate the run's block: a landing inside
/// the fork's F arm is reachable from the A arm without it, so the run
/// would go missing on traversals through A.
#[test]
fn undominated_hoist_landings_refuse() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fork_deep_fixture(target);
    assert_eq!(
        relocate(&source, &environment, &[HEAD], F_TAIL).unwrap_err(),
        ScheduledRelocationError::UnsupportedPair
    );
    // J does not dominate F either: the request bounds no window.
    assert_eq!(
        relocate(&source, &environment, &[F_HEAD], J_HEAD).unwrap_err(),
        ScheduledRelocationError::UnsupportedPair
    );
}

/// A continuation off the destination that can exit without the run's
/// block gains a run the source never executed on that traversal.
#[test]
fn hoist_refuses_continuations_that_exit() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Point the fork's F arm at a return instead of the join.
    let exiting = mutated(target, true, |function, environment| {
        let parts = parts(target);
        function.blocks[2].terminator = SelectedTerminator::Return {
            instruction: instruction(
                F_JUMP,
                SelectedInstructionKind::ReturnUnit,
                &parts.return_row,
                &[],
            ),
            psi_return_edge: EdgeId::new(36).unwrap(),
        };
        let _ = environment;
    });
    assert_eq!(
        relocate(&exiting, &environment, &[HEAD], TRAIL).unwrap_err(),
        ScheduledRelocationError::UnsupportedPair
    );
}

/// A continuation off the destination that can cycle without the run's
/// block would run the relocated member again before the source position
/// arrived once.
#[test]
fn hoist_refuses_continuations_that_cycle() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Let T branch back to A, closing an A -> T -> A loop beside the join.
    let cycling = mutated(target, true, |function, environment| {
        let parts = parts(target);
        function.blocks[3].terminator = SelectedTerminator::ConditionalBranch {
            instruction: instruction(
                T_JUMP,
                SelectedInstructionKind::ConditionalBranchNonZero,
                &parts.branch_row,
                &[],
            ),
            when_nonzero: successor(BLOCK_J, BlockId::new(5).unwrap(), 34),
            when_zero: successor(BLOCK_A, BlockId::new(2).unwrap(), 37),
        };
        let _ = environment;
    });
    assert_eq!(
        relocate(&cycling, &environment, &[HEAD], TRAIL).unwrap_err(),
        ScheduledRelocationError::UnsupportedPair
    );
}

/// A run block re-enterable from its own successors without crossing the
/// destination again would execute the relocated member once where the
/// source ran it per entry.
#[test]
fn hoist_refuses_a_run_block_reenterable_beside_the_landing() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Give J a branch back into A: J -> A -> T -> J re-enters J while B's
    // landing ran the member once.
    let reentering = mutated(target, true, |function, environment| {
        let parts = parts(target);
        function.blocks[4].terminator = SelectedTerminator::ConditionalBranch {
            instruction: instruction(
                JUMP_B,
                SelectedInstructionKind::ConditionalBranchNonZero,
                &parts.branch_row,
                &[],
            ),
            when_nonzero: successor(BLOCK_A, BlockId::new(2).unwrap(), 38),
            when_zero: successor(BLOCK_A, BlockId::new(2).unwrap(), 39),
        };
        let _ = environment;
    });
    assert_eq!(
        relocate(&reentering, &environment, &[HEAD], TRAIL).unwrap_err(),
        ScheduledRelocationError::UnsupportedPair
    );
}

/// The hoist crosses positions behind the landing and ahead of the run's
/// old position: a write to the member's register on a crossed arm
/// refuses, while a read of it before the landing index stays uncrossed.
#[test]
fn hoist_crossed_hazards_keep_order() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // F_TAIL writing R_HEAD trades order with the hoisted member.
    let crossed_waw = mutated(target, true, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[2].instructions[1] = instruction(
            F_TAIL,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[R_HEAD, R_FHEAD],
        );
    });
    assert_eq!(
        relocate(&crossed_waw, &environment, &[HEAD], TRAIL).unwrap_err(),
        ScheduledRelocationError::UnsupportedPair
    );
    // A_TAIL reads R_HEAD inside a crossed intermediate: refused.
    let crossed_raw = mutated(target, true, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[1].instructions[1] = instruction(
            A_TAIL,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[R_ATAIL, R_HEAD],
        );
    });
    assert_eq!(
        relocate(&crossed_raw, &environment, &[HEAD], TRAIL).unwrap_err(),
        ScheduledRelocationError::UnsupportedPair
    );
    // LEAD reads R_HEAD but sits before the landing index: it keeps the
    // run on the side it always had, so the hoist stays admitted.
    let early_read = mutated(target, true, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[0] = instruction(
            LEAD,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[R_LEAD, R_HEAD],
        );
    });
    relocate(&early_read, &environment, &[HEAD], TRAIL).unwrap();
}

/// A settlement inside the run's old block past the vacated position, or
/// past the landing index in the destination, observes a changed executed
/// prefix; one at the landing boundary keeps its own executed set.
#[test]
fn hoist_boundary_settlements_bound_the_window() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let inside_run_block = mutated(target, true, |function, _| {
        function.boundary_settlements = vec![settlement(BLOCK_J, 1, 40)];
    });
    assert_eq!(
        relocate(&inside_run_block, &environment, &[HEAD], TRAIL).unwrap_err(),
        ScheduledRelocationError::UnsupportedPair
    );
    let past_landing = mutated(target, true, |function, _| {
        function.boundary_settlements = vec![settlement(BLOCK_B, 3, 40)];
    });
    assert_eq!(
        relocate(&past_landing, &environment, &[HEAD], TRAIL).unwrap_err(),
        ScheduledRelocationError::UnsupportedPair
    );
    let at_landing = mutated(target, true, |function, _| {
        function.boundary_settlements = vec![settlement(BLOCK_B, 2, 40)];
    });
    relocate(&at_landing, &environment, &[HEAD], TRAIL).unwrap();
}
