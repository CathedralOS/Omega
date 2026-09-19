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
    ForkRunRelocationError, ForkRunRelocationReceipt, ValidatedForkRunRelocation,
    relocate_selected_run_into_arm, validate_fork_run_relocation,
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
const TRAIL: SelectedInstructionId = SelectedInstructionId(3);
const RUN_A: SelectedInstructionId = SelectedInstructionId(4);
const RUN_B: SelectedInstructionId = SelectedInstructionId(5);
const T_HEAD: SelectedInstructionId = SelectedInstructionId(6);
const T_TAIL: SelectedInstructionId = SelectedInstructionId(7);
const F_HEAD: SelectedInstructionId = SelectedInstructionId(8);
const F_TAIL: SelectedInstructionId = SelectedInstructionId(9);
const HEAD: SelectedInstructionId = SelectedInstructionId(10);
const MID: SelectedInstructionId = SelectedInstructionId(11);
const TAIL: SelectedInstructionId = SelectedInstructionId(12);
const BRANCH: SelectedInstructionId = SelectedInstructionId(13);
const T_JUMP: SelectedInstructionId = SelectedInstructionId(14);
const F_JUMP: SelectedInstructionId = SelectedInstructionId(15);
const RET: SelectedInstructionId = SelectedInstructionId(16);

const POINTER: VirtualRegisterId = VirtualRegisterId(0);
const R_LEAD: VirtualRegisterId = VirtualRegisterId(1);
const R_TRAIL: VirtualRegisterId = VirtualRegisterId(2);
const R_MOVE_A: VirtualRegisterId = VirtualRegisterId(3);
const R_MOVE_B: VirtualRegisterId = VirtualRegisterId(4);
const R_THEAD: VirtualRegisterId = VirtualRegisterId(5);
const R_TTAIL: VirtualRegisterId = VirtualRegisterId(6);
const R_FHEAD: VirtualRegisterId = VirtualRegisterId(7);
const R_FTAIL: VirtualRegisterId = VirtualRegisterId(8);
const R_HEAD: VirtualRegisterId = VirtualRegisterId(9);
const R_MID: VirtualRegisterId = VirtualRegisterId(10);
const R_TAIL: VirtualRegisterId = VirtualRegisterId(11);
const R_BOUND: VirtualRegisterId = VirtualRegisterId(12);

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
/// `B = [LEAD; RUN_A; RUN_B; TRAIL] -> branch -> T = [T_HEAD; T_TAIL] ->
/// jump -> J` and `F = [F_HEAD; F_TAIL] -> jump -> J = [HEAD; MID; TAIL]
/// -> return`.
/// The default move relocates the run `RUN_A..=RUN_B` onto `T_HEAD`'s
/// position — the head of T's body — crossing `TRAIL`, the branch
/// terminator, and the landing edge, while J's body keeps its order. On
/// the other edge's path the run no longer executes: F and the F-side
/// traversal of J must never read `R_MOVE_A` or `R_MOVE_B` before a write
/// retires them.
fn fixture(target: NativeTarget) -> ValidatedForkRunRelocation {
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
        result_register(R_TRAIL, TRAIL, 3),
        result_register(R_MOVE_A, RUN_A, 4),
        result_register(R_MOVE_B, RUN_B, 5),
        result_register(R_THEAD, T_HEAD, 6),
        result_register(R_TTAIL, T_TAIL, 7),
        result_register(R_FHEAD, F_HEAD, 8),
        result_register(R_FTAIL, F_TAIL, 9),
        result_register(R_HEAD, HEAD, 10),
        result_register(R_MID, MID, 11),
        result_register(R_TAIL, TAIL, 12),
        result_register(R_BOUND, LEAD, 13),
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
                        materialization(RUN_A, R_MOVE_A, 7),
                        materialization(RUN_B, R_MOVE_B, 11),
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
                        materialization(T_HEAD, R_THEAD, 13),
                        materialization(T_TAIL, R_TTAIL, 15),
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
                        materialization(F_HEAD, R_FHEAD, 17),
                        materialization(F_TAIL, R_FTAIL, 19),
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
                        materialization(HEAD, R_HEAD, 21),
                        materialization(MID, R_MID, 23),
                        materialization(TAIL, R_TAIL, 25),
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
    ValidatedForkRunRelocation {
        receipt: ForkRunRelocationReceipt {
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
) -> ValidatedForkRunRelocation {
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
    source: &ValidatedForkRunRelocation,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    first: SelectedInstructionId,
    last: SelectedInstructionId,
    destination: SelectedInstructionId,
) -> Result<ValidatedForkRunRelocation, ForkRunRelocationError> {
    relocate_selected_run_into_arm(source, 0, first, last, destination, environment, budget())
}

fn block_order(block: &SelectedBlock) -> Vec<SelectedInstructionId> {
    block
        .instructions
        .iter()
        .map(|instruction| instruction.id)
        .collect()
}

/// The run sinks through the fork onto the destination's position on
/// every target: `RUN_A` and `RUN_B` leave B's body in their own order,
/// `LEAD` and `TRAIL` keep their order around the vacated span, the run
/// lands at T's head with `T_HEAD`, `T_TAIL`, and the jump terminator
/// untouched, and the skipped arm and join keep their order — identity,
/// kind, operands, and provenance intact. The replayed proposal restores
/// the source bit-identically.
#[test]
fn run_relocates_into_the_arm() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = fixture(target);
        let result = relocate(&source, &environment, RUN_A, RUN_B, T_HEAD).unwrap();
        let original = &source.transformed().functions[0];
        let moved = &result.transformed().functions[0];
        assert_eq!(block_order(&moved.blocks[0]), vec![LEAD, TRAIL]);
        assert_eq!(
            block_order(&moved.blocks[1]),
            vec![RUN_A, RUN_B, T_HEAD, T_TAIL]
        );
        assert_eq!(
            moved.blocks[2].instructions,
            original.blocks[2].instructions
        );
        assert_eq!(
            moved.blocks[3].instructions,
            original.blocks[3].instructions
        );
        // The members moved bit-identically; every terminator, edge, and
        // roster stayed untouched.
        assert_eq!(
            moved.blocks[1].instructions[0],
            original.blocks[0].instructions[1]
        );
        assert_eq!(
            moved.blocks[1].instructions[1],
            original.blocks[0].instructions[2]
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
        validate_fork_run_relocation(
            &source,
            0,
            RUN_A,
            RUN_B,
            T_HEAD,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
    }
}

/// The destination names the landing position directly: a body
/// instruction puts the run on its index, the arm's terminator-carried
/// instruction lands the run at the body end, and the destination selects
/// which arm the run sinks into — naming a position in F lands it there
/// while the nonzero edge becomes the skipped path.
#[test]
fn run_lands_at_the_named_position() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let middle = relocate(&source, &environment, RUN_A, RUN_B, T_TAIL).unwrap();
    assert_eq!(
        block_order(&middle.transformed().functions[0].blocks[1]),
        vec![T_HEAD, RUN_A, RUN_B, T_TAIL]
    );
    let body_end = relocate(&source, &environment, RUN_A, RUN_B, T_JUMP).unwrap();
    assert_eq!(
        block_order(&body_end.transformed().functions[0].blocks[1]),
        vec![T_HEAD, T_TAIL, RUN_A, RUN_B]
    );
    let other_arm = relocate(&source, &environment, RUN_A, RUN_B, F_TAIL).unwrap();
    assert_eq!(
        block_order(&other_arm.transformed().functions[0].blocks[2]),
        vec![F_HEAD, RUN_A, RUN_B, F_TAIL]
    );
}

/// Any contiguous run of at least two members sinks: the block-tail run
/// crosses only the branch and the landing edge, the block-head run
/// crosses its trailing body first, and the whole body leaves the block
/// empty — a run of one member is the sibling family's case.
#[test]
fn tail_and_head_runs_relocate() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let tail = relocate(&source, &environment, RUN_B, TRAIL, T_HEAD).unwrap();
    assert_eq!(
        block_order(&tail.transformed().functions[0].blocks[0]),
        vec![LEAD, RUN_A]
    );
    assert_eq!(
        block_order(&tail.transformed().functions[0].blocks[1]),
        vec![RUN_B, TRAIL, T_HEAD, T_TAIL]
    );
    let head = relocate(&source, &environment, LEAD, RUN_A, T_TAIL).unwrap();
    assert_eq!(
        block_order(&head.transformed().functions[0].blocks[0]),
        vec![RUN_B, TRAIL]
    );
    assert_eq!(
        block_order(&head.transformed().functions[0].blocks[1]),
        vec![T_HEAD, LEAD, RUN_A, T_TAIL]
    );
    let whole = relocate(&source, &environment, LEAD, TRAIL, T_HEAD).unwrap();
    assert_eq!(
        block_order(&whole.transformed().functions[0].blocks[0]),
        Vec::<SelectedInstructionId>::new()
    );
    assert_eq!(
        block_order(&whole.transformed().functions[0].blocks[1]),
        vec![LEAD, RUN_A, RUN_B, TRAIL, T_HEAD, T_TAIL]
    );
}

