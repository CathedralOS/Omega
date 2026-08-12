use omega_object_file::{
    RelocationKind, RelocationOrigin, SectionKind, SymbolKind, object_symbol_name,
};
use omega_target::NativeTarget;
use omega_terminal_image_emission::{
    TerminalInstallationError, TerminalObjectError, build_terminal_installation_record,
    build_terminal_object_artifact, can_emit_terminal_executable_image,
    decode_terminal_installation_record, emit_terminal_executable_image,
    emit_terminal_object_container, encode_terminal_installation_record,
    terminal_installation_fingerprint, validate_terminal_installation_record,
};
use omega_terminal_machine_code::{
    TerminalBoundarySettlementRecord, TerminalInternalCallRelocation, TerminalMachineCodeFunction,
    TerminalMachineCodePlan, TerminalNativeFuelAttribution, TerminalNativeFuelSite,
    TerminalPortEffectRecord,
};
use omega_terminal_target_operations::{
    TerminalMetadataOnlyPortRealization, TerminalProviderExecutionBinding,
    TerminalProviderPlanIdentity, TerminalPsiProvenance,
};
use psi_core::{BoundaryMachineId, EdgeId, MachineId, OperationId, ProfileDecisionId, ServiceId};
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

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
    assert_eq!(&container.output.bytes[8..12], &5_u32.to_le_bytes());
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
fn x86_internal_call_is_a_typed_relocation_and_the_only_final_text_mutation() {
    let mut plan = internal_call_plan(NativeTarget::linux_x64());
    let full_width_operation = operation_id(u64::from(u32::MAX) + 1);
    plan.functions[1].provenance.operations[0] = full_width_operation;
    plan.functions[1].internal_calls[0].psi_operation = full_width_operation;
    let artifact = build_terminal_object_artifact(&plan).expect("terminal object artifact");

    assert_eq!(artifact.relocations().record_count(), 1);
    let (_, relocation) = artifact.relocations().records().next().expect("relocation");
    assert_eq!(relocation.kind, RelocationKind::X86_64Relative32);
    assert_eq!(relocation.section, SectionKind::Text);
    assert_eq!(relocation.offset, 7);
    assert_eq!(relocation.byte_width, 4);
    assert_eq!(relocation.symbol_handle, artifact.functions()[0].symbol);
    assert_eq!(
        relocation.origin.semantic_operation_identity(),
        Some(u64::from(u32::MAX) + 1)
    );
    assert_eq!(
        relocation.origin,
        RelocationOrigin::SemanticOperation {
            function_symbol_handle: artifact.functions()[1].symbol,
            operation_identity: u64::from(u32::MAX) + 1,
        }
    );

    let container = emit_terminal_object_container(&artifact);
    assert_eq!(container.output.relocations, 1);
    let image = emit_terminal_executable_image(&artifact, 3).expect("Linux x86-64 image");
    let output = image.output();
    assert_eq!(&output.final_text_bytes[7..11], &[0xf5, 0xff, 0xff, 0xff]);
    assert_eq!(output.final_image_relocations, 1);
    let evidence = output
        .compiler_text_validation
        .expect("relocation evidence");
    assert_ne!(
        evidence.encoded_text_fingerprint,
        evidence.final_compiler_text_fingerprint
    );
    assert_eq!(evidence.text_relocation_count, 1);

    let record = build_terminal_installation_record(&image, ProfileDecisionId::new(1).unwrap())
        .expect("installation record");
    validate_terminal_installation_record(&record, &image).expect("image binding");
}

