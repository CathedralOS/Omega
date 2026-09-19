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
    DiamondRelocationError, DiamondRelocationReceipt, ValidatedDiamondRelocation,
    relocate_selected_instruction_through_diamond, validate_diamond_relocation,
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
/// Block B is the entry and ends in a `ConditionalBranchNonZero` whose
/// nonzero edge leads to block T and whose zero edge leads to block F; each
/// arm's only predecessor is that branch and each ends in a `Jump` to the
/// join block J, whose only predecessors are the two arms and whose
/// terminator is a plain return:
/// `B = [LEAD; MOVING; TRAIL] -> branch -> T = [T_HEAD; T_TAIL] -> jump -> J`
/// and `F = [F_HEAD; F_TAIL] -> jump -> J = [HEAD; MID; TAIL] -> return`.
/// The default move relocates `MOVING` onto `HEAD`'s position — the head of
/// J's body — crossing `TRAIL`, the branch terminator, both branch edges,
/// both arms' bodies, both arm jumps and their edges, while J's body keeps
/// its order behind the landing index.
fn fixture(target: NativeTarget) -> ValidatedDiamondRelocation {
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
    ValidatedDiamondRelocation {
        receipt: DiamondRelocationReceipt {
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
) -> ValidatedDiamondRelocation {
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
    source: &ValidatedDiamondRelocation,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
) -> Result<ValidatedDiamondRelocation, DiamondRelocationError> {
    relocate_selected_instruction_through_diamond(
        source,
        0,
        member,
        destination,
        environment,
        budget(),
    )
}

/// The member crosses the whole diamond onto the destination's position on
/// every target: `MOVING` leaves B's body, `TRAIL`, the branch, both edges,
/// both arms, and both arm jumps keep their positions, and the member
/// lands at J's head with the destination and every later position one
/// slot later in their original order — identity, kind, operands, and
/// provenance intact. The replayed proposal restores the source
/// bit-identically.
#[test]
fn member_relocates_through_the_diamond() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = fixture(target);
        let result = relocate(&source, &environment, MOVING, HEAD).unwrap();
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
            moved.blocks[1].instructions,
            original.blocks[1].instructions
        );
        assert_eq!(
            moved.blocks[2].instructions,
            original.blocks[2].instructions
        );
        assert_eq!(
            moved.blocks[3]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![MOVING, HEAD, MID, TAIL]
        );
        // The member moved bit-identically; every terminator, edge, and
        // roster stayed untouched.
        assert_eq!(
            moved.blocks[3].instructions[0],
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
        validate_diamond_relocation(
            &source,
            0,
            MOVING,
            HEAD,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
    }
}

/// The destination names the landing position directly: a body instruction
/// puts the member on its index, and the join's terminator-carried
/// instruction lands the member at the body end.
#[test]
fn member_lands_at_the_named_position() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let middle = relocate(&source, &environment, MOVING, MID).unwrap();
    assert_eq!(
        middle.transformed().functions[0].blocks[3]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![HEAD, MOVING, MID, TAIL]
    );
    let last_body = relocate(&source, &environment, MOVING, TAIL).unwrap();
    assert_eq!(
        last_body.transformed().functions[0].blocks[3]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![HEAD, MID, MOVING, TAIL]
    );
    let body_end = relocate(&source, &environment, MOVING, RET).unwrap();
    assert_eq!(
        body_end.transformed().functions[0].blocks[3]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![HEAD, MID, TAIL, MOVING]
    );
}

/// Every body index relocates: the block-tail member crosses only the
/// branch, both edges, and both arms, while the block-head member crosses
/// its whole trailing body first.
#[test]
fn tail_and_head_members_relocate() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let tail = relocate(&source, &environment, TRAIL, HEAD).unwrap();
    assert_eq!(
        tail.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![LEAD, MOVING]
    );
    assert_eq!(
        tail.transformed().functions[0].blocks[3]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![TRAIL, HEAD, MID, TAIL]
    );
    let head = relocate(&source, &environment, LEAD, HEAD).unwrap();
    assert_eq!(
        head.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![MOVING, TRAIL]
    );
    assert_eq!(
        head.transformed().functions[0].blocks[3]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![LEAD, HEAD, MID, TAIL]
    );
}

/// Both branch edges naming the same arm collapse the diamond to a single
/// arm the branch alone still reaches: the member crosses it once.
#[test]
fn degenerate_single_arm_diamond_relocates() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        let terminator = match &mut function.blocks[0].terminator {
            SelectedTerminator::ConditionalBranch { when_zero, .. } => when_zero,
            _ => unreachable!(),
        };
        *terminator = successor(BLOCK_T, BlockId::new(2).unwrap(), EDGE_BF);
        function.blocks.remove(2);
    });
    let result = relocate(&source, &environment, MOVING, HEAD).unwrap();
    let moved = &result.transformed().functions[0];
    assert_eq!(moved.blocks.len(), 3);
    assert_eq!(
        moved.blocks[2]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![MOVING, HEAD, MID, TAIL]
    );
}

