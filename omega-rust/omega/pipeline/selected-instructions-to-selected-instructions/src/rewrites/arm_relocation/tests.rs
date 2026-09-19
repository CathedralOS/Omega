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
    ArmRelocationError, ArmRelocationReceipt, ValidatedArmRelocation,
    relocate_selected_instruction_out_of_arm, validate_arm_relocation,
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
/// `B = [LEAD; TRAIL] -> branch -> T = [T_HEAD; MOVING; T_TAIL] -> jump -> J`
/// and `F = [F_HEAD; F_TAIL] -> jump -> J = [HEAD; MID; TAIL] -> return`.
/// The default move relocates `MOVING` onto `LEAD`'s position — the head
/// of B's body — crossing `LEAD`, `TRAIL`, the branch terminator, the
/// landing edge, and `T_HEAD`, while J's body keeps its order. On the
/// other edge's path the member newly executes: F and the F-side
/// traversal of J must never read `R_MOVE` before a write retires the
/// foreign definition.
fn fixture(target: NativeTarget) -> ValidatedArmRelocation {
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
                        materialization(MOVING, R_MOVE, 7),
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
    ValidatedArmRelocation {
        receipt: ArmRelocationReceipt {
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
) -> ValidatedArmRelocation {
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
    source: &ValidatedArmRelocation,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
) -> Result<ValidatedArmRelocation, ArmRelocationError> {
    relocate_selected_instruction_out_of_arm(source, 0, member, destination, environment, budget())
}

/// The member rises through the fork onto the destination's position on
/// every target: `MOVING` leaves T's body, `T_HEAD` and `T_TAIL` keep
/// their order in the arm, the member lands at B's head with `LEAD`,
/// `TRAIL`, and the branch terminator untouched, and the skipped edge's
/// blocks keep their order — identity, kind, operands, and provenance
/// intact. The replayed proposal restores the source bit-identically.
#[test]
fn member_relocates_into_the_head() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = fixture(target);
        let result = relocate(&source, &environment, MOVING, LEAD).unwrap();
        let original = &source.transformed().functions[0];
        let moved = &result.transformed().functions[0];
        assert_eq!(
            moved.blocks[0]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![MOVING, LEAD, TRAIL]
        );
        assert_eq!(
            moved.blocks[1]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![T_HEAD, T_TAIL]
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
            moved.blocks[0].instructions[0],
            original.blocks[1].instructions[1]
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
        validate_arm_relocation(
            &source,
            0,
            MOVING,
            LEAD,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
    }
}

/// The destination names the landing position directly: a head body
/// instruction puts the member on its index, and the head's
/// terminator-carried instruction lands the member at the body end.
/// Either arm's member rises — `F_HEAD` leaves the zero arm onto
/// `TRAIL`'s position while the nonzero edge becomes the speculative
/// path.
#[test]
fn member_lands_at_the_named_position() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let middle = relocate(&source, &environment, MOVING, TRAIL).unwrap();
    assert_eq!(
        middle.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![LEAD, MOVING, TRAIL]
    );
    let body_end = relocate(&source, &environment, MOVING, BRANCH).unwrap();
    assert_eq!(
        body_end.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![LEAD, TRAIL, MOVING]
    );
    let other_arm = relocate(&source, &environment, F_HEAD, TRAIL).unwrap();
    assert_eq!(
        other_arm.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![LEAD, F_HEAD, TRAIL]
    );
    assert_eq!(
        other_arm.transformed().functions[0].blocks[2]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![F_TAIL]
    );
}

/// Every body index relocates: the arm-tail member crosses the whole arm
/// prefix plus the head suffix, while the arm-head member crosses only the
/// head's own window — and each member's write must still die on the
/// speculative path.
#[test]
fn tail_and_head_members_relocate() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let tail = relocate(&source, &environment, T_TAIL, TRAIL).unwrap();
    assert_eq!(
        tail.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![LEAD, T_TAIL, TRAIL]
    );
    assert_eq!(
        tail.transformed().functions[0].blocks[1]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![T_HEAD, MOVING]
    );
    let head = relocate(&source, &environment, T_HEAD, TRAIL).unwrap();
    assert_eq!(
        head.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![LEAD, T_HEAD, TRAIL]
    );
    assert_eq!(
        head.transformed().functions[0].blocks[1]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![MOVING, T_TAIL]
    );
}

/// A degenerate fork whose both edges reach one arm still hoists the
/// member — every traversal of the arm already followed a head traversal,
/// so no path is speculative and the dead-path audit is vacuous.
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
    let result = relocate(&single, &environment, MOVING, LEAD).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![MOVING, LEAD, TRAIL]
    );
    assert_eq!(
        result.transformed().functions[0].blocks[1]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![T_HEAD, T_TAIL]
    );
}

