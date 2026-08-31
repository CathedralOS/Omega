//! Structural catalog, service, domain, and affine-local fixtures.

use super::*;

pub(crate) fn boolean_structural_field_unit() -> PsiOptimizationUnit {
    let machine = id(4_700, MachineId::new);
    let block = id(4_701, BlockId::new);
    let place = id(4_702, PlaceId::new);
    let structural_type = id(4_703, StructuralTypeId::new);
    let field = id(4_704, psi_core::StructuralFieldId::new);
    let scalar_parameter = id(4_705, ValueId::new);
    let result = id(4_706, ValueId::new);
    let cleanup_machine = id(4_709, MachineId::new);
    let cleanup_block = id(4_710, BlockId::new);
    reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([47; 32]),
            },
            entry: machine,
            structural_types: vec![psi_terminal::StructuralTypeDeclaration {
                id: structural_type,
                identity: "validation::observed-affine-record".into(),
                shape: psi_terminal::StructuralTypeShape::Record {
                    fields: vec![psi_terminal::StructuralFieldDeclaration {
                        id: field,
                        identity: "ready".into(),
                        relevance: psi_terminal::BindingRelevance::Relevant,
                        field_type: psi_terminal::StructuralFieldType::Scalar(ScalarType::Boolean),
                    }],
                },
            }],
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![
                AbstractFunction {
                    machine,
                    attachment: None,
                    entry: block,
                    parameters: vec![AbstractParameter {
                        value: scalar_parameter,
                        scalar_type: ScalarType::Boolean,
                    }],
                    structural_parameters: vec![psi_terminal::StructuralParameterDeclaration {
                        place,
                        position: 0,
                        is_self: false,
                        structural_type,
                        multiplicity: psi_terminal::StructuralMultiplicity::Affine,
                        access: psi_terminal::StructuralAccess::Owned,
                        qualifications: Vec::new(),
                    }],
                    result: AbstractFunctionResult::Scalar(AbstractResult {
                        value: result,
                        scalar_type: ScalarType::Boolean,
                    }),
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![AbstractBlockEntry {
                        block,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    }],
                    operations: vec![
                        AbstractOperation::BooleanStructuralField {
                            psi_operation: id(4_707, OperationId::new),
                            result,
                            source: place,
                            field,
                        },
                        AbstractOperation::Return {
                            psi_edge: id(4_708, EdgeId::new),
                            result,
                            value: result,
                            scalar_type: ScalarType::Boolean,
                            cleanup_actions: vec![
                                psi_terminal::TerminalAffineCleanupAction::InvokeNominal(
                                    psi_terminal::NominalAffineCleanup {
                                        place,
                                        structural_type,
                                        cleanup_machine,
                                        cleanup_receiver: None,
                                        requirement_obligations: Vec::new(),
                                    },
                                ),
                            ],
                        },
                    ],
                },
                AbstractFunction {
                    machine: cleanup_machine,
                    attachment: Some(structural_type),
                    entry: cleanup_block,
                    parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    result: AbstractFunctionResult::Unit,
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![AbstractBlockEntry {
                        block: cleanup_block,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    }],
                    operations: vec![AbstractOperation::ReturnUnit {
                        psi_edge: id(4_711, EdgeId::new),
                        cleanup_actions: Vec::new(),
                    }],
                },
            ],
        },
        FuelScheduleIdentity::new(1).expect("nonzero schedule"),
    )
    .expect("Boolean structural-field fixture")
}

pub(crate) fn content_entry_claim(
    claim: ClaimId,
    root: PlaceId,
) -> psi_terminal::ContentEntryClaim {
    let algebra = psi_core::ContentAlgebra {
        kind: psi_core::ContentAlgebraKind::CountedQuantity,
        parameter: "validation::content-only-claim".into(),
    };
    let expression = psi_core::ContentProjectionExpression::CountedQuantity(
        psi_core::ContentProjectionScalar::Natural("1".into()),
    );
    psi_terminal::ContentEntryClaim {
        claim,
        input: psi_core::ContentStructuralPlace {
            version: psi_core::ContentPlaceVersion::Entry,
            root,
            segments: Vec::new(),
        },
        projections: vec![psi_terminal::ClaimContentProjection {
            projection: psi_core::ContentProjectionIdentity {
                domain: id(1, psi_core::ContentDomainId::new),
                projection_report_fingerprint:
                    psi_language_semantics::content::terminal_projection_report_fingerprint(
                        &algebra,
                        &expression,
                    ),
            },
            algebra,
        }],
    }
}

