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
    ForkRelocationError, ForkRelocationReceipt, ValidatedForkRelocation,
    relocate_selected_instruction_into_arm, validate_fork_relocation,
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
const MOVING: SelectedInstructionId = SelectedInstructionId(3);
const TRAIL: SelectedInstructionId = SelectedInstructionId(4);
const T_HEAD: SelectedInstructionId = SelectedInstructionId(5);
const T_TAIL: SelectedInstructionId = SelectedInstructionId(6);
const F_HEAD: SelectedInstructionId = SelectedInstructionId(7);
const F_TAIL: SelectedInstructionId = SelectedInstructionId(8);
const HEAD: SelectedInstructionId = SelectedInstructionId(9);
const MID: SelectedInstructionId = SelectedInstructionId(10);
const TAIL: SelectedInstructionId = SelectedInstructionId(11);
const BRANCH: SelectedInstructionId = SelectedInstructionId(12);
const T_JUMP: SelectedInstructionId = SelectedInstructionId(13);
const F_JUMP: SelectedInstructionId = SelectedInstructionId(14);
const RET: SelectedInstructionId = SelectedInstructionId(15);

const POINTER: VirtualRegisterId = VirtualRegisterId(0);
const R_LEAD: VirtualRegisterId = VirtualRegisterId(1);
const R_MOVE: VirtualRegisterId = VirtualRegisterId(2);
const R_TRAIL: VirtualRegisterId = VirtualRegisterId(3);
const R_THEAD: VirtualRegisterId = VirtualRegisterId(4);
const R_TTAIL: VirtualRegisterId = VirtualRegisterId(5);
const R_FHEAD: VirtualRegisterId = VirtualRegisterId(6);
const R_FTAIL: VirtualRegisterId = VirtualRegisterId(7);
const R_HEAD: VirtualRegisterId = VirtualRegisterId(8);
const R_MID: VirtualRegisterId = VirtualRegisterId(9);
const R_TAIL: VirtualRegisterId = VirtualRegisterId(10);
const R_BOUND: VirtualRegisterId = VirtualRegisterId(11);

const BLOCK_B: SelectedBlockId = SelectedBlockId(0);
const BLOCK_T: SelectedBlockId = SelectedBlockId(1);
const BLOCK_F: SelectedBlockId = SelectedBlockId(2);
const BLOCK_J: SelectedBlockId = SelectedBlockId(3);
const EDGE_BT: u64 = 20;
const EDGE_BF: u64 = 21;
const EDGE_TJ: u64 = 22;
const EDGE_FJ: u64 = 23;

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

/// A raw selected-stage unit fixture, not a source/Terminal admission claim.
/// Block B is the entry and ends in a `ConditionalBranchNonZero` whose
/// nonzero edge leads to arm block T and whose zero edge leads to arm
/// block F; each arm's only predecessor is that branch and each ends in a
/// `Jump` to block J, whose terminator is a plain return:
/// `B = [LEAD; MOVING; TRAIL] -> branch -> T = [T_HEAD; T_TAIL] -> jump -> J`
/// and `F = [F_HEAD; F_TAIL] -> jump -> J = [HEAD; MID; TAIL] -> return`.
/// The default move relocates `MOVING` onto `T_HEAD`'s position — the head
/// of T's body — crossing `TRAIL`, the branch terminator, and the landing
/// edge, while J's body keeps its order. On the other edge's path the
/// member no longer executes: F and the F-side traversal of J must never
/// read `R_MOVE` before a write retires it.
fn fixture(target: NativeTarget) -> ValidatedForkRelocation {
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let materialize = environment.constraint(keys.materialize_i64).unwrap();
    let branch_row = environment.constraint(keys.conditional_branch).unwrap();
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
        result_register(R_MOVE, MOVING, 3),
        result_register(R_TRAIL, TRAIL, 4),
        result_register(R_THEAD, T_HEAD, 5),
        result_register(R_TTAIL, T_TAIL, 6),
        result_register(R_FHEAD, F_HEAD, 7),
        result_register(R_FTAIL, F_TAIL, 8),
        result_register(R_HEAD, HEAD, 9),
        result_register(R_MID, MID, 10),
        result_register(R_TAIL, TAIL, 11),
        result_register(R_BOUND, LEAD, 12),
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
            entry_block: BLOCK_B,
            virtual_registers: registers,
            blocks: vec![
                SelectedBlock {
                    id: BLOCK_B,
                    origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
                    instructions: vec![
                        materialization(LEAD, R_LEAD, 5),
                        materialization(MOVING, R_MOVE, 7),
                        materialization(TRAIL, R_TRAIL, 9),
                    ],
                    terminator: SelectedTerminator::ConditionalBranch {
                        instruction: instruction(
                            BRANCH,
                            SelectedInstructionKind::ConditionalBranchNonZero,
                            branch_row,
                            &[],
                        ),
                        when_nonzero: successor(BLOCK_T, BlockId::new(2).unwrap(), EDGE_BT),
                        when_zero: successor(BLOCK_F, BlockId::new(3).unwrap(), EDGE_BF),
                    },
                },
                SelectedBlock {
                    id: BLOCK_T,
                    origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
                    instructions: vec![
                        materialization(T_HEAD, R_THEAD, 11),
                        materialization(T_TAIL, R_TTAIL, 13),
                    ],
                    terminator: jump_terminator(
                        instruction(T_JUMP, SelectedInstructionKind::Jump, jump_row, &[]),
                        successor(BLOCK_J, BlockId::new(4).unwrap(), EDGE_TJ),
                    ),
                },
                SelectedBlock {
                    id: BLOCK_F,
                    origin: SelectedBlockOrigin::Source(BlockId::new(3).unwrap()),
                    instructions: vec![
                        materialization(F_HEAD, R_FHEAD, 15),
                        materialization(F_TAIL, R_FTAIL, 17),
                    ],
                    terminator: jump_terminator(
                        instruction(F_JUMP, SelectedInstructionKind::Jump, jump_row, &[]),
                        successor(BLOCK_J, BlockId::new(4).unwrap(), EDGE_FJ),
                    ),
                },
                SelectedBlock {
                    id: BLOCK_J,
                    origin: SelectedBlockOrigin::Source(BlockId::new(4).unwrap()),
                    instructions: vec![
                        materialization(HEAD, R_HEAD, 19),
                        materialization(MID, R_MID, 21),
                        materialization(TAIL, R_TAIL, 23),
                    ],
                    terminator: SelectedTerminator::Return {
                        instruction: instruction(
                            RET,
                            SelectedInstructionKind::ReturnUnit,
                            return_row,
                            &[],
                        ),
                        psi_return_edge: EdgeId::new(24).unwrap(),
                    },
                },
            ],
        }]
        .into(),
    };
    let identity = selected_instruction_plan_identity(&plan);
    ValidatedForkRelocation {
        receipt: ForkRelocationReceipt {
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
) -> ValidatedForkRelocation {
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
    source: &ValidatedForkRelocation,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
) -> Result<ValidatedForkRelocation, ForkRelocationError> {
    relocate_selected_instruction_into_arm(source, 0, member, destination, environment, budget())
}

/// The member sinks through the fork onto the destination's position on
/// every target: `MOVING` leaves B's body, `TRAIL`, the branch, and the
/// landing edge keep their positions, and the member lands at T's head
/// with the destination and every later position one slot later in their
/// original order — identity, kind, operands, and provenance intact. The
/// replayed proposal restores the source bit-identically.
#[test]
fn member_relocates_into_the_arm() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = fixture(target);
        let result = relocate(&source, &environment, MOVING, T_HEAD).unwrap();
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
            moved.blocks[1]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![MOVING, T_HEAD, T_TAIL]
        );
        assert_eq!(
            moved.blocks[2].instructions,
            original.blocks[2].instructions
        );
        assert_eq!(
            moved.blocks[3].instructions,
            original.blocks[3].instructions
        );
        // The member moved bit-identically; every terminator, edge, and
        // roster stayed untouched.
        assert_eq!(
            moved.blocks[1].instructions[0],
            original.blocks[0].instructions[1]
        );
        for index in 0..4 {
            assert_eq!(
                moved.blocks[index].terminator,
                original.blocks[index].terminator
            );
        }
        assert_eq!(moved.memory_accesses, original.memory_accesses);
        assert_eq!(moved.calls, original.calls);
        assert_eq!(moved.boundary_settlements, original.boundary_settlements);
        validate_fork_relocation(
            &source,
            0,
            MOVING,
            T_HEAD,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
    }
}