/// A member write a crossed position reads would starve the consumer: in
/// the head suffix and in the arm prefix it refuses, while a read at or
/// after the member's old index — the arm tail, the arm terminator — still
/// observes the member's own definition and admits.
#[test]
fn raw_hazard_keeps_order_through_the_edge() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let suffix_reads = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[1] = instruction(
            TRAIL,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[R_MOVE, R_TRAIL],
        );
    });
    // Landing at `LEAD` crosses `TRAIL`; landing at the body end leaves
    // the member after it, so the read still observes the same reaching
    // definition.
    assert_eq!(
        relocate(&suffix_reads, &environment, MOVING, LEAD).unwrap_err(),
        ArmRelocationError::UnsupportedPair
    );
    relocate(&suffix_reads, &environment, MOVING, BRANCH).unwrap();
    let prefix_reads = mutated(target, |function, environment| {
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
    // The arm prefix ran before the member; after the move it runs after —
    // the read would observe the member's value it never saw.
    assert_eq!(
        relocate(&prefix_reads, &environment, MOVING, LEAD).unwrap_err(),
        ArmRelocationError::UnsupportedPair
    );
    // The arm tail stays after the member on every path through the arm:
    // its read still observes the member's write.
    let tail_reads = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[1].instructions[2] = instruction(
            T_TAIL,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[R_MOVE, R_TTAIL],
        );
    });
    relocate(&tail_reads, &environment, MOVING, LEAD).unwrap();
}

/// A member reading a location a crossed instruction writes would observe
/// the new value after the move: a head-suffix write refuses only while
/// the landing index leaves the member ahead of it, and an arm-prefix
/// write refuses at every landing because the member rose past it.
#[test]
fn war_hazard_keeps_order_through_the_edge() {
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
            function.blocks[1].instructions[1] = instruction(
                MOVING,
                SelectedInstructionKind::CopyI64,
                &copy,
                &[POINTER, R_MOVE],
            );
            edit(function, environment);
        })
    };
    let suffix_writes = member_reads_pointer(&mut |function, environment| {
        let materialize = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[1] = instruction(
            TRAIL,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(9),
            },
            &materialize,
            &[POINTER],
        );
    });
    // Landing at `LEAD` puts the member's read ahead of `TRAIL`'s write —
    // the arm position read the value the write published. Landing at the
    // body end keeps the read after the write.
    assert_eq!(
        relocate(&suffix_writes, &environment, MOVING, LEAD).unwrap_err(),
        ArmRelocationError::UnsupportedPair
    );
    relocate(&suffix_writes, &environment, MOVING, BRANCH).unwrap();
    let prefix_writes = member_reads_pointer(&mut |function, environment| {
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
    // `T_HEAD` wrote `POINTER` ahead of the member; the hoisted read would
    // observe the value before that write.
    assert_eq!(
        relocate(&prefix_writes, &environment, MOVING, LEAD).unwrap_err(),
        ArmRelocationError::UnsupportedPair
    );
}

/// A member and a crossed instruction writing the same register would
/// change which definition later positions observe.
#[test]
fn waw_hazard_keeps_order_through_the_edge() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let suffix_writes = mutated(target, |function, environment| {
        let materialize = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[1] = instruction(
            TRAIL,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(9),
            },
            &materialize,
            &[R_MOVE],
        );
    });
    assert_eq!(
        relocate(&suffix_writes, &environment, MOVING, LEAD).unwrap_err(),
        ArmRelocationError::UnsupportedPair
    );
    // Landing at the body end keeps the member's write after `TRAIL`'s,
    // the order the arm position had.
    relocate(&suffix_writes, &environment, MOVING, BRANCH).unwrap();
    let prefix_writes = mutated(target, |function, environment| {
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
        relocate(&prefix_writes, &environment, MOVING, LEAD).unwrap_err(),
        ArmRelocationError::UnsupportedPair
    );
}