pub(crate) fn install_content_owner(unit: &mut PsiOptimizationUnit) {
    let carrier = unit.structural_types[0].id;
    let semantic_domain = id(1, psi_core::DomainSemanticId::new);
    let algebra = psi_core::ContentAlgebra {
        kind: psi_core::ContentAlgebraKind::CountedQuantity,
        parameter: "validation::content-only-claim".into(),
    };
    let expression = psi_core::ContentProjectionExpression::CountedQuantity(
        psi_core::ContentProjectionScalar::Natural("1".into()),
    );
    unit.structural_domains = vec![psi_terminal::StructuralDomainDeclaration {
        id: id(1, StructuralDomainId::new),
        semantic_domain,
        identity: "validation::content-only-domain".into(),
        carrier,
        content_projection: Some(psi_terminal::StructuralContentProjection {
            identity: psi_core::ContentProjectionIdentity {
                domain: id(semantic_domain.get(), psi_core::ContentDomainId::new),
                projection_report_fingerprint:
                    psi_language_semantics::content::terminal_projection_report_fingerprint(
                        &algebra,
                        &expression,
                    ),
            },
            algebra,
            expression,
        }),
    }]
    .into();
}

pub(crate) fn structural_field(
    raw: u64,
    target: StructuralTypeId,
) -> psi_terminal::StructuralFieldDeclaration {
    structural_leaf_field(
        raw,
        psi_terminal::BindingRelevance::Relevant,
        psi_terminal::StructuralFieldType::Structural(target),
    )
}

pub(crate) fn structural_leaf_field(
    raw: u64,
    relevance: psi_terminal::BindingRelevance,
    field_type: psi_terminal::StructuralFieldType,
) -> psi_terminal::StructuralFieldDeclaration {
    psi_terminal::StructuralFieldDeclaration {
        id: id(raw, psi_core::StructuralFieldId::new),
        identity: format!("validation::field-{raw}"),
        relevance,
        field_type,
    }
}

pub(crate) fn structural_case(
    raw: u64,
    fields: Vec<psi_terminal::StructuralFieldDeclaration>,
) -> psi_terminal::StructuralCaseDeclaration {
    psi_terminal::StructuralCaseDeclaration {
        id: id(raw, psi_core::StructuralCaseId::new),
        identity: format!("validation::case-{raw}"),
        fields,
    }
}

pub(crate) fn structural_type(
    raw: u64,
    shape: psi_terminal::StructuralTypeShape,
) -> psi_terminal::StructuralTypeDeclaration {
    psi_terminal::StructuralTypeDeclaration {
        id: id(raw, StructuralTypeId::new),
        identity: format!("validation::type-{raw}"),
        shape,
    }
}

pub(crate) fn structural_catalog_unit(
    structural_types: Vec<psi_terminal::StructuralTypeDeclaration>,
) -> PsiOptimizationUnit {
    let mut candidate = unit();
    candidate.structural_types = structural_types;
    refresh_identity(&mut candidate);
    candidate
}

pub(crate) fn service_declarations() -> Vec<psi_terminal::ServiceDeclaration> {
    let root = id(701, ServiceId::new);
    let middle = id(702, ServiceId::new);
    let leaf = id(703, ServiceId::new);
    vec![
        psi_terminal::ServiceDeclaration {
            id: root,
            identity: "validation::service-root".into(),
            parents: Vec::new(),
        },
        psi_terminal::ServiceDeclaration {
            id: middle,
            identity: "validation::service-middle".into(),
            parents: vec![root],
        },
        psi_terminal::ServiceDeclaration {
            id: leaf,
            identity: "validation::service-leaf".into(),
            parents: vec![root, middle],
        },
    ]
}