/// The two named members bound the run: a repeated id, a last member
/// that does not follow the first, and a last member outside the first's
/// block all name no multi-member run — the single-member fork
/// relocation is the sibling family's case.
#[test]
fn the_named_members_bound_a_contiguous_run() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // One member is no run: the member-level family carries it.
    assert_eq!(
        relocate(&source, &environment, RUN_A, RUN_A, T_HEAD).unwrap_err(),
        ForkRunRelocationError::UnsupportedPair
    );
    // Reversed names bound no span, and neither does a last member ahead
    // of the first inside the same block.
    assert_eq!(
        relocate(&source, &environment, RUN_B, RUN_A, T_HEAD).unwrap_err(),
        ForkRunRelocationError::UnsupportedPair
    );
    assert_eq!(
        relocate(&source, &environment, RUN_A, LEAD, T_HEAD).unwrap_err(),
        ForkRunRelocationError::UnsupportedPair
    );
    // A last member in another block bounds no span in the first's.
    for last in [T_HEAD, F_HEAD, HEAD, T_JUMP, RET, SelectedInstructionId(99)] {
        assert_eq!(
            relocate(&source, &environment, RUN_A, last, T_HEAD).unwrap_err(),
            ForkRunRelocationError::UnsupportedPair,
            "last {last:?}"
        );
    }
}

/// A producer whose only crossed reader is the run's own next member
/// moves with it: `RUN_B` reads `RUN_A`'s result inside the run, where
/// internal coupling never trades order — the member move would starve
/// the consumer it leaves behind, so the single-member family refuses
/// where the run admits.
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
    let moved = relocate(&source, &environment, RUN_A, RUN_B, T_HEAD).unwrap();
    assert_eq!(
        block_order(&moved.transformed().functions[0].blocks[1]),
        vec![RUN_A, RUN_B, T_HEAD, T_TAIL]
    );
    // `RUN_A` alone cannot cross `RUN_B`, whose read of its result sits in
    // the member move's window — the single-member family refuses where
    // the run admits.
    assert_eq!(
        crate::relocate_selected_instruction_into_arm(
            &source,
            0,
            RUN_A,
            T_HEAD,
            &environment,
            budget(),
        )
        .unwrap_err(),
        crate::ForkRelocationError::UnsupportedPair
    );
}

/// A degenerate fork whose both edges reach one arm still sinks the run
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
    let result = relocate(&single, &environment, RUN_A, RUN_B, T_TAIL).unwrap();
    assert_eq!(
        block_order(&result.transformed().functions[0].blocks[0]),
        vec![LEAD, TRAIL]
    );
    assert_eq!(
        block_order(&result.transformed().functions[0].blocks[1]),
        vec![T_HEAD, RUN_A, RUN_B, T_TAIL]
    );
}

