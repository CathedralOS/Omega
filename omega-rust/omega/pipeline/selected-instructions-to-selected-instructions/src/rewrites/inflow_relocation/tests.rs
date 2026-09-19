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
    InflowRelocationError, InflowRelocationReceipt, ValidatedInflowRelocation,
    relocate_selected_instruction_onto_inflow, validate_inflow_relocation,
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
const T_TAIL: SelectedInstructionId = SelectedInstructionId(5);
const F_HEAD: SelectedInstructionId = SelectedInstructionId(6);
const F_TAIL: SelectedInstructionId = SelectedInstructionId(7);
const HEAD: SelectedInstructionId = SelectedInstructionId(8);
const MOVING: SelectedInstructionId = SelectedInstructionId(9);
const MID: SelectedInstructionId = SelectedInstructionId(10);
const TAIL: SelectedInstructionId = SelectedInstructionId(11);
const BRANCH: SelectedInstructionId = SelectedInstructionId(12);
const T_JUMP: SelectedInstructionId = SelectedInstructionId(13);
const F_JUMP: SelectedInstructionId = SelectedInstructionId(14);
const RET: SelectedInstructionId = SelectedInstructionId(15);
const DEEP: SelectedInstructionId = SelectedInstructionId(16);
const J_JUMP: SelectedInstructionId = SelectedInstructionId(17);
const DEEP_HEAD: SelectedInstructionId = SelectedInstructionId(18);

const POINTER: VirtualRegisterId = VirtualRegisterId(0);
const R_LEAD: VirtualRegisterId = VirtualRegisterId(1);
const R_TRAIL: VirtualRegisterId = VirtualRegisterId(2);
const R_THEAD: VirtualRegisterId = VirtualRegisterId(3);
const R_TTAIL: VirtualRegisterId = VirtualRegisterId(4);
const R_FHEAD: VirtualRegisterId = VirtualRegisterId(5);
const R_FTAIL: VirtualRegisterId = VirtualRegisterId(6);
const R_HEAD: VirtualRegisterId = VirtualRegisterId(7);
const R_MOVE: VirtualRegisterId = VirtualRegisterId(8);
const R_MID: VirtualRegisterId = VirtualRegisterId(9);
const R_TAIL: VirtualRegisterId = VirtualRegisterId(10);
const R_BOUND: VirtualRegisterId = VirtualRegisterId(11);
const R_DEEP: VirtualRegisterId = VirtualRegisterId(12);

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
/// `B = [LEAD; TRAIL] -> branch -> T = [T_HEAD; T_TAIL] -> jump -> J`
/// and `F = [F_HEAD; F_TAIL] -> jump -> J = [HEAD; MOVING; MID; TAIL] ->
/// return`. The default move relocates `MOVING` onto `T_TAIL`'s position
/// in the inflow block — crossing `T_TAIL`, the jump terminator, the
/// landing edge's transports, and `HEAD` — while B and F keep their
/// order. On F's inflow the member never again executes: `MID`, `TAIL`,
/// the return, and every reachable continuation must never read `R_MOVE`
/// before a write retires the missing definition.
fn fixture(target: NativeTarget) -> ValidatedInflowRelocation {
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
        result_register(R_TTAIL, T_TAIL, 5),
        result_register(R_FHEAD, F_HEAD, 6),
        result_register(R_FTAIL, F_TAIL, 7),
        result_register(R_HEAD, HEAD, 8),
        result_register(R_MOVE, MOVING, 9),
        result_register(R_MID, MID, 10),
        result_register(R_TAIL, TAIL, 11),
        result_register(R_BOUND, LEAD, 12),
        result_register(R_DEEP, DEEP, 13),
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
                        materialization(MOVING, R_MOVE, 7),
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
    ValidatedInflowRelocation {
        receipt: InflowRelocationReceipt {
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
) -> ValidatedInflowRelocation {
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
    source: &ValidatedInflowRelocation,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
) -> Result<ValidatedInflowRelocation, InflowRelocationError> {
    relocate_selected_instruction_onto_inflow(source, 0, member, destination, environment, budget())
}

/// The member rises through the confluence edge onto the destination's
/// position on every target: `MOVING` leaves J's body, `HEAD` keeps its
/// position ahead of the vacated index and `MID` and `TAIL` keep their
/// order behind it, the member lands at `T_TAIL`'s index with `T_HEAD`
/// untouched ahead of it, and the other inflow's block keeps its order —
/// identity, kind, operands, and provenance intact. The replayed proposal
/// restores the source bit-identically.
#[test]
fn member_relocates_onto_the_inflow() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = fixture(target);
        let result = relocate(&source, &environment, MOVING, T_TAIL).unwrap();
        let original = &source.transformed().functions[0];
        let moved = &result.transformed().functions[0];
        assert_eq!(
            moved.blocks[1]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![T_HEAD, MOVING, T_TAIL]
        );
        assert_eq!(
            moved.blocks[3]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![HEAD, MID, TAIL]
        );
        assert_eq!(
            moved.blocks[0].instructions,
            original.blocks[0].instructions
        );
        assert_eq!(
            moved.blocks[2].instructions,
            original.blocks[2].instructions
        );
        // The member moved bit-identically; every terminator, edge, and
        // roster stayed untouched.
        assert_eq!(
            moved.blocks[1].instructions[1],
            original.blocks[3].instructions[1]
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
        validate_inflow_relocation(
            &source,
            0,
            MOVING,
            T_TAIL,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
    }
}

/// The destination names the landing position directly: an inflow body
/// instruction puts the member on its index, and the inflow's
/// terminator-carried instruction lands the member at the body end.
/// Either inflow's edge carries the move — landing onto `F_TAIL`'s
/// position makes T's edge the other inflow whose traversals lose the
/// member.
#[test]
fn member_lands_at_the_named_position() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let result = relocate(&source, &environment, MOVING, T_HEAD).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[1]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![MOVING, T_HEAD, T_TAIL]
    );
    let result = relocate(&source, &environment, MOVING, T_JUMP).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[1]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![T_HEAD, T_TAIL, MOVING]
    );
    let result = relocate(&source, &environment, MOVING, F_TAIL).unwrap();
    let moved = &result.transformed().functions[0];
    assert_eq!(
        moved.blocks[2]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![F_HEAD, MOVING, F_TAIL]
    );
    assert_eq!(
        moved.blocks[3]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![HEAD, MID, TAIL]
    );
}

