use super::super::id;
use abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan,
};
use optimization_unit::{PsiOptimizationUnit, reconstruct_psi_optimization_unit_seed};
use semantic_vocabulary::{
    BlockId, ClaimId, EdgeId, FuelScheduleIdentity, MachineId, OperationId, PlaceId,
};
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

pub(crate) fn structural_result_call_unit() -> PsiOptimizationUnit {
    let caller = id(350, MachineId::new);
    let callee = id(351, MachineId::new);
    let caller_block = id(352, BlockId::new);
    let callee_block = id(353, BlockId::new);
    let structural_type = id(354, semantic_vocabulary::StructuralTypeId::new);
    let callee_result = id(355, PlaceId::new);
    let call_result = id(356, PlaceId::new);
    let caller_result = id(362, PlaceId::new);
    let caller_input = id(360, PlaceId::new);
    let callee_input = id(361, PlaceId::new);
    let claim = id(1, ClaimId::new);
    let parameter = |place| terminal_psi::StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: terminal_psi::StructuralMultiplicity::Linear,
        access: terminal_psi::StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let entry_claim = |input| terminal_psi::EntryClaim {
        claim,
        input,
        path: Vec::new(),
    };
    let plan = AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([16; 32]),
        },
        entry: caller,
        structural_types: vec![terminal_psi::StructuralTypeDeclaration {
            id: structural_type,
            identity: "validation::structural-call-result".into(),
            shape: terminal_psi::StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView,
            ),
        }],
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![
            AbstractFunction {
                machine: caller,
                attachment: None,
                entry: caller_block,
                parameters: Vec::new(),
                structural_parameters: vec![parameter(caller_input)],
                result: AbstractFunctionResult::Structural(
                    terminal_psi::StructuralResultDeclaration {
                        place: caller_result,
                        structural_type,
                        multiplicity: terminal_psi::StructuralMultiplicity::Linear,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                    },
                ),
                entry_claims: vec![entry_claim(caller_input)],
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block: caller_block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    AbstractOperation::CallStructural {
                        psi_operation: id(357, OperationId::new),
                        result: terminal_psi::StructuralOperationResult {
                            place: call_result,
                            structural_type,
                            multiplicity: terminal_psi::StructuralMultiplicity::Linear,
                            qualifications: Vec::new(),
                            projected_qualifications: Vec::new(),
                            claims: vec![terminal_psi::StructuralResultClaimBinding {
                                claim,
                                path: Vec::new(),
                            }],
                        },
                        callee,
                        arguments: Vec::new(),
                        structural_arguments: vec![terminal_psi::StructuralArgument {
                            place: caller_input,
                            path: Vec::new(),
                            access: terminal_psi::StructuralAccess::Owned,
                        }],
                        claim_transfers: vec![terminal_psi::ClaimTransfer {
                            claim,
                            argument_index: 0,
                        }],
                        returned_claim_transfers: vec![
                            terminal_psi::StructuralResultClaimTransfer {
                                callee_claim: claim,
                                caller_claim: claim,
                            },
                        ],
                        requirement_obligations: Vec::new(),
                        crash_continuations: Vec::new(),
                        selected_evidence: Vec::new(),
                    },
                    AbstractOperation::ReturnStructural {
                        psi_edge: id(358, EdgeId::new),
                        source: call_result,
                        returned_claims: vec![claim],
                        trivial_affine_locals: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                ],
            },
            AbstractFunction {
                machine: callee,
                attachment: None,
                entry: callee_block,
                parameters: Vec::new(),
                structural_parameters: vec![parameter(callee_input)],
                result: AbstractFunctionResult::Structural(
                    terminal_psi::StructuralResultDeclaration {
                        place: callee_result,
                        structural_type,
                        multiplicity: terminal_psi::StructuralMultiplicity::Linear,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                    },
                ),
                entry_claims: vec![entry_claim(callee_input)],
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block: callee_block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![AbstractOperation::ReturnStructural {
                    psi_edge: id(359, EdgeId::new),
                    source: callee_input,
                    returned_claims: vec![claim],
                    trivial_affine_locals: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                }],
            },
        ],
    };
    reconstruct_psi_optimization_unit_seed(&plan, FuelScheduleIdentity::new(1).unwrap()).unwrap()
}
