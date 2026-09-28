//! Optimizer module role: stage group.
use crate::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, FixedViewCopy,
    FixedViewCopyDestination, FixedViewCopyPolicy,
};
use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use optimization_unit::{FuelSettlement, PsiProvenance, ValueDefinitionSite};
use register_model::{
    RegisterClassId, RegisterConstraintFamily, RegisterConstraintKey, RegisterOperandAccess,
    RegisterViewId, TargetRegisterEnvironmentIdentity,
};
use selected_instructions::{
    LiveRangeIdentity, LiveRangePoint, LivenessPosition, SelectedBlock, SelectedBlockId,
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionPlan, SelectedInstructionPlanIdentity, SelectedInstructionProvenance,
    SelectedOperand, SelectedSuccessor, SelectedTerminator, VirtualFixedConstraintSite,
    VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, MachineId, OperationId,
    ScalarType, ValueId,
};
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use super::FixedViewCopyPlan;

mod golden;
mod rejection;
mod round_trip;
mod structural;
mod transport;

// Deliberately mutate only the current header; do not manufacture old schemas.
fn with_stale_version(plan: &FixedViewCopyPlan, version: u32) -> Vec<u8> {
    let mut encoded = plan.encode();
    let current = u32::from_le_bytes(encoded[8..12].try_into().unwrap());
    assert!(version < current);
    encoded[8..12].copy_from_slice(&version.to_le_bytes());
    encoded
}

fn instruction(
    id: u32,
    kind: SelectedInstructionKind,
    operands: Vec<SelectedOperand>,
) -> SelectedInstruction {
    SelectedInstruction {
        id: SelectedInstructionId(id),
        kind,
        constraint: RegisterConstraintKey {
            family: RegisterConstraintFamily::Instruction,
            variant: id,
        },
        operands,
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: Vec::new(),
        provenance: SelectedInstructionProvenance::default(),
    }
}

fn use_operand(
    register: u32,
    class: RegisterClassId,
    view: Option<RegisterViewId>,
) -> SelectedOperand {
    SelectedOperand {
        operand: 0,
        virtual_register: VirtualRegisterId(register),
        access: RegisterOperandAccess::Use,
        class,
        fixed_view: view,
        tied_to: None,
        early_clobber: false,
    }
}

fn return_successor(edge: u64, block: u32, source: u64) -> SelectedSuccessor {
    SelectedSuccessor {
        role: selected_instructions::SelectedSuccessorRole::Semantic,
        structural_case: None,
        structural_bindings: Vec::new(),
        psi_edge: EdgeId::new(edge).unwrap(),
        block: SelectedBlockId(block),
        source_target: BlockId::new(source).unwrap(),
        bindings: Vec::new(),
        fuel: Vec::new(),
    }
}

/// Two-block compare/branch function with two return leaves and two entry
/// parameter registers — the shared-exit shape the V35 envelope exercises.
fn shared_function() -> SelectedFunction {
    let class = RegisterClassId(0);
    let from = RegisterViewId(1);
    let to = RegisterViewId(2);
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    SelectedFunction {
        machine: MachineId::new(1).unwrap(),
        attachment: None,
        provenance: Default::default(),
        structural: None,
        outgoing_arguments: Vec::new(),
        local_storage_slots: Vec::new(),
        calls: Vec::new(),
        normalized_foreign_calls: Vec::new(),
        memory_accesses: Vec::new(),
        boundary_settlements: Vec::new(),
        entry_block: SelectedBlockId(0),
        virtual_registers: vec![
            VirtualRegister {
                id: VirtualRegisterId(0),
                scalar_type: scalar,
                class,
                origin: VirtualRegisterOrigin::EntryParameter {
                    source_value: ValueId::new(1).unwrap(),
                    parameter_index: 0,
                },
                definition_site: Some(ValueDefinitionSite::FunctionParameter(0)),
                entry_fixed_view: None,
            },
            VirtualRegister {
                id: VirtualRegisterId(1),
                scalar_type: scalar,
                class,
                origin: VirtualRegisterOrigin::EntryParameter {
                    source_value: ValueId::new(2).unwrap(),
                    parameter_index: 1,
                },
                definition_site: Some(ValueDefinitionSite::FunctionParameter(1)),
                entry_fixed_view: Some(from),
            },
        ],
        blocks: vec![
            SelectedBlock {
                id: SelectedBlockId(0),
                origin: selected_instructions::SelectedBlockOrigin::Source(
                    BlockId::new(1).unwrap(),
                ),
                instructions: vec![instruction(
                    0,
                    SelectedInstructionKind::CompareI64Zero,
                    vec![use_operand(0, class, None)],
                )],
                terminator: SelectedTerminator::ConditionalBranch {
                    instruction: instruction(
                        1,
                        SelectedInstructionKind::ConditionalBranchNonZero,
                        Vec::new(),
                    ),
                    when_nonzero: return_successor(1, 1, 2),
                    when_zero: return_successor(2, 2, 3),
                },
            },
            SelectedBlock {
                id: SelectedBlockId(1),
                origin: selected_instructions::SelectedBlockOrigin::Source(
                    BlockId::new(2).unwrap(),
                ),
                instructions: Vec::new(),
                terminator: SelectedTerminator::Return {
                    instruction: instruction(
                        2,
                        SelectedInstructionKind::ReturnScalar,
                        vec![use_operand(1, class, Some(to))],
                    ),
                    psi_return_edge: EdgeId::new(3).unwrap(),
                },
            },
            SelectedBlock {
                id: SelectedBlockId(2),
                origin: selected_instructions::SelectedBlockOrigin::Source(
                    BlockId::new(3).unwrap(),
                ),
                instructions: Vec::new(),
                terminator: SelectedTerminator::Return {
                    instruction: instruction(
                        3,
                        SelectedInstructionKind::ReturnScalar,
                        vec![use_operand(1, class, Some(to))],
                    ),
                    psi_return_edge: EdgeId::new(4).unwrap(),
                },
            },
        ],
    }
}

