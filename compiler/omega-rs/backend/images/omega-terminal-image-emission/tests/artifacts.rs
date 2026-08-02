use omega_object_file::{SectionKind, SymbolKind, object_symbol_name};
use omega_target::NativeTarget;
use omega_terminal_image_emission::{
    SelectedProviderPlanIdentity, TerminalInstallationError, TerminalObjectError,
    build_terminal_installation_record, build_terminal_object_artifact,
    can_emit_terminal_executable_image, decode_terminal_installation_record,
    emit_terminal_executable_image, emit_terminal_object_container,
    encode_terminal_installation_record, terminal_installation_fingerprint,
    validate_terminal_installation_record,
};
use omega_terminal_machine_code::{TerminalMachineCodeFunction, TerminalMachineCodePlan};
use omega_terminal_target_operations::TerminalPsiProvenance;
use psi_core::{EdgeId, MachineId, OperationId, ProfileDecisionId};
use psi_terminal::{SemanticFingerprint, SemanticVersion, TerminalPsiIdentity};

#[test]
fn object_artifact_owns_canonical_function_spans_and_psi_provenance() {
    let plan = two_function_plan();
    let artifact = build_terminal_object_artifact(&plan).expect("terminal object artifact");

    assert_eq!(artifact.terminal_psi(), plan.terminal_psi);
    assert_eq!(artifact.target(), plan.target);
    assert_eq!(artifact.entry(), machine_id(2));
    assert_eq!(artifact.relocations().record_count(), 0);
    assert_eq!(artifact.functions().len(), 2);
    assert_eq!(artifact.functions()[0].text_offset, 0);
    assert_eq!(artifact.functions()[0].byte_count, 6);
    assert_eq!(artifact.functions()[0].bytes(&artifact), &integer_return(3));
    assert_eq!(artifact.functions()[1].text_offset, 6);
    assert_eq!(artifact.functions()[1].bytes(&artifact), &integer_return(7));
    assert_eq!(
        artifact.functions()[1].provenance,
        TerminalPsiProvenance {
            operations: vec![operation_id(2)],
            edges: vec![edge_id(2)],
        }
    );

    let symbols = artifact
        .object()
        .layout
        .symbols
        .iter()
        .map(|(_, symbol)| symbol)
        .collect::<Vec<_>>();
    assert_eq!(symbols.len(), 2);
    assert_eq!(symbols[0].name, "omega_terminal_machine_1");
    assert_eq!(symbols[0].kind, SymbolKind::Function);
    assert_eq!(symbols[1].name, "main");
    assert_eq!(
        object_symbol_name(artifact.object(), artifact.object().layout.entry_symbol),
        "main"
    );
    let sections = artifact
        .object()
        .layout
        .sections
        .iter()
        .map(|(_, section)| section)
        .collect::<Vec<_>>();
    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0].kind, SectionKind::Text);
    assert_eq!(sections[0].size, 12);

    let container = emit_terminal_object_container(&artifact);
    assert_eq!(container.terminal_psi, plan.terminal_psi);
    assert_eq!(&container.output.bytes[..8], b"OMGOBJ\0\0");
    assert_eq!(container.output.text_bytes, 12);
    assert_eq!(container.output.data_bytes, 0);
    assert_eq!(container.output.bss_bytes, 0);
    assert_eq!(container.output.symbols, 2);
    assert_eq!(container.output.relocations, 0);
}

#[test]
fn object_boundary_rejects_noncanonical_or_incomplete_machine_code_plans() {
    let mut reordered = two_function_plan();
    reordered.functions.swap(0, 1);
    assert_eq!(
        build_terminal_object_artifact(&reordered),
        Err(TerminalObjectError::NonCanonicalFunctionOrder {
            previous: machine_id(2),
            current: machine_id(1),
        })
    );

    let mut missing_entry = two_function_plan();
    missing_entry.entry = machine_id(3);
    assert_eq!(
        build_terminal_object_artifact(&missing_entry),
        Err(TerminalObjectError::EntryFunctionMissing(machine_id(3)))
    );

    let mut empty_function = two_function_plan();
    empty_function.functions[0].bytes.clear();
    assert_eq!(
        build_terminal_object_artifact(&empty_function),
        Err(TerminalObjectError::EmptyFunction(machine_id(1)))
    );
}