/// The destination names the landing position directly: a body instruction
/// puts the member on its index, the arm's terminator-carried instruction
/// lands the member at the body end, and the destination selects which arm
/// the member sinks into — naming a position in F lands it there while the
/// nonzero edge becomes the skipped path.
#[test]
fn member_lands_at_the_named_position() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let middle = relocate(&source, &environment, MOVING, T_TAIL).unwrap();
    assert_eq!(
        middle.transformed().functions[0].blocks[1]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![T_HEAD, MOVING, T_TAIL]
    );
    let body_end = relocate(&source, &environment, MOVING, T_JUMP).unwrap();
    assert_eq!(
        body_end.transformed().functions[0].blocks[1]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![T_HEAD, T_TAIL, MOVING]
    );
    let other_arm = relocate(&source, &environment, MOVING, F_TAIL).unwrap();
    assert_eq!(
        other_arm.transformed().functions[0].blocks[2]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![F_HEAD, MOVING, F_TAIL]
    );
}

/// Every body index relocates: the block-tail member crosses only the
/// branch and the landing edge, while the block-head member crosses its
/// whole trailing body first — and each member's write must still die on
/// the skipped path.
#[test]
fn tail_and_head_members_relocate() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let tail = relocate(&source, &environment, TRAIL, T_HEAD).unwrap();
    assert_eq!(
        tail.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![LEAD, MOVING]
    );
    assert_eq!(
        tail.transformed().functions[0].blocks[1]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![TRAIL, T_HEAD, T_TAIL]
    );
    let head = relocate(&source, &environment, LEAD, T_TAIL).unwrap();
    assert_eq!(
        head.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![MOVING, TRAIL]
    );
    assert_eq!(
        head.transformed().functions[0].blocks[1]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![T_HEAD, LEAD, T_TAIL]
    );
}

/// A degenerate fork whose both edges reach one arm still sinks the member
/// — every traversal of the branch block still executes it exactly once,
/// so no path skips it and the dead-path audit is vacuous.
#[test]
fn degenerate_single_arm_fork_relocates() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let single = mutated(target, |function, _| {
        let successor = match &mut function.blocks[0].terminator {
            SelectedTerminator::ConditionalBranch { when_zero, .. } => when_zero,
            _ => unreachable!(),
        };
        successor.block = BLOCK_T;
        successor.source_target = BlockId::new(2).unwrap();
    });
    let result = relocate(&single, &environment, MOVING, T_TAIL).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![LEAD, TRAIL]
    );
    assert_eq!(
        result.transformed().functions[0].blocks[1]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![T_HEAD, MOVING, T_TAIL]
    );
}

/// A member write a crossed position reads would starve the consumer: in
/// the block tail, in the arm prefix when the landing index is past it —
/// while a read at the landing index itself still observes the member —
/// and on the skipped side, where the member never runs, a read of its
/// definition refuses outright.
#[test]
fn raw_hazard_keeps_order_through_the_fork() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let tail_reads = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[2] = instruction(
            TRAIL,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[R_MOVE, R_TRAIL],
        );
    });
    assert_eq!(
        relocate(&tail_reads, &environment, MOVING, T_HEAD).unwrap_err(),
        ForkRelocationError::UnsupportedPair
    );
    let arm_reads = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[1].instructions[0] = instruction(
            T_HEAD,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[R_MOVE, R_THEAD],
        );
    });
    // Landing at the head does not cross `T_HEAD`; landing at `T_TAIL`
    // crosses it and the member's write would starve the crossed read.
    relocate(&arm_reads, &environment, MOVING, T_HEAD).unwrap();
    assert_eq!(
        relocate(&arm_reads, &environment, MOVING, T_TAIL).unwrap_err(),
        ForkRelocationError::UnsupportedPair
    );
}

/// A member reading a location a crossed instruction writes would observe
/// the new value after the move.
#[test]
fn war_hazard_keeps_order_through_the_fork() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let member_reads_pointer = |edit: &mut dyn FnMut(
        &mut SelectedFunction,
        &register_environment::ValidatedTargetRegisterEnvironment,
    )| {
        mutated(target, |function, environment| {
            let copy = environment
                .constraint(environment.selected_keys().copy_i64)
                .unwrap()
                .clone();
            function.blocks[0].instructions[1] = instruction(
                MOVING,
                SelectedInstructionKind::CopyI64,
                &copy,
                &[POINTER, R_MOVE],
            );
            edit(function, environment);
        })
    };
    let arm_writes = member_reads_pointer(&mut |function, environment| {
        let materialize = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap()
            .clone();
        function.blocks[1].instructions[0] = instruction(
            T_HEAD,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(9),
            },
            &materialize,
            &[POINTER],
        );
    });
    relocate(&arm_writes, &environment, MOVING, T_HEAD).unwrap();
    assert_eq!(
        relocate(&arm_writes, &environment, MOVING, T_TAIL).unwrap_err(),
        ForkRelocationError::UnsupportedPair
    );
}