/// Condition state couples like registers through the edge: the branch
/// terminator reads the target's condition units, so a flag-publishing
/// member can never rise past it; a flag-reading member cannot cross a
/// flag writer in the head suffix or the arm prefix, while a flag-inert
/// window admits it.
#[test]
fn condition_state_couples_through_the_edge() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let subtract_kind = SelectedInstructionKind::ExactSubtractI64 {
        obligation: ObligationId::new(11).unwrap(),
        accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes([7; 32]),
    };
    // A flag-publishing member rewrites the units the branch terminator
    // itself reads: the member's definition would land before the read on
    // every traversal, changing what the branch observes.
    let flag_writer = mutated(target, |function, environment| {
        let subtract = environment
            .constraint(environment.selected_keys().subtract_i64)
            .unwrap()
            .clone();
        function.blocks[1].instructions[1] =
            instruction(MOVING, subtract_kind, &subtract, &[POINTER, R_MOVE, R_MOVE]);
    });
    assert_eq!(
        relocate(&flag_writer, &environment, MOVING, LEAD).unwrap_err(),
        ArmRelocationError::UnsupportedPair
    );
    let member_reads_flags = |edit: &mut dyn FnMut(
        &mut SelectedFunction,
        &register_environment::ValidatedTargetRegisterEnvironment,
    )| {
        mutated(target, |function, environment| {
            let boolean = environment
                .constraint(environment.selected_keys().materialize_boolean)
                .unwrap()
                .clone();
            function.blocks[1].instructions[1] = instruction(
                MOVING,
                SelectedInstructionKind::MaterializeBooleanEqual,
                &boolean,
                &[R_MOVE],
            );
            edit(function, environment);
        })
    };
    // A flag-reading member cannot rise ahead of a flag writer in the
    // head suffix — it would observe the flags before that write; at the
    // body end it reads the flags the writer published, as before.
    let suffix_writes_flags = member_reads_flags(&mut |function, environment| {
        let subtract = environment
            .constraint(environment.selected_keys().subtract_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[1] = instruction(
            TRAIL,
            subtract_kind,
            &subtract,
            &[POINTER, R_TRAIL, R_TRAIL],
        );
    });
    assert_eq!(
        relocate(&suffix_writes_flags, &environment, MOVING, LEAD).unwrap_err(),
        ArmRelocationError::UnsupportedPair
    );
    relocate(&suffix_writes_flags, &environment, MOVING, BRANCH).unwrap();
    // The same coupling holds for a compare publishing flags in the arm
    // prefix: the member's read would lose the flags it observed.
    let prefix_writes_flags = member_reads_flags(&mut |function, environment| {
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
    assert_eq!(
        relocate(&prefix_writes_flags, &environment, MOVING, LEAD).unwrap_err(),
        ArmRelocationError::UnsupportedPair
    );
}

/// The branch terminator is a crossed position, not a window barrier: a
/// register the branch reads couples like any crossed read, while a read
/// it shares with the member admits. The member's own arm terminator is
/// never crossed — the member leaves the body ahead of it — so its read
/// of the member's result still observes the same definition.
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
        relocate(&terminator_reads, &environment, MOVING, LEAD).unwrap_err(),
        ArmRelocationError::UnsupportedPair
    );
    // The arm's terminator stays after the member on every traversal of
    // the arm — its read of the member's result admits.
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
    relocate(&arm_terminator_reads, &environment, MOVING, LEAD).unwrap();
}

/// The landing edge's register transports sit between the member's old
/// and new positions: a member defining the transported argument would
/// hand the binding a new value, a member defining the parameter would be
/// overwritten by it, and a member reading the parameter would observe
/// the pre-transport value — while a member merely reading the argument
/// crosses freely. The skipped edge's transports are never crossed — they
/// join the dead-path audit, where an argument reading a live member
/// definition refuses and a parameter writing one retires it.
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
            function.blocks[1].instructions[1] = instruction(
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
            function.blocks[1].instructions[1] = instruction(
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
    relocate(&on_landing_edge(&mut |_, _| {}), &environment, MOVING, LEAD).unwrap();
    relocate(
        &on_landing_edge(&mut member_reads(POINTER)),
        &environment,
        MOVING,
        LEAD,
    )
    .unwrap();
    assert_eq!(
        relocate(
            &on_landing_edge(&mut member_writes(POINTER)),
            &environment,
            MOVING,
            LEAD,
        )
        .unwrap_err(),
        ArmRelocationError::UnsupportedPair
    );
    assert_eq!(
        relocate(
            &on_landing_edge(&mut member_writes(R_BOUND)),
            &environment,
            MOVING,
            LEAD,
        )
        .unwrap_err(),
        ArmRelocationError::UnsupportedPair
    );
    assert_eq!(
        relocate(
            &on_landing_edge(&mut member_reads(R_BOUND)),
            &environment,
            MOVING,
            LEAD,
        )
        .unwrap_err(),
        ArmRelocationError::UnsupportedPair
    );
    // On the skipped edge a transport argument reading the member's write
    // observes the foreign definition on a path that carried a different
    // value before; a parameter writing it retires the foreign definition
    // before any reader.
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
            LEAD,
        )
        .unwrap_err(),
        ArmRelocationError::UnsupportedPair
    );
    relocate(
        &on_skipped_edge(&mut || binding(POINTER, R_MOVE)),
        &environment,
        MOVING,
        LEAD,
    )
    .unwrap();
}