/// A member whose write feeds a crossed read keeps order: the crossed
/// `TRAIL` reading the member's result refuses, a crossed arm instruction
/// reading it refuses on either side of the branch, and a crossed `HEAD`
/// reading it refuses only once the window actually reaches it.
#[test]
fn raw_hazard_keeps_order_through_the_diamond() {
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
        relocate(&tail_reads, &environment, MOVING, HEAD).unwrap_err(),
        DiamondRelocationError::UnsupportedPair
    );
    for arm in [1usize, 2] {
        let arm_reads = mutated(target, |function, environment| {
            let copy = environment
                .constraint(environment.selected_keys().copy_i64)
                .unwrap()
                .clone();
            function.blocks[arm].instructions[0] = instruction(
                function.blocks[arm].instructions[0].id,
                SelectedInstructionKind::CopyI64,
                &copy,
                &[R_MOVE, VirtualRegisterId(30)],
            );
        });
        assert_eq!(
            relocate(&arm_reads, &environment, MOVING, HEAD).unwrap_err(),
            DiamondRelocationError::UnsupportedPair,
            "arm {arm}"
        );
    }
    let head_reads = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[3].instructions[0] = instruction(
            HEAD,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[R_MOVE, R_HEAD],
        );
    });
    // Landing at the head does not cross `HEAD`; landing at `MID` crosses
    // it and the member's write would starve the crossed read.
    relocate(&head_reads, &environment, MOVING, HEAD).unwrap();
    assert_eq!(
        relocate(&head_reads, &environment, MOVING, MID).unwrap_err(),
        DiamondRelocationError::UnsupportedPair
    );
}

/// A member reading a location a crossed instruction writes would observe
/// the new value after the move — in an arm the write is always crossed,
/// and in the join it is crossed once the landing passes it.
#[test]
fn war_hazard_keeps_order_through_the_diamond() {
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
    assert_eq!(
        relocate(&arm_writes, &environment, MOVING, HEAD).unwrap_err(),
        DiamondRelocationError::UnsupportedPair
    );
    let join_writes = member_reads_pointer(&mut |function, environment| {
        let materialize = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap()
            .clone();
        function.blocks[3].instructions[0] = instruction(
            HEAD,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(9),
            },
            &materialize,
            &[POINTER],
        );
    });
    relocate(&join_writes, &environment, MOVING, HEAD).unwrap();
    assert_eq!(
        relocate(&join_writes, &environment, MOVING, MID).unwrap_err(),
        DiamondRelocationError::UnsupportedPair
    );
}

/// A member and a crossed instruction writing the same register would
/// change which definition later positions observe.
#[test]
fn waw_hazard_keeps_order_through_the_diamond() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let arm_writes = mutated(target, |function, environment| {
        let materialize = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap()
            .clone();
        function.blocks[2].instructions[1] = instruction(
            F_TAIL,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(9),
            },
            &materialize,
            &[R_MOVE],
        );
    });
    assert_eq!(
        relocate(&arm_writes, &environment, MOVING, HEAD).unwrap_err(),
        DiamondRelocationError::UnsupportedPair
    );
    let join_writes = mutated(target, |function, environment| {
        let materialize = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap()
            .clone();
        function.blocks[3].instructions[0] = instruction(
            HEAD,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(9),
            },
            &materialize,
            &[R_MOVE],
        );
    });
    assert_eq!(
        relocate(&join_writes, &environment, MOVING, MID).unwrap_err(),
        DiamondRelocationError::UnsupportedPair
    );
    relocate(&join_writes, &environment, MOVING, HEAD).unwrap();
}

/// Condition state couples like registers through the diamond: the branch
/// terminator reads the target's condition units, so a flag-publishing
/// member can never leave; a flag-reading member cannot cross a flag
/// writer in its own tail or inside an arm; and a flag consumer sitting in
/// an arm does not block a flag-inert member.
#[test]
fn condition_state_couples_through_the_diamond() {
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
        relocate(&flag_writer, &environment, MOVING, HEAD).unwrap_err(),
        DiamondRelocationError::UnsupportedPair
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
        relocate(&tail_writes_flags, &environment, MOVING, HEAD).unwrap_err(),
        DiamondRelocationError::UnsupportedPair
    );
    // The same coupling holds for a compare publishing flags inside an arm.
    let arm_writes_flags = member_reads_flags(&mut |function, environment| {
        let compare = environment
            .constraint(environment.selected_keys().compare_i64)
            .unwrap()
            .clone();
        function.blocks[2].instructions[0] = instruction(
            F_HEAD,
            SelectedInstructionKind::CompareI64,
            &compare,
            &[R_FHEAD, R_FTAIL],
        );
    });
    assert_eq!(
        relocate(&arm_writes_flags, &environment, MOVING, HEAD).unwrap_err(),
        DiamondRelocationError::UnsupportedPair
    );
    // A flag consumer in an arm reads the units the branch observed; a
    // flag-inert member does not disturb them and still crosses.
    let flag_consumer = mutated(target, |function, environment| {
        let boolean = environment
            .constraint(environment.selected_keys().materialize_boolean)
            .unwrap()
            .clone();
        function.blocks[1].instructions[0] = instruction(
            T_HEAD,
            SelectedInstructionKind::MaterializeBooleanEqual,
            &boolean,
            &[R_THEAD],
        );
    });
    relocate(&flag_consumer, &environment, MOVING, HEAD).unwrap();
}