/// A member and a crossed instruction writing the same register would
/// change which definition later positions observe.
#[test]
fn waw_hazard_keeps_order_through_the_fork() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let tail_writes = mutated(target, |function, environment| {
        let materialize = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[2] = instruction(
            TRAIL,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(9),
            },
            &materialize,
            &[R_MOVE],
        );
    });
    assert_eq!(
        relocate(&tail_writes, &environment, MOVING, T_HEAD).unwrap_err(),
        ForkRelocationError::UnsupportedPair
    );
    let arm_writes = mutated(target, |function, environment| {
        let materialize = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap()
            .clone();
        function.blocks[1].instructions[0] = instruction(
            T_HEAD,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(9),
            },
            &materialize,
            &[R_MOVE],
        );
    });
    assert_eq!(
        relocate(&arm_writes, &environment, MOVING, T_TAIL).unwrap_err(),
        ForkRelocationError::UnsupportedPair
    );
    relocate(&arm_writes, &environment, MOVING, T_HEAD).unwrap();
}

/// Condition state couples like registers through the fork: the branch
/// terminator reads the target's condition units, so a flag-publishing
/// member can never leave; a flag-reading member cannot cross a flag
/// writer in its own tail or the arm prefix; and a flag consumer in the
/// arm does not block a flag-inert member.
#[test]
fn condition_state_couples_through_the_fork() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let subtract_kind = SelectedInstructionKind::ExactSubtractI64 {
        obligation: ObligationId::new(11).unwrap(),
        accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes([7; 32]),
    };
    // A flag-publishing member rewrites the units the branch terminator
    // itself reads: the member's definition would land after the read.
    let flag_writer = mutated(target, |function, environment| {
        let subtract = environment
            .constraint(environment.selected_keys().subtract_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[1] =
            instruction(MOVING, subtract_kind, &subtract, &[POINTER, R_MOVE, R_MOVE]);
    });
    assert_eq!(
        relocate(&flag_writer, &environment, MOVING, T_HEAD).unwrap_err(),
        ForkRelocationError::UnsupportedPair
    );
    // A flag-reading member cannot cross a flag writer in its own block
    // tail — it would observe the new flags after the move.
    let member_reads_flags = |edit: &mut dyn FnMut(
        &mut SelectedFunction,
        &register_environment::ValidatedTargetRegisterEnvironment,
    )| {
        mutated(target, |function, environment| {
            let boolean = environment
                .constraint(environment.selected_keys().materialize_boolean)
                .unwrap()
                .clone();
            function.blocks[0].instructions[1] = instruction(
                MOVING,
                SelectedInstructionKind::MaterializeBooleanEqual,
                &boolean,
                &[R_MOVE],
            );
            edit(function, environment);
        })
    };
    let tail_writes_flags = member_reads_flags(&mut |function, environment| {
        let subtract = environment
            .constraint(environment.selected_keys().subtract_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[2] = instruction(
            TRAIL,
            subtract_kind,
            &subtract,
            &[POINTER, R_TRAIL, R_TRAIL],
        );
    });
    assert_eq!(
        relocate(&tail_writes_flags, &environment, MOVING, T_HEAD).unwrap_err(),
        ForkRelocationError::UnsupportedPair
    );
    // The same coupling holds for a compare publishing flags in the arm
    // prefix; landing before the flag writer admits because the member's
    // read still observes the flags the branch saw.
    let arm_writes_flags = member_reads_flags(&mut |function, environment| {
        let compare = environment
            .constraint(environment.selected_keys().compare_i64)
            .unwrap()
            .clone();
        function.blocks[1].instructions[0] = instruction(
            T_HEAD,
            SelectedInstructionKind::CompareI64,
            &compare,
            &[R_THEAD, R_TTAIL],
        );
    });
    relocate(&arm_writes_flags, &environment, MOVING, T_HEAD).unwrap();
    assert_eq!(
        relocate(&arm_writes_flags, &environment, MOVING, T_TAIL).unwrap_err(),
        ForkRelocationError::UnsupportedPair
    );
}

/// The branch terminator is a crossed position, not a window barrier: a
/// register the branch reads couples like any crossed read, while a read
/// it shares with the member admits. The landing arm's own terminator is
/// never crossed — the member lands inside the body — so its reads do not
/// bound the move.
#[test]
fn terminator_positions_couple() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let terminator_reads = mutated(target, |function, environment| {
        let branch_row = environment
            .constraint(environment.selected_keys().conditional_branch)
            .unwrap()
            .clone();
        let class = function.blocks[0].instructions[0].operands[0].class;
        let mut branch = instruction(
            BRANCH,
            SelectedInstructionKind::ConditionalBranchNonZero,
            &branch_row,
            &[],
        );
        branch.operands.push(SelectedOperand {
            operand: 0,
            virtual_register: R_MOVE,
            access: RegisterOperandAccess::Use,
            class,
            fixed_view: None,
            tied_to: None,
            early_clobber: false,
        });
        let (when_nonzero, when_zero) = match &function.blocks[0].terminator {
            SelectedTerminator::ConditionalBranch {
                when_nonzero,
                when_zero,
                ..
            } => (when_nonzero.clone(), when_zero.clone()),
            _ => unreachable!(),
        };
        function.blocks[0].terminator = SelectedTerminator::ConditionalBranch {
            instruction: branch,
            when_nonzero,
            when_zero,
        };
    });
    assert_eq!(
        relocate(&terminator_reads, &environment, MOVING, T_HEAD).unwrap_err(),
        ForkRelocationError::UnsupportedPair
    );
    // The landing arm's terminator is past the landing index, never a
    // crossed position — its read of the member's result admits.
    let arm_terminator_reads = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        let class = function.blocks[0].instructions[0].operands[0].class;
        let mut jump = instruction(T_JUMP, SelectedInstructionKind::Jump, &jump_row, &[]);
        jump.operands.push(SelectedOperand {
            operand: 0,
            virtual_register: R_MOVE,
            access: RegisterOperandAccess::Use,
            class,
            fixed_view: None,
            tied_to: None,
            early_clobber: false,
        });
        let successor = match &function.blocks[1].terminator {
            SelectedTerminator::Jump { successor, .. } => successor.clone(),
            _ => unreachable!(),
        };
        function.blocks[1].terminator = jump_terminator(jump, successor);
    });
    relocate(&arm_terminator_reads, &environment, MOVING, T_HEAD).unwrap();
}

