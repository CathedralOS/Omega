//! Affine claim, place, and join fixture units.

use super::*;

pub(crate) fn affine_claim_transfer_unit() -> PsiOptimizationUnit {
    let mut unit = structural_call_unit();
    let claim = id(1, ClaimId::new);
    for function in &mut unit.functions {
        function.structural_parameters[0].multiplicity =
            terminal_psi::StructuralMultiplicity::Affine;
        function
            .entry_claim_declarations
            .push(terminal_psi::EntryClaim {
                claim,
                input: function.structural_parameters[0].place,
                path: Vec::new(),
            });
        function.entry_claims.insert(claim);
    }
    let AbstractOperation::CallUnit {
        claim_transfers, ..
    } = &mut unit.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture begins with a structural Unit call")
    };
    claim_transfers.push(terminal_psi::ClaimTransfer {
        claim,
        argument_index: 0,
    });
    refresh_node_derivatives(&mut unit, 0, 0, 0);
    unit
}

pub(crate) fn affine_place_transfer_unit() -> PsiOptimizationUnit {
    let mut unit = structural_call_unit();
    for function in &mut unit.functions {
        function.structural_parameters[0].multiplicity =
            terminal_psi::StructuralMultiplicity::Affine;
    }
    let callee_place = unit.functions[1].structural_parameters[0].place;
    let AbstractOperation::ReturnUnit {
        cleanup_actions, ..
    } = &mut unit.functions[1].blocks[0].nodes[0].operation
    else {
        panic!("callee fixture returns Unit")
    };
    cleanup_actions.push(terminal_psi::TerminalAffineCleanupAction::DiscardRoot(
        callee_place,
    ));
    refresh_node_derivatives(&mut unit, 1, 0, 0);
    refresh_identity(&mut unit);
    unit
}

pub(crate) fn partial_affine_place_unit() -> PsiOptimizationUnit {
    let caller = id(4_850, MachineId::new);
    let callee = id(4_851, MachineId::new);
    let caller_block = id(4_852, BlockId::new);
    let callee_block = id(4_853, BlockId::new);
    let left = id(4_854, StructuralTypeId::new);
    let right = id(4_855, StructuralTypeId::new);
    let pair = id(4_856, StructuralTypeId::new);
    let caller_place = id(4_857, PlaceId::new);
    let callee_place = id(4_858, PlaceId::new);
    let empty_record =
        |id: StructuralTypeId, identity: &str| terminal_psi::StructuralTypeDeclaration {
            id,
            identity: identity.into(),
            shape: terminal_psi::StructuralTypeShape::Record { fields: Vec::new() },
        };
    let parameter = |place, structural_type| terminal_psi::StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: terminal_psi::StructuralMultiplicity::Affine,
        access: terminal_psi::StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let plan = AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([49; 32]),
        },
        entry: caller,
        structural_types: vec![
            empty_record(left, "validation::partial-left"),
            empty_record(right, "validation::partial-right"),
            terminal_psi::StructuralTypeDeclaration {
                id: pair,
                identity: "validation::partial-pair".into(),
                shape: terminal_psi::StructuralTypeShape::Record {
                    fields: vec![
                        terminal_psi::StructuralFieldDeclaration {
                            id: id(1, semantic_vocabulary::StructuralFieldId::new),
                            identity: "left".into(),
                            relevance: terminal_psi::BindingRelevance::Relevant,
                            field_type: terminal_psi::StructuralFieldType::Structural(left),
                        },
                        terminal_psi::StructuralFieldDeclaration {
                            id: id(2, semantic_vocabulary::StructuralFieldId::new),
                            identity: "right".into(),
                            relevance: terminal_psi::BindingRelevance::Relevant,
                            field_type: terminal_psi::StructuralFieldType::Structural(right),
                        },
                    ],
                },
            },
        ],
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![
            AbstractFunction {
                machine: caller,
                attachment: None,
                entry: caller_block,
                parameters: Vec::new(),
                structural_parameters: vec![parameter(caller_place, pair)],
                result: AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block: caller_block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    AbstractOperation::CallUnit {
                        psi_operation: id(4_859, OperationId::new),
                        callee,
                        arguments: Vec::new(),
                        structural_arguments: vec![terminal_psi::StructuralArgument {
                            place: caller_place,
                            path: vec![terminal_psi::StructuralPathSegment::Field("right".into())],
                            access: terminal_psi::StructuralAccess::Owned,
                        }],
                        claim_transfers: Vec::new(),
                        requirement_obligations: Vec::new(),
                        crash_continuations: Vec::new(),
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: id(4_860, EdgeId::new),
                        cleanup_actions: vec![
                            terminal_psi::TerminalAffineCleanupAction::DiscardResidual(
                                terminal_psi::StructuralAffineDiscard {
                                    place: caller_place,
                                    path: vec![terminal_psi::StructuralPathSegment::Field(
                                        "left".into(),
                                    )],
                                    structural_type: left,
                                },
                            ),
                        ],
                    },
                ],
            },
            AbstractFunction {
                machine: callee,
                attachment: None,
                entry: callee_block,
                parameters: Vec::new(),
                structural_parameters: vec![parameter(callee_place, right)],
                result: AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block: callee_block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![AbstractOperation::ReturnUnit {
                    psi_edge: id(4_861, EdgeId::new),
                    cleanup_actions: vec![terminal_psi::TerminalAffineCleanupAction::DiscardRoot(
                        callee_place,
                    )],
                }],
            },
        ],
    };
    reconstruct_psi_optimization_unit_seed(&plan, FuelScheduleIdentity::new(1).unwrap()).unwrap()
}

