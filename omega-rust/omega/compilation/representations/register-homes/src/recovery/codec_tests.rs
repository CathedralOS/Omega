/*
Wire envelope, round-trip, rejection precedence, and structural catalog tests for the fixed-view copy recovery codec.

Verifies that `FixedViewCopyPlan` deterministic wire encoding (`OMGFCV\0\0` v98), authentication digests, stale-version rejection precedence, edge transport bindings, call-site owners, local and outgoing storage rosters, returned descriptor origins, and complete placement/lifecycle catalogs survive round-trip encoding and fail closed under tampering or truncation.
*/

use abstract_operations::StructuralTypeCatalog;
use access_plans::{AccessFields, AccessPlan, BoundaryReach, validate_access_plan};
use calling_conventions::{CallPlan, CallingPolicy, EntryControl, MachineRegister, RegisterSet};
use layout_plans::{LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport};
use legalized_operations::{LegalizedScalarCall, LegalizedStructuralContract, NativeCallOrigin};
use optimization_core::{
    AcceptedObligationFactIdentity, OptimizationWorkBudget, OptimizationWorkUsage,
};
use optimization_unit::{EffectLink, PsiProvenance, ValueDefinitionSite, WorkSettlement};
use register_model::{
    RegisterClassId, RegisterConstraintFamily, RegisterConstraintKey, RegisterOperandAccess,
    RegisterViewId, TargetRegisterEnvironmentIdentity,
};
use selected_instructions::{
    LiveRangeIdentity, LiveRangePoint, LivenessPosition, OutgoingArgumentSlotId, SelectedBlock,
    SelectedBlockId, SelectedBlockOrigin, SelectedCallContract, SelectedFunction,
    SelectedInstruction, SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan,
    SelectedInstructionPlanIdentity, SelectedInstructionProvenance, SelectedMemoryAccess,
    SelectedMemoryAccessRole, SelectedOperand, SelectedOutgoingArgumentSlot, SelectedSuccessor,
    SelectedTerminator, SelectedValueBinding, SelectedValueTransport, VirtualFixedConstraintSite,
    VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, EdgeId, IntegerSign, IntegerType, MachineId, ObligationId,
    OperationId, PlaceId, ScalarType, StructuralDomainId, StructuralFieldId, StructuralTypeId,
    ValueId, WorkScheduleIdentity,
};
use terminal_psi::{
    CrashCause, CrashRouteBucket, CrashRouteGuard, NominalDeclarationIdentity, SemanticFingerprint,
    TerminalAccessLifecycleExecution, TerminalAccessLifecycleOperation,
    TerminalAccessLifecyclePlan, TerminalAccessProfileRequirement, TerminalAccessStorage,
    TerminalAccessViewPlan, TerminalIdentityIndex, TerminalPsiIdentity, VocabularyMarker,
};

use super::fixed_view_copy_codec::{
    Cursor, VERSION, decode_copy, decode_fixed_site, decode_source_evidence,
};
use super::selected_instructions::decode_kind;
use crate::codec_bytes::patch_encoded;
use crate::digest_encoder::content_identity;
use crate::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity,
    FixedPrecoloredSplitRequirementPlanIdentity, FixedViewCopy, FixedViewCopyDecodeError,
    FixedViewCopyDestination, FixedViewCopyPlan, FixedViewCopyPolicy,
};

fn with_stale_version(plan: &FixedViewCopyPlan, version: u32) -> Vec<u8> {
    let mut encoded = plan.encode().unwrap();
    let current = u32::from_le_bytes(encoded[8..12].try_into().unwrap());
    assert!(version < current);
    patch_encoded(&mut encoded, 8, version.to_le_bytes());
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
        psi_edge: EdgeId::new(edge),
        block: SelectedBlockId(block),
        source_target: BlockId::new(source).unwrap(),
        bindings: Vec::new(),
        work: Vec::new(),
    }
}

fn shared_function() -> SelectedFunction {
    let class = RegisterClassId(0);
    let from = RegisterViewId(1);
    let to = RegisterViewId(2);
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    SelectedFunction {
        machine: MachineId::new(1).unwrap(),
        attachment: None,
        nominal_cleanup_subject: None,
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
        blocks: (vec![
            SelectedBlock {
                id: SelectedBlockId(0),
                origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
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
                origin: SelectedBlockOrigin::Source(BlockId::new(2).unwrap()),
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
                origin: SelectedBlockOrigin::Source(BlockId::new(3).unwrap()),
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
        ])
        .into(),
    }
}

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
            psi_edge: EdgeId::new(1),
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
            work: Vec::new(),
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
        nominal_cleanup_subject: None,
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
        blocks: (vec![SelectedBlock {
            id: SelectedBlockId(0),
            origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
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
        }])
        .into(),
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
        .work
        .push(WorkSettlement {
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
            work_schedule: WorkScheduleIdentity::new(1).unwrap(),
            target: target::NativeTarget::linux_x64(),
            entry: function.machine,
            functions: vec![function].into(),
        }
        .into(),
    }
}

pub(super) fn transformed_identity_offset(encoded: &[u8]) -> usize {
    let mut cursor = Cursor::new(encoded);
    cursor.take(44 + (5 * 32) + 1 + 40 + 40).unwrap();
    let copy_count = cursor.length().unwrap();
    for _ in 0..copy_count {
        decode_copy(&mut cursor).unwrap();
    }
    cursor.offset
}

pub(super) fn selected_payload_offset(encoded: &[u8]) -> usize {
    let mut cursor = Cursor::new(encoded);
    cursor
        .take(transformed_identity_offset(encoded) + 32)
        .unwrap();
    decode_source_evidence(&mut cursor).unwrap();
    cursor.offset
}

#[test]
fn current_envelope_has_exact_header_and_deterministic_bytes() {
    let plan = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    let encoded = plan.encode().unwrap();
    assert_eq!(&encoded[..8], b"OMGFCV\0\0");
    assert_eq!(&encoded[8..12], &98_u32.to_le_bytes());
    assert_eq!(encoded, plan.encode().unwrap());
    assert_eq!(FixedViewCopyPlan::decode(&encoded).unwrap(), plan);
}

#[test]
fn envelope_rejects_non_current_formats() {
    let plan = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    for version in [0, VERSION - 1, VERSION + 1, u32::MAX] {
        let mut encoded = plan.encode().unwrap();
        patch_encoded(&mut encoded, 8, version.to_le_bytes());
        assert_eq!(
            FixedViewCopyPlan::decode(&encoded),
            Err(FixedViewCopyDecodeError::UnsupportedVersion(version))
        );
    }
}

