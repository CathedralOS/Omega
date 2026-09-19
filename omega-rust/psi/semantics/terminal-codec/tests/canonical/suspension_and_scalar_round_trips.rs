use super::{
    call_fixture, five_nominal_affine_fixture, fixture, i32_type, nominal_affine_fixture,
    partial_affine_fixture, proof_recursive_component_fixture, ranked_countdown_fixture,
    structural_effect_fixture, two_nominal_affine_fixture, unit_fixture,
};
use crate::canonical::{
    block_id, claim_id, contract_id, edge_id, machine_id, obligation_id, operation_id, place_id,
    service_id, structural_case_id, structural_domain_id, structural_field_id, structural_type_id,
    suspension_crossing_id, value_id,
};
use language_semantics::CarryPolicy;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, ContentAlgebra, ContentAlgebraKind, ContentDomainId,
    ContentProjectionExpression, ContentProjectionIdentity, ContentProjectionScalar,
    IeeeFloatFormat, IeeeFloatStructuralField, IeeeFloatValue, IntegerSign, IntegerType,
    Proposition, ScalarTerm, ScalarType, StructuralFieldId, StructuralPlaceKind,
};
use terminal_codec::{
    CodecError, decode_module, encode_module, semantic_fingerprint, terminal_psi_identity,
};
use terminal_psi::{
    BindingRelevance, Block, DirectBlockFloatParameter, DirectCallFloatResult,
    DirectMachineFloatParameter, DirectMachineFloatResult, DirectOperationFloatResult,
    DirectStructuralFloatLeaf, EntryClaim, FloatMeaningEqualityProposition, FloatMeaningProjection,
    FloatMeaningProjectionOperation, FloatMeaningSource, InstallationReachDependency,
    MachineContract, Operation, OperationKind, OperationResult, ProofOnlyValueType,
    ProofPropositionId, ProofValueDeclaration, ProofValueId, ServiceDeclaration, StructuralAccess,
    StructuralCaseDeclaration, StructuralContentProjection, StructuralDomainDeclaration,
    StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalAffineCleanupAction, TerminalMachine, TerminalMachineResult,
    TerminalPlacedViewInput, TerminalRankedScc, TerminalSuspensionCallPlan,
    TerminalSuspensionCallSite, TerminalSuspensionCallTarget, TerminalSuspensionLiveValue,
    TerminalSuspensionPlace, TerminalSuspensionStorage, TerminalSuspensionValueType, Terminator,
    ValueDeclaration, VocabularyMarker,
};

#[test]
fn suspension_call_plan_round_trips_canonically_and_rejects_prior_format() {
    let mut module = call_fixture();
    let live = |storage| TerminalSuspensionLiveValue {
        place: TerminalSuspensionPlace::Scalar(value_id(100)),
        value_type: TerminalSuspensionValueType::Scalar(ScalarType::Boolean),
        storage,
        claim_count: 0,
        claims: Vec::new(),
        effective: CarryPolicy::PERMISSIVE,
    };
    let plan = TerminalSuspensionCallPlan {
        operation: operation_id(101),
        crossing: suspension_crossing_id(1),
        target: TerminalSuspensionCallTarget::Machine(machine_id(101)),
        effective: CarryPolicy::PERMISSIVE,
        live_value_count: 2,
        live_values: vec![
            live(TerminalSuspensionStorage::Local),
            live(TerminalSuspensionStorage::CallArgument),
        ],
    };
    module.suspension_call_plan_count = 1;
    module.suspension_call_sites = vec![TerminalSuspensionCallSite {
        operation: plan.operation,
        crossing: plan.crossing,
        target: plan.target,
        frontier_commitment: terminal_psi::suspension_frontier_commitment(&plan),
    }];
    module.suspension_call_plans = vec![plan];

    let bytes = encode_module(&module).expect("suspension plan encodes");
    assert_eq!(&bytes[8..10], 103_u16.to_le_bytes());
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(
        encode_module(&decode_module(&bytes).unwrap()),
        Ok(bytes.clone())
    );

    let mut stale = bytes;
    stale[8..10].copy_from_slice(&73_u16.to_le_bytes());
    assert_eq!(
        decode_module(&stale),
        Err(CodecError::UnsupportedFormatMarker(73))
    );

    let mut noncanonical = module;
    noncanonical.suspension_call_plans[0].live_values.reverse();
    assert_eq!(
        encode_module(&noncanonical),
        Err(CodecError::NonCanonicalOrder(
            "suspension live values by place, storage, type, and policy"
        ))
    );
}

#[test]
fn current_vocabulary_has_one_stable_canonical_encoding_and_identity() {
    let module = fixture();
    let bytes = encode_module(&module).expect("fixture should encode");

    assert_eq!(&bytes[..8], b"PSITERM\0");
    assert_eq!(&bytes[8..10], 103_u16.to_le_bytes());
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));

    let identity = terminal_psi_identity(&module).expect("fixture should have an identity");
    assert_eq!(identity.vocabulary_marker, VocabularyMarker::CURRENT);
    assert_eq!(
        identity.program_fingerprint.to_string(),
        "57f3de0eb46959f2636feae425dc04d1196b5baa209ad9949158c20469d14485"
    );
    assert_eq!(
        identity.program_fingerprint,
        semantic_fingerprint(&module).unwrap()
    );
}