#[test]
fn aarch64_internal_call_patches_only_the_branch_immediate() {
    let plan = internal_call_plan(NativeTarget::linux_arm64());
    let artifact = build_terminal_object_artifact(&plan).expect("terminal object artifact");
    let (_, relocation) = artifact.relocations().records().next().expect("relocation");
    assert_eq!(relocation.kind, RelocationKind::Aarch64Branch26);
    assert_eq!(relocation.offset, 8);

    let image = emit_terminal_executable_image(&artifact, 3).expect("Linux AArch64 image");
    let output = image.output();
    assert_eq!(&output.final_text_bytes[8..12], &[0xfe, 0xff, 0xff, 0x97]);
    assert_eq!(output.final_image_relocations, 1);
    assert_eq!(
        output
            .compiler_text_validation
            .expect("relocation evidence")
            .text_relocation_count,
        1
    );
}

#[test]
fn object_boundary_rejects_unproved_internal_call_relocations() {
    let mut unknown_target = internal_call_plan(NativeTarget::linux_x64());
    unknown_target.functions[1].internal_calls[0].target = machine_id(3);
    assert_eq!(
        build_terminal_object_artifact(&unknown_target),
        Err(TerminalObjectError::UnknownInternalCallTarget {
            caller: machine_id(2),
            target: machine_id(3),
        })
    );

    let mut invalid_site = internal_call_plan(NativeTarget::linux_x64());
    invalid_site.functions[1].bytes[0] = 0x90;
    assert_eq!(
        build_terminal_object_artifact(&invalid_site),
        Err(TerminalObjectError::InvalidInternalCallSite {
            caller: machine_id(2),
            operation: operation_id(2),
            offset: 1,
        })
    );

    let mut duplicate_site = internal_call_plan(NativeTarget::linux_x64());
    let duplicate_call = duplicate_site.functions[1].internal_calls[0];
    duplicate_site.functions[1]
        .internal_calls
        .push(duplicate_call);
    assert_eq!(
        build_terminal_object_artifact(&duplicate_site),
        Err(TerminalObjectError::NonCanonicalInternalCallOrder(
            machine_id(2)
        ))
    );

    let mut missing_provenance = internal_call_plan(NativeTarget::linux_x64());
    missing_provenance.functions[1]
        .provenance
        .operations
        .clear();
    assert_eq!(
        build_terminal_object_artifact(&missing_provenance),
        Err(TerminalObjectError::InternalCallOperationNotInProvenance {
            caller: machine_id(2),
            operation: operation_id(2),
        })
    );

    let mut duplicate_operation = internal_call_plan(NativeTarget::linux_x64());
    duplicate_operation.functions[1].bytes = vec![0xe8, 0, 0, 0, 0, 0xe8, 0, 0, 0, 0, 0xc3];
    duplicate_operation.functions[1]
        .internal_calls
        .push(TerminalInternalCallRelocation {
            psi_operation: operation_id(2),
            target: machine_id(1),
            offset: 6,
        });
    assert_eq!(
        build_terminal_object_artifact(&duplicate_operation),
        Err(TerminalObjectError::DuplicateInternalCallOperation {
            caller: machine_id(2),
            operation: operation_id(2),
        })
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
                internal_calls: Vec::new(),
                fuel_attribution: Vec::new(),
                port_effects: Vec::new(),
                boundary_settlements: Vec::new(),
            }],
        };
        let artifact = build_terminal_object_artifact(&plan).expect("artifact");
        let image = emit_terminal_executable_image(&artifact, 3)
            .unwrap_or_else(|error| panic!("{target:?} image failed: {error}"));
        assert_eq!(image.terminal_psi(), plan.terminal_psi);
        let installation =
            build_terminal_installation_record(&image, ProfileDecisionId::new(1).unwrap())
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
    let record = build_terminal_installation_record(
        &image,
        ProfileDecisionId::new(11).expect("profile decision"),
    )
    .expect("installation record");

    assert_eq!(record.terminal_psi(), plan.terminal_psi);
    assert_eq!(record.target(), plan.target);
    assert_eq!(record.subsystem(), None);
    assert!(record.selected_provider_plans().is_empty());
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
        "f1a637482775555fd523fdd5386ca8b09cc4a66cd39adaa3090d5d10e1798b14"
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
}