#[test]
fn artifact_rejects_corruption_truncation_trailing_and_closed_tags() {
    let encoded = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1)
        .encode()
        .unwrap();
    let mut identity_tamper = encoded.clone();
    identity_tamper[12] ^= 1;
    assert_eq!(
        FixedViewCopyPlan::decode(&identity_tamper),
        Err(FixedViewCopyDecodeError::IdentityMismatch)
    );
    let mut wrong_magic = encoded.clone();
    wrong_magic[0] ^= 1;
    assert_eq!(
        FixedViewCopyPlan::decode(&wrong_magic),
        Err(FixedViewCopyDecodeError::WrongMagic)
    );
    let current_version = u32::from_le_bytes(encoded[8..12].try_into().unwrap());
    for version in [current_version - 1, current_version.checked_add(1).unwrap()] {
        let mut wrong_version = encoded.clone();
        patch_encoded(&mut wrong_version, 8, version.to_le_bytes());
        assert_eq!(
            FixedViewCopyPlan::decode(&wrong_version),
            Err(FixedViewCopyDecodeError::UnsupportedVersion(version))
        );
    }
    let mut policy_tag = encoded.clone();
    let policy_offset = 8 + 4 + 32 + (5 * 32);
    policy_tag[policy_offset] = 99;
    assert_eq!(
        FixedViewCopyPlan::decode(&policy_tag),
        Err(FixedViewCopyDecodeError::UnknownPolicy(99))
    );
    let mut source_identity_tamper = encoded.clone();
    source_identity_tamper[44] ^= 1;
    assert_eq!(
        FixedViewCopyPlan::decode(&source_identity_tamper),
        Err(FixedViewCopyDecodeError::IdentityMismatch)
    );
    let mut transformed_identity_tamper = encoded.clone();
    transformed_identity_tamper[transformed_identity_offset(&encoded)] ^= 1;
    assert_eq!(
        FixedViewCopyPlan::decode(&transformed_identity_tamper),
        Err(FixedViewCopyDecodeError::TransformedIdentityMismatch)
    );
    let mut evidence_tag = encoded.clone();
    evidence_tag[transformed_identity_offset(&encoded) + 32] = 99;
    assert_eq!(
        FixedViewCopyPlan::decode(&evidence_tag),
        Err(FixedViewCopyDecodeError::UnknownSourceEvidence(99))
    );
    assert_eq!(
        FixedViewCopyPlan::decode(&encoded[..encoded.len() - 1]),
        Err(FixedViewCopyDecodeError::Truncated)
    );
    let mut trailing = encoded.clone();
    trailing.push(0);
    assert_eq!(
        FixedViewCopyPlan::decode(&trailing),
        Err(FixedViewCopyDecodeError::TrailingBytes)
    );
    assert_eq!(
        decode_fixed_site(&mut Cursor::new(&[9])),
        Err(FixedViewCopyDecodeError::UnknownFixedSite(9))
    );
    assert_eq!(
        decode_kind(&mut Cursor::new(&[255])),
        Err(FixedViewCopyDecodeError::UnknownInstructionKind(255))
    );
}

#[test]
fn stale_artifact_version_rejects_before_payload_or_authentication() {
    let encoded = with_stale_version(
        &plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1),
        4,
    );
    let transformed_offset = transformed_identity_offset(&encoded);

    let mut trailing_and_transformed = encoded.clone();
    trailing_and_transformed[transformed_offset] ^= 1;
    trailing_and_transformed.push(0);
    assert_eq!(
        FixedViewCopyPlan::decode(&trailing_and_transformed),
        Err(FixedViewCopyDecodeError::UnsupportedVersion(4))
    );

    let mut transformed_and_outer = encoded;
    transformed_and_outer[transformed_offset] ^= 1;
    transformed_and_outer[12] ^= 1;
    assert_eq!(
        FixedViewCopyPlan::decode(&transformed_and_outer),
        Err(FixedViewCopyDecodeError::UnsupportedVersion(4))
    );
}

#[test]
fn artifact_current_rejection_precedence_is_trailing_payload_semantic_then_outer() {
    let encoded = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1)
        .encode()
        .unwrap();
    let transformed_offset = transformed_identity_offset(&encoded);
    let payload_digest_offset = selected_payload_offset(&encoded);

    let mut trailing_payload_semantic_outer = encoded.clone();
    trailing_payload_semantic_outer[payload_digest_offset] ^= 1;
    trailing_payload_semantic_outer[transformed_offset] ^= 1;
    trailing_payload_semantic_outer[12] ^= 1;
    trailing_payload_semantic_outer.push(0);
    assert_eq!(
        FixedViewCopyPlan::decode(&trailing_payload_semantic_outer),
        Err(FixedViewCopyDecodeError::TrailingBytes)
    );

    let mut payload_semantic_outer = encoded.clone();
    payload_semantic_outer[payload_digest_offset] ^= 1;
    payload_semantic_outer[transformed_offset] ^= 1;
    payload_semantic_outer[12] ^= 1;
    assert_eq!(
        FixedViewCopyPlan::decode(&payload_semantic_outer),
        Err(FixedViewCopyDecodeError::TransformedPayloadMismatch)
    );

    let mut semantic_outer = encoded;
    semantic_outer[transformed_offset] ^= 1;
    semantic_outer[12] ^= 1;
    assert_eq!(
        FixedViewCopyPlan::decode(&semantic_outer),
        Err(FixedViewCopyDecodeError::TransformedIdentityMismatch)
    );
}

#[test]
fn every_previous_wire_generation_rejects_before_payload_decoding() {
    let encoded = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1)
        .encode()
        .unwrap();
    for version in 0..32_u32 {
        let mut stale = encoded.clone();
        patch_encoded(&mut stale, 8, version.to_le_bytes());
        assert_eq!(
            FixedViewCopyPlan::decode(&stale),
            Err(FixedViewCopyDecodeError::UnsupportedVersion(version))
        );
        stale.truncate(12);
        assert_eq!(
            FixedViewCopyPlan::decode(&stale),
            Err(FixedViewCopyDecodeError::UnsupportedVersion(version))
        );
    }
}

#[test]
fn cleanup_call_owner_wire_requires_the_new_envelope() {
    let original = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    assert_eq!(
        FixedViewCopyPlan::decode(&with_stale_version(&original, 40)),
        Err(FixedViewCopyDecodeError::UnsupportedVersion(40))
    );
}

#[test]
fn process_exit_codec_preserves_terminal_nominal_edge_and_source_without_scratch() {
    let mut original = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    let function = &mut std::sync::Arc::make_mut(&mut original.transformed).functions[0];
    let SelectedTerminator::ConditionalBranch {
        mut instruction, ..
    } = function.blocks[0].terminator.clone()
    else {
        panic!("conditional fixture");
    };
    instruction.kind = SelectedInstructionKind::HostedExitProcessI32;
    let block = &mut function.blocks[0];
    block.terminator = SelectedTerminator::HostedExitProcess {
        instruction,
        completion_edge: semantic_vocabulary::EdgeId::new(311).unwrap(),
    };
    function
        .boundary_settlements
        .push(selected_instructions::SelectedBoundarySettlement {
            block: block.id,
            instruction_index: block.instructions.len() as u32,
            settlement:
                selected_instructions::SelectedBoundarySettlementPayload::HostedExitProcessI32 {
                    edge: semantic_vocabulary::EdgeId::new(311).unwrap(),
                    execution: target_operations::BoundaryExecutionBinding::CompilerBuiltin(
                        target_operations::CompilerBuiltinExecution::HostedExitProcessI32,
                    ),
                    boundary: semantic_vocabulary::BoundaryMachineId::new(317).unwrap(),
                    source: semantic_vocabulary::ValueId::new(331).unwrap(),
                },
        });
    assert_eq!(
        FixedViewCopyPlan::decode(&original.encode().unwrap()).unwrap(),
        original
    );
}