/// The branch and arm-jump terminators are crossed positions, not window
/// barriers: a register the branch reads couples like any crossed read,
/// while a read it shares with the member admits, and an arm's `Jump`
/// reading the member's result refuses.
#[test]
fn terminator_positions_couple() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The branch terminator reads the member's result: the member's write
    // would land after the read.
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
        relocate(&terminator_reads, &environment, MOVING, HEAD).unwrap_err(),
        DiamondRelocationError::UnsupportedPair
    );
    // The branch reading only the entry parameter the member also reads
    // shares no written location: the member still crosses.
    let shared_read = mutated(target, |function, environment| {
        let branch_row = environment
            .constraint(environment.selected_keys().conditional_branch)
            .unwrap()
            .clone();
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
        let class = function.blocks[0].instructions[0].operands[0].class;
        let mut branch = instruction(
            BRANCH,
            SelectedInstructionKind::ConditionalBranchNonZero,
            &branch_row,
            &[],
        );
        branch.operands.push(SelectedOperand {
            operand: 0,
            virtual_register: POINTER,
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
    relocate(&shared_read, &environment, MOVING, HEAD).unwrap();
    // An arm's `Jump` reading the member's result refuses: the member
    // would land after the read.
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
    assert_eq!(
        relocate(&arm_terminator_reads, &environment, MOVING, HEAD).unwrap_err(),
        DiamondRelocationError::UnsupportedPair
    );
}

/// Every crossed edge's register transports sit between the member's old
/// and new positions — both branch edges and each arm's jump edge: a
/// member defining the transported argument would hand the binding a stale
/// value, a member defining or reading the parameter would be overwritten
/// or observe the transported value, while a member merely reading the
/// argument crosses freely.
#[test]
fn register_transports_bind_each_crossed_edge() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let binding = || SelectedValueBinding {
        semantic: abstract_operations::ValueBinding {
            parameter: ValueId::new(20).unwrap(),
            argument: ValueId::new(1).unwrap(),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        },
        transport: SelectedValueTransport::Registers {
            argument: POINTER,
            parameter: R_BOUND,
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
    // A binding on a branch edge audits the member the same way the
    // single-edge family's did.
    let on_branch_edge = |edit: &mut dyn FnMut(
        &mut SelectedFunction,
        &register_environment::ValidatedTargetRegisterEnvironment,
    )| {
        mutated(target, |function, environment| {
            let successor = match &mut function.blocks[0].terminator {
                SelectedTerminator::ConditionalBranch { when_nonzero, .. } => when_nonzero,
                _ => unreachable!(),
            };
            successor.bindings.push(binding());
            edit(function, environment);
        })
    };
    relocate(&on_branch_edge(&mut |_, _| {}), &environment, MOVING, HEAD).unwrap();
    relocate(
        &on_branch_edge(&mut member_reads(POINTER)),
        &environment,
        MOVING,
        HEAD,
    )
    .unwrap();
    assert_eq!(
        relocate(
            &on_branch_edge(&mut member_writes(POINTER)),
            &environment,
            MOVING,
            HEAD,
        )
        .unwrap_err(),
        DiamondRelocationError::UnsupportedPair
    );
    assert_eq!(
        relocate(
            &on_branch_edge(&mut member_writes(R_BOUND)),
            &environment,
            MOVING,
            HEAD,
        )
        .unwrap_err(),
        DiamondRelocationError::UnsupportedPair
    );
    assert_eq!(
        relocate(
            &on_branch_edge(&mut member_reads(R_BOUND)),
            &environment,
            MOVING,
            HEAD,
        )
        .unwrap_err(),
        DiamondRelocationError::UnsupportedPair
    );
    // A binding on an arm's jump edge sits inside the window too.
    let on_arm_edge = |edit: &mut dyn FnMut(
        &mut SelectedFunction,
        &register_environment::ValidatedTargetRegisterEnvironment,
    )| {
        mutated(target, |function, environment| {
            let successor = match &mut function.blocks[1].terminator {
                SelectedTerminator::Jump { successor, .. } => successor,
                _ => unreachable!(),
            };
            successor.bindings.push(binding());
            edit(function, environment);
        })
    };
    relocate(&on_arm_edge(&mut |_, _| {}), &environment, MOVING, HEAD).unwrap();
    assert_eq!(
        relocate(
            &on_arm_edge(&mut member_writes(POINTER)),
            &environment,
            MOVING,
            HEAD,
        )
        .unwrap_err(),
        DiamondRelocationError::UnsupportedPair
    );
    assert_eq!(
        relocate(
            &on_arm_edge(&mut member_reads(R_BOUND)),
            &environment,
            MOVING,
            HEAD,
        )
        .unwrap_err(),
        DiamondRelocationError::UnsupportedPair
    );
}

/// The diamond must actually close: a non-branch terminator is the
/// single-edge family's case or no move at all, each arm must be a plain
/// source block the branch alone reaches and must end in a `Jump` to the
/// one join, and the join must be a plain source block the arms alone
/// reach.
#[test]
fn the_diamond_must_close() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A `Jump` terminator is the single-edge family's window, not a
    // diamond.
    let jump_only = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        function.blocks[0].terminator = jump_terminator(
            instruction(BRANCH, SelectedInstructionKind::Jump, &jump_row, &[]),
            successor(BLOCK_J, BlockId::new(4).unwrap(), EDGE_BT),
        );
    });
    assert_eq!(
        relocate(&jump_only, &environment, MOVING, HEAD).unwrap_err(),
        DiamondRelocationError::UnsupportedPair
    );
    // A return terminator has no successor to land on.
    let returned = mutated(target, |function, environment| {
        let return_row = environment
            .constraint(environment.selected_keys().return_unit)
            .unwrap()
            .clone();
        function.blocks[0].terminator = SelectedTerminator::Return {
            instruction: instruction(
                BRANCH,
                SelectedInstructionKind::ReturnUnit,
                &return_row,
                &[],
            ),
            psi_return_edge: EdgeId::new(EDGE_BT).unwrap(),
        };
    });
    assert_eq!(
        relocate(&returned, &environment, MOVING, HEAD).unwrap_err(),
        DiamondRelocationError::UnsupportedPair
    );
    // An arm ending in a second conditional is not the bounded jump edge.
    let arm_branches = mutated(target, |function, environment| {
        let branch_row = environment
            .constraint(environment.selected_keys().conditional_branch)
            .unwrap()
            .clone();
        function.blocks[1].terminator = SelectedTerminator::ConditionalBranch {
            instruction: instruction(
                T_JUMP,
                SelectedInstructionKind::ConditionalBranchNonZero,
                &branch_row,
                &[],
            ),
            when_nonzero: successor(BLOCK_J, BlockId::new(4).unwrap(), EDGE_TJ),
            when_zero: successor(BLOCK_J, BlockId::new(4).unwrap(), 25),
        };
    });
    assert_eq!(
        relocate(&arm_branches, &environment, MOVING, HEAD).unwrap_err(),
        DiamondRelocationError::UnsupportedPair
    );
    // The arms must converge on one join: an arm jumping anywhere else
    // leaves two exits this step does not cross.
    let diverged = mutated(target, |function, environment| {
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(4),
            origin: SelectedBlockOrigin::Source(BlockId::new(5).unwrap()),
            instructions: Vec::new(),
            terminator: SelectedTerminator::Return {
                instruction: instruction(
                    SelectedInstructionId(30),
                    SelectedInstructionKind::ReturnUnit,
                    &environment
                        .constraint(environment.selected_keys().return_unit)
                        .unwrap()
                        .clone(),
                    &[],
                ),
                psi_return_edge: EdgeId::new(26).unwrap(),
            },
        });
        let successor = match &mut function.blocks[2].terminator {
            SelectedTerminator::Jump { successor, .. } => successor,
            _ => unreachable!(),
        };
        successor.block = SelectedBlockId(4);
    });
    assert_eq!(
        relocate(&diverged, &environment, MOVING, HEAD).unwrap_err(),
        DiamondRelocationError::UnsupportedPair
    );
    // A second predecessor into an arm hands the join a path the member
    // never executed on.
    let arm_two_predecessors = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(4),
            origin: SelectedBlockOrigin::Source(BlockId::new(5).unwrap()),
            instructions: Vec::new(),
            terminator: jump_terminator(
                instruction(
                    SelectedInstructionId(30),
                    SelectedInstructionKind::Jump,
                    &jump_row,
                    &[],
                ),
                successor(BLOCK_T, BlockId::new(2).unwrap(), 26),
            ),
        });
    });
    assert_eq!(
        relocate(&arm_two_predecessors, &environment, MOVING, HEAD).unwrap_err(),
        DiamondRelocationError::UnsupportedPair
    );
    // A second predecessor into the join does the same directly.
    let join_two_predecessors = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(4),
            origin: SelectedBlockOrigin::Source(BlockId::new(5).unwrap()),
            instructions: Vec::new(),
            terminator: jump_terminator(
                instruction(
                    SelectedInstructionId(30),
                    SelectedInstructionKind::Jump,
                    &jump_row,
                    &[],
                ),
                successor(BLOCK_J, BlockId::new(4).unwrap(), 26),
            ),
        });
    });
    assert_eq!(
        relocate(&join_two_predecessors, &environment, MOVING, HEAD).unwrap_err(),
        DiamondRelocationError::UnsupportedPair
    );
    // An arm that is the member's own block loops the move back.
    let self_arm = mutated(target, |function, _| {
        let successor = match &mut function.blocks[0].terminator {
            SelectedTerminator::ConditionalBranch { when_nonzero, .. } => when_nonzero,
            _ => unreachable!(),
        };
        successor.block = BLOCK_B;
    });
    assert_eq!(
        relocate(&self_arm, &environment, MOVING, HEAD).unwrap_err(),
        DiamondRelocationError::UnsupportedPair
    );
    // The join as the member's own block is an in-block move through a
    // back-edge pair, not a diamond.
    let join_is_self = mutated(target, |function, _| {
        for index in [1usize, 2] {
            let successor = match &mut function.blocks[index].terminator {
                SelectedTerminator::Jump { successor, .. } => successor,
                _ => unreachable!(),
            };
            successor.block = BLOCK_B;
        }
    });
    assert_eq!(
        relocate(&join_is_self, &environment, MOVING, HEAD).unwrap_err(),
        DiamondRelocationError::UnsupportedPair
    );
    // The entry block is never an arm or the join: both carry no
    // predecessor at all.
    let entry_arm = mutated(target, |function, _| {
        function.entry_block = BLOCK_T;
    });
    assert_eq!(
        relocate(&entry_arm, &environment, MOVING, HEAD).unwrap_err(),
        DiamondRelocationError::UnsupportedPair
    );
    let entry_join = mutated(target, |function, _| {
        function.entry_block = BLOCK_J;
    });
    assert_eq!(
        relocate(&entry_join, &environment, MOVING, HEAD).unwrap_err(),
        DiamondRelocationError::UnsupportedPair
    );
    // Implementation blocks carry edge or case work the bounded audit
    // does not cross — as an arm or as the join.
    for origin in [
        SelectedBlockOrigin::EdgeTransfer {
            edge: EdgeId::new(EDGE_TJ).unwrap(),
            target: BlockId::new(2).unwrap(),
        },
        SelectedBlockOrigin::CaseDispatch {
            source: BlockId::new(2).unwrap(),
            case_ordinal: 0,
        },
    ] {
        let implementation_arm = mutated(target, |function, _| {
            function.blocks[1].origin = origin;
        });
        assert_eq!(
            relocate(&implementation_arm, &environment, MOVING, HEAD).unwrap_err(),
            DiamondRelocationError::UnsupportedPair
        );
        let implementation_join = mutated(target, |function, _| {
            function.blocks[3].origin = origin;
        });
        assert_eq!(
            relocate(&implementation_join, &environment, MOVING, HEAD).unwrap_err(),
            DiamondRelocationError::UnsupportedPair
        );
    }
}