/// A member write a crossed position reads would starve the consumer: in
/// the block tail, in the arm prefix when the landing index is past it —
/// while a read at the landing index itself still observes the run — and
/// on the skipped side, where the run never runs, a read of either
/// definition refuses outright.
#[test]
fn raw_hazard_keeps_order_through_the_fork() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The run's own block tail is crossed: either member's result read
    // there refuses.
    for input in [R_MOVE_A, R_MOVE_B] {
        let tail_reads = mutated(target, |function, environment| {
            let copy = environment
                .constraint(environment.selected_keys().copy_i64)
                .unwrap()
                .clone();
            function.blocks[0].instructions[3] = instruction(
                TRAIL,
                SelectedInstructionKind::CopyI64,
                &copy,
                &[input, R_TRAIL],
            );
        });
        assert_eq!(
            relocate(&tail_reads, &environment, RUN_A, RUN_B, T_HEAD).unwrap_err(),
            ForkRunRelocationError::UnsupportedPair,
            "tail reading {input:?}"
        );
    }
    // The arm prefix is crossed when the run lands deeper: `T_HEAD` reads
    // a member's result, so landing at `T_TAIL` refuses while landing at
    // `T_HEAD` — where the read still observes the member — admits.
    let arm_reads = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[1].instructions[0] = instruction(
            T_HEAD,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[R_MOVE_A, R_THEAD],
        );
    });
    relocate(&arm_reads, &environment, RUN_A, RUN_B, T_HEAD).unwrap();
    assert_eq!(
        relocate(&arm_reads, &environment, RUN_A, RUN_B, T_TAIL).unwrap_err(),
        ForkRunRelocationError::UnsupportedPair
    );
    // A reader before the run's first index is never crossed: it kept the
    // pre-run value on either order.
    let before_reads = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[0] = instruction(
            LEAD,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[R_MOVE_A, R_LEAD],
        );
    });
    relocate(&before_reads, &environment, RUN_A, RUN_B, T_HEAD).unwrap();
    // The branch terminator is a crossed position: its read of a member's
    // result refuses.
    let branch_reads = mutated(target, |function, environment| {
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
            virtual_register: R_MOVE_B,
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
        relocate(&branch_reads, &environment, RUN_A, RUN_B, T_HEAD).unwrap_err(),
        ForkRunRelocationError::UnsupportedPair
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
                RUN_A,
                SelectedInstructionKind::CopyI64,
                &copy,
                &[POINTER, R_MOVE_A],
            );
            edit(function, environment);
        })
    };
    let tail_writes = member_reads_pointer(&mut |function, environment| {
        let materialize = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[3] = instruction(
            TRAIL,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(9),
            },
            &materialize,
            &[POINTER],
        );
    });
    assert_eq!(
        relocate(&tail_writes, &environment, RUN_A, RUN_B, T_HEAD).unwrap_err(),
        ForkRunRelocationError::UnsupportedPair
    );
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
    relocate(&arm_writes, &environment, RUN_A, RUN_B, T_HEAD).unwrap();
    assert_eq!(
        relocate(&arm_writes, &environment, RUN_A, RUN_B, T_TAIL).unwrap_err(),
        ForkRunRelocationError::UnsupportedPair
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
        function.blocks[0].instructions[3] = instruction(
            TRAIL,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(9),
            },
            &materialize,
            &[R_MOVE_B],
        );
    });
    assert_eq!(
        relocate(&tail_writes, &environment, RUN_A, RUN_B, T_HEAD).unwrap_err(),
        ForkRunRelocationError::UnsupportedPair
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
            &[R_MOVE_A],
        );
    });
    assert_eq!(
        relocate(&arm_writes, &environment, RUN_A, RUN_B, T_TAIL).unwrap_err(),
        ForkRunRelocationError::UnsupportedPair
    );
    relocate(&arm_writes, &environment, RUN_A, RUN_B, T_HEAD).unwrap();
}

