//! Structural dominance and operation-result fixture units.

use super::*;
use terminal_psi::{ByteSequenceCarrier, StructuralTypeShape};

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
        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView),
    );
    reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([20; 32]),
            },
            entry: machine,
            structural_types: vec![declaration.clone()],
            boundary_machines: vec![terminal_psi::BoundaryMachineDeclaration {
                id: boundary,
                identity: "validation::byte-literal-boundary".into(),
                attachment: None,
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
            ],
            boundary_machines: vec![terminal_psi::BoundaryMachineDeclaration {
                id: boundary,
                identity: "validation::consume-qualified-field".into(),
                attachment: None,
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
            id: domain,
            semantic_domain: id(4_725, semantic_vocabulary::DomainSemanticId::new),
            identity: "validation::qualified-left".into(),
            carrier: leaf,
            content_projection: None,
        },
        terminal_psi::StructuralDomainDeclaration {
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

pub(crate) fn byte_literal_dominating_non_topological_unit() -> PsiOptimizationUnit {
    let mut unit = byte_literal_boundary_unit();
    let producer = id(4_601, BlockId::new);
    let use_block = id(4_609, BlockId::new);
    let mut nodes = std::mem::take(&mut unit.functions[0].blocks[0].nodes).into_iter();
    let establish = nodes.next().expect("literal establishment");
    let boundary = nodes.next().expect("literal boundary use");
    let returned = nodes.next().expect("Unit return");
    let mut jump = returned.clone();
    jump.operation = AbstractOperation::Jump {
        structural_bindings: Vec::new(),
        psi_edge: id(4_610, EdgeId::new),
        target: use_block,
        bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    unit.functions[0].entry = producer;
    unit.functions[0].blocks = vec![
        optimization_unit::OptimizationBlock {
            structural_parameters: Vec::new(),
            id: use_block,
            parameters: Vec::new(),
            nodes: vec![boundary, returned],
        },
        optimization_unit::OptimizationBlock {
            structural_parameters: Vec::new(),
            id: producer,
            parameters: Vec::new(),
            nodes: vec![establish, jump],
        },
    ];
    refresh_function_derivatives(&mut unit, 0);
    unit
}

pub(crate) fn byte_literal_sibling_use_unit() -> PsiOptimizationUnit {
    let mut unit = byte_literal_boundary_unit();
    let entry = id(4_611, BlockId::new);
    let producer = id(4_612, BlockId::new);
    let use_block = id(4_613, BlockId::new);
    let condition = id(4_614, ValueId::new);
    let mut nodes = std::mem::take(&mut unit.functions[0].blocks[0].nodes).into_iter();
    let establish = nodes.next().expect("literal establishment");
    let boundary = nodes.next().expect("literal boundary use");
    let returned = nodes.next().expect("Unit return");
    let mut boolean = establish.clone();
    boolean.operation = AbstractOperation::BooleanConstant {
        psi_operation: id(4_615, OperationId::new),
        result: condition,
        value: true,
    };
    let mut conditional = returned.clone();
    conditional.operation = AbstractOperation::Conditional {
        condition,
        when_true: abstract_operations::AbstractSuccessor {
            structural_bindings: Vec::new(),
            psi_edge: id(4_616, EdgeId::new),
            target: producer,
            bindings: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
        when_false: abstract_operations::AbstractSuccessor {
            structural_bindings: Vec::new(),
            psi_edge: id(4_617, EdgeId::new),
            target: use_block,
            bindings: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    };
    let mut producer_return = returned.clone();
    producer_return.operation = AbstractOperation::ReturnUnit {
        psi_edge: id(4_618, EdgeId::new),
        cleanup_actions: Vec::new(),
    };
    unit.functions[0].entry = entry;
    unit.functions[0].blocks = vec![
        optimization_unit::OptimizationBlock {
            structural_parameters: Vec::new(),
            id: entry,
            parameters: Vec::new(),
            nodes: vec![boolean, conditional],
        },
        optimization_unit::OptimizationBlock {
            structural_parameters: Vec::new(),
            id: producer,
            parameters: Vec::new(),
            nodes: vec![establish, producer_return],
        },
        optimization_unit::OptimizationBlock {
            structural_parameters: Vec::new(),
            id: use_block,
            parameters: Vec::new(),
            nodes: vec![boundary, returned],
        },
    ];
    refresh_function_derivatives(&mut unit, 0);
    unit
}

pub(crate) fn byte_literal_partial_predecessor_unit() -> PsiOptimizationUnit {
    let mut unit = byte_literal_boundary_unit();
    let entry = id(4_630, BlockId::new);
    let producer = id(4_631, BlockId::new);
    let bypass = id(4_632, BlockId::new);
    let join = id(4_633, BlockId::new);
    let condition = id(4_634, ValueId::new);
    let mut nodes = std::mem::take(&mut unit.functions[0].blocks[0].nodes).into_iter();
    let establish = nodes.next().expect("literal establishment");
    let boundary = nodes.next().expect("literal boundary use");
    let returned = nodes.next().expect("Unit return");
    let mut boolean = establish.clone();
    boolean.operation = AbstractOperation::BooleanConstant {
        psi_operation: id(4_635, OperationId::new),
        result: condition,
        value: true,
    };
    let mut conditional = returned.clone();
    conditional.operation = AbstractOperation::Conditional {
        condition,
        when_true: abstract_operations::AbstractSuccessor {
            structural_bindings: Vec::new(),
            psi_edge: id(4_636, EdgeId::new),
            target: producer,
            bindings: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
        when_false: abstract_operations::AbstractSuccessor {
            structural_bindings: Vec::new(),
            psi_edge: id(4_637, EdgeId::new),
            target: bypass,
            bindings: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    };
    let jump = |edge| AbstractOperation::Jump {
        structural_bindings: Vec::new(),
        psi_edge: id(edge, EdgeId::new),
        target: join,
        bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    let mut producer_jump = returned.clone();
    producer_jump.operation = jump(4_638);
    let mut bypass_jump = returned.clone();
    bypass_jump.operation = jump(4_639);
    unit.functions[0].entry = entry;
    unit.functions[0].blocks = vec![
        optimization_unit::OptimizationBlock {
            structural_parameters: Vec::new(),
            id: entry,
            parameters: Vec::new(),
            nodes: vec![boolean, conditional],
        },
        optimization_unit::OptimizationBlock {
            structural_parameters: Vec::new(),
            id: producer,
            parameters: Vec::new(),
            nodes: vec![establish, producer_jump],
        },
        optimization_unit::OptimizationBlock {
            structural_parameters: Vec::new(),
            id: bypass,
            parameters: Vec::new(),
            nodes: vec![bypass_jump],
        },
        optimization_unit::OptimizationBlock {
            structural_parameters: Vec::new(),
            id: join,
            parameters: Vec::new(),
            nodes: vec![boundary, returned],
        },
    ];
    refresh_function_derivatives(&mut unit, 0);
    unit
}

pub(crate) fn explicit_local_dominating_non_topological_unit() -> PsiOptimizationUnit {
    let mut unit = explicit_trivial_affine_return_unit();
    let producer = id(391, BlockId::new);
    let cleanup = id(4_640, BlockId::new);
    let mut nodes = std::mem::take(&mut unit.functions[0].blocks[0].nodes).into_iter();
    let establish = nodes.next().expect("local establishment");
    let returned = nodes.next().expect("local cleanup return");
    let mut jump = returned.clone();
    jump.operation = AbstractOperation::Jump {
        structural_bindings: Vec::new(),
        psi_edge: id(4_641, EdgeId::new),
        target: cleanup,
        bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    unit.functions[0].blocks = vec![
        optimization_unit::OptimizationBlock {
            structural_parameters: Vec::new(),
            id: cleanup,
            parameters: Vec::new(),
            nodes: vec![returned],
        },
        optimization_unit::OptimizationBlock {
            structural_parameters: Vec::new(),
            id: producer,
            parameters: Vec::new(),
            nodes: vec![establish, jump],
        },
    ];
    refresh_function_derivatives(&mut unit, 0);
    unit
}

pub(crate) fn explicit_local_same_block_use_before_definition_unit() -> PsiOptimizationUnit {
    let mut unit = explicit_trivial_affine_return_unit();
    let local = id(393, PlaceId::new);
    let mut observation = unit.functions[0].blocks[0].nodes[0].clone();
    observation.operation = AbstractOperation::BooleanStructuralField {
        psi_operation: id(4_642, OperationId::new),
        result: id(4_643, ValueId::new),
        source: local,
        field: id(4_644, semantic_vocabulary::StructuralFieldId::new),
    };
    unit.functions[0].blocks[0].nodes.insert(0, observation);
    refresh_function_derivatives(&mut unit, 0);
    unit
}

pub(crate) fn explicit_local_sibling_cleanup_unit() -> PsiOptimizationUnit {
    let mut unit = explicit_trivial_affine_return_unit();
    let entry = id(4_620, BlockId::new);
    let producer = id(4_621, BlockId::new);
    let cleanup = id(4_622, BlockId::new);
    let condition = id(4_623, ValueId::new);
    let mut nodes = std::mem::take(&mut unit.functions[0].blocks[0].nodes).into_iter();
    let establish = nodes.next().expect("local establishment");
    let returned = nodes.next().expect("local cleanup return");
    let mut boolean = establish.clone();
    boolean.operation = AbstractOperation::BooleanConstant {
        psi_operation: id(4_624, OperationId::new),
        result: condition,
        value: true,
    };
    let mut conditional = returned.clone();
    conditional.operation = AbstractOperation::Conditional {
        condition,
        when_true: abstract_operations::AbstractSuccessor {
            structural_bindings: Vec::new(),
            psi_edge: id(4_625, EdgeId::new),
            target: producer,
            bindings: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
        when_false: abstract_operations::AbstractSuccessor {
            structural_bindings: Vec::new(),
            psi_edge: id(4_626, EdgeId::new),
            target: cleanup,
            bindings: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    };
    let mut producer_return = returned.clone();
    producer_return.operation = AbstractOperation::ReturnUnit {
        psi_edge: id(4_627, EdgeId::new),
        cleanup_actions: match &returned.operation {
            AbstractOperation::ReturnUnit {
                cleanup_actions, ..
            } => cleanup_actions.clone(),
            _ => unreachable!("fixture return"),
        },
    };
    unit.functions[0].entry = entry;
    unit.functions[0].blocks = vec![
        optimization_unit::OptimizationBlock {
            structural_parameters: Vec::new(),
            id: entry,
            parameters: Vec::new(),
            nodes: vec![boolean, conditional],
        },
        optimization_unit::OptimizationBlock {
            structural_parameters: Vec::new(),
            id: producer,
            parameters: Vec::new(),
            nodes: vec![establish, producer_return],
        },
        optimization_unit::OptimizationBlock {
            structural_parameters: Vec::new(),
            id: cleanup,
            parameters: Vec::new(),
            nodes: vec![returned],
        },
    ];
    refresh_function_derivatives(&mut unit, 0);
    unit
}