#[test]
fn successor_transfer_vocabulary_requires_the_current_envelope() {
    let mut transferred = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    std::sync::Arc::make_mut(&mut transferred.transformed).functions[0] =
        successor_parameter_function();
    assert_eq!(
        FixedViewCopyPlan::decode(&transferred.encode().unwrap()).unwrap(),
        transferred
    );
    for encoded in [
        with_stale_version(&transferred, 4),
        with_stale_version(&transferred, 5),
        with_stale_version(&transferred, 6),
        with_stale_version(&transferred, 7),
        with_stale_version(&transferred, 8),
        with_stale_version(&transferred, 9),
        with_stale_version(&transferred, 10),
        with_stale_version(&transferred, 11),
    ] {
        assert_eq!(
            FixedViewCopyPlan::decode(&encoded),
            Err(FixedViewCopyDecodeError::UnsupportedVersion(
                u32::from_le_bytes(encoded[8..12].try_into().unwrap())
            ))
        );
    }

    let mut jumped = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    let SelectedTerminator::ConditionalBranch {
        mut instruction,
        when_nonzero,
        ..
    } = jumped.transformed.functions[0].blocks[0].terminator.clone()
    else {
        unreachable!()
    };
    instruction.kind = SelectedInstructionKind::Jump;
    std::sync::Arc::make_mut(&mut jumped.transformed).functions[0].blocks[0].terminator =
        SelectedTerminator::Jump {
            instruction,
            successor: when_nonzero,
        };
    assert_eq!(
        FixedViewCopyPlan::decode(&jumped.encode().unwrap()).unwrap(),
        jumped
    );
    for encoded in [
        with_stale_version(&jumped, 4),
        with_stale_version(&jumped, 5),
        with_stale_version(&jumped, 6),
        with_stale_version(&jumped, 7),
        with_stale_version(&jumped, 8),
        with_stale_version(&jumped, 9),
        with_stale_version(&jumped, 10),
        with_stale_version(&jumped, 11),
    ] {
        assert_eq!(
            FixedViewCopyPlan::decode(&encoded),
            Err(FixedViewCopyDecodeError::UnsupportedVersion(
                u32::from_le_bytes(encoded[8..12].try_into().unwrap())
            ))
        );
    }

    let mut instruction_only = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    std::sync::Arc::make_mut(&mut instruction_only.transformed).functions[0].blocks[0]
        .instructions[0]
        .kind = SelectedInstructionKind::Jump;
    assert_eq!(
        FixedViewCopyPlan::decode(&with_stale_version(&instruction_only, 11)),
        Err(FixedViewCopyDecodeError::UnsupportedVersion(11))
    );
}

#[test]
fn artifact_round_trips_every_policy_and_full_transformed_custody() {
    for policy in [
        FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1,
        FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1,
        FixedViewCopyPolicy::ImmediateBeforeFixedUseV1,
        FixedViewCopyPolicy::SharedSourceExitBeforeFixedUseV1,
    ] {
        let plan = plan(policy);
        let decoded = FixedViewCopyPlan::decode(&plan.encode().unwrap()).unwrap();
        assert_eq!(decoded, plan);
        assert_eq!(decoded.copies[0].destinations.len(), 2);
        assert_eq!(
            decoded.transformed.functions[0].blocks[0].instructions[0]
                .provenance
                .work[0]
                .units,
            7
        );
    }
}

#[test]
fn artifact_current_retains_segment_home_evidence_and_rejects_older_authority() {
    let mut plan = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    plan.source_evidence = crate::FixedViewCopySourceEvidence::FixedPrecoloredSegmentHomesV1 {
        fixed_intervals: crate::FixedPrecoloredIntervalPlanIdentity::from_bytes([21; 32]),
        split_requirements: FixedPrecoloredSplitRequirementPlanIdentity::from_bytes([22; 32]),
        segment_homes: crate::FixedPrecoloredSegmentHomePlanIdentity::from_bytes([23; 32]),
    };
    let encoded = plan.encode().unwrap();
    assert_eq!(FixedViewCopyPlan::decode(&encoded).unwrap(), plan);
    for encoded in [
        with_stale_version(&plan, 10),
        with_stale_version(&plan, 11),
        with_stale_version(&plan, 23),
        with_stale_version(&plan, 25),
        with_stale_version(&plan, 39),
        with_stale_version(&plan, 40),
        with_stale_version(&plan, 41),
        with_stale_version(&plan, 42),
        with_stale_version(&plan, 43),
        with_stale_version(&plan, 44),
        with_stale_version(&plan, 45),
        with_stale_version(&plan, 46),
        with_stale_version(&plan, 93),
    ] {
        let version = u32::from_le_bytes(encoded[8..12].try_into().unwrap());
        assert_eq!(
            FixedViewCopyPlan::decode(&encoded),
            Err(FixedViewCopyDecodeError::UnsupportedVersion(version))
        );
    }
}

#[test]
fn artifact_current_retains_absent_structural_contracts() {
    let plan = plan(FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1);
    let decoded = FixedViewCopyPlan::decode(&plan.encode().unwrap()).unwrap();
    assert_eq!(decoded, plan);
    assert!(
        decoded
            .transformed
            .functions
            .iter()
            .all(|function| function.structural.is_none())
    );
}

#[test]
fn artifact_rejects_pre_compare_identity_version() {
    let plan = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    assert_eq!(
        FixedViewCopyPlan::decode(&with_stale_version(&plan, 6)),
        Err(FixedViewCopyDecodeError::UnsupportedVersion(6))
    );
}

#[test]
fn artifact_rejects_pre_predicate_identity_version() {
    let plan = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    assert_eq!(
        FixedViewCopyPlan::decode(&with_stale_version(&plan, 7)),
        Err(FixedViewCopyDecodeError::UnsupportedVersion(7))
    );
}

#[test]
fn artifact_current_round_trips_u64_less_than_terminator_vocabulary() {
    let mut plan = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    let terminator = plan.transformed.functions[0].blocks[0].terminator.clone();
    let SelectedTerminator::ConditionalBranch {
        mut instruction,
        when_nonzero,
        when_zero,
    } = terminator
    else {
        panic!("shared fixture must begin with a conditional branch")
    };
    instruction.kind = SelectedInstructionKind::ConditionalBranchU64LessThan;
    std::sync::Arc::make_mut(&mut plan.transformed).functions[0].blocks[0].terminator =
        SelectedTerminator::ConditionalBranchU64LessThan {
            instruction,
            when_less: when_nonzero,
            when_not_less: when_zero,
        };

    assert_eq!(
        FixedViewCopyPlan::decode(&plan.encode().unwrap()).unwrap(),
        plan
    );
}

#[test]
fn artifact_current_round_trips_scalar_call_callee_vocabulary() {
    let mut plan = plan(FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1);
    let callee = MachineId::new(901).unwrap();
    std::sync::Arc::make_mut(&mut plan.transformed).functions[0].blocks[0].instructions[0].kind =
        SelectedInstructionKind::CallScalar {
            target: selected_instructions::SelectedCallTarget::Machine(callee),
        };

    assert_eq!(
        FixedViewCopyPlan::decode(&plan.encode().unwrap()).unwrap(),
        plan
    );
}

#[test]
fn artifact_current_round_trips_signed_less_than_terminator_vocabulary() {
    let mut plan = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    let terminator = plan.transformed.functions[0].blocks[0].terminator.clone();
    let SelectedTerminator::ConditionalBranch {
        mut instruction,
        when_nonzero,
        when_zero,
    } = terminator
    else {
        panic!("shared fixture must begin with a conditional branch")
    };
    instruction.kind = SelectedInstructionKind::ConditionalBranchI64LessThan;
    std::sync::Arc::make_mut(&mut plan.transformed).functions[0].blocks[0].terminator =
        SelectedTerminator::ConditionalBranchI64LessThan {
            instruction,
            when_less: when_nonzero,
            when_not_less: when_zero,
        };

    assert_eq!(
        FixedViewCopyPlan::decode(&plan.encode().unwrap()).unwrap(),
        plan
    );

    assert_eq!(
        FixedViewCopyPlan::decode(&with_stale_version(&plan, 9)),
        Err(FixedViewCopyDecodeError::UnsupportedVersion(9))
    );
}

fn bindings_mut(plan: &mut FixedViewCopyPlan) -> &mut Vec<SelectedValueBinding> {
    let SelectedTerminator::ConditionalBranch { when_nonzero, .. } =
        &mut std::sync::Arc::make_mut(&mut plan.transformed).functions[0].blocks[0].terminator
    else {
        panic!("conditional fixture");
    };
    &mut when_nonzero.bindings
}