#[test]
fn supported_writers_preserve_exact_terminal_text_and_complete_regions() {
    let targets = [
        (NativeTarget::linux_x64(), b"\x7fELF".as_slice()),
        (NativeTarget::linux_arm64(), b"\x7fELF".as_slice()),
        (NativeTarget::macos_arm64(), b"\xcf\xfa\xed\xfe".as_slice()),
        (NativeTarget::windows_x64(), b"MZ".as_slice()),
    ];

    for (target, magic) in targets {
        let bytes = match target.architecture {
            omega_target::Architecture::X86_64 => integer_return(7),
            omega_target::Architecture::Aarch64 => {
                vec![0xe0, 0x00, 0x80, 0x52, 0xc0, 0x03, 0x5f, 0xd6]
            }
        };
        let machine = machine_id(1);
        let plan = TerminalMachineCodePlan {
            terminal_psi: identity(),
            target,
            entry: machine,
            functions: vec![TerminalMachineCodeFunction {
                machine,
                provenance: TerminalPsiProvenance {
                    operations: vec![operation_id(1)],
                    edges: vec![edge_id(1)],
                },
                bytes: bytes.clone(),
            }],
        };
        let artifact = build_terminal_object_artifact(&plan).expect("artifact");
        let image = emit_terminal_executable_image(&artifact, 3)
            .unwrap_or_else(|error| panic!("{target:?} image failed: {error}"));
        assert_eq!(image.terminal_psi(), plan.terminal_psi);
        let installation =
            build_terminal_installation_record(&image, ProfileDecisionId::new(1).unwrap(), [])
                .expect("installation record");
        assert_eq!(
            installation.subsystem(),
            matches!(target.object_format, omega_target::ObjectFormat::Coff).then_some(3)
        );
        let installation_bytes =
            encode_terminal_installation_record(&installation).expect("installation bytes");
        assert_eq!(
            decode_terminal_installation_record(&installation_bytes),
            Ok(installation)
        );
        let image = image.output();

        assert!(image.bytes.starts_with(magic), "{target:?} image magic");
        assert_eq!(image.final_text_bytes, bytes, "{target:?} final text");
        assert_eq!(image.text_bytes, bytes.len());
        assert_eq!(image.relocations, 0);
        assert_eq!(image.final_image_imports, 0);
        assert_eq!(image.final_image_relocations, 0);
        assert!(image.executable_regions.unclassified_gaps.is_empty());
        assert_eq!(image.executable_regions.regions.len(), 1);
        assert_eq!(
            image.executable_regions.regions[0].symbol,
            artifact_symbol(&artifact)
        );
        let evidence = image
            .compiler_text_validation
            .expect("exact terminal text should publish validation evidence");
        assert_eq!(
            evidence.encoded_text_fingerprint,
            evidence.final_compiler_text_fingerprint
        );
        assert_eq!(evidence.text_relocation_count, 0);
        assert_eq!(evidence.checked_instruction_validation_count, 0);
    }
}

#[test]
fn installation_record_is_canonical_and_binds_exact_image_and_target_facts() {
    let plan = two_function_plan();
    let artifact = build_terminal_object_artifact(&plan).expect("artifact");
    let image = emit_terminal_executable_image(&artifact, 3).expect("Linux image");
    let provider_three = provider_id(3);
    let provider_nine = provider_id(9);
    let record = build_terminal_installation_record(
        &image,
        ProfileDecisionId::new(11).expect("profile decision"),
        [provider_nine, provider_three],
    )
    .expect("installation record");

    assert_eq!(record.terminal_psi(), plan.terminal_psi);
    assert_eq!(record.target(), plan.target);
    assert_eq!(record.subsystem(), None);
    assert_eq!(
        record.selected_provider_plans(),
        [provider_three, provider_nine]
    );
    let bytes = encode_terminal_installation_record(&record).expect("canonical bytes");
    assert_eq!(&bytes[..8], b"PSIINST\0");
    assert_eq!(
        decode_terminal_installation_record(&bytes),
        Ok(record.clone())
    );
    validate_terminal_installation_record(&record, &image).expect("exact image binding");
    assert_eq!(
        terminal_installation_fingerprint(&record)
            .expect("installation fingerprint")
            .to_string(),
        "7adc75f2910ab4268dfe3aa02ad44142ec98884a3df973645cbdb01b84a86344"
    );

    let mut changed_plan = plan;
    changed_plan.functions[1].bytes = integer_return(8);
    let changed_artifact = build_terminal_object_artifact(&changed_plan).expect("changed artifact");
    let changed_image =
        emit_terminal_executable_image(&changed_artifact, 3).expect("changed Linux image");
    assert_eq!(
        validate_terminal_installation_record(&record, &changed_image),
        Err(TerminalInstallationError::ImageBindingMismatch)
    );

    assert_eq!(
        build_terminal_installation_record(
            &image,
            ProfileDecisionId::new(11).unwrap(),
            [provider_three, provider_three],
        ),
        Err(TerminalInstallationError::DuplicateProviderPlan(
            provider_three
        ))
    );
}