/// Condition state couples like registers through the fork: the branch
/// terminator reads the target's condition units, so a flag-publishing
/// member can never leave; a flag-reading member cannot cross a flag
/// writer in its own tail or the arm prefix; and a flag consumer in the
/// arm does not block a flag-inert run.
#[test]
fn condition_state_couples_through_the_fork() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let subtract_kind = SelectedInstructionKind::ExactSubtractI64 {
        obligation: ObligationId::new(11).unwrap(),
        accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes([7; 32]),
    };
    // A flag-publishing member rewrites the units the branch terminator
    // itself reads: the member's definition would land after the read —
    // in either run position.
    for index in [1usize, 2] {
        let flag_writer = mutated(target, |function, environment| {
            let subtract = environment
                .constraint(environment.selected_keys().subtract_i64)
                .unwrap()
                .clone();
            let id = function.blocks[0].instructions[index].id;
            let output = function.blocks[0].instructions[index].operands[0].virtual_register;
            function.blocks[0].instructions[index] =
                instruction(id, subtract_kind, &subtract, &[POINTER, output, output]);
        });
        assert_eq!(
            relocate(&flag_writer, &environment, RUN_A, RUN_B, T_HEAD).unwrap_err(),
            ForkRunRelocationError::UnsupportedPair,
            "flag writer at index {index}"
        );
    }
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
            function.blocks[0].instructions[2] = instruction(
                RUN_B,
                SelectedInstructionKind::MaterializeBooleanEqual,
                &boolean,
                &[R_MOVE_B],
            );
            edit(function, environment);
        })
    };
    let tail_writes_flags = member_reads_flags(&mut |function, environment| {
        let subtract = environment
            .constraint(environment.selected_keys().subtract_i64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[3] = instruction(
            TRAIL,
            subtract_kind,
            &subtract,
            &[POINTER, R_TRAIL, R_TRAIL],
        );
    });
    assert_eq!(
        relocate(&tail_writes_flags, &environment, RUN_A, RUN_B, T_HEAD).unwrap_err(),
        ForkRunRelocationError::UnsupportedPair
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
    relocate(&arm_writes_flags, &environment, RUN_A, RUN_B, T_HEAD).unwrap();
    assert_eq!(
        relocate(&arm_writes_flags, &environment, RUN_A, RUN_B, T_TAIL).unwrap_err(),
        ForkRunRelocationError::UnsupportedPair
    );
}

/// The branch terminator is a crossed position, not a window barrier: a
/// register the branch reads couples like any crossed read. The landing
/// arm's own terminator is never crossed — the run lands inside the body
/// — so its reads do not bound the move.
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
            virtual_register: R_MOVE_A,
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
        relocate(&terminator_reads, &environment, RUN_A, RUN_B, T_HEAD).unwrap_err(),
        ForkRunRelocationError::UnsupportedPair
    );
    // The landing arm's terminator is past the landing index, never a
    // crossed position — its read of a member's result admits.
    let arm_terminator_reads = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        let class = function.blocks[0].instructions[0].operands[0].class;
        let mut jump = instruction(T_JUMP, SelectedInstructionKind::Jump, &jump_row, &[]);
        jump.operands.push(SelectedOperand {
            operand: 0,
            virtual_register: R_MOVE_B,
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
    relocate(&arm_terminator_reads, &environment, RUN_A, RUN_B, T_HEAD).unwrap();
}

/// The landing edge's register transports sit between the run's old and
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
    let member_writes = |index: usize, register: VirtualRegisterId| {
        move |function: &mut SelectedFunction,
              environment: &register_environment::ValidatedTargetRegisterEnvironment| {
            let materialize = environment
                .constraint(environment.selected_keys().materialize_i64)
                .unwrap()
                .clone();
            let id = function.blocks[0].instructions[index].id;
            function.blocks[0].instructions[index] = instruction(
                id,
                SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(9),
                },
                &materialize,
                &[register],
            );
        }
    };
    let member_reads = |index: usize, register: VirtualRegisterId| {
        move |function: &mut SelectedFunction,
              environment: &register_environment::ValidatedTargetRegisterEnvironment| {
            let copy = environment
                .constraint(environment.selected_keys().copy_i64)
                .unwrap()
                .clone();
            let id = function.blocks[0].instructions[index].id;
            let output = match index {
                1 => R_MOVE_A,
                _ => R_MOVE_B,
            };
            function.blocks[0].instructions[index] = instruction(
                id,
                SelectedInstructionKind::CopyI64,
                &copy,
                &[register, output],
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
        RUN_A,
        RUN_B,
        T_HEAD,
    )
    .unwrap();
    relocate(
        &on_landing_edge(&mut member_reads(1, POINTER)),
        &environment,
        RUN_A,
        RUN_B,
        T_HEAD,
    )
    .unwrap();
    for index in [1usize, 2] {
        assert_eq!(
            relocate(
                &on_landing_edge(&mut member_writes(index, POINTER)),
                &environment,
                RUN_A,
                RUN_B,
                T_HEAD,
            )
            .unwrap_err(),
            ForkRunRelocationError::UnsupportedPair,
            "member at {index} writing the transported argument"
        );
        assert_eq!(
            relocate(
                &on_landing_edge(&mut member_writes(index, R_BOUND)),
                &environment,
                RUN_A,
                RUN_B,
                T_HEAD,
            )
            .unwrap_err(),
            ForkRunRelocationError::UnsupportedPair,
            "member at {index} writing the parameter"
        );
        assert_eq!(
            relocate(
                &on_landing_edge(&mut member_reads(index, R_BOUND)),
                &environment,
                RUN_A,
                RUN_B,
                T_HEAD,
            )
            .unwrap_err(),
            ForkRunRelocationError::UnsupportedPair,
            "member at {index} reading the parameter"
        );
    }
    // On the skipped edge a transport argument reading a member's write
    // is a stale observation on a path the run never ran; a parameter
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
            &on_skipped_edge(&mut || binding(R_MOVE_A, R_BOUND)),
            &environment,
            RUN_A,
            RUN_B,
            T_HEAD,
        )
        .unwrap_err(),
        ForkRunRelocationError::UnsupportedPair
    );
    relocate(
        &on_skipped_edge(&mut || binding(POINTER, R_MOVE_A)),
        &environment,
        RUN_A,
        RUN_B,
        T_HEAD,
    )
    .unwrap();
}

/// The fork must be a real fork for this move: the run's block needs a
/// two-successor conditional terminator, the destination must name a
/// position inside a block the branch reaches, the arm's only
/// predecessors are that branch's edges, and the arm must be a plain
/// source block — never the run's own block, the entry block, or an
/// implementation block.
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
        relocate(&jumped, &environment, RUN_A, RUN_B, T_HEAD).unwrap_err(),
        ForkRunRelocationError::UnsupportedPair
    );
    // The destination must name a position in a branch target: a member
    // of the run's own block, a join position or its terminator-carried
    // instruction, the branch terminator itself, and a dangling id
    // refuse.
    let source = fixture(target);
    for destination in [LEAD, HEAD, RET, BRANCH, SelectedInstructionId(99)] {
        assert_eq!(
            relocate(&source, &environment, RUN_A, RUN_B, destination).unwrap_err(),
            ForkRunRelocationError::UnsupportedPair,
            "destination {destination:?}"
        );
    }
    // A second predecessor into the arm gives its stream a run that
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
        relocate(&extra_pred, &environment, RUN_A, RUN_B, T_HEAD).unwrap_err(),
        ForkRunRelocationError::UnsupportedPair
    );
    // An implementation-origin arm carries boundary work the bounded
    // audit does not cross.
    let cased = mutated(target, |function, _| {
        function.blocks[1].origin = SelectedBlockOrigin::CaseDispatch {
            source: BlockId::new(2).unwrap(),
            case_ordinal: 0,
        };
    });
    assert_eq!(
        relocate(&cased, &environment, RUN_A, RUN_B, T_HEAD).unwrap_err(),
        ForkRunRelocationError::UnsupportedPair
    );
    // An arm that is the run's own block names an in-block move, and an
    // arm that is the entry block was reached with no predecessor at
    // all.
    let self_edge = mutated(target, |function, _| {
        let successor = match &mut function.blocks[0].terminator {
            SelectedTerminator::ConditionalBranch { when_nonzero, .. } => when_nonzero,
            _ => unreachable!(),
        };
        successor.block = BLOCK_B;
        successor.source_target = BlockId::new(1).unwrap();
    });
    assert_eq!(
        relocate(&self_edge, &environment, RUN_A, RUN_B, LEAD).unwrap_err(),
        ForkRunRelocationError::UnsupportedPair
    );
}

