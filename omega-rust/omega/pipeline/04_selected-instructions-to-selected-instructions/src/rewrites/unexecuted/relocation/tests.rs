use optimization_core::OptimizationUnitIdentity;
use optimization_unit::ValueDefinitionSite;
use register_environment::{
    ValidatedTargetRegisterEnvironment, baseline_target_register_environment,
};
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedBoundarySettlement,
    SelectedBoundarySettlementPayload, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan, SelectedMemoryAccess,
    SelectedMemoryAccessOrigin, SelectedMemoryAccessRole, SelectedSuccessor, SelectedSuccessorRole,
    SelectedTerminator, SelectedValueBinding, SelectedValueTransport, VirtualRegister,
    VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType,
    IntegerValue, MachineId, OperationId, PlaceId, ScalarType, ValueId,
};
use target::NativeTarget;
use target_operations_to_selected_instructions::selected_instruction_plan_identity;
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use super::{
    MemberRunRelocationError, MemberRunRelocationReceipt, ValidatedMemberRunRelocation,
    relocate_selected_member_run, validate_member_run_relocation,
};
use crate::rewrites::test_support::{budget, instruction};

const LEAD: SelectedInstructionId = SelectedInstructionId(2);
const MOVING: SelectedInstructionId = SelectedInstructionId(3);
const TRAIL: SelectedInstructionId = SelectedInstructionId(4);
const HEAD: SelectedInstructionId = SelectedInstructionId(5);
const MID: SelectedInstructionId = SelectedInstructionId(6);
const TAIL: SelectedInstructionId = SelectedInstructionId(7);
const JUMP: SelectedInstructionId = SelectedInstructionId(8);
const RET: SelectedInstructionId = SelectedInstructionId(9);
const BRIDGE: SelectedInstructionId = SelectedInstructionId(10);
const BRIDGE_JUMP: SelectedInstructionId = SelectedInstructionId(11);
const SIDE: SelectedInstructionId = SelectedInstructionId(12);
const SIDE_JUMP: SelectedInstructionId = SelectedInstructionId(13);
const MOVING_SECOND: SelectedInstructionId = SelectedInstructionId(14);

const POINTER: VirtualRegisterId = VirtualRegisterId(0);
const R_LEAD: VirtualRegisterId = VirtualRegisterId(1);
const R_MOVE: VirtualRegisterId = VirtualRegisterId(2);
const R_TRAIL: VirtualRegisterId = VirtualRegisterId(3);
const R_HEAD: VirtualRegisterId = VirtualRegisterId(4);
const R_MID: VirtualRegisterId = VirtualRegisterId(5);
const R_TAIL: VirtualRegisterId = VirtualRegisterId(6);
const R_BOUND: VirtualRegisterId = VirtualRegisterId(7);
const R_BRIDGE: VirtualRegisterId = VirtualRegisterId(8);
const R_SIDE: VirtualRegisterId = VirtualRegisterId(9);
const R_MOVE_SECOND: VirtualRegisterId = VirtualRegisterId(10);

const BLOCK_A: SelectedBlockId = SelectedBlockId(0);
const BLOCK_B: SelectedBlockId = SelectedBlockId(1);
const BLOCK_C: SelectedBlockId = SelectedBlockId(2);
const BLOCK_D: SelectedBlockId = SelectedBlockId(3);
const EDGE_AB: u64 = 10;
const EDGE_BC: u64 = 11;
const EDGE_CD: u64 = 12;
const EDGE_AD: u64 = 13;
const EDGE_DB: u64 = 14;

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

fn return_terminator(instruction: SelectedInstruction) -> SelectedTerminator {
    SelectedTerminator::Return {
        instruction,
        psi_return_edge: EdgeId::new(40).unwrap(),
    }
}

fn materialization(
    id: SelectedInstructionId,
    register: VirtualRegisterId,
    value: u128,
    environment: &ValidatedTargetRegisterEnvironment,
) -> SelectedInstruction {
    let keys = environment.selected_keys();
    let materialize = environment.constraint(keys.materialize_i64).unwrap();
    instruction(
        id,
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(value),
        },
        materialize,
        &[register],
    )
}