/// The landing edge's register transports sit between the member's old and
/// new positions: a member defining the transported argument would hand
/// the binding a stale value, a member defining or reading the parameter
/// would be overwritten or observe the transported value, while a member
/// merely reading the argument crosses freely. The skipped edge's
/// transports are not crossed — they join the dead-path audit, where an
/// argument reading a live member definition refuses and a parameter
/// writing one retires it.
#[test]
fn register_transports_bind_the_landing_edge() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let binding =
        |argument: VirtualRegisterId, parameter: VirtualRegisterId| SelectedValueBinding {
            semantic: abstract_operations::ValueBinding {
                parameter: ValueId::new(20).unwrap(),
                argument: ValueId::new(1).unwrap(),
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                ),
            },
            transport: SelectedValueTransport::Registers {
                argument,
                parameter,
            },
        };
    let member_writes = |register: VirtualRegisterId| {
        move |function: &mut SelectedFunction,
              environment: &register_environment::ValidatedTargetRegisterEnvironment| {
            let materialize = environment
                .constraint(environment.selected_keys().materialize_i64)
                .unwrap()
                .clone();
            function.blocks[0].instructions[1] = instruction(
                MOVING,
                SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(9),
                },
                &materialize,
                &[register],
            );
        }
    };
    let member_reads = |register: VirtualRegisterId| {
        move |function: &mut SelectedFunction,
              environment: &register_environment::ValidatedTargetRegisterEnvironment| {
            let copy = environment
                .constraint(environment.selected_keys().copy_i64)
                .unwrap()
                .clone();
            function.blocks[0].instructions[1] = instruction(
                MOVING,
                SelectedInstructionKind::CopyI64,
                &copy,
                &[register, R_MOVE],
            );
        }
    };
    let on_landing_edge = |edit: &mut dyn FnMut(
        &mut SelectedFunction,
        &register_environment::ValidatedTargetRegisterEnvironment,
    )| {
        mutated(target, |function, environment| {
            let successor = match &mut function.blocks[0].terminator {
                SelectedTerminator::ConditionalBranch { when_nonzero, .. } => when_nonzero,
                _ => unreachable!(),
            };
            successor.bindings.push(binding(POINTER, R_BOUND));
            edit(function, environment);
        })
    };
    relocate(
        &on_landing_edge(&mut |_, _| {}),
        &environment,
        MOVING,
        T_HEAD,
    )
    .unwrap();
    relocate(
        &on_landing_edge(&mut member_reads(POINTER)),
        &environment,
        MOVING,
        T_HEAD,
    )
    .unwrap();
    assert_eq!(
        relocate(
            &on_landing_edge(&mut member_writes(POINTER)),
            &environment,
            MOVING,
            T_HEAD,
        )
        .unwrap_err(),
        ForkRelocationError::UnsupportedPair
    );
    assert_eq!(
        relocate(
            &on_landing_edge(&mut member_writes(R_BOUND)),
            &environment,
            MOVING,
            T_HEAD,
        )
        .unwrap_err(),
        ForkRelocationError::UnsupportedPair
    );
    assert_eq!(
        relocate(
            &on_landing_edge(&mut member_reads(R_BOUND)),
            &environment,
            MOVING,
            T_HEAD,
        )
        .unwrap_err(),
        ForkRelocationError::UnsupportedPair
    );
    // On the skipped edge a transport argument reading the member's write
    // is a stale observation on a path the member never ran; a parameter
    // writing it retires the stale definition before any reader.
    let on_skipped_edge = |binding_of: &mut dyn FnMut() -> SelectedValueBinding| {
        mutated(target, |function, _| {
            let successor = match &mut function.blocks[0].terminator {
                SelectedTerminator::ConditionalBranch { when_zero, .. } => when_zero,
                _ => unreachable!(),
            };
            successor.bindings.push(binding_of());
        })
    };
    assert_eq!(
        relocate(
            &on_skipped_edge(&mut || binding(R_MOVE, R_BOUND)),
            &environment,
            MOVING,
            T_HEAD,
        )
        .unwrap_err(),
        ForkRelocationError::UnsupportedPair
    );
    relocate(
        &on_skipped_edge(&mut || binding(POINTER, R_MOVE)),
        &environment,
        MOVING,
        T_HEAD,
    )
    .unwrap();
}

/// The fork must be a real fork for this move: the member's block needs a
/// two-successor conditional terminator, the destination must name a
/// position inside a block the branch reaches, the arm's only predecessors
/// are that branch's edges, and the arm must be a plain source block —
/// never the member's own block, the entry block, or an implementation
/// block.
#[test]
fn the_fork_must_open() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A `Jump` terminator is the single-edge family's case.
    let jumped = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        function.blocks[0].terminator = jump_terminator(
            instruction(BRANCH, SelectedInstructionKind::Jump, &jump_row, &[]),
            successor(BLOCK_T, BlockId::new(2).unwrap(), EDGE_BT),
        );
    });
    assert_eq!(
        relocate(&jumped, &environment, MOVING, T_HEAD).unwrap_err(),
        ForkRelocationError::UnsupportedPair
    );
    // The destination must name a position in a branch target: a member of
    // the member's own block, a join position, and a dangling id refuse.
    let source = fixture(target);
    for destination in [LEAD, HEAD, SelectedInstructionId(99)] {
        assert_eq!(
            relocate(&source, &environment, MOVING, destination).unwrap_err(),
            ForkRelocationError::UnsupportedPair,
            "destination {destination:?}"
        );
    }
    // A second predecessor into the arm gives its stream a member that
    // never ran on that path.
    let extra_pred = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        function.blocks[2].terminator = jump_terminator(
            instruction(F_JUMP, SelectedInstructionKind::Jump, &jump_row, &[]),
            successor(BLOCK_T, BlockId::new(2).unwrap(), EDGE_FJ),
        );
    });
    assert_eq!(
        relocate(&extra_pred, &environment, MOVING, T_HEAD).unwrap_err(),
        ForkRelocationError::UnsupportedPair
    );
    // An implementation-origin arm carries boundary work the bounded audit
    // does not cross.
    let cased = mutated(target, |function, _| {
        function.blocks[1].origin = SelectedBlockOrigin::CaseDispatch {
            source: BlockId::new(2).unwrap(),
            case_ordinal: 0,
        };
    });
    assert_eq!(
        relocate(&cased, &environment, MOVING, T_HEAD).unwrap_err(),
        ForkRelocationError::UnsupportedPair
    );
    // An arm that is the member's own block names an in-block move, and an
    // arm that is the entry block was reached with no predecessor at all.
    let self_edge = mutated(target, |function, _| {
        let successor = match &mut function.blocks[0].terminator {
            SelectedTerminator::ConditionalBranch { when_nonzero, .. } => when_nonzero,
            _ => unreachable!(),
        };
        successor.block = BLOCK_B;
        successor.source_target = BlockId::new(1).unwrap();
    });
    assert_eq!(
        relocate(&self_edge, &environment, MOVING, LEAD).unwrap_err(),
        ForkRelocationError::UnsupportedPair
    );
}