/// Only a plain semantic edge carries the run into the arm: a
/// continuation role, case custody, per-edge fuel, or a live structural
/// transport on a landing edge all refuse. The skipped edge is never
/// crossed, so fuel and inert structural payloads on it stay free —
/// while a case payload or structural binding that reads a live member
/// definition is a dead-path observation and refuses.
#[test]
fn only_plain_semantic_edges_carry_the_run() {
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
            relocate(&continued, &environment, RUN_A, RUN_B, T_HEAD).unwrap_err(),
            ForkRunRelocationError::UnsupportedPair
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
        relocate(&case_edge, &environment, RUN_A, RUN_B, T_HEAD).unwrap_err(),
        ForkRunRelocationError::UnsupportedPair
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
        relocate(&fueled, &environment, RUN_A, RUN_B, T_HEAD).unwrap_err(),
        ForkRunRelocationError::UnsupportedPair
    );
    // A live structural transport on the landing edge moves a stored
    // value the run would cross.
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
        relocate(&structural, &environment, RUN_A, RUN_B, T_HEAD).unwrap_err(),
        ForkRunRelocationError::UnsupportedPair
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
    relocate(&fueled_skipped, &environment, RUN_A, RUN_B, T_HEAD).unwrap();
    // A case payload on the skipped edge whose transport reads a member's
    // definition is a dead-path observation; an `Unused` payload moves
    // nothing.
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
                    argument: R_MOVE_B,
                    parameter: R_BOUND,
                },
            }],
            trivial_affine_discards: Vec::new(),
        });
    });
    assert_eq!(
        relocate(&case_reads_member, &environment, RUN_A, RUN_B, T_HEAD).unwrap_err(),
        ForkRunRelocationError::UnsupportedPair
    );
    // A structural binding on the skipped edge reads its argument
    // register at the boundary — reading a member's definition refuses.
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
                    argument: R_MOVE_A,
                    destination: selected_instructions::LocalStorageSlotId::Spill {
                        register: R_BOUND,
                    },
                    byte_size: 8,
                    alignment: 8,
                },
            });
    });
    assert_eq!(
        relocate(&structural_reads_member, &environment, RUN_A, RUN_B, T_HEAD).unwrap_err(),
        ForkRunRelocationError::UnsupportedPair
    );
}

/// Barrier kinds and call-roster entries refuse as a run member or
/// anywhere inside the crossed window — the block tail and the arm prefix
/// included — while a barrier sitting at the landing index is never
/// crossed and a call-roster row on the skipped side still executes on
/// its own path.
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
        for index in [1usize, 2] {
            let member_barrier = mutated(target, |function, _| {
                function.blocks[0].instructions[index].kind = kind;
            });
            assert_eq!(
                relocate(&member_barrier, &environment, RUN_A, RUN_B, T_HEAD).unwrap_err(),
                ForkRunRelocationError::UnsupportedInstruction,
                "member at {index} {kind:?}"
            );
        }
        let tail_barrier = mutated(target, |function, _| {
            function.blocks[0].instructions[3].kind = kind;
        });
        assert_eq!(
            relocate(&tail_barrier, &environment, RUN_A, RUN_B, T_HEAD).unwrap_err(),
            ForkRunRelocationError::UnsupportedInstruction,
            "crossed tail {kind:?}"
        );
        let arm_barrier = mutated(target, |function, _| {
            function.blocks[1].instructions[0].kind = kind;
        });
        assert_eq!(
            relocate(&arm_barrier, &environment, RUN_A, RUN_B, T_TAIL).unwrap_err(),
            ForkRunRelocationError::UnsupportedInstruction,
            "crossed arm {kind:?}"
        );
        // A barrier at the landing index is never crossed.
        relocate(&arm_barrier, &environment, RUN_A, RUN_B, T_HEAD).unwrap();
        // A barrier on the skipped side still runs on its own path; it
        // only refuses if it would read a still-live member definition.
        let skipped_barrier = mutated(target, |function, _| {
            function.blocks[2].instructions[0].kind = kind;
        });
        relocate(&skipped_barrier, &environment, RUN_A, RUN_B, T_HEAD).unwrap();
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
        (RUN_A, T_HEAD),
        (RUN_B, T_HEAD),
        (TRAIL, T_HEAD),
        (T_HEAD, T_TAIL),
        (BRANCH, T_HEAD),
    ] {
        let contract = mutated(target, |function, _| {
            function.calls.push(call_contract(instruction_id));
        });
        assert_eq!(
            relocate(&contract, &environment, RUN_A, RUN_B, destination).unwrap_err(),
            ForkRunRelocationError::UnsupportedInstruction,
            "call roster {instruction_id:?}"
        );
    }
    // The arm terminator is outside the window: a call-roster row there
    // is a dead-path position like any other.
    let contract = mutated(target, |function, _| {
        function.calls.push(call_contract(T_JUMP));
    });
    relocate(&contract, &environment, RUN_A, RUN_B, T_HEAD).unwrap();
}