fn result_register(
    id: VirtualRegisterId,
    instruction: SelectedInstructionId,
    value: u64,
    class: register_model::RegisterClassId,
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

fn block(
    id: SelectedBlockId,
    source: u64,
    instructions: Vec<SelectedInstruction>,
    terminator: SelectedTerminator,
) -> SelectedBlock {
    SelectedBlock {
        id,
        origin: SelectedBlockOrigin::Source(BlockId::new(source).unwrap()),
        instructions,
        terminator,
    }
}

/// A raw selected-stage unit fixture, not a source/Terminal admission claim.
/// The base shape mirrors the edge family's fixture: block A is the entry and
/// ends in a semantic `Jump` to block B, whose only predecessor is that edge
/// and whose terminator is a plain return:
/// `A = [LEAD; MOVING; MOVING_SECOND; TRAIL] -> jump -> B = [HEAD; MID; TAIL]`
/// — the second member lets run moves exercise the contiguous-span rule.
fn fixture(target: NativeTarget) -> ValidatedMemberRunRelocation {
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let materialize = environment.constraint(keys.materialize_i64).unwrap();
    let jump_row = environment.constraint(keys.jump).unwrap();
    let return_row = environment.constraint(keys.return_unit).unwrap();
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
        result_register(R_LEAD, LEAD, 2, class),
        result_register(R_MOVE, MOVING, 3, class),
        result_register(R_TRAIL, TRAIL, 4, class),
        result_register(R_HEAD, HEAD, 5, class),
        result_register(R_MID, MID, 6, class),
        result_register(R_TAIL, TAIL, 7, class),
        result_register(R_BOUND, LEAD, 8, class),
        result_register(R_MOVE_SECOND, MOVING_SECOND, 41, class),
        result_register(R_BRIDGE, BRIDGE, 61, class),
        result_register(R_SIDE, SIDE, 62, class),
    ];
    let materialization =
        |id, register: VirtualRegisterId, value| materialization(id, register, value, &environment);
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
                block(
                    BLOCK_A,
                    1,
                    vec![
                        materialization(LEAD, R_LEAD, 5),
                        materialization(MOVING, R_MOVE, 7),
                        materialization(MOVING_SECOND, R_MOVE_SECOND, 8),
                        materialization(TRAIL, R_TRAIL, 9),
                    ],
                    jump_terminator(
                        instruction(JUMP, SelectedInstructionKind::Jump, jump_row, &[]),
                        successor(BLOCK_B, BlockId::new(2).unwrap(), EDGE_AB),
                    ),
                ),
                block(
                    BLOCK_B,
                    2,
                    vec![
                        materialization(HEAD, R_HEAD, 11),
                        materialization(MID, R_MID, 13),
                        materialization(TAIL, R_TAIL, 15),
                    ],
                    return_terminator(instruction(
                        RET,
                        SelectedInstructionKind::ReturnUnit,
                        return_row,
                        &[],
                    )),
                ),
            ],
        }]
        .into(),
    };
    let identity = selected_instruction_plan_identity(&plan);
    ValidatedMemberRunRelocation {
        receipt: MemberRunRelocationReceipt {
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
    edit: impl FnOnce(&mut SelectedFunction, &ValidatedTargetRegisterEnvironment),
) -> ValidatedMemberRunRelocation {
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
    source: &ValidatedMemberRunRelocation,
    environment: &ValidatedTargetRegisterEnvironment,
    first_member: SelectedInstructionId,
    last_member: SelectedInstructionId,
    destination: SelectedInstructionId,
) -> Result<ValidatedMemberRunRelocation, MemberRunRelocationError> {
    relocate_selected_member_run(
        source,
        0,
        first_member,
        last_member,
        destination,
        environment,
        budget(),
    )
}

fn relocate_member(
    source: &ValidatedMemberRunRelocation,
    environment: &ValidatedTargetRegisterEnvironment,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
) -> Result<ValidatedMemberRunRelocation, MemberRunRelocationError> {
    relocate(source, environment, member, member, destination)
}

fn ids(block: &SelectedBlock) -> Vec<SelectedInstructionId> {
    block
        .instructions
        .iter()
        .map(|instruction| instruction.id)
        .collect()
}

/// A member crosses its block's `Jump` edge onto the destination's position —
/// the edge family's shape through the shared admission: `MOVING` leaves A's
/// body and lands at B's head with the destination and every later position
/// one run-width later in their original order, identity, kind, operands, and
/// provenance intact. The replayed proposal restores the source bit-identical.
#[test]
fn member_relocates_across_the_jump_edge() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = fixture(target);
        let result = relocate_member(&source, &environment, MOVING, HEAD).unwrap();
        let original = &source.transformed().functions[0];
        let moved = &result.transformed().functions[0];
        assert_eq!(ids(&moved.blocks[0]), vec![LEAD, MOVING_SECOND, TRAIL]);
        assert_eq!(ids(&moved.blocks[1]), vec![MOVING, HEAD, MID, TAIL]);
        // The member moved bit-identically; both terminators and every
        // roster stayed untouched.
        assert_eq!(
            moved.blocks[1].instructions[0],
            original.blocks[0].instructions[1]
        );
        assert_eq!(moved.blocks[0].terminator, original.blocks[0].terminator);
        assert_eq!(moved.blocks[1].terminator, original.blocks[1].terminator);
        assert_eq!(moved.memory_accesses, original.memory_accesses);
        assert_eq!(moved.calls, original.calls);
        assert_eq!(moved.boundary_settlements, original.boundary_settlements);
        validate_member_run_relocation(
            &source,
            0,
            MOVING,
            MOVING,
            HEAD,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
    }
}

/// The contiguous run the two named members bound leaves its block as one
/// body: `MOVING..MOVING_SECOND` relocate onto `HEAD`'s position in member
/// order.
#[test]
fn contiguous_run_relocates_as_one_body() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let moved = relocate(&source, &environment, MOVING, MOVING_SECOND, HEAD).unwrap();
    assert_eq!(
        ids(&moved.transformed().functions[0].blocks[0]),
        vec![LEAD, TRAIL]
    );
    assert_eq!(
        ids(&moved.transformed().functions[0].blocks[1]),
        vec![MOVING, MOVING_SECOND, HEAD, MID, TAIL]
    );
    validate_member_run_relocation(
        &source,
        0,
        MOVING,
        MOVING_SECOND,
        HEAD,
        &environment,
        budget(),
        moved.transformed().clone(),
    )
    .unwrap();
}