#[test]
fn proof_recursive_components_round_trip_and_enter_terminal_identity() {
    let mut module = unit_fixture();
    module.proof_recursive_components = vec![proof_recursive_component_fixture()];
    let bytes = encode_module(&module).expect("proof-recursive module should encode");
    assert_eq!(&bytes[8..10], 103_u16.to_le_bytes());
    assert_eq!(decode_module(&bytes), Ok(module.clone()));

    let original = semantic_fingerprint(&module).expect("recursive semantic identity");
    let mut changed_path = module.clone();
    changed_path.proof_recursive_components[0].edges[0].strict_member_path =
        vec!["package::Node::right".into()];
    assert_ne!(
        semantic_fingerprint(&changed_path).expect("changed recursive semantic identity"),
        original,
    );

    let mut reordered = module;
    reordered.proof_recursive_components[0].edges.reverse();
    assert!(encode_module(&reordered).is_err());
}

#[test]
fn ieee_float_constants_and_nearest_fma_round_trip_exact_interchange_bits() {
    let mut module = unit_fixture();
    let binary32 = ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(901),
        scalar_type: ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
    };
    let binary64 = ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(902),
        scalar_type: ScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
    };
    module.machines[0].blocks[0].operations = vec![
        Operation {
            static_reach_binding: None,
            id: operation_id(901),
            result: OperationResult::Scalar(binary32),
            kind: OperationKind::IeeeFloatConstant {
                value: IeeeFloatValue::Binary32(0x8000_0000),
            },
        },
        Operation {
            static_reach_binding: None,
            id: operation_id(902),
            result: OperationResult::Scalar(binary64),
            kind: OperationKind::IeeeFloatConstant {
                value: IeeeFloatValue::Binary64(0x7ff8_0000_0000_0042),
            },
        },
        Operation {
            static_reach_binding: None,
            id: operation_id(903),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: value_id(903),
                scalar_type: ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
            }),
            kind: OperationKind::NearestIeeeFloatFusedMultiplyAdd {
                left: binary32.id,
                right: binary32.id,
                addend: binary32.id,
            },
        },
    ];

    let bytes = encode_module(&module).expect("IEEE scalar module encodes");
    let decoded = decode_module(&bytes).expect("IEEE scalar module decodes");
    assert_eq!(decoded, module);
    assert_eq!(encode_module(&decoded).unwrap(), bytes);
}

#[test]
fn placed_view_input_round_trips_with_exact_semantic_identity() {
    let mut module = unit_fixture();
    let identity = |name: &str| format!("package:{}::{name}", "01".repeat(32));
    let policy_identity = identity("Uart");
    let schema_identity = identity("Registers");
    let input = TerminalPlacedViewInput {
        machine: module.entry,
        position: 0,
        source_machine_identity: identity("inspect"),
        source_state_identity: identity("inspect::entry"),
        source_parameter_identity: identity("inspect::entry::view"),
        access: StructuralAccess::MutableBorrow,
        binding_is_const: false,
        binding_is_mutable: true,
        view_identity: terminal_psi::canonical_placed_view_identity(
            &policy_identity,
            &schema_identity,
        ),
        policy_identity,
        policy_plan_machine_identity: identity("Uart::plan"),
        schema_identity,
        placement_report_fingerprint: 41,
        placement_commitment: [0x5a; 32],
    };
    module.placed_view_inputs.push(input.clone());
    module.placed_view_inputs.push(TerminalPlacedViewInput {
        position: 0,
        source_state_identity: identity("inspect::next"),
        source_parameter_identity: identity("inspect::next::view"),
        ..input
    });

    let bytes = encode_module(&module).expect("placed-view input should encode");
    assert_eq!(decode_module(&bytes), Ok(module));
}

#[test]
fn ranked_countdown_round_trips_in_current_terminal_identity() {
    let module = ranked_countdown_fixture();
    let bytes = encode_module(&module).expect("ranked representation should encode");
    assert_eq!(&bytes[8..10], 103_u16.to_le_bytes());
    assert_eq!(
        &bytes[10..12],
        VocabularyMarker::CURRENT.get().to_le_bytes()
    );
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(
        encode_module(&decode_module(&bytes).unwrap()),
        Ok(bytes.clone())
    );

    let mut stale_format = bytes;
    stale_format[8..10].copy_from_slice(&33_u16.to_le_bytes());
    assert_eq!(
        decode_module(&stale_format),
        Err(CodecError::UnsupportedFormatMarker(33))
    );
}