/// Only a plain semantic edge carries the member into the arm: a
/// continuation role, case custody, per-edge fuel, or a live structural
/// transport on a landing edge all refuse. The skipped edge is never
/// crossed, so fuel and inert structural payloads on it stay free — while
/// a case payload or structural binding that reads a live member
/// definition is a dead-path observation and refuses.
#[test]
fn only_plain_semantic_edges_carry_the_member() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for role in [
        SelectedSuccessorRole::EdgeTransferContinuation,
        SelectedSuccessorRole::CaseDispatchContinuation,
    ] {
        let continued = mutated(target, |function, _| {
            let successor = match &mut function.blocks[0].terminator {
                SelectedTerminator::ConditionalBranch { when_nonzero, .. } => when_nonzero,
                _ => unreachable!(),
            };
            successor.role = role;
        });
        assert_eq!(
            relocate(&continued, &environment, MOVING, T_HEAD).unwrap_err(),
            ForkRelocationError::UnsupportedPair
        );
    }
    let case_edge = mutated(target, |function, _| {
        let successor = match &mut function.blocks[0].terminator {
            SelectedTerminator::ConditionalBranch { when_nonzero, .. } => when_nonzero,
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
        relocate(&case_edge, &environment, MOVING, T_HEAD).unwrap_err(),
        ForkRelocationError::UnsupportedPair
    );
    let fueled = mutated(target, |function, _| {
        let successor = match &mut function.blocks[0].terminator {
            SelectedTerminator::ConditionalBranch { when_nonzero, .. } => when_nonzero,
            _ => unreachable!(),
        };
        successor.fuel.push(optimization_unit::FuelSettlement {
            site: optimization_unit::PsiProvenance::Operation(OperationId::new(30).unwrap()),
            units: 1,
        });
    });
    assert_eq!(
        relocate(&fueled, &environment, MOVING, T_HEAD).unwrap_err(),
        ForkRelocationError::UnsupportedPair
    );
    // A live structural transport on the landing edge moves a stored value
    // the member would cross.
    let structural = mutated(target, |function, _| {
        let successor = match &mut function.blocks[0].terminator {
            SelectedTerminator::ConditionalBranch { when_nonzero, .. } => when_nonzero,
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
        relocate(&structural, &environment, MOVING, T_HEAD).unwrap_err(),
        ForkRelocationError::UnsupportedPair
    );
    // Fuel on the skipped edge is never crossed and stays free.
    let fueled_skipped = mutated(target, |function, _| {
        let successor = match &mut function.blocks[0].terminator {
            SelectedTerminator::ConditionalBranch { when_zero, .. } => when_zero,
            _ => unreachable!(),
        };
        successor.fuel.push(optimization_unit::FuelSettlement {
            site: optimization_unit::PsiProvenance::Operation(OperationId::new(30).unwrap()),
            units: 1,
        });
    });
    relocate(&fueled_skipped, &environment, MOVING, T_HEAD).unwrap();
    // A case payload on the skipped edge whose transport reads the
    // member's definition is a dead-path observation; an `Unused` payload
    // moves nothing.
    let case_reads_member = mutated(target, |function, _| {
        let successor = match &mut function.blocks[0].terminator {
            SelectedTerminator::ConditionalBranch { when_zero, .. } => when_zero,
            _ => unreachable!(),
        };
        successor.structural_case = Some(selected_instructions::SelectedStructuralCaseEdge {
            slot: selected_instructions::LocalStorageSlotId::Spill { register: R_BOUND },
            case: StructuralCaseId::new(1).unwrap(),
            case_tag: 0,
            payloads: vec![selected_instructions::SelectedCasePayloadBinding {
                semantic: legalized_operations::LegalizedStructuralCasePayload {
                    field: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
                    field_byte_offset: 0,
                    parameter: legalized_operations::LegalizedValueDefinition {
                        value: ValueId::new(21).unwrap(),
                        scalar_type: ScalarType::Integer(
                            IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                        ),
                        definition_site: ValueDefinitionSite::BlockParameter {
                            block: BlockId::new(3).unwrap(),
                            position: 0,
                        },
                    },
                },
                transport: selected_instructions::SelectedCasePayloadTransport::Registers {
                    argument: R_MOVE,
                    parameter: R_BOUND,
                },
            }],
            trivial_affine_discards: Vec::new(),
        });
    });
    assert_eq!(
        relocate(&case_reads_member, &environment, MOVING, T_HEAD).unwrap_err(),
        ForkRelocationError::UnsupportedPair
    );
    // A structural binding on the skipped edge reads its argument register
    // at the boundary — reading the member's definition refuses.
    let structural_reads_member = mutated(target, |function, _| {
        let successor = match &mut function.blocks[0].terminator {
            SelectedTerminator::ConditionalBranch { when_zero, .. } => when_zero,
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
                    argument: R_MOVE,
                    destination: selected_instructions::LocalStorageSlotId::Spill {
                        register: R_BOUND,
                    },
                    byte_size: 8,
                    alignment: 8,
                },
            });
    });
    assert_eq!(
        relocate(&structural_reads_member, &environment, MOVING, T_HEAD).unwrap_err(),
        ForkRelocationError::UnsupportedPair
    );
}

/// Barrier kinds and call-roster entries refuse as the member or anywhere
/// inside the crossed window — the block tail and the arm prefix included
/// — while a barrier sitting at the landing index is never crossed and a
/// call-roster row on the skipped side still executes on its own path.
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
            relocate(&member_barrier, &environment, MOVING, T_HEAD).unwrap_err(),
            ForkRelocationError::UnsupportedInstruction,
            "member {kind:?}"
        );
        let tail_barrier = mutated(target, |function, _| {
            function.blocks[0].instructions[2].kind = kind;
        });
        assert_eq!(
            relocate(&tail_barrier, &environment, MOVING, T_HEAD).unwrap_err(),
            ForkRelocationError::UnsupportedInstruction,
            "crossed tail {kind:?}"
        );
        let arm_barrier = mutated(target, |function, _| {
            function.blocks[1].instructions[0].kind = kind;
        });
        assert_eq!(
            relocate(&arm_barrier, &environment, MOVING, T_TAIL).unwrap_err(),
            ForkRelocationError::UnsupportedInstruction,
            "crossed arm {kind:?}"
        );
        // A barrier at the landing index is never crossed.
        relocate(&arm_barrier, &environment, MOVING, T_HEAD).unwrap();
        // A barrier on the skipped side still runs on its own path; it
        // only refuses if it would read a still-live member definition.
        let skipped_barrier = mutated(target, |function, _| {
            function.blocks[2].instructions[0].kind = kind;
        });
        relocate(&skipped_barrier, &environment, MOVING, T_HEAD).unwrap();
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
        (MOVING, T_HEAD),
        (TRAIL, T_HEAD),
        (T_HEAD, T_TAIL),
        (BRANCH, T_HEAD),
    ] {
        let contract = mutated(target, |function, _| {
            function.calls.push(call_contract(instruction_id));
        });
        assert_eq!(
            relocate(&contract, &environment, MOVING, destination).unwrap_err(),
            ForkRelocationError::UnsupportedInstruction,
            "call roster {instruction_id:?}"
        );
    }
    // The arm terminator is outside the window: a call-roster row there is
    // a dead-path position like any other.
    let contract = mutated(target, |function, _| {
        function.calls.push(call_contract(T_JUMP));
    });
    relocate(&contract, &environment, MOVING, T_HEAD).unwrap();
}

