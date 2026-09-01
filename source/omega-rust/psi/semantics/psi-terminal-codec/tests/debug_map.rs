use psi_core::{
    BlockId, ClaimId, ContentAlgebra, ContentAlgebraKind, ContentDomainId, ContentPlaceVersion,
    ContentProjectionExpression, ContentProjectionIdentity, ContentProjectionScalar,
    ContentStructuralPlace, ContractId, EdgeId, IntegerSign, IntegerType, MachineId, OperationId,
    PlaceId, ScalarType, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use psi_terminal::{
    Block, ClaimContentProjection, ContentEntryClaim, MachineContract, StructuralContentProjection,
    StructuralDomainDeclaration, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
    ValueDeclaration, VocabularyMarker,
};
use psi_terminal_codec::{
    DebugFileId, DebugMapError, DebugSite, DebugSourceFile, DebugSourceOrigin, DebugSourceSpan,
    DebugSubject, TerminalDebugMap, decode_debug_map, encode_debug_map, source_digest,
    terminal_psi_identity, validate_debug_map,
};

#[test]
fn typed_debug_map_round_trips_and_binds_exact_semantics() {
    let module = fixture();
    let debug_map = debug_map(&module);
    let bytes = encode_debug_map(&module, &debug_map).expect("debug map should encode");

    assert_eq!(&bytes[..8], b"PSIDBG\0\0");
    assert_eq!(&bytes[8..10], 1_u16.to_le_bytes());
    assert_eq!(decode_debug_map(&module, &bytes), Ok(debug_map.clone()));
    assert_eq!(encode_debug_map(&module, &debug_map), Ok(bytes));

    let mut other = fixture();
    other.machines[0].id = machine_id(99);
    other.entry = machine_id(99);
    assert!(matches!(
        validate_debug_map(&other, &debug_map),
        Err(DebugMapError::SemanticIdentityMismatch { .. })
    ));
}

#[test]
fn typed_debug_map_rejects_unknown_subjects_invalid_spans_and_order_drift() {
    let module = fixture();

    let mut unknown = debug_map(&module);
    unknown.sites[1].subject = DebugSubject::Operation(operation_id(99));
    assert_eq!(
        validate_debug_map(&module, &unknown),
        Err(DebugMapError::UnknownSubject(DebugSubject::Operation(
            operation_id(99)
        )))
    );

    let mut invalid_span = debug_map(&module);
    invalid_span.sites[0].span.end = 100;
    assert_eq!(
        validate_debug_map(&module, &invalid_span),
        Err(DebugMapError::InvalidSpan(invalid_span.sites[0].span))
    );

    let mut reordered = debug_map(&module);
    reordered.sites.swap(0, 1);
    assert_eq!(
        validate_debug_map(&module, &reordered),
        Err(DebugMapError::NonCanonicalOrder("debug sites by subject"))
    );
}

#[test]
fn unit_machine_debug_map_cannot_name_an_absent_result_value() {
    let mut module = fixture();
    module.machines[0].result = TerminalMachineResult::Unit;
    module.machines[0].blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(1),
        trivial_affine_discards: Vec::new(),
    };
    let mut map = debug_map(&module);
    map.sites.push(DebugSite {
        subject: DebugSubject::Value(value_id(2)),
        span: DebugSourceSpan {
            file: DebugFileId::new(1).unwrap(),
            start: 13,
            end: 15,
        },
    });

    assert_eq!(
        validate_debug_map(&module, &map),
        Err(DebugMapError::UnknownSubject(DebugSubject::Value(
            value_id(2)
        )))
    );
}