#[test]
fn natural_ranking_round_trips_exact_semantic_rows_and_rejects_malformed_coverage() {
    let mut module = ranked_countdown_fixture();
    let mut unranked = module.clone();
    unranked.machines[0].ranked_scc = None;
    let unranked_identity = semantic_fingerprint(&unranked).unwrap();
    let rank_type = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
    let cycle = terminal_psi::TerminalNaturalCycle {
        rank_type,
        ranks: vec![
            terminal_psi::TerminalBlockNaturalRank {
                block: block_id(901),
                value: value_id(902),
            },
            terminal_psi::TerminalBlockNaturalRank {
                block: block_id(902),
                value: value_id(902),
            },
        ],
        edges: vec![
            terminal_psi::TerminalNaturalRankEdge {
                edge: edge_id(901),
                source: block_id(901),
                target: block_id(902),
                successor_rank: value_id(902),
                comparison: terminal_psi::TerminalNaturalRankComparison::Preserving,
            },
            terminal_psi::TerminalNaturalRankEdge {
                edge: edge_id(903),
                source: block_id(902),
                target: block_id(901),
                successor_rank: value_id(906),
                comparison: terminal_psi::TerminalNaturalRankComparison::Strict,
            },
        ],
    };
    module.machines[0].ranked_scc = Some(TerminalRankedScc::Natural(vec![cycle.clone()]));
    let bytes = encode_module(&module).expect("natural ranking representation encodes");
    assert_eq!(&bytes[8..12], &[103, 0, 107, 0]);
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_ne!(semantic_fingerprint(&module).unwrap(), unranked_identity);
    let mut stale = bytes;
    stale[8..10].copy_from_slice(&78_u16.to_le_bytes());
    assert_eq!(
        decode_module(&stale),
        Err(CodecError::UnsupportedFormatMarker(78))
    );

    let mut reordered_ranks = cycle.clone();
    reordered_ranks.ranks.reverse();
    let mut duplicate_rank = cycle.clone();
    duplicate_rank.ranks[1].block = duplicate_rank.ranks[0].block;
    let mut reordered_edges = cycle.clone();
    reordered_edges.edges.reverse();
    let mut missing_edge = cycle.clone();
    missing_edge.edges.pop();
    let mut redirected_edge = cycle.clone();
    redirected_edge.edges[1].target = block_id(903);
    let mut missing_rank_value = cycle.clone();
    missing_rank_value.ranks[0].value = value_id(999);
    let mut wrong_successor_rank = cycle.clone();
    wrong_successor_rank.edges[1].successor_rank = value_id(905);
    for malformed in [
        reordered_ranks,
        duplicate_rank,
        reordered_edges,
        missing_edge,
        redirected_edge,
        missing_rank_value,
        wrong_successor_rank,
    ] {
        let mut changed = module.clone();
        changed.machines[0].ranked_scc = Some(TerminalRankedScc::Natural(vec![malformed]));
        assert!(encode_module(&changed).is_err());
    }
    for components in [Vec::new(), vec![cycle.clone(), cycle]] {
        let mut changed = module.clone();
        changed.machines[0].ranked_scc = Some(TerminalRankedScc::Natural(components));
        assert!(encode_module(&changed).is_err());
    }
}

#[test]
fn structural_domain_owner_content_projection_round_trips_and_enters_identity() {
    let mut module = nominal_affine_fixture();
    let algebra = ContentAlgebra {
        kind: ContentAlgebraKind::CountedQuantity,
        parameter: "ByteUnit".into(),
    };
    let expression =
        ContentProjectionExpression::CountedQuantity(ContentProjectionScalar::Natural("4".into()));
    let projection_report_fingerprint =
        language_semantics::content::terminal_projection_report_fingerprint(&algebra, &expression);
    module.structural_domains.push(StructuralDomainDeclaration {
        id: structural_domain_id(1),
        semantic_domain: semantic_vocabulary::DomainSemanticId::new(1).unwrap(),
        identity: "example::NominalResource::Content".into(),
        carrier: structural_type_id(1),
        content_projection: Some(StructuralContentProjection {
            identity: ContentProjectionIdentity {
                domain: ContentDomainId::new(1).unwrap(),
                projection_report_fingerprint,
            },
            algebra,
            expression,
        }),
    });

    let bytes = encode_module(&module).expect("owner content projection encodes");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    let original_identity = terminal_psi_identity(&module).expect("owner projection identity");

    let mut changed = module.clone();
    let owner = changed.structural_domains[0]
        .content_projection
        .as_mut()
        .expect("owner projection");
    owner.expression =
        ContentProjectionExpression::CountedQuantity(ContentProjectionScalar::Natural("5".into()));
    owner.identity.projection_report_fingerprint =
        language_semantics::content::terminal_projection_report_fingerprint(
            &owner.algebra,
            &owner.expression,
        );
    assert_ne!(
        terminal_psi_identity(&changed).expect("changed owner projection identity"),
        original_identity
    );

    let mut malformed = module;
    malformed.structural_domains[0]
        .content_projection
        .as_mut()
        .expect("owner projection")
        .expression =
        ContentProjectionExpression::CountedQuantity(ContentProjectionScalar::Natural("6".into()));
    assert!(encode_module(&malformed).is_err());
}

#[test]
fn installation_reach_dependencies_round_trip_and_require_canonical_identity() {
    let mut module = structural_effect_fixture();
    module.services[0].identity = "MachineControl".into();
    module.services.push(ServiceDeclaration {
        id: service_id(2),
        identity: "PortIo".into(),
        parents: Vec::new(),
    });
    module.boundary_machines[0].identity = "InterruptCompletion::complete".into();
    module.boundary_machines[0].published_service_ceiling = vec![service_id(1), service_id(2)];
    for machine in &mut module.machines {
        machine.published_service_ceiling = vec![service_id(1), service_id(2)];
    }
    module.root_service_reach.installation_dependencies = vec![InstallationReachDependency {
        requirement_identity: "InterruptCompletion::complete".into(),
        upper_bound: vec![service_id(1), service_id(2)],
    }];

    let bytes = encode_module(&module).expect("installation reach dependency encodes");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));

    module.root_service_reach.installation_dependencies[0]
        .upper_bound
        .reverse();
    assert_eq!(
        encode_module(&module),
        Err(CodecError::NonCanonicalOrder(
            "installation reach upper bounds by ServiceId"
        ))
    );
}