/// Any member of the join may rise onto an inflow: the head member
/// crosses no join position, the last member crosses the join's whole
/// prefix, and the tail member can land ahead of the inflow's head.
#[test]
fn members_at_each_join_index_relocate() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let result = relocate(&source, &environment, HEAD, T_TAIL).unwrap();
    let moved = &result.transformed().functions[0];
    assert_eq!(
        moved.blocks[3]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![MOVING, MID, TAIL]
    );
    assert_eq!(
        moved.blocks[1]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![T_HEAD, HEAD, T_TAIL]
    );
    let result = relocate(&source, &environment, TAIL, T_HEAD).unwrap();
    let moved = &result.transformed().functions[0];
    assert_eq!(
        moved.blocks[3]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![HEAD, MOVING, MID]
    );
    assert_eq!(
        moved.blocks[1]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![TAIL, T_HEAD, T_TAIL]
    );
}

/// The inflow block may itself be the entry block — reached with no
/// predecessor of its own, its lone `Jump` still carries every traversal
/// into the join: a member of J rises onto B's tail position once B's
/// terminator is a `Jump` into the join, crossing B's tail instruction,
/// the jump, and J's prefix.
#[test]
fn entry_inflow_predecessor_relocates() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let entered = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        function.blocks[0].terminator = jump_terminator(
            instruction(BRANCH, SelectedInstructionKind::Jump, &jump_row, &[]),
            successor(BLOCK_J, BlockId::new(4).unwrap(), EDGE_BT),
        );
    });
    let result = relocate(&entered, &environment, MOVING, TRAIL).unwrap();
    let moved = &result.transformed().functions[0];
    assert_eq!(
        moved.blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![LEAD, MOVING, TRAIL]
    );
    assert_eq!(
        moved.blocks[3]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![HEAD, MID, TAIL]
    );
}

/// The destination's block must end in the lone unconditional `Jump`
/// reaching the member's block, and the member's block must be a join at
/// least one other predecessor's edge also reaches: a conditional
/// terminator keeps a second exit the member would newly execute on, a
/// sole-predecessor join is the predecessor family's case, a destination
/// in the member's own block is the in-block family's case, the entry
/// block crosses no inflow edge on its first traversal, and an
/// implementation-origin predecessor carries boundary work the audit does
/// not cross. The other inflow's own shape is unconstrained — a
/// conditional terminator feeding the join on both edges still admits,
/// and so does a third inflow.
#[test]
fn the_confluence_must_open() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // A destination in B names a block whose conditional terminator keeps
    // a second exit the member would newly execute on.
    for destination in [LEAD, TRAIL, BRANCH] {
        assert_eq!(
            relocate(&source, &environment, MOVING, destination).unwrap_err(),
            InflowRelocationError::UnsupportedPair,
            "destination {destination:?}"
        );
    }
    // A destination in the member's own block is the in-block family's
    // case — the member itself included — and a dangling id names no
    // landing position.
    for destination in [HEAD, MOVING, MID, TAIL, RET, SelectedInstructionId(99)] {
        assert_eq!(
            relocate(&source, &environment, MOVING, destination).unwrap_err(),
            InflowRelocationError::UnsupportedPair,
            "destination {destination:?}"
        );
    }
    // The join reached by only the destination's edge is the
    // sole-predecessor family's case — no traversal loses the member.
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
            successor(BLOCK_D, BlockId::new(5).unwrap(), EDGE_DJ),
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
        relocate(&sole, &environment, MOVING, T_TAIL).unwrap_err(),
        InflowRelocationError::UnsupportedPair
    );
    // The destination's jump must reach the member's block: retargeting
    // it elsewhere names a landing position the member never followed.
    let elsewhere = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        function.blocks[1].terminator = jump_terminator(
            instruction(T_JUMP, SelectedInstructionKind::Jump, &jump_row, &[]),
            successor(BLOCK_F, BlockId::new(3).unwrap(), EDGE_TJ),
        );
    });
    assert_eq!(
        relocate(&elsewhere, &environment, MOVING, T_TAIL).unwrap_err(),
        InflowRelocationError::UnsupportedPair
    );
    // A conditional predecessor keeps a second exit the member would
    // newly execute on — the arm and join families' burden.
    let branched = mutated(target, |function, environment| {
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
            when_zero: successor(BLOCK_J, BlockId::new(4).unwrap(), EDGE_DJ),
        };
    });
    assert_eq!(
        relocate(&branched, &environment, MOVING, T_TAIL).unwrap_err(),
        InflowRelocationError::UnsupportedPair
    );
    // The member's block may not be the entry block: its first traversal
    // crosses no inflow edge, so the move would remove an execution the
    // dead-path audit cannot see. Both inflow blocks jumping to B gives
    // the entry the other inflow the shape requires.
    let entry_member = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        for block in [1usize, 2usize] {
            let instruction_id = if block == 1 { T_JUMP } else { F_JUMP };
            function.blocks[block].terminator = jump_terminator(
                instruction(
                    instruction_id,
                    SelectedInstructionKind::Jump,
                    &jump_row,
                    &[],
                ),
                successor(BLOCK_B, BlockId::new(1).unwrap(), EDGE_TJ),
            );
        }
    });
    assert_eq!(
        relocate(&entry_member, &environment, LEAD, T_TAIL).unwrap_err(),
        InflowRelocationError::UnsupportedPair
    );
    // An implementation-origin predecessor carries boundary work the
    // bounded audit does not cross.
    let cased_inflow = mutated(target, |function, _| {
        function.blocks[1].origin = SelectedBlockOrigin::CaseDispatch {
            source: BlockId::new(2).unwrap(),
            case_ordinal: 0,
        };
    });
    assert_eq!(
        relocate(&cased_inflow, &environment, MOVING, T_TAIL).unwrap_err(),
        InflowRelocationError::UnsupportedPair
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
    relocate(&branched_inflow, &environment, MOVING, T_TAIL).unwrap();
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
    relocate(&three_way, &environment, MOVING, T_TAIL).unwrap();
}