pub(crate) fn partial_affine_quartet_unit() -> PsiOptimizationUnit {
    let caller = id(4_870, MachineId::new);
    let callee = id(4_871, MachineId::new);
    let caller_block = id(4_872, BlockId::new);
    let callee_block = id(4_873, BlockId::new);
    let token = id(4_874, StructuralTypeId::new);
    let quartet = id(4_875, StructuralTypeId::new);
    let caller_place = id(4_876, PlaceId::new);
    let callee_place = id(4_877, PlaceId::new);
    let parameter = |place, structural_type| terminal_psi::StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: terminal_psi::StructuralMultiplicity::Affine,
        access: terminal_psi::StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let projected_call = |psi_operation, index| AbstractOperation::CallUnit {
        psi_operation,
        callee,
        arguments: Vec::new(),
        structural_arguments: vec![terminal_psi::StructuralArgument {
            place: caller_place,
            path: vec![terminal_psi::StructuralPathSegment::FixedIndex(index)],
            access: terminal_psi::StructuralAccess::Owned,
        }],
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    let residual = |index| {
        terminal_psi::TerminalAffineCleanupAction::DiscardResidual(
            terminal_psi::StructuralAffineDiscard {
                place: caller_place,
                path: vec![terminal_psi::StructuralPathSegment::FixedIndex(index)],
                structural_type: token,
            },
        )
    };
    let plan = AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([50; 32]),
        },
        entry: caller,
        structural_types: vec![
            terminal_psi::StructuralTypeDeclaration {
                id: token,
                identity: "validation::quartet-token".into(),
                shape: terminal_psi::StructuralTypeShape::Record { fields: Vec::new() },
            },
            terminal_psi::StructuralTypeDeclaration {
                id: quartet,
                identity: "validation::affine-quartet".into(),
                shape: terminal_psi::StructuralTypeShape::FixedArray {
                    element: token,
                    length: 4,
                },
            },
        ],
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![
            AbstractFunction {
                machine: caller,
                attachment: None,
                entry: caller_block,
                parameters: Vec::new(),
                structural_parameters: vec![parameter(caller_place, quartet)],
                result: AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block: caller_block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    projected_call(id(4_878, OperationId::new), 1),
                    projected_call(id(4_879, OperationId::new), 3),
                    AbstractOperation::ReturnUnit {
                        psi_edge: id(4_880, EdgeId::new),
                        cleanup_actions: vec![residual(2), residual(0)],
                    },
                ],
            },
            AbstractFunction {
                machine: callee,
                attachment: None,
                entry: callee_block,
                parameters: Vec::new(),
                structural_parameters: vec![parameter(callee_place, token)],
                result: AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block: callee_block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![AbstractOperation::ReturnUnit {
                    psi_edge: id(4_881, EdgeId::new),
                    cleanup_actions: vec![terminal_psi::TerminalAffineCleanupAction::DiscardRoot(
                        callee_place,
                    )],
                }],
            },
        ],
    };
    reconstruct_psi_optimization_unit_seed(&plan, FuelScheduleIdentity::new(1).unwrap()).unwrap()
}

pub(crate) fn affine_place_join_unit(settle_false_arm: bool) -> PsiOptimizationUnit {
    let mut unit = affine_claim_join_unit(settle_false_arm);
    let function = &mut unit.functions[0];
    function.entry_claim_declarations.clear();
    function.entry_claims.clear();
    for node in function
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.nodes)
    {
        if let AbstractOperation::BoundaryCall {
            completion_claim_sources,
            completion_receipts,
            ..
        } = &mut node.operation
        {
            completion_claim_sources.clear();
            completion_receipts.clear();
        }
    }
    refresh_function_derivatives(&mut unit, 0);
    unit
}