/// Only pure register and condition-state work may sink: a member carrying
/// a memory roster row would run its access only on the landing path, and
/// a row-less load or private-slot store sheds the same access — so every
/// memory-touching kind refuses as the member, while an accounted or
/// unaccounted access elsewhere on the skipped paths never moves.
#[test]
fn member_must_be_memory_inert() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A roster-carrying member's recorded access would become conditional.
    let roster_member = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[1] = instruction(
            MOVING,
            SelectedInstructionKind::Load8 { byte_offset: 0 },
            &load,
            &[POINTER, R_MOVE],
        );
        function.memory_accesses.push(access(
            MOVING,
            PlaceId::new(1).unwrap(),
            SelectedMemoryAccessRole::ReadPlace,
        ));
    });
    assert_eq!(
        relocate(&roster_member, &environment, MOVING, T_HEAD).unwrap_err(),
        ForkRelocationError::UnsupportedInstruction
    );
    // A row-less load still performs an access whose absence the skipped
    // path would observe through its result — and any fault it carried.
    let rowless_load = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[1] = instruction(
            MOVING,
            SelectedInstructionKind::Load8 { byte_offset: 0 },
            &load,
            &[POINTER, R_MOVE],
        );
    });
    assert_eq!(
        relocate(&rowless_load, &environment, MOVING, T_HEAD).unwrap_err(),
        ForkRelocationError::UnsupportedInstruction
    );
    // A row-less private-slot store still writes storage on every
    // traversal; sinking it would leave the slot stale on the skipped
    // path.
    let rowless_store = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[1] = instruction(
            MOVING,
            SelectedInstructionKind::Store64 {
                slot: selected_instructions::FrameStorageSlotId::Local(
                    selected_instructions::LocalStorageSlotId::Spill { register: R_BOUND },
                ),
                byte_offset: 0,
            },
            &store,
            &[POINTER, R_MOVE],
        );
    });
    assert_eq!(
        relocate(&rowless_store, &environment, MOVING, T_HEAD).unwrap_err(),
        ForkRelocationError::UnsupportedInstruction
    );
    // Memory work on the skipped paths never moves: a store in F refuses
    // only if it would read a live member definition.
    let skipped_store = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
        function.blocks[2].instructions[0] = instruction(
            F_HEAD,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            &store,
            &[POINTER, R_FHEAD],
        );
        function.memory_accesses.push(access(
            F_HEAD,
            PlaceId::new(2).unwrap(),
            SelectedMemoryAccessRole::WritePlace,
        ));
    });
    relocate(&skipped_store, &environment, MOVING, T_HEAD).unwrap();
    let skipped_store_reads_member = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
        function.blocks[2].instructions[0] = instruction(
            F_HEAD,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            &store,
            &[R_MOVE, R_FHEAD],
        );
        function.memory_accesses.push(access(
            F_HEAD,
            PlaceId::new(2).unwrap(),
            SelectedMemoryAccessRole::WritePlace,
        ));
    });
    assert_eq!(
        relocate(&skipped_store_reads_member, &environment, MOVING, T_HEAD).unwrap_err(),
        ForkRelocationError::UnsupportedPair
    );
}

/// Every location the member writes must be dead — unread until rewritten —
/// on every path the branch's other edges reach: a reader in the skipped
/// arm, in a deeper block, or in the shared join reached through the
/// skipped side all refuse, while a write that retires the stale
/// definition before any reader admits. A loop back through the member's
/// own block scans it without the member's slot.
#[test]
fn member_writes_must_die_on_the_skipped_paths() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let reads_member = |block: usize, index: usize, destination: VirtualRegisterId| {
        move |function: &mut SelectedFunction,
              environment: &register_environment::ValidatedTargetRegisterEnvironment| {
            let copy = environment
                .constraint(environment.selected_keys().copy_i64)
                .unwrap()
                .clone();
            let id = function.blocks[block].instructions[index].id;
            function.blocks[block].instructions[index] = instruction(
                id,
                SelectedInstructionKind::CopyI64,
                &copy,
                &[R_MOVE, destination],
            );
        }
    };
    // A reader anywhere on the skipped side observes the stale definition.
    for (block, index, destination, label) in [
        (2usize, 0usize, R_FHEAD, "skipped arm"),
        (2usize, 1usize, R_FTAIL, "skipped arm tail"),
        (3usize, 0usize, R_HEAD, "join via the skipped arm"),
        (3usize, 2usize, R_TAIL, "join tail via the skipped arm"),
    ] {
        let reading = mutated(target, reads_member(block, index, destination));
        assert_eq!(
            relocate(&reading, &environment, MOVING, T_HEAD).unwrap_err(),
            ForkRelocationError::UnsupportedPair,
            "{label}"
        );
    }
    // The skipped arm's terminator reading the member's write refuses just
    // the same — its position is on the skipped path.
    let terminator_reads = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        let class = function.blocks[0].instructions[0].operands[0].class;
        let mut jump = instruction(F_JUMP, SelectedInstructionKind::Jump, &jump_row, &[]);
        jump.operands.push(SelectedOperand {
            operand: 0,
            virtual_register: R_MOVE,
            access: RegisterOperandAccess::Use,
            class,
            fixed_view: None,
            tied_to: None,
            early_clobber: false,
        });
        let successor = match &function.blocks[2].terminator {
            SelectedTerminator::Jump { successor, .. } => successor.clone(),
            _ => unreachable!(),
        };
        function.blocks[2].terminator = jump_terminator(jump, successor);
    });
    assert_eq!(
        relocate(&terminator_reads, &environment, MOVING, T_HEAD).unwrap_err(),
        ForkRelocationError::UnsupportedPair
    );
    // A write retiring the stale definition before any reader admits — the
    // reader then observes the rewrite it always saw on that path.
    let retired = mutated(target, |function, environment| {
        let materialize = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap()
            .clone();
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[2].instructions[0] = instruction(
            F_HEAD,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(9),
            },
            &materialize,
            &[R_MOVE],
        );
        function.blocks[2].instructions[1] = instruction(
            F_TAIL,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[R_MOVE, R_FTAIL],
        );
        function.blocks[3].instructions[0] = instruction(
            HEAD,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[R_MOVE, R_HEAD],
        );
    });
    relocate(&retired, &environment, MOVING, T_HEAD).unwrap();
    // A deeper dead-path reader still refuses: the walk crosses blocks.
    let deeper = mutated(target, |function, environment| {
        let materialize = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap()
            .clone();
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        // F jumps to a new intermediate block D, which jumps to J.
        function.blocks[2].terminator = jump_terminator(
            instruction(F_JUMP, SelectedInstructionKind::Jump, &jump_row, &[]),
            successor(SelectedBlockId(4), BlockId::new(5).unwrap(), 25),
        );
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(4),
            origin: SelectedBlockOrigin::Source(BlockId::new(5).unwrap()),
            instructions: vec![
                instruction(
                    SelectedInstructionId(30),
                    SelectedInstructionKind::MaterializeI64 {
                        value: IntegerValue::Unsigned(29),
                    },
                    &materialize,
                    &[R_HEAD],
                ),
                instruction(
                    SelectedInstructionId(31),
                    SelectedInstructionKind::CopyI64,
                    &copy,
                    &[R_MOVE, R_MID],
                ),
            ],
            terminator: jump_terminator(
                instruction(
                    SelectedInstructionId(32),
                    SelectedInstructionKind::Jump,
                    &jump_row,
                    &[],
                ),
                successor(BLOCK_J, BlockId::new(4).unwrap(), 26),
            ),
        });
    });
    assert_eq!(
        relocate(&deeper, &environment, MOVING, T_HEAD).unwrap_err(),
        ForkRelocationError::UnsupportedPair
    );
    // A loop back through the member's own block scans the vacated stream:
    // a prefix read of the still-live definition refuses, while a prefix
    // rewrite retires it before any reader and admits.
    let looped_read = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[2].terminator = jump_terminator(
            instruction(F_JUMP, SelectedInstructionKind::Jump, &jump_row, &[]),
            successor(BLOCK_B, BlockId::new(1).unwrap(), 25),
        );
        function.blocks[0].instructions[0] = instruction(
            LEAD,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[R_MOVE, R_LEAD],
        );
    });
    assert_eq!(
        relocate(&looped_read, &environment, MOVING, T_HEAD).unwrap_err(),
        ForkRelocationError::UnsupportedPair
    );
    let looped_retired = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        let materialize = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap()
            .clone();
        function.blocks[2].terminator = jump_terminator(
            instruction(F_JUMP, SelectedInstructionKind::Jump, &jump_row, &[]),
            successor(BLOCK_B, BlockId::new(1).unwrap(), 25),
        );
        function.blocks[0].instructions[0] = instruction(
            LEAD,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(9),
            },
            &materialize,
            &[R_MOVE],
        );
    });
    relocate(&looped_retired, &environment, MOVING, T_HEAD).unwrap();
}

