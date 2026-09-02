//! Structural dominance and operation-result fixture units.

use super::*;

pub(crate) fn byte_literal_boundary_unit() -> PsiOptimizationUnit {
    let machine = id(4_600, MachineId::new);
    let block = id(4_601, BlockId::new);
    let boundary = id(4_602, BoundaryMachineId::new);
    let byte_type = id(4_603, StructuralTypeId::new);
    let literal = id(4_604, PlaceId::new);
    let boundary_place = id(4_605, PlaceId::new);
    let declaration = structural_type(
        4_603,
        psi_terminal::StructuralTypeShape::ByteSequence(
            psi_terminal::ByteSequenceCarrier::BorrowedView,
        ),
    );
    reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([20; 32]),
            },
            entry: machine,
            structural_types: vec![declaration.clone()],
            boundary_machines: vec![psi_terminal::BoundaryMachineDeclaration {
                id: boundary,
                identity: "validation::byte-literal-boundary".into(),
                attachment: None,
                scalar_parameters: Vec::new(),
                structural_parameters: vec![psi_terminal::StructuralParameterDeclaration {
                    place: boundary_place,
                    position: 0,
                    is_self: false,
                    structural_type: byte_type,
                    multiplicity: psi_terminal::StructuralMultiplicity::Unrestricted,
                    access: psi_terminal::StructuralAccess::SharedBorrow,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                }],
                result: None,
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
                    block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    AbstractOperation::EstablishByteSequenceLiteral {
                        psi_operation: id(4_606, OperationId::new),
                        place: psi_terminal::StructuralPlaceDeclaration {
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
                        result: None,
                        boundary,
                        arguments: Vec::new(),
                        structural_arguments: vec![psi_terminal::StructuralArgument {
                            place: literal,
                            access: psi_terminal::StructuralAccess::SharedBorrow,
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
    let path = vec![psi_terminal::StructuralPathSegment::Field("left".into())];
    let mut unit = reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([72; 32]),
            },
            entry: machine,
            structural_types: vec![
                psi_terminal::StructuralTypeDeclaration {
                    id: leaf,
                    identity: "validation::qualified-leaf".into(),
                    shape: psi_terminal::StructuralTypeShape::Record { fields: Vec::new() },
                },
                psi_terminal::StructuralTypeDeclaration {
                    id: root,
                    identity: "validation::qualified-root".into(),
                    shape: psi_terminal::StructuralTypeShape::Record {
                        fields: ["left", "right"]
                            .into_iter()
                            .enumerate()
                            .map(
                                |(index, identity)| psi_terminal::StructuralFieldDeclaration {
                                    id: id(4_729 + index as u64, psi_core::StructuralFieldId::new),
                                    identity: identity.into(),
                                    relevance: psi_terminal::BindingRelevance::Relevant,
                                    field_type: psi_terminal::StructuralFieldType::Structural(leaf),
                                },
                            )
                            .collect(),
                    },
                },
            ],
            boundary_machines: vec![psi_terminal::BoundaryMachineDeclaration {
                id: boundary,
                identity: "validation::consume-qualified-field".into(),
                attachment: None,
                scalar_parameters: Vec::new(),
                structural_parameters: vec![psi_terminal::StructuralParameterDeclaration {
                    place: boundary_place,
                    position: 0,
                    is_self: false,
                    structural_type: leaf,
                    multiplicity: psi_terminal::StructuralMultiplicity::Affine,
                    access: psi_terminal::StructuralAccess::SharedBorrow,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                }],
                result: None,
                requires: vec![psi_terminal::StructuralDomainRequirement {
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
                structural_parameters: vec![psi_terminal::StructuralParameterDeclaration {
                    place: caller_place,
                    position: 0,
                    is_self: false,
                    structural_type: root,
                    multiplicity: psi_terminal::StructuralMultiplicity::Affine,
                    access: psi_terminal::StructuralAccess::Owned,
                    qualifications: Vec::new(),
                    projected_qualifications: vec![psi_terminal::StructuralPathQualification {
                        path: path.clone(),
                        domain,
                    }],
                }],
                result: AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    AbstractOperation::BoundaryCall {
                        psi_operation: id(4_731, OperationId::new),
                        result: None,
                        boundary,
                        arguments: Vec::new(),
                        structural_arguments: vec![psi_terminal::StructuralArgument {
                            place: caller_place,
                            path,
                            access: psi_terminal::StructuralAccess::SharedBorrow,
                        }],
                        completion_claim_sources: Vec::new(),
                        completion_receipts: Vec::new(),
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: id(4_732, EdgeId::new),
                        cleanup_actions: vec![
                            psi_terminal::TerminalAffineCleanupAction::DiscardRoot(caller_place),
                        ],
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).expect("nonzero schedule"),
    )
    .expect("partial-path qualified boundary unit");
    unit.structural_domains = vec![
        psi_terminal::StructuralDomainDeclaration {
            id: domain,
            semantic_domain: id(4_725, psi_core::DomainSemanticId::new),
            identity: "validation::qualified-left".into(),
            carrier: leaf,
            content_projection: None,
        },
        psi_terminal::StructuralDomainDeclaration {
            id: foreign_domain,
            semantic_domain: id(4_726, psi_core::DomainSemanticId::new),
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
                .map(|site| omega_optimization_unit::FuelSettlement { site, units: 1 })
                .collect();
            node.effect = omega_optimization_unit::EffectLink {
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
        psi_edge: id(4_610, EdgeId::new),
        target: use_block,
        bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    unit.functions[0].entry = producer;
    unit.functions[0].blocks = vec![
        omega_optimization_unit::OptimizationBlock {
            id: use_block,
            parameters: Vec::new(),
            nodes: vec![boundary, returned],
        },
        omega_optimization_unit::OptimizationBlock {
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
        when_true: omega_abstract_operations::AbstractSuccessor {
            psi_edge: id(4_616, EdgeId::new),
            target: producer,
            bindings: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
        when_false: omega_abstract_operations::AbstractSuccessor {
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
        omega_optimization_unit::OptimizationBlock {
            id: entry,
            parameters: Vec::new(),
            nodes: vec![boolean, conditional],
        },
        omega_optimization_unit::OptimizationBlock {
            id: producer,
            parameters: Vec::new(),
            nodes: vec![establish, producer_return],
        },
        omega_optimization_unit::OptimizationBlock {
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
        when_true: omega_abstract_operations::AbstractSuccessor {
            psi_edge: id(4_636, EdgeId::new),
            target: producer,
            bindings: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
        when_false: omega_abstract_operations::AbstractSuccessor {
            psi_edge: id(4_637, EdgeId::new),
            target: bypass,
            bindings: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    };
    let jump = |edge| AbstractOperation::Jump {
        psi_edge: id(edge, EdgeId::new),
        target: join,
        bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let mut producer_jump = returned.clone();
    producer_jump.operation = jump(4_638);
    let mut bypass_jump = returned.clone();
    bypass_jump.operation = jump(4_639);
    unit.functions[0].entry = entry;
    unit.functions[0].blocks = vec![
        omega_optimization_unit::OptimizationBlock {
            id: entry,
            parameters: Vec::new(),
            nodes: vec![boolean, conditional],
        },
        omega_optimization_unit::OptimizationBlock {
            id: producer,
            parameters: Vec::new(),
            nodes: vec![establish, producer_jump],
        },
        omega_optimization_unit::OptimizationBlock {
            id: bypass,
            parameters: Vec::new(),
            nodes: vec![bypass_jump],
        },
        omega_optimization_unit::OptimizationBlock {
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
        psi_edge: id(4_641, EdgeId::new),
        target: cleanup,
        bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    unit.functions[0].blocks = vec![
        omega_optimization_unit::OptimizationBlock {
            id: cleanup,
            parameters: Vec::new(),
            nodes: vec![returned],
        },
        omega_optimization_unit::OptimizationBlock {
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
        field: id(4_644, psi_core::StructuralFieldId::new),
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
        when_true: omega_abstract_operations::AbstractSuccessor {
            psi_edge: id(4_625, EdgeId::new),
            target: producer,
            bindings: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
        when_false: omega_abstract_operations::AbstractSuccessor {
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
        omega_optimization_unit::OptimizationBlock {
            id: entry,
            parameters: Vec::new(),
            nodes: vec![boolean, conditional],
        },
        omega_optimization_unit::OptimizationBlock {
            id: producer,
            parameters: Vec::new(),
            nodes: vec![establish, producer_return],
        },
        omega_optimization_unit::OptimizationBlock {
            id: cleanup,
            parameters: Vec::new(),
            nodes: vec![returned],
        },
    ];
    refresh_function_derivatives(&mut unit, 0);
    unit
}

#[derive(Clone, Copy)]
pub(crate) enum OperationResultCfgShape {
    DominatingNonTopological,
    SiblingReturn,
    PartialPredecessor,
}

pub(crate) fn operation_result_cfg_unit(shape: OperationResultCfgShape) -> PsiOptimizationUnit {
    use omega_abstract_operations::AbstractSuccessor;

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
    let parameter = |place| psi_terminal::StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: psi_terminal::StructuralMultiplicity::Linear,
        access: psi_terminal::StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let entry_claim = |input| psi_terminal::EntryClaim {
        claim,
        input,
        path: Vec::new(),
    };
    let call = || AbstractOperation::CallStructural {
        psi_operation: id(382, OperationId::new),
        result: psi_terminal::StructuralOperationResult {
            place: call_result,
            structural_type,
            multiplicity: psi_terminal::StructuralMultiplicity::Linear,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: vec![psi_terminal::StructuralResultClaimBinding {
                claim,
                path: Vec::new(),
            }],
        },
        callee,
        arguments: Vec::new(),
        structural_arguments: vec![psi_terminal::StructuralArgument {
            place: caller_input,
            path: Vec::new(),
            access: psi_terminal::StructuralAccess::Owned,
        }],
        claim_transfers: vec![psi_terminal::ClaimTransfer {
            claim,
            argument_index: 0,
        }],
        returned_claim_transfers: vec![psi_terminal::StructuralResultClaimTransfer {
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
        psi_edge: edge,
        target: join,
        bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let conditional = || AbstractOperation::Conditional {
        condition,
        when_true: AbstractSuccessor {
            psi_edge: id(383, EdgeId::new),
            target: producer_block,
            bindings: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
        when_false: AbstractSuccessor {
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
                    block: join,
                    parameters: Vec::new(),
                    operation_offset: 0,
                },
                AbstractBlockEntry {
                    block: producer_block,
                    parameters: Vec::new(),
                    operation_offset: 1,
                },
                AbstractBlockEntry {
                    block: bypass_block,
                    parameters: Vec::new(),
                    operation_offset: 2,
                },
                AbstractBlockEntry {
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
                    block: entry,
                    parameters: Vec::new(),
                    operation_offset: 0,
                },
                AbstractBlockEntry {
                    block: producer_block,
                    parameters: Vec::new(),
                    operation_offset: 1,
                },
                AbstractBlockEntry {
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
                    block: entry,
                    parameters: Vec::new(),
                    operation_offset: 0,
                },
                AbstractBlockEntry {
                    block: producer_block,
                    parameters: Vec::new(),
                    operation_offset: 1,
                },
                AbstractBlockEntry {
                    block: bypass_block,
                    parameters: Vec::new(),
                    operation_offset: 3,
                },
                AbstractBlockEntry {
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
        structural_types: vec![psi_terminal::StructuralTypeDeclaration {
            id: structural_type,
            identity: "validation::operation-result-availability".into(),
            shape: psi_terminal::StructuralTypeShape::ByteSequence(
                psi_terminal::ByteSequenceCarrier::BorrowedView,
            ),
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
                    psi_terminal::StructuralResultDeclaration {
                        place: caller_result,
                        structural_type,
                        multiplicity: psi_terminal::StructuralMultiplicity::Linear,
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
                    psi_terminal::StructuralResultDeclaration {
                        place: callee_result,
                        structural_type,
                        multiplicity: psi_terminal::StructuralMultiplicity::Linear,
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
