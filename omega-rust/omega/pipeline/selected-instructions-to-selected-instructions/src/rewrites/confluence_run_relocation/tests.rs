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
    ConfluenceRunRelocationError, ConfluenceRunRelocationReceipt, ValidatedConfluenceRunRelocation,
    relocate_selected_run_into_confluence, validate_confluence_run_relocation,
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
const T_HEAD: SelectedInstructionId = SelectedInstructionId(4);
const RUN_A: SelectedInstructionId = SelectedInstructionId(5);
const RUN_B: SelectedInstructionId = SelectedInstructionId(6);
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
const DEEP: SelectedInstructionId = SelectedInstructionId(17);
const J_JUMP: SelectedInstructionId = SelectedInstructionId(18);
const DEEP_HEAD: SelectedInstructionId = SelectedInstructionId(19);

const POINTER: VirtualRegisterId = VirtualRegisterId(0);
const R_LEAD: VirtualRegisterId = VirtualRegisterId(1);
const R_TRAIL: VirtualRegisterId = VirtualRegisterId(2);
const R_THEAD: VirtualRegisterId = VirtualRegisterId(3);
const R_MOVE_A: VirtualRegisterId = VirtualRegisterId(4);
const R_MOVE_B: VirtualRegisterId = VirtualRegisterId(5);
const R_TTAIL: VirtualRegisterId = VirtualRegisterId(6);
const R_FHEAD: VirtualRegisterId = VirtualRegisterId(7);
const R_FTAIL: VirtualRegisterId = VirtualRegisterId(8);
const R_HEAD: VirtualRegisterId = VirtualRegisterId(9);
const R_MID: VirtualRegisterId = VirtualRegisterId(10);
const R_TAIL: VirtualRegisterId = VirtualRegisterId(11);
const R_BOUND: VirtualRegisterId = VirtualRegisterId(12);
const R_DEEP: VirtualRegisterId = VirtualRegisterId(13);