/// Naming the destination instruction chooses the landing position: a body
/// instruction puts the run on its index and the terminator-carried
/// instruction lands the run at the body end.
#[test]
fn the_named_destination_chooses_the_landing_position() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let middle = relocate_member(&source, &environment, MOVING, MID).unwrap();
    assert_eq!(
        ids(&middle.transformed().functions[0].blocks[1]),
        vec![HEAD, MOVING, MID, TAIL]
    );
    let body_end = relocate_member(&source, &environment, MOVING, RET).unwrap();
    assert_eq!(
        ids(&body_end.transformed().functions[0].blocks[1]),
        vec![HEAD, MID, TAIL, MOVING]
    );
}

/// A destination inside the run's own block is the in-block move: backward
/// places the member on the named position, forward places the run's last
/// member on it — every crossed body position checked once in between.
#[test]
fn member_relocates_within_its_own_block() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let backward = relocate_member(&source, &environment, TRAIL, LEAD).unwrap();
    assert_eq!(
        ids(&backward.transformed().functions[0].blocks[0]),
        vec![TRAIL, LEAD, MOVING, MOVING_SECOND]
    );
    validate_member_run_relocation(
        &source,
        0,
        TRAIL,
        TRAIL,
        LEAD,
        &environment,
        budget(),
        backward.transformed().clone(),
    )
    .unwrap();
    let forward = relocate_member(&source, &environment, LEAD, TRAIL).unwrap();
    assert_eq!(
        ids(&forward.transformed().functions[0].blocks[0]),
        vec![MOVING, MOVING_SECOND, TRAIL, LEAD]
    );
    validate_member_run_relocation(
        &source,
        0,
        LEAD,
        LEAD,
        TRAIL,
        &environment,
        budget(),
        forward.transformed().clone(),
    )
    .unwrap();
    let forward_run = relocate(&source, &environment, MOVING, MOVING_SECOND, TRAIL).unwrap();
    assert_eq!(
        ids(&forward_run.transformed().functions[0].blocks[0]),
        vec![LEAD, TRAIL, MOVING, MOVING_SECOND]
    );
}

/// The admission the per-shape families do not reach: a member relocates
/// through an intermediate block — `MOVING` leaves A and lands in B across
/// the `A -> C -> B` chain, crossing A's tail, both edges, and C's whole
/// body. Every traversal still runs the member exactly once: C's only exit
/// reaches B and B's only predecessor lies on a crossed path.
#[test]
fn member_relocates_across_a_chain_window() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let keys = environment.selected_keys();
        let jump_row = environment.constraint(keys.jump).unwrap();
        let materialize = environment.constraint(keys.materialize_i64).unwrap();
        // Splice a bridge block between A and B: A -> C -> B.
        if let SelectedTerminator::Jump {
            successor: edge, ..
        } = &mut function.blocks[0].terminator
        {
            *edge = successor(BLOCK_C, BlockId::new(3).unwrap(), EDGE_AB);
        }
        function.blocks.insert(
            1,
            block(
                BLOCK_C,
                3,
                vec![instruction(
                    BRIDGE,
                    SelectedInstructionKind::MaterializeI64 {
                        value: IntegerValue::Unsigned(17),
                    },
                    materialize,
                    &[R_BRIDGE],
                )],
                jump_terminator(
                    instruction(BRIDGE_JUMP, SelectedInstructionKind::Jump, jump_row, &[]),
                    successor(BLOCK_B, BlockId::new(2).unwrap(), EDGE_CD),
                ),
            ),
        );
    });
    let moved = relocate_member(&source, &environment, MOVING, HEAD).unwrap();
    let function = &moved.transformed().functions[0];
    assert_eq!(ids(&function.blocks[0]), vec![LEAD, MOVING_SECOND, TRAIL]);
    assert_eq!(ids(&function.blocks[1]), vec![BRIDGE]);
    assert_eq!(ids(&function.blocks[2]), vec![MOVING, HEAD, MID, TAIL]);
    validate_member_run_relocation(
        &source,
        0,
        MOVING,
        MOVING,
        HEAD,
        &environment,
        budget(),
        moved.transformed().clone(),
    )
    .unwrap();
}