#[test]
fn current_artifact_and_identity_bind_transport_role_and_both_registers() {
    let mut original = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    bindings_mut(&mut original).push(SelectedValueBinding {
        semantic: abstract_operations::ValueBinding {
            parameter: ValueId::new(40).unwrap(),
            argument: ValueId::new(41).unwrap(),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        },
        transport: SelectedValueTransport::Registers {
            argument: VirtualRegisterId(17),
            parameter: VirtualRegisterId(29),
        },
    });
    let identity = selected_instructions::selected_instruction_plan_identity(&original.transformed);
    assert_eq!(
        FixedViewCopyPlan::decode(&original.encode().unwrap()).unwrap(),
        original
    );
    for transport in [
        SelectedValueTransport::Unused,
        SelectedValueTransport::Registers {
            argument: VirtualRegisterId(18),
            parameter: VirtualRegisterId(29),
        },
        SelectedValueTransport::Registers {
            argument: VirtualRegisterId(17),
            parameter: VirtualRegisterId(30),
        },
    ] {
        let mut changed = original.clone();
        bindings_mut(&mut changed)[0].transport = transport;
        assert_ne!(
            selected_instructions::selected_instruction_plan_identity(&changed.transformed),
            identity
        );
        assert_ne!(changed.encode().unwrap(), original.encode().unwrap());
        assert_eq!(
            FixedViewCopyPlan::decode(&changed.encode().unwrap()).unwrap(),
            changed
        );
    }
}

fn call_plan(clobbers: &[MachineRegister], shadow_bytes: u16) -> CallPlan {
    CallPlan {
        policy: CallingPolicy::MicrosoftX64,
        parameters: Vec::new(),
        result: None,
        callback_materializations: Vec::new(),
        ordinary_clobbers: RegisterSet::new(clobbers.iter().copied()),
        stack_alignment: 16,
        shadow_bytes,
        entry_control: EntryControl::CallReturn,
    }
}

fn structural_function() -> SelectedFunction {
    let mut function = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1)
        .transformed
        .functions[0]
        .clone();
    function.machine = MachineId::new(41).unwrap();
    let mut pointer = function.virtual_registers[0].clone();
    pointer.id = VirtualRegisterId(function.virtual_registers.len() as u32);
    pointer.origin = VirtualRegisterOrigin::StructuralParameter {
        place: PlaceId::new(1).unwrap(),
        parameter_index: 1,
    };
    pointer.definition_site = None;
    function.virtual_registers.push(pointer.clone());
    pointer.id = VirtualRegisterId(function.virtual_registers.len() as u32);
    pointer.origin = VirtualRegisterOrigin::AbiTransport {
        instruction: SelectedInstructionId(0),
        place: PlaceId::new(1).unwrap(),
        byte_offset: 8,
    };
    function.virtual_registers.push(pointer);
    function.structural = Some(LegalizedStructuralContract {
        result: None,
        structural_types: Vec::new().into(),
        parameters: Vec::new(),
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
    });
    function.calls = vec![SelectedCallContract {
        instruction: SelectedInstructionId(0),
        owner: target_operations::CallSiteOwner::Operation(OperationId::new(41).unwrap()),
        call: LegalizedScalarCall {
            descriptor_origins: Vec::new(),
            access_bindings: Vec::new(),
            structural_result: None,
            source: NativeCallOrigin::Authored,
            target: target_operations::NativeCallTarget::Machine(MachineId::new(42).unwrap()),
            call_plan: call_plan(&[MachineRegister::X86Rax, MachineRegister::X86Rcx], 0x4567),
            arguments: Vec::new(),
            result_placement: None,
            claim_transfers: Vec::new(),
            requirement_obligations: vec![ObligationId::new(43).unwrap()],
            crash_continuations: vec![CrashRouteBucket {
                cause: CrashCause::Trap,
                alternatives: vec![CrashRouteGuard::Truth],
            }],
        },
        effect: EffectLink {
            input: 10,
            output: 11,
        },
        ownership: Vec::new(),
    }];
    let slot = OutgoingArgumentSlotId {
        role: selected_instructions::OutgoingArgumentSlotRole::Argument,
        invocation_site: OperationId::new(41).unwrap().into(),
        argument_index: 0,
    };
    function.outgoing_arguments = vec![SelectedOutgoingArgumentSlot {
        id: slot,
        byte_size: 16,
        alignment: 8,
        abi_stack_byte_offset: 32,
    }];
    function.memory_accesses = vec![SelectedMemoryAccess {
        instruction: SelectedInstructionId(0),
        origin: selected_instructions::SelectedMemoryAccessOrigin::Operation(
            slot.invocation_site
                .operation()
                .expect("operation-backed test slot"),
        ),
        place: PlaceId::new(1).unwrap(),
        byte_offset: 8,
        byte_count: 8,
        role: SelectedMemoryAccessRole::WriteOutgoing { slot },
    }];
    function
}

macro_rules! eq {
    ($left:expr => $right:expr) => {
        assert_eq!($left, $right);
    };
}

#[test]
fn artifact_v15_round_trips_structural_functions_call_plans_and_semantic_call_rows() {
    let mut plan = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    std::sync::Arc::make_mut(&mut plan.transformed)
        .functions
        .push(structural_function());
    eq!(FixedViewCopyPlan::decode(&plan.encode().unwrap()).unwrap() => plan);
}

#[test]
fn dynamic_call_target_round_trips_selector_and_requirement_identity() {
    let mut plan = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    let mut function = structural_function();
    function.calls[0].call.target = target_operations::NativeCallTarget::Dynamic {
        selector: semantic_vocabulary::ValueId::new(47).unwrap(),
        interface: 13,
        slot: 29,
    };
    std::sync::Arc::make_mut(&mut plan.transformed)
        .functions
        .push(function);
    eq!(FixedViewCopyPlan::decode(&plan.encode().unwrap()).unwrap() => plan);
}

#[test]
fn activation_local_roster_and_memory_roles_round_trip() {
    let mut plan = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    let mut function = structural_function();
    let slot = selected_instructions::LocalStorageSlotId::Structural {
        operation: OperationId::new(53).unwrap(),
        place: PlaceId::new(59).unwrap(),
    };
    function
        .local_storage_slots
        .push(selected_instructions::SelectedLocalStorageSlot {
            id: selected_instructions::LocalStorageSlotId::EdgeSnapshot {
                edge: semantic_vocabulary::EdgeId::new(83).unwrap(),
                binding_index: 2,
            },
            byte_size: 13,
            alignment: 1,
        });
    function
        .local_storage_slots
        .push(selected_instructions::SelectedLocalStorageSlot {
            id: slot,
            byte_size: 24,
            alignment: 8,
        });
    for (role, byte_count) in [
        (SelectedMemoryAccessRole::ReadLocal { slot }, 8),
        (SelectedMemoryAccessRole::ReadBacking, 8),
        (SelectedMemoryAccessRole::WriteBacking, 8),
        (SelectedMemoryAccessRole::ReadBackingSpan, 0),
        (SelectedMemoryAccessRole::WriteBackingSpan, 0),
        (SelectedMemoryAccessRole::WriteLocal { slot }, 8),
        (SelectedMemoryAccessRole::AddressLocal { slot }, 8),
        (
            SelectedMemoryAccessRole::WriteByteSequence {
                index: semantic_vocabulary::ValueId::new(61).unwrap(),
                value: semantic_vocabulary::ValueId::new(67).unwrap(),
                length: semantic_vocabulary::ValueId::new(71).unwrap(),
                obligation: ObligationId::new(73).unwrap(),
                accepted_fact: AcceptedObligationFactIdentity::from_bytes([79; 32]),
            },
            1,
        ),
    ] {
        function.memory_accesses.push(SelectedMemoryAccess {
            instruction: SelectedInstructionId(1),
            origin: selected_instructions::SelectedMemoryAccessOrigin::Operation(
                slot.operation().expect("source-backed local slot"),
            ),
            place: slot.structural_place().unwrap(),
            byte_offset: 0,
            byte_count,
            role,
        });
    }
    std::sync::Arc::make_mut(&mut plan.transformed)
        .functions
        .push(function);
    eq!(FixedViewCopyPlan::decode(&plan.encode().unwrap()).unwrap() => plan);
}