/// A settlement positioned past the member's index in its own block
/// observed it inside that executed prefix, and one past the landing index
/// in the arm newly observes it there; settlements on the skipped paths
/// never had the member in their streams.
#[test]
fn boundary_settlements_bound_the_window() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // In the member's block, settlements at or before the member's index
    // admit; positions past it observed the member inside the prefix.
    for (position, admits) in [(0u32, true), (1, true), (2, false), (3, false)] {
        let settled = mutated(target, |function, _| {
            function
                .boundary_settlements
                .push(settlement(BLOCK_B, position, 50));
        });
        let result = relocate(&settled, &environment, MOVING, T_HEAD);
        assert_eq!(
            result.is_ok(),
            admits,
            "source-block settlement at {position}"
        );
    }
    // In the arm the bound is the landing index: at or before it the
    // executed prefix is unchanged; past it the member joins the prefix.
    for (position, admits) in [(0u32, true), (1, true), (2, false)] {
        let settled = mutated(target, |function, _| {
            function
                .boundary_settlements
                .push(settlement(BLOCK_T, position, 51));
        });
        let result = relocate(&settled, &environment, MOVING, T_TAIL);
        assert_eq!(result.is_ok(), admits, "arm settlement at {position}");
    }
    // Landing at the body end keeps every arm settlement: none sits past
    // the member's new index.
    let settled = mutated(target, |function, _| {
        function
            .boundary_settlements
            .push(settlement(BLOCK_T, 2, 53));
    });
    relocate(&settled, &environment, MOVING, T_JUMP).unwrap();
    // Settlements on the skipped side and in the join never observe the
    // member — it was never in those streams.
    for (block, position) in [(BLOCK_F, 0u32), (BLOCK_F, 2), (BLOCK_J, 0), (BLOCK_J, 3)] {
        let settled = mutated(target, |function, _| {
            function
                .boundary_settlements
                .push(settlement(block, position, 54));
        });
        relocate(&settled, &environment, MOVING, T_HEAD).unwrap();
    }
}

/// The pair must name one member in the branch block's body and one
/// position in a block the branch reaches: an unknown member, a wrong
/// function, a destination outside the branch targets, a member whose own
/// block lacks the two-successor terminator, and a terminator id as member
/// all refuse.
#[test]
fn only_the_named_fork_window_relocates() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // An unknown member or function index never locates the window.
    assert_eq!(
        relocate_selected_instruction_into_arm(
            &source,
            0,
            SelectedInstructionId(99),
            T_HEAD,
            &environment,
            budget(),
        )
        .unwrap_err(),
        ForkRelocationError::SourceMismatch
    );
    assert_eq!(
        relocate_selected_instruction_into_arm(&source, 9, MOVING, T_HEAD, &environment, budget(),)
            .unwrap_err(),
        ForkRelocationError::SourceMismatch
    );
    // A body member of a block without the branch names no window: the
    // join's own members have only a return terminator, and an arm's block
    // ends in `Jump`, not the two-successor form.
    assert_eq!(
        relocate(&source, &environment, HEAD, TAIL).unwrap_err(),
        ForkRelocationError::UnsupportedPair
    );
    assert_eq!(
        relocate(&source, &environment, T_HEAD, HEAD).unwrap_err(),
        ForkRelocationError::UnsupportedPair
    );
    // Naming the branch block's terminator as member is not a body
    // position; naming it as destination names no branch target.
    assert_eq!(
        relocate(&source, &environment, BRANCH, T_HEAD).unwrap_err(),
        ForkRelocationError::SourceMismatch
    );
    assert_eq!(
        relocate(&source, &environment, MOVING, BRANCH).unwrap_err(),
        ForkRelocationError::UnsupportedPair
    );
}