/// Only plain semantic edges carry the member: continuation roles, case
/// custody, edge fuel, and a live structural transport are all boundary
/// work the window does not cross — on the branch edges and on each arm's
/// jump edge alike.
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
                SelectedTerminator::ConditionalBranch { when_zero, .. } => when_zero,
                _ => unreachable!(),
            };
            successor.role = role;
        });
        assert_eq!(
            relocate(&continued, &environment, MOVING, HEAD).unwrap_err(),
            DiamondRelocationError::UnsupportedPair
        );
        let continued_arm = mutated(target, |function, _| {
            let successor = match &mut function.blocks[1].terminator {
                SelectedTerminator::Jump { successor, .. } => successor,
                _ => unreachable!(),
            };
            successor.role = role;
        });
        assert_eq!(
            relocate(&continued_arm, &environment, MOVING, HEAD).unwrap_err(),
            DiamondRelocationError::UnsupportedPair
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
        relocate(&case_edge, &environment, MOVING, HEAD).unwrap_err(),
        DiamondRelocationError::UnsupportedPair
    );
    let fueled = mutated(target, |function, _| {
        let successor = match &mut function.blocks[2].terminator {
            SelectedTerminator::Jump { successor, .. } => successor,
            _ => unreachable!(),
        };
        successor.fuel.push(optimization_unit::FuelSettlement {
            site: optimization_unit::PsiProvenance::Operation(OperationId::new(30).unwrap()),
            units: 1,
        });
    });
    assert_eq!(
        relocate(&fueled, &environment, MOVING, HEAD).unwrap_err(),
        DiamondRelocationError::UnsupportedPair
    );
    // A live structural transport moves a stored value at the boundary.
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
        relocate(&structural, &environment, MOVING, HEAD).unwrap_err(),
        DiamondRelocationError::UnsupportedPair
    );
    // An `Unused` structural transport moves nothing and admits.
    let inert_structural = mutated(target, |function, _| {
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
                transport: SelectedStructuralTransport::Unused,
            });
    });
    relocate(&inert_structural, &environment, MOVING, HEAD).unwrap();
}