/// A member above a diamond lands beneath it: with A branching to C and to D
/// and both arms jumping to B, every predecessor edge into B lies on a
/// crossed path and every exit of every crossed block reaches B — the shape
/// the dedicated diamond family enumerates, through the shared admission.
#[test]
fn member_relocates_through_a_diamond_window() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let keys = environment.selected_keys();
        let branch_row = environment.constraint(keys.conditional_branch).unwrap();
        let jump_row = environment.constraint(keys.jump).unwrap();
        let materialize = environment.constraint(keys.materialize_i64).unwrap();
        let arm = |block_id, member_id, register, jump_id, edge, source_target| {
            block(
                block_id,
                source_target,
                vec![instruction(
                    member_id,
                    SelectedInstructionKind::MaterializeI64 {
                        value: IntegerValue::Unsigned(31),
                    },
                    materialize,
                    &[register],
                )],
                jump_terminator(
                    instruction(jump_id, SelectedInstructionKind::Jump, jump_row, &[]),
                    successor(BLOCK_B, BlockId::new(2).unwrap(), edge),
                ),
            )
        };
        function
            .blocks
            .insert(1, arm(BLOCK_C, BRIDGE, R_BRIDGE, BRIDGE_JUMP, EDGE_BC, 3));
        function
            .blocks
            .insert(2, arm(BLOCK_D, SIDE, R_SIDE, SIDE_JUMP, EDGE_AD, 4));
        function.blocks[0].terminator = SelectedTerminator::ConditionalBranch {
            instruction: instruction(
                JUMP,
                SelectedInstructionKind::ConditionalBranchNonZero,
                branch_row,
                &[],
            ),
            when_nonzero: successor(BLOCK_C, BlockId::new(3).unwrap(), EDGE_AB),
            when_zero: successor(BLOCK_D, BlockId::new(4).unwrap(), EDGE_CD),
        };
    });
    let moved = relocate_member(&source, &environment, MOVING, HEAD).unwrap();
    let function = &moved.transformed().functions[0];
    assert_eq!(ids(&function.blocks[0]), vec![LEAD, MOVING_SECOND, TRAIL]);
    assert_eq!(ids(&function.blocks[1]), vec![BRIDGE]);
    assert_eq!(ids(&function.blocks[2]), vec![SIDE]);
    assert_eq!(ids(&function.blocks[3]), vec![MOVING, HEAD, MID, TAIL]);
    validate_member_run_relocation(
        &source,
        0,
        MOVING,
        MOVING,
        HEAD,
        &environment,
        budget(),
        moved.transformed().clone(),
    )
    .unwrap();
    let run = relocate(&source, &environment, MOVING, MOVING_SECOND, HEAD).unwrap();
    let function = &run.transformed().functions[0];
    assert_eq!(ids(&function.blocks[0]), vec![LEAD, TRAIL]);
    assert_eq!(
        ids(&function.blocks[3]),
        vec![MOVING, MOVING_SECOND, HEAD, MID, TAIL]
    );
    validate_member_run_relocation(
        &source,
        0,
        MOVING,
        MOVING_SECOND,
        HEAD,
        &environment,
        budget(),
        run.transformed().clone(),
    )
    .unwrap();
}

/// The upstream move is the same admission with the paths reversed: the
/// member leaves B and takes a position in A, the block whose sole edge
/// reaches it. Every traversal that reaches B comes through A, and A's only
/// exit is B, so the run executes exactly as often as before. This is what
/// the per-shape upstream families prove by hand.
#[test]
fn member_relocates_upstream_across_the_jump_edge() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let moved = relocate_member(&source, &environment, HEAD, MOVING).unwrap();
    let function = &moved.transformed().functions[0];
    assert_eq!(
        ids(&function.blocks[0]),
        vec![LEAD, HEAD, MOVING, MOVING_SECOND, TRAIL]
    );
    assert_eq!(ids(&function.blocks[1]), vec![MID, TAIL]);
    validate_member_run_relocation(
        &source,
        0,
        HEAD,
        HEAD,
        MOVING,
        &environment,
        budget(),
        moved.transformed().clone(),
    )
    .unwrap();
    let run = relocate(&source, &environment, HEAD, MID, MOVING).unwrap();
    let function = &run.transformed().functions[0];
    assert_eq!(
        ids(&function.blocks[0]),
        vec![LEAD, HEAD, MID, MOVING, MOVING_SECOND, TRAIL]
    );
    assert_eq!(ids(&function.blocks[1]), vec![TAIL]);
    validate_member_run_relocation(
        &source,
        0,
        HEAD,
        MID,
        MOVING,
        &environment,
        budget(),
        run.transformed().clone(),
    )
    .unwrap();
}

/// Upstream out of a converging join, through the complete diamond that
/// feeds it: the member leaves B and lands in the branching head A, whose
/// arms C and D alone reach B. Both edges into B are crossed and every exit
/// of A, C and D is crossed, so no traversal is gained or lost.
#[test]
fn member_relocates_upstream_out_of_a_join() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let keys = environment.selected_keys();
        let branch_row = environment.constraint(keys.conditional_branch).unwrap();
        let jump_row = environment.constraint(keys.jump).unwrap();
        let materialize = environment.constraint(keys.materialize_i64).unwrap();
        let arm = |block_id, member_id, register, jump_id, edge, source_target| {
            block(
                block_id,
                source_target,
                vec![instruction(
                    member_id,
                    SelectedInstructionKind::MaterializeI64 {
                        value: IntegerValue::Unsigned(31),
                    },
                    materialize,
                    &[register],
                )],
                jump_terminator(
                    instruction(jump_id, SelectedInstructionKind::Jump, jump_row, &[]),
                    successor(BLOCK_B, BlockId::new(2).unwrap(), edge),
                ),
            )
        };
        function
            .blocks
            .insert(1, arm(BLOCK_C, BRIDGE, R_BRIDGE, BRIDGE_JUMP, EDGE_BC, 3));
        function
            .blocks
            .insert(2, arm(BLOCK_D, SIDE, R_SIDE, SIDE_JUMP, EDGE_AD, 4));
        function.blocks[0].terminator = SelectedTerminator::ConditionalBranch {
            instruction: instruction(
                JUMP,
                SelectedInstructionKind::ConditionalBranchNonZero,
                branch_row,
                &[],
            ),
            when_nonzero: successor(BLOCK_C, BlockId::new(3).unwrap(), EDGE_AB),
            when_zero: successor(BLOCK_D, BlockId::new(4).unwrap(), EDGE_CD),
        };
    });
    let moved = relocate_member(&source, &environment, HEAD, MOVING).unwrap();
    let function = &moved.transformed().functions[0];
    assert_eq!(
        ids(&function.blocks[0]),
        vec![LEAD, HEAD, MOVING, MOVING_SECOND, TRAIL]
    );
    assert_eq!(ids(&function.blocks[3]), vec![MID, TAIL]);
    validate_member_run_relocation(
        &source,
        0,
        HEAD,
        HEAD,
        MOVING,
        &environment,
        budget(),
        moved.transformed().clone(),
    )
    .unwrap();
}