/// Only a plain semantic edge carries the member across: a continuation
/// role, case custody, per-edge fuel, or a live structural transport on
/// the crossed edge all refuse. The other inflow's edge is never crossed
/// — it runs before the member's old position on its own arrivals — so
/// fuel, a continuation role, a case payload, or a structural transport
/// on it all stay free, even when they read the member's register.
#[test]
fn only_plain_semantic_edges_carry_the_member() {
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
            relocate(&continued, &environment, MOVING, T_TAIL).unwrap_err(),
            InflowRelocationError::UnsupportedPair
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
        relocate(&case_edge, &environment, MOVING, T_TAIL).unwrap_err(),
        InflowRelocationError::UnsupportedPair
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
        relocate(&fueled, &environment, MOVING, T_TAIL).unwrap_err(),
        InflowRelocationError::UnsupportedPair
    );
    // A live structural transport on the crossed edge moves a stored value
    // the member would cross.
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
        relocate(&structural, &environment, MOVING, T_TAIL).unwrap_err(),
        InflowRelocationError::UnsupportedPair
    );
    // Everything on the other inflow's edge stays free: it runs before the
    // member's old position on its own arrivals and is never crossed —
    // even a payload or structural transport reading the member's
    // register, which observes the value that inflow always carried.
    let laden_inflow = mutated(target, |function, _| {
        let successor = match &mut function.blocks[2].terminator {
            SelectedTerminator::Jump { successor, .. } => successor,
            _ => unreachable!(),
        };
        successor.fuel.push(optimization_unit::FuelSettlement {
            site: optimization_unit::PsiProvenance::Operation(OperationId::new(30).unwrap()),
            units: 1,
        });
        successor.role = SelectedSuccessorRole::EdgeTransferContinuation;
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
                    argument: R_MOVE,
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
                    argument: R_MOVE,
                    destination: selected_instructions::LocalStorageSlotId::Spill {
                        register: R_BOUND,
                    },
                    byte_size: 8,
                    alignment: 8,
                },
            });
    });
    relocate(&laden_inflow, &environment, MOVING, T_TAIL).unwrap();
}

/// The crossed edge's register transports sit between the member's old
/// and new positions: a member defining the transported argument would
/// hand the binding a different value where the source bound the
/// pre-member one, a member defining the parameter would be overwritten
/// by it on this inflow's arrivals, and a member reading the parameter
/// would observe the transported value only before the move — while a
/// member merely reading the argument crosses freely. A transport on the
/// other inflow's edge writes the member's register on that arrival only
/// to see the member's own write replace it at the vacated index: a later
/// reader still refuses, but with none the moved write stays dead.
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
    let member_writes = |register: VirtualRegisterId| {
        move |function: &mut SelectedFunction,
              environment: &register_environment::ValidatedTargetRegisterEnvironment| {
            let materialize = environment
                .constraint(environment.selected_keys().materialize_i64)
                .unwrap()
                .clone();
            function.blocks[3].instructions[1] = instruction(
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
            function.blocks[3].instructions[1] = instruction(
                MOVING,
                SelectedInstructionKind::CopyI64,
                &copy,
                &[register, R_MOVE],
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
        MOVING,
        T_TAIL,
    )
    .unwrap();
    relocate(
        &on_crossed_edge(&mut member_reads(POINTER)),
        &environment,
        MOVING,
        T_TAIL,
    )
    .unwrap();
    assert_eq!(
        relocate(
            &on_crossed_edge(&mut member_writes(POINTER)),
            &environment,
            MOVING,
            T_TAIL,
        )
        .unwrap_err(),
        InflowRelocationError::UnsupportedPair
    );
    assert_eq!(
        relocate(
            &on_crossed_edge(&mut member_writes(R_BOUND)),
            &environment,
            MOVING,
            T_TAIL,
        )
        .unwrap_err(),
        InflowRelocationError::UnsupportedPair
    );
    assert_eq!(
        relocate(
            &on_crossed_edge(&mut member_reads(R_BOUND)),
            &environment,
            MOVING,
            T_TAIL,
        )
        .unwrap_err(),
        InflowRelocationError::UnsupportedPair
    );
    // On the other inflow's edge a transport runs before the member's old
    // position on its own arrivals: an argument reading the member's
    // register observes the value that inflow always carried, and a
    // parameter writing it is overwritten where the member's own write
    // stood — dead unless a reader remains.
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
        &on_inflow_edge(&mut || binding(R_MOVE, R_BOUND)),
        &environment,
        MOVING,
        T_TAIL,
    )
    .unwrap();
    relocate(
        &on_inflow_edge(&mut || binding(POINTER, R_MOVE)),
        &environment,
        MOVING,
        T_TAIL,
    )
    .unwrap();
    // The same parameter write leaves a live missing definition when a
    // reader behind the vacated index remains.
    let inflow_writes_and_reads = mutated(target, |function, environment| {
        let successor = match &mut function.blocks[2].terminator {
            SelectedTerminator::Jump { successor, .. } => successor,
            _ => unreachable!(),
        };
        successor.bindings.push(binding(POINTER, R_MOVE));
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[3].instructions[2] = instruction(
            MID,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[R_MOVE, R_MID],
        );
    });
    assert_eq!(
        relocate(&inflow_writes_and_reads, &environment, MOVING, T_TAIL).unwrap_err(),
        InflowRelocationError::UnsupportedPair
    );
}

/// A reader of the member's written register anywhere in the crossed run
/// keeps the old order: the inflow's tail, the `Jump` terminator itself,
/// and the join's prefix all refuse — the source program observed the
/// pre-member value there on this inflow's path. A reader before the
/// landing index or after the member's old index meets the dead-path
/// audit instead: behind the vacated index the member's write is missing
/// on the other inflows' arrivals, so it refuses there too.
#[test]
fn raw_hazard_keeps_order_through_the_edge() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let read_move = |function: &mut SelectedFunction,
                     environment: &register_environment::ValidatedTargetRegisterEnvironment,
                     block: usize,
                     index: usize,
                     id: SelectedInstructionId,
                     output: VirtualRegisterId| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[block].instructions[index] = instruction(
            id,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[R_MOVE, output],
        );
    };
    // The inflow's tail is crossed.
    let tail_reads = mutated(target, |function, environment| {
        read_move(function, environment, 1, 1, T_TAIL, R_TTAIL);
    });
    assert_eq!(
        relocate(&tail_reads, &environment, MOVING, T_TAIL).unwrap_err(),
        InflowRelocationError::UnsupportedPair
    );
    // The join's prefix is crossed.
    let head_reads = mutated(target, |function, environment| {
        read_move(function, environment, 3, 0, HEAD, R_HEAD);
    });
    assert_eq!(
        relocate(&head_reads, &environment, MOVING, T_TAIL).unwrap_err(),
        InflowRelocationError::UnsupportedPair
    );
    // A reader before the landing index is never crossed: on this
    // inflow's path the member ran behind it either way.
    let before_reads = mutated(target, |function, environment| {
        read_move(function, environment, 1, 0, T_HEAD, R_THEAD);
    });
    relocate(&before_reads, &environment, MOVING, T_TAIL).unwrap();
    // The `Jump` terminator is a crossed position: the member rises past
    // it, so its read of the member's result refuses.
    let jump_reads = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        let class = function.blocks[1].instructions[0].operands[0].class;
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
        relocate(&jump_reads, &environment, MOVING, T_TAIL).unwrap_err(),
        InflowRelocationError::UnsupportedPair
    );
    // Behind the vacated index the member's write is missing on the other
    // inflows' arrivals: the tail reader meets a stale value.
    let mid_reads = mutated(target, |function, environment| {
        read_move(function, environment, 3, 2, MID, R_MID);
    });
    assert_eq!(
        relocate(&mid_reads, &environment, MOVING, T_TAIL).unwrap_err(),
        InflowRelocationError::UnsupportedPair
    );
}

