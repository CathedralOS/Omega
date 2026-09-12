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
    80, 83, 73, 84, 69, 82, 77, 0, 94, 0, 105, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0,
    0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5, 1, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0,
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

fn closed_reach_module() -> terminal_psi::TerminalModule {
    use semantic_vocabulary::{BlockId, ContractId, EdgeId, MachineId, OperationId};
    use terminal_psi::{
        ClosedReachApplication, ClosedReachCall, ClosedReachMachineBinding, ClosedReachParameter,
        Operation, OperationKind, OperationResult, Terminator,
    };
    let mut module = declared_service_module();
    let service = module.services[0].id;
    for ordinal in [2, 3] {
        let mut selected = module.machines[0].clone();
        selected.id = MachineId::new(ordinal).unwrap();
        selected.entry = BlockId::new(ordinal).unwrap();
        selected.contract.id = ContractId::new(ordinal).unwrap();
        selected.blocks[0].id = selected.entry;
        selected.blocks[0].terminator = Terminator::ReturnUnit {
            edge: EdgeId::new(ordinal).unwrap(),
            trivial_affine_discards: Vec::new(),
        };
        module.machines.push(selected);
    }
    let binding = |ordinal| {
        ClosedReachParameter::Machine(ClosedReachMachineBinding {
            nominal_requirement: Some("Callback::call".into()),
            upper_bound: vec![service],
            selected_identity: format!("selected-{ordinal}"),
            selected_contract_commitment: [3; 32],
            selected_reach: vec![service],
            callee: Some(MachineId::new(ordinal).unwrap()),
            schema: None,
        })
    };
    let owner = &mut module.machines[0];
    owner.declared_service_reach.clear();
    owner.closed_reach_application = Some(ClosedReachApplication {
        template_identity: "forward".into(),
        template_commitment: [1; 32],
        specialization_commitment: [2; 32],
        telescope: vec![
            ClosedReachParameter::Type {
                argument: "u64".into(),
            },
            binding(2),
            ClosedReachParameter::Const {
                argument: "u64:2".into(),
            },
            binding(3),
        ],
        fixed: Vec::new(),
        dependencies: vec![1, 3],
        calls: vec![
            ClosedReachCall {
                operation: OperationId::new(1).unwrap(),
                binder: 1,
                application: None,
            },
            ClosedReachCall {
                operation: OperationId::new(2).unwrap(),
                binder: 3,
                application: None,
            },
        ],
    });
    for (ordinal, binder, callee) in [(1, 1, 2), (2, 3, 3)] {
        owner.blocks[0].operations.push(Operation {
            id: OperationId::new(ordinal).unwrap(),
            static_reach_binding: Some(binder),
            result: OperationResult::Unit,
            kind: OperationKind::CallUnit {
                callee: MachineId::new(callee).unwrap(),
                arguments: Vec::new(),
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        });
    }
    module
}

#[test]
fn closed_reach_relation_roundtrips_and_binds_semantic_identity() {
    let module = closed_reach_module();
    let bytes = encode_module(&module).expect("closed substitution encodes");
    assert_eq!(decode_module(&bytes).unwrap(), module);
    for end in 0..bytes.len() {
        assert!(decode_module(&bytes[..end]).is_err(), "truncated at {end}");
    }
    let mut changed = module.clone();
    changed.machines[0]
        .closed_reach_application
        .as_mut()
        .unwrap()
        .template_identity
        .push_str("-other");
    assert_ne!(
        super::terminal_psi_identity(&module).unwrap(),
        super::terminal_psi_identity(&changed).unwrap()
    );
    // Provenance is deliberately not a fabricated opening of the original
    // source contract. A changed origin remains a different ordinary product.
    assert!(encode_module(&changed).is_ok());
}

#[test]
fn closed_reach_decoder_rejects_tampered_substitutions_and_call_joins() {
    use terminal_psi::{ClosedReachParameter, OperationKind};
    let original = closed_reach_module();
    let mutations: &[fn(&mut terminal_psi::TerminalModule)] = &[
        |module| module.machines[0].closed_reach_application = None,
        |module| {
            module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .dependencies
                .remove(0);
        },
        |module| {
            module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .calls
                .clear()
        },
        |module| module.machines[0].blocks[0].operations[0].static_reach_binding = None,
        |module| module.machines[0].blocks[0].operations[0].static_reach_binding = Some(3),
        |module| {
            module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .dependencies = vec![0]
        },
        |module| {
            module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .dependencies = vec![1, 1]
        },
        |module| {
            module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .dependencies
                .clear()
        },
        |module| {
            module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .telescope
                .remove(0);
        },
        |module| {
            let ClosedReachParameter::Machine(binding) = &mut module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .telescope[1]
            else {
                panic!("fixture");
            };
            binding.selected_reach.clear();
        },
        |module| {
            let ClosedReachParameter::Machine(binding) = &mut module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .telescope[1]
            else {
                panic!("fixture");
            };
            binding.upper_bound.clear();
        },
        |module| {
            let ClosedReachParameter::Machine(binding) = &mut module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .telescope[1]
            else {
                panic!("fixture");
            };
            binding.nominal_requirement = None;
        },
        |module| {
            let ClosedReachParameter::Machine(binding) = &mut module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .telescope[1]
            else {
                panic!("fixture");
            };
            binding.callee = None;
        },
        |module| {
            let OperationKind::CallUnit { callee, .. } =
                &mut module.machines[0].blocks[0].operations[0].kind
            else {
                panic!("fixture");
            };
            *callee = semantic_vocabulary::MachineId::new(3).unwrap();
        },
        |module| {
            module.machines[1].blocks[0].operations =
                std::mem::take(&mut module.machines[0].blocks[0].operations);
        },
    ];
    for (ordinal, mutate) in mutations.iter().enumerate() {
        let mut changed = original.clone();
        mutate(&mut changed);
        let bytes = super::encode_raw(&changed).expect("raw tampered product");
        assert!(
            decode_module(&bytes).is_err(),
            "mutation {ordinal} accepted"
        );
    }
}

#[test]
fn structural_callback_requirement_remains_fixed_beside_nominal_reach() {
    use terminal_psi::ClosedReachParameter;
    let mut module = closed_reach_module();
    let application = module.machines[0]
        .closed_reach_application
        .as_mut()
        .unwrap();
    application.dependencies = vec![3];
    application.fixed = vec![module.services[0].id];
    let ClosedReachParameter::Machine(binding) = &mut application.telescope[1] else {
        panic!("fixture");
    };
    binding.nominal_requirement = None;
    binding.selected_reach.clear();
    module.machines[1].declared_service_reach.clear();
    module.machines[1].published_service_ceiling.clear();
    let bytes = encode_module(&module).unwrap();
    assert_eq!(decode_module(&bytes).unwrap(), module);
    module.machines[0]
        .closed_reach_application
        .as_mut()
        .unwrap()
        .fixed
        .clear();
    // The nominal selection still supplies Console. Matching the output union
    // cannot excuse omitting the structural callback's fixed requirement row.
    assert!(decode_module(&super::encode_raw(&module).unwrap()).is_err());
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
            static_reach_binding: None,
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

fn schema_reach_module() -> terminal_psi::TerminalModule {
    use terminal_psi::{
        ClosedReachApplication, ClosedReachArgument, ClosedReachCallApplication,
        ClosedReachParameter, ClosedReachSchema,
    };
    let mut module = closed_reach_module();
    let service = module.services[0].id;
    let application = module.machines[0]
        .closed_reach_application
        .as_mut()
        .unwrap();
    application.telescope.pop();
    application.dependencies = vec![1];
    let ClosedReachParameter::Machine(binding) = &mut application.telescope[1] else {
        panic!("schema fixture machine binder");
    };
    binding.selected_identity = "schema|selected=schema::entry".into();
    binding.callee = None;
    binding.schema = Some(ClosedReachSchema {
        template_identity: "schema".into(),
        template_commitment: [4; 32],
    });
    for (call, ordinal) in application.calls.iter_mut().zip([2_u8, 3]) {
        call.binder = 1;
        call.application = Some(ClosedReachCallApplication {
            callee: semantic_vocabulary::MachineId::new(u64::from(ordinal)).unwrap(),
            specialization_commitment: [ordinal; 32],
            arguments: vec![ClosedReachArgument::Const(format!("u64:{ordinal}"))],
        });
    }
    for operation in &mut module.machines[0].blocks[0].operations {
        operation.static_reach_binding = Some(1);
    }
    for (machine, ordinal) in module.machines[1..].iter_mut().zip([2_u8, 3]) {
        machine.closed_reach_application = Some(ClosedReachApplication {
            template_identity: "schema".into(),
            template_commitment: [4; 32],
            specialization_commitment: [ordinal; 32],
            telescope: vec![ClosedReachParameter::Const {
                argument: format!("u64:{ordinal}"),
            }],
            fixed: vec![service],
            dependencies: Vec::new(),
            calls: Vec::new(),
        });
    }
    module
}

#[test]
fn schema_reach_calls_roundtrip_distinct_tuples_and_reject_every_truncation() {
    let module = schema_reach_module();
    let bytes = encode_module(&module).expect("two applications of one schema encode");
    assert_eq!(decode_module(&bytes).unwrap(), module);
    for length in 0..bytes.len() {
        assert!(
            decode_module(&bytes[..length]).is_err(),
            "schema truncation at {length}"
        );
    }
}

#[test]
fn schema_reach_decoder_rejects_changed_template_tuple_and_callee_joins() {
    use terminal_psi::{ClosedReachArgument, ClosedReachParameter, OperationKind};
    let original = schema_reach_module();
    encode_module(&original).expect("unmodified schema fixture must validate");
    let mutations: &[fn(&mut terminal_psi::TerminalModule)] = &[
        |module| {
            module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .calls[0]
                .application = None
        },
        |module| {
            let ClosedReachParameter::Machine(binding) = &mut module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .telescope[1]
            else {
                panic!("fixture");
            };
            binding.schema = None;
        },
        |module| {
            let ClosedReachParameter::Machine(binding) = &mut module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .telescope[1]
            else {
                panic!("fixture");
            };
            binding.callee = Some(semantic_vocabulary::MachineId::new(2).unwrap());
        },
        |module| {
            let ClosedReachParameter::Machine(binding) = &mut module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .telescope[1]
            else {
                panic!("fixture");
            };
            binding.selected_identity = "other|selected=schema::entry".into();
        },
        |module| {
            let ClosedReachParameter::Machine(binding) = &mut module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .telescope[1]
            else {
                panic!("fixture");
            };
            binding.selected_identity = "schema|selected=".into();
        },
        |module| {
            let ClosedReachParameter::Machine(binding) = &mut module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .telescope[1]
            else {
                panic!("fixture");
            };
            binding.schema.as_mut().unwrap().template_commitment = [0; 32];
        },
        |module| {
            // Keep both header copies in agreement: the selected owner must
            // still prevent a coherent substitution of an unrelated template.
            let ClosedReachParameter::Machine(binding) = &mut module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .telescope[1]
            else {
                panic!("fixture");
            };
            binding.schema.as_mut().unwrap().template_identity = "other".into();
            for machine in &mut module.machines[1..] {
                machine
                    .closed_reach_application
                    .as_mut()
                    .unwrap()
                    .template_identity = "other".into();
            }
        },
        |module| {
            module.machines[1]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .template_identity = "other".into()
        },
        |module| {
            module.machines[1]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .template_commitment = [5; 32]
        },
        |module| module.machines[1].closed_reach_application = None,
        |module| {
            module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .calls[0]
                .application
                .as_mut()
                .unwrap()
                .specialization_commitment = [3; 32]
        },
        |module| {
            module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .calls[0]
                .application
                .as_mut()
                .unwrap()
                .arguments
                .clear()
        },
        |module| {
            module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .calls[0]
                .application
                .as_mut()
                .unwrap()
                .arguments[0] = ClosedReachArgument::Const("u64:3".into())
        },
        |module| {
            module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .calls[0]
                .application
                .as_mut()
                .unwrap()
                .arguments[0] = ClosedReachArgument::Type("u64:2".into())
        },
        |module| {
            module.machines[1]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .telescope[0] = ClosedReachParameter::Const {
                argument: "u64:3".into(),
            }
        },
        |module| {
            let OperationKind::CallUnit { callee, .. } =
                &mut module.machines[0].blocks[0].operations[0].kind
            else {
                panic!("fixture");
            };
            *callee = semantic_vocabulary::MachineId::new(3).unwrap();
        },
        |module| {
            // Redirect both executable and roster callees, but retain the
            // expected original tuple and commitment.
            let redirected = semantic_vocabulary::MachineId::new(3).unwrap();
            let OperationKind::CallUnit { callee, .. } =
                &mut module.machines[0].blocks[0].operations[0].kind
            else {
                panic!("fixture");
            };
            *callee = redirected;
            module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .calls[0]
                .application
                .as_mut()
                .unwrap()
                .callee = redirected;
        },
        |module| {
            let ClosedReachParameter::Machine(binding) = &mut module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .telescope[1]
            else {
                panic!("fixture");
            };
            binding.selected_reach.clear();
            module.machines[0]
                .closed_reach_application
                .as_mut()
                .unwrap()
                .fixed = vec![module.services[0].id];
        },
    ];
    for (ordinal, mutate) in mutations.iter().enumerate() {
        let mut changed = original.clone();
        mutate(&mut changed);
        let bytes = super::encode_raw(&changed).expect("raw schema corruption encodes");
        assert!(
            decode_module(&bytes).is_err(),
            "schema mutation {ordinal} accepted"
        );
    }
}

#[test]
fn nominal_schema_dependency_follows_helpers_but_not_disconnected_applications() {
    use semantic_vocabulary::{BlockId, ContractId, EdgeId, MachineId, OperationId};
    use terminal_psi::{Operation, OperationKind, OperationResult, Terminator};
    let mut module = schema_reach_module();
    let mut helper = module.machines[0].clone();
    helper.id = MachineId::new(4).unwrap();
    helper.entry = BlockId::new(4).unwrap();
    helper.contract.id = ContractId::new(4).unwrap();
    helper.blocks[0].id = helper.entry;
    helper.blocks[0].terminator = Terminator::ReturnUnit {
        edge: EdgeId::new(4).unwrap(),
        trivial_affine_discards: Vec::new(),
    };
    helper
        .closed_reach_application
        .as_mut()
        .unwrap()
        .template_identity = "helper".into();
    helper
        .closed_reach_application
        .as_mut()
        .unwrap()
        .template_commitment = [6; 32];
    helper
        .closed_reach_application
        .as_mut()
        .unwrap()
        .specialization_commitment = [7; 32];
    let owner = &mut module.machines[0];
    owner
        .closed_reach_application
        .as_mut()
        .unwrap()
        .calls
        .clear();
    owner.blocks[0].operations = vec![Operation {
        id: OperationId::new(3).unwrap(),
        static_reach_binding: None,
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: helper.id,
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    }];
    module.machines.push(helper);
    let bytes = encode_module(&module).expect("nominal schema dependency forwards through helper");
    assert_eq!(decode_module(&bytes).unwrap(), module);

    module.machines[0].blocks[0].operations.clear();
    // Preserve ordinary concrete reach using a declaration, isolating the
    // missing schema application coverage from ordinary service-closure errors.
    module.machines[0].declared_service_reach = vec![module.services[0].id];
    assert!(matches!(
        terminal_verifier::validate_module_representation(&module),
        Err(terminal_verifier::ModuleError::InvalidClosedReachApplication { .. })
    ));
    let disconnected = super::encode_raw(&module).expect("disconnected schema fixture encodes raw");
    assert!(decode_module(&disconnected).is_err());
}