/// Only pure register and condition-state work may sink: a member
/// carrying a memory roster row would run its access only on the landing
/// path, and a row-less load or private-slot store sheds the same access
/// — so every memory-touching or potentially-faulting kind refuses as a
/// run member, while an accounted or unaccounted access on the skipped
/// paths never moves.
#[test]
fn run_must_be_pure_work() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A roster-carrying member's recorded access would become conditional
    // — in either run position.
    for index in [1usize, 2] {
        let roster_member = mutated(target, |function, environment| {
            let load = environment
                .constraint(environment.selected_keys().load8.unwrap())
                .unwrap()
                .clone();
            let id = function.blocks[0].instructions[index].id;
            let output = function.blocks[0].instructions[index].operands[0].virtual_register;
            function.blocks[0].instructions[index] = instruction(
                id,
                SelectedInstructionKind::Load8 { byte_offset: 0 },
                &load,
                &[POINTER, output],
            );
            function.memory_accesses.push(access(
                id,
                PlaceId::new(1).unwrap(),
                SelectedMemoryAccessRole::ReadPlace,
            ));
        });
        assert_eq!(
            relocate(&roster_member, &environment, RUN_A, RUN_B, T_HEAD).unwrap_err(),
            ForkRunRelocationError::UnsupportedInstruction,
            "roster member at index {index}"
        );
    }
    // A row-less load still performs an access whose absence the skipped
    // path would observe through its result — and any fault it carried.
    let rowless_load = mutated(target, |function, environment| {
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
    });
    assert_eq!(
        relocate(&rowless_load, &environment, RUN_A, RUN_B, T_HEAD).unwrap_err(),
        ForkRunRelocationError::UnsupportedInstruction
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
            RUN_A,
            SelectedInstructionKind::Store64 {
                slot: selected_instructions::FrameStorageSlotId::Local(
                    selected_instructions::LocalStorageSlotId::Spill { register: R_BOUND },
                ),
                byte_offset: 0,
            },
            &store,
            &[POINTER, R_MOVE_A],
        );
    });
    assert_eq!(
        relocate(&rowless_store, &environment, RUN_A, RUN_B, T_HEAD).unwrap_err(),
        ForkRunRelocationError::UnsupportedInstruction
    );
    // A potentially-faulting kind ran on every traversal before the move:
    // the divide would vanish from the skipped paths — and could fault.
    let exact_divide = mutated(target, |function, environment| {
        let divide = environment
            .constraint(environment.selected_keys().divide_u64)
            .unwrap()
            .clone();
        function.blocks[0].instructions[2] = instruction(
            RUN_B,
            SelectedInstructionKind::ExactDivideU64 {
                obligation: ObligationId::new(11).unwrap(),
                accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                    [7; 32],
                ),
            },
            &divide,
            &[POINTER, R_MOVE_B, R_MOVE_B],
        );
    });
    assert_eq!(
        relocate(&exact_divide, &environment, RUN_A, RUN_B, T_HEAD).unwrap_err(),
        ForkRunRelocationError::UnsupportedInstruction
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
    relocate(&skipped_store, &environment, RUN_A, RUN_B, T_HEAD).unwrap();
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
            &[R_MOVE_A, R_FHEAD],
        );
        function.memory_accesses.push(access(
            F_HEAD,
            PlaceId::new(2).unwrap(),
            SelectedMemoryAccessRole::WritePlace,
        ));
    });
    assert_eq!(
        relocate(
            &skipped_store_reads_member,
            &environment,
            RUN_A,
            RUN_B,
            T_HEAD
        )
        .unwrap_err(),
        ForkRunRelocationError::UnsupportedPair
    );
}

/// Every location a member writes must be dead — unread until rewritten —
/// on every path the branch's other edges reach: a reader in the skipped
/// arm, in a deeper block, or in the shared join reached through the
/// skipped side all refuse, while writes that retire the stale
/// definitions before any reader admit. A loop back through the run's
/// own block scans it without the run's slots.
#[test]
fn run_writes_must_die_on_the_skipped_paths() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let reads_member = |block: usize, index: usize, input: VirtualRegisterId| {
        move |function: &mut SelectedFunction,
              environment: &register_environment::ValidatedTargetRegisterEnvironment| {
            let copy = environment
                .constraint(environment.selected_keys().copy_i64)
                .unwrap()
                .clone();
            let id = function.blocks[block].instructions[index].id;
            let output = function.blocks[block].instructions[index].operands[0].virtual_register;
            function.blocks[block].instructions[index] = instruction(
                id,
                SelectedInstructionKind::CopyI64,
                &copy,
                &[input, output],
            );
        }
    };
    // A reader anywhere on the skipped side observes the stale
    // definition — either member's.
    for (block, index, input, label) in [
        (2usize, 0usize, R_MOVE_A, "skipped arm"),
        (2, 1, R_MOVE_B, "skipped arm tail"),
        (3, 0, R_MOVE_A, "join via the skipped arm"),
        (3, 2, R_MOVE_B, "join tail via the skipped arm"),
    ] {
        let reading = mutated(target, reads_member(block, index, input));
        assert_eq!(
            relocate(&reading, &environment, RUN_A, RUN_B, T_HEAD).unwrap_err(),
            ForkRunRelocationError::UnsupportedPair,
            "{label}"
        );
    }
    // The skipped arm's terminator reading a member's write refuses just
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
            virtual_register: R_MOVE_A,
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
        relocate(&terminator_reads, &environment, RUN_A, RUN_B, T_HEAD).unwrap_err(),
        ForkRunRelocationError::UnsupportedPair
    );
    // Writes retiring the stale definitions before any reader admit —
    // the readers then observe the rewrites they always saw on that
    // path.
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
            &[R_MOVE_A],
        );
        function.blocks[2].instructions[1] = instruction(
            F_TAIL,
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(10),
            },
            &materialize,
            &[R_MOVE_B],
        );
        function.blocks[3].instructions[0] = instruction(
            HEAD,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[R_MOVE_A, R_HEAD],
        );
        function.blocks[3].instructions[2] = instruction(
            TAIL,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[R_MOVE_B, R_TAIL],
        );
    });
    relocate(&retired, &environment, RUN_A, RUN_B, T_HEAD).unwrap();
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
                    &[R_MOVE_B, R_MID],
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
        relocate(&deeper, &environment, RUN_A, RUN_B, T_HEAD).unwrap_err(),
        ForkRunRelocationError::UnsupportedPair
    );
    // A loop back through the run's own block scans the vacated stream:
    // a prefix read of a still-live definition refuses, while a prefix
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
            &[R_MOVE_A, R_LEAD],
        );
    });
    assert_eq!(
        relocate(&looped_read, &environment, RUN_A, RUN_B, T_HEAD).unwrap_err(),
        ForkRunRelocationError::UnsupportedPair
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
            &[R_MOVE_A],
        );
    });
    relocate(&looped_retired, &environment, RUN_A, RUN_B, T_HEAD).unwrap();
}