/// The arm must hang from one head for this move: the member's block is a
/// source block — never the entry block or an implementation block —
/// whose predecessor edges all leave one block ending in a two-successor
/// conditional terminator, and the destination names a position in that
/// head.
#[test]
fn the_arm_must_rise_from_one_head() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A member of the entry block never rose out of an arm.
    let source = fixture(target);
    assert_eq!(
        relocate(&source, &environment, LEAD, TRAIL).unwrap_err(),
        ArmRelocationError::UnsupportedPair
    );
    // The join's members have two predecessor blocks — the move would add
    // an execution on the path that never ran it.
    assert_eq!(
        relocate(&source, &environment, HEAD, TRAIL).unwrap_err(),
        ArmRelocationError::UnsupportedPair
    );
    // The destination must name a position in the head: positions in the
    // arm, the other arm, the join, and dangling ids all refuse.
    for destination in [T_HEAD, F_HEAD, HEAD, SelectedInstructionId(99)] {
        assert_eq!(
            relocate(&source, &environment, MOVING, destination).unwrap_err(),
            ArmRelocationError::UnsupportedPair,
            "destination {destination:?}"
        );
    }
    // A head ending in `Jump` is the sole-predecessor family's case.
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
        relocate(&jumped, &environment, MOVING, LEAD).unwrap_err(),
        ArmRelocationError::UnsupportedPair
    );
    // A second predecessor into the arm — even from the arm itself —
    // hands its stream a member that ran an extra time on that path.
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
        relocate(&extra_pred, &environment, MOVING, LEAD).unwrap_err(),
        ArmRelocationError::UnsupportedPair
    );
    let self_loop = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        function.blocks[1].terminator = jump_terminator(
            instruction(T_JUMP, SelectedInstructionKind::Jump, &jump_row, &[]),
            successor(BLOCK_T, BlockId::new(2).unwrap(), EDGE_TJ),
        );
    });
    assert_eq!(
        relocate(&self_loop, &environment, MOVING, LEAD).unwrap_err(),
        ArmRelocationError::UnsupportedPair
    );
    // An unreachable arm has no head at all.
    let unreachable_arm = mutated(target, |function, _| {
        let successor = match &mut function.blocks[0].terminator {
            SelectedTerminator::ConditionalBranch { when_nonzero, .. } => when_nonzero,
            _ => unreachable!(),
        };
        successor.block = BLOCK_F;
        successor.source_target = BlockId::new(3).unwrap();
    });
    assert_eq!(
        relocate(&unreachable_arm, &environment, MOVING, LEAD).unwrap_err(),
        ArmRelocationError::UnsupportedPair
    );
    // An implementation-origin arm or head carries boundary work the
    // bounded audit does not cross.
    let cased_arm = mutated(target, |function, _| {
        function.blocks[1].origin = SelectedBlockOrigin::CaseDispatch {
            source: BlockId::new(2).unwrap(),
            case_ordinal: 0,
        };
    });
    assert_eq!(
        relocate(&cased_arm, &environment, MOVING, LEAD).unwrap_err(),
        ArmRelocationError::UnsupportedPair
    );
    let cased_head = mutated(target, |function, _| {
        function.blocks[0].origin = SelectedBlockOrigin::CaseDispatch {
            source: BlockId::new(1).unwrap(),
            case_ordinal: 0,
        };
    });
    assert_eq!(
        relocate(&cased_head, &environment, MOVING, LEAD).unwrap_err(),
        ArmRelocationError::UnsupportedPair
    );
    // The entry block cannot be the arm even when an edge reaches it.
    let entry_arm = mutated(target, |function, _| {
        function.entry_block = BLOCK_T;
    });
    assert_eq!(
        relocate(&entry_arm, &environment, MOVING, LEAD).unwrap_err(),
        ArmRelocationError::UnsupportedPair
    );
}

/// Only a plain semantic edge carries the member into the arm: a
/// continuation role, case custody, per-edge fuel, or a live structural
/// transport on a landing edge all refuse. The skipped edge is never
/// crossed, so fuel and inert payloads on it stay free — while a case
/// payload or structural binding that reads a live member definition is a
/// dead-path observation and refuses.
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
            relocate(&continued, &environment, MOVING, LEAD).unwrap_err(),
            ArmRelocationError::UnsupportedPair
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
        relocate(&case_edge, &environment, MOVING, LEAD).unwrap_err(),
        ArmRelocationError::UnsupportedPair
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
        relocate(&fueled, &environment, MOVING, LEAD).unwrap_err(),
        ArmRelocationError::UnsupportedPair
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
        relocate(&structural, &environment, MOVING, LEAD).unwrap_err(),
        ArmRelocationError::UnsupportedPair
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
    relocate(&fueled_skipped, &environment, MOVING, LEAD).unwrap();
    // A case payload on the skipped edge whose transport reads the
    // member's definition is a speculative observation; an `Unused`
    // payload moves nothing.
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
        relocate(&case_reads_member, &environment, MOVING, LEAD).unwrap_err(),
        ArmRelocationError::UnsupportedPair
    );
    // A structural binding on the skipped edge reads its argument register
    // at the boundary — reading the member's foreign definition refuses.
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
        relocate(&structural_reads_member, &environment, MOVING, LEAD).unwrap_err(),
        ArmRelocationError::UnsupportedPair
    );
}