/// A writer of a register the member reads anywhere in the crossed run
/// would hand the member a different input at its new position: the
/// inflow's tail and the join's prefix refuse, while a writer before the
/// landing index already published the value the member reads on this
/// inflow's path.
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
            function.blocks[3].instructions[1] = instruction(
                MOVING,
                SelectedInstructionKind::CopyI64,
                &copy,
                &[POINTER, R_MOVE],
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
        writes_pointer(1, 1, T_TAIL)(function, environment);
    });
    assert_eq!(
        relocate(&tail_writes, &environment, MOVING, T_TAIL).unwrap_err(),
        InflowRelocationError::UnsupportedPair
    );
    let prefix_writes = mutated(target, |function, environment| {
        member_reads_pointer(function, environment);
        writes_pointer(3, 0, HEAD)(function, environment);
    });
    assert_eq!(
        relocate(&prefix_writes, &environment, MOVING, T_TAIL).unwrap_err(),
        InflowRelocationError::UnsupportedPair
    );
    // A writer before the landing index already published the input: the
    // member reads the same value at its new position on this inflow's
    // path.
    let before_writes = mutated(target, |function, environment| {
        member_reads_pointer(function, environment);
        writes_pointer(1, 0, T_HEAD)(function, environment);
    });
    relocate(&before_writes, &environment, MOVING, T_TAIL).unwrap();
    // A writer behind the member's old index is never crossed: the member
    // read ahead of it on the inflow's path either way.
    let behind_writes = mutated(target, |function, environment| {
        member_reads_pointer(function, environment);
        writes_pointer(3, 2, MID)(function, environment);
    });
    relocate(&behind_writes, &environment, MOVING, T_TAIL).unwrap();
}

/// A writer of the member's written register in the crossed run refuses —
/// either side would observe the other's value where the source observed
/// its own. A writer behind the member's old index simply retires the
/// missing definition: readers after it see the rewrite on every arrival,
/// exactly what the source computed.
#[test]
fn waw_hazard_keeps_order_through_the_edge() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let writes_move = |block: usize, index: usize, id: SelectedInstructionId| {
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
                &[R_TRAIL, R_MOVE],
            );
        }
    };
    let tail_writes = mutated(target, writes_move(1, 1, T_TAIL));
    assert_eq!(
        relocate(&tail_writes, &environment, MOVING, T_TAIL).unwrap_err(),
        InflowRelocationError::UnsupportedPair
    );
    let prefix_writes = mutated(target, writes_move(3, 0, HEAD));
    assert_eq!(
        relocate(&prefix_writes, &environment, MOVING, T_TAIL).unwrap_err(),
        InflowRelocationError::UnsupportedPair
    );
    // `MID` sits behind the member's old index: it overwrites the member's
    // register on every arrival, retiring the missing definition before
    // any reader — while `HEAD`, ahead of the vacated index, is overwritten
    // by it where the member's write stood.
    let retires = mutated(target, writes_move(3, 2, MID));
    relocate(&retires, &environment, MOVING, T_TAIL).unwrap();
    let still_live = mutated(target, |function, environment| {
        writes_move(3, 0, HEAD)(function, environment);
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[3].instructions[2] = instruction(
            MID,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[R_MOVE, R_MID],
        );
    });
    assert_eq!(
        relocate(&still_live, &environment, MOVING, T_TAIL).unwrap_err(),
        InflowRelocationError::UnsupportedPair
    );
}

/// Condition state couples the same way registers do: a member clobbering
/// the flags refuses against a crossed flag reader — in the inflow's tail
/// or the join's prefix — and against a reader behind the vacated index,
/// where the dead-path audit sees the missing flags still live; a
/// flag-reading member refuses a crossed flag writer in either block.
/// Pure flag work in either direction admits when nothing crossed or
/// reached observes it.
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
        flag_writer(function, environment, 3, 1, MOVING, R_MOVE);
    });
    relocate(&member_writes_flags, &environment, MOVING, T_TAIL).unwrap();
    // A crossed flag reader in the inflow's tail refuses.
    let tail_reads_flags = mutated(target, |function, environment| {
        flag_writer(function, environment, 3, 1, MOVING, R_MOVE);
        flag_reader(function, environment, 1, 1, T_TAIL, R_TTAIL);
    });
    assert_eq!(
        relocate(&tail_reads_flags, &environment, MOVING, T_TAIL).unwrap_err(),
        InflowRelocationError::UnsupportedPair
    );
    // A crossed flag reader in the join's prefix refuses.
    let prefix_reads_flags = mutated(target, |function, environment| {
        flag_writer(function, environment, 3, 1, MOVING, R_MOVE);
        flag_reader(function, environment, 3, 0, HEAD, R_HEAD);
    });
    assert_eq!(
        relocate(&prefix_reads_flags, &environment, MOVING, T_TAIL).unwrap_err(),
        InflowRelocationError::UnsupportedPair
    );
    // A flag reader behind the vacated index meets the member's missing
    // flags still live on the other inflows' arrivals.
    let behind_reads_flags = mutated(target, |function, environment| {
        flag_writer(function, environment, 3, 1, MOVING, R_MOVE);
        flag_reader(function, environment, 3, 2, MID, R_MID);
    });
    assert_eq!(
        relocate(&behind_reads_flags, &environment, MOVING, T_TAIL).unwrap_err(),
        InflowRelocationError::UnsupportedPair
    );
    // A flag-reading member refuses a crossed flag writer in the inflow's
    // tail.
    let tail_writes_flags = mutated(target, |function, environment| {
        flag_reader(function, environment, 3, 1, MOVING, R_MOVE);
        flag_writer(function, environment, 1, 1, T_TAIL, R_TTAIL);
    });
    assert_eq!(
        relocate(&tail_writes_flags, &environment, MOVING, T_TAIL).unwrap_err(),
        InflowRelocationError::UnsupportedPair
    );
    // … and in the join's prefix.
    let head_writes_flags = mutated(target, |function, environment| {
        flag_reader(function, environment, 3, 1, MOVING, R_MOVE);
        flag_writer(function, environment, 3, 0, HEAD, R_HEAD);
    });
    assert_eq!(
        relocate(&head_writes_flags, &environment, MOVING, T_TAIL).unwrap_err(),
        InflowRelocationError::UnsupportedPair
    );
    // With nothing crossed writing the flags the flag-reading member
    // rises freely: its output dies unread on every arrival.
    let member_reads_flags = mutated(target, |function, environment| {
        flag_reader(function, environment, 3, 1, MOVING, R_MOVE);
    });
    relocate(&member_reads_flags, &environment, MOVING, T_TAIL).unwrap();
}