const BLOCK_B: SelectedBlockId = SelectedBlockId(0);
const BLOCK_T: SelectedBlockId = SelectedBlockId(1);
const BLOCK_F: SelectedBlockId = SelectedBlockId(2);
const BLOCK_J: SelectedBlockId = SelectedBlockId(3);
const BLOCK_D: SelectedBlockId = SelectedBlockId(4);
const EDGE_BT: u64 = 20;
const EDGE_BF: u64 = 21;
const EDGE_TJ: u64 = 22;
const EDGE_FJ: u64 = 23;
const EDGE_JD: u64 = 25;
const EDGE_DJ: u64 = 26;

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
/// nonzero edge leads to block T and whose zero edge leads to block F;
/// T and F each end in a `Jump` to the shared join J, whose terminator is
/// a plain return:
/// `B = [LEAD; TRAIL] -> branch -> T = [T_HEAD; RUN_A; RUN_B; T_TAIL] ->
/// jump -> J` and `F = [F_HEAD; F_TAIL] -> jump -> J = [HEAD; MID; TAIL]
/// -> return`.
/// The default move relocates the run `RUN_A..=RUN_B` onto `HEAD`'s
/// position — the head of J's body — crossing `T_TAIL`, the jump
/// terminator, and the landing edge's transports while B and F keep their
/// order. On F's inflow the run newly executes: J's tail, its return, and
/// every reachable continuation must never read `R_MOVE_A` or `R_MOVE_B`
/// before a write retires the foreign definition.
fn fixture(target: NativeTarget) -> ValidatedConfluenceRunRelocation {
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
        result_register(R_THEAD, T_HEAD, 4),
        result_register(R_MOVE_A, RUN_A, 5),
        result_register(R_MOVE_B, RUN_B, 6),
        result_register(R_TTAIL, T_TAIL, 7),
        result_register(R_FHEAD, F_HEAD, 8),
        result_register(R_FTAIL, F_TAIL, 9),
        result_register(R_HEAD, HEAD, 10),
        result_register(R_MID, MID, 11),
        result_register(R_TAIL, TAIL, 12),
        result_register(R_BOUND, LEAD, 13),
        result_register(R_DEEP, DEEP, 14),
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
                        materialization(RUN_A, R_MOVE_A, 7),
                        materialization(RUN_B, R_MOVE_B, 13),
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
    ValidatedConfluenceRunRelocation {
        receipt: ConfluenceRunRelocationReceipt {
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
) -> ValidatedConfluenceRunRelocation {
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
    source: &ValidatedConfluenceRunRelocation,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    first: SelectedInstructionId,
    last: SelectedInstructionId,
    destination: SelectedInstructionId,
) -> Result<ValidatedConfluenceRunRelocation, ConfluenceRunRelocationError> {
    relocate_selected_run_into_confluence(
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

/// The run sinks through the confluence edge onto the destination's
/// position on every target: `RUN_A` and `RUN_B` leave T's body in their
/// own order, `T_HEAD` and `T_TAIL` keep their order behind them, the run
/// lands at J's head with `HEAD`, `MID`, `TAIL`, and the return
/// terminator untouched, and the other inflow's block keeps its order —
/// identity, kind, operands, and provenance intact. The replayed proposal
/// restores the source bit-identically.
#[test]
fn run_relocates_into_the_join() {
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
        assert_eq!(
            block_order(&moved.blocks[3]),
            vec![RUN_A, RUN_B, HEAD, MID, TAIL]
        );
        assert_eq!(block_order(&moved.blocks[1]), vec![T_HEAD, T_TAIL]);
        assert_eq!(
            moved.blocks[0].instructions,
            original.blocks[0].instructions
        );
        assert_eq!(
            moved.blocks[2].instructions,
            original.blocks[2].instructions
        );
        // The members moved bit-identically; every terminator, edge, and
        // roster stayed untouched.
        assert_eq!(
            moved.blocks[3].instructions[0],
            original.blocks[1].instructions[1]
        );
        assert_eq!(
            moved.blocks[3].instructions[1],
            original.blocks[1].instructions[2]
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
        validate_confluence_run_relocation(
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

/// The destination names the landing position directly: a join body
/// instruction puts the run on its index, and the join's
/// terminator-carried instruction lands the run at the body end. Either
/// inflow's run sinks — `F_HEAD..=F_TAIL` leaves the zero arm's whole
/// body onto `HEAD`'s position while T's edge becomes the other inflow
/// whose traversals speculate.
#[test]
fn run_lands_at_the_named_position() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let result = relocate(&source, &environment, RUN_A, RUN_B, MID).unwrap();
    assert_eq!(
        block_order(&result.transformed().functions[0].blocks[3]),
        vec![HEAD, RUN_A, RUN_B, MID, TAIL]
    );
    let result = relocate(&source, &environment, RUN_A, RUN_B, RET).unwrap();
    assert_eq!(
        block_order(&result.transformed().functions[0].blocks[3]),
        vec![HEAD, MID, TAIL, RUN_A, RUN_B]
    );
    let result = relocate(&source, &environment, F_HEAD, F_TAIL, HEAD).unwrap();
    let moved = &result.transformed().functions[0];
    assert_eq!(
        block_order(&moved.blocks[2]),
        Vec::<SelectedInstructionId>::new()
    );
    assert_eq!(
        block_order(&moved.blocks[3]),
        vec![F_HEAD, F_TAIL, HEAD, MID, TAIL]
    );
}

/// Any contiguous run of at least two members sinks: the block-tail run
/// crosses only the terminator and edge, the block-head run crosses its
/// trailing body, and the whole body leaves the block empty — a run of
/// one member is the sibling family's case.
#[test]
fn tail_and_head_runs_relocate() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let tail = relocate(&source, &environment, RUN_B, T_TAIL, HEAD).unwrap();
    assert_eq!(
        block_order(&tail.transformed().functions[0].blocks[1]),
        vec![T_HEAD, RUN_A]
    );
    assert_eq!(
        block_order(&tail.transformed().functions[0].blocks[3]),
        vec![RUN_B, T_TAIL, HEAD, MID, TAIL]
    );
    let head = relocate(&source, &environment, T_HEAD, RUN_A, HEAD).unwrap();
    assert_eq!(
        block_order(&head.transformed().functions[0].blocks[1]),
        vec![RUN_B, T_TAIL]
    );
    assert_eq!(
        block_order(&head.transformed().functions[0].blocks[3]),
        vec![T_HEAD, RUN_A, HEAD, MID, TAIL]
    );
    let whole = relocate(&source, &environment, T_HEAD, T_TAIL, HEAD).unwrap();
    assert_eq!(
        block_order(&whole.transformed().functions[0].blocks[1]),
        Vec::<SelectedInstructionId>::new()
    );
    assert_eq!(
        block_order(&whole.transformed().functions[0].blocks[3]),
        vec![T_HEAD, RUN_A, RUN_B, T_TAIL, HEAD, MID, TAIL]
    );
}

/// The two named members bound the run: a repeated id, a last member
/// that does not follow the first, and a last member outside the first's
/// block all name no multi-member run — the single-member confluence
/// relocation is the sibling family's case.
#[test]
fn the_named_members_bound_a_contiguous_run() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // One member is no run: the member-level family carries it.
    assert_eq!(
        relocate(&source, &environment, RUN_A, RUN_A, HEAD).unwrap_err(),
        ConfluenceRunRelocationError::UnsupportedPair
    );
    // Reversed names bound no span.
    assert_eq!(
        relocate(&source, &environment, RUN_B, RUN_A, HEAD).unwrap_err(),
        ConfluenceRunRelocationError::UnsupportedPair
    );
    // A last member in another block bounds no span in the first's.
    for last in [LEAD, F_HEAD, HEAD, T_JUMP, RET, SelectedInstructionId(99)] {
        assert_eq!(
            relocate(&source, &environment, RUN_A, last, HEAD).unwrap_err(),
            ConfluenceRunRelocationError::UnsupportedPair,
            "last {last:?}"
        );
    }
}

/// The run's block needs only its lone `Jump` exit — the entry block is
/// a legal inflow: `LEAD..=TRAIL` sinks out of B once B's terminator is a
/// `Jump` to the join, crossing the jump and the edge alone.
#[test]
fn entry_inflow_run_relocates() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let jumped = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        function.blocks[0].terminator = jump_terminator(
            instruction(BRANCH, SelectedInstructionKind::Jump, &jump_row, &[]),
            successor(BLOCK_J, BlockId::new(4).unwrap(), EDGE_BT),
        );
    });
    let result = relocate(&jumped, &environment, LEAD, TRAIL, HEAD).unwrap();
    let moved = &result.transformed().functions[0];
    assert_eq!(
        block_order(&moved.blocks[0]),
        Vec::<SelectedInstructionId>::new()
    );
    assert_eq!(
        block_order(&moved.blocks[3]),
        vec![LEAD, TRAIL, HEAD, MID, TAIL]
    );
}

/// The run's block must end in the lone unconditional `Jump`, its edge
/// must land on a join at least one other predecessor's edge also
/// reaches, and the destination must name a position in that join: a
/// conditional terminator keeps a second exit the run still executes on,
/// a sole-predecessor target is the single-edge family's case, a
/// self-edge is the in-block family's case, the entry block is reached
/// with no predecessor, and an implementation block's origin carries
/// boundary work the audit does not cross. The other inflow's own shape
/// is unconstrained — a conditional terminator feeding the join on both
/// edges still admits, and so does a third inflow.
#[test]
fn the_confluence_must_open() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // A run of B sits behind a conditional terminator: the branch keeps a
    // second exit the run would still execute on after the move.
    assert_eq!(
        relocate(&source, &environment, LEAD, TRAIL, HEAD).unwrap_err(),
        ConfluenceRunRelocationError::UnsupportedPair
    );
    // A destination outside the join names no landing position: the run's
    // own block mates, the other inflow's, B's, and dangling ids.
    for destination in [
        RUN_A,
        T_HEAD,
        T_TAIL,
        F_HEAD,
        LEAD,
        T_JUMP,
        SelectedInstructionId(99),
    ] {
        assert_eq!(
            relocate(&source, &environment, RUN_A, RUN_B, destination).unwrap_err(),
            ConfluenceRunRelocationError::UnsupportedPair,
            "destination {destination:?}"
        );
    }
    // The join reached by only the run's edge is the sole-predecessor
    // edge family's case — no traversal speculates.
    let sole = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        let return_row = environment
            .constraint(environment.selected_keys().return_unit)
            .unwrap()
            .clone();
        function.blocks[2].terminator = jump_terminator(
            instruction(F_JUMP, SelectedInstructionKind::Jump, &jump_row, &[]),
            successor(BLOCK_D, BlockId::new(5).unwrap(), EDGE_JD),
        );
        function.blocks.push(SelectedBlock {
            id: BLOCK_D,
            origin: SelectedBlockOrigin::Source(BlockId::new(5).unwrap()),
            instructions: Vec::new(),
            terminator: SelectedTerminator::Return {
                instruction: instruction(
                    DEEP,
                    SelectedInstructionKind::ReturnUnit,
                    &return_row,
                    &[],
                ),
                psi_return_edge: EdgeId::new(27).unwrap(),
            },
        });
    });
    assert_eq!(
        relocate(&sole, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        ConfluenceRunRelocationError::UnsupportedPair
    );
    // A self-edge is the in-block family's case with a back-edge transport
    // reading, not a confluence.
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
        relocate(&self_loop, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        ConfluenceRunRelocationError::UnsupportedPair
    );
    // The entry block is reached with no predecessor at all.
    let entry_target = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        function.blocks[1].terminator = jump_terminator(
            instruction(T_JUMP, SelectedInstructionKind::Jump, &jump_row, &[]),
            successor(BLOCK_B, BlockId::new(1).unwrap(), EDGE_TJ),
        );
    });
    assert_eq!(
        relocate(&entry_target, &environment, RUN_A, RUN_B, LEAD).unwrap_err(),
        ConfluenceRunRelocationError::UnsupportedPair
    );
    // An implementation-origin join carries boundary work the bounded
    // audit does not cross.
    let cased_join = mutated(target, |function, _| {
        function.blocks[3].origin = SelectedBlockOrigin::CaseDispatch {
            source: BlockId::new(4).unwrap(),
            case_ordinal: 0,
        };
    });
    assert_eq!(
        relocate(&cased_join, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        ConfluenceRunRelocationError::UnsupportedPair
    );
    // The other inflow's own terminator shape is unconstrained: a
    // conditional branch feeding the join on both edges still opens the
    // confluence.
    let branched_inflow = mutated(target, |function, environment| {
        let branch_row = environment
            .constraint(environment.selected_keys().conditional_branch)
            .unwrap()
            .clone();
        function.blocks[2].terminator = SelectedTerminator::ConditionalBranch {
            instruction: instruction(
                F_JUMP,
                SelectedInstructionKind::ConditionalBranchNonZero,
                &branch_row,
                &[],
            ),
            when_nonzero: successor(BLOCK_J, BlockId::new(4).unwrap(), EDGE_FJ),
            when_zero: successor(BLOCK_J, BlockId::new(4).unwrap(), EDGE_DJ),
        };
    });
    relocate(&branched_inflow, &environment, RUN_A, RUN_B, HEAD).unwrap();
    // A third inflow opens the same confluence.
    let three_way = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        function.blocks.push(SelectedBlock {
            id: BLOCK_D,
            origin: SelectedBlockOrigin::Source(BlockId::new(5).unwrap()),
            instructions: Vec::new(),
            terminator: jump_terminator(
                instruction(J_JUMP, SelectedInstructionKind::Jump, &jump_row, &[]),
                successor(BLOCK_J, BlockId::new(4).unwrap(), EDGE_JD),
            ),
        });
    });
    relocate(&three_way, &environment, RUN_A, RUN_B, HEAD).unwrap();
}