#[test]
fn operation_scratch_read_identity_is_not_its_anchor_place() {
    use selected_instructions::{LocalStorageSlotId, selected_instruction_plan_identity};
    let mut source = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    let mut function = structural_function();
    let operation = OperationId::new(53).unwrap();
    function.memory_accesses.clear();
    function.memory_accesses.push(SelectedMemoryAccess {
        instruction: SelectedInstructionId(1),
        origin: selected_instructions::SelectedMemoryAccessOrigin::Operation(operation),
        place: PlaceId::new(59).unwrap(),
        byte_offset: 8,
        byte_count: 8,
        role: SelectedMemoryAccessRole::ReadLocal {
            slot: LocalStorageSlotId::Boundary { operation },
        },
    });
    std::sync::Arc::make_mut(&mut source.transformed)
        .functions
        .push(function);
    let identity = selected_instruction_plan_identity(&source.transformed);
    let encoded = source.encode().unwrap();
    assert_eq!(FixedViewCopyPlan::decode(&encoded).unwrap(), source);
    for role in [
        SelectedMemoryAccessRole::ReadPlace,
        SelectedMemoryAccessRole::ReadBacking,
        SelectedMemoryAccessRole::WriteBacking,
        SelectedMemoryAccessRole::ReadBackingSpan,
        SelectedMemoryAccessRole::WriteBackingSpan,
        SelectedMemoryAccessRole::WriteLocal {
            slot: LocalStorageSlotId::Boundary { operation },
        },
        SelectedMemoryAccessRole::ReadLocal {
            slot: LocalStorageSlotId::Boundary {
                operation: OperationId::new(54).unwrap(),
            },
        },
    ] {
        let mut changed = source.clone();
        std::sync::Arc::make_mut(&mut changed.transformed)
            .functions
            .iter_mut()
            .next_back()
            .unwrap()
            .memory_accesses[0]
            .role = role;
        assert_ne!(
            selected_instruction_plan_identity(&changed.transformed),
            identity
        );
        assert_ne!(changed.encode().unwrap(), encoded);
        eq!(FixedViewCopyPlan::decode(&changed.encode().unwrap()).unwrap() => changed);
    }
}

#[test]
fn outgoing_slot_roles_bind_rosters_memory_and_authenticated_round_trips() {
    use selected_instructions::{OutgoingArgumentSlotRole, selected_instruction_plan_identity};

    let mut source = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    let mut function = structural_function();
    let argument = function.outgoing_arguments[0].id;
    let copy = OutgoingArgumentSlotId {
        role: OutgoingArgumentSlotRole::ValueCopy,
        ..argument
    };
    function
        .outgoing_arguments
        .push(SelectedOutgoingArgumentSlot {
            id: copy,
            byte_size: 24,
            alignment: 16,
            abi_stack_byte_offset: 48,
        });
    for role in [
        SelectedMemoryAccessRole::AddressOutgoing { slot: copy },
        SelectedMemoryAccessRole::WriteOutgoing { slot: copy },
    ] {
        function.memory_accesses.push(SelectedMemoryAccess {
            instruction: SelectedInstructionId(1),
            origin: selected_instructions::SelectedMemoryAccessOrigin::Operation(
                copy.invocation_site
                    .operation()
                    .expect("operation-backed test slot"),
            ),
            place: PlaceId::new(1).unwrap(),
            byte_offset: 0,
            byte_count: 24,
            role,
        });
    }
    std::sync::Arc::make_mut(&mut source.transformed)
        .functions
        .push(function);
    let encoded = source.encode().unwrap();
    assert_eq!(FixedViewCopyPlan::decode(&encoded).unwrap(), source);
    let baseline = selected_instruction_plan_identity(&source.transformed);
    for mutation in 0..3 {
        let mut changed = source.clone();
        let function = std::sync::Arc::make_mut(&mut changed.transformed)
            .functions
            .iter_mut()
            .next_back()
            .unwrap();
        match mutation {
            0 => function.outgoing_arguments[1].id.role = OutgoingArgumentSlotRole::Argument,
            1 => {
                function
                    .memory_accesses
                    .iter_mut()
                    .next_back()
                    .unwrap()
                    .role = SelectedMemoryAccessRole::WriteOutgoing { slot: argument }
            }
            2 => {
                let position = function.memory_accesses.len() - 2;
                function.memory_accesses[position].role =
                    SelectedMemoryAccessRole::AddressOutgoing { slot: argument };
            }
            _ => unreachable!(),
        }
        assert_ne!(
            selected_instruction_plan_identity(&changed.transformed),
            baseline
        );
    }
    let mut slot_bytes = Vec::new();
    copy.encode_identity(&mut slot_bytes);
    let offset = encoded
        .windows(slot_bytes.len())
        .position(|bytes| bytes == slot_bytes)
        .unwrap();
    let mut substitution = encoded.clone();
    substitution[offset + slot_bytes.len() - 1] = 0;
    assert!(matches!(
        FixedViewCopyPlan::decode(&substitution),
        Err(FixedViewCopyDecodeError::TransformedPayloadMismatch
            | FixedViewCopyDecodeError::TransformedIdentityMismatch
            | FixedViewCopyDecodeError::IdentityMismatch)
    ));
    let mut unknown_role = encoded;
    unknown_role[offset + slot_bytes.len() - 1] = 2;
    assert_eq!(
        FixedViewCopyPlan::decode(&unknown_role),
        Err(FixedViewCopyDecodeError::UnknownOption(2))
    );
    assert_eq!(
        FixedViewCopyPlan::decode(&with_stale_version(&source, 34)),
        Err(FixedViewCopyDecodeError::UnsupportedVersion(34))
    );
}

#[test]
fn block_descriptor_slots_and_non_operation_memory_origins_round_trip() {
    use selected_instructions::{
        LocalStorageSlotId, SelectedLocalStorageSlot, SelectedMemoryAccessOrigin,
    };
    let block = BlockId::new(61).unwrap();
    let place = PlaceId::new(67).unwrap();
    let slot = LocalStorageSlotId::StructuralBlockParameter { block, place };
    assert_eq!(slot.operation(), None);
    assert_eq!(slot.structural_place(), Some(place));
    let mut plan = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    let mut function = structural_function();
    function.local_storage_slots.push(SelectedLocalStorageSlot {
        id: slot,
        byte_size: 16,
        alignment: 8,
    });
    for (origin, role) in [
        (
            SelectedMemoryAccessOrigin::Block(block),
            SelectedMemoryAccessRole::AddressLocal { slot },
        ),
        (
            SelectedMemoryAccessOrigin::Edge(EdgeId::new(71).unwrap()),
            SelectedMemoryAccessRole::WriteLocal { slot },
        ),
    ] {
        function.memory_accesses.push(SelectedMemoryAccess {
            instruction: SelectedInstructionId(1),
            origin,
            place,
            byte_offset: 8,
            byte_count: 8,
            role,
        });
    }
    std::sync::Arc::make_mut(&mut plan.transformed)
        .functions
        .push(function);
    eq!(FixedViewCopyPlan::decode(&plan.encode().unwrap()).unwrap() => plan);
    let mut identities = Vec::new();
    for origin in [
        SelectedMemoryAccessOrigin::Block(block),
        SelectedMemoryAccessOrigin::Edge(EdgeId::new(61).unwrap()),
        SelectedMemoryAccessOrigin::Operation(OperationId::new(61).unwrap()),
    ] {
        let mut bytes = Vec::new();
        origin.encode_identity(&mut bytes);
        assert!(!identities.contains(&bytes));
        identities.push(bytes);
    }
}