/// Both terminators bordering the window couple: the inflow's `Jump` — a
/// crossed position — refuses a read of the member's register, while a
/// read of anything else crosses freely; the join's `Return` is a
/// dead-path position, so a read of the member's still-live register
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
            &mutated(target, jump_reads(R_MOVE)),
            &environment,
            MOVING,
            T_TAIL,
        )
        .unwrap_err(),
        InflowRelocationError::UnsupportedPair
    );
    relocate(
        &mutated(target, jump_reads(R_TTAIL)),
        &environment,
        MOVING,
        T_TAIL,
    )
    .unwrap();
    // The join's return is a dead-path position: a read of the member's
    // still-live register observes the missing definition.
    let return_reads = mutated(target, |function, environment| {
        let return_row = environment
            .constraint(environment.selected_keys().return_unit)
            .unwrap()
            .clone();
        let class = function.blocks[3].instructions[0].operands[0].class;
        let mut ret = instruction(RET, SelectedInstructionKind::ReturnUnit, &return_row, &[]);
        ret.operands.push(SelectedOperand {
            operand: 0,
            virtual_register: R_MOVE,
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
        relocate(&return_reads, &environment, MOVING, T_TAIL).unwrap_err(),
        InflowRelocationError::UnsupportedPair
    );
    // The other inflow's terminator is never crossed or walked: its read
    // of the member's register sees the value its own path always had —
    // on its arrival the member's write was still ahead, and it stays so.
    let other_terminator_reads = mutated(target, |function, environment| {
        let jump_row = environment
            .constraint(environment.selected_keys().jump)
            .unwrap()
            .clone();
        let class = function.blocks[2].instructions[0].operands[0].class;
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
    relocate(&other_terminator_reads, &environment, MOVING, T_TAIL).unwrap();
}

/// Barrier kinds and call-roster entries refuse as the member or anywhere
/// inside the crossed window — the inflow's tail, the `Jump` terminator's
/// call contract, and the join's prefix included — while a barrier before
/// the landing index, behind the member's old index, or on the other
/// inflow is never crossed: every crossed position is one the member
/// trades order with, and a rostered crossed position is an accounted
/// access the row-less member passes without reordering.
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
            function.blocks[3].instructions[1].kind = kind;
        });
        assert_eq!(
            relocate(&member_barrier, &environment, MOVING, T_TAIL).unwrap_err(),
            InflowRelocationError::UnsupportedInstruction,
            "member {kind:?}"
        );
        let tail_barrier = mutated(target, |function, _| {
            function.blocks[1].instructions[1].kind = kind;
        });
        assert_eq!(
            relocate(&tail_barrier, &environment, MOVING, T_TAIL).unwrap_err(),
            InflowRelocationError::UnsupportedInstruction,
            "crossed tail {kind:?}"
        );
        let prefix_barrier = mutated(target, |function, _| {
            function.blocks[3].instructions[0].kind = kind;
        });
        assert_eq!(
            relocate(&prefix_barrier, &environment, MOVING, T_TAIL).unwrap_err(),
            InflowRelocationError::UnsupportedInstruction,
            "crossed prefix {kind:?}"
        );
        // A barrier before the landing index is never crossed — the
        // member lands behind it either way.
        let head_barrier = mutated(target, |function, _| {
            function.blocks[1].instructions[0].kind = kind;
        });
        relocate(&head_barrier, &environment, MOVING, T_TAIL).unwrap();
        // A barrier behind the member's old index is never crossed — the
        // member leaves ahead of it either way.
        let behind_barrier = mutated(target, |function, _| {
            function.blocks[3].instructions[2].kind = kind;
        });
        relocate(&behind_barrier, &environment, MOVING, T_TAIL).unwrap();
        // A barrier on the other inflow still runs on its own path; the
        // member never enters its stream.
        let inflow_barrier = mutated(target, |function, _| {
            function.blocks[2].instructions[0].kind = kind;
        });
        relocate(&inflow_barrier, &environment, MOVING, T_TAIL).unwrap();
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
        (MOVING, T_TAIL),
        (T_TAIL, T_TAIL),
        (HEAD, T_TAIL),
        (T_JUMP, T_TAIL),
    ] {
        let contract = mutated(target, |function, _| {
            function.calls.push(call_contract(instruction_id));
        });
        assert_eq!(
            relocate(&contract, &environment, MOVING, destination).unwrap_err(),
            InflowRelocationError::UnsupportedInstruction,
            "call roster {instruction_id:?}"
        );
    }
    // A call-roster row before the landing index, behind the member's old
    // index, on the join's own terminator, or on the other inflow is
    // outside the window: those positions keep the member on the side
    // they always had.
    for instruction_id in [T_HEAD, MID, TAIL, RET, F_HEAD, F_JUMP, LEAD, TRAIL, BRANCH] {
        let contract = mutated(target, |function, _| {
            function.calls.push(call_contract(instruction_id));
        });
        relocate(&contract, &environment, MOVING, T_TAIL).unwrap();
    }
    // A rostered crossed position is an accounted access the row-less
    // member passes without reordering a recorded access.
    let rostered_tail = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
        function.blocks[1].instructions[1] = instruction(
            T_TAIL,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            &store,
            &[POINTER, R_TTAIL],
        );
        function.memory_accesses.push(access(
            T_TAIL,
            PlaceId::new(2).unwrap(),
            SelectedMemoryAccessRole::WritePlace,
        ));
    });
    relocate(&rostered_tail, &environment, MOVING, T_TAIL).unwrap();
    // An unaccounted memory-capable crossed position can never trade
    // order at all: a row-less `Store` reaches place-backed storage the
    // roster does not record.
    let rowless_tail_store = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap()
            .clone();
        function.blocks[1].instructions[1] = instruction(
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
        relocate(&rowless_tail_store, &environment, MOVING, T_TAIL).unwrap_err(),
        InflowRelocationError::UnsupportedInstruction
    );
}