#[test]
fn proof_only_float_projections_round_trip_and_reject_tampering() {
    let contract = |operation: numerics::float_projection::FloatProjectionOperation| {
        let contract = operation.contract_identity();
        terminal_psi::FloatProjectionContractIdentity {
            format: contract.format,
            operation: contract.operation,
            declaration: contract.declaration,
            catalog_version: contract.catalog_version,
            commitment: contract.commitment,
        }
    };
    let mut module = fixture();
    let direct_parameter = ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(6),
        scalar_type: ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
    };
    module.machines[0].parameters.push(direct_parameter);
    let direct_result_owner = machine_id(2);
    let direct_result = ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(8),
        scalar_type: ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
    };
    let direct_operation_result = ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(9),
        scalar_type: ScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
    };
    let direct_block_parameter = ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(20),
        scalar_type: ScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
    };
    let direct_operation = operation_id(9);
    module.machines.push(TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: direct_result_owner,
        attachment: None,
        parameters: vec![ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(7),
            scalar_type: ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
        }],
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(direct_result),
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(3),
        blocks: vec![
            Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: block_id(3),
                parameters: Vec::new(),
                operations: vec![Operation {
                    static_reach_binding: None,
                    id: direct_operation,
                    result: OperationResult::Scalar(direct_operation_result),
                    kind: OperationKind::IeeeFloatConstant {
                        value: IeeeFloatValue::Binary64(0x3ff0_0000_0000_0000),
                    },
                }],
                terminator: Terminator::Jump {
                    structural_arguments: Vec::new(),
                    edge: edge_id(3),
                    target: block_id(4),
                    arguments: vec![direct_operation_result.id],
                    erased_arguments: Vec::new(),
                    residual_affine_discards: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
            },
            Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: block_id(4),
                parameters: vec![direct_block_parameter],
                operations: Vec::new(),
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: edge_id(4),
                    value: value_id(7),
                },
            },
        ],
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
            id: contract_id(2),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    });
    module.float_meaning_projections = vec![
        FloatMeaningProjection {
            result: ProofValueDeclaration {
                id: ProofValueId(0),
                value_type: ProofOnlyValueType::FloatMeaning,
            },
            source: FloatMeaningSource::ExactBinary32Literal(0x1234_5678),
            operation: FloatMeaningProjectionOperation::Meaning32,
            contract: contract(numerics::float_projection::FloatProjectionOperation::Meaning32),
        },
        FloatMeaningProjection {
            result: ProofValueDeclaration {
                id: ProofValueId(1),
                value_type: ProofOnlyValueType::FloatMeaning,
            },
            source: FloatMeaningSource::ExactBinary32Literal(0x8765_4321),
            operation: FloatMeaningProjectionOperation::Meaning32,
            contract: contract(numerics::float_projection::FloatProjectionOperation::Meaning32),
        },
        FloatMeaningProjection {
            result: ProofValueDeclaration {
                id: ProofValueId(2),
                value_type: ProofOnlyValueType::FloatMeaning,
            },
            source: FloatMeaningSource::ExactBinary64Literal(0x8877_6655_4433_2211),
            operation: FloatMeaningProjectionOperation::Meaning64,
            contract: contract(numerics::float_projection::FloatProjectionOperation::Meaning64),
        },
        FloatMeaningProjection {
            result: ProofValueDeclaration {
                id: ProofValueId(3),
                value_type: ProofOnlyValueType::FloatMeaning,
            },
            source: FloatMeaningSource::DirectMachineParameter(DirectMachineFloatParameter {
                owner: module.entry,
                parameter: direct_parameter.id,
                format: IeeeFloatFormat::Binary32,
            }),
            operation: FloatMeaningProjectionOperation::Meaning32,
            contract: contract(numerics::float_projection::FloatProjectionOperation::Meaning32),
        },
        FloatMeaningProjection {
            result: ProofValueDeclaration {
                id: ProofValueId(4),
                value_type: ProofOnlyValueType::FloatMeaning,
            },
            source: FloatMeaningSource::DirectMachineResult(DirectMachineFloatResult {
                owner: direct_result_owner,
                result: direct_result.id,
                format: IeeeFloatFormat::Binary32,
            }),
            operation: FloatMeaningProjectionOperation::Meaning32,
            contract: contract(numerics::float_projection::FloatProjectionOperation::Meaning32),
        },
        FloatMeaningProjection {
            result: ProofValueDeclaration {
                id: ProofValueId(5),
                value_type: ProofOnlyValueType::FloatMeaning,
            },
            source: FloatMeaningSource::DirectOperationResult(DirectOperationFloatResult {
                owner: direct_result_owner,
                producer: direct_operation,
                result: direct_operation_result.id,
                format: IeeeFloatFormat::Binary64,
            }),
            operation: FloatMeaningProjectionOperation::Meaning64,
            contract: contract(numerics::float_projection::FloatProjectionOperation::Meaning64),
        },
        FloatMeaningProjection {
            result: ProofValueDeclaration {
                id: ProofValueId(6),
                value_type: ProofOnlyValueType::FloatMeaning,
            },
            source: FloatMeaningSource::DirectBlockParameter(DirectBlockFloatParameter {
                owner: direct_result_owner,
                block: block_id(4),
                parameter: direct_block_parameter.id,
                format: IeeeFloatFormat::Binary64,
            }),
            operation: FloatMeaningProjectionOperation::Meaning64,
            contract: contract(numerics::float_projection::FloatProjectionOperation::Meaning64),
        },
    ];
    module.float_meaning_equalities = vec![FloatMeaningEqualityProposition {
        id: ProofPropositionId(0),
        left: ProofValueId(0),
        right: ProofValueId(1),
    }];
    let bytes = encode_module(&module).expect("proof-only projections encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));

    let exact_source_prefix = [0, 0, 0, 0, 1, 2, 0x78, 0x56, 0x34, 0x12, 1];
    let source_offset = bytes
        .windows(exact_source_prefix.len())
        .position(|window| window == exact_source_prefix)
        .expect("exact binary32 source encoding is unique");
    let mut unknown_source_kind = bytes.clone();
    unknown_source_kind[source_offset + 5] = 0xff;
    assert_eq!(
        decode_module(&unknown_source_kind),
        Err(CodecError::InvalidTag("FloatMeaningSource", 0xff))
    );

    let original_identity = semantic_fingerprint(&module).expect("literal identity");
    let mut changed_catalog = module.clone();
    changed_catalog.float_meaning_projections[0]
        .contract
        .catalog_version += 1;
    assert!(matches!(
        encode_module(&changed_catalog),
        Err(CodecError::InvalidModule(
            terminal_verifier::ModuleError::InvalidFloatMeaningProjection { .. }
        ))
    ));
    let mut changed_declaration = module.clone();
    changed_declaration.float_meaning_projections[0]
        .contract
        .declaration ^= 1;
    assert!(matches!(
        encode_module(&changed_declaration),
        Err(CodecError::InvalidModule(
            terminal_verifier::ModuleError::InvalidFloatMeaningProjection { .. }
        ))
    ));
    let mut changed_owner_commitment = module.clone();
    changed_owner_commitment.float_meaning_projections[0]
        .contract
        .commitment[0] ^= 1;
    assert!(matches!(
        encode_module(&changed_owner_commitment),
        Err(CodecError::InvalidModule(
            terminal_verifier::ModuleError::InvalidFloatMeaningProjection { .. }
        ))
    ));
    let mut changed_bits = module.clone();
    changed_bits.float_meaning_projections[0].source =
        FloatMeaningSource::ExactBinary32Literal(0x8000_0000);
    assert_ne!(
        semantic_fingerprint(&changed_bits).expect("changed literal identity"),
        original_identity,
        "raw literal bits are Terminal semantic identity"
    );
    let mut changed_direct_parameter = module.clone();
    changed_direct_parameter.machines[0]
        .parameters
        .push(ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(10),
            scalar_type: ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
        });
    changed_direct_parameter.float_meaning_projections[3].source =
        FloatMeaningSource::DirectMachineParameter(DirectMachineFloatParameter {
            owner: changed_direct_parameter.entry,
            parameter: value_id(10),
            format: IeeeFloatFormat::Binary32,
        });
    assert_ne!(
        semantic_fingerprint(&changed_direct_parameter).expect("changed direct parameter identity"),
        original_identity,
        "direct owner and parameter coordinates enter Terminal semantic identity"
    );
    let mut changed_direct_result = module.clone();
    changed_direct_result.machines[1].result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(10),
        scalar_type: ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
    });
    changed_direct_result.float_meaning_projections[4].source =
        FloatMeaningSource::DirectMachineResult(DirectMachineFloatResult {
            owner: direct_result_owner,
            result: value_id(10),
            format: IeeeFloatFormat::Binary32,
        });
    assert_ne!(
        semantic_fingerprint(&changed_direct_result).expect("changed direct result identity"),
        original_identity,
        "direct owner and result coordinates enter Terminal semantic identity"
    );
    let mut changed_direct_operation = module.clone();
    changed_direct_operation.machines[1].blocks[0].operations[0].id = operation_id(10);
    changed_direct_operation.float_meaning_projections[5].source =
        FloatMeaningSource::DirectOperationResult(DirectOperationFloatResult {
            owner: direct_result_owner,
            producer: operation_id(10),
            result: direct_operation_result.id,
            format: IeeeFloatFormat::Binary64,
        });
    assert_ne!(
        semantic_fingerprint(&changed_direct_operation)
            .expect("changed direct operation-result identity"),
        original_identity,
        "direct owner, producer, and result coordinates enter Terminal semantic identity"
    );
    let mut changed_direct_block = module.clone();
    changed_direct_block.machines[1].blocks[1].id = block_id(5);
    let Terminator::Jump { target, .. } =
        &mut changed_direct_block.machines[1].blocks[0].terminator
    else {
        unreachable!("fixture entry jumps to the direct-parameter block")
    };
    *target = block_id(5);
    changed_direct_block.float_meaning_projections[6].source =
        FloatMeaningSource::DirectBlockParameter(DirectBlockFloatParameter {
            owner: direct_result_owner,
            block: block_id(5),
            parameter: direct_block_parameter.id,
            format: IeeeFloatFormat::Binary64,
        });
    assert_ne!(
        semantic_fingerprint(&changed_direct_block)
            .expect("changed direct block-parameter identity"),
        original_identity,
        "direct owner, block, and parameter coordinates enter Terminal semantic identity"
    );

    let mut reordered = module.clone();
    reordered.float_meaning_projections.swap(0, 1);
    assert!(matches!(
        encode_module(&reordered),
        Err(CodecError::NonCanonicalOrder(
            "float-meaning projections by dense proof value and first-use source IDs"
        ))
    ));

    let mut duplicate_value = module.clone();
    duplicate_value.float_meaning_projections[1].source =
        FloatMeaningSource::ExactBinary32Literal(0x1234_5678);
    duplicate_value.float_meaning_projections[1].operation =
        FloatMeaningProjectionOperation::Meaning32;
    assert!(matches!(
        encode_module(&duplicate_value),
        Err(CodecError::InvalidModule(
            terminal_verifier::ModuleError::DuplicateFloatMeaningProjection {
                first: 0,
                duplicate: 1,
            }
        ))
    ));

    let mut unknown_operand = module.clone();
    unknown_operand.float_meaning_equalities[0].right = ProofValueId(7);
    assert!(matches!(
        encode_module(&unknown_operand),
        Err(CodecError::InvalidModule(
            terminal_verifier::ModuleError::UnknownFloatMeaningEqualityOperand { .. }
        ))
    ));

    let mut noncanonical_operands = module.clone();
    noncanonical_operands.float_meaning_equalities[0].left = ProofValueId(1);
    noncanonical_operands.float_meaning_equalities[0].right = ProofValueId(0);
    assert!(matches!(
        encode_module(&noncanonical_operands),
        Err(CodecError::NonCanonicalOrder(
            "float-meaning equalities by dense proposition ID and ordered operands"
        ))
    ));

    let mut cross_format_equality = module.clone();
    cross_format_equality.float_meaning_equalities[0].right = ProofValueId(2);
    assert!(matches!(
        encode_module(&cross_format_equality),
        Err(CodecError::InvalidModule(
            terminal_verifier::ModuleError::InvalidFloatMeaningProjection { .. }
        ))
    ));

    let mut cross_format = module;
    cross_format.float_meaning_projections[0].source =
        FloatMeaningSource::ExactBinary64Literal(0x0000_0000_0000_0000);
    assert!(matches!(
        encode_module(&cross_format),
        Err(CodecError::InvalidModule(
            terminal_verifier::ModuleError::InvalidFloatMeaningProjection { .. }
        ))
    ));
}

