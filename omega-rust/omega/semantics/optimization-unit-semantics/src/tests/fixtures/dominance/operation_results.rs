//! Operation-result availability across distinct control-flow shapes.

use super::*;

#[derive(Clone, Copy)]
pub(crate) enum OperationResultCfgShape {
    DominatingNonTopological,
    SiblingReturn,
    PartialPredecessor,
}

pub(crate) fn operation_result_cfg_unit(shape: OperationResultCfgShape) -> PsiOptimizationUnit {
    use abstract_operations::AbstractSuccessor;

    let caller = id(370, MachineId::new);
    let callee = id(371, MachineId::new);
    let entry = id(372, BlockId::new);
    let producer_block = id(373, BlockId::new);
    let bypass_block = id(374, BlockId::new);
    let join = id(375, BlockId::new);
    let callee_block = id(376, BlockId::new);
    let condition = id(377, ValueId::new);
    let structural_type = id(378, StructuralTypeId::new);
    let callee_result = id(379, PlaceId::new);
    let caller_result = id(380, PlaceId::new);
    let call_result = id(381, PlaceId::new);
    let caller_input = id(389, PlaceId::new);
    let callee_input = id(390, PlaceId::new);
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
    let call = || AbstractOperation::CallStructural {
        psi_operation: id(382, OperationId::new),
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
        returned_claim_transfers: vec![terminal_psi::StructuralResultClaimTransfer {
            callee_claim: claim,
            caller_claim: claim,
        }],
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
        selected_evidence: Vec::new(),
    };
    let return_result = |edge| AbstractOperation::ReturnStructural {
        psi_edge: edge,
        source: call_result,
        returned_claims: vec![claim],
        trivial_affine_locals: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let jump = |edge| AbstractOperation::Jump {
        structural_bindings: Vec::new(),
        psi_edge: edge,
        target: join,
        bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    let conditional = || AbstractOperation::Conditional {
        condition,
        when_true: AbstractSuccessor {
            structural_bindings: Vec::new(),
            psi_edge: id(383, EdgeId::new),
            target: producer_block,
            bindings: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
        when_false: AbstractSuccessor {
            structural_bindings: Vec::new(),
            psi_edge: id(384, EdgeId::new),
            target: bypass_block,
            bindings: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    };
    let (block_entries, operations) = match shape {
        OperationResultCfgShape::DominatingNonTopological => (
            vec![
                AbstractBlockEntry {
                    structural_parameters: Vec::new(),
                    block: join,
                    parameters: Vec::new(),
                    operation_offset: 0,
                },
                AbstractBlockEntry {
                    structural_parameters: Vec::new(),
                    block: producer_block,
                    parameters: Vec::new(),
                    operation_offset: 1,
                },
                AbstractBlockEntry {
                    structural_parameters: Vec::new(),
                    block: bypass_block,
                    parameters: Vec::new(),
                    operation_offset: 2,
                },
                AbstractBlockEntry {
                    structural_parameters: Vec::new(),
                    block: entry,
                    parameters: Vec::new(),
                    operation_offset: 3,
                },
            ],
            vec![
                return_result(id(385, EdgeId::new)),
                jump(id(386, EdgeId::new)),
                jump(id(387, EdgeId::new)),
                call(),
                conditional(),
            ],
        ),
        OperationResultCfgShape::SiblingReturn => (
            vec![
                AbstractBlockEntry {
                    structural_parameters: Vec::new(),
                    block: entry,
                    parameters: Vec::new(),
                    operation_offset: 0,
                },
                AbstractBlockEntry {
                    structural_parameters: Vec::new(),
                    block: producer_block,
                    parameters: Vec::new(),
                    operation_offset: 1,
                },
                AbstractBlockEntry {
                    structural_parameters: Vec::new(),
                    block: bypass_block,
                    parameters: Vec::new(),
                    operation_offset: 3,
                },
            ],
            vec![
                conditional(),
                call(),
                return_result(id(385, EdgeId::new)),
                return_result(id(386, EdgeId::new)),
            ],
        ),
        OperationResultCfgShape::PartialPredecessor => (
            vec![
                AbstractBlockEntry {
                    structural_parameters: Vec::new(),
                    block: entry,
                    parameters: Vec::new(),
                    operation_offset: 0,
                },
                AbstractBlockEntry {
                    structural_parameters: Vec::new(),
                    block: producer_block,
                    parameters: Vec::new(),
                    operation_offset: 1,
                },
                AbstractBlockEntry {
                    structural_parameters: Vec::new(),
                    block: bypass_block,
                    parameters: Vec::new(),
                    operation_offset: 3,
                },
                AbstractBlockEntry {
                    structural_parameters: Vec::new(),
                    block: join,
                    parameters: Vec::new(),
                    operation_offset: 4,
                },
            ],
            vec![
                conditional(),
                call(),
                jump(id(385, EdgeId::new)),
                jump(id(386, EdgeId::new)),
                return_result(id(387, EdgeId::new)),
            ],
        ),
    };
    let plan = AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([17; 32]),
        },
        entry: caller,
        structural_types: vec![terminal_psi::StructuralTypeDeclaration {
            id: structural_type,
            identity: "validation::operation-result-availability".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView),
        }],
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![
            AbstractFunction {
                machine: caller,
                attachment: None,
                entry,
                parameters: vec![AbstractParameter {
                    value: condition,
                    scalar_type: ScalarType::Boolean,
                }],
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
                block_entries,
                operations,
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
                    structural_parameters: Vec::new(),
                    block: callee_block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![AbstractOperation::ReturnStructural {
                    psi_edge: id(388, EdgeId::new),
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