/// A predecessor of the run's own block that no crossed path covers is a
/// traversal the upstream move would lose: with a second jump edge into B
/// from a detached side block, the member no longer executes on it.
#[test]
fn an_uncrossed_predecessor_of_the_run_block_refuses_upstream() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        function.blocks.push(block(
            BLOCK_D,
            4,
            Vec::new(),
            jump_terminator(
                instruction(SIDE_JUMP, SelectedInstructionKind::Jump, jump_row, &[]),
                successor(BLOCK_B, BlockId::new(2).unwrap(), EDGE_DB),
            ),
        ));
    });
    assert_eq!(
        relocate_member(&source, &environment, HEAD, MOVING),
        Err(MemberRunRelocationError::UnsupportedPair)
    );
}

/// Two blocks on a common cycle reach each other in both directions. A move
/// either way changes how often the run executes, and neither traversal
/// audit is the sound one, so the admission refuses rather than choosing.
#[test]
fn blocks_on_a_cycle_refuse_in_both_directions() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        // B jumps back to A, so A reaches B and B reaches A.
        function.blocks[1].terminator = jump_terminator(
            instruction(RET, SelectedInstructionKind::Jump, jump_row, &[]),
            successor(BLOCK_A, BlockId::new(1).unwrap(), EDGE_DB),
        );
    });
    assert_eq!(
        relocate_member(&source, &environment, MOVING, HEAD),
        Err(MemberRunRelocationError::UnsupportedPair)
    );
    assert_eq!(
        relocate_member(&source, &environment, HEAD, MOVING),
        Err(MemberRunRelocationError::UnsupportedPair)
    );
}

/// The bypassed triangle — a branch head whose one arm reaches the join and
/// whose other edge reaches it directly — is a window the shared derivation
/// covers without a shape of its own: A branches to C and to B, C jumps to B,
/// and the member lands in B. Both edges into B lie on a crossed path, so no
/// traversal is gained, and C's only exit reaches B, so none is lost.
#[test]
fn member_relocates_through_a_bypassed_triangle_window() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let keys = environment.selected_keys();
        let branch_row = environment.constraint(keys.conditional_branch).unwrap();
        let jump_row = environment.constraint(keys.jump).unwrap();
        let materialize = environment.constraint(keys.materialize_i64).unwrap();
        function.blocks.insert(
            1,
            block(
                BLOCK_C,
                3,
                vec![instruction(
                    BRIDGE,
                    SelectedInstructionKind::MaterializeI64 {
                        value: IntegerValue::Unsigned(31),
                    },
                    materialize,
                    &[R_BRIDGE],
                )],
                jump_terminator(
                    instruction(BRIDGE_JUMP, SelectedInstructionKind::Jump, jump_row, &[]),
                    successor(BLOCK_B, BlockId::new(2).unwrap(), EDGE_BC),
                ),
            ),
        );
        function.blocks[0].terminator = SelectedTerminator::ConditionalBranch {
            instruction: instruction(
                JUMP,
                SelectedInstructionKind::ConditionalBranchNonZero,
                branch_row,
                &[],
            ),
            when_nonzero: successor(BLOCK_C, BlockId::new(3).unwrap(), EDGE_AB),
            when_zero: successor(BLOCK_B, BlockId::new(2).unwrap(), EDGE_AD),
        };
    });
    let moved = relocate_member(&source, &environment, MOVING, HEAD).unwrap();
    let function = &moved.transformed().functions[0];
    assert_eq!(ids(&function.blocks[0]), vec![LEAD, MOVING_SECOND, TRAIL]);
    assert_eq!(ids(&function.blocks[1]), vec![BRIDGE]);
    assert_eq!(ids(&function.blocks[2]), vec![MOVING, HEAD, MID, TAIL]);
    validate_member_run_relocation(
        &source,
        0,
        MOVING,
        MOVING,
        HEAD,
        &environment,
        budget(),
        moved.transformed().clone(),
    )
    .unwrap();
    let run = relocate(&source, &environment, MOVING, MOVING_SECOND, HEAD).unwrap();
    let function = &run.transformed().functions[0];
    assert_eq!(ids(&function.blocks[0]), vec![LEAD, TRAIL]);
    assert_eq!(
        ids(&function.blocks[2]),
        vec![MOVING, MOVING_SECOND, HEAD, MID, TAIL]
    );
    validate_member_run_relocation(
        &source,
        0,
        MOVING,
        MOVING_SECOND,
        HEAD,
        &environment,
        budget(),
        run.transformed().clone(),
    )
    .unwrap();
    // The same triangle upstream, which is what the per-shape triangle family
    // proves: the member leaves the join and lands in the branching head.
    let upstream = relocate_member(&source, &environment, HEAD, MOVING).unwrap();
    let function = &upstream.transformed().functions[0];
    assert_eq!(
        ids(&function.blocks[0]),
        vec![LEAD, HEAD, MOVING, MOVING_SECOND, TRAIL]
    );
    assert_eq!(ids(&function.blocks[2]), vec![MID, TAIL]);
    validate_member_run_relocation(
        &source,
        0,
        HEAD,
        HEAD,
        MOVING,
        &environment,
        budget(),
        upstream.transformed().clone(),
    )
    .unwrap();
}