/// Calls, hosted effects, and terminator kinds are barriers as the member
/// or anywhere inside the crossed window — both arms' bodies included —
/// and a call-roster row makes an instruction a barrier even when its kind
/// is register-pure; the branch and arm-jump terminator ids included.
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
            relocate(&member_barrier, &environment, MOVING, HEAD).unwrap_err(),
            DiamondRelocationError::UnsupportedInstruction,
            "member {kind:?}"
        );
        let tail_barrier = mutated(target, |function, _| {
            function.blocks[0].instructions[2].kind = kind;
        });
        assert_eq!(
            relocate(&tail_barrier, &environment, MOVING, HEAD).unwrap_err(),
            DiamondRelocationError::UnsupportedInstruction,
            "crossed tail {kind:?}"
        );
        let arm_barrier = mutated(target, |function, _| {
            function.blocks[1].instructions[0].kind = kind;
        });
        assert_eq!(
            relocate(&arm_barrier, &environment, MOVING, HEAD).unwrap_err(),
            DiamondRelocationError::UnsupportedInstruction,
            "crossed arm {kind:?}"
        );
        let head_barrier = mutated(target, |function, _| {
            function.blocks[3].instructions[0].kind = kind;
        });
        assert_eq!(
            relocate(&head_barrier, &environment, MOVING, MID).unwrap_err(),
            DiamondRelocationError::UnsupportedInstruction,
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
        (MOVING, HEAD),
        (TRAIL, HEAD),
        (T_HEAD, HEAD),
        (HEAD, MID),
        (BRANCH, HEAD),
        (T_JUMP, HEAD),
    ] {
        let contract = mutated(target, |function, _| {
            function.calls.push(call_contract(instruction_id));
        });
        assert_eq!(
            relocate(&contract, &environment, MOVING, destination).unwrap_err(),
            DiamondRelocationError::UnsupportedInstruction,
            "call roster {instruction_id:?}"
        );
    }
}

