use optimization_core::OptimizationUnitIdentity;
use optimization_unit::{EffectLink, ValueDefinitionSite};
use register_environment::baseline_target_register_environment;
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedBoundarySettlement,
    SelectedBoundarySettlementPayload, SelectedCallContract, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan, SelectedMemoryAccess,
    SelectedMemoryAccessOrigin, SelectedMemoryAccessRole, SelectedStructuralBinding,
    SelectedStructuralTransport, SelectedSuccessor, SelectedSuccessorRole, SelectedTerminator,
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
    ConfluenceRelocationError, ConfluenceRelocationReceipt, ValidatedConfluenceRelocation,
    relocate_selected_members_into_confluence, validate_confluence_relocation,
};
use crate::rewrites::test_support::{budget, instruction, measured_step_budget};

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
fn fixture(target: NativeTarget) -> ValidatedConfluenceRelocation {
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
    ValidatedConfluenceRelocation {
        receipt: ConfluenceRelocationReceipt {
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
) -> ValidatedConfluenceRelocation {
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
    source: &ValidatedConfluenceRelocation,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    first: SelectedInstructionId,
    last: SelectedInstructionId,
    destination: SelectedInstructionId,
) -> Result<ValidatedConfluenceRelocation, ConfluenceRelocationError> {
    relocate_selected_members_into_confluence(
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

/// The two named members bound the run. Naming one member twice bounds the
/// run of one, which this admission carries itself; a last member that does
/// not follow the first, or that sits outside the first's block, bounds no
/// span at all.
#[test]
fn the_named_members_bound_a_contiguous_run() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // The run of one: the same admission, one member wide.
    let one = relocate(&source, &environment, RUN_A, RUN_A, HEAD).unwrap();
    validate_confluence_relocation(
        &source,
        0,
        RUN_A,
        RUN_A,
        HEAD,
        &environment,
        budget(),
        one.transformed().clone(),
    )
    .unwrap();
    // Reversed names bound no span.
    assert_eq!(
        relocate(&source, &environment, RUN_B, RUN_A, HEAD).unwrap_err(),
        ConfluenceRelocationError::UnsupportedPair
    );
    // A last member in another block bounds no span in the first's.
    for last in [LEAD, F_HEAD, HEAD, T_JUMP, RET, SelectedInstructionId(99)] {
        assert_eq!(
            relocate(&source, &environment, RUN_A, last, HEAD).unwrap_err(),
            ConfluenceRelocationError::UnsupportedPair,
            "last {last:?}"
        );
    }
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
        ConfluenceRelocationError::UnsupportedPair
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
            ConfluenceRelocationError::UnsupportedPair,
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
        ConfluenceRelocationError::UnsupportedPair
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
        ConfluenceRelocationError::UnsupportedPair
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
        ConfluenceRelocationError::UnsupportedPair
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
        ConfluenceRelocationError::UnsupportedPair
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
            ConfluenceRelocationError::UnsupportedPair
        );
    }
    let case_edge = mutated(target, |function, _| {
        let successor = match &mut function.blocks[1].terminator {
            SelectedTerminator::Jump { successor, .. } => successor,
            _ => unreachable!(),
        };
        successor.structural_case = Some(selected_instructions::SelectedStructuralCaseEdge {
            source: selected_instructions::SelectedCaseDispatchSource::Local {
                slot: selected_instructions::LocalStorageSlotId::Spill { register: R_BOUND },
            },
            case: StructuralCaseId::new(1).unwrap(),
            case_tag: 0,
            payloads: Vec::new(),
            trivial_affine_discards: Vec::new(),
        });
    });
    assert_eq!(
        relocate(&case_edge, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        ConfluenceRelocationError::UnsupportedPair
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
        ConfluenceRelocationError::UnsupportedPair
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
        ConfluenceRelocationError::UnsupportedPair
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
            source: selected_instructions::SelectedCaseDispatchSource::Local {
                slot: selected_instructions::LocalStorageSlotId::Spill { register: R_BOUND },
            },
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
                ConfluenceRelocationError::UnsupportedInstruction,
                "member {kind:?} at index {index}"
            );
        }
        let tail_barrier = mutated(target, |function, _| {
            function.blocks[1].instructions[3].kind = kind;
        });
        assert_eq!(
            relocate(&tail_barrier, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
            ConfluenceRelocationError::UnsupportedInstruction,
            "crossed tail {kind:?}"
        );
        let prefix_barrier = mutated(target, |function, _| {
            function.blocks[3].instructions[0].kind = kind;
        });
        assert_eq!(
            relocate(&prefix_barrier, &environment, RUN_A, RUN_B, MID).unwrap_err(),
            ConfluenceRelocationError::UnsupportedInstruction,
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
            ConfluenceRelocationError::UnsupportedInstruction,
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
        ConfluenceRelocationError::UnsupportedInstruction
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
            ConfluenceRelocationError::UnsupportedInstruction,
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
        ConfluenceRelocationError::UnsupportedInstruction
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
        ConfluenceRelocationError::UnsupportedInstruction
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
        ConfluenceRelocationError::UnsupportedInstruction
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
        ConfluenceRelocationError::UnsupportedInstruction
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
        ConfluenceRelocationError::UnsupportedInstruction
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
        ConfluenceRelocationError::UnsupportedPair
    );
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
        ConfluenceRelocationError::UnsupportedPair
    );
    // Past the run's span in its own block.
    let past_run = mutated(target, |function, _| {
        function
            .boundary_settlements
            .push(settlement(BLOCK_T, 3, 61));
    });
    assert_eq!(
        relocate(&past_run, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        ConfluenceRelocationError::UnsupportedPair
    );
    // Past the landing index in the join.
    let past_landing = mutated(target, |function, _| {
        function
            .boundary_settlements
            .push(settlement(BLOCK_J, 1, 62));
    });
    assert_eq!(
        relocate(&past_landing, &environment, RUN_A, RUN_B, HEAD).unwrap_err(),
        ConfluenceRelocationError::UnsupportedPair
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
        ConfluenceRelocationError::UnsupportedPair
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
        relocate_selected_members_into_confluence(
            &source,
            0,
            SelectedInstructionId(99),
            RUN_B,
            HEAD,
            &environment,
            budget(),
        )
        .unwrap_err(),
        ConfluenceRelocationError::SourceMismatch
    );
    assert_eq!(
        relocate_selected_members_into_confluence(
            &source,
            9,
            RUN_A,
            RUN_B,
            HEAD,
            &environment,
            budget(),
        )
        .unwrap_err(),
        ConfluenceRelocationError::SourceMismatch
    );
    // Naming a terminator-carried instruction as the run's first member
    // is not a body position.
    for first in [T_JUMP, RET, BRANCH] {
        assert_eq!(
            relocate(&source, &environment, first, T_TAIL, HEAD).unwrap_err(),
            ConfluenceRelocationError::SourceMismatch,
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
            ConfluenceRelocationError::UnsupportedPair,
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
    validate_confluence_relocation(
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
        validate_confluence_relocation(
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
        ConfluenceRelocationError::ReplayMismatch
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
        validate_confluence_relocation(
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
        ConfluenceRelocationError::ReplayMismatch
    );
    // A dropped instruction in the join rejects.
    let mut dropped = result.transformed().clone();
    dropped.functions[0].blocks[3].instructions.pop();
    assert_eq!(
        validate_confluence_relocation(
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
        ConfluenceRelocationError::ReplayMismatch
    );
    // An unrelated literal edit inside the join rejects.
    let mut edited = result.transformed().clone();
    edited.functions[0].blocks[3].instructions[0].kind = SelectedInstructionKind::MaterializeI64 {
        value: IntegerValue::Unsigned(12),
    };
    assert_eq!(
        validate_confluence_relocation(
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
        ConfluenceRelocationError::ReplayMismatch
    );
    // Naming a different window on the same proposal re-derives a
    // different landing and rejects.
    assert_eq!(
        validate_confluence_relocation(
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
        ConfluenceRelocationError::ReplayMismatch
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
    // blocks (15); the path walk pushes the lone `Jump` edge once (1).
    // The crossed surfaces pair each member (1) against `T_TAIL` (1) —
    // 2 per member — and each member against the `Jump` terminator (2
    // uses + defs on x86-64) plus the plain edge's empty surface — 3 per
    // member: (2+3)*2 = 10 steps. The dead-path bound prices each
    // block's body, terminator, and edge surfaces once per run location
    // plus the initial scan: on x86-64 the materializations cost 1 each,
    // the jumps 2, the branch 3, and the return 9 — (2+3)+(4+2)+(2+2)+
    // (3+9) = 27 — times two written member registers plus one:
    // 27*3 = 81.
    let steps: u64 = 15 + 15 + 1 + 10 + 81;
    let exact = measured_step_budget(steps);
    relocate_selected_members_into_confluence(&source, 0, RUN_A, RUN_B, HEAD, &environment, exact)
        .unwrap();
    let starved = measured_step_budget(steps - 1);
    assert_eq!(
        relocate_selected_members_into_confluence(
            &source,
            0,
            RUN_A,
            RUN_B,
            HEAD,
            &environment,
            starved,
        )
        .unwrap_err(),
        ConfluenceRelocationError::WorkBudgetExceeded
    );
    // Landing at the body end crosses the whole join body: each member
    // pairs against `T_TAIL`, `HEAD`, `MID`, `TAIL`, and the `Jump`
    // terminator's edge — (2+2+2+2+3) per member = 22.
    let steps_end: u64 = 15 + 15 + 1 + 22 + 81;
    let exact = measured_step_budget(steps_end);
    relocate_selected_members_into_confluence(&source, 0, RUN_A, RUN_B, RET, &environment, exact)
        .unwrap();
    let starved = measured_step_budget(steps_end - 1);
    assert_eq!(
        relocate_selected_members_into_confluence(
            &source,
            0,
            RUN_A,
            RUN_B,
            RET,
            &environment,
            starved,
        )
        .unwrap_err(),
        ConfluenceRelocationError::WorkBudgetExceeded
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
        ConfluenceRelocationError::SourceMismatch
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
    let again = relocate_selected_members_into_confluence(
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
        relocate_selected_members_into_confluence(
            &first,
            0,
            T_HEAD,
            T_TAIL,
            HEAD,
            &environment,
            budget(),
        )
        .unwrap_err(),
        ConfluenceRelocationError::UnsupportedPair
    );
    // A different scheduling family still admits on the second input.
    let swapped = crate::rewrites::unexecuted::relocate_selected_member_run(
        &first,
        0,
        MID,
        MID,
        TAIL,
        &environment,
        budget(),
    )
    .unwrap();
    assert_eq!(
        block_order(&swapped.transformed().functions[0].blocks[3]),
        vec![RUN_A, RUN_B, HEAD, TAIL, MID]
    );
}

mod independence_tests {
    use super::{
        BLOCK_J, ConfluenceRelocationError, HEAD, NativeTarget, POINTER, R_MOVE_A, R_MOVE_B,
        R_TAIL, R_TTAIL, RUN_A, RUN_B, SelectedInstructionKind, SelectedInstructionPlan, T_TAIL,
        TAIL, ValidatedConfluenceRelocation, baseline_target_register_environment, budget,
        instruction, mutated, settlement, validate_confluence_relocation,
    };

    /// Move the run at `run` inside the source's T block onto
    /// `landing_index` inside the J block — the edit a producer emitting
    /// that relocation would publish — without asking admission whether
    /// the window is legal. The indices are block-body positions in the
    /// fixture's T (index 1) and J (index 3) blocks.
    fn forged(
        source: &ValidatedConfluenceRelocation,
        run: std::ops::RangeInclusive<usize>,
        landing_index: usize,
    ) -> SelectedInstructionPlan {
        let mut proposed = source.transformed().clone();
        let function = &mut proposed.functions[0];
        let members: Vec<_> = function.blocks[1]
            .instructions
            .drain(*run.start()..=*run.end())
            .collect();
        function.blocks[3]
            .instructions
            .splice(landing_index..landing_index, members);
        proposed
    }

    /// A producer that admitted an impure run anyway would publish the
    /// run sunk into the join with a member whose memory effect newly
    /// runs on the other inflows' arrivals — here `RUN_B` mutated to a
    /// row-less `Load8`. The validator's own purity audit refuses with
    /// `UnsupportedInstruction`, not a replay mismatch, because it
    /// reconstructs the members' effects instead of trusting the
    /// producer's admission record.
    #[test]
    fn forged_relocation_with_an_impure_member_rejects() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let source = mutated(target, |function, environment| {
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
            validate_confluence_relocation(
                &source,
                0,
                RUN_A,
                RUN_B,
                HEAD,
                &environment,
                budget(),
                forged(&source, 1..=2, 0),
            )
            .unwrap_err(),
            ConfluenceRelocationError::UnsupportedInstruction
        );
    }

    /// A producer that admitted a hazard-coupled window anyway would
    /// publish the run sunk past a crossed position reading a register a
    /// member writes — here `T_TAIL` mutated to read `R_MOVE_A`. The
    /// validator's own legality audit refuses with `UnsupportedPair`
    /// because it reconstructs the window's hazards instead of trusting
    /// the producer's admission record.
    #[test]
    fn forged_relocation_across_a_coupled_hazard_rejects() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let source = mutated(target, |function, environment| {
            let copy = environment
                .constraint(environment.selected_keys().copy_i64)
                .unwrap()
                .clone();
            function.blocks[1].instructions[3] = instruction(
                T_TAIL,
                SelectedInstructionKind::CopyI64,
                &copy,
                &[R_MOVE_A, R_TTAIL],
            );
        });
        assert_eq!(
            validate_confluence_relocation(
                &source,
                0,
                RUN_A,
                RUN_B,
                HEAD,
                &environment,
                budget(),
                forged(&source, 1..=2, 0),
            )
            .unwrap_err(),
            ConfluenceRelocationError::UnsupportedPair
        );
    }

    /// A producer that admitted a live member write anyway would publish
    /// the run sunk while a join position past the landing index still
    /// reads what a member writes — here `TAIL` mutated to read
    /// `R_MOVE_B`. The validator's own dead-path audit refuses with
    /// `UnsupportedPair`: on the other inflows' arrivals the write is
    /// new, so its readers may not observe it before a rewrite.
    #[test]
    fn forged_relocation_with_a_live_member_write_rejects() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let source = mutated(target, |function, environment| {
            let copy = environment
                .constraint(environment.selected_keys().copy_i64)
                .unwrap()
                .clone();
            function.blocks[3].instructions[2] = instruction(
                TAIL,
                SelectedInstructionKind::CopyI64,
                &copy,
                &[R_MOVE_B, R_TAIL],
            );
        });
        assert_eq!(
            validate_confluence_relocation(
                &source,
                0,
                RUN_A,
                RUN_B,
                HEAD,
                &environment,
                budget(),
                forged(&source, 1..=2, 0),
            )
            .unwrap_err(),
            ConfluenceRelocationError::UnsupportedPair
        );
    }

    /// A producer that admitted a settled window anyway would publish the
    /// run sunk across a boundary settlement past the landing index — the
    /// settlement would observe the run inside the join's executed prefix
    /// on the other inflows' arrivals, so the validator's own audit
    /// refuses with `UnsupportedPair`.
    #[test]
    fn forged_relocation_past_a_settlement_rejects() {
        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let source = mutated(target, |function, _| {
            function
                .boundary_settlements
                .push(settlement(BLOCK_J, 1, 41));
        });
        assert_eq!(
            validate_confluence_relocation(
                &source,
                0,
                RUN_A,
                RUN_B,
                HEAD,
                &environment,
                budget(),
                forged(&source, 1..=2, 0),
            )
            .unwrap_err(),
            ConfluenceRelocationError::UnsupportedPair
        );
    }
}