#[test]
fn installation_decoder_rejects_alternate_and_malformed_encodings() {
    let artifact = build_terminal_object_artifact(&two_function_plan()).expect("artifact");
    let image = emit_terminal_executable_image(&artifact, 3).expect("image");
    let record = build_terminal_installation_record(
        &image,
        ProfileDecisionId::new(1).unwrap(),
        [provider_id(3), provider_id(9)],
    )
    .expect("record");
    let bytes = encode_terminal_installation_record(&record).expect("bytes");

    let mut future = bytes.clone();
    future[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        decode_terminal_installation_record(&future),
        Err(TerminalInstallationError::UnsupportedFormatVersion(2))
    );

    let mut reordered = bytes.clone();
    let provider_offset = reordered.len() - 16;
    reordered[provider_offset..provider_offset + 8].copy_from_slice(&9_u64.to_le_bytes());
    reordered[provider_offset + 8..].copy_from_slice(&3_u64.to_le_bytes());
    assert_eq!(
        decode_terminal_installation_record(&reordered),
        Err(TerminalInstallationError::NonCanonicalProviderPlanOrder)
    );

    let mut wrong_pointer_width = bytes.clone();
    wrong_pointer_width[48..56].copy_from_slice(&4_u64.to_le_bytes());
    assert!(matches!(
        decode_terminal_installation_record(&wrong_pointer_width),
        Err(TerminalInstallationError::UnsupportedTarget(_))
    ));

    let mut zero_profile = bytes.clone();
    zero_profile[68..76].copy_from_slice(&0_u64.to_le_bytes());
    assert_eq!(
        decode_terminal_installation_record(&zero_profile),
        Err(TerminalInstallationError::ZeroProfileDecision)
    );

    let mut zero_provider = bytes.clone();
    zero_provider[166..174].copy_from_slice(&0_u64.to_le_bytes());
    assert_eq!(
        decode_terminal_installation_record(&zero_provider),
        Err(TerminalInstallationError::ZeroProviderPlan)
    );

    assert_eq!(
        decode_terminal_installation_record(&bytes[..bytes.len() - 1]),
        Err(TerminalInstallationError::UnexpectedEnd)
    );

    let mut trailing = bytes;
    trailing.push(0);
    assert_eq!(
        decode_terminal_installation_record(&trailing),
        Err(TerminalInstallationError::TrailingBytes(1))
    );
}

#[test]
fn image_boundary_rejects_noncanonical_pointer_facts() {
    let mut plan = two_function_plan();
    plan.target.pointer_size = 4;
    assert!(!can_emit_terminal_executable_image(plan.target));
    let artifact = build_terminal_object_artifact(&plan).expect("owned artifact");
    assert!(emit_terminal_executable_image(&artifact, 3).is_err());
}

fn artifact_symbol(artifact: &omega_terminal_image_emission::TerminalObjectArtifact) -> &str {
    object_symbol_name(artifact.object(), artifact.entry_function().symbol)
}

fn two_function_plan() -> TerminalMachineCodePlan {
    TerminalMachineCodePlan {
        terminal_psi: identity(),
        target: NativeTarget::linux_x64(),
        entry: machine_id(2),
        functions: vec![
            TerminalMachineCodeFunction {
                machine: machine_id(1),
                provenance: TerminalPsiProvenance {
                    operations: vec![operation_id(1)],
                    edges: vec![edge_id(1)],
                },
                bytes: integer_return(3),
            },
            TerminalMachineCodeFunction {
                machine: machine_id(2),
                provenance: TerminalPsiProvenance {
                    operations: vec![operation_id(2)],
                    edges: vec![edge_id(2)],
                },
                bytes: integer_return(7),
            },
        ],
    }
}

fn integer_return(value: u8) -> Vec<u8> {
    vec![0xb8, value, 0, 0, 0, 0xc3]
}

fn machine_id(raw: u64) -> MachineId {
    MachineId::new(raw).expect("machine")
}

fn operation_id(raw: u64) -> OperationId {
    OperationId::new(raw).expect("operation")
}

fn edge_id(raw: u64) -> EdgeId {
    EdgeId::new(raw).expect("edge")
}

fn provider_id(raw: u64) -> SelectedProviderPlanIdentity {
    SelectedProviderPlanIdentity::new(raw).expect("provider plan")
}

fn identity() -> TerminalPsiIdentity {
    TerminalPsiIdentity {
        semantic_version: SemanticVersion::CURRENT,
        program_fingerprint: SemanticFingerprint::from_bytes([9; 32]),
    }
}