/// Only pure register and condition-state work may rise: a member
/// carrying a memory roster row would silently drop its access from every
/// arrival through the other inflows, and a row-less load or private-slot
/// store loses the same access — as would a kind whose target encoding
/// may fault on the arrivals that ran it before. Memory work on the other
/// inflow never moves: its positions run before the member's old position
/// on their own arrivals, and the member never enters their stream.
#[test]
fn member_must_be_pure_work() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A roster-carrying member's recorded access would vanish from the
    // other inflows' arrivals.
    let roster_member = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap()
            .clone();
        function.blocks[3].instructions[1] = instruction(
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
        relocate(&roster_member, &environment, MOVING, T_TAIL).unwrap_err(),
        InflowRelocationError::UnsupportedInstruction
    );
    // A row-less load still performs an access — and carries any fault —
    // on every arrival it ran on before.
    let rowless_load = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap()
            .clone();
        function.blocks[3].instructions[1] = instruction(
            MOVING,
            SelectedInstructionKind::Load8 { byte_offset: 0 },
            &load,
            &[POINTER, R_MOVE],
        );
    });
    assert_eq!(
        relocate(&rowless_load, &environment, MOVING, T_TAIL).unwrap_err(),
        InflowRelocationError::UnsupportedInstruction
    );
    // A row-less private-slot store still writes storage on every
    // traversal; hoisting it would drop the write from the other inflows'
    // arrivals.
    let rowless_store = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap()
            .clone();
        function.blocks[3].instructions[1] = instruction(
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
        relocate(&rowless_store, &environment, MOVING, T_TAIL).unwrap_err(),
        InflowRelocationError::UnsupportedInstruction
    );
    // A potentially-faulting kind ran on every arrival: the divide would
    // silently stop faulting on the arrivals the move removes.
    let exact_divide = mutated(target, |function, environment| {
        let divide = environment
            .constraint(environment.selected_keys().divide_u64)
            .unwrap()
            .clone();
        function.blocks[3].instructions[1] = instruction(
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
        relocate(&exact_divide, &environment, MOVING, T_TAIL).unwrap_err(),
        InflowRelocationError::UnsupportedInstruction
    );
    let saturating_divide = mutated(target, |function, environment| {
        let divide = environment
            .constraint(environment.selected_keys().saturating_divide_signed)
            .unwrap()
            .clone();
        function.blocks[3].instructions[1] = instruction(
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
        relocate(&saturating_divide, &environment, MOVING, T_TAIL).unwrap_err(),
        InflowRelocationError::UnsupportedInstruction
    );
    // Memory work on the other inflow never moves: a rostered store in F
    // runs before the member's old position on its own arrivals whatever
    // it reads.
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
            &[R_MOVE, R_FHEAD],
        );
        function.memory_accesses.push(access(
            F_HEAD,
            PlaceId::new(2).unwrap(),
            SelectedMemoryAccessRole::WritePlace,
        ));
    });
    relocate(&inflow_store, &environment, MOVING, T_TAIL).unwrap();
    // A rostered position ahead of the vacated index is a dead-path
    // position like any other: it refuses only when it would read a
    // still-live member definition.
    let prefix_store = mutated(target, |function, environment| {
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
            PlaceId::new(3).unwrap(),
            SelectedMemoryAccessRole::WritePlace,
        ));
    });
    relocate(&prefix_store, &environment, MOVING, T_TAIL).unwrap();
    let prefix_store_reads_member = mutated(target, |function, environment| {
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
            &[R_MOVE, R_HEAD],
        );
        function.memory_accesses.push(access(
            HEAD,
            PlaceId::new(3).unwrap(),
            SelectedMemoryAccessRole::WritePlace,
        ));
    });
    assert_eq!(
        relocate(&prefix_store_reads_member, &environment, MOVING, T_TAIL).unwrap_err(),
        InflowRelocationError::UnsupportedPair
    );
}

/// Every location the member writes must be dead — unread until
/// rewritten — on every arrival through the join's other inflows: a
/// reader behind the vacated index, in the join's terminator, in a deeper
/// successor block, or in a block a loop reaches all refuse, while a
/// write that retires the missing definition before any reader admits. A
/// loop back through the join rescans its positions against the live
/// missing set — the vacated index republishes them on every re-entry —
/// and a loop back through the inflow block runs the member at its new
/// position, where its write republishes and clears.
#[test]
fn member_writes_must_die_on_the_other_inflows() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let reads_move =
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
                &[R_MOVE, output],
            );
        }
        };
    let writes_move = |block: usize, index: usize, id: SelectedInstructionId| {
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
                &[R_TRAIL, R_MOVE],
            );
        }
    };
    // A reader anywhere behind the vacated index observes the missing
    // definition on the other inflows' arrivals.
    for (block, index, id, output) in [(3usize, 2usize, MID, R_MID), (3, 3, TAIL, R_TAIL)] {
        let reader = mutated(target, reads_move(block, index, id, output));
        assert_eq!(
            relocate(&reader, &environment, MOVING, T_TAIL).unwrap_err(),
            InflowRelocationError::UnsupportedPair,
            "reader {id:?}"
        );
    }
    // A write that retires the missing definition before any reader
    // admits: `TAIL` observes `MID`'s rewrite on every arrival, exactly
    // what the source computed.
    let retired = mutated(target, |function, environment| {
        writes_move(3, 2, MID)(function, environment);
        reads_move(3, 3, TAIL, R_TAIL)(function, environment);
    });
    relocate(&retired, &environment, MOVING, T_TAIL).unwrap();
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
    // The deep block reading `R_MOVE` observes the missing definition.
    let deep_reads = deep(&mut |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[4].instructions[0] = instruction(
            DEEP,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[R_MOVE, R_DEEP],
        );
    });
    assert_eq!(
        relocate(&deep_reads, &environment, MOVING, T_TAIL).unwrap_err(),
        InflowRelocationError::UnsupportedPair
    );
    // A deep write retires the definition before the read.
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
                &[R_TRAIL, R_MOVE],
            ),
        );
        function.blocks[4].instructions[1] = instruction(
            DEEP,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[R_MOVE, R_DEEP],
        );
    });
    relocate(&deep_retired, &environment, MOVING, T_TAIL).unwrap();
    // A loop back through the other inflow's block: J exits to F, F
    // re-enters the join, and the member's still-live definition meets
    // F's reader on the second traversal.
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
        reads_move(2, 0, F_HEAD, R_FHEAD)(function, environment);
    });
    assert_eq!(
        relocate(&looped_reads, &environment, MOVING, T_TAIL).unwrap_err(),
        InflowRelocationError::UnsupportedPair
    );
    // The looped inflow's own write retires the missing definition before
    // its reader — and the re-entering edge carries nothing live back.
    let looped_retired = looped(&mut |function, environment| {
        writes_move(2, 0, F_HEAD)(function, environment);
        reads_move(2, 1, F_TAIL, R_FTAIL)(function, environment);
    });
    relocate(&looped_retired, &environment, MOVING, T_TAIL).unwrap();
    // The re-entering edge's transports are boundary positions: a
    // transport argument reading the member's still-live register on a
    // re-crossed inflow edge refuses, even though the same edge admits
    // freely when no loop carries the missing definition back to it.
    let looped_edge_reads = looped(&mut |function, _| {
        let successor = match &mut function.blocks[2].terminator {
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
                argument: R_MOVE,
                parameter: R_BOUND,
            },
        });
    });
    assert_eq!(
        relocate(&looped_edge_reads, &environment, MOVING, T_TAIL).unwrap_err(),
        InflowRelocationError::UnsupportedPair
    );
    // A loop back through the inflow block itself converges: J exits to
    // T, the member's fresh execution at its new position clears the live
    // set, and the T-edge carries nothing live back into the join.
    let inflow_loop = |edit: &mut dyn FnMut(
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
                successor(BLOCK_T, BlockId::new(2).unwrap(), EDGE_JD),
            );
            edit(function, environment);
        })
    };
    relocate(&inflow_loop(&mut |_, _| {}), &environment, MOVING, T_TAIL).unwrap();
    // A reader ahead of the member's new position on the re-entered
    // inflow meets the still-live missing definition: on an F-arrival
    // looping through T the member's write never happened where the
    // source's did.
    let inflow_loop_reads = inflow_loop(&mut |function, environment| {
        reads_move(1, 0, T_HEAD, R_THEAD)(function, environment);
    });
    assert_eq!(
        relocate(&inflow_loop_reads, &environment, MOVING, T_TAIL).unwrap_err(),
        InflowRelocationError::UnsupportedPair
    );
}