pub(crate) fn install_service_catalog(unit: &mut PsiOptimizationUnit) {
    let services = service_declarations();
    let ceiling = services
        .iter()
        .map(|service| service.id)
        .collect::<Vec<_>>();
    unit.services = services.into();
    for function in &mut unit.functions {
        function.published_service_ceiling = ceiling.clone();
    }
    for boundary in &mut unit.boundary_machines {
        boundary.published_service_ceiling = ceiling.clone();
    }
    refresh_root_service_reach(unit).expect("service fixture has a closed root reach");
    refresh_identity(unit);
}

pub(crate) fn service_effect_unit() -> PsiOptimizationUnit {
    let mut candidate = unit();
    install_service_catalog(&mut candidate);
    let block = candidate.functions[0].blocks[0].id;
    let mut node = candidate.functions[0].blocks[0].nodes[0].clone();
    node.operation = AbstractOperation::PortWrite {
        psi_operation: id(704, OperationId::new),
        service: id(703, ServiceId::new),
        port: 0x3f8,
        value: 0x41,
    };
    node.provenance = expected_provenance(&node.operation);
    node.fuel = node
        .provenance
        .iter()
        .copied()
        .map(|site| omega_optimization_unit::FuelSettlement { site, units: 1 })
        .collect();
    node.definitions = expected_definitions(&node.operation, block, 1);
    node.uses = expected_uses(&node.operation, block, 1);
    node.successors = expected_edges(&node.operation);
    node.ownership = expected_ownership(&node.operation);
    candidate.functions[0].blocks[0].nodes.insert(1, node);
    for index in 0..candidate.functions[0].blocks[0].nodes.len() {
        let operation = candidate.functions[0].blocks[0].nodes[index]
            .operation
            .clone();
        let node = &mut candidate.functions[0].blocks[0].nodes[index];
        node.effect.input = index as u64;
        node.effect.output = index as u64 + 1;
        node.provenance = expected_provenance(&operation);
        node.fuel = node
            .provenance
            .iter()
            .copied()
            .map(|site| omega_optimization_unit::FuelSettlement { site, units: 1 })
            .collect();
        node.definitions = expected_definitions(&operation, block, index as u32);
        node.uses = expected_uses(&operation, block, index as u32);
        node.successors = expected_edges(&operation);
        node.ownership = expected_ownership(&operation);
    }
    candidate.functions[0].facts = reconstruct_fact_index(&candidate.functions[0]);
    refresh_root_service_reach(&mut candidate).expect("PortWrite fixture has exact root reach");
    refresh_identity(&mut candidate);
    candidate
}

pub(crate) fn provider_service_unit() -> PsiOptimizationUnit {
    let mut candidate = provider_attachment_specialization_unit();
    install_service_catalog(&mut candidate);
    let boundary = candidate.boundary_machines[0].id;
    let requirement_identity = candidate.boundary_machines[0].identity.clone();
    let callee = candidate.functions[0].machine;
    let ceiling = service_declarations()
        .iter()
        .map(|service| service.id)
        .collect::<Vec<_>>();
    candidate
        .provider_candidates
        .push(psi_terminal::ProviderCandidateConformance {
            boundary,
            requirement_identity,
            provider_identity: "validation::service-provider".into(),
            candidate_identity: "validation::service-provider-candidate".into(),
            candidate: callee,
            signature: psi_terminal::ProviderUnitSignature {
                parameters: Vec::new(),
            },
            refinement: psi_terminal::ProviderUnitRefinement {
                positional_parameters: Vec::new(),
                required_domains: Vec::new(),
                realized_service_ceiling: ceiling,
            },
        });
    refresh_identity(&mut candidate);
    candidate
}

pub(crate) fn installation_root_service_unit() -> PsiOptimizationUnit {
    let mut candidate = scalar_boundary_call_unit();
    install_service_catalog(&mut candidate);
    let boundary = &candidate.boundary_machines[0];
    candidate.root_service_reach.installation_dependencies =
        vec![psi_terminal::InstallationReachDependency {
            requirement_identity: boundary.identity.clone(),
            upper_bound: boundary.published_service_ceiling.clone(),
        }];
    refresh_root_service_reach(&mut candidate)
        .expect("installation-bound fixture has exact root reach");
    refresh_identity(&mut candidate);
    candidate
}