/// Jump-terminated function with one parameter value binding — the successor
/// transfer vocabulary the shared-exit envelope must carry.
pub(super) fn successor_parameter_function() -> SelectedFunction {
    let mut function = shared_function();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let jump = instruction(0, SelectedInstructionKind::Jump, Vec::new());
    let return_instruction = instruction(
        1,
        SelectedInstructionKind::ReturnScalar,
        vec![SelectedOperand {
            operand: 0,
            virtual_register: VirtualRegisterId(2),
            access: RegisterOperandAccess::Use,
            class: RegisterClassId(0),
            fixed_view: None,
            tied_to: None,
            early_clobber: false,
        }],
    );
    function.blocks[0].instructions = vec![jump.clone()];
    function.blocks[0].terminator = SelectedTerminator::Jump {
        instruction: jump,
        successor: SelectedSuccessor {
            role: selected_instructions::SelectedSuccessorRole::Semantic,
            structural_case: None,
            structural_bindings: Vec::new(),
            psi_edge: EdgeId::new(1).unwrap(),
            block: SelectedBlockId(1),
            source_target: BlockId::new(2).unwrap(),
            bindings: vec![selected_instructions::SelectedValueBinding {
                semantic: abstract_operations::ValueBinding {
                    parameter: ValueId::new(3).unwrap(),
                    argument: ValueId::new(1).unwrap(),
                    scalar_type,
                },
                transport: selected_instructions::SelectedValueTransport::Registers {
                    argument: VirtualRegisterId(0),
                    parameter: VirtualRegisterId(2),
                },
            }],
            fuel: Vec::new(),
        },
    };
    function.blocks.truncate(2);
    function.blocks[1].terminator = SelectedTerminator::Return {
        instruction: return_instruction,
        psi_return_edge: EdgeId::new(2).unwrap(),
    };
    function.virtual_registers.push(VirtualRegister {
        id: VirtualRegisterId(2),
        scalar_type,
        class: RegisterClassId(0),
        entry_fixed_view: None,
        origin: VirtualRegisterOrigin::BlockParameter {
            source_value: ValueId::new(3).unwrap(),
            block: SelectedBlockId(1),
            parameter_index: 0,
        },
        definition_site: Some(ValueDefinitionSite::BlockParameter {
            block: BlockId::new(2).unwrap(),
            position: 0,
        }),
    });
    function
}