/// Only a plain semantic edge carries the run across: a continuation
/// role, case custody, per-edge fuel, or a live structural transport on
/// the crossed edge all refuse. The other inflow's edge is never crossed
/// — it runs before the landing index on its own arrivals — so fuel, a
/// continuation role, a case payload, or a structural transport on it all
/// stay free, even when they read a member's register.
#[test]
fn only_plain_semantic_edges_carry_the_run() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for role in [
        SelectedSuccessorRole::EdgeTransferContinuation,
        SelectedSuccessorRole::CaseDispatchContinuation,
    ] {
        let continued = mutated(target, |function, _| {
            let successor = match &mut function.blocks[1].terminator {
                SelectedTerminator::Jump { successor, .. } => successor,
                _ => unreachable!(),
            };
            successor.role = role;
        });
        assert_eq!(
            relocate(&continued, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
            ConfluenceRunRelocationError::UnsupportedPair
        );
    }
    let case_edge = mutated(target, |function, _| {
        let successor = match &mut function.blocks[1].terminator {
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
        ConfluenceRunRelocationError::UnsupportedPair
    );
    let fueled = mutated(target, |function, _| {
        let successor = match &mut function.blocks[1].terminator {
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
        ConfluenceRunRelocationError::UnsupportedPair
    );
    // A live structural transport on the crossed edge moves a stored value
    // the run would cross.
    let structural = mutated(target, |function, _| {
        let successor = match &mut function.blocks[1].terminator {
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
        ConfluenceRunRelocationError::UnsupportedPair
    );
    // Everything on the other inflow's edge stays free: it runs before the
    // landing index on its own arrivals and is never crossed.
    let fueled_inflow = mutated(target, |function, _| {
        let successor = match &mut function.blocks[2].terminator {
            SelectedTerminator::Jump { successor, .. } => successor,
            _ => unreachable!(),
        };
        successor.fuel.push(optimization_unit::FuelSettlement {
            site: optimization_unit::PsiProvenance::Operation(OperationId::new(30).unwrap()),
            units: 1,
        });
        successor.role = SelectedSuccessorRole::EdgeTransferContinuation;
    });
    relocate(&fueled_inflow, &environment, RUN_A, RUN_B, HEAD).unwrap();
    let cased_inflow = mutated(target, |function, _| {
        let successor = match &mut function.blocks[2].terminator {
            SelectedTerminator::Jump { successor, .. } => successor,
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
                            block: BlockId::new(4).unwrap(),
                            position: 0,
                        },
                    },
                },
                transport: selected_instructions::SelectedCasePayloadTransport::Registers {
                    argument: R_MOVE_A,
                    parameter: R_BOUND,
                },
            }],
            trivial_affine_discards: Vec::new(),
        });
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
                    argument: R_MOVE_B,
                    destination: selected_instructions::LocalStorageSlotId::Spill {
                        register: R_BOUND,
                    },
                    byte_size: 8,
                    alignment: 8,
                },
            });
    });
    relocate(&cased_inflow, &environment, RUN_A, RUN_B, HEAD).unwrap();
}

/// The crossed edge's register transports sit between the run's old and
/// new positions: a member defining the transported argument would hand
/// the binding a new value, a member defining the parameter would be
/// overwritten by it, and a member reading the parameter would observe
/// the transported value only after the move — while a member merely
/// reading the argument crosses freely. Every member meets the audit:
/// either run member writing the argument refuses on its own. The other
/// inflow's transports are never crossed — they join the boundary run
/// before the landing index on their own arrivals, whatever they read or
/// write.
#[test]
fn register_transports_bind_the_crossed_edge() {
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
    let member_writes = |index: usize, id: SelectedInstructionId, register: VirtualRegisterId| {
        move |function: &mut SelectedFunction,
              environment: &register_environment::ValidatedTargetRegisterEnvironment| {
            let materialize = environment
                .constraint(environment.selected_keys().materialize_i64)
                .unwrap()
                .clone();
            function.blocks[1].instructions[index] = instruction(
                id,
                SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(9),
                },
                &materialize,
                &[register],
            );
        }
    };
    let member_reads = |index: usize,
                        id: SelectedInstructionId,
                        register: VirtualRegisterId,
                        output: VirtualRegisterId| {
        move |function: &mut SelectedFunction,
              environment: &register_environment::ValidatedTargetRegisterEnvironment| {
            let copy = environment
                .constraint(environment.selected_keys().copy_i64)
                .unwrap()
                .clone();
            function.blocks[1].instructions[index] = instruction(
                id,
                SelectedInstructionKind::CopyI64,
                &copy,
                &[register, output],
            );
        }
    };
    let on_crossed_edge = |edit: &mut dyn FnMut(
        &mut SelectedFunction,
        &register_environment::ValidatedTargetRegisterEnvironment,
    )| {
        mutated(target, |function, environment| {
            let successor = match &mut function.blocks[1].terminator {
                SelectedTerminator::Jump { successor, .. } => successor,
                _ => unreachable!(),
            };
            successor.bindings.push(binding(POINTER, R_BOUND));
            edit(function, environment);
        })
    };
    relocate(
        &on_crossed_edge(&mut |_, _| {}),
        &environment,
        RUN_A,
        RUN_B,
        HEAD,
    )
    .unwrap();
    relocate(
        &on_crossed_edge(&mut member_reads(1, RUN_A, POINTER, R_MOVE_A)),
        &environment,
        RUN_A,
        RUN_B,
        HEAD,
    )
    .unwrap();
    // Either member defining the transported argument refuses on its own.
    for (index, id) in [(1usize, RUN_A), (2, RUN_B)] {
        assert_eq!(
            relocate(
                &on_crossed_edge(&mut member_writes(index, id, POINTER)),
                &environment,
                RUN_A,
                RUN_B,
                HEAD,
            )
            .unwrap_err(),
            ConfluenceRunRelocationError::UnsupportedPair,
            "member {id:?} writing the argument"
        );
    }
    assert_eq!(
        relocate(
            &on_crossed_edge(&mut member_writes(2, RUN_B, R_BOUND)),
            &environment,
            RUN_A,
            RUN_B,
            HEAD,
        )
        .unwrap_err(),
        ConfluenceRunRelocationError::UnsupportedPair
    );
    assert_eq!(
        relocate(
            &on_crossed_edge(&mut member_reads(2, RUN_B, R_BOUND, R_MOVE_B)),
            &environment,
            RUN_A,
            RUN_B,
            HEAD,
        )
        .unwrap_err(),
        ConfluenceRunRelocationError::UnsupportedPair
    );
    // On the other inflow's edge a transport runs before the landing index
    // on its own arrivals: a parameter writing a member's register is
    // overwritten again at the landing position, and an argument reading
    // it observes the value that inflow always carried.
    let on_inflow_edge = |binding_of: &mut dyn FnMut() -> SelectedValueBinding| {
        mutated(target, |function, _| {
            let successor = match &mut function.blocks[2].terminator {
                SelectedTerminator::Jump { successor, .. } => successor,
                _ => unreachable!(),
            };
            successor.bindings.push(binding_of());
        })
    };
    relocate(
        &on_inflow_edge(&mut || binding(POINTER, R_MOVE_A)),
        &environment,
        RUN_A,
        RUN_B,
        HEAD,
    )
    .unwrap();
    relocate(
        &on_inflow_edge(&mut || binding(R_MOVE_B, R_BOUND)),
        &environment,
        RUN_A,
        RUN_B,
        HEAD,
    )
    .unwrap();
}

/// The run moves as one body: a producer whose only crossed reader is the
/// run's own next member cannot leave its block alone — the member move
/// would starve the consumer it leaves behind — while the run carries the
/// consumer with it through the confluence.
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
    // The run admits: `RUN_B` reads `RUN_A`'s result inside the run, where
    // internal coupling never trades order.
    let moved = relocate(&source, &environment, RUN_A, RUN_B, HEAD).unwrap();
    assert_eq!(
        block_order(&moved.transformed().functions[0].blocks[3]),
        vec![RUN_A, RUN_B, HEAD, MID, TAIL]
    );
    // `RUN_A` alone cannot cross `RUN_B`, whose read of its result sits in
    // the member move's window — the single-member family refuses where
    // the run admits.
    assert_eq!(
        crate::relocate_selected_instruction_into_confluence(
            &source,
            0,
            RUN_A,
            HEAD,
            &environment,
            budget(),
        )
        .unwrap_err(),
        crate::ConfluenceRelocationError::UnsupportedPair
    );
}