/// A roster-carrying member may cross only row-less positions — each
/// terminator and each crossed edge's own recorded accesses included —
/// while a row-less member crosses any accounted mix because no recorded
/// access changes order.
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
        };
    // The roster-carrying member crosses row-less positions: the tail
    // materialization, the branch, both edges, both arms, both arm jumps,
    // and their edges.
    relocate(
        &mutated(target, |function, environment| {
            load_member(function, environment)
        }),
        &environment,
        MOVING,
        HEAD,
    )
    .unwrap();
    // A second accounted actor anywhere in the window refuses: the
    // member's recorded read would trade order with the crossed write.
    for block_index in [0usize, 1, 2, 3] {
        let accounted = mutated(target, |function, environment| {
            load_member(function, environment);
            let store = environment
                .constraint(environment.selected_keys().store.unwrap())
                .unwrap()
                .clone();
            let index = match block_index {
                0 => 2,
                1 | 2 => 1,
                _ => 0,
            };
            let id = function.blocks[block_index].instructions[index].id;
            function.blocks[block_index].instructions[index] = instruction(
                id,
                SelectedInstructionKind::Store {
                    byte_offset: 0,
                    byte_size: 8,
                },
                &store,
                &[POINTER, R_TRAIL],
            );
            function.memory_accesses.push(access(
                id,
                PlaceId::new(2).unwrap(),
                SelectedMemoryAccessRole::WritePlace,
            ));
        });
        let destination = if block_index == 3 { MID } else { HEAD };
        assert_eq!(
            relocate(&accounted, &environment, MOVING, destination).unwrap_err(),
            DiamondRelocationError::UnsupportedPair,
            "accounted position in block {block_index}"
        );
    }
    // The same accounted position at the join head is not crossed when the
    // member lands on it.
    let accounted_head = mutated(target, |function, environment| {
        load_member(function, environment);
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
        function.blocks[3].instructions[0] = instruction(
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
    relocate(&accounted_head, &environment, MOVING, HEAD).unwrap();
    // A row recorded against any crossed edge counts as that edge
    // position's memory surface.
    for edge in [EDGE_BT, EDGE_BF, EDGE_TJ, EDGE_FJ] {
        let accounted_edge = mutated(target, |function, environment| {
            load_member(function, environment);
            function.memory_accesses.push(edge_access(RET, edge));
        });
        assert_eq!(
            relocate(&accounted_edge, &environment, MOVING, HEAD).unwrap_err(),
            DiamondRelocationError::UnsupportedPair,
            "edge {edge}"
        );
    }
    // A row recorded against a crossed terminator is that boundary
    // position's memory surface.
    for terminator in [BRANCH, T_JUMP, F_JUMP] {
        let accounted_terminator = mutated(target, |function, environment| {
            load_member(function, environment);
            function.memory_accesses.push(access(
                terminator,
                PlaceId::new(2).unwrap(),
                SelectedMemoryAccessRole::WritePlace,
            ));
        });
        assert_eq!(
            relocate(&accounted_terminator, &environment, MOVING, HEAD).unwrap_err(),
            DiamondRelocationError::UnsupportedPair,
            "terminator {terminator:?}"
        );
    }
    // A row on another edge is not crossed: it names a position outside
    // the window.
    let foreign_edge = mutated(target, |function, environment| {
        load_member(function, environment);
        function.memory_accesses.push(edge_access(TAIL, 99));
    });
    relocate(&foreign_edge, &environment, MOVING, HEAD).unwrap();
    // A row-less member crosses any accounted mix.
    let rowless = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
        function.blocks[1].instructions[0] = instruction(
            T_HEAD,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            &store,
            &[POINTER, R_THEAD],
        );
        function.blocks[3].instructions[0] = instruction(
            HEAD,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            &store,
            &[POINTER, R_HEAD],
        );
        function.memory_accesses.push(access(
            T_HEAD,
            PlaceId::new(1).unwrap(),
            SelectedMemoryAccessRole::WritePlace,
        ));
        function.memory_accesses.push(access(
            HEAD,
            PlaceId::new(2).unwrap(),
            SelectedMemoryAccessRole::WritePlace,
        ));
    });
    let result = relocate(&rowless, &environment, MOVING, MID).unwrap();
    assert_eq!(
        result.transformed().functions[0].memory_accesses,
        rowless.transformed().functions[0].memory_accesses
    );
    // A memory-capable kind without a roster row is an unaccounted access:
    // the relocation cannot prove what it reaches, so it refuses outright
    // whether the bare access is the member or a crossed position.
    let bare_member = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
        function.blocks[0].instructions[1] = instruction(
            MOVING,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            &store,
            &[POINTER, R_MOVE],
        );
    });
    assert_eq!(
        relocate(&bare_member, &environment, MOVING, HEAD).unwrap_err(),
        DiamondRelocationError::UnsupportedInstruction
    );
    let bare_arm = mutated(target, |function, environment| {
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
    });
    assert_eq!(
        relocate(&bare_arm, &environment, MOVING, HEAD).unwrap_err(),
        DiamondRelocationError::UnsupportedInstruction
    );
}