#[test]
fn stale_header_rejects_current_structural_payload() {
    let mut plan = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    std::sync::Arc::make_mut(&mut plan.transformed)
        .functions
        .push(structural_function());

    let encoded = with_stale_version(&plan, 5);
    assert_eq!(
        FixedViewCopyPlan::decode(&encoded),
        Err(FixedViewCopyDecodeError::UnsupportedVersion(5))
    );
}

#[test]
fn artifact_v15_payload_digest_and_outer_envelope_close_call_plan_blind_spots() {
    let mut plan = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    std::sync::Arc::make_mut(&mut plan.transformed)
        .functions
        .push(structural_function());
    let encoded = plan.encode().unwrap();
    let digest_offset = selected_payload_offset(&encoded);

    let mut digest_tamper = encoded.clone();
    digest_tamper[digest_offset] ^= 1;
    assert_eq!(
        FixedViewCopyPlan::decode(&digest_tamper),
        Err(FixedViewCopyDecodeError::TransformedPayloadMismatch)
    );

    let payload_offset = digest_offset + 32 + 8;
    let marker = 0x4567_u16.to_le_bytes();
    let matches = encoded[payload_offset..]
        .windows(marker.len())
        .enumerate()
        .filter_map(|(offset, bytes)| (bytes == marker).then_some(payload_offset + offset))
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "call-plan marker must be unique");
    let mut payload_and_digest_tamper = encoded;
    payload_and_digest_tamper[matches[0]..matches[0] + 2]
        .copy_from_slice(&0x4568_u16.to_le_bytes());
    let payload_digest = content_identity(&payload_and_digest_tamper[payload_offset..]);
    payload_and_digest_tamper[digest_offset..digest_offset + 32].copy_from_slice(&payload_digest);
    assert_eq!(
        FixedViewCopyPlan::decode(&payload_and_digest_tamper),
        Err(FixedViewCopyDecodeError::TransformedIdentityMismatch)
    );

    let clobber_marker = [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0x10, 0, 0x67, 0x45, 1];
    let matches = plan
        .encode()
        .unwrap()
        .windows(clobber_marker.len())
        .enumerate()
        .filter_map(|(offset, bytes)| (bytes == clobber_marker).then_some(offset))
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "call clobber marker must be unique");
    let mut noncanonical = plan.encode().unwrap();
    noncanonical[matches[0] + 8..matches[0] + 12].copy_from_slice(&[1, 0, 0, 0]);
    let payload_digest = content_identity(&noncanonical[payload_offset..]);
    noncanonical[digest_offset..digest_offset + 32].copy_from_slice(&payload_digest);
    assert_eq!(
        FixedViewCopyPlan::decode(&noncanonical),
        Err(FixedViewCopyDecodeError::TransformedPayloadMismatch)
    );
}

#[test]
fn compiler_spill_origin_and_local_slot_round_trip_without_source_authority() {
    let mut source = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    let function = &mut std::sync::Arc::make_mut(&mut source.transformed).functions[0];
    let slot = selected_instructions::LocalStorageSlotId::Spill {
        register: VirtualRegisterId(0),
    };
    assert_eq!(slot.operation(), None);
    assert_eq!(slot.structural_place(), None);
    function
        .local_storage_slots
        .push(selected_instructions::SelectedLocalStorageSlot {
            id: slot,
            byte_size: 8,
            alignment: 8,
        });
    function.virtual_registers[0].origin = VirtualRegisterOrigin::SpillAddress {
        instruction: SelectedInstructionId(0),
        register: VirtualRegisterId(0),
    };
    function.virtual_registers[0].definition_site = None;
    eq!(FixedViewCopyPlan::decode(&source.encode().unwrap()).unwrap() => source);
}

#[test]
fn owned_entry_slot_round_trip_binds_parameter_place_and_origin() {
    use selected_instructions::{LocalStorageSlotId, SelectedLocalStorageSlot};
    let place = PlaceId::new(67).unwrap();
    let slot = LocalStorageSlotId::StructuralParameter { place };
    let mut source = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    std::sync::Arc::make_mut(&mut source.transformed).functions[0]
        .local_storage_slots
        .push(SelectedLocalStorageSlot {
            id: slot,
            byte_size: 16,
            alignment: 8,
        });
    let encoded = source.encode().unwrap();
    assert_eq!(FixedViewCopyPlan::decode(&encoded).unwrap(), source);
    let mut stale = encoded.clone();
    patch_encoded(&mut stale, 8, 32_u32.to_le_bytes());
    assert_eq!(
        FixedViewCopyPlan::decode(&stale),
        Err(FixedViewCopyDecodeError::UnsupportedVersion(32))
    );
    let identity = selected_instructions::selected_instruction_plan_identity(&source.transformed);
    for changed_slot in [
        LocalStorageSlotId::StructuralParameter {
            place: PlaceId::new(68).unwrap(),
        },
        LocalStorageSlotId::StructuralBlockParameter {
            block: semantic_vocabulary::BlockId::new(1).unwrap(),
            place,
        },
        LocalStorageSlotId::Structural {
            operation: OperationId::new(1).unwrap(),
            place,
        },
    ] {
        let mut changed = source.clone();
        std::sync::Arc::make_mut(&mut changed.transformed).functions[0].local_storage_slots[0].id =
            changed_slot;
        assert_ne!(changed.encode().unwrap(), encoded);
        assert_ne!(
            selected_instructions::selected_instruction_plan_identity(&changed.transformed),
            identity
        );
    }
}

#[test]
fn scalar_abi_address_codec_binds_origin_instruction_and_payload_identity() {
    let mut source = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    let value = semantic_vocabulary::ValueId::new(17).unwrap();
    std::sync::Arc::make_mut(&mut source.transformed).functions[0].virtual_registers[0].origin =
        VirtualRegisterOrigin::ScalarAbiAddress {
            instruction: SelectedInstructionId(3),
            source_value: value,
        };
    let encoded = source.encode().unwrap();
    assert_eq!(FixedViewCopyPlan::decode(&encoded).unwrap(), source);
    let identity = selected_instructions::selected_instruction_plan_identity(&source.transformed);
    for origin in [
        VirtualRegisterOrigin::ScalarAbiAddress {
            instruction: SelectedInstructionId(4),
            source_value: value,
        },
        VirtualRegisterOrigin::ScalarAbiAddress {
            instruction: SelectedInstructionId(3),
            source_value: semantic_vocabulary::ValueId::new(18).unwrap(),
        },
        VirtualRegisterOrigin::InstructionResult {
            instruction: SelectedInstructionId(3),
            source_value: value,
        },
    ] {
        let mut changed = source.clone();
        std::sync::Arc::make_mut(&mut changed.transformed).functions[0].virtual_registers[0]
            .origin = origin;
        assert_ne!(changed.encode().unwrap(), encoded);
        assert_ne!(
            selected_instructions::selected_instruction_plan_identity(&changed.transformed),
            identity
        );
    }
}