#[test]
fn direct_call_float_result_round_trips_and_enters_semantic_identity() {
    let format = IeeeFloatFormat::Binary32;
    let scalar_type = ScalarType::IeeeFloat(format);
    let mut module = call_fixture();
    let TerminalMachineResult::Scalar(caller_result) = &mut module.machines[0].result else {
        unreachable!("fixture caller has a scalar result")
    };
    caller_result.scalar_type = scalar_type;
    let OperationResult::Scalar(constant_result) =
        &mut module.machines[0].blocks[0].operations[0].result
    else {
        unreachable!("fixture constant has a scalar result")
    };
    constant_result.scalar_type = scalar_type;
    module.machines[0].blocks[0].operations[0].kind = OperationKind::IeeeFloatConstant {
        value: IeeeFloatValue::Binary32(0x3f80_0000),
    };
    let OperationResult::Scalar(call_result) =
        &mut module.machines[0].blocks[0].operations[1].result
    else {
        unreachable!("fixture call has a scalar result")
    };
    call_result.scalar_type = scalar_type;
    module.machines[1].parameters[0].scalar_type = scalar_type;
    let TerminalMachineResult::Scalar(callee_result) = &mut module.machines[1].result else {
        unreachable!("fixture callee has a scalar result")
    };
    callee_result.scalar_type = scalar_type;

    let contract =
        numerics::float_projection::FloatProjectionOperation::Meaning32.contract_identity();
    module.float_meaning_projections = vec![FloatMeaningProjection {
        result: ProofValueDeclaration {
            id: ProofValueId(0),
            value_type: ProofOnlyValueType::FloatMeaning,
        },
        source: FloatMeaningSource::DirectCallResult(DirectCallFloatResult {
            owner: machine_id(100),
            producer: operation_id(101),
            result: value_id(101),
            format,
        }),
        operation: FloatMeaningProjectionOperation::Meaning32,
        contract: terminal_psi::FloatProjectionContractIdentity {
            format: contract.format,
            operation: contract.operation,
            declaration: contract.declaration,
            catalog_version: contract.catalog_version,
            commitment: contract.commitment,
        },
    }];

    let bytes = encode_module(&module).expect("direct call-result source should encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    let source_prefix = [0, 0, 0, 0, 1, 8];
    let source_offset = bytes
        .windows(source_prefix.len())
        .position(|window| window == source_prefix)
        .expect("direct call-result source encoding is unique");
    let mut unknown_source = bytes.clone();
    unknown_source[source_offset + 5] = 0xff;
    assert_eq!(
        decode_module(&unknown_source),
        Err(CodecError::InvalidTag("FloatMeaningSource", 0xff))
    );

    let original = semantic_fingerprint(&module).expect("direct call-result identity");
    module.machines[0].blocks[0].operations[1].id = operation_id(102);
    module.float_meaning_projections[0].source =
        FloatMeaningSource::DirectCallResult(DirectCallFloatResult {
            owner: machine_id(100),
            producer: operation_id(102),
            result: value_id(101),
            format,
        });
    assert_ne!(
        semantic_fingerprint(&module).expect("changed direct call-result identity"),
        original,
        "direct call producer identity enters Terminal semantic identity"
    );
}

#[test]
fn direct_structural_float_leaf_round_trips_complete_path_and_identity() {
    let mut module = unit_fixture();
    let owner = module.entry;
    let root = place_id(500);
    let structural_type = structural_type_id(500);
    let field = structural_field_id(500);
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type,
        identity: "Fixture::FloatRecord".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![
                StructuralFieldDeclaration {
                    id: field,
                    identity: "value".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary32),
                },
                StructuralFieldDeclaration {
                    id: structural_field_id(501),
                    identity: "alternate".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary32),
                },
            ],
        },
    });
    module.machines[0]
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: root,
            position: 0,
            is_self: false,
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    module.machines[0]
        .structural_places
        .push(StructuralPlaceDeclaration {
            id: root,
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        });
    let contract =
        numerics::float_projection::FloatProjectionOperation::Meaning32.contract_identity();
    module.float_meaning_projections = vec![FloatMeaningProjection {
        result: ProofValueDeclaration {
            id: ProofValueId(0),
            value_type: ProofOnlyValueType::FloatMeaning,
        },
        source: FloatMeaningSource::DirectStructuralLeaf(DirectStructuralFloatLeaf {
            owner,
            field: IeeeFloatStructuralField::new(
                root,
                vec![CanonicalStructuralPathSegment::Field(field)],
            )
            .expect("nonempty structural path"),
            format: IeeeFloatFormat::Binary32,
        }),
        operation: FloatMeaningProjectionOperation::Meaning32,
        contract: terminal_psi::FloatProjectionContractIdentity {
            format: contract.format,
            operation: contract.operation,
            declaration: contract.declaration,
            catalog_version: contract.catalog_version,
            commitment: contract.commitment,
        },
    }];

    let bytes = encode_module(&module).expect("direct structural leaf should encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    let source_prefix = [0, 0, 0, 0, 1, 9];
    let source_offset = bytes
        .windows(source_prefix.len())
        .position(|window| window == source_prefix)
        .expect("direct structural-leaf source encoding is unique");
    let mut unknown_source = bytes.clone();
    unknown_source[source_offset + 5] = 0xff;
    assert_eq!(
        decode_module(&unknown_source),
        Err(CodecError::InvalidTag("FloatMeaningSource", 0xff))
    );

    let original = semantic_fingerprint(&module).expect("direct structural-leaf identity");
    let FloatMeaningSource::DirectStructuralLeaf(source) =
        &mut module.float_meaning_projections[0].source
    else {
        unreachable!()
    };
    source.field = IeeeFloatStructuralField::new(
        root,
        vec![CanonicalStructuralPathSegment::Field(structural_field_id(
            501,
        ))],
    )
    .expect("nonempty structural path");
    assert_ne!(
        semantic_fingerprint(&module).expect("changed direct structural-leaf identity"),
        original,
        "the complete canonical leaf path enters Terminal semantic identity"
    );
}