pub(crate) fn affine_claim_join_unit(settle_false_arm: bool) -> PsiOptimizationUnit {
    let machine = id(4_800, MachineId::new);
    let entry_block = id(4_801, BlockId::new);
    let true_block = id(4_802, BlockId::new);
    let false_block = id(4_803, BlockId::new);
    let join_block = id(4_804, BlockId::new);
    let boundary = id(4_805, BoundaryMachineId::new);
    let structural_type = id(4_806, StructuralTypeId::new);
    let root = id(4_807, PlaceId::new);
    let boundary_root = id(4_808, PlaceId::new);
    let condition = id(4_809, ValueId::new);
    let claim = id(1, ClaimId::new);
    let parameter = |place| terminal_psi::StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: terminal_psi::StructuralMultiplicity::Affine,
        access: terminal_psi::StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let entry_claim = terminal_psi::EntryClaim {
        claim,
        input: root,
        path: Vec::new(),
    };
    let completion = |psi_operation| AbstractOperation::BoundaryCall {
        psi_operation,
        result: abstract_operations::AbstractBoundaryResult::Unit,
        boundary,
        arguments: Vec::new(),
        structural_arguments: vec![terminal_psi::StructuralArgument {
            place: root,
            path: Vec::new(),
            access: terminal_psi::StructuralAccess::Owned,
        }],
        completion_claim_sources: vec![abstract_operations::CompletionClaimSource {
            claim,
            entry: Some(entry_claim.clone()),
            content: None,
        }],
        completion_receipts: vec![terminal_psi::CompletionReceipt {
            claim,
            argument_index: 0,
        }],
    };
    let mut operations = vec![
        AbstractOperation::BooleanConstant {
            psi_operation: id(4_810, OperationId::new),
            result: condition,
            value: true,
        },
        AbstractOperation::Conditional {
            condition,
            when_true: abstract_operations::AbstractSuccessor {
                psi_edge: id(4_811, EdgeId::new),
                target: true_block,
                bindings: Vec::new(),
                trivial_affine_discards: Vec::new(),
            },
            when_false: abstract_operations::AbstractSuccessor {
                psi_edge: id(4_812, EdgeId::new),
                target: false_block,
                bindings: Vec::new(),
                trivial_affine_discards: Vec::new(),
            },
        },
    ];
    let true_offset = operations.len();
    operations.extend([
        completion(id(4_813, OperationId::new)),
        AbstractOperation::Jump {
            psi_edge: id(4_814, EdgeId::new),
            target: join_block,
            bindings: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    ]);
    let false_offset = operations.len();
    if settle_false_arm {
        operations.push(completion(id(4_815, OperationId::new)));
    }
    operations.push(AbstractOperation::Jump {
        psi_edge: id(4_816, EdgeId::new),
        target: join_block,
        bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
    });
    let join_offset = operations.len();
    operations.push(AbstractOperation::ReturnUnit {
        psi_edge: id(4_817, EdgeId::new),
        cleanup_actions: Vec::new(),
    });
    let plan = AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([48; 32]),
        },
        entry: machine,
        structural_types: vec![terminal_psi::StructuralTypeDeclaration {
            id: structural_type,
            identity: "validation::affine-claim-join".into(),
            shape: terminal_psi::StructuralTypeShape::Record { fields: Vec::new() },
        }],
        boundary_machines: vec![terminal_psi::BoundaryMachineDeclaration {
            id: boundary,
            identity: "validation::affine-claim-settlement".into(),
            attachment: None,
            scalar_parameters: Vec::new(),
            structural_parameters: vec![parameter(boundary_root)],
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
            entry: entry_block,
            parameters: Vec::new(),
            structural_parameters: vec![parameter(root)],
            result: AbstractFunctionResult::Unit,
            entry_claims: vec![entry_claim],
            published_service_ceiling: Vec::new(),
            block_entries: vec![
                AbstractBlockEntry {
                    block: entry_block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                },
                AbstractBlockEntry {
                    block: true_block,
                    parameters: Vec::new(),
                    operation_offset: true_offset,
                },
                AbstractBlockEntry {
                    block: false_block,
                    parameters: Vec::new(),
                    operation_offset: false_offset,
                },
                AbstractBlockEntry {
                    block: join_block,
                    parameters: Vec::new(),
                    operation_offset: join_offset,
                },
            ],
            operations,
        }],
    };
    reconstruct_psi_optimization_unit_seed(&plan, FuelScheduleIdentity::new(1).unwrap()).unwrap()
}