#[test]
fn installation_decoder_rejects_alternate_and_malformed_encodings() {
    let artifact = build_terminal_object_artifact(&two_function_plan()).expect("artifact");
    let image = emit_terminal_executable_image(&artifact, 3).expect("image");
    let record = build_terminal_installation_record(&image, ProfileDecisionId::new(1).unwrap())
        .expect("record");
    let bytes = encode_terminal_installation_record(&record).expect("bytes");

    let mut future = bytes.clone();
    future[8..10].copy_from_slice(&4_u16.to_le_bytes());
    assert_eq!(
        decode_terminal_installation_record(&future),
        Err(TerminalInstallationError::UnsupportedFormatMarker(4))
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
fn privileged_effect_and_exact_provider_execution_survive_installation() {
    let port_operation = operation_id(1);
    let settlement_operation = operation_id(2);
    let service = ServiceId::new(1).unwrap();
    let boundary = BoundaryMachineId::new(1).unwrap();
    let provider_plan = TerminalProviderPlanIdentity::new(7).unwrap();
    let provider_execution =
        TerminalProviderExecutionBinding::from_execution_record(provider_plan, 8, 9, 10, 11)
            .unwrap();
    let realization = TerminalMetadataOnlyPortRealization {
        effect_operation: port_operation,
        service,
        port: 0x20,
        value: 0x20,
    };
    let mut bytes = omega_x86_encoding::encode_immediate_port_write(0x20, 0x20).to_vec();
    bytes.push(0xc3);
    let plan = TerminalMachineCodePlan {
        terminal_psi: identity(),
        target: NativeTarget::linux_x64(),
        entry: machine_id(1),
        functions: vec![TerminalMachineCodeFunction {
            machine: machine_id(1),
            provenance: TerminalPsiProvenance {
                operations: vec![port_operation, settlement_operation],
                edges: vec![edge_id(1)],
            },
            bytes,
            internal_calls: Vec::new(),
            fuel_attribution: vec![
                TerminalNativeFuelAttribution {
                    schedule: psi_core::FuelScheduleIdentity::new(1).unwrap(),
                    site: TerminalNativeFuelSite::Operation(port_operation),
                    units: 1,
                    operation_ordinal: 0,
                    code_offset: 0,
                    byte_count: 27,
                },
                TerminalNativeFuelAttribution {
                    schedule: psi_core::FuelScheduleIdentity::new(1).unwrap(),
                    site: TerminalNativeFuelSite::Operation(settlement_operation),
                    units: 1,
                    operation_ordinal: 1,
                    code_offset: 27,
                    byte_count: 0,
                },
                TerminalNativeFuelAttribution {
                    schedule: psi_core::FuelScheduleIdentity::new(1).unwrap(),
                    site: TerminalNativeFuelSite::Edge(edge_id(1)),
                    units: 1,
                    operation_ordinal: 2,
                    code_offset: 27,
                    byte_count: 1,
                },
            ],
            port_effects: vec![TerminalPortEffectRecord {
                psi_operation: port_operation,
                service,
                port: 0x20,
                value: 0x20,
                operation_ordinal: 0,
                code_offset: 0,
                byte_count: 27,
            }],
            boundary_settlements: vec![TerminalBoundarySettlementRecord {
                psi_operation: settlement_operation,
                boundary,
                provider_execution: provider_execution.into(),
                realization,
                argument_places: Vec::new(),
                claim_settlements: Vec::new(),
                operation_ordinal: 1,
                code_offset: 27,
            }],
        }],
    };
    let artifact = build_terminal_object_artifact(&plan).expect("effect artifact");
    assert_eq!(artifact.fuel_attribution().len(), 3);
    assert_eq!(artifact.fuel_attribution()[1].attribution.byte_count, 0);
    assert_eq!(artifact.port_effects()[0].effect.service, service);
    assert_eq!(
        artifact.boundary_settlements()[0].settlement.realization,
        realization
    );
    let image = emit_terminal_executable_image(&artifact, 3).expect("effect image");
    assert_eq!(image.fuel_attribution(), artifact.fuel_attribution());
    assert_eq!(
        build_terminal_installation_record(&image, ProfileDecisionId::new(1).unwrap()),
        Err(TerminalInstallationError::ProviderExecutionClosureMismatch)
    );

    let mut wrong_bytes = plan.clone();
    wrong_bytes.functions[0].bytes[0] ^= 1;
    assert!(matches!(
        build_terminal_object_artifact(&wrong_bytes),
        Err(TerminalObjectError::PortEffectBytesMismatch { .. })
    ));
    let mut wrong_schedule = plan.clone();
    wrong_schedule.functions[0].fuel_attribution[0].schedule =
        psi_core::FuelScheduleIdentity::new(2).unwrap();
    assert_eq!(
        build_terminal_object_artifact(&wrong_schedule),
        Err(TerminalObjectError::InvalidFuelAttribution(machine_id(1)))
    );
    let mut wrong_realization = plan;
    wrong_realization.functions[0].boundary_settlements[0]
        .realization
        .value = 0x21;
    assert!(matches!(
        build_terminal_object_artifact(&wrong_realization),
        Err(TerminalObjectError::BoundaryRealizationMismatch { .. })
    ));
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
                internal_calls: Vec::new(),
                fuel_attribution: Vec::new(),
                port_effects: Vec::new(),
                boundary_settlements: Vec::new(),
            },
            TerminalMachineCodeFunction {
                machine: machine_id(2),
                provenance: TerminalPsiProvenance {
                    operations: vec![operation_id(2)],
                    edges: vec![edge_id(2)],
                },
                bytes: integer_return(7),
                internal_calls: Vec::new(),
                fuel_attribution: Vec::new(),
                port_effects: Vec::new(),
                boundary_settlements: Vec::new(),
            },
        ],
    }
}