/// A boundary settlement observes the executed prefix at its position:
/// a settlement inside the run's span or past the run's first index in
/// its own block observed a member inside that executed prefix, and one
/// past the landing index in the arm newly observes the run there;
/// settlements on the skipped paths never had the run in their streams.
#[test]
fn boundary_settlements_bound_the_window() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // In the run's block, settlements at or before the run's first index
    // admit; positions inside the span or past it observed a member in
    // the executed prefix.
    for (position, admits) in [(0u32, true), (1, true), (2, false), (3, false), (4, false)] {
        let settled = mutated(target, |function, _| {
            function
                .boundary_settlements
                .push(settlement(BLOCK_B, position, 50));
        });
        let result = relocate(&settled, &environment, RUN_A, RUN_B, T_HEAD);
        assert_eq!(
            result.is_ok(),
            admits,
            "source-block settlement at {position}"
        );
    }
    // In the arm the bound is the landing index: at or before it the
    // executed prefix is unchanged; past it the run joins the prefix.
    for (position, admits) in [(0u32, true), (1, true), (2, false)] {
        let settled = mutated(target, |function, _| {
            function
                .boundary_settlements
                .push(settlement(BLOCK_T, position, 51));
        });
        let result = relocate(&settled, &environment, RUN_A, RUN_B, T_TAIL);
        assert_eq!(result.is_ok(), admits, "arm settlement at {position}");
    }
    // Landing at the body end keeps every arm settlement: none sits past
    // the run's new index.
    let settled = mutated(target, |function, _| {
        function
            .boundary_settlements
            .push(settlement(BLOCK_T, 2, 53));
    });
    relocate(&settled, &environment, RUN_A, RUN_B, T_JUMP).unwrap();
    // Settlements on the skipped side and in the join never observe the
    // run — it was never in those streams.
    for (block, position) in [(BLOCK_F, 0u32), (BLOCK_F, 2), (BLOCK_J, 0), (BLOCK_J, 3)] {
        let settled = mutated(target, |function, _| {
            function
                .boundary_settlements
                .push(settlement(block, position, 54));
        });
        relocate(&settled, &environment, RUN_A, RUN_B, T_HEAD).unwrap();
    }
}

/// The triple must name one contiguous run in the branch block's body
/// and one position in a block the branch reaches: an unknown member, a
/// wrong function, a destination outside the branch targets, a run
/// whose own block lacks the two-successor terminator, and a terminator
/// id as member all refuse.
#[test]
fn only_the_named_fork_window_relocates() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // An unknown member or function index never locates the window.
    assert_eq!(
        relocate_selected_run_into_arm(
            &source,
            0,
            SelectedInstructionId(99),
            RUN_B,
            T_HEAD,
            &environment,
            budget(),
        )
        .unwrap_err(),
        ForkRunRelocationError::SourceMismatch
    );
    assert_eq!(
        relocate_selected_run_into_arm(&source, 9, RUN_A, RUN_B, T_HEAD, &environment, budget(),)
            .unwrap_err(),
        ForkRunRelocationError::SourceMismatch
    );
    // A run in a block without the branch names no window: the join's own
    // members have only a return terminator, and an arm's block ends in
    // `Jump`, not the two-successor form.
    assert_eq!(
        relocate(&source, &environment, HEAD, MID, T_HEAD).unwrap_err(),
        ForkRunRelocationError::UnsupportedPair
    );
    assert_eq!(
        relocate(&source, &environment, T_HEAD, T_TAIL, HEAD).unwrap_err(),
        ForkRunRelocationError::UnsupportedPair
    );
    // Naming the branch block's terminator as member is not a body
    // position; naming it as destination names no branch target.
    assert_eq!(
        relocate(&source, &environment, BRANCH, TRAIL, T_HEAD).unwrap_err(),
        ForkRunRelocationError::SourceMismatch
    );
    assert_eq!(
        relocate(&source, &environment, RUN_A, RUN_B, BRANCH).unwrap_err(),
        ForkRunRelocationError::UnsupportedPair
    );
}