/// A boundary settlement observes the executed prefix at its position:
/// any settlement past the member's index in its own block or past the
/// landing index in the inflow observes a different executed set once the
/// member lands on the other side of the crossed run, while positions at
/// or before either boundary — and settlements in blocks the member's
/// stream never touches — keep the prefix they always had.
#[test]
fn boundary_settlements_bound_the_window() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Past the member's index in its own block.
    let past_member = mutated(target, |function, _| {
        function
            .boundary_settlements
            .push(settlement(BLOCK_J, 2, 60));
    });
    assert_eq!(
        relocate(&past_member, &environment, MOVING, T_TAIL).unwrap_err(),
        InflowRelocationError::UnsupportedPair
    );
    // Past the landing index in the inflow.
    let past_landing = mutated(target, |function, _| {
        function
            .boundary_settlements
            .push(settlement(BLOCK_T, 2, 61));
    });
    assert_eq!(
        relocate(&past_landing, &environment, MOVING, T_TAIL).unwrap_err(),
        InflowRelocationError::UnsupportedPair
    );
    // At or before either boundary the executed prefix is unchanged.
    let at_member = mutated(target, |function, _| {
        function
            .boundary_settlements
            .push(settlement(BLOCK_J, 1, 62));
        function
            .boundary_settlements
            .push(settlement(BLOCK_T, 1, 63));
    });
    relocate(&at_member, &environment, MOVING, T_TAIL).unwrap();
    // Landing earlier moves the inflow's boundary with it.
    let earlier = mutated(target, |function, _| {
        function
            .boundary_settlements
            .push(settlement(BLOCK_T, 1, 64));
    });
    assert_eq!(
        relocate(&earlier, &environment, MOVING, T_HEAD).unwrap_err(),
        InflowRelocationError::UnsupportedPair
    );
    relocate(&earlier, &environment, MOVING, T_TAIL).unwrap();
    // Settlements in blocks the member never enters — the other inflow,
    // the fork head — keep their executed prefixes.
    let elsewhere = mutated(target, |function, _| {
        function
            .boundary_settlements
            .push(settlement(BLOCK_F, 2, 65));
        function
            .boundary_settlements
            .push(settlement(BLOCK_B, 2, 66));
    });
    relocate(&elsewhere, &environment, MOVING, T_TAIL).unwrap();
}