/// Replay consumes only the exact move: a proposal that drops the member,
/// lands it anywhere else, permutes the crossed positions, or carries an
/// unrelated edit all reject.
#[test]
fn replay_rejects_anything_but_the_move() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let result = relocate(&source, &environment, MOVING, T_TAIL).unwrap();
    // The honest proposal replays.
    validate_fork_relocation(
        &source,
        0,
        MOVING,
        T_TAIL,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // The member at the wrong index rejects.
    let mut displaced = result.transformed().clone();
    let member = displaced.functions[0].blocks[1].instructions.remove(1);
    displaced.functions[0].blocks[1]
        .instructions
        .insert(2, member.clone());
    assert_eq!(
        validate_fork_relocation(
            &source,
            0,
            MOVING,
            T_TAIL,
            &environment,
            budget(),
            displaced
        )
        .unwrap_err(),
        ForkRelocationError::ReplayMismatch
    );
    // The member left in its own block rejects.
    let mut unmoved = result.transformed().clone();
    unmoved.functions[0].blocks[1].instructions.remove(1);
    unmoved.functions[0].blocks[0]
        .instructions
        .insert(2, member);
    assert_eq!(
        validate_fork_relocation(&source, 0, MOVING, T_TAIL, &environment, budget(), unmoved)
            .unwrap_err(),
        ForkRelocationError::ReplayMismatch
    );
    // A dropped instruction in the landing arm rejects.
    let mut dropped = result.transformed().clone();
    dropped.functions[0].blocks[1].instructions.pop();
    assert_eq!(
        validate_fork_relocation(&source, 0, MOVING, T_TAIL, &environment, budget(), dropped)
            .unwrap_err(),
        ForkRelocationError::ReplayMismatch
    );
    // An unrelated literal edit inside the source block rejects.
    let mut edited = result.transformed().clone();
    edited.functions[0].blocks[0].instructions[0].kind = SelectedInstructionKind::MaterializeI64 {
        value: IntegerValue::Unsigned(12),
    };
    assert_eq!(
        validate_fork_relocation(&source, 0, MOVING, T_TAIL, &environment, budget(), edited)
            .unwrap_err(),
        ForkRelocationError::ReplayMismatch
    );
    // Naming a different window on the same proposal re-derives a
    // different landing and rejects.
    assert_eq!(
        validate_fork_relocation(
            &source,
            0,
            MOVING,
            T_HEAD,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap_err(),
        ForkRelocationError::ReplayMismatch
    );
}

/// The bounded audit is measured: the fork window prices every scan,
/// crossed-surface pair, roster row, and the dead-path fixpoint bound
/// against the work budget, and a budget one step short refuses rather
/// than skimping. Landing at `T_TAIL` crosses one more arm position than
/// landing at `T_HEAD`.
#[test]
fn measured_validation_step_boundary() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // The member-locate scan prices every block's body plus terminator
    // once across the plan (4+3+3+4 = 14), and again for this function's
    // blocks (14). The crossed surfaces pair the member (1) against TRAIL
    // (1) and the branch terminator (2 uses + 1 definition on x86-64):
    // 2+4 = 6 steps. The dead-path bound prices each block's body,
    // terminator, and edge surfaces once per member location plus the
    // initial scan: on x86-64 the arm bodies cost 1 each, the jumps 2,
    // the branch 3, and the return 9 — (3+3)+(2+2)+(2+2)+(3+9) = 26 —
    // times one written member register plus one: 26*2 = 52.
    let steps: u64 = 14 + 14 + 6 + 52;
    let exact = OptimizationWorkBudget::new(1, 1, steps, 1, 1).unwrap();
    relocate_selected_instruction_into_arm(&source, 0, MOVING, T_HEAD, &environment, exact)
        .unwrap();
    let starved = OptimizationWorkBudget::new(1, 1, steps - 1, 1, 1).unwrap();
    assert_eq!(
        relocate_selected_instruction_into_arm(&source, 0, MOVING, T_HEAD, &environment, starved,)
            .unwrap_err(),
        ForkRelocationError::WorkBudgetExceeded
    );
    // Landing one position deeper crosses the arm head's surface pair.
    let exact = OptimizationWorkBudget::new(1, 1, steps + 2, 1, 1).unwrap();
    relocate_selected_instruction_into_arm(&source, 0, MOVING, T_TAIL, &environment, exact)
        .unwrap();
    let starved = OptimizationWorkBudget::new(1, 1, steps + 1, 1, 1).unwrap();
    assert_eq!(
        relocate_selected_instruction_into_arm(&source, 0, MOVING, T_TAIL, &environment, starved,)
            .unwrap_err(),
        ForkRelocationError::WorkBudgetExceeded
    );
}

/// The environment's target must be the plan's target: a relocation proven
/// for one target's constraints is not evidence on another.
#[test]
fn target_mismatch_rejects() {
    let source = fixture(NativeTarget::linux_x64());
    let environment = baseline_target_register_environment(NativeTarget::linux_arm64()).unwrap();
    assert_eq!(
        relocate(&source, &environment, MOVING, T_HEAD).unwrap_err(),
        ForkRelocationError::SourceMismatch
    );
}

/// Two runs over the identical source produce the identical validated
/// result, and the published plan is a legal second input through the
/// sealed analysis boundary: the tail member sinks through the same fork
/// onto it, a hazard-coupled member still declines, and an in-block family
/// still admits.
#[test]
fn fork_relocation_is_deterministic_and_re_admitted() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let first = relocate(&source, &environment, MOVING, T_HEAD).unwrap();
    let second = relocate(&source, &environment, MOVING, T_HEAD).unwrap();
    assert_eq!(first, second);
    // The validated output carries the sealed analysis boundary, so it is
    // a legal second input — not merely a reconstruction of one. The tail
    // member sinks through the same fork onto it.
    let again =
        relocate_selected_instruction_into_arm(&first, 0, TRAIL, T_TAIL, &environment, budget())
            .unwrap();
    assert_eq!(
        again.transformed().functions[0].blocks[1]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![MOVING, T_HEAD, TRAIL, T_TAIL]
    );
    assert_eq!(
        again.receipt().source_selected(),
        first.receipt().transformed_selected()
    );
    // A hazard-coupled member still declines on the second input: once
    // `MOVING` sits at T's head, `T_HEAD` reads `R_TRAIL` ahead of
    // `T_TAIL`, so `TRAIL` cannot cross it to land on `T_TAIL`'s position.
    let coupled = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[1].instructions[0] = instruction(
            T_HEAD,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[R_TRAIL, R_THEAD],
        );
    });
    let moved = relocate(&coupled, &environment, MOVING, T_HEAD).unwrap();
    assert_eq!(
        relocate_selected_instruction_into_arm(&moved, 0, TRAIL, T_TAIL, &environment, budget(),)
            .unwrap_err(),
        ForkRelocationError::UnsupportedPair
    );
    // A different scheduling family still admits on the second input.
    let swapped =
        crate::relocate_selected_instruction(&first, 0, MID, TAIL, &environment, budget()).unwrap();
    assert_eq!(
        swapped.transformed().functions[0].blocks[3]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![HEAD, TAIL, MID]
    );
}