/// Replay consumes only the exact move: a proposal that drops a member,
/// lands the run anywhere else, permutes the crossed positions, or
/// carries an unrelated edit all reject.
#[test]
fn replay_rejects_anything_but_the_move() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let result = relocate(&source, &environment, RUN_A, RUN_B, T_TAIL).unwrap();
    // The honest proposal replays.
    validate_fork_run_relocation(
        &source,
        0,
        RUN_A,
        RUN_B,
        T_TAIL,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // The run at the wrong index rejects.
    let mut displaced = result.transformed().clone();
    let run: Vec<_> = displaced.functions[0].blocks[1]
        .instructions
        .drain(1..3)
        .collect();
    displaced.functions[0].blocks[1]
        .instructions
        .splice(2..2, run);
    assert_eq!(
        validate_fork_run_relocation(
            &source,
            0,
            RUN_A,
            RUN_B,
            T_TAIL,
            &environment,
            budget(),
            displaced
        )
        .unwrap_err(),
        ForkRunRelocationError::ReplayMismatch
    );
    // The run left in its own block rejects.
    let mut unmoved = result.transformed().clone();
    let run: Vec<_> = unmoved.functions[0].blocks[1]
        .instructions
        .drain(1..3)
        .collect();
    unmoved.functions[0].blocks[0]
        .instructions
        .splice(1..1, run);
    assert_eq!(
        validate_fork_run_relocation(
            &source,
            0,
            RUN_A,
            RUN_B,
            T_TAIL,
            &environment,
            budget(),
            unmoved
        )
        .unwrap_err(),
        ForkRunRelocationError::ReplayMismatch
    );
    // A dropped instruction in the landing arm rejects.
    let mut dropped = result.transformed().clone();
    dropped.functions[0].blocks[1].instructions.pop();
    assert_eq!(
        validate_fork_run_relocation(
            &source,
            0,
            RUN_A,
            RUN_B,
            T_TAIL,
            &environment,
            budget(),
            dropped
        )
        .unwrap_err(),
        ForkRunRelocationError::ReplayMismatch
    );
    // An unrelated literal edit inside the source block rejects.
    let mut edited = result.transformed().clone();
    edited.functions[0].blocks[0].instructions[0].kind = SelectedInstructionKind::MaterializeI64 {
        value: IntegerValue::Unsigned(12),
    };
    assert_eq!(
        validate_fork_run_relocation(
            &source,
            0,
            RUN_A,
            RUN_B,
            T_TAIL,
            &environment,
            budget(),
            edited
        )
        .unwrap_err(),
        ForkRunRelocationError::ReplayMismatch
    );
    // Naming a different window on the same proposal re-derives a
    // different landing and rejects.
    assert_eq!(
        validate_fork_run_relocation(
            &source,
            0,
            RUN_A,
            RUN_B,
            T_HEAD,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap_err(),
        ForkRunRelocationError::ReplayMismatch
    );
}

/// The bounded audit is measured: the fork window prices every scan,
/// crossed member-against-position surface pair, roster row, and the
/// dead-path fixpoint bound against the work budget, and a budget one
/// step short refuses rather than skimping. Landing at `T_TAIL` crosses
/// one more arm position per member than landing at `T_HEAD`.
#[test]
fn measured_validation_step_boundary() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // The member-locate scan prices every block's body plus terminator
    // once across the plan (5+3+3+4 = 15), and again for this function's
    // blocks (15). The crossed surfaces pair each member (1) against
    // `TRAIL` (1) and the branch terminator (2 uses + 1 definition on
    // x86-64): (2+4) per member = 12 steps. The dead-path bound prices
    // each block's body, terminator, and edge surfaces once per run
    // location plus the initial scan: on x86-64 the materializations cost
    // 1 each, the jumps 2, the branch 3, and the return 9 —
    // (4+3)+(2+2)+(2+2)+(3+9) = 27 — times two written member registers
    // plus one: 27*3 = 81.
    let steps: u64 = 15 + 15 + 12 + 81;
    let exact = OptimizationWorkBudget::new(1, 1, steps, 1, 1).unwrap();
    relocate_selected_run_into_arm(&source, 0, RUN_A, RUN_B, T_HEAD, &environment, exact).unwrap();
    let starved = OptimizationWorkBudget::new(1, 1, steps - 1, 1, 1).unwrap();
    assert_eq!(
        relocate_selected_run_into_arm(&source, 0, RUN_A, RUN_B, T_HEAD, &environment, starved,)
            .unwrap_err(),
        ForkRunRelocationError::WorkBudgetExceeded
    );
    // Landing one position deeper crosses the arm head's surface pair per
    // member.
    let exact = OptimizationWorkBudget::new(1, 1, steps + 4, 1, 1).unwrap();
    relocate_selected_run_into_arm(&source, 0, RUN_A, RUN_B, T_TAIL, &environment, exact).unwrap();
    let starved = OptimizationWorkBudget::new(1, 1, steps + 3, 1, 1).unwrap();
    assert_eq!(
        relocate_selected_run_into_arm(&source, 0, RUN_A, RUN_B, T_TAIL, &environment, starved,)
            .unwrap_err(),
        ForkRunRelocationError::WorkBudgetExceeded
    );
}

/// The environment's target must be the plan's target: a relocation proven
/// for one target's constraints is not evidence on another.
#[test]
fn target_mismatch_rejects() {
    let source = fixture(NativeTarget::linux_x64());
    let environment = baseline_target_register_environment(NativeTarget::linux_arm64()).unwrap();
    assert_eq!(
        relocate(&source, &environment, RUN_A, RUN_B, T_HEAD).unwrap_err(),
        ForkRunRelocationError::SourceMismatch
    );
}

/// Two runs over the identical source produce the identical validated
/// result, and the published plan is a legal second input through the
/// sealed analysis boundary: the tail member sinks through the same fork
/// onto it, a hazard-coupled member still declines, and an in-block
/// family still admits.
#[test]
fn fork_run_relocation_is_deterministic_and_re_admitted() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let first = relocate(&source, &environment, RUN_A, RUN_B, T_HEAD).unwrap();
    let second = relocate(&source, &environment, RUN_A, RUN_B, T_HEAD).unwrap();
    assert_eq!(first, second);
    // The validated output carries the sealed analysis boundary, so it is
    // a legal second input — not merely a reconstruction of one. The tail
    // member sinks through the same fork onto it.
    let again =
        relocate_selected_run_into_arm(&first, 0, LEAD, TRAIL, T_TAIL, &environment, budget())
            .unwrap();
    assert_eq!(
        block_order(&again.transformed().functions[0].blocks[1]),
        vec![RUN_A, RUN_B, T_HEAD, LEAD, TRAIL, T_TAIL]
    );
    assert_eq!(
        again.receipt().source_selected(),
        first.receipt().transformed_selected()
    );
    // A hazard-coupled move still declines on the second input: once
    // `RUN_A` and `RUN_B` sit at T's head, `T_HEAD` — reading `R_TRAIL`
    // ahead of `T_TAIL` — refuses to let `TRAIL` cross it onto `T_TAIL`'s
    // position.
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
    let moved = relocate(&coupled, &environment, RUN_A, RUN_B, T_HEAD).unwrap();
    assert_eq!(
        crate::relocate_selected_instruction_into_arm(
            &moved,
            0,
            TRAIL,
            T_TAIL,
            &environment,
            budget(),
        )
        .unwrap_err(),
        crate::ForkRelocationError::UnsupportedPair
    );
    // A different scheduling family still admits on the second input.
    let swapped =
        crate::relocate_selected_instruction(&first, 0, MID, TAIL, &environment, budget()).unwrap();
    assert_eq!(
        block_order(&swapped.transformed().functions[0].blocks[3]),
        vec![HEAD, TAIL, MID]
    );
}