/// Only the named member inside the named confluence relocates: unknown
/// member or function ids, a terminator-carried member, and destinations
/// naming no inflow landing all refuse without touching the plan.
#[test]
fn only_the_named_inflow_window_relocates() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // An unknown member or function index never locates the window.
    assert_eq!(
        relocate_selected_instruction_onto_inflow(
            &source,
            0,
            SelectedInstructionId(99),
            T_TAIL,
            &environment,
            budget(),
        )
        .unwrap_err(),
        InflowRelocationError::SourceMismatch
    );
    assert_eq!(
        relocate_selected_instruction_onto_inflow(
            &source,
            9,
            MOVING,
            T_TAIL,
            &environment,
            budget(),
        )
        .unwrap_err(),
        InflowRelocationError::SourceMismatch
    );
    // Naming a terminator-carried instruction as member is not a body
    // position.
    assert_eq!(
        relocate(&source, &environment, T_JUMP, T_TAIL).unwrap_err(),
        InflowRelocationError::SourceMismatch
    );
    assert_eq!(
        relocate(&source, &environment, RET, T_TAIL).unwrap_err(),
        InflowRelocationError::SourceMismatch
    );
    assert_eq!(
        relocate(&source, &environment, BRANCH, T_TAIL).unwrap_err(),
        InflowRelocationError::SourceMismatch
    );
    // A destination naming no inflow landing position refuses: the
    // member's own block mates, the fork head's, and dangling ids.
    for destination in [
        LEAD,
        TRAIL,
        BRANCH,
        HEAD,
        MOVING,
        MID,
        TAIL,
        RET,
        SelectedInstructionId(99),
    ] {
        assert_eq!(
            relocate(&source, &environment, MOVING, destination).unwrap_err(),
            InflowRelocationError::UnsupportedPair,
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
    let result = relocate(&source, &environment, MOVING, T_HEAD).unwrap();
    // The honest proposal replays.
    validate_inflow_relocation(
        &source,
        0,
        MOVING,
        T_HEAD,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // The member at the wrong index rejects.
    let mut displaced = result.transformed().clone();
    let member = displaced.functions[0].blocks[1].instructions.remove(0);
    displaced.functions[0].blocks[1]
        .instructions
        .insert(2, member.clone());
    assert_eq!(
        validate_inflow_relocation(
            &source,
            0,
            MOVING,
            T_HEAD,
            &environment,
            budget(),
            displaced,
        )
        .unwrap_err(),
        InflowRelocationError::ReplayMismatch
    );
    // The member left in its own block rejects.
    let mut unmoved = result.transformed().clone();
    unmoved.functions[0].blocks[1].instructions.remove(0);
    unmoved.functions[0].blocks[3]
        .instructions
        .insert(1, member);
    assert_eq!(
        validate_inflow_relocation(&source, 0, MOVING, T_HEAD, &environment, budget(), unmoved,)
            .unwrap_err(),
        InflowRelocationError::ReplayMismatch
    );
    // A dropped instruction in the inflow rejects.
    let mut dropped = result.transformed().clone();
    dropped.functions[0].blocks[1].instructions.pop();
    assert_eq!(
        validate_inflow_relocation(&source, 0, MOVING, T_HEAD, &environment, budget(), dropped,)
            .unwrap_err(),
        InflowRelocationError::ReplayMismatch
    );
    // An unrelated literal edit inside the inflow rejects.
    let mut edited = result.transformed().clone();
    edited.functions[0].blocks[1].instructions[0].kind = SelectedInstructionKind::MaterializeI64 {
        value: IntegerValue::Unsigned(12),
    };
    assert_eq!(
        validate_inflow_relocation(&source, 0, MOVING, T_HEAD, &environment, budget(), edited,)
            .unwrap_err(),
        InflowRelocationError::ReplayMismatch
    );
    // Naming a different window on the same proposal re-derives a
    // different landing and rejects.
    assert_eq!(
        validate_inflow_relocation(
            &source,
            0,
            MOVING,
            T_TAIL,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap_err(),
        InflowRelocationError::ReplayMismatch
    );
}

/// The bounded audit is measured: the inflow window prices every scan,
/// crossed-surface pair, roster row, and the dead-path fixpoint bound
/// against the work budget, and a budget one step short refuses rather
/// than skimping. Landing deeper into the join's prefix crosses more join
/// positions.
#[test]
fn measured_validation_step_boundary() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // The member-locate scan prices every block's body plus terminator
    // once across the plan (3+3+3+5 = 14), again for this function's
    // blocks (14), and each block's successor edge count (2+1+1+0 = 4).
    // The crossed surfaces pair the member (1) against `T_TAIL` (1),
    // `HEAD` (1), and the `Jump` terminator (2 uses + defs on x86-64):
    // 2+2+3 = 7 steps. The dead-path bound prices each block's body,
    // terminator, and edge surfaces once per member location plus the
    // initial scan: on x86-64 the materializations cost 1 each, the jumps
    // 2, the branch 3, and the return 9 — (2+3)+(2+2)+(2+2)+(4+9) = 26 —
    // times one written member register plus one: 26*2 = 52.
    let steps: u64 = 14 + 14 + 4 + 7 + 52;
    let exact = OptimizationWorkBudget::new(1, 1, steps, 1, 1).unwrap();
    relocate_selected_instruction_onto_inflow(&source, 0, MOVING, T_TAIL, &environment, exact)
        .unwrap();
    let starved = OptimizationWorkBudget::new(1, 1, steps - 1, 1, 1).unwrap();
    assert_eq!(
        relocate_selected_instruction_onto_inflow(
            &source,
            0,
            MOVING,
            T_TAIL,
            &environment,
            starved,
        )
        .unwrap_err(),
        InflowRelocationError::WorkBudgetExceeded
    );
    // Landing at the body end crosses the whole inflow body: the member
    // pairs against `T_HEAD`, `T_TAIL`, `HEAD`, and the terminator.
    let steps_head: u64 = 14 + 14 + 4 + (2 + 2 + 2 + 3) + 52;
    let exact = OptimizationWorkBudget::new(1, 1, steps_head, 1, 1).unwrap();
    relocate_selected_instruction_onto_inflow(&source, 0, MOVING, T_HEAD, &environment, exact)
        .unwrap();
    let starved = OptimizationWorkBudget::new(1, 1, steps_head - 1, 1, 1).unwrap();
    assert_eq!(
        relocate_selected_instruction_onto_inflow(
            &source,
            0,
            MOVING,
            T_HEAD,
            &environment,
            starved,
        )
        .unwrap_err(),
        InflowRelocationError::WorkBudgetExceeded
    );
}

/// The environment's target must be the plan's target: a relocation proven
/// for one target's constraints is not evidence on another.
#[test]
fn target_mismatch_rejects() {
    let source = fixture(NativeTarget::linux_x64());
    let environment = baseline_target_register_environment(NativeTarget::linux_arm64()).unwrap();
    assert_eq!(
        relocate(&source, &environment, MOVING, T_TAIL).unwrap_err(),
        InflowRelocationError::SourceMismatch
    );
}

/// Two runs over the identical source produce the identical validated
/// result, and the published plan is a legal second input through the
/// sealed analysis boundary: the join's next member rises onto the same
/// inflow, a member whose write is still observed on the other inflow's
/// arrivals declines, and an in-block family still admits.
#[test]
fn inflow_relocation_is_deterministic_and_re_admitted() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // `TAIL` reading `R_MOVE` keeps `MOVING` pinned to the join on every
    // later input — its write is still observed on the other inflow's
    // arrivals — while `HEAD` rises freely.
    let source = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap()
            .clone();
        function.blocks[3].instructions[3] = instruction(
            TAIL,
            SelectedInstructionKind::CopyI64,
            &copy,
            &[R_MOVE, R_TAIL],
        );
    });
    let first = relocate(&source, &environment, HEAD, T_TAIL).unwrap();
    let second = relocate(&source, &environment, HEAD, T_TAIL).unwrap();
    assert_eq!(first, second);
    // The validated output carries the sealed analysis boundary, so it is
    // a legal second input — not merely a reconstruction of one. The
    // join's next member rises onto the same inflow behind `HEAD`.
    let again =
        relocate_selected_instruction_onto_inflow(&first, 0, MID, T_TAIL, &environment, budget())
            .unwrap();
    assert_eq!(
        again.transformed().functions[0].blocks[1]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![T_HEAD, HEAD, MID, T_TAIL]
    );
    assert_eq!(
        again.receipt().source_selected(),
        first.receipt().transformed_selected()
    );
    // A member whose write is still observed on the other inflow's
    // arrivals declines on the second input: `MOVING` cannot rise while
    // `TAIL` reads `R_MOVE` behind its vacated index.
    assert_eq!(
        relocate_selected_instruction_onto_inflow(
            &first,
            0,
            MOVING,
            T_TAIL,
            &environment,
            budget(),
        )
        .unwrap_err(),
        InflowRelocationError::UnsupportedPair
    );
    // A different scheduling family still admits on the second input.
    let swapped =
        crate::relocate_selected_instruction(&first, 0, MOVING, MID, &environment, budget())
            .unwrap();
    assert_eq!(
        swapped.transformed().functions[0].blocks[3]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![MID, MOVING, TAIL]
    );
}