/// A boundary settlement positioned past the member's index observed it
/// inside the source block's executed prefix, and one positioned past the
/// landing index observes it inside the join's — both refuse, while
/// positions at or before either boundary keep the executed set they
/// always had. A settlement inside an arm never observed the member: the
/// member never enters an arm's body, so every arm prefix is unchanged.
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
        let result = relocate(&settled, &environment, MOVING, HEAD);
        assert_eq!(
            result.is_ok(),
            admits,
            "source-block settlement at {position}"
        );
    }
    // The member never enters an arm's body: no arm prefix ever contained
    // or loses it, so a settlement anywhere in an arm admits.
    for position in [0u32, 1, 2] {
        let settled = mutated(target, |function, _| {
            function
                .boundary_settlements
                .push(settlement(BLOCK_T, position, 51));
        });
        relocate(&settled, &environment, MOVING, HEAD).unwrap();
    }
    // In the join block the bound is the landing index: at or before it
    // the executed prefix is unchanged; past it the member joins the
    // prefix.
    for (position, admits) in [(0u32, true), (1, true), (2, false), (3, false)] {
        let settled = mutated(target, |function, _| {
            function
                .boundary_settlements
                .push(settlement(BLOCK_J, position, 52));
        });
        let result = relocate(&settled, &environment, MOVING, MID);
        assert_eq!(
            result.is_ok(),
            admits,
            "join-block settlement at {position}"
        );
    }
    // Landing at the body end keeps every join settlement: none sits past
    // the member's new index.
    let settled = mutated(target, |function, _| {
        function
            .boundary_settlements
            .push(settlement(BLOCK_J, 3, 53));
    });
    relocate(&settled, &environment, MOVING, RET).unwrap();
    // A settlement in an untouched block never observes the move.
    let elsewhere = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(4),
            origin: SelectedBlockOrigin::Source(BlockId::new(5).unwrap()),
            instructions: Vec::new(),
            terminator: jump_terminator(
                instruction(
                    SelectedInstructionId(30),
                    SelectedInstructionKind::Jump,
                    &jump_row,
                    &[],
                ),
                successor(BLOCK_B, BlockId::new(1).unwrap(), 26),
            ),
        });
        function
            .boundary_settlements
            .push(settlement(SelectedBlockId(4), 0, 54));
    });
    relocate(&elsewhere, &environment, MOVING, HEAD).unwrap();
}