#[test]
fn cleanup_call_owner_round_trips_and_identity_keeps_edge_and_action() {
    let mut original = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    std::sync::Arc::make_mut(&mut original.transformed).functions[0] = structural_function();
    let function = &mut std::sync::Arc::make_mut(&mut original.transformed).functions[0];
    function.calls[0].owner = target_operations::CallSiteOwner::CleanupAction {
        edge: semantic_vocabulary::EdgeId::new(41).unwrap(),
        action_ordinal: 7,
    };
    eq!(FixedViewCopyPlan::decode(&original.encode().unwrap()).unwrap() => original);
    let identity = selected_instructions::selected_instruction_plan_identity(&original.transformed);
    for owner in [
        target_operations::CallSiteOwner::CleanupAction {
            edge: semantic_vocabulary::EdgeId::new(43).unwrap(),
            action_ordinal: 7,
        },
        target_operations::CallSiteOwner::CleanupAction {
            edge: semantic_vocabulary::EdgeId::new(41).unwrap(),
            action_ordinal: 8,
        },
        target_operations::CallSiteOwner::Operation(OperationId::new(41).unwrap()),
    ] {
        let mut changed = original.clone();
        std::sync::Arc::make_mut(&mut changed.transformed).functions[0].calls[0].owner = owner;
        assert_ne!(
            selected_instructions::selected_instruction_plan_identity(&changed.transformed),
            identity
        );
        eq!(FixedViewCopyPlan::decode(&changed.encode().unwrap()).unwrap() => changed);
    }
}

#[test]
fn returned_descriptor_origins_preserve_namespace_and_memory_role_in_wire_identity() {
    use legalized_operations::LegalizedDescriptorOrigin as Origin;
    let mut original = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    let mut function = structural_function();
    function.calls[0].call.descriptor_origins = vec![
        Origin::Argument {
            source: terminal_psi::StructuralArgument {
                place: PlaceId::new(61).unwrap(),
                path: vec![terminal_psi::StructuralPathSegment::Referent],
                access: terminal_psi::StructuralAccess::SharedBorrow,
            },
            exact: false,
            indirect: true,
        },
        Origin::Literal {
            machine: MachineId::new(67).unwrap(),
            place: PlaceId::new(61).unwrap(),
        },
        Origin::Empty,
    ];
    let target_operations::CallSiteOwner::Operation(producer) = function.calls[0].owner else {
        unreachable!("fixture call is operation-owned")
    };
    function.memory_accesses.push(SelectedMemoryAccess {
        instruction: SelectedInstructionId(2),
        origin: selected_instructions::SelectedMemoryAccessOrigin::Operation(
            OperationId::new(71).unwrap(),
        ),
        place: PlaceId::new(73).unwrap(),
        byte_offset: 0,
        byte_count: 0,
        role: SelectedMemoryAccessRole::ReadReturnedView { producer },
    });
    std::sync::Arc::make_mut(&mut original.transformed)
        .functions
        .push(function);
    let encoded = original.encode().unwrap();
    assert_eq!(FixedViewCopyPlan::decode(&encoded).unwrap(), original);
    for change in 0..5 {
        let mut changed = original.clone();
        let functions = &mut std::sync::Arc::make_mut(&mut changed.transformed).functions;
        let last = functions.len() - 1;
        let function = &mut functions[last];
        match change {
            0 => {
                if let Origin::Argument { exact, .. } =
                    &mut function.calls[0].call.descriptor_origins[0]
                {
                    *exact = true;
                }
            }
            1 => {
                if let Origin::Argument { indirect, .. } =
                    &mut function.calls[0].call.descriptor_origins[0]
                {
                    *indirect = false;
                }
            }
            2 => {
                if let Origin::Literal { machine, .. } =
                    &mut function.calls[0].call.descriptor_origins[1]
                {
                    *machine = MachineId::new(79).unwrap();
                }
            }
            3 => {
                eq!(function.calls[0].call.descriptor_origins.pop() => Some(Origin::Empty));
            }
            _ => {
                function
                    .memory_accesses
                    .iter_mut()
                    .next_back()
                    .unwrap()
                    .role = SelectedMemoryAccessRole::ReadReturnedView {
                    producer: OperationId::new(83).unwrap(),
                }
            }
        }
        assert_ne!(changed.encode().unwrap(), encoded);
        eq!(FixedViewCopyPlan::decode(&changed.encode().unwrap()).unwrap() => changed);
    }
}

#[test]
fn scalar_result_buffer_round_trips_separately_from_boundary_controls() {
    use selected_instructions::{LocalStorageSlotId, SelectedLocalStorageSlot};
    let operation = OperationId::new(317).unwrap();
    let mut source = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    let mut function = structural_function();
    for id in [
        LocalStorageSlotId::Boundary { operation },
        LocalStorageSlotId::BoundaryResult { operation },
    ] {
        function.local_storage_slots.push(SelectedLocalStorageSlot {
            id,
            byte_size: 8,
            alignment: 8,
        });
    }
    std::sync::Arc::make_mut(&mut source.transformed)
        .functions
        .push(function);
    eq!(FixedViewCopyPlan::decode(&source.encode().unwrap()).unwrap() => source);
}

#[test]
fn installation_call_owner_round_trips_and_identity_keeps_operation_and_role() {
    use target_operations::{CallSiteOwner, InstallationCallRole};
    let mut original = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    std::sync::Arc::make_mut(&mut original.transformed).functions[0] = structural_function();
    std::sync::Arc::make_mut(&mut original.transformed).functions[0].calls[0].owner =
        CallSiteOwner::Installation {
            operation: OperationId::new(41).unwrap(),
            role: InstallationCallRole::Validate,
        };
    eq!(FixedViewCopyPlan::decode(&original.encode().unwrap()).unwrap() => original);
    let identity = selected_instructions::selected_instruction_plan_identity(&original.transformed);
    let encoded = original.encode().unwrap();
    for owner in [
        CallSiteOwner::Installation {
            operation: OperationId::new(41).unwrap(),
            role: InstallationCallRole::Work,
        },
        CallSiteOwner::Installation {
            operation: OperationId::new(43).unwrap(),
            role: InstallationCallRole::Validate,
        },
        CallSiteOwner::Operation(OperationId::new(41).unwrap()),
    ] {
        let mut changed = original.clone();
        std::sync::Arc::make_mut(&mut changed.transformed).functions[0].calls[0].owner = owner;
        assert_ne!(
            selected_instructions::selected_instruction_plan_identity(&changed.transformed),
            identity
        );
        assert_ne!(changed.encode().unwrap(), encoded);
        eq!(FixedViewCopyPlan::decode(&changed.encode().unwrap()).unwrap() => changed);
    }
}

#[test]
fn retained_plan_preserves_returned_input_fact_lifetimes() {
    use terminal_psi::{
        StructuralMultiplicity, StructuralOperationResult, StructuralPathQualification,
    };
    let mut plan = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    let mut function = structural_function();
    let domain = StructuralDomainId::new(3).unwrap();
    function.calls[0].call.structural_result = Some(StructuralOperationResult {
        place: PlaceId::new(7).unwrap(),
        structural_type: StructuralTypeId::new(2).unwrap(),
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: vec![domain],
        projected_qualifications: Vec::new(),
        additional_input_qualifications: vec![StructuralPathQualification {
            path: Vec::new(),
            domain,
        }],
        qualification_establishments: Vec::new(),
        claims: Vec::new(),
    });
    std::sync::Arc::make_mut(&mut plan.transformed)
        .functions
        .push(function);
    let encoded = plan.encode().unwrap();
    assert_eq!(FixedViewCopyPlan::decode(&encoded).unwrap(), plan);
    let identity = selected_instructions::selected_instruction_plan_identity(&plan.transformed);
    let mut standing = plan.clone();
    std::sync::Arc::make_mut(&mut standing.transformed)
        .functions
        .iter_mut()
        .next_back()
        .unwrap()
        .calls[0]
        .call
        .structural_result
        .as_mut()
        .unwrap()
        .additional_input_qualifications
        .clear();
    assert_ne!(
        selected_instructions::selected_instruction_plan_identity(&standing.transformed),
        identity
    );
    let standing_bytes = standing.encode().unwrap();
    assert_ne!(standing_bytes, encoded);
    assert_eq!(
        FixedViewCopyPlan::decode(&standing_bytes).unwrap(),
        standing
    );
}