/// A reader of a member's written register anywhere in the crossed run
/// keeps the old order: the run's own block tail, the `Jump` terminator
/// itself, and the join's prefix all refuse — the source program
/// observed the member's definition there. A reader before the run's
/// first index keeps the pre-run value on every path.
#[test]
fn raw_hazard_keeps_order_through_the_edge() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let reads = |block: usize,
                 index: usize,
                 id: SelectedInstructionId,
                 input: VirtualRegisterId,
                 output: VirtualRegisterId| {
        move |function: &mut SelectedFunction,
              environment: &register_environment::ValidatedTargetRegisterEnvironment| {
            let copy = environment
                .constraint(environment.selected_keys().copy_i64)
                .unwrap()
                .clone();
            function.blocks[block].instructions[index] = instruction(
                id,
                SelectedInstructionKind::CopyI64,
                &copy,
                &[input, output],
            );
        }
    };
    // The run's own block tail is crossed: either member's result read
    // there refuses.
    for input in [R_MOVE_A, R_MOVE_B] {
        let tail_reads = mutated(target, reads(1, 3, T_TAIL, input, R_TTAIL));
        assert_eq!(
            relocate(&tail_reads, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
            ConfluenceRunRelocationError::UnsupportedPair,
            "tail reading {input:?}"
        );
    }
    // The join's prefix is crossed when the run lands deeper.
    let head_reads = mutated(target, reads(3, 0, HEAD, R_MOVE_B, R_HEAD));
    assert_eq!(
        relocate(&head_reads, &environment, RUN_A, RUN_B, MID).unwrap_err(),
        ConfluenceRunRelocationError::UnsupportedPair
    );
    // A reader before the run's first index is never crossed: it kept the
    // pre-run value on either order.
    let before_reads = mutated(target, reads(1, 0, T_HEAD, R_MOVE_A, R_THEAD));
    relocate(&before_reads, &environment, RUN_A, RUN_B, HEAD).unwrap();
    // The `Jump` terminator is a crossed position: the run sinks past it,
    // so its read of a member's result refuses.
    let jump_reads = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        let class = function.blocks[1].instructions[0].operands[0].class;
        let mut jump = instruction(T_JUMP, SelectedInstructionKind::Jump, &jump_row, &[]);
        jump.operands.push(SelectedOperand {
            operand: 0,
            virtual_register: R_MOVE_A,
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
        relocate(&jump_reads, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        ConfluenceRunRelocationError::UnsupportedPair
    );
}

/// A writer of a register a member reads anywhere in the crossed run
/// would hand the member a different input at its new position: the run's
/// own block tail and the join's prefix refuse, while a writer before
/// the run's first index already published the value the member reads.
#[test]
fn war_hazard_keeps_order_through_the_edge() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let member_reads_pointer =
        |function: &mut SelectedFunction,
         environment: &register_environment::ValidatedTargetRegisterEnvironment| {
            let copy = environment
                .constraint(environment.selected_keys().copy_i64)
                .unwrap()
                .clone();
            function.blocks[1].instructions[1] = instruction(
                RUN_A,
                SelectedInstructionKind::CopyI64,
                &copy,
                &[POINTER, R_MOVE_A],
            );
        };
    let writes_pointer = |block: usize, index: usize, id: SelectedInstructionId| {
        move |function: &mut SelectedFunction,
              environment: &register_environment::ValidatedTargetRegisterEnvironment| {
            let copy = environment
                .constraint(environment.selected_keys().copy_i64)
                .unwrap()
                .clone();
            function.blocks[block].instructions[index] = instruction(
                id,
                SelectedInstructionKind::CopyI64,
                &copy,
                &[R_TRAIL, POINTER],
            );
        }
    };
    let tail_writes = mutated(target, |function, environment| {
        member_reads_pointer(function, environment);
        writes_pointer(1, 3, T_TAIL)(function, environment);
    });
    assert_eq!(
        relocate(&tail_writes, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        ConfluenceRunRelocationError::UnsupportedPair
    );
    let prefix_writes = mutated(target, |function, environment| {
        member_reads_pointer(function, environment);
        writes_pointer(3, 0, HEAD)(function, environment);
    });
    assert_eq!(
        relocate(&prefix_writes, &environment, RUN_A, RUN_B, MID).unwrap_err(),
        ConfluenceRunRelocationError::UnsupportedPair
    );
    // A writer before the run's first index already published the input:
    // the member reads the same value at its new position on this inflow's
    // path.
    let before_writes = mutated(target, |function, environment| {
        member_reads_pointer(function, environment);
        writes_pointer(1, 0, T_HEAD)(function, environment);
    });
    relocate(&before_writes, &environment, RUN_A, RUN_B, HEAD).unwrap();
}

/// A writer of a member's written register in the crossed run refuses —
/// either side would observe the other's value where the source observed
/// its own. A writer at or after the landing index simply retires the
/// member's foreign definition: readers after it see the rewrite on every
/// arrival, as they did in the source.
#[test]
fn waw_hazard_keeps_order_through_the_edge() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let writes =
        |block: usize, index: usize, id: SelectedInstructionId, output: VirtualRegisterId| {
            move |function: &mut SelectedFunction,
              environment: &register_environment::ValidatedTargetRegisterEnvironment| {
            let copy = environment
                .constraint(environment.selected_keys().copy_i64)
                .unwrap()
                .clone();
            function.blocks[block].instructions[index] = instruction(
                id,
                SelectedInstructionKind::CopyI64,
                &copy,
                &[R_TRAIL, output],
            );
        }
        };
    let tail_writes = mutated(target, writes(1, 3, T_TAIL, R_MOVE_A));
    assert_eq!(
        relocate(&tail_writes, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        ConfluenceRunRelocationError::UnsupportedPair
    );
    let prefix_writes = mutated(target, writes(3, 0, HEAD, R_MOVE_B));
    assert_eq!(
        relocate(&prefix_writes, &environment, RUN_A, RUN_B, MID).unwrap_err(),
        ConfluenceRunRelocationError::UnsupportedPair
    );
    // `TAIL` sits after the landing index: it overwrites the member's
    // register on every arrival, retiring the foreign definition before
    // any reader.
    let retires = mutated(target, writes(3, 2, TAIL, R_MOVE_A));
    relocate(&retires, &environment, RUN_A, RUN_B, HEAD).unwrap();
}

/// Condition state couples the same way registers do: a member clobbering
/// the flags refuses against a crossed flag reader — in the run's own
/// tail or the join's prefix — and against a reader at or after the
/// landing index, where the dead-path audit sees the foreign flags still
/// live; a flag reader in the run's own tail or a flag writer crossed by
/// a flag-reading member refuses the other direction. Pure flag work in
/// either direction admits when nothing crossed or reached observes it.
#[test]
fn condition_state_couples_through_the_edge() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let flag_writer = |function: &mut SelectedFunction,
                       environment: &register_environment::ValidatedTargetRegisterEnvironment,
                       block: usize,
                       index: usize,
                       id: SelectedInstructionId,
                       output: VirtualRegisterId| {
        let subtract = environment
            .constraint(environment.selected_keys().subtract_i64)
            .unwrap()
            .clone();
        function.blocks[block].instructions[index] = instruction(
            id,
            SelectedInstructionKind::ExactSubtractI64 {
                obligation: ObligationId::new(11).unwrap(),
                accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                    [7; 32],
                ),
            },
            &subtract,
            &[POINTER, R_TRAIL, output],
        );
    };
    let flag_reader = |function: &mut SelectedFunction,
                       environment: &register_environment::ValidatedTargetRegisterEnvironment,
                       block: usize,
                       index: usize,
                       id: SelectedInstructionId,
                       output: VirtualRegisterId| {
        let boolean = environment
            .constraint(environment.selected_keys().materialize_boolean)
            .unwrap()
            .clone();
        function.blocks[block].instructions[index] = instruction(
            id,
            SelectedInstructionKind::MaterializeBooleanEqual,
            &boolean,
            &[output],
        );
    };
    // A flag-writing member lands cleanly while nothing crossed or
    // reached observes the flags.
    let member_writes_flags = mutated(target, |function, environment| {
        flag_writer(function, environment, 1, 1, RUN_A, R_MOVE_A);
    });
    relocate(&member_writes_flags, &environment, RUN_A, RUN_B, HEAD).unwrap();
    // A crossed flag reader in the join's prefix refuses.
    let prefix_reads_flags = mutated(target, |function, environment| {
        flag_writer(function, environment, 1, 1, RUN_A, R_MOVE_A);
        flag_reader(function, environment, 3, 0, HEAD, R_HEAD);
    });
    assert_eq!(
        relocate(&prefix_reads_flags, &environment, RUN_A, RUN_B, MID).unwrap_err(),
        ConfluenceRunRelocationError::UnsupportedPair
    );
    // A flag reader past the landing index meets the member's foreign
    // flags still live.
    let tail_reads_flags = mutated(target, |function, environment| {
        flag_writer(function, environment, 1, 1, RUN_A, R_MOVE_A);
        flag_reader(function, environment, 3, 2, TAIL, R_TAIL);
    });
    assert_eq!(
        relocate(&tail_reads_flags, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        ConfluenceRunRelocationError::UnsupportedPair
    );
    // A flag-reading member refuses a crossed flag writer in its own tail.
    let tail_writes_flags = mutated(target, |function, environment| {
        flag_reader(function, environment, 1, 2, RUN_B, R_MOVE_B);
        flag_writer(function, environment, 1, 3, T_TAIL, R_TTAIL);
    });
    assert_eq!(
        relocate(&tail_writes_flags, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        ConfluenceRunRelocationError::UnsupportedPair
    );
    // … and in the join's prefix when the run lands deeper.
    let head_writes_flags = mutated(target, |function, environment| {
        flag_reader(function, environment, 1, 2, RUN_B, R_MOVE_B);
        flag_writer(function, environment, 3, 0, HEAD, R_HEAD);
    });
    assert_eq!(
        relocate(&head_writes_flags, &environment, RUN_A, RUN_B, MID).unwrap_err(),
        ConfluenceRunRelocationError::UnsupportedPair
    );
    // With nothing crossed writing the flags the flag-reading member
    // sinks freely: its speculative executions read foreign flags, but
    // their output dies unread.
    let member_reads_flags = mutated(target, |function, environment| {
        flag_reader(function, environment, 1, 2, RUN_B, R_MOVE_B);
    });
    relocate(&member_reads_flags, &environment, RUN_A, RUN_B, HEAD).unwrap();
}

/// Both terminators bordering the window couple: the run's own `Jump` —
/// a crossed position — refuses a read of a member's register, while a
/// read of anything else crosses freely; the join's `Return` is a
/// dead-path position, so a read of a member's still-live register
/// refuses and a read of anything else admits; the other inflow's
/// terminator is never crossed or walked and stays free.
#[test]
fn terminator_positions_couple() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let jump_reads = |register: VirtualRegisterId| {
        move |function: &mut SelectedFunction,
              environment: &register_environment::ValidatedTargetRegisterEnvironment| {
            let jump_row = environment
                .constraint(environment.selected_keys().jump)
                .unwrap()
                .clone();
            let class = function.blocks[1].instructions[0].operands[0].class;
            let mut jump = instruction(T_JUMP, SelectedInstructionKind::Jump, &jump_row, &[]);
            jump.operands.push(SelectedOperand {
                operand: 0,
                virtual_register: register,
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
        }
    };
    assert_eq!(
        relocate(
            &mutated(target, jump_reads(R_MOVE_B)),
            &environment,
            RUN_A,
            RUN_B,
            HEAD,
        )
        .unwrap_err(),
        ConfluenceRunRelocationError::UnsupportedPair
    );
    relocate(
        &mutated(target, jump_reads(R_TTAIL)),
        &environment,
        RUN_A,
        RUN_B,
        HEAD,
    )
    .unwrap();
    // The join's return is a dead-path position: a read of a member's
    // still-live register observes the foreign definition.
    let return_reads = mutated(target, |function, environment| {
        let return_row = environment
            .constraint(environment.selected_keys().return_unit)
            .unwrap()
            .clone();
        let class = function.blocks[3].instructions[0].operands[0].class;
        let mut ret = instruction(RET, SelectedInstructionKind::ReturnUnit, &return_row, &[]);
        ret.operands.push(SelectedOperand {
            operand: 0,
            virtual_register: R_MOVE_A,
            access: RegisterOperandAccess::Use,
            class,
            fixed_view: None,
            tied_to: None,
            early_clobber: false,
        });
        function.blocks[3].terminator = SelectedTerminator::Return {
            instruction: ret,
            psi_return_edge: EdgeId::new(24).unwrap(),
        };
    });
    assert_eq!(
        relocate(&return_reads, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        ConfluenceRunRelocationError::UnsupportedPair
    );
    // The other inflow's terminator is never crossed or walked: its read
    // of a member's register sees the value its own path always had.
    let other_terminator_reads = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        let class = function.blocks[2].instructions[0].operands[0].class;
        let mut jump = instruction(F_JUMP, SelectedInstructionKind::Jump, &jump_row, &[]);
        jump.operands.push(SelectedOperand {
            operand: 0,
            virtual_register: R_MOVE_B,
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
    relocate(&other_terminator_reads, &environment, RUN_A, RUN_B, HEAD).unwrap();
}

/// Barrier kinds and call-roster entries refuse as a run member or
/// anywhere inside the crossed window — the run's own block tail, the
/// `Jump` terminator's call contract, and the join's prefix included —
/// while a barrier before the run's first index, at or after the landing
/// index, or on the other inflow is never crossed: every crossed position
/// is one the run trades order with, and a rostered crossed position is
/// an accounted access the row-less run passes without reordering.
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
                function.blocks[1].instructions[index].kind = kind;
            });
            assert_eq!(
                relocate(&member_barrier, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
                ConfluenceRunRelocationError::UnsupportedInstruction,
                "member {kind:?} at index {index}"
            );
        }
        let tail_barrier = mutated(target, |function, _| {
            function.blocks[1].instructions[3].kind = kind;
        });
        assert_eq!(
            relocate(&tail_barrier, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
            ConfluenceRunRelocationError::UnsupportedInstruction,
            "crossed tail {kind:?}"
        );
        let prefix_barrier = mutated(target, |function, _| {
            function.blocks[3].instructions[0].kind = kind;
        });
        assert_eq!(
            relocate(&prefix_barrier, &environment, RUN_A, RUN_B, MID).unwrap_err(),
            ConfluenceRunRelocationError::UnsupportedInstruction,
            "crossed prefix {kind:?}"
        );
        // A barrier before the run's first index is never crossed — the
        // run leaves ahead of it either way.
        let head_barrier = mutated(target, |function, _| {
            function.blocks[1].instructions[0].kind = kind;
        });
        relocate(&head_barrier, &environment, RUN_A, RUN_B, HEAD).unwrap();
        // A barrier at or after the landing index is never crossed — the
        // run stays ahead of it on every arrival.
        let past_barrier = mutated(target, |function, _| {
            function.blocks[3].instructions[1].kind = kind;
        });
        relocate(&past_barrier, &environment, RUN_A, RUN_B, HEAD).unwrap();
        // A barrier on the other inflow still runs on its own path; the
        // run never enters its stream.
        let inflow_barrier = mutated(target, |function, _| {
            function.blocks[2].instructions[0].kind = kind;
        });
        relocate(&inflow_barrier, &environment, RUN_A, RUN_B, HEAD).unwrap();
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
        (T_TAIL, HEAD),
        (HEAD, MID),
        (T_JUMP, HEAD),
    ] {
        let contract = mutated(target, |function, _| {
            function.calls.push(call_contract(instruction_id));
        });
        assert_eq!(
            relocate(&contract, &environment, RUN_A, RUN_B, destination).unwrap_err(),
            ConfluenceRunRelocationError::UnsupportedInstruction,
            "call roster {instruction_id:?}"
        );
    }
    // A call-roster row before the run's first index, at or after the
    // landing index, on the join's own terminator, or on the other inflow
    // is outside the window: those positions keep the run on the side
    // they always had.
    for instruction_id in [T_HEAD, MID, TAIL, RET, F_HEAD, F_JUMP, LEAD, TRAIL, BRANCH] {
        let contract = mutated(target, |function, _| {
            function.calls.push(call_contract(instruction_id));
        });
        relocate(&contract, &environment, RUN_A, RUN_B, HEAD).unwrap();
    }
    // A rostered crossed position is an accounted access the row-less run
    // passes without reordering a recorded access.
    let rostered_prefix = mutated(target, |function, environment| {
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
    relocate(&rostered_prefix, &environment, RUN_A, RUN_B, MID).unwrap();
    // An unaccounted memory-capable crossed position can never trade
    // order at all: a row-less `Store` reaches place-backed storage the
    // roster does not record.
    let rowless_tail_store = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
        function.blocks[1].instructions[3] = instruction(
            T_TAIL,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            &store,
            &[POINTER, R_TTAIL],
        );
    });
    assert_eq!(
        relocate(&rowless_tail_store, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        ConfluenceRunRelocationError::UnsupportedInstruction
    );
}

/// Only pure register and condition-state work may speculate: a member
/// carrying a memory roster row would run its access on every arrival,
/// and a row-less load or private-slot store gains the same access — as
/// would a kind whose target encoding may fault on the arrivals that
/// never ran it. Every member meets the bar independently: one impure
/// member sinks the whole run. Memory work on the other inflow never
/// moves: its positions run before the landing index on their own
/// arrivals, and the run never enters their stream.
#[test]
fn run_must_be_pure_work() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A roster-carrying member's recorded access would become
    // unconditional across the join's arrivals — in either run position.
    for index in [1usize, 2] {
        let roster_member = mutated(target, |function, environment| {
            let load = environment
                .constraint(environment.selected_keys().load8.unwrap())
                .unwrap()
                .clone();
            let id = function.blocks[1].instructions[index].id;
            let output = function.blocks[1].instructions[index].operands[0].virtual_register;
            function.blocks[1].instructions[index] = instruction(
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
            relocate(&roster_member, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
            ConfluenceRunRelocationError::UnsupportedInstruction,
            "roster member at index {index}"
        );
    }
    // A row-less load still performs an access — and carries any fault —
    // on every arrival after the move.
    let rowless_load = mutated(target, |function, environment| {
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
    });
    assert_eq!(
        relocate(&rowless_load, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        ConfluenceRunRelocationError::UnsupportedInstruction
    );
    // A row-less private-slot store still writes storage on every
    // traversal; sinking it would add the write to the other inflows'
    // arrivals.
    let rowless_store = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap()
            .clone();
        function.blocks[1].instructions[1] = instruction(
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
        relocate(&rowless_store, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        ConfluenceRunRelocationError::UnsupportedInstruction
    );
    // A potentially-faulting kind ran only on this inflow's path: the
    // divide would newly run — and could fault — on every arrival.
    let exact_divide = mutated(target, |function, environment| {
        let divide = environment
            .constraint(environment.selected_keys().divide_u64)
            .unwrap()
            .clone();
        function.blocks[1].instructions[2] = instruction(
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
        relocate(&exact_divide, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        ConfluenceRunRelocationError::UnsupportedInstruction
    );
    let saturating_divide = mutated(target, |function, environment| {
        let divide = environment
            .constraint(environment.selected_keys().saturating_divide_signed)
            .unwrap()
            .clone();
        function.blocks[1].instructions[2] = instruction(
            RUN_B,
            SelectedInstructionKind::SaturatingDivide {
                carrier: selected_instructions::SaturatingCarrier::U64,
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
        relocate(&saturating_divide, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        ConfluenceRunRelocationError::UnsupportedInstruction
    );
    // Saturating remainder divides at the machine level; its carried
    // nonzero-divisor obligation does not make the fault unreachable on
    // the arrivals it would newly execute on.
    let saturating_remainder = mutated(target, |function, environment| {
        let remainder = environment
            .constraint(environment.selected_keys().remainder_i64)
            .unwrap()
            .clone();
        function.blocks[1].instructions[2] = instruction(
            RUN_B,
            SelectedInstructionKind::SaturatingRemainder {
                carrier: selected_instructions::SaturatingCarrier::I64,
                obligation: ObligationId::new(11).unwrap(),
                accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                    [7; 32],
                ),
            },
            &remainder,
            &[POINTER, R_MOVE_B, R_MOVE_B, R_MOVE_B],
        );
    });
    assert_eq!(
        relocate(&saturating_remainder, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        ConfluenceRunRelocationError::UnsupportedInstruction
    );
    // Memory work on the other inflow never moves: a rostered store in F
    // runs before the landing index on its own arrivals whatever it reads.
    let inflow_store = mutated(target, |function, environment| {
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
    relocate(&inflow_store, &environment, RUN_A, RUN_B, HEAD).unwrap();
    // A rostered position after the landing index is a dead-path position
    // like any other: it refuses only when it would read a still-live
    // member definition.
    let tail_store = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
        function.blocks[3].instructions[2] = instruction(
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
            PlaceId::new(3).unwrap(),
            SelectedMemoryAccessRole::WritePlace,
        ));
    });
    relocate(&tail_store, &environment, RUN_A, RUN_B, HEAD).unwrap();
    let tail_store_reads_member = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
        function.blocks[3].instructions[2] = instruction(
            TAIL,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            &store,
            &[R_MOVE_B, R_TAIL],
        );
        function.memory_accesses.push(access(
            TAIL,
            PlaceId::new(3).unwrap(),
            SelectedMemoryAccessRole::WritePlace,
        ));
    });
    assert_eq!(
        relocate(&tail_store_reads_member, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        ConfluenceRunRelocationError::UnsupportedPair
    );
}

/// Every location a member writes must be dead — unread until rewritten —
/// from the landing index forward: a reader in the join's tail, in the
/// join's terminator, in a deeper successor block, or in a block a loop
/// reaches all refuse, while a write that retires the foreign definition
/// before any reader admits. A loop back through the join scans the
/// positions before the landing index against the live foreign set,
/// republishes at the run's new position, and walks on; a loop back
/// through the run's vacated block scans its remaining positions against
/// the same set.
#[test]
fn run_writes_must_die_on_the_other_inflows() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let reads = |block: usize,
                 index: usize,
                 id: SelectedInstructionId,
                 input: VirtualRegisterId,
                 output: VirtualRegisterId| {
        move |function: &mut SelectedFunction,
              environment: &register_environment::ValidatedTargetRegisterEnvironment| {
            let copy = environment
                .constraint(environment.selected_keys().copy_i64)
                .unwrap()
                .clone();
            function.blocks[block].instructions[index] = instruction(
                id,
                SelectedInstructionKind::CopyI64,
                &copy,
                &[input, output],
            );
        }
    };
    let writes =
        |block: usize, index: usize, id: SelectedInstructionId, output: VirtualRegisterId| {
            move |function: &mut SelectedFunction,
              environment: &register_environment::ValidatedTargetRegisterEnvironment| {
            let copy = environment
                .constraint(environment.selected_keys().copy_i64)
                .unwrap()
                .clone();
            function.blocks[block].instructions[index] = instruction(
                id,
                SelectedInstructionKind::CopyI64,
                &copy,
                &[R_TRAIL, output],
            );
        }
        };
    // A reader anywhere after the landing index observes the foreign
    // definition on the other inflows' arrivals — either member's.
    for (block, index, id, input, output) in [
        (3usize, 1usize, MID, R_MOVE_A, R_MID),
        (3, 2, TAIL, R_MOVE_B, R_TAIL),
    ] {
        let reader = mutated(target, reads(block, index, id, input, output));
        assert_eq!(
            relocate(&reader, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
            ConfluenceRunRelocationError::UnsupportedPair,
            "reader {id:?} of {input:?}"
        );
    }
    // A write that retires a foreign definition before any reader admits:
    // `MID` observes `HEAD`'s rewrite of `R_MOVE_A` on every arrival,
    // exactly what the source computed — and `R_MOVE_B` simply dies
    // unread past the landing index.
    let retired = mutated(target, |function, environment| {
        writes(3, 0, HEAD, R_MOVE_A)(function, environment);
        reads(3, 1, MID, R_MOVE_A, R_MID)(function, environment);
    });
    relocate(&retired, &environment, RUN_A, RUN_B, HEAD).unwrap();
    // A deeper successor block is walked across the join's exit edge.
    let deep = |edit: &mut dyn FnMut(
        &mut SelectedFunction,
        &register_environment::ValidatedTargetRegisterEnvironment,
    )| {
        mutated(target, |function, environment| {
            let jump_row = environment
                .constraint(environment.selected_keys().jump)
                .unwrap()
                .clone();
            let return_row = environment
                .constraint(environment.selected_keys().return_unit)
                .unwrap()
                .clone();
            function.blocks[3].terminator = jump_terminator(
                instruction(J_JUMP, SelectedInstructionKind::Jump, &jump_row, &[]),
                successor(BLOCK_D, BlockId::new(5).unwrap(), EDGE_JD),
            );
            let mut deep = SelectedBlock {
                id: BLOCK_D,
                origin: SelectedBlockOrigin::Source(BlockId::new(5).unwrap()),
                instructions: Vec::new(),
                terminator: SelectedTerminator::Return {
                    instruction: instruction(
                        RET,
                        SelectedInstructionKind::ReturnUnit,
                        &return_row,
                        &[],
                    ),
                    psi_return_edge: EdgeId::new(27).unwrap(),
                },
            };
            deep.instructions.push(instruction(
                DEEP,
                SelectedInstructionKind::CopyI64,
                &environment
                    .constraint(environment.selected_keys().copy_i64)
                    .unwrap()
                    .clone(),
                &[R_TRAIL, R_DEEP],
            ));
            function.blocks.push(deep);
            edit(function, environment);
        })
    };
    // The deep block reading a member's register observes the foreign
    // definition.
    let deep_reads = deep(&mut |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[4].instructions[0] = instruction(
            DEEP,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[R_MOVE_A, R_DEEP],
        );
    });
    assert_eq!(
        relocate(&deep_reads, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        ConfluenceRunRelocationError::UnsupportedPair
    );
    // A deep write retires the read member's definition before the read;
    // the other member's register stays live but is never observed.
    let deep_retired = deep(&mut |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[4].instructions.insert(
            0,
            instruction(
                DEEP_HEAD,
                SelectedInstructionKind::CopyI64,
                &copy,
                &[R_TRAIL, R_MOVE_A],
            ),
        );
        function.blocks[4].instructions[1] = instruction(
            DEEP,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[R_MOVE_A, R_DEEP],
        );
    });
    relocate(&deep_retired, &environment, RUN_A, RUN_B, HEAD).unwrap();
    // A loop back through the other inflow's block: J exits to F, F
    // re-enters the join, and a member's still-live definition meets F's
    // reader on the second traversal.
    let looped = |edit: &mut dyn FnMut(
        &mut SelectedFunction,
        &register_environment::ValidatedTargetRegisterEnvironment,
    )| {
        mutated(target, |function, environment| {
            let jump_row = environment
                .constraint(environment.selected_keys().jump)
                .unwrap()
                .clone();
            function.blocks[3].terminator = jump_terminator(
                instruction(J_JUMP, SelectedInstructionKind::Jump, &jump_row, &[]),
                successor(BLOCK_F, BlockId::new(3).unwrap(), EDGE_JD),
            );
            edit(function, environment);
        })
    };
    let looped_reads = looped(&mut |function, environment| {
        reads(2, 0, F_HEAD, R_MOVE_B, R_FHEAD)(function, environment);
    });
    assert_eq!(
        relocate(&looped_reads, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        ConfluenceRunRelocationError::UnsupportedPair
    );
    // The looped inflow's own write retires the foreign definition before
    // its reader — and the re-entering edge carries nothing live back.
    let looped_retired = looped(&mut |function, environment| {
        writes(2, 0, F_HEAD, R_MOVE_B)(function, environment);
        reads(2, 1, F_TAIL, R_MOVE_B, R_FTAIL)(function, environment);
    });
    relocate(&looped_retired, &environment, RUN_A, RUN_B, HEAD).unwrap();
    // A loop back through the run's own vacated block scans its remaining
    // positions against the foreign set: the tail reader refuses.
    let vacated_loop = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        function.blocks[3].terminator = jump_terminator(
            instruction(J_JUMP, SelectedInstructionKind::Jump, &jump_row, &[]),
            successor(BLOCK_T, BlockId::new(2).unwrap(), EDGE_JD),
        );
        reads(1, 3, T_TAIL, R_MOVE_A, R_TTAIL)(function, environment);
    });
    assert_eq!(
        relocate(&vacated_loop, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        ConfluenceRunRelocationError::UnsupportedPair
    );
    // Without a loop the other inflow is never walked: a read of a
    // member's register in F sees the value F's own path always carried.
    let inflow_reads = mutated(target, reads(2, 0, F_HEAD, R_MOVE_A, R_FHEAD));
    relocate(&inflow_reads, &environment, RUN_A, RUN_B, HEAD).unwrap();
}

/// A boundary settlement observes the executed prefix at its position:
/// any settlement inside the run's span or past it in the run's own
/// block, or past the landing index in the join, observes a different
/// executed set once the run lands on the other side of the crossed
/// positions, while positions at or before either boundary — and
/// settlements in blocks the run's stream never touches — keep the
/// prefix they always had.
#[test]
fn boundary_settlements_bound_the_window() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Inside the run's span — the settlement at `RUN_B`'s index observed
    // the run's first member in the executed prefix.
    let inside_run = mutated(target, |function, _| {
        function
            .boundary_settlements
            .push(settlement(BLOCK_T, 2, 60));
    });
    assert_eq!(
        relocate(&inside_run, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        ConfluenceRunRelocationError::UnsupportedPair
    );
    // Past the run's span in its own block.
    let past_run = mutated(target, |function, _| {
        function
            .boundary_settlements
            .push(settlement(BLOCK_T, 3, 61));
    });
    assert_eq!(
        relocate(&past_run, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        ConfluenceRunRelocationError::UnsupportedPair
    );
    // Past the landing index in the join.
    let past_landing = mutated(target, |function, _| {
        function
            .boundary_settlements
            .push(settlement(BLOCK_J, 1, 62));
    });
    assert_eq!(
        relocate(&past_landing, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        ConfluenceRunRelocationError::UnsupportedPair
    );
    // At or before either boundary the executed prefix is unchanged.
    let at_run = mutated(target, |function, _| {
        function
            .boundary_settlements
            .push(settlement(BLOCK_T, 1, 63));
        function
            .boundary_settlements
            .push(settlement(BLOCK_J, 0, 64));
    });
    relocate(&at_run, &environment, RUN_A, RUN_B, HEAD).unwrap();
    // Landing deeper moves the join's boundary with it.
    let deeper = mutated(target, |function, _| {
        function
            .boundary_settlements
            .push(settlement(BLOCK_J, 2, 65));
    });
    assert_eq!(
        relocate(&deeper, &environment, RUN_A, RUN_B, MID).unwrap_err(),
        ConfluenceRunRelocationError::UnsupportedPair
    );
    relocate(&deeper, &environment, RUN_A, RUN_B, TAIL).unwrap();
    // Settlements in blocks the run never enters — the other inflow, the
    // fork head — keep their executed prefixes.
    let elsewhere = mutated(target, |function, _| {
        function
            .boundary_settlements
            .push(settlement(BLOCK_F, 2, 66));
        function
            .boundary_settlements
            .push(settlement(BLOCK_B, 2, 67));
    });
    relocate(&elsewhere, &environment, RUN_A, RUN_B, HEAD).unwrap();
}

/// Only the named run inside the named confluence relocates: unknown
/// member or function ids, a terminator-carried first member, and
/// destinations outside the join all refuse without touching the plan.
#[test]
fn only_the_named_confluence_window_relocates() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // An unknown first member or function index never locates the window.
    assert_eq!(
        relocate_selected_run_into_confluence(
            &source,
            0,
            SelectedInstructionId(99),
            RUN_B,
            HEAD,
            &environment,
            budget(),
        )
        .unwrap_err(),
        ConfluenceRunRelocationError::SourceMismatch
    );
    assert_eq!(
        relocate_selected_run_into_confluence(
            &source,
            9,
            RUN_A,
            RUN_B,
            HEAD,
            &environment,
            budget(),
        )
        .unwrap_err(),
        ConfluenceRunRelocationError::SourceMismatch
    );
    // Naming a terminator-carried instruction as the run's first member
    // is not a body position.
    for first in [T_JUMP, RET, BRANCH] {
        assert_eq!(
            relocate(&source, &environment, first, T_TAIL, HEAD).unwrap_err(),
            ConfluenceRunRelocationError::SourceMismatch,
            "first {first:?}"
        );
    }
    // A destination outside the join names no landing position: the run's
    // own block mates, the other inflow's, B's, and dangling ids.
    for destination in [
        LEAD,
        TRAIL,
        BRANCH,
        T_HEAD,
        T_TAIL,
        T_JUMP,
        F_HEAD,
        F_TAIL,
        F_JUMP,
        SelectedInstructionId(99),
    ] {
        assert_eq!(
            relocate(&source, &environment, RUN_A, RUN_B, destination).unwrap_err(),
            ConfluenceRunRelocationError::UnsupportedPair,
            "destination {destination:?}"
        );
    }
}

/// Replay consumes only the exact move: a proposal that drops a member,
/// lands the run anywhere else, permutes the crossed positions, or
/// carries an unrelated edit all reject.
#[test]
fn replay_rejects_anything_but_the_move() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let result = relocate(&source, &environment, RUN_A, RUN_B, MID).unwrap();
    // The honest proposal replays.
    validate_confluence_run_relocation(
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
    let run: Vec<_> = displaced.functions[0].blocks[3]
        .instructions
        .drain(1..3)
        .collect();
    displaced.functions[0].blocks[3].instructions.extend(run);
    assert_eq!(
        validate_confluence_run_relocation(
            &source,
            0,
            RUN_A,
            RUN_B,
            MID,
            &environment,
            budget(),
            displaced,
        )
        .unwrap_err(),
        ConfluenceRunRelocationError::ReplayMismatch
    );
    // The run left in its own block rejects.
    let mut unmoved = result.transformed().clone();
    let run: Vec<_> = unmoved.functions[0].blocks[3]
        .instructions
        .drain(1..3)
        .collect();
    unmoved.functions[0].blocks[1]
        .instructions
        .splice(1..1, run);
    assert_eq!(
        validate_confluence_run_relocation(
            &source,
            0,
            RUN_A,
            RUN_B,
            MID,
            &environment,
            budget(),
            unmoved,
        )
        .unwrap_err(),
        ConfluenceRunRelocationError::ReplayMismatch
    );
    // A dropped instruction in the join rejects.
    let mut dropped = result.transformed().clone();
    dropped.functions[0].blocks[3].instructions.pop();
    assert_eq!(
        validate_confluence_run_relocation(
            &source,
            0,
            RUN_A,
            RUN_B,
            MID,
            &environment,
            budget(),
            dropped,
        )
        .unwrap_err(),
        ConfluenceRunRelocationError::ReplayMismatch
    );
    // An unrelated literal edit inside the join rejects.
    let mut edited = result.transformed().clone();
    edited.functions[0].blocks[3].instructions[0].kind = SelectedInstructionKind::MaterializeI64 {
        value: IntegerValue::Unsigned(12),
    };
    assert_eq!(
        validate_confluence_run_relocation(
            &source,
            0,
            RUN_A,
            RUN_B,
            MID,
            &environment,
            budget(),
            edited,
        )
        .unwrap_err(),
        ConfluenceRunRelocationError::ReplayMismatch
    );
    // Naming a different window on the same proposal re-derives a
    // different landing and rejects.
    assert_eq!(
        validate_confluence_run_relocation(
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
        ConfluenceRunRelocationError::ReplayMismatch
    );
}

/// The bounded audit is measured: the confluence window prices every scan,
/// crossed member-against-position surface pair, roster row, and the
/// dead-path fixpoint bound against the work budget, and a budget one
/// step short refuses rather than skimping. Landing deeper into the
/// join's body crosses more join positions per member.
#[test]
fn measured_validation_step_boundary() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // The member-locate scan prices every block's body plus terminator
    // once across the plan (3+5+3+4 = 15), and again for this function's
    // blocks (15). The crossed surfaces pair each member (1) against
    // `T_TAIL` (1) and the `Jump` terminator (2 uses + defs on x86-64):
    // (2+3) per member = 10 steps. The dead-path bound prices each
    // block's body, terminator, and edge surfaces once per run location
    // plus the initial scan: on x86-64 the materializations cost 1 each,
    // the jumps 2, the branch 3, and the return 9 — (2+3)+(4+2)+(2+2)+
    // (3+9) = 27 — times two written member registers plus one:
    // 27*3 = 81.
    let steps: u64 = 15 + 15 + 10 + 81;
    let exact = OptimizationWorkBudget::new(1, 1, steps, 1, 1).unwrap();
    relocate_selected_run_into_confluence(&source, 0, RUN_A, RUN_B, HEAD, &environment, exact)
        .unwrap();
    let starved = OptimizationWorkBudget::new(1, 1, steps - 1, 1, 1).unwrap();
    assert_eq!(
        relocate_selected_run_into_confluence(
            &source,
            0,
            RUN_A,
            RUN_B,
            HEAD,
            &environment,
            starved,
        )
        .unwrap_err(),
        ConfluenceRunRelocationError::WorkBudgetExceeded
    );
    // Landing at the body end crosses the whole join body: each member
    // pairs against `T_TAIL`, `HEAD`, `MID`, `TAIL`, and the terminator.
    let steps_end: u64 = 15 + 15 + 22 + 81;
    let exact = OptimizationWorkBudget::new(1, 1, steps_end, 1, 1).unwrap();
    relocate_selected_run_into_confluence(&source, 0, RUN_A, RUN_B, RET, &environment, exact)
        .unwrap();
    let starved = OptimizationWorkBudget::new(1, 1, steps_end - 1, 1, 1).unwrap();
    assert_eq!(
        relocate_selected_run_into_confluence(
            &source,
            0,
            RUN_A,
            RUN_B,
            RET,
            &environment,
            starved,
        )
        .unwrap_err(),
        ConfluenceRunRelocationError::WorkBudgetExceeded
    );
}

/// The environment's target must be the plan's target: a relocation proven
/// for one target's constraints is not evidence on another.
#[test]
fn target_mismatch_rejects() {
    let source = fixture(NativeTarget::linux_x64());
    let environment = baseline_target_register_environment(NativeTarget::linux_arm64()).unwrap();
    assert_eq!(
        relocate(&source, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        ConfluenceRunRelocationError::SourceMismatch
    );
}

/// Two runs over the identical source produce the identical validated
/// result, and the published plan is a legal second input through the
/// sealed analysis boundary: the other inflow's run sinks through the
/// same confluence onto it, a hazard-coupled run still declines, and an
/// in-block family still admits.
#[test]
fn confluence_run_relocation_is_deterministic_and_re_admitted() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // `T_HEAD` reads `R_MOVE_A` before the run: the first move's crossed
    // window never contains it, while on the second input the landed
    // `RUN_A` sits in the join's prefix ahead of any deeper sink.
    let source = mutated(target, |function, environment| {
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
    let first = relocate(&source, &environment, RUN_A, RUN_B, HEAD).unwrap();
    let second = relocate(&source, &environment, RUN_A, RUN_B, HEAD).unwrap();
    assert_eq!(first, second);
    // The validated output carries the sealed analysis boundary, so it is
    // a legal second input — not merely a reconstruction of one. The
    // other inflow's whole-body run sinks through the same confluence
    // onto it.
    let again = relocate_selected_run_into_confluence(
        &first,
        0,
        F_HEAD,
        F_TAIL,
        HEAD,
        &environment,
        budget(),
    )
    .unwrap();
    assert_eq!(
        block_order(&again.transformed().functions[0].blocks[3]),
        vec![RUN_A, RUN_B, F_HEAD, F_TAIL, HEAD, MID, TAIL]
    );
    assert_eq!(
        again.receipt().source_selected(),
        first.receipt().transformed_selected()
    );
    // A hazard-coupled run still declines on the second input: once
    // `RUN_A` sits at J's head, `T_HEAD` — reading `R_MOVE_A` — cannot
    // sink past it, so the run `T_HEAD` opens never lands.
    assert_eq!(
        relocate_selected_run_into_confluence(
            &first,
            0,
            T_HEAD,
            T_TAIL,
            HEAD,
            &environment,
            budget(),
        )
        .unwrap_err(),
        ConfluenceRunRelocationError::UnsupportedPair
    );
    // A different scheduling family still admits on the second input.
    let swapped =
        crate::relocate_selected_instruction(&first, 0, MID, TAIL, &environment, budget()).unwrap();
    assert_eq!(
        block_order(&swapped.transformed().functions[0].blocks[3]),
        vec![RUN_A, RUN_B, HEAD, TAIL, MID]
    );
}