/// Barrier kinds and call-roster entries refuse as the member or anywhere
/// inside the crossed window — the head suffix, the branch terminator's
/// call contract, and the arm prefix included — while a barrier past the
/// landing index is never crossed... every crossed position is at or past
/// it, so a barrier on the skipped side still executes on its own path.
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
            relocate(&member_barrier, &environment, MOVING, LEAD).unwrap_err(),
            ArmRelocationError::UnsupportedInstruction,
            "member {kind:?}"
        );
        let suffix_barrier = mutated(target, |function, _| {
            function.blocks[0].instructions[1].kind = kind;
        });
        assert_eq!(
            relocate(&suffix_barrier, &environment, MOVING, LEAD).unwrap_err(),
            ArmRelocationError::UnsupportedInstruction,
            "crossed suffix {kind:?}"
        );
        // A barrier at or before the landing index's exclusive side keeps
        // the member on the side it always had: landing at the body end
        // leaves the whole head body ahead of the member.
        relocate(&suffix_barrier, &environment, MOVING, BRANCH).unwrap();
        let prefix_barrier = mutated(target, |function, _| {
            function.blocks[1].instructions[0].kind = kind;
        });
        assert_eq!(
            relocate(&prefix_barrier, &environment, MOVING, LEAD).unwrap_err(),
            ArmRelocationError::UnsupportedInstruction,
            "crossed prefix {kind:?}"
        );
        // A barrier after the member's old index in the arm is never
        // crossed — the member leaves ahead of it either way.
        let arm_tail_barrier = mutated(target, |function, _| {
            function.blocks[1].instructions[2].kind = kind;
        });
        relocate(&arm_tail_barrier, &environment, MOVING, LEAD).unwrap();
        // A barrier on the speculative side still runs on its own path; it
        // only refuses if it would read a still-live member definition.
        let skipped_barrier = mutated(target, |function, _| {
            function.blocks[2].instructions[0].kind = kind;
        });
        relocate(&skipped_barrier, &environment, MOVING, LEAD).unwrap();
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
        (MOVING, LEAD),
        (TRAIL, LEAD),
        (T_HEAD, LEAD),
        (BRANCH, LEAD),
    ] {
        let contract = mutated(target, |function, _| {
            function.calls.push(call_contract(instruction_id));
        });
        assert_eq!(
            relocate(&contract, &environment, MOVING, destination).unwrap_err(),
            ArmRelocationError::UnsupportedInstruction,
            "call roster {instruction_id:?}"
        );
    }
    // The arm terminator and the speculative arm are outside the window: a
    // call-roster row there is a dead-path position like any other.
    for instruction_id in [T_JUMP, F_HEAD, F_JUMP, HEAD] {
        let contract = mutated(target, |function, _| {
            function.calls.push(call_contract(instruction_id));
        });
        relocate(&contract, &environment, MOVING, LEAD).unwrap();
    }
}