fn catalog() -> StructuralTypeCatalog {
    let layout = LayoutPlanReport {
        schema_report_fingerprint: 19,
        offsets: None,
        size: Some(8),
        align: 8,
        entries: vec![LayoutFieldEntryReport {
            field: "payload".into(),
            member_identity: None,
            placement: LayoutPlacementReport::At { offset: 0 },
        }],
    };
    let placement = validate_access_plan(AccessPlan {
        access: AccessFields::inaccessible(&layout).unwrap(),
        layout,
        reach: BoundaryReach::default(),
    })
    .unwrap();
    let schema = StructuralTypeId::new(1).unwrap();
    let view = TerminalAccessViewPlan {
        carrier: StructuralTypeId::new(2).unwrap(),
        schema,
        view_identity: TerminalIdentityIndex(3),
        policy_identity: TerminalIdentityIndex(4),
        policy_plan_machine_identity: TerminalIdentityIndex(5),
        schema_identity: TerminalIdentityIndex(6),
        placement: placement.clone(),
        fields: vec![],
    };
    let initialize = TerminalAccessLifecyclePlan {
        target: Some(terminal_psi::TargetLayoutInput {
            target: "linux_x86_64".into(),
            pointer_size: 8,
            pointer_alignment: 8,
            case_tag_size: 4,
            case_tag_alignment: 4,
            little_endian: true,
        }),
        boundary: BoundaryMachineId::new(7).unwrap(),
        operation: TerminalAccessLifecycleOperation::Initialize,
        execution: TerminalAccessLifecycleExecution::CompilerPrimitive {
            profile_type: Some(StructuralTypeId::new(8).unwrap()),
            profile_requirements: vec![TerminalAccessProfileRequirement {
                boundary: BoundaryMachineId::new(9).unwrap(),
                owner: NominalDeclarationIdentity::new(TerminalIdentityIndex(10)),
                callable: TerminalIdentityIndex(11),
            }],
        },
        schema,
        image_fields: vec![StructuralFieldId::new(12).unwrap()],
        storage: TerminalAccessStorage {
            owner: terminal_psi::TerminalAccessOwner::Extent,
            carrier: StructuralTypeId::new(13).unwrap(),
            base: StructuralFieldId::new(14).unwrap(),
            length: StructuralFieldId::new(15).unwrap(),
            granted: StructuralDomainId::new(16).unwrap(),
            vacant: Some(StructuralDomainId::new(17).unwrap()),
            resident: Some(StructuralDomainId::new(18).unwrap()),
        },
        policy_identity: view.policy_identity,
        policy_plan_machine_identity: view.policy_plan_machine_identity,
        schema_identity: view.schema_identity,
        placement,
    };
    let mut extraction = initialize.clone();
    extraction.boundary = BoundaryMachineId::new(20).unwrap();
    extraction.operation = TerminalAccessLifecycleOperation::MoveOut;
    let mut selected = initialize.clone();
    selected.boundary = BoundaryMachineId::new(21).unwrap();
    selected.execution = TerminalAccessLifecycleExecution::SelectedProvider;
    StructuralTypeCatalog::default()
        .with_access_views(vec![view])
        .with_access_lifecycles(vec![initialize, extraction, selected])
        .unwrap()
}

#[test]
fn recovery_preserves_complete_placement_catalog_and_its_identity() {
    use selected_instructions::selected_instruction_plan_identity;
    let catalog = catalog();
    let mut source = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    let mut function = structural_function();
    function.structural.as_mut().unwrap().structural_types = catalog.clone();
    std::sync::Arc::make_mut(&mut source.transformed)
        .functions
        .push(function);
    let bytes = source.encode().unwrap();
    let decoded = FixedViewCopyPlan::decode(&bytes).unwrap();
    assert_eq!(decoded, source);
    assert_eq!(decoded.encode().unwrap(), bytes);
    let identity = selected_instruction_plan_identity(&source.transformed);
    assert_eq!(
        selected_instruction_plan_identity(&decoded.transformed),
        identity
    );

    for changed_catalog in [
        catalog.clone().with_access_views(vec![]),
        catalog.clone().with_access_lifecycles(vec![]).unwrap(),
        {
            let mut lifecycle = catalog.access_lifecycles().to_vec();
            lifecycle[0].target.as_mut().unwrap().case_tag_size = 8;
            catalog.clone().with_access_lifecycles(lifecycle).unwrap()
        },
        {
            let mut lifecycle = catalog.access_lifecycles().to_vec();
            lifecycle[0].target = None;
            catalog.clone().with_access_lifecycles(lifecycle).unwrap()
        },
        {
            let mut lifecycle = catalog.access_lifecycles().to_vec();
            lifecycle[0].storage.base = StructuralFieldId::new(22).unwrap();
            catalog.clone().with_access_lifecycles(lifecycle).unwrap()
        },
        {
            let mut lifecycle = catalog.access_lifecycles().to_vec();
            lifecycle[0].image_fields.clear();
            catalog.clone().with_access_lifecycles(lifecycle).unwrap()
        },
        {
            let mut lifecycle = catalog.access_lifecycles().to_vec();
            lifecycle[0].storage.owner = terminal_psi::TerminalAccessOwner::Run {
                carrier: StructuralTypeId::new(23).unwrap(),
                storage: StructuralFieldId::new(24).unwrap(),
                count: StructuralFieldId::new(25).unwrap(),
                singleton: Some(StructuralDomainId::new(26).unwrap()),
            };
            catalog.clone().with_access_lifecycles(lifecycle).unwrap()
        },
    ] {
        let mut changed = source.clone();
        std::sync::Arc::make_mut(&mut changed.transformed)
            .functions
            .iter_mut()
            .next_back()
            .unwrap()
            .structural
            .as_mut()
            .unwrap()
            .structural_types = changed_catalog;
        assert_ne!(changed.encode().unwrap(), bytes);
        assert_ne!(
            selected_instruction_plan_identity(&changed.transformed),
            identity
        );
        assert_eq!(
            FixedViewCopyPlan::decode(&changed.encode().unwrap()).unwrap(),
            changed
        );
    }
}

#[test]
fn standalone_catalogs_reject_truncation_and_trailing_bytes() {
    let catalog = catalog();
    let lifecycle =
        terminal_codec::encode_access_lifecycle_catalog(catalog.access_lifecycles()).unwrap();
    let views = terminal_codec::encode_access_view_catalog(catalog.access_views()).unwrap();
    for end in 0..lifecycle.len() {
        assert!(
            terminal_codec::decode_access_lifecycle_catalog(&lifecycle[..end]).is_err(),
            "truncated lifecycle catalog accepted at {end}"
        );
    }
    for end in 0..views.len() {
        assert!(
            terminal_codec::decode_access_view_catalog(&views[..end]).is_err(),
            "truncated view catalog accepted at {end}"
        );
    }
    let mut trailing = lifecycle;
    trailing.push(0);
    assert_eq!(
        terminal_codec::decode_access_lifecycle_catalog(&trailing),
        Err(terminal_codec::CodecError::TrailingBytes(1))
    );
    let mut trailing = views;
    trailing.push(0);
    assert_eq!(
        terminal_codec::decode_access_view_catalog(&trailing),
        Err(terminal_codec::CodecError::TrailingBytes(1))
    );
}