fn internal_call_plan(target: NativeTarget) -> TerminalMachineCodePlan {
    let (callee, caller, call_offset) = match target.architecture {
        omega_target::Architecture::X86_64 => (integer_return(3), vec![0xe8, 0, 0, 0, 0, 0xc3], 1),
        omega_target::Architecture::Aarch64 => (
            vec![0x60, 0x00, 0x80, 0x52, 0xc0, 0x03, 0x5f, 0xd6],
            vec![0x00, 0x00, 0x00, 0x94, 0xc0, 0x03, 0x5f, 0xd6],
            0,
        ),
    };
    TerminalMachineCodePlan {
        terminal_psi: identity(),
        target,
        entry: machine_id(2),
        functions: vec![
            TerminalMachineCodeFunction {
                machine: machine_id(1),
                provenance: TerminalPsiProvenance {
                    operations: vec![operation_id(1)],
                    edges: vec![edge_id(1)],
                },
                bytes: callee,
                internal_calls: Vec::new(),
                fuel_attribution: Vec::new(),
                port_effects: Vec::new(),
                boundary_settlements: Vec::new(),
            },
            TerminalMachineCodeFunction {
                machine: machine_id(2),
                provenance: TerminalPsiProvenance {
                    operations: vec![operation_id(2)],
                    edges: vec![edge_id(2)],
                },
                bytes: caller,
                internal_calls: vec![TerminalInternalCallRelocation {
                    psi_operation: operation_id(2),
                    target: machine_id(1),
                    offset: call_offset,
                }],
                fuel_attribution: Vec::new(),
                port_effects: Vec::new(),
                boundary_settlements: Vec::new(),
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

fn identity() -> TerminalPsiIdentity {
    TerminalPsiIdentity {
        vocabulary_marker: VocabularyMarker::CURRENT,
        program_fingerprint: SemanticFingerprint::from_bytes([9; 32]),
    }
}
