//! Current-only admission preserves exact canonical bytes and refuses migration.
use super::{CodecError, FORMAT_MARKER, decode_module, encode_module};
use terminal_psi::VocabularyMarker;

// Captured from d743b4e805 before retiring its legacy encoder. This is a
// complete previously accepted artifact, not a current body with stale markers.
const LEGACY_UNIT: &[u8] = &[
    80, 83, 73, 84, 69, 82, 77, 0, 56, 0, 59, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0,
    0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

// The same Unit semantics with the current declaration rosters and markers.
const CURRENT_UNIT: &[u8] = &[
    80, 83, 73, 84, 69, 82, 77, 0, 91, 0, 102, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0,
    0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5, 1, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

#[test]
fn complete_legacy_artifact_rejects_without_migration() {
    assert_eq!(
        decode_module(LEGACY_UNIT),
        Err(CodecError::UnsupportedFormatMarker(56))
    );
    let mut relabeled = LEGACY_UNIT.to_vec();
    relabeled[8..10].copy_from_slice(&FORMAT_MARKER.to_le_bytes());
    relabeled[10..12].copy_from_slice(&VocabularyMarker::CURRENT.get().to_le_bytes());
    assert!(
        decode_module(&relabeled).is_err(),
        "markers cannot repair missing current rosters"
    );
}

#[test]
fn current_unit_golden_bytes_remain_identical() {
    let module = decode_module(CURRENT_UNIT).expect("current Unit fixture");
    assert_eq!(module.vocabulary_marker, VocabularyMarker::CURRENT);
    assert_eq!(module.machines.len(), 1);
    assert_eq!(
        module.machines[0].result,
        terminal_psi::TerminalMachineResult::Unit
    );
    assert_eq!(encode_module(&module).unwrap(), CURRENT_UNIT);
}

fn declared_service_module() -> terminal_psi::TerminalModule {
    let mut module = decode_module(CURRENT_UNIT).expect("current Unit fixture");
    let service = semantic_vocabulary::ServiceId::new(1).unwrap();
    module.services.push(terminal_psi::ServiceDeclaration {
        id: service,
        identity: "Console".into(),
        parents: Vec::new(),
    });
    module.machines[0].declared_service_reach = vec![service];
    module.machines[0].published_service_ceiling = vec![service];
    module.root_service_reach.concrete = vec![service];
    module
}

#[test]
fn declared_service_reach_roundtrips_and_participates_in_semantic_identity() {
    let module = declared_service_module();
    let bytes = encode_module(&module).expect("declared service contract encodes");
    assert_eq!(decode_module(&bytes).unwrap(), module);
    assert_eq!(
        encode_module(&decode_module(&bytes).unwrap()).unwrap(),
        bytes
    );
    let mut undeclared = module.clone();
    undeclared.machines[0].declared_service_reach.clear();
    undeclared.root_service_reach.concrete.clear();
    assert_ne!(
        super::terminal_psi_identity(&module).unwrap(),
        super::terminal_psi_identity(&undeclared).unwrap()
    );
}

#[test]
fn declared_service_reach_decoder_rejects_corrupt_rows_and_missing_root() {
    let module = declared_service_module();
    let mut duplicate = module.clone();
    duplicate.machines[0]
        .declared_service_reach
        .push(semantic_vocabulary::ServiceId::new(1).unwrap());
    let bytes = super::encode_raw(&duplicate).expect("raw corrupt fixture");
    assert!(matches!(
        decode_module(&bytes),
        Err(CodecError::InvalidModule(
            terminal_verifier::ModuleError::DuplicatePublishedService { .. }
        ))
    ));
    let mut missing_root = module;
    missing_root.root_service_reach.concrete.clear();
    let bytes = super::encode_raw(&missing_root).expect("raw stale root fixture");
    assert!(matches!(
        decode_module(&bytes),
        Err(CodecError::InvalidModule(
            terminal_verifier::ModuleError::RootConcreteServiceReachMismatch { .. }
        ))
    ));
}

#[test]
fn declared_service_reach_format_rejects_previous_contract_encoding() {
    let mut bytes = encode_module(&declared_service_module()).unwrap();
    bytes[8..10].copy_from_slice(&(FORMAT_MARKER - 1).to_le_bytes());
    assert_eq!(
        decode_module(&bytes),
        Err(CodecError::UnsupportedFormatMarker(FORMAT_MARKER - 1))
    );
}

fn fixed_boundary_service_module() -> terminal_psi::TerminalModule {
    let mut module = decode_module(CURRENT_UNIT).expect("current Unit fixture");
    let fixed = semantic_vocabulary::ServiceId::new(1).unwrap();
    let bound = semantic_vocabulary::ServiceId::new(2).unwrap();
    let boundary = semantic_vocabulary::BoundaryMachineId::new(1).unwrap();
    module.services = vec![
        terminal_psi::ServiceDeclaration {
            id: fixed,
            identity: "Installer".into(),
            parents: Vec::new(),
        },
        terminal_psi::ServiceDeclaration {
            id: bound,
            identity: "Console".into(),
            parents: Vec::new(),
        },
    ];
    module
        .boundary_machines
        .push(terminal_psi::BoundaryMachineDeclaration {
            id: boundary,
            identity: "Installer::step".into(),
            attachment: None,
            scalar_parameters: Vec::new(),
            crash_routes: Vec::new(),
            structural_parameters: Vec::new(),
            result: terminal_psi::BoundaryMachineResult::Unit,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            fixed_service_reach: vec![fixed],
            published_service_ceiling: vec![fixed, bound],
        });
    module.machines[0].published_service_ceiling = vec![fixed, bound];
    module.machines[0].blocks[0]
        .operations
        .push(terminal_psi::Operation {
            id: semantic_vocabulary::OperationId::new(1).unwrap(),
            result: terminal_psi::OperationResult::Unit,
            kind: terminal_psi::OperationKind::BoundaryCall {
                boundary,
                arguments: Vec::new(),
                structural_arguments: Vec::new(),
                completion_receipts: Vec::new(),
            },
        });
    module.root_service_reach.concrete = vec![fixed];
    module.root_service_reach.installation_dependencies =
        vec![terminal_psi::InstallationReachDependency {
            requirement_identity: "Installer::step".into(),
            upper_bound: vec![bound],
        }];
    module
}

#[test]
fn fixed_boundary_service_reach_roundtrips_with_distinct_installation_identity() {
    let module = fixed_boundary_service_module();
    let bytes = encode_module(&module).expect("fixed boundary contract");
    assert_eq!(decode_module(&bytes).unwrap(), module);
    assert_eq!(
        encode_module(&decode_module(&bytes).unwrap()).unwrap(),
        bytes
    );
    let mut overlapping = module.clone();
    let bound = overlapping.root_service_reach.installation_dependencies[0].upper_bound[0];
    overlapping.boundary_machines[0]
        .fixed_service_reach
        .push(bound);
    overlapping.root_service_reach.concrete.push(bound);
    let overlapping_bytes = encode_module(&overlapping).unwrap();
    assert_eq!(decode_module(&overlapping_bytes).unwrap(), overlapping);
    assert_ne!(
        super::semantic_fingerprint(&module).unwrap(),
        super::semantic_fingerprint(&overlapping).unwrap()
    );
    assert_eq!(
        module.root_service_reach.installation_dependencies,
        overlapping.root_service_reach.installation_dependencies
    );
}

#[test]
fn fixed_boundary_service_reach_decoder_rejects_corrupt_partition_and_rosters() {
    let module = fixed_boundary_service_module();
    for mutation in 0..6 {
        let mut corrupt = module.clone();
        match mutation {
            0 => corrupt.boundary_machines[0].fixed_service_reach.clear(),
            1 => corrupt.root_service_reach.concrete.clear(),
            2 => corrupt.root_service_reach.installation_dependencies[0]
                .upper_bound
                .clear(),
            3 => {
                corrupt.boundary_machines[0].fixed_service_reach =
                    vec![semantic_vocabulary::ServiceId::new(3).unwrap()];
            }
            4 => {
                let fixed = corrupt.boundary_machines[0].fixed_service_reach[0];
                corrupt.boundary_machines[0].fixed_service_reach.push(fixed);
            }
            5 => corrupt.boundary_machines[0]
                .published_service_ceiling
                .clear(),
            _ => unreachable!(),
        }
        let bytes = super::encode_raw(&corrupt).expect("raw corruption fixture");
        assert!(
            decode_module(&bytes).is_err(),
            "mutation {mutation} must reject"
        );
    }
}

#[test]
fn every_noncurrent_format_and_vocabulary_marker_rejects() {
    let mut header = CURRENT_UNIT[..12].to_vec();
    for marker in 0..=u16::MAX {
        if marker != FORMAT_MARKER {
            header[8..10].copy_from_slice(&marker.to_le_bytes());
            assert_eq!(
                decode_module(&header),
                Err(CodecError::UnsupportedFormatMarker(marker))
            );
        }
    }
    header[8..10].copy_from_slice(&FORMAT_MARKER.to_le_bytes());
    for marker in 0..=u16::MAX {
        if marker != VocabularyMarker::CURRENT.get() {
            header[10..12].copy_from_slice(&marker.to_le_bytes());
            assert_eq!(
                decode_module(&header),
                Err(CodecError::UnsupportedVocabularyMarker(marker))
            );
        }
    }
}

#[test]
fn crossed_markers_truncation_and_trailing_bytes_reject() {
    for format in [
        0,
        56,
        FORMAT_MARKER - 1,
        FORMAT_MARKER,
        FORMAT_MARKER + 1,
        u16::MAX,
    ] {
        for vocabulary in [
            0,
            59,
            VocabularyMarker::CURRENT.get() - 1,
            VocabularyMarker::CURRENT.get(),
            VocabularyMarker::CURRENT.get() + 1,
            u16::MAX,
        ] {
            let mut bytes = CURRENT_UNIT.to_vec();
            bytes[8..10].copy_from_slice(&format.to_le_bytes());
            bytes[10..12].copy_from_slice(&vocabulary.to_le_bytes());
            assert_eq!(
                decode_module(&bytes).is_ok(),
                format == FORMAT_MARKER && vocabulary == VocabularyMarker::CURRENT.get()
            );
        }
    }
    for length in 0..CURRENT_UNIT.len() {
        assert!(
            decode_module(&CURRENT_UNIT[..length]).is_err(),
            "truncation at {length}"
        );
    }
    let mut trailing = CURRENT_UNIT.to_vec();
    trailing.push(0);
    assert_eq!(decode_module(&trailing), Err(CodecError::TrailingBytes(1)));
}

#[test]
fn portable_envelope_cannot_hide_legacy_semantics() {
    // Use the public envelope decoder: semantic admission precedes proof and
    // optimization admission, even when those sections have no bytes.
    let mut envelope = b"PSIART\0\0".to_vec();
    envelope.extend_from_slice(&2_u16.to_le_bytes());
    envelope.extend_from_slice(&(LEGACY_UNIT.len() as u64).to_le_bytes());
    envelope.extend_from_slice(&0_u64.to_le_bytes());
    envelope.extend_from_slice(&0_u64.to_le_bytes());
    envelope.push(0);
    envelope.extend_from_slice(LEGACY_UNIT);
    assert!(matches!(
        super::CanonicalTerminalArtifact::from_bytes(&envelope),
        Err(super::CanonicalTerminalArtifactError::Semantic(
            CodecError::UnsupportedFormatMarker(56)
        ))
    ));
}