pub(crate) fn multiple_installation_root_service_unit() -> PsiOptimizationUnit {
    let mut candidate = provider_attachment_specialization_unit();
    install_service_catalog(&mut candidate);
    candidate.root_service_reach.installation_dependencies = candidate.boundary_machines[..2]
        .iter()
        .map(|boundary| psi_terminal::InstallationReachDependency {
            requirement_identity: boundary.identity.clone(),
            upper_bound: boundary.published_service_ceiling.clone(),
        })
        .collect();
    candidate
        .root_service_reach
        .installation_dependencies
        .sort_by(|left, right| left.requirement_identity.cmp(&right.requirement_identity));
    refresh_root_service_reach(&mut candidate)
        .expect("multi-dependency fixture has exact root reach");
    refresh_identity(&mut candidate);
    candidate
}

pub(crate) fn provider_attachment_specialization_unit() -> PsiOptimizationUnit {
    let machine = id(440, MachineId::new);
    let block = id(441, BlockId::new);
    let attachment = id(444, StructuralTypeId::new);
    let provider_field = id(1, psi_core::StructuralFieldId::new);
    let first_boundary = id(446, BoundaryMachineId::new);
    let second_boundary = id(447, BoundaryMachineId::new);
    let unused_boundary = id(448, BoundaryMachineId::new);
    let boundary = |id, identity: &str| psi_terminal::BoundaryMachineDeclaration {
        id,
        identity: identity.into(),
        attachment: None,
        scalar_parameters: Vec::new(),
        structural_parameters: Vec::new(),
        result: None,
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        published_service_ceiling: Vec::new(),
    };
    let call = |psi_operation, boundary| AbstractOperation::BoundaryCall {
        psi_operation,
        result: None,
        boundary,
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        completion_claim_sources: Vec::new(),
        completion_receipts: Vec::new(),
    };
    let plan = AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([44; 32]),
        },
        entry: machine,
        structural_types: vec![structural_type(
            444,
            psi_terminal::StructuralTypeShape::Record {
                fields: vec![structural_leaf_field(
                    1,
                    psi_terminal::BindingRelevance::Relevant,
                    psi_terminal::StructuralFieldType::Erased {
                        type_identity: "validation::provider".into(),
                    },
                )],
            },
        )],
        boundary_machines: vec![
            boundary(first_boundary, "validation::provider-first"),
            boundary(second_boundary, "validation::provider-second"),
            boundary(unused_boundary, "validation::provider-unused"),
        ],
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine,
            attachment: Some(attachment),
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
                call(id(449, OperationId::new), first_boundary),
                call(id(450, OperationId::new), first_boundary),
                call(id(451, OperationId::new), second_boundary),
                AbstractOperation::ReturnUnit {
                    psi_edge: id(452, EdgeId::new),
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    };
    let mut unit = reconstruct_psi_optimization_unit_seed(
        &plan,
        FuelScheduleIdentity::new(1).expect("nonzero schedule"),
    )
    .expect("provider specialization fixture");
    unit.functions[0].structural_places.extend([
        psi_terminal::StructuralPlaceDeclaration {
            id: id(445, PlaceId::new),
            kind: StructuralPlaceKind::ProviderAttachment {
                attachment,
                field: provider_field,
                boundary: first_boundary,
            },
        },
        psi_terminal::StructuralPlaceDeclaration {
            id: id(446, PlaceId::new),
            kind: StructuralPlaceKind::ProviderAttachment {
                attachment,
                field: provider_field,
                boundary: second_boundary,
            },
        },
    ]);
    refresh_identity(&mut unit);
    unit
}

pub(crate) fn structural_domain(
    raw: u64,
    semantic_raw: u64,
    carrier: StructuralTypeId,
) -> psi_terminal::StructuralDomainDeclaration {
    psi_terminal::StructuralDomainDeclaration {
        id: id(raw, StructuralDomainId::new),
        semantic_domain: id(semantic_raw, psi_core::DomainSemanticId::new),
        identity: format!("validation::domain-{raw}"),
        carrier,
        content_projection: None,
    }
}