#[test]
fn payload_sum_shape_round_trips_exact_fields_and_requires_canonical_order() {
    let mut valid = fixture();
    valid.structural_types.push(StructuralTypeDeclaration {
        id: structural_type_id(99),
        identity: "Mode".to_owned(),
        shape: StructuralTypeShape::Sum {
            cases: vec![
                StructuralCaseDeclaration {
                    id: structural_case_id(1),
                    identity: "Off".to_owned(),
                    fields: Vec::new(),
                },
                StructuralCaseDeclaration {
                    id: structural_case_id(2),
                    identity: "On".to_owned(),
                    fields: vec![
                        StructuralFieldDeclaration {
                            id: structural_field_id(1),
                            identity: "enabled".to_owned(),
                            relevance: BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                        },
                        StructuralFieldDeclaration {
                            id: structural_field_id(2),
                            identity: "count".to_owned(),
                            relevance: BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                                IntegerType::new(IntegerSign::Signed, 32).expect("i32"),
                            )),
                        },
                    ],
                },
            ],
        },
    });
    valid
        .structural_types
        .sort_by_key(|declaration| declaration.id);
    let bytes = encode_module(&valid).expect("canonical payload sum encodes");
    assert_eq!(decode_module(&bytes), Ok(valid.clone()));

    let mut reordered = valid.clone();
    let StructuralTypeShape::Sum { cases } = &mut reordered
        .structural_types
        .iter_mut()
        .find(|declaration| declaration.identity == "Mode")
        .expect("sum")
        .shape
    else {
        unreachable!()
    };
    cases.reverse();
    assert_eq!(
        encode_module(&reordered),
        Err(CodecError::NonCanonicalOrder(
            "structural cases by StructuralCaseId"
        ))
    );

    let mut reordered_fields = valid.clone();
    let StructuralTypeShape::Sum { cases } = &mut reordered_fields
        .structural_types
        .iter_mut()
        .find(|declaration| declaration.identity == "Mode")
        .expect("sum")
        .shape
    else {
        unreachable!()
    };
    cases[1].fields.reverse();
    assert_eq!(
        encode_module(&reordered_fields),
        Err(CodecError::NonCanonicalOrder(
            "structural case fields by StructuralFieldId"
        ))
    );

    let mut empty = valid;
    let StructuralTypeShape::Sum { cases } = &mut empty
        .structural_types
        .iter_mut()
        .find(|declaration| declaration.identity == "Mode")
        .expect("sum")
        .shape
    else {
        unreachable!()
    };
    cases.clear();
    assert!(encode_module(&empty).is_err());
}