#[test]
fn typed_debug_map_accepts_an_entry_only_claim_subject() {
    let mut module = fixture();
    module.vocabulary_marker = VocabularyMarker::CURRENT;
    let claim = ClaimId::new(1).expect("claim");
    let place = PlaceId::new(1).expect("place");
    module.machines[0].structural_places = vec![StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    }];
    let algebra = ContentAlgebra {
        kind: ContentAlgebraKind::CountedQuantity,
        parameter: "Byte".to_owned(),
    };
    let expression = ContentProjectionExpression::CountedQuantity(
        ContentProjectionScalar::Natural("1".to_owned()),
    );
    let projection = ContentProjectionIdentity {
        domain: ContentDomainId::new(1).expect("content domain"),
        projection_report_fingerprint:
            psi_language_semantics::content::terminal_projection_report_fingerprint(
                &algebra,
                &expression,
            ),
    };
    let structural_type = StructuralTypeId::new(1).expect("structural type");
    module.structural_types = vec![StructuralTypeDeclaration {
        id: structural_type,
        identity: "DebugStorage".to_owned(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    }];
    module.structural_domains = vec![StructuralDomainDeclaration {
        id: psi_core::StructuralDomainId::new(1).expect("structural domain"),
        semantic_domain: psi_core::DomainSemanticId::new(1).expect("semantic domain"),
        identity: "DebugStorage::Content".to_owned(),
        carrier: structural_type,
        content_projection: Some(StructuralContentProjection {
            identity: projection,
            algebra: algebra.clone(),
            expression,
        }),
    }];
    module.machines[0].content_entry_claims = vec![ContentEntryClaim {
        claim,
        input: ContentStructuralPlace {
            version: ContentPlaceVersion::Entry,
            root: place,
            segments: Vec::new(),
        },
        projections: vec![ClaimContentProjection {
            projection,
            algebra,
        }],
    }];
    let mut debug_map = debug_map(&module);
    debug_map.sites.push(DebugSite {
        subject: DebugSubject::Claim {
            machine: machine_id(1),
            claim,
        },
        span: DebugSourceSpan {
            file: DebugFileId::new(1).expect("file"),
            start: 13,
            end: 15,
        },
    });

    let bytes = encode_debug_map(&module, &debug_map).expect("entry claim has a debug subject");
    assert_eq!(decode_debug_map(&module, &bytes), Ok(debug_map));
}

#[test]
fn typed_debug_map_decoder_rejects_hostile_encodings() {
    let module = fixture();
    let debug_map = debug_map(&module);
    let bytes = encode_debug_map(&module, &debug_map).expect("debug map should encode");

    let mut zero_file = bytes.clone();
    // magic + format marker + vocabulary marker + fingerprint + file count
    zero_file[48..52].copy_from_slice(&0_u32.to_le_bytes());
    assert_eq!(
        decode_debug_map(&module, &zero_file),
        Err(DebugMapError::ZeroFileIdentity)
    );

    let mut unknown_subject_tag = bytes.clone();
    // Header (48), one file record (49 + "main.omg"), then site count (4).
    unknown_subject_tag[109] = 0xff;
    assert_eq!(
        decode_debug_map(&module, &unknown_subject_tag),
        Err(DebugMapError::InvalidTag("DebugSubject", 0xff))
    );

    let mut trailing = bytes;
    trailing.push(0);
    assert_eq!(
        decode_debug_map(&module, &trailing),
        Err(DebugMapError::TrailingBytes(1))
    );
}

fn debug_map(module: &TerminalModule) -> TerminalDebugMap {
    let source = b"machine main {}";
    let file = DebugFileId::new(1).expect("nonzero file id");
    TerminalDebugMap {
        semantic: terminal_psi_identity(module).expect("module identity"),
        files: vec![DebugSourceFile {
            id: file,
            origin: DebugSourceOrigin::User,
            byte_len: source.len() as u64,
            digest: source_digest(source),
            path: "main.omg".to_owned(),
        }],
        sites: vec![
            DebugSite {
                subject: DebugSubject::Machine(machine_id(1)),
                span: DebugSourceSpan {
                    file,
                    start: 0,
                    end: 7,
                },
            },
            DebugSite {
                subject: DebugSubject::Edge(edge_id(1)),
                span: DebugSourceSpan {
                    file,
                    start: 8,
                    end: 12,
                },
            },
        ],
    }
}

fn fixture() -> TerminalModule {
    let scalar_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).expect("valid integer type"));
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine_id(1),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![ValueDeclaration {
                id: value_id(1),
                scalar_type,
            }],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                id: value_id(2),
                scalar_type,
            }),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(1),
            blocks: vec![Block {
                id: block_id(1),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: edge_id(1),
                    value: value_id(1),
                },
            }],
            contract: MachineContract {
                id: contract_id(1),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

fn machine_id(raw: u64) -> MachineId {
    MachineId::new(raw).expect("nonzero machine id")
}

fn block_id(raw: u64) -> BlockId {
    BlockId::new(raw).expect("nonzero block id")
}

fn value_id(raw: u64) -> ValueId {
    ValueId::new(raw).expect("nonzero value id")
}

fn edge_id(raw: u64) -> EdgeId {
    EdgeId::new(raw).expect("nonzero edge id")
}

fn operation_id(raw: u64) -> OperationId {
    OperationId::new(raw).expect("nonzero operation id")
}

fn contract_id(raw: u64) -> ContractId {
    ContractId::new(raw).expect("nonzero contract id")
}