/// Only pure register and condition-state work may speculate: a member
/// carrying a memory roster row would run its access on every traversal,
/// and a row-less load or private-slot store gains the same access — as
/// would a kind whose target encoding may fault on the paths that never
/// ran it. Memory work on the speculative paths never moves.
#[test]
fn member_must_be_pure_work() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A roster-carrying member's recorded access would become
    // unconditional.
    let roster_member = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap()
            .clone();
        function.blocks[1].instructions[1] = instruction(
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
        relocate(&roster_member, &environment, MOVING, LEAD).unwrap_err(),
        ArmRelocationError::UnsupportedInstruction
    );
    // A row-less load still performs an access — and carries any fault —
    // on every traversal after the move.
    let rowless_load = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap()
            .clone();
        function.blocks[1].instructions[1] = instruction(
            MOVING,
            SelectedInstructionKind::Load8 { byte_offset: 0 },
            &load,
            &[POINTER, R_MOVE],
        );
    });
    assert_eq!(
        relocate(&rowless_load, &environment, MOVING, LEAD).unwrap_err(),
        ArmRelocationError::UnsupportedInstruction
    );
    // A row-less private-slot store still writes storage on every
    // traversal; hoisting it would add the write to the speculative path.
    let rowless_store = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap()
            .clone();
        function.blocks[1].instructions[1] = instruction(
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
        relocate(&rowless_store, &environment, MOVING, LEAD).unwrap_err(),
        ArmRelocationError::UnsupportedInstruction
    );
    // A potentially-faulting kind ran only on the arm's path: the divide
    // would newly run — and could fault — on every head traversal.
    let exact_divide = mutated(target, |function, environment| {
        let divide = environment
            .constraint(environment.selected_keys().divide_u64)
            .unwrap()
            .clone();
        function.blocks[1].instructions[1] = instruction(
            MOVING,
            SelectedInstructionKind::ExactDivideU64 {
                obligation: ObligationId::new(11).unwrap(),
                accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                    [7; 32],
                ),
            },
            &divide,
            &[POINTER, R_MOVE, R_MOVE],
        );
    });
    assert_eq!(
        relocate(&exact_divide, &environment, MOVING, LEAD).unwrap_err(),
        ArmRelocationError::UnsupportedInstruction
    );
    let saturating_divide = mutated(target, |function, environment| {
        let divide = environment
            .constraint(environment.selected_keys().saturating_divide_signed)
            .unwrap()
            .clone();
        function.blocks[1].instructions[1] = instruction(
            MOVING,
            SelectedInstructionKind::SaturatingDivide {
                carrier: selected_instructions::SaturatingCarrier::U64,
                obligation: ObligationId::new(11).unwrap(),
                accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                    [7; 32],
                ),
            },
            &divide,
            &[POINTER, R_MOVE, R_MOVE],
        );
    });
    assert_eq!(
        relocate(&saturating_divide, &environment, MOVING, LEAD).unwrap_err(),
        ArmRelocationError::UnsupportedInstruction
    );
    // Memory work on the speculative paths never moves: a store in F
    // refuses only if it would read a live member definition.
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
    relocate(&skipped_store, &environment, MOVING, LEAD).unwrap();
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
        relocate(&skipped_store_reads_member, &environment, MOVING, LEAD).unwrap_err(),
        ArmRelocationError::UnsupportedPair
    );
}

/// Every location the member writes must be dead — unread until
/// rewritten — on every path the head's other edges reach: a reader in
/// the skipped arm, in a deeper block, or in the shared join reached
/// through the skipped side all refuse, while a write that retires the
/// foreign definition before any reader admits. A loop back through the
/// head scans the positions before the landing index against the live
/// foreign set, republishes at the member's new position, and walks on.
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
    // A reader anywhere on the speculative side observes the foreign
    // definition.
    for (block, index, destination, label) in [
        (2usize, 0usize, R_FHEAD, "skipped arm"),
        (2usize, 1usize, R_FTAIL, "skipped arm tail"),
        (3usize, 0usize, R_HEAD, "join via the skipped arm"),
        (3usize, 2usize, R_TAIL, "join tail via the skipped arm"),
    ] {
        let reading = mutated(target, reads_member(block, index, destination));
        assert_eq!(
            relocate(&reading, &environment, MOVING, LEAD).unwrap_err(),
            ArmRelocationError::UnsupportedPair,
            "{label}"
        );
    }
    // The skipped arm's terminator reading the member's write refuses just
    // the same — its position is on the speculative path.
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
        relocate(&terminator_reads, &environment, MOVING, LEAD).unwrap_err(),
        ArmRelocationError::UnsupportedPair
    );
    // A write retiring the foreign definition before any reader admits —
    // the reader then observes the rewrite it always saw on that path.
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
    relocate(&retired, &environment, MOVING, LEAD).unwrap();
    // A deeper speculative reader still refuses: the walk crosses blocks.
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
        relocate(&deeper, &environment, MOVING, LEAD).unwrap_err(),
        ArmRelocationError::UnsupportedPair
    );
    // A loop back through the head re-enters it with the member's
    // definition still foreign: a read ahead of the member's new position
    // refuses, while a rewrite there retires the foreign value — the
    // member's own new position then republishes it, so later positions
    // on the same traversal are audited against it again.
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
    // Landing at `TRAIL` leaves `LEAD` ahead of the member — uncrossed —
    // so only the speculative looped traversal can refuse.
    assert_eq!(
        relocate(&looped_read, &environment, MOVING, TRAIL).unwrap_err(),
        ArmRelocationError::UnsupportedPair
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
    relocate(&looped_retired, &environment, MOVING, TRAIL).unwrap();
}