#[test]
fn partial_affine_unit_return_round_trips_exact_path_and_leaf_type() {
    let module = partial_affine_fixture();
    let bytes = encode_module(&module).expect("partial affine return should encode");
    assert_eq!(&bytes[8..10], 103_u16.to_le_bytes());
    assert_eq!(
        &bytes[10..12],
        VocabularyMarker::CURRENT.get().to_le_bytes()
    );
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));
}

#[test]
fn nominal_affine_unit_return_round_trips_exact_root_type_and_cleanup_machine() {
    let module = nominal_affine_fixture();
    let bytes = encode_module(&module).expect("nominal affine return should encode");
    assert_eq!(&bytes[8..10], 103_u16.to_le_bytes());
    assert_eq!(
        &bytes[10..12],
        VocabularyMarker::CURRENT.get().to_le_bytes()
    );
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));
}

#[test]
fn scalar_return_round_trips_nominal_affine_cleanup_action() {
    let mut module = nominal_affine_fixture();
    let machine = &mut module.machines[0];
    let source = ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(50),
        scalar_type: ScalarType::Boolean,
    };
    machine.parameters = vec![source];
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(51),
        scalar_type: ScalarType::Boolean,
    });
    let Terminator::ReturnUnitNominalAffine { edge, cleanups } = &machine.blocks[0].terminator
    else {
        unreachable!()
    };
    machine.blocks[0].terminator = Terminator::Return {
        edge: *edge,
        value: source.id,
        cleanup_actions: vec![TerminalAffineCleanupAction::InvokeNominal(
            cleanups[0].clone(),
        )],
    };

    let bytes = encode_module(&module).expect("scalar nominal cleanup should encode");
    assert_eq!(&bytes[8..10], 103_u16.to_le_bytes());
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));
}

