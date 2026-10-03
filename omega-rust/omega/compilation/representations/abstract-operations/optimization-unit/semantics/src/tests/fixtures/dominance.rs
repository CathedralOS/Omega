//! Structural dominance and operation-result fixture units.
use crate::tests::fixtures::structural_catalog::structural_type;
use crate::tests::support::{id, refresh_identity};
use crate::unit_validation::derived_metadata::{
    expected_definitions, expected_edges, expected_ownership, expected_provenance, expected_uses,
};
use crate::unit_validation::function_structure::reconstruct_fact_index;
use abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan,
};
use optimization_unit::{PsiOptimizationUnit, reconstruct_psi_optimization_unit_seed};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, EdgeId, FuelScheduleIdentity, MachineId, OperationId, PlaceId,
    StructuralDomainId, StructuralPlaceKind, StructuralTypeId,
};
use terminal_psi::{
    ByteSequenceCarrier, SemanticFingerprint, StructuralTypeShape, TerminalPsiIdentity,
    VocabularyMarker,
};

mod operation_results;
pub(crate) use operation_results::{OperationResultCfgShape, operation_result_cfg_unit};

pub(crate) fn byte_literal_boundary_unit() -> PsiOptimizationUnit {
    let machine = id(4_600, MachineId::new);
    let block = id(4_601, BlockId::new);
    let boundary = id(4_602, BoundaryMachineId::new);
    let byte_type = id(4_603, StructuralTypeId::new);
    let literal = id(4_604, PlaceId::new);
    let boundary_place = id(4_605, PlaceId::new);
    let declaration = structural_type(
        4_603,
        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
            access: Some(terminal_psi::StructuralAccess::SharedBorrow),
        }),
    );
    reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([20; 32]),
            },
            entry: machine,
            structural_types: vec![declaration.clone()].into(),
            boundary_machines: vec![terminal_psi::BoundaryMachineDeclaration {
                unconditional_termination: false,
                completion: terminal_psi::BoundaryCompletion::NormalReturn,
                kind: terminal_psi::BoundaryRequirementKind::Trait,
                scalar_ensures: Vec::new(),
                scalar_requires: Vec::new(),
                fixed_service_reach: Vec::new(),
                id: boundary,
                identity: "validation::byte-literal-boundary".into(),
                attachment: None,
                parameter_order: vec![terminal_psi::BoundaryParameterKind::Structural],
                scalar_parameters: Vec::new(),
                structural_parameters: vec![terminal_psi::StructuralParameterDeclaration {
                    place: boundary_place,
                    position: 0,
                    is_self: false,
                    structural_type: byte_type,
                    multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                    access: terminal_psi::StructuralAccess::SharedBorrow,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                }],
                result: terminal_psi::BoundaryMachineResult::Unit,
                requires: Vec::new(),
                program_local_root_introductions: Vec::new(),
                content_guarantees: Vec::new(),
                published_service_ceiling: Vec::new(),
                crash_routes: Vec::new(),
            }],
            provider_candidates: Vec::new(),
            functions: vec![AbstractFunction {
                machine,
                attachment: None,
                entry: block,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    structural_parameters: Vec::new(),
                    block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    AbstractOperation::EstablishByteSequenceLiteral {
                        psi_operation: id(4_606, OperationId::new),
                        place: terminal_psi::StructuralPlaceDeclaration {
                            id: literal,
                            kind: StructuralPlaceKind::ByteSequenceLiteral {
                                declaration_ordinal: 0,
                                structural_type: byte_type,
                            },
                        },
                        structural_type: declaration,
                        bytes: vec![0, 0x7f, 0x80, 0xff],
                        qualifications: Vec::new(),
                    },
                    AbstractOperation::BoundaryCall {
                        psi_operation: id(4_607, OperationId::new),
                        result: abstract_operations::AbstractBoundaryResult::Unit,
                        boundary,
                        arguments: Vec::new(),
                        structural_arguments: vec![terminal_psi::StructuralArgument {
                            place: literal,
                            access: terminal_psi::StructuralAccess::SharedBorrow,
                            path: Vec::new(),
                        }],
                        completion_claim_sources: Vec::new(),
                        completion_receipts: Vec::new(),
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: id(4_608, EdgeId::new),
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).expect("nonzero schedule"),
    )
    .expect("byte literal boundary unit")
}