/// A destination predecessor that no crossed path covers is a traversal the
/// run would gain: with a second jump edge into B from a detached side
/// block, the move refuses.
#[test]
fn a_second_destination_predecessor_refuses() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        function.blocks.push(block(
            BLOCK_D,
            4,
            Vec::new(),
            jump_terminator(
                instruction(SIDE_JUMP, SelectedInstructionKind::Jump, jump_row, &[]),
                successor(BLOCK_B, BlockId::new(2).unwrap(), EDGE_DB),
            ),
        ));
    });
    assert_eq!(
        relocate_member(&source, &environment, MOVING, HEAD),
        Err(MemberRunRelocationError::UnsupportedPair)
    );
}

/// An exit of a crossed block that never reaches the destination is a
/// traversal the run would lose: with A branching to B and to a side block
/// that returns without rejoining, the move refuses.
#[test]
fn an_uncrossed_exit_of_the_run_block_refuses() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let keys = environment.selected_keys();
        let branch_row = environment.constraint(keys.conditional_branch).unwrap();
        let return_row = environment.constraint(keys.return_unit).unwrap();
        function.blocks.push(block(
            BLOCK_D,
            4,
            vec![materialization(SIDE, R_SIDE, 23, environment)],
            return_terminator(instruction(
                RET,
                SelectedInstructionKind::ReturnUnit,
                return_row,
                &[],
            )),
        ));
        function.blocks[0].terminator = SelectedTerminator::ConditionalBranch {
            instruction: instruction(
                JUMP,
                SelectedInstructionKind::ConditionalBranchNonZero,
                branch_row,
                &[],
            ),
            when_nonzero: successor(BLOCK_B, BlockId::new(2).unwrap(), EDGE_AB),
            when_zero: successor(BLOCK_D, BlockId::new(4).unwrap(), EDGE_AD),
        };
    });
    assert_eq!(
        relocate_member(&source, &environment, MOVING, HEAD),
        Err(MemberRunRelocationError::UnsupportedPair)
    );
}

/// Nothing arrives at a detached block, so the arrival audit would prove
/// nothing there: a B member naming a position in a block with no
/// predecessor edge refuses in both directions.
#[test]
fn an_arrival_block_with_no_predecessor_refuses() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        // D reaches B but nothing reaches D, so the upstream walk from D
        // finds the run's block while D itself is never arrived at.
        function.blocks.push(block(
            BLOCK_D,
            4,
            vec![instruction(
                SIDE,
                SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(31),
                },
                environment
                    .constraint(environment.selected_keys().materialize_i64)
                    .unwrap(),
                &[R_SIDE],
            )],
            jump_terminator(
                instruction(SIDE_JUMP, SelectedInstructionKind::Jump, jump_row, &[]),
                successor(BLOCK_B, BlockId::new(2).unwrap(), EDGE_DB),
            ),
        ));
    });
    assert_eq!(
        relocate_member(&source, &environment, HEAD, SIDE),
        Err(MemberRunRelocationError::UnsupportedPair)
    );
}

/// A destination that no acyclic path from the run's block reaches refuses:
/// the detached block D has no edge into it, so a B member naming one of its
/// instructions cannot relocate.
#[test]
fn an_unreachable_destination_refuses() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let return_row = environment
            .constraint(environment.selected_keys().return_unit)
            .unwrap();
        function.blocks.push(block(
            BLOCK_D,
            4,
            vec![materialization(SIDE, R_SIDE, 23, environment)],
            return_terminator(instruction(
                RET,
                SelectedInstructionKind::ReturnUnit,
                return_row,
                &[],
            )),
        ));
    });
    assert_eq!(
        relocate_member(&source, &environment, HEAD, SIDE),
        Err(MemberRunRelocationError::UnsupportedPair)
    );
}

/// A run whose named last member does not close the contiguous span after
/// the first names no run at all.
#[test]
fn a_reversed_or_foreign_last_member_refuses() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    assert_eq!(
        relocate(&source, &environment, MOVING_SECOND, MOVING, HEAD),
        Err(MemberRunRelocationError::UnsupportedPair)
    );
    assert_eq!(
        relocate(&source, &environment, MOVING, HEAD, HEAD),
        Err(MemberRunRelocationError::UnsupportedPair)
    );
    assert_eq!(
        relocate_member(&source, &environment, MOVING, SelectedInstructionId(999)),
        Err(MemberRunRelocationError::UnsupportedPair)
    );
    assert_eq!(
        relocate_member(&source, &environment, SelectedInstructionId(999), HEAD),
        Err(MemberRunRelocationError::SourceMismatch)
    );
}

/// Landing inside the run's own span is a degenerate move.
#[test]
fn landing_inside_the_run_refuses() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    assert_eq!(
        relocate(&source, &environment, MOVING, MOVING_SECOND, MOVING_SECOND),
        Err(MemberRunRelocationError::UnsupportedPair)
    );
}

/// A barrier-kind member can never trade order with anything.
#[test]
fn a_barrier_member_refuses() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        function.blocks[0].instructions[1] =
            instruction(MOVING, SelectedInstructionKind::Jump, jump_row, &[]);
    });
    assert_eq!(
        relocate_member(&source, &environment, MOVING, HEAD),
        Err(MemberRunRelocationError::UnsupportedInstruction)
    );
}