/// One fixed-view copy materializing both return-leaf uses of register 1 —
/// the synthetic companion to `shared_function`'s entry transitions.
fn shared_copy() -> FixedViewCopy {
    let site = |instruction| VirtualFixedConstraintSite::Operand {
        position: LivenessPosition(instruction),
        point: LiveRangePoint(instruction),
        instruction: SelectedInstructionId(instruction),
        operand: 0,
        access: RegisterOperandAccess::Use,
    };
    FixedViewCopy {
        function: 0,
        machine: MachineId::new(1).unwrap(),
        source_virtual_register: VirtualRegisterId(1),
        source_value: ValueId::new(2).unwrap(),
        source_definition_site: ValueDefinitionSite::FunctionParameter(1),
        from_view: RegisterViewId(1),
        to_view: RegisterViewId(2),
        insertion_block: SelectedBlockId(0),
        before_instruction: SelectedInstructionId(1),
        destinations: vec![
            FixedViewCopyDestination {
                site: site(2),
                block: SelectedBlockId(1),
                view: RegisterViewId(2),
            },
            FixedViewCopyDestination {
                site: site(3),
                block: SelectedBlockId(2),
                view: RegisterViewId(2),
            },
        ],
        copy_instruction: SelectedInstructionId(4),
        result_virtual_register: VirtualRegisterId(10),
        copy_constraint: RegisterConstraintKey {
            family: RegisterConstraintFamily::Instruction,
            variant: 9,
        },
    }
}

/// Minimal single-block function with one ordinary operand — the transfer
/// fixture's building block for successor and structural payload shapes.
pub(super) fn function_with_operand(access: RegisterOperandAccess) -> SelectedFunction {
    let key = RegisterConstraintKey {
        family: RegisterConstraintFamily::Instruction,
        variant: 99,
    };
    let instruction = SelectedInstruction {
        id: SelectedInstructionId(0),
        kind: SelectedInstructionKind::CompareI64Zero,
        constraint: key,
        operands: vec![SelectedOperand {
            operand: 0,
            virtual_register: VirtualRegisterId(0),
            access,
            class: RegisterClassId(0),
            fixed_view: None,
            tied_to: None,
            early_clobber: false,
        }],
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: Vec::new(),
        provenance: SelectedInstructionProvenance::default(),
    };
    SelectedFunction {
        machine: MachineId::new(1).unwrap(),
        attachment: None,
        provenance: Default::default(),
        structural: None,
        outgoing_arguments: Vec::new(),
        local_storage_slots: Vec::new(),
        calls: Vec::new(),
        normalized_foreign_calls: Vec::new(),
        memory_accesses: Vec::new(),
        boundary_settlements: Vec::new(),
        entry_block: SelectedBlockId(0),
        virtual_registers: Vec::new(),
        blocks: vec![SelectedBlock {
            id: SelectedBlockId(0),
            origin: selected_instructions::SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
            instructions: vec![instruction],
            terminator: SelectedTerminator::Return {
                instruction: SelectedInstruction {
                    id: SelectedInstructionId(1),
                    kind: SelectedInstructionKind::ReturnScalar,
                    constraint: key,
                    operands: Vec::new(),
                    implicit_uses: Vec::new(),
                    implicit_defs: Vec::new(),
                    clobbers: Vec::new(),
                    provenance: SelectedInstructionProvenance::default(),
                },
                psi_return_edge: EdgeId::new(1).unwrap(),
            },
        }],
    }
}

pub(super) fn plan(policy: FixedViewCopyPolicy) -> FixedViewCopyPlan {
    let copy = shared_copy();
    let mut function = shared_function();
    let operation = OperationId::new(1).unwrap();
    function.provenance.operations.push(operation);
    function.blocks[0].instructions[0]
        .provenance
        .operations
        .push(operation);
    function.blocks[0].instructions[0]
        .provenance
        .fuel
        .push(FuelSettlement {
            site: PsiProvenance::Operation(operation),
            units: 7,
        });
    FixedViewCopyPlan {
        source_selected: SelectedInstructionPlanIdentity::from_bytes([1; 32]),
        source_ranges: LiveRangeIdentity::from_bytes([2; 32]),
        source_legality: AllocationLegalityIdentity::from_bytes([3; 32]),
        register_environment: TargetRegisterEnvironmentIdentity::from_bytes([4; 32]),
        allocator_availability: AllocatorAvailabilityIdentity::from_bytes([5; 32]),
        source_evidence: crate::FixedViewCopySourceEvidence::LegacyLegalityTransitionsV1,
        policy,
        budget: OptimizationWorkBudget::new(3, 3, 3, 3, 1).unwrap(),
        usage: OptimizationWorkUsage {
            rule_evaluations: 1,
            candidates: 2,
            validation_steps: 2,
            commits: 1,
            iterations: 1,
        },
        copies: vec![copy],
        transformed: SelectedInstructionPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([6; 32]),
            },
            fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
            target: target::NativeTarget::linux_x64(),
            entry: function.machine,
            functions: vec![function].into(),
        }
        .into(),
    }
}