pub(crate) fn partial_path_qualified_boundary_unit() -> PsiOptimizationUnit {
    let machine = id(4_720, MachineId::new);
    let block = id(4_721, BlockId::new);
    let boundary = id(4_722, BoundaryMachineId::new);
    let leaf = id(4_723, StructuralTypeId::new);
    let root = id(4_724, StructuralTypeId::new);
    let domain = id(4_725, StructuralDomainId::new);
    let foreign_domain = id(4_726, StructuralDomainId::new);
    let caller_place = id(4_727, PlaceId::new);
    let boundary_place = id(4_728, PlaceId::new);
    let path = vec![terminal_psi::StructuralPathSegment::Field("left".into())];
    let mut unit = reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([72; 32]),
            },
            entry: machine,
            structural_types: vec![
                terminal_psi::StructuralTypeDeclaration {
                    id: leaf,
                    identity: "validation::qualified-leaf".into(),
                    shape: StructuralTypeShape::Record { fields: Vec::new() },
                },
                terminal_psi::StructuralTypeDeclaration {
                    id: root,
                    identity: "validation::qualified-root".into(),
                    shape: StructuralTypeShape::Record {
                        fields: ["left", "right"]
                            .into_iter()
                            .enumerate()
                            .map(
                                |(index, identity)| terminal_psi::StructuralFieldDeclaration {
                                    id: id(
                                        4_729 + index as u64,
                                        semantic_vocabulary::StructuralFieldId::new,
                                    ),
                                    identity: identity.into(),
                                    relevance: terminal_psi::BindingRelevance::Relevant,
                                    field_type: terminal_psi::StructuralFieldType::Structural(leaf),
                                },
                            )
                            .collect(),
                    },
                },
            ]
            .into(),
            boundary_machines: vec![terminal_psi::BoundaryMachineDeclaration {
                unconditional_termination: false,
                completion: terminal_psi::BoundaryCompletion::NormalReturn,
                kind: terminal_psi::BoundaryRequirementKind::Trait,
                scalar_ensures: Vec::new(),
                scalar_requires: Vec::new(),
                fixed_service_reach: Vec::new(),
                id: boundary,
                identity: "validation::consume-qualified-field".into(),
                attachment: None,
                parameter_order: vec![terminal_psi::BoundaryParameterKind::Structural],
                scalar_parameters: Vec::new(),
                structural_parameters: vec![terminal_psi::StructuralParameterDeclaration {
                    place: boundary_place,
                    position: 0,
                    is_self: false,
                    structural_type: leaf,
                    multiplicity: terminal_psi::StructuralMultiplicity::Affine,
                    access: terminal_psi::StructuralAccess::SharedBorrow,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                }],
                result: terminal_psi::BoundaryMachineResult::Unit,
                requires: vec![terminal_psi::StructuralDomainRequirement {
                    argument_index: 0,
                    domain,
                }],
                program_local_root_introductions: Vec::new(),
                content_guarantees: Vec::new(),
                published_service_ceiling: Vec::new(),
                crash_routes: Vec::new(),
            }],
            provider_candidates: Vec::new(),
            functions: vec![AbstractFunction {
                machine,
                attachment: None,
                entry: block,
                parameters: Vec::new(),
                structural_parameters: vec![terminal_psi::StructuralParameterDeclaration {
                    place: caller_place,
                    position: 0,
                    is_self: false,
                    structural_type: root,
                    multiplicity: terminal_psi::StructuralMultiplicity::Affine,
                    access: terminal_psi::StructuralAccess::Owned,
                    qualifications: Vec::new(),
                    projected_qualifications: vec![terminal_psi::StructuralPathQualification {
                        path: path.clone(),
                        domain,
                    }],
                }],
                result: AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    structural_parameters: Vec::new(),
                    block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    AbstractOperation::BoundaryCall {
                        psi_operation: id(4_731, OperationId::new),
                        result: abstract_operations::AbstractBoundaryResult::Unit,
                        boundary,
                        arguments: Vec::new(),
                        structural_arguments: vec![terminal_psi::StructuralArgument {
                            place: caller_place,
                            path,
                            access: terminal_psi::StructuralAccess::SharedBorrow,
                        }],
                        completion_claim_sources: Vec::new(),
                        completion_receipts: Vec::new(),
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: id(4_732, EdgeId::new),
                        cleanup_actions: vec![
                            terminal_psi::TerminalAffineCleanupAction::DiscardRoot(caller_place),
                        ],
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).expect("nonzero schedule"),
    )
    .expect("partial-path qualified boundary unit");
    unit.structural_domains = vec![
        terminal_psi::StructuralDomainDeclaration {
            byte_predicate: None,
            establishment_routes: Vec::new(),
            id: domain,
            semantic_domain: id(4_725, semantic_vocabulary::DomainSemanticId::new),
            identity: "validation::qualified-left".into(),
            carrier: leaf,
            content_projection: None,
        },
        terminal_psi::StructuralDomainDeclaration {
            byte_predicate: None,
            establishment_routes: Vec::new(),
            id: foreign_domain,
            semantic_domain: id(4_726, semantic_vocabulary::DomainSemanticId::new),
            identity: "validation::qualified-foreign".into(),
            carrier: leaf,
            content_projection: None,
        },
    ]
    .into();
    refresh_identity(&mut unit);
    unit
}

pub(crate) fn refresh_function_derivatives(unit: &mut PsiOptimizationUnit, function_index: usize) {
    let function = &mut unit.functions[function_index];
    let mut effect = 0_u64;
    for block in &mut function.blocks {
        for (node_index, node) in block.nodes.iter_mut().enumerate() {
            let node_index = u32::try_from(node_index).expect("test node index fits u32");
            node.definitions = expected_definitions(&node.operation, block.id, node_index);
            node.uses = expected_uses(&node.operation, block.id, node_index);
            node.provenance = expected_provenance(&node.operation);
            node.fuel = node
                .provenance
                .iter()
                .copied()
                .map(|site| optimization_unit::FuelSettlement { site, units: 1 })
                .collect();
            node.effect = optimization_unit::EffectLink {
                input: effect,
                output: effect + 1,
            };
            effect += 1;
            node.successors = expected_edges(&node.operation);
            node.ownership = expected_ownership(&node.operation);
        }
    }
    function.facts = reconstruct_fact_index(function);
    refresh_identity(unit);
}