/// A member coupled with a crossed position — writing a register a crossed
/// instruction reads — refuses.
#[test]
fn a_member_coupled_with_a_crossed_position_refuses() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        // MOVING and TRAIL now share R_MOVE: MOVING defines it, TRAIL reads
        // it — relocating MOVING past TRAIL would change TRAIL's operand.
        let materialize = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            MOVING_SECOND,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(8),
            },
            materialize,
            &[R_MOVE],
        );
        function.blocks[0].instructions[3] = instruction(
            TRAIL,
            SelectedInstructionKind::CopyI64,
            environment
                .constraint(environment.selected_keys().copy_i64)
                .unwrap(),
            &[R_MOVE, R_TRAIL],
        );
    });
    assert_eq!(
        relocate_member(&source, &environment, MOVING, HEAD),
        Err(MemberRunRelocationError::UnsupportedPair)
    );
}

/// A roster-carrying run sharing its window with a second accounted memory
/// actor refuses: MOVING's access row plus an edge-origin row on the crossed
/// Jump edge reorder two recorded accesses.
#[test]
fn a_second_memory_actor_in_the_window_refuses() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        function.memory_accesses.push(access(
            MOVING,
            PlaceId::new(3).unwrap(),
            SelectedMemoryAccessRole::WritePlace,
        ));
        function.memory_accesses.push(edge_access(JUMP, EDGE_AB));
    });
    assert_eq!(
        relocate_member(&source, &environment, MOVING, HEAD),
        Err(MemberRunRelocationError::UnsupportedPair)
    );
}

/// A register transport on the crossed edge writing the parameter a member
/// would hand a stale value or observe out of order refuses.
#[test]
fn an_edge_transport_conflicting_with_a_member_refuses() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        if let SelectedTerminator::Jump { successor, .. } = &mut function.blocks[0].terminator {
            successor.bindings.push(SelectedValueBinding {
                semantic: abstract_operations::ValueBinding {
                    parameter: ValueId::new(50).unwrap(),
                    argument: ValueId::new(51).unwrap(),
                    scalar_type: ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                    ),
                },
                transport: SelectedValueTransport::Registers {
                    argument: R_LEAD,
                    parameter: R_MOVE,
                },
            });
        }
    });
    assert_eq!(
        relocate_member(&source, &environment, MOVING, HEAD),
        Err(MemberRunRelocationError::UnsupportedPair)
    );
}

/// A settlement positioned past the run's first index observed a member
/// inside the source block's executed prefix; past the landing index it
/// observes the run inside the destination's. Both refuse.
#[test]
fn settlements_at_the_window_boundary_refuse() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let in_run = mutated(target, |function, _| {
        function
            .boundary_settlements
            .push(settlement(BLOCK_A, 2, 70));
    });
    assert_eq!(
        relocate_member(&in_run, &environment, MOVING, HEAD),
        Err(MemberRunRelocationError::UnsupportedPair)
    );
    let in_destination = mutated(target, |function, _| {
        function
            .boundary_settlements
            .push(settlement(BLOCK_B, 1, 71));
    });
    assert_eq!(
        relocate_member(&in_destination, &environment, MOVING, HEAD),
        Err(MemberRunRelocationError::UnsupportedPair)
    );
}

/// A proposal whose run does not sit at the landing position — or that
/// perturbs anything outside the window — is a replay mismatch, not a
/// relocation.
#[test]
fn a_proposal_outside_the_window_refuses() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let moved = relocate_member(&source, &environment, MOVING, HEAD).unwrap();
    let mut wrong = moved.transformed().clone();
    wrong.functions[0].blocks[1].instructions.swap(0, 1);
    assert_eq!(
        validate_member_run_relocation(
            &source,
            0,
            MOVING,
            MOVING,
            HEAD,
            &environment,
            budget(),
            wrong,
        ),
        Err(MemberRunRelocationError::ReplayMismatch)
    );
}

/// The validator proves its legality reconstruction is its own: a forged
/// proposal — the same edit a producer would publish — is produced
/// directly on the source's plan without consulting admission, so the
/// validator's verdict cannot ride on the producer's admission record. A
/// legal forged move validates; a forged move through an illegal window
/// rejects with the legality error, not a replay mismatch.
mod independence_tests {
    use super::{
        BLOCK_B, BLOCK_D, BlockId, EDGE_DB, HEAD, IntegerValue, LEAD, MOVING, MOVING_SECOND,
        MemberRunRelocationError, NativeTarget, R_MOVE, R_TRAIL, SIDE_JUMP, SelectedInstructionId,
        SelectedInstructionKind, SelectedInstructionPlan, TRAIL, ValidatedMemberRunRelocation,
        baseline_target_register_environment, block, budget, fixture, instruction, jump_terminator,
        mutated, successor, validate_member_run_relocation,
    };

    /// Move the contiguous run `first`..`last` out of its block's body and
    /// splice it at `landing_index` in `landing_block`'s body — the edit a
    /// producer emitting that relocation would publish — without asking
    /// admission whether the window is legal.
    fn forged(
        source: &ValidatedMemberRunRelocation,
        first: SelectedInstructionId,
        last: SelectedInstructionId,
        landing_block: usize,
        landing_index: usize,
    ) -> SelectedInstructionPlan {
        let mut proposed = source.transformed().clone();
        let function = &mut proposed.functions[0];
        let (run_block, run_start, run_end) = function
            .blocks
            .iter()
            .enumerate()
            .find_map(|(block_index, block)| {
                let run_start = block
                    .instructions
                    .iter()
                    .position(|instruction| instruction.id == first)?;
                let run_end = block
                    .instructions
                    .iter()
                    .position(|instruction| instruction.id == last)?;
                Some((block_index, run_start, run_end))
            })
            .unwrap();
        let run: Vec<_> = function.blocks[run_block]
            .instructions
            .drain(run_start..=run_end)
            .collect();
        function.blocks[landing_block]
            .instructions
            .splice(landing_index..landing_index, run);
        proposed
    }