/// A settlement positioned past the member's index in the arm observed it
/// inside that executed prefix, and one past the landing index in the
/// head newly observes it there; settlements on the speculative paths
/// never had the member in their streams.
#[test]
fn boundary_settlements_bound_the_window() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // In the arm, settlements at or before the member's index admit;
    // positions past it observed the member inside the prefix.
    for (position, admits) in [(0u32, true), (1, true), (2, false), (3, false)] {
        let settled = mutated(target, |function, _| {
            function
                .boundary_settlements
                .push(settlement(BLOCK_T, position, 50));
        });
        let result = relocate(&settled, &environment, MOVING, LEAD);
        assert_eq!(result.is_ok(), admits, "arm settlement at {position}");
    }
    // In the head the bound is the landing index: at or before it the
    // executed prefix is unchanged; past it the member joins the prefix.
    for (position, admits) in [(0u32, true), (1, true), (2, false)] {
        let settled = mutated(target, |function, _| {
            function
                .boundary_settlements
                .push(settlement(BLOCK_B, position, 51));
        });
        let result = relocate(&settled, &environment, MOVING, TRAIL);
        assert_eq!(result.is_ok(), admits, "head settlement at {position}");
    }
    // Landing at the body end keeps every head settlement: none sits past
    // the member's new index.
    let settled = mutated(target, |function, _| {
        function
            .boundary_settlements
            .push(settlement(BLOCK_B, 2, 53));
    });
    relocate(&settled, &environment, MOVING, BRANCH).unwrap();
    // Settlements on the speculative side and in the join never observe
    // the member — it was never in those streams.
    for (block, position) in [(BLOCK_F, 0u32), (BLOCK_F, 2), (BLOCK_J, 0), (BLOCK_J, 3)] {
        let settled = mutated(target, |function, _| {
            function
                .boundary_settlements
                .push(settlement(block, position, 54));
        });
        relocate(&settled, &environment, MOVING, LEAD).unwrap();
    }
}

/// The pair must name one member in an arm's body and one position in the
/// head: an unknown member, a wrong function, a destination outside the
/// head, a member whose own block lacks the fork, and a terminator id as
/// member all refuse.
#[test]
fn only_the_named_arm_window_relocates() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // An unknown member or function index never locates the window.
    assert_eq!(
        relocate_selected_instruction_out_of_arm(
            &source,
            0,
            SelectedInstructionId(99),
            LEAD,
            &environment,
            budget(),
        )
        .unwrap_err(),
        ArmRelocationError::SourceMismatch
    );
    assert_eq!(
        relocate_selected_instruction_out_of_arm(&source, 9, MOVING, LEAD, &environment, budget(),)
            .unwrap_err(),
        ArmRelocationError::SourceMismatch
    );
    // Naming a terminator-carried instruction as member is not a body
    // position.
    assert_eq!(
        relocate(&source, &environment, T_JUMP, LEAD).unwrap_err(),
        ArmRelocationError::SourceMismatch
    );
    assert_eq!(
        relocate(&source, &environment, BRANCH, LEAD).unwrap_err(),
        ArmRelocationError::SourceMismatch
    );
    // A destination outside the head names no landing position: the arm's
    // own members and terminator, the join's, and dangling ids.
    for destination in [T_HEAD, T_TAIL, T_JUMP, HEAD, SelectedInstructionId(99)] {
        assert_eq!(
            relocate(&source, &environment, MOVING, destination).unwrap_err(),
            ArmRelocationError::UnsupportedPair,
            "destination {destination:?}"
        );
    }
}