#[test]
fn contextual_nominal_affine_cleanup_round_trips_receiver_and_ordered_obligations() {
    let mut module = nominal_affine_fixture();
    let receiver = place_id(99);
    let first = StructuralFieldId::new(1).expect("first field");
    let second = StructuralFieldId::new(2).expect("second field");
    module.structural_types[0].shape = StructuralTypeShape::Record {
        fields: vec![
            StructuralFieldDeclaration {
                id: first,
                identity: "ready".into(),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
            },
            StructuralFieldDeclaration {
                id: second,
                identity: "settled".into(),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
            },
        ],
    };
    module.machines[1].contract.requires = [first, second]
        .into_iter()
        .map(|field| {
            Proposition::Equal(
                ScalarTerm::boolean(true),
                ScalarTerm::boolean_field(receiver, field),
            )
        })
        .collect();
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } =
        &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    cleanups[0].cleanup_receiver = Some(receiver);
    cleanups[0].requirement_obligations = vec![obligation_id(8), obligation_id(3)];

    let bytes = encode_module(&module).expect("contextual nominal cleanup should encode");
    let decoded = decode_module(&bytes).expect("contextual nominal cleanup should decode");
    assert_eq!(decoded, module);
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } =
        &decoded.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    assert_eq!(cleanups[0].cleanup_receiver, Some(receiver));
    assert_eq!(
        cleanups[0].requirement_obligations,
        vec![obligation_id(8), obligation_id(3)]
    );

    let mut truncated = bytes;
    truncated.pop();
    assert_eq!(decode_module(&truncated), Err(CodecError::UnexpectedEnd));

    let mut reordered = module.clone();
    reordered.machines[1].contract.requires.reverse();
    assert_eq!(
        encode_module(&reordered),
        Err(CodecError::NonCanonicalOrder("requires propositions"))
    );

    let mut duplicate = module;
    duplicate.machines[1].contract.requires[1] = duplicate.machines[1].contract.requires[0].clone();
    assert_eq!(
        encode_module(&duplicate),
        Err(CodecError::NonCanonicalOrder("requires propositions"))
    );
}

#[test]
fn nominal_affine_unit_return_round_trips_two_roots_in_reverse_parameter_order() {
    let module = two_nominal_affine_fixture();
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } =
        &module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    assert_eq!(
        cleanups
            .iter()
            .map(|cleanup| cleanup.place)
            .collect::<Vec<_>>(),
        vec![place_id(2), place_id(1)]
    );

    let bytes = encode_module(&module).expect("two nominal affine roots should encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));

    let mut reordered = module;
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } =
        &mut reordered.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    cleanups.reverse();
    assert_eq!(
        encode_module(&reordered),
        Err(CodecError::MalformedStructuralFoundation(
            "nominal affine cleanup list is not in reverse parameter order"
        ))
    );
}

#[test]
fn nominal_affine_unit_return_round_trips_five_roots() {
    let module = five_nominal_affine_fixture();
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } =
        &module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    assert_eq!(
        cleanups
            .iter()
            .map(|cleanup| cleanup.place)
            .collect::<Vec<_>>(),
        vec![
            place_id(5),
            place_id(4),
            place_id(3),
            place_id(2),
            place_id(1)
        ]
    );
    let bytes = encode_module(&module).expect("five nominal affine roots should encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
}

#[test]
fn nominal_affine_unit_return_rejects_malformed_source_carriers() {
    let mut valued = nominal_affine_fixture();
    valued.machines[0].result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(99),
        scalar_type: ScalarType::Integer(i32_type()),
    });
    assert_eq!(
        encode_module(&valued),
        Err(CodecError::MalformedStructuralFoundation(
            "nominal affine cleanup requires a Unit result"
        ))
    );

    let mut wrong_type = nominal_affine_fixture();
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } =
        &mut wrong_type.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    cleanups[0].structural_type = structural_type_id(2);
    assert_eq!(
        encode_module(&wrong_type),
        Err(CodecError::MalformedStructuralFoundation(
            "nominal affine cleanup type does not match its structural parameter"
        ))
    );

    let mut claimed = nominal_affine_fixture();
    let place = claimed.machines[0].structural_parameters[0].place;
    claimed.machines[0].entry_claims.push(EntryClaim {
        claim: claim_id(1),
        input: place,
        path: Vec::new(),
    });
    assert_eq!(
        encode_module(&claimed),
        Err(CodecError::MalformedStructuralFoundation(
            "nominal affine cleanup is duplicated or not a claim-free qualified-free affine root"
        ))
    );

    let mut unrestricted = nominal_affine_fixture();
    unrestricted.machines[0].structural_parameters[0].multiplicity =
        StructuralMultiplicity::Unrestricted;
    assert_eq!(
        encode_module(&unrestricted),
        Err(CodecError::MalformedStructuralFoundation(
            "nominal affine cleanup is duplicated or not a claim-free qualified-free affine root"
        ))
    );

    let mut qualified = nominal_affine_fixture();
    qualified
        .structural_domains
        .push(StructuralDomainDeclaration {
            id: structural_domain_id(1),
            semantic_domain: semantic_vocabulary::DomainSemanticId::new(1).unwrap(),
            identity: "example::NominalResource::Ready".to_owned(),
            carrier: structural_type_id(1),
            content_projection: None,
        });
    qualified.machines[0].structural_parameters[0]
        .qualifications
        .push(structural_domain_id(1));
    assert_eq!(
        encode_module(&qualified),
        Err(CodecError::MalformedStructuralFoundation(
            "nominal affine cleanup is duplicated or not a claim-free qualified-free affine root"
        ))
    );

    let mut missing_root = nominal_affine_fixture();
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } =
        &mut missing_root.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    cleanups[0].place = place_id(99);
    assert_eq!(
        encode_module(&missing_root),
        Err(CodecError::MalformedStructuralFoundation(
            "nominal affine cleanup root is not a structural parameter"
        ))
    );
}