    /// A forged relocation of a window the validator's own audit admits
    /// validates: the member and the crossed positions carry no hazards,
    /// no roster rows, and no barriers, so the audit derives the move and
    /// the content comparison accepts it.
    #[test]
    fn forged_member_move_on_a_legal_window_validates() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let source = fixture(target);
        validate_member_run_relocation(
            &source,
            0,
            MOVING,
            MOVING,
            HEAD,
            &environment,
            budget(),
            forged(&source, MOVING, MOVING, 1, 0),
        )
        .unwrap();
    }

    /// The same holds for the multi-member run: `MOVING..MOVING_SECOND`
    /// spliced at `HEAD`'s index in block B is the legal cross-edge move
    /// the contract admits, and the validator's own audit agrees.
    #[test]
    fn forged_run_move_on_a_legal_window_validates() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let source = fixture(target);
        validate_member_run_relocation(
            &source,
            0,
            MOVING,
            MOVING_SECOND,
            HEAD,
            &environment,
            budget(),
            forged(&source, MOVING, MOVING_SECOND, 1, 0),
        )
        .unwrap();
    }

    /// The in-block move is the same contract one block earlier: `LEAD`
    /// forwarded onto `TRAIL`'s index lands at the body end, the trailing
    /// edge the derived window admits.
    #[test]
    fn forged_member_move_inside_its_own_block_validates() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let source = fixture(target);
        validate_member_run_relocation(
            &source,
            0,
            LEAD,
            LEAD,
            TRAIL,
            &environment,
            budget(),
            forged(&source, LEAD, LEAD, 0, 3),
        )
        .unwrap();
    }

    /// A producer that admitted a hazard-coupled window anyway would
    /// publish the member moved past crossed positions reading the
    /// register it defines — here `MOVING_SECOND` mutated to define
    /// `R_MOVE` and `TRAIL` mutated to read it. The validator's own
    /// legality audit refuses with `UnsupportedPair`, not a replay
    /// mismatch, because it reconstructs the window's hazards instead of
    /// trusting the producer's admission record.
    #[test]
    fn forged_member_past_a_coupled_crossed_rejects() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let source = mutated(target, |function, environment| {
            let materialize = environment
                .constraint(environment.selected_keys().materialize_i64)
                .unwrap();
            function.blocks[0].instructions[2] = instruction(
                MOVING_SECOND,
                SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(8),
                },
                materialize,
                &[R_MOVE],
            );
            function.blocks[0].instructions[3] = instruction(
                TRAIL,
                SelectedInstructionKind::CopyI64,
                environment
                    .constraint(environment.selected_keys().copy_i64)
                    .unwrap(),
                &[R_MOVE, R_TRAIL],
            );
        });
        assert_eq!(
            validate_member_run_relocation(
                &source,
                0,
                MOVING,
                MOVING,
                HEAD,
                &environment,
                budget(),
                forged(&source, MOVING, MOVING, 1, 0),
            )
            .unwrap_err(),
            MemberRunRelocationError::UnsupportedPair
        );
    }

    /// A producer that missed a traversal gain would publish the member
    /// into a destination a second, uncrossed predecessor also reaches —
    /// here a detached side block jumping into B. The validator's own
    /// gained-edge audit refuses with `UnsupportedPair`.
    #[test]
    fn forged_move_with_a_gained_traversal_rejects() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let source = mutated(target, |function, environment| {
            let jump_row = environment
                .constraint(environment.selected_keys().jump)
                .unwrap();
            function.blocks.push(block(
                BLOCK_D,
                4,
                Vec::new(),
                jump_terminator(
                    instruction(SIDE_JUMP, SelectedInstructionKind::Jump, jump_row, &[]),
                    successor(BLOCK_B, BlockId::new(2).unwrap(), EDGE_DB),
                ),
            ));
        });
        assert_eq!(
            validate_member_run_relocation(
                &source,
                0,
                MOVING,
                MOVING,
                HEAD,
                &environment,
                budget(),
                forged(&source, MOVING, MOVING, 1, 0),
            )
            .unwrap_err(),
            MemberRunRelocationError::UnsupportedPair
        );
    }

    /// A forged run landing off the derived index publishes a window
    /// whose content is not the admitted move: the member at `MID`'s slot
    /// rather than `HEAD`'s fails the content comparison with
    /// `ReplayMismatch` — the legality audit accepted the window, so only
    /// the placement distinguishes the proposal.
    #[test]
    fn forged_member_off_the_derived_landing_rejects() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let source = fixture(target);
        assert_eq!(
            validate_member_run_relocation(
                &source,
                0,
                MOVING,
                MOVING,
                HEAD,
                &environment,
                budget(),
                forged(&source, MOVING, MOVING, 1, 1),
            )
            .unwrap_err(),
            MemberRunRelocationError::ReplayMismatch
        );
    }
}