/// Replay consumes only the exact move: a proposal that drops the member,
/// lands it anywhere else, permutes the crossed positions, or carries an
/// unrelated edit all reject.
#[test]
fn replay_rejects_anything_but_the_move() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let result = relocate(&source, &environment, MOVING, TRAIL).unwrap();
    // The honest proposal replays.
    validate_arm_relocation(
        &source,
        0,
        MOVING,
        TRAIL,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // The member at the wrong index rejects.
    let mut displaced = result.transformed().clone();
    let member = displaced.functions[0].blocks[0].instructions.remove(1);
    displaced.functions[0].blocks[0]
        .instructions
        .insert(2, member.clone());
    assert_eq!(
        validate_arm_relocation(&source, 0, MOVING, TRAIL, &environment, budget(), displaced)
            .unwrap_err(),
        ArmRelocationError::ReplayMismatch
    );
    // The member left in its arm rejects.
    let mut unmoved = result.transformed().clone();
    unmoved.functions[0].blocks[0].instructions.remove(1);
    unmoved.functions[0].blocks[1]
        .instructions
        .insert(1, member);
    assert_eq!(
        validate_arm_relocation(&source, 0, MOVING, TRAIL, &environment, budget(), unmoved)
            .unwrap_err(),
        ArmRelocationError::ReplayMismatch
    );
    // A dropped instruction in the head rejects.
    let mut dropped = result.transformed().clone();
    dropped.functions[0].blocks[0].instructions.pop();
    assert_eq!(
        validate_arm_relocation(&source, 0, MOVING, TRAIL, &environment, budget(), dropped)
            .unwrap_err(),
        ArmRelocationError::ReplayMismatch
    );
    // An unrelated literal edit inside the source arm rejects.
    let mut edited = result.transformed().clone();
    edited.functions[0].blocks[1].instructions[0].kind = SelectedInstructionKind::MaterializeI64 {
        value: IntegerValue::Unsigned(12),
    };
    assert_eq!(
        validate_arm_relocation(&source, 0, MOVING, TRAIL, &environment, budget(), edited)
            .unwrap_err(),
        ArmRelocationError::ReplayMismatch
    );
    // Naming a different window on the same proposal re-derives a
    // different landing and rejects.
    assert_eq!(
        validate_arm_relocation(
            &source,
            0,
            MOVING,
            LEAD,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap_err(),
        ArmRelocationError::ReplayMismatch
    );
}

/// The bounded audit is measured: the arm window prices every scan,
/// crossed-surface pair, roster row, and the dead-path fixpoint bound
/// against the work budget, and a budget one step short refuses rather
/// than skimping. Landing deeper into the head's body crosses fewer head
/// positions.
#[test]
fn measured_validation_step_boundary() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // The member-locate scan prices every block's body plus terminator
    // once across the plan (3+4+3+4 = 14), and again for this function's
    // blocks (14). The crossed surfaces pair the member (1) against LEAD
    // (1), TRAIL (1), the branch terminator (2 uses + 1 definition on
    // x86-64), and T_HEAD (1): 2+2+4+2 = 10 steps. The dead-path bound
    // prices each block's body, terminator, and edge surfaces once per
    // member location plus the initial scan: on x86-64 the materializations
    // cost 1 each, the jumps 2, the branch 3, and the return 9 —
    // (2+3)+(3+2)+(2+2)+(3+9) = 26 — times one written member register
    // plus one: 26*2 = 52.
    let steps: u64 = 14 + 14 + 10 + 52;
    let exact = OptimizationWorkBudget::new(1, 1, steps, 1, 1).unwrap();
    relocate_selected_instruction_out_of_arm(&source, 0, MOVING, LEAD, &environment, exact)
        .unwrap();
    let starved = OptimizationWorkBudget::new(1, 1, steps - 1, 1, 1).unwrap();
    assert_eq!(
        relocate_selected_instruction_out_of_arm(&source, 0, MOVING, LEAD, &environment, starved,)
            .unwrap_err(),
        ArmRelocationError::WorkBudgetExceeded
    );
    // Landing at the body end crosses no head body position: the member
    // pairs against the terminator and the arm prefix only.
    let exact = OptimizationWorkBudget::new(1, 1, steps - 4, 1, 1).unwrap();
    relocate_selected_instruction_out_of_arm(&source, 0, MOVING, BRANCH, &environment, exact)
        .unwrap();
    let starved = OptimizationWorkBudget::new(1, 1, steps - 5, 1, 1).unwrap();
    assert_eq!(
        relocate_selected_instruction_out_of_arm(&source, 0, MOVING, BRANCH, &environment, starved,)
            .unwrap_err(),
        ArmRelocationError::WorkBudgetExceeded
    );
}

/// The environment's target must be the plan's target: a relocation proven
/// for one target's constraints is not evidence on another.
#[test]
fn target_mismatch_rejects() {
    let source = fixture(NativeTarget::linux_x64());
    let environment = baseline_target_register_environment(NativeTarget::linux_arm64()).unwrap();
    assert_eq!(
        relocate(&source, &environment, MOVING, LEAD).unwrap_err(),
        ArmRelocationError::SourceMismatch
    );
}

/// Two runs over the identical source produce the identical validated
/// result, and the published plan is a legal second input through the
/// sealed analysis boundary: the arm-tail member rises through the same
/// fork onto it, a hazard-coupled member still declines, and an in-block
/// family still admits.
#[test]
fn arm_relocation_is_deterministic_and_re_admitted() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let first = relocate(&source, &environment, MOVING, LEAD).unwrap();
    let second = relocate(&source, &environment, MOVING, LEAD).unwrap();
    assert_eq!(first, second);
    // The validated output carries the sealed analysis boundary, so it is
    // a legal second input — not merely a reconstruction of one. The arm
    // tail member rises through the same fork onto it.
    let again =
        relocate_selected_instruction_out_of_arm(&first, 0, T_TAIL, TRAIL, &environment, budget())
            .unwrap();
    assert_eq!(
        again.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![MOVING, LEAD, T_TAIL, TRAIL]
    );
    assert_eq!(
        again.receipt().source_selected(),
        first.receipt().transformed_selected()
    );
    // A hazard-coupled member still declines on the second input: once
    // `MOVING` sits at B's head, a `T_TAIL` reading `R_THEAD` cannot rise
    // past `T_HEAD`'s write.
    let coupled = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[1].instructions[2] = instruction(
            T_TAIL,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[R_THEAD, R_TTAIL],
        );
    });
    let moved = relocate(&coupled, &environment, MOVING, LEAD).unwrap();
    assert_eq!(
        relocate_selected_instruction_out_of_arm(&moved, 0, T_TAIL, TRAIL, &environment, budget(),)
            .unwrap_err(),
        ArmRelocationError::UnsupportedPair
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