/// The pair must name one member in the branch block's body and one
/// position in the join: an unknown member, a wrong function, a
/// destination outside the join, a member outside the branch block, and a
/// terminator id as member all refuse.
#[test]
fn only_the_named_diamond_window_relocates() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // An unknown member or function index never locates the window.
    assert_eq!(
        relocate_selected_instruction_through_diamond(
            &source,
            0,
            SelectedInstructionId(99),
            HEAD,
            &environment,
            budget(),
        )
        .unwrap_err(),
        DiamondRelocationError::SourceMismatch
    );
    assert_eq!(
        relocate_selected_instruction_through_diamond(
            &source,
            9,
            MOVING,
            HEAD,
            &environment,
            budget(),
        )
        .unwrap_err(),
        DiamondRelocationError::SourceMismatch
    );
    // The destination must name a position in the join — a member of the
    // member's own block, an arm position, and a dangling id all refuse.
    for destination in [LEAD, T_HEAD, SelectedInstructionId(99)] {
        assert_eq!(
            relocate(&source, &environment, MOVING, destination).unwrap_err(),
            DiamondRelocationError::UnsupportedPair
        );
    }
    // A body member of a block without the branch names no window: the
    // join's own members have only a return terminator.
    assert_eq!(
        relocate(&source, &environment, HEAD, TAIL).unwrap_err(),
        DiamondRelocationError::UnsupportedPair
    );
    // An arm's members sit behind a conditional edge from the branch but
    // their own blocks end in `Jump`, not the two-successor form.
    assert_eq!(
        relocate(&source, &environment, T_HEAD, MID).unwrap_err(),
        DiamondRelocationError::UnsupportedPair
    );
    // Naming the branch block's terminator as member is not a body
    // position.
    assert_eq!(
        relocate(&source, &environment, BRANCH, HEAD).unwrap_err(),
        DiamondRelocationError::SourceMismatch
    );
    // Naming the source block's terminator as destination names a position
    // in the wrong block.
    assert_eq!(
        relocate(&source, &environment, MOVING, BRANCH).unwrap_err(),
        DiamondRelocationError::UnsupportedPair
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
    let result = relocate(&source, &environment, MOVING, MID).unwrap();
    // The honest proposal replays.
    validate_diamond_relocation(
        &source,
        0,
        MOVING,
        MID,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // The member at the wrong index rejects.
    let mut displaced = result.transformed().clone();
    let member = displaced.functions[0].blocks[3].instructions.remove(1);
    displaced.functions[0].blocks[3]
        .instructions
        .insert(3, member.clone());
    assert_eq!(
        validate_diamond_relocation(&source, 0, MOVING, MID, &environment, budget(), displaced)
            .unwrap_err(),
        DiamondRelocationError::ReplayMismatch
    );
    // The member left in its own block rejects.
    let mut unmoved = result.transformed().clone();
    unmoved.functions[0].blocks[3].instructions.remove(1);
    unmoved.functions[0].blocks[0]
        .instructions
        .insert(2, member);
    assert_eq!(
        validate_diamond_relocation(&source, 0, MOVING, MID, &environment, budget(), unmoved)
            .unwrap_err(),
        DiamondRelocationError::ReplayMismatch
    );
    // A dropped instruction in a crossed arm rejects.
    let mut dropped = result.transformed().clone();
    dropped.functions[0].blocks[1].instructions.pop();
    assert_eq!(
        validate_diamond_relocation(&source, 0, MOVING, MID, &environment, budget(), dropped)
            .unwrap_err(),
        DiamondRelocationError::ReplayMismatch
    );
    // An unrelated literal edit inside the source block rejects.
    let mut edited = result.transformed().clone();
    edited.functions[0].blocks[0].instructions[0].kind = SelectedInstructionKind::MaterializeI64 {
        value: IntegerValue::Unsigned(12),
    };
    assert_eq!(
        validate_diamond_relocation(&source, 0, MOVING, MID, &environment, budget(), edited)
            .unwrap_err(),
        DiamondRelocationError::ReplayMismatch
    );
    // Naming a different window on the same proposal re-derives a
    // different landing and rejects.
    assert_eq!(
        validate_diamond_relocation(
            &source,
            0,
            MOVING,
            TAIL,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap_err(),
        DiamondRelocationError::ReplayMismatch
    );
}

/// The environment's target must be the plan's target: a relocation proven
/// for one target's constraints is not evidence on another.
#[test]
fn target_mismatch_rejects() {
    let source = fixture(NativeTarget::linux_x64());
    let environment = baseline_target_register_environment(NativeTarget::linux_arm64()).unwrap();
    assert_eq!(
        relocate(&source, &environment, MOVING, HEAD).unwrap_err(),
        DiamondRelocationError::SourceMismatch
    );
}

/// The bounded audit is measured: the full-diamond window prices every
/// scan, crossed-surface pair, and roster row against the work budget, and
/// a budget one step short refuses rather than skimping. The head landing
/// crosses the member's own tail, the branch with its two edges, and both
/// arms with their terminators and edges — twenty crossed-surface pairs —
/// and naming `MID` lands the member one position deeper, adding the join
/// head's pair.
#[test]
fn measured_validation_step_boundary() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // Each block contributes its body plus its terminator once to the
    // whole-function scan and once to this function's blocks: 14 + 14.
    // The crossed surfaces pair the member (1) against TRAIL (1), both arm
    // bodies (1 each), both arm `Jump` terminators (1 use + 1 definition
    // each on x86-64), and the branch terminator (2 uses + 1 definition):
    // 2+2+2+3+2+2+3+4 = 20 steps.
    let steps: u64 = 14 /* whole plan */ + 14 /* this function's blocks */ + 20;
    let exact = OptimizationWorkBudget::new(1, 1, steps, 1, 1).unwrap();
    relocate_selected_instruction_through_diamond(&source, 0, MOVING, HEAD, &environment, exact)
        .unwrap();
    let starved = OptimizationWorkBudget::new(1, 1, steps - 1, 1, 1).unwrap();
    assert_eq!(
        relocate_selected_instruction_through_diamond(
            &source,
            0,
            MOVING,
            HEAD,
            &environment,
            starved,
        )
        .unwrap_err(),
        DiamondRelocationError::WorkBudgetExceeded
    );
    // Landing at `MID` crosses one more surface pair — the join head.
    let exact = OptimizationWorkBudget::new(1, 1, steps + 2, 1, 1).unwrap();
    relocate_selected_instruction_through_diamond(&source, 0, MOVING, MID, &environment, exact)
        .unwrap();
    let starved = OptimizationWorkBudget::new(1, 1, steps + 1, 1, 1).unwrap();
    assert_eq!(
        relocate_selected_instruction_through_diamond(
            &source,
            0,
            MOVING,
            MID,
            &environment,
            starved,
        )
        .unwrap_err(),
        DiamondRelocationError::WorkBudgetExceeded
    );
}

/// Two runs over the identical source produce the identical validated
/// result, and the published plan is a legal second input through the
/// sealed analysis boundary: the tail member crosses the same diamond onto
/// it, a hazard-coupled member still declines, and an in-block family
/// still admits.
#[test]
fn diamond_relocation_is_deterministic_and_re_admitted() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let first = relocate(&source, &environment, MOVING, HEAD).unwrap();
    let second = relocate(&source, &environment, MOVING, HEAD).unwrap();
    assert_eq!(first, second);
    // The validated output carries the sealed analysis boundary, so it is
    // a legal second input — not merely a reconstruction of one. The tail
    // member crosses the same diamond onto it.
    let again = relocate_selected_instruction_through_diamond(
        &first,
        0,
        TRAIL,
        HEAD,
        &environment,
        budget(),
    )
    .unwrap();
    assert_eq!(
        again.transformed().functions[0].blocks[3]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![MOVING, TRAIL, HEAD, MID, TAIL]
    );
    assert_eq!(
        again.receipt().source_selected(),
        first.receipt().transformed_selected()
    );
    // A hazard-coupled member still declines on the second input: once
    // `MOVING` sits at J's head, `HEAD` reads `R_TRAIL` ahead of `MID`, so
    // `TRAIL` cannot cross it to land on `MID`'s position.
    let coupled = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[3].instructions[0] = instruction(
            HEAD,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[R_TRAIL, R_HEAD],
        );
    });
    let moved = relocate(&coupled, &environment, MOVING, HEAD).unwrap();
    assert_eq!(
        relocate_selected_instruction_through_diamond(
            &moved,
            0,
            TRAIL,
            MID,
            &environment,
            budget(),
        )
        .unwrap_err(),
        DiamondRelocationError::UnsupportedPair
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
        vec![MOVING, HEAD, TAIL, MID]
    );
}