pub(crate) fn structural_result_call_unit() -> PsiOptimizationUnit {
    let caller = id(350, MachineId::new);
    let callee = id(351, MachineId::new);
    let caller_block = id(352, BlockId::new);
    let callee_block = id(353, BlockId::new);
    let structural_type = id(354, psi_core::StructuralTypeId::new);
    let callee_result = id(355, PlaceId::new);
    let call_result = id(356, PlaceId::new);
    let caller_result = id(362, PlaceId::new);
    let caller_input = id(360, PlaceId::new);
    let callee_input = id(361, PlaceId::new);
    let claim = id(1, ClaimId::new);
    let parameter = |place| psi_terminal::StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: psi_terminal::StructuralMultiplicity::Linear,
        access: psi_terminal::StructuralAccess::Owned,
        qualifications: Vec::new(),
    };
    let entry_claim = |input| psi_terminal::EntryClaim {
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
        structural_types: vec![psi_terminal::StructuralTypeDeclaration {
            id: structural_type,
            identity: "validation::structural-call-result".into(),
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
                entry: caller_block,
                parameters: Vec::new(),
                structural_parameters: vec![parameter(caller_input)],
                result: AbstractFunctionResult::Structural(
                    psi_terminal::StructuralResultDeclaration {
                        place: caller_result,
                        structural_type,
                        multiplicity: psi_terminal::StructuralMultiplicity::Linear,
                        qualifications: Vec::new(),
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
                        result: psi_terminal::StructuralOperationResult {
                            place: call_result,
                            structural_type,
                            multiplicity: psi_terminal::StructuralMultiplicity::Linear,
                            qualifications: Vec::new(),
                            claims: vec![psi_terminal::StructuralResultClaimBinding {
                                claim,
                                path: Vec::new(),
                            }],
                        },
                        callee,
                        structural_arguments: vec![psi_terminal::StructuralArgument {
                            place: caller_input,
                            path: Vec::new(),
                            access: psi_terminal::StructuralAccess::Owned,
                        }],
                        claim_transfers: vec![psi_terminal::ClaimTransfer {
                            claim,
                            argument_index: 0,
                        }],
                        returned_claim_transfers: vec![
                            psi_terminal::StructuralResultClaimTransfer {
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
                    psi_terminal::StructuralResultDeclaration {
                        place: callee_result,
                        structural_type,
                        multiplicity: psi_terminal::StructuralMultiplicity::Linear,
                        qualifications: Vec::new(),
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

pub(crate) fn compressed_trivial_affine_return_unit_with_prefix(
    executable_collision: bool,
    explicit_witnesses: bool,
) -> PsiOptimizationUnit {
    let machine = id(360, MachineId::new);
    let block = id(361, BlockId::new);
    let structural_type = id(362, StructuralTypeId::new);
    let source = id(363, PlaceId::new);
    let first_tail = id(364, PlaceId::new);
    let second_tail = id(365, PlaceId::new);
    let result = id(366, PlaceId::new);
    let first_local = id(367, PlaceId::new);
    let second_local = id(368, PlaceId::new);
    let claim = id(1, ClaimId::new);
    let local_type = psi_terminal::StructuralTypeDeclaration {
        id: structural_type,
        identity: "validation::trivial-affine-empty-record".into(),
        shape: psi_terminal::StructuralTypeShape::Record { fields: Vec::new() },
    };
    let parameter = |place, position, multiplicity| psi_terminal::StructuralParameterDeclaration {
        place,
        position,
        is_self: false,
        structural_type,
        multiplicity,
        access: psi_terminal::StructuralAccess::Owned,
        qualifications: Vec::new(),
    };
    let local = |place, declaration_ordinal| psi_terminal::StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::TrivialAffineLocal {
            declaration_ordinal,
            structural_type,
            construction: None,
        },
    };
    let first_declaration = local(first_local, 0);
    let second_declaration = local(second_local, 1);
    let mut operations = Vec::new();
    if executable_collision {
        operations.push(AbstractOperation::BooleanConstant {
            psi_operation: id(371, OperationId::new),
            result: id(389, ValueId::new),
            value: false,
        });
    }
    if explicit_witnesses {
        operations.extend([
            AbstractOperation::EstablishTrivialAffineLocal {
                psi_operation: id(373, OperationId::new),
                place: first_declaration,
                structural_type: local_type.clone(),
            },
            AbstractOperation::EstablishTrivialAffineLocal {
                psi_operation: id(374, OperationId::new),
                place: second_declaration,
                structural_type: local_type.clone(),
            },
        ]);
    }
    operations.push(AbstractOperation::ReturnStructural {
        psi_edge: id(370, EdgeId::new),
        source,
        returned_claims: vec![claim],
        trivial_affine_locals: vec![
            (
                id(371, OperationId::new),
                first_declaration,
                local_type.clone(),
            ),
            (
                id(372, OperationId::new),
                second_declaration,
                local_type.clone(),
            ),
        ],
        trivial_affine_discards: vec![second_local, first_local, second_tail, first_tail],
    });
    let plan = AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([18; 32]),
        },
        entry: machine,
        structural_types: vec![local_type.clone()],
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine,
            attachment: None,
            entry: block,
            parameters: Vec::new(),
            structural_parameters: vec![
                parameter(source, 0, psi_terminal::StructuralMultiplicity::Linear),
                parameter(first_tail, 1, psi_terminal::StructuralMultiplicity::Affine),
                parameter(second_tail, 2, psi_terminal::StructuralMultiplicity::Affine),
            ],
            result: AbstractFunctionResult::Structural(psi_terminal::StructuralResultDeclaration {
                place: result,
                structural_type,
                multiplicity: psi_terminal::StructuralMultiplicity::Linear,
                qualifications: Vec::new(),
            }),
            entry_claims: vec![psi_terminal::EntryClaim {
                claim,
                input: source,
                path: Vec::new(),
            }],
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                block,
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations,
        }],
    };
    let mut unit = reconstruct_psi_optimization_unit_seed(
        &plan,
        FuelScheduleIdentity::new(1).expect("nonzero schedule"),
    )
    .expect("compressed structural return unit");
    for declaration in [first_declaration, second_declaration] {
        if !unit.functions[0]
            .structural_places
            .iter()
            .any(|place| place.id == declaration.id)
        {
            unit.functions[0].structural_places.push(declaration);
        }
    }
    refresh_identity(&mut unit);
    unit
}

pub(crate) fn compressed_trivial_affine_return_unit() -> PsiOptimizationUnit {
    compressed_trivial_affine_return_unit_with_prefix(false, false)
}

pub(crate) fn explicit_trivial_affine_return_unit() -> PsiOptimizationUnit {
    let machine = id(390, MachineId::new);
    let block = id(391, BlockId::new);
    let structural_type = id(392, StructuralTypeId::new);
    let place = id(393, PlaceId::new);
    let structural_type_declaration = psi_terminal::StructuralTypeDeclaration {
        id: structural_type,
        identity: "validation::explicit-trivial-affine-empty-record".into(),
        shape: psi_terminal::StructuralTypeShape::Record { fields: Vec::new() },
    };
    let place_declaration = psi_terminal::StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::TrivialAffineLocal {
            declaration_ordinal: 0,
            structural_type,
            construction: None,
        },
    };
    reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([19; 32]),
            },
            entry: machine,
            structural_types: vec![structural_type_declaration.clone()],
            boundary_machines: Vec::new(),
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
                    AbstractOperation::EstablishTrivialAffineLocal {
                        psi_operation: id(394, OperationId::new),
                        place: place_declaration,
                        structural_type: structural_type_declaration,
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: id(395, EdgeId::new),
                        cleanup_actions: vec![
                            psi_terminal::TerminalAffineCleanupAction::DiscardRoot(place),
                        ],
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).expect("nonzero schedule"),
    )
    .expect("explicit affine local unit")
}
