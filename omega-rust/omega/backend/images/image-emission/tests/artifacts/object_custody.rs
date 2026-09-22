use super::{
    WriteExitProvider, admitted_x86_fma_provider, edge_id, identity, integer_return,
    linux_write_line_exit_plan, machine_id, operation_id, refresh_x86_fma_identity,
    two_function_plan, x86_fma_plan,
};
use crate::hosted_exit_runtime;
use calling_conventions::{ValuePlacement, ValueShape};
use image_emission::{
    InstallationError, ObjectError, build_admitted_x86_fma_object_artifact,
    build_feature_required_x86_fma_object_artifact, build_installation_record,
    build_installation_record_with_provider_executions, build_object_artifact,
    decode_installation_record, emit_direct_executable_image, emit_object_container,
    encode_installation_record, installation_fingerprint, validate_installation_record,
};
use machine_code::{
    BoundarySettlementRecord, MachineCodeFunction, MachineCodePlan, SemanticCodeAttribution,
    SemanticCodeSite, X86ScalarFmaFormat,
};
use object_file::{SectionKind, SymbolKind, object_symbol_name};
use semantic_vocabulary::{
    BoundaryMachineId, ClaimId, PlaceId, ProfileDecisionId, StructuralTypeId,
};
use target::{NativeTarget, TargetProfile, X86FeatureRequirement, X86ScalarFmaSlot};
use target_operations::{
    BoundaryRealization, BoundaryScalarArgument, CompletionClaimSource,
    HostedExitProcessI32Realization, TerminalPsiProvenance,
};
use terminal_psi::{CompletionReceipt, EntryClaim, StructuralAccess, StructuralArgument};

#[test]
fn object_artifact_owns_canonical_function_spans_and_psi_provenance() {
    let plan = two_function_plan();
    let artifact = build_object_artifact(&plan).expect("terminal object artifact");

    assert_eq!(artifact.psi(), plan.psi);
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

    let container = emit_object_container(&artifact);
    assert_eq!(container.psi, plan.psi);
    assert_eq!(&container.output.bytes[..8], b"OMGOBJ\0\0");
    assert_eq!(&container.output.bytes[8..12], &6_u32.to_le_bytes());
    assert_eq!(container.output.text_bytes, 12);
    assert_eq!(container.output.data_bytes, 0);
    assert_eq!(container.output.bss_bytes, 0);
    assert_eq!(container.output.symbols, 2);
    assert_eq!(container.output.relocations, 0);
}

#[test]
fn source_free_x86_fma_object_replays_exact_feature_profile_and_instruction_custody() {
    for (profile, format) in [
        (TargetProfile::LinuxX64, X86ScalarFmaFormat::Binary32),
        (TargetProfile::WindowsX64, X86ScalarFmaFormat::Binary64),
        (TargetProfile::UefiX64, X86ScalarFmaFormat::Binary32),
    ] {
        let plan = x86_fma_plan(profile, format);
        assert!(matches!(
            build_object_artifact(&plan),
            Err(ObjectError::MissingX86ScalarFmaProfile(_))
        ));
        let artifact = build_feature_required_x86_fma_object_artifact(&plan, profile)
            .expect("feature-requiring scalar FMA object");
        assert_eq!(artifact.x86_feature_profile(), Some(profile));
        assert_eq!(
            artifact.functions()[0].x86_scalar_fma,
            plan.functions[0].x86_scalar_fma
        );
        assert_eq!(
            artifact.functions()[0].bytes(&artifact),
            &plan.functions[0].bytes
        );
        assert!(
            emit_direct_executable_image(&artifact, 3).is_err(),
            "retained requirements are not hardware admission"
        );
    }
}

#[test]
fn admitted_x86_fma_provider_selects_generic_slot_and_reaches_an_exact_image() {
    for (profile, format, slot) in [
        (
            TargetProfile::LinuxX64,
            X86ScalarFmaFormat::Binary32,
            X86ScalarFmaSlot::Binary32,
        ),
        (
            TargetProfile::WindowsX64,
            X86ScalarFmaFormat::Binary64,
            X86ScalarFmaSlot::Binary64,
        ),
    ] {
        let plan = x86_fma_plan(profile, format);
        let provider = admitted_x86_fma_provider(profile);
        assert!(provider.admits(plan.functions[0].x86_scalar_fma[0].requirement, slot));
        let artifact = build_admitted_x86_fma_object_artifact(&plan, provider)
            .expect("feature-qualified generic FMA object");
        assert_eq!(artifact.x86_scalar_fma_provider(), Some(provider));
        let image = emit_direct_executable_image(&artifact, 3)
            .expect("feature-qualified generic FMA object should reach exact image emission");
        assert_eq!(image.x86_scalar_fma_provider(), Some(provider));
        image_emission::validate_direct_executable_image(&artifact, &image)
            .expect("image replay must retain exact FMA admission");
    }
}

#[test]
fn admitted_x86_fma_object_rejects_profile_and_slot_custody_drift() {
    let linux_plan = x86_fma_plan(TargetProfile::LinuxX64, X86ScalarFmaFormat::Binary32);
    let windows_provider = admitted_x86_fma_provider(TargetProfile::WindowsX64);
    assert_eq!(
        build_admitted_x86_fma_object_artifact(&linux_plan, windows_provider),
        Err(ObjectError::InvalidX86ScalarFmaProviderAdmission)
    );

    let feature_only =
        build_feature_required_x86_fma_object_artifact(&linux_plan, TargetProfile::LinuxX64)
            .unwrap();
    assert!(emit_direct_executable_image(&feature_only, 3).is_err());
}

#[test]
fn source_free_x86_fma_object_rejects_stripped_and_mutated_custody() {
    let baseline = x86_fma_plan(TargetProfile::LinuxX64, X86ScalarFmaFormat::Binary32);

    let mut stripped = baseline.clone();
    stripped.functions[0].x86_scalar_fma.clear();
    assert!(matches!(
        build_object_artifact(&stripped),
        Err(ObjectError::MissingX86ScalarFmaCustody { .. })
    ));
    assert_eq!(
        build_feature_required_x86_fma_object_artifact(&stripped, TargetProfile::LinuxX64),
        Err(ObjectError::MissingX86ScalarFmaFragment)
    );

    let mut candidates = Vec::new();
    let mut changed = baseline.clone();
    changed.functions[0].bytes[3] = 0x98;
    candidates.push(changed);
    let mut changed = baseline.clone();
    changed.functions[0].x86_scalar_fma[0].format = X86ScalarFmaFormat::Binary64;
    refresh_x86_fma_identity(&mut changed.functions[0].x86_scalar_fma[0]);
    candidates.push(changed);
    let mut changed = baseline.clone();
    changed.functions[0].x86_scalar_fma[0].destination =
        calling_conventions::MachineRegister::X86Xmm(3);
    refresh_x86_fma_identity(&mut changed.functions[0].x86_scalar_fma[0]);
    candidates.push(changed);
    let mut changed = baseline.clone();
    changed.functions[0].x86_scalar_fma[0].addend = calling_conventions::MachineRegister::X86Xmm(4);
    refresh_x86_fma_identity(&mut changed.functions[0].x86_scalar_fma[0]);
    candidates.push(changed);
    let mut changed = baseline.clone();
    changed.functions[0].x86_scalar_fma[0].multiplicand =
        calling_conventions::MachineRegister::X86Xmm(5);
    refresh_x86_fma_identity(&mut changed.functions[0].x86_scalar_fma[0]);
    candidates.push(changed);
    let mut changed = baseline.clone();
    changed.functions[0].x86_scalar_fma[0].code_offset = 1;
    refresh_x86_fma_identity(&mut changed.functions[0].x86_scalar_fma[0]);
    candidates.push(changed);
    let mut changed = baseline.clone();
    changed.functions[0].x86_scalar_fma[0].byte_count = 4;
    refresh_x86_fma_identity(&mut changed.functions[0].x86_scalar_fma[0]);
    candidates.push(changed);
    let mut changed = baseline.clone();
    changed.functions[0].x86_scalar_fma[0].identity = [0; 32];
    candidates.push(changed);
    let mut changed = baseline.clone();
    let duplicate = changed.functions[0].x86_scalar_fma[0];
    changed.functions[0].x86_scalar_fma.push(duplicate);
    candidates.push(changed);

    for candidate in candidates {
        assert!(
            build_feature_required_x86_fma_object_artifact(&candidate, TargetProfile::LinuxX64)
                .is_err()
        );
    }
}

#[test]
fn source_free_x86_fma_object_rejects_cross_profile_even_when_native_target_matches() {
    let mut windows = x86_fma_plan(TargetProfile::WindowsX64, X86ScalarFmaFormat::Binary64);
    assert_eq!(
        windows.target,
        TargetProfile::UefiX64.native_target(),
        "Windows and UEFI intentionally share one physical NativeTarget"
    );
    assert!(
        build_feature_required_x86_fma_object_artifact(&windows, TargetProfile::UefiX64).is_err()
    );

    windows.functions[0].x86_scalar_fma[0].requirement =
        X86FeatureRequirement::scalar_fma(TargetProfile::UefiX64).unwrap();
    refresh_x86_fma_identity(&mut windows.functions[0].x86_scalar_fma[0]);
    assert!(
        build_feature_required_x86_fma_object_artifact(&windows, TargetProfile::WindowsX64)
            .is_err()
    );
}

#[test]
fn hosted_exit_process_object_validation_replays_exact_scalar_and_trap_bytes() {
    for (target, destination) in [
        (
            NativeTarget::linux_x64(),
            calling_conventions::MachineRegister::X86Rdi,
        ),
        (
            NativeTarget::linux_arm64(),
            calling_conventions::MachineRegister::Aarch64X(0),
        ),
        (
            NativeTarget::macos_arm64(),
            calling_conventions::MachineRegister::Aarch64X(0),
        ),
    ] {
        for value in [0, 1, 37, 255, 256, -1, i32::MIN, i32::MAX] {
            let machine = machine_id(91);
            let constant = operation_id(91);
            let settlement_operation = operation_id(92);
            let nominal_return = edge_id(91);
            let boundary = BoundaryMachineId::new(91).unwrap();
            let source_value = semantic_vocabulary::ValueId::new(91).unwrap();
            let scalar_type = semantic_vocabulary::ScalarType::Integer(
                semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 32)
                    .unwrap(),
            );
            let argument = BoundaryScalarArgument {
                source_value,
                scalar_type,
                immediate: semantic_vocabulary::IntegerValue::Signed(i128::from(value)),
                destination,
            };
            let bytes = match target.architecture {
                target::Architecture::X86_64 => isa_x86_64::encode_hosted_exit_process_i32(value),
                target::Architecture::Aarch64 => {
                    isa_aarch64::encode_hosted_exit_process_i32(target, value).unwrap()
                }
            };
            let plan = MachineCodePlan {
                psi: identity(),
                target,
                entry: machine,
                functions: vec![MachineCodeFunction {
                    scalar_abi: None,
                    mixed_structural_scalar_abi: None,
                    structural_call_scalar_return: None,
                    parameter_abi: None,
                    internal_unit_scalar_calls: Vec::new(),
                    installed_provider_unit_scalar_calls: Vec::new(),
                    dynamic_calls: Vec::new(),
                    stored_dynamic_calls: Vec::new(),
                    dynamic_parameter_calls: Vec::new(),
                    forwarded_dynamic_parameter_calls: Vec::new(),
                    forwarded_dynamic_descriptor_calls: Vec::new(),
                    unit_scalar_homes: Vec::new(),
                    unit_integer_constants: Vec::new(),
                    unit_affine_scalar_records: Vec::new(),
                    unit_structural_scalar_field_stores: Vec::new(),
                    unit_write_only_primitive_stores: Vec::new(),
                    scalar_structural_scalar_field_stores: Vec::new(),
                    machine,
                    attachment: None,
                    provenance: TerminalPsiProvenance {
                        operations: vec![constant, settlement_operation],
                        edges: vec![nominal_return],
                    },
                    bytes: bytes.clone(),
                    x86_scalar_fma: Vec::new(),
                    x86_scalar_fma_occurrences: Vec::new(),
                    x86_floating_control: None,
                    unit_stack: None,
                    unit_parameter_homes: Vec::new(),
                    unit_parameters: Vec::new(),
                    scalar_stack: None,
                    internal_calls: Vec::new(),
                    foreign_calls: Vec::new(),
                    internal_unit_calls: Vec::new(),
                    unit_continuations: Vec::new(),
                    unit_affine_cleanup: None,
                    semantic_code_attribution: vec![
                        SemanticCodeAttribution {
                            site: SemanticCodeSite::Operation(constant),
                            operation_ordinal: 0,
                            code_offset: 0,
                            byte_count: 0,
                        },
                        SemanticCodeAttribution {
                            site: SemanticCodeSite::Operation(settlement_operation),
                            operation_ordinal: 1,
                            code_offset: 0,
                            byte_count: bytes.len(),
                        },
                        SemanticCodeAttribution {
                            site: SemanticCodeSite::Edge(nominal_return),
                            operation_ordinal: 2,
                            code_offset: bytes.len(),
                            byte_count: 0,
                        },
                    ],
                    port_effects: Vec::new(),
                    boundary_settlements: vec![BoundarySettlementRecord {
                        psi_operation: settlement_operation,
                        boundary,
                        execution: machine_code::BoundaryExecutionRecord::CompilerBuiltin(
                            target_operations::CompilerBuiltinExecution::HostedExitProcessI32,
                        ),
                        realization: BoundaryRealization::HostedExitProcessI32(
                            HostedExitProcessI32Realization,
                        ),
                        scalar_arguments: vec![argument],
                        runtime_scalar_arguments: Vec::new(),
                        arguments: Vec::new(),
                        byte_sequence_arguments: Vec::new(),
                        completion_claim_sources: Vec::new(),
                        completion_receipts: Vec::new(),
                        completion_provider_custody: Vec::new(),
                        native_result: machine_code::BoundaryResultRecord::Unit,
                        operation_ordinal: 1,
                        code_offset: 0,
                        byte_count: bytes.len(),
                    }],
                    scalar_affine_cleanup: None,
                    scalar_control_affine_cleanups: Vec::new(),
                    scalar_structural_parameters: Vec::new(),
                    scalar_structural_parameter_homes: Vec::new(),
                    structural_return: None,
                }],
            };
            let object = build_object_artifact(&plan).expect("validated exit object");
            let again = build_object_artifact(&plan).expect("deterministic exit object");
            assert_eq!(object.text_bytes(), again.text_bytes());
            assert_eq!(
                object.boundary_settlements()[0].settlement.scalar_arguments,
                [argument]
            );
            let image = emit_direct_executable_image(&object, 3).expect("import-free exit image");
            image_emission::validate_direct_executable_image(&object, &image)
                .expect("exact exit image replay");
            assert_eq!(
                image.boundary_settlements()[0].settlement.byte_count,
                bytes.len()
            );
            let installation =
                build_installation_record(&image, ProfileDecisionId::new(91).unwrap())
                    .unwrap_or_else(|error| {
                        panic!("exit installation for {target:?}, {value}: {error:?}")
                    });
            let encoded =
                encode_installation_record(&installation).expect("exit installation encoding");
            let decoded = decode_installation_record(&encoded).expect("exit installation decoding");
            assert_eq!(decoded, installation);
            assert_eq!(
                decoded.boundary_settlements()[0]
                    .settlement
                    .scalar_arguments,
                [argument]
            );
            validate_installation_record(&decoded, &image)
                .expect("decoded exit installation binds image");
            if target == NativeTarget::host() {
                hosted_exit_runtime::assert_exit(&image.output().bytes, value);
            }

            for byte_position in 0..bytes.len() {
                for bit_position in 0..8 {
                    let mut corrupted = plan.clone();
                    corrupted.functions[0].bytes[byte_position] ^= 1 << bit_position;
                    assert!(matches!(
                        build_object_artifact(&corrupted),
                        Err(ObjectError::BoundaryRealizationMismatch { .. })
                    ));
                }
            }
            if target.architecture == target::Architecture::Aarch64 {
                let mut wrong_target = plan.clone();
                wrong_target.target = if target == NativeTarget::linux_arm64() {
                    NativeTarget::macos_arm64()
                } else {
                    NativeTarget::linux_arm64()
                };
                assert!(build_object_artifact(&wrong_target).is_err());
            }
        }
    }
    #[cfg(not(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    eprintln!("SKIP: physical hosted exit runtime requires Linux x64/AArch64 or macOS AArch64");
}

#[test]
fn linux_write_line_then_exit_survives_object_image_and_installation_replay() {
    let write_provider = WriteExitProvider(970);
    let plan = linux_write_line_exit_plan(&write_provider);
    let object = build_object_artifact(&plan).expect("composed object validates");
    let image = emit_direct_executable_image(&object, 3).expect("Linux image emits");
    let installation = build_installation_record_with_provider_executions(
        &image,
        ProfileDecisionId::new(97).unwrap(),
        [&write_provider],
    )
    .expect("composed installation validates");
    let encoded = encode_installation_record(&installation).unwrap();
    let decoded = decode_installation_record(&encoded).unwrap();
    assert_eq!(decoded, installation);
    assert_eq!(
        decoded.boundary_settlements()[0]
            .settlement
            .byte_sequence_arguments[0]
            .bytes,
        vec![0, 0x80, 0xff]
    );
    validate_installation_record(&decoded, &image).expect("decoded custody replays");
}

/// Every representable field of an installed boundary-settlement row is an
/// authenticated custody axis: a one-field substitution either cannot encode
/// canonically or still encodes, recomputes a distinct installation
/// fingerprint, and independent replay against the unchanged image rejects it.
#[test]
fn installation_boundary_settlement_rejects_every_one_field_substitution() {
    let write_provider = WriteExitProvider(970);
    let plan = linux_write_line_exit_plan(&write_provider);
    let artifact = build_object_artifact(&plan).expect("settlement artifact");
    let image = emit_direct_executable_image(&artifact, 3).expect("settlement image");
    let record = build_installation_record_with_provider_executions(
        &image,
        ProfileDecisionId::new(97).unwrap(),
        [&write_provider],
    )
    .expect("settlement installation");
    validate_installation_record(&record, &image).expect("exact image binding");
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");
    let [write_row, exit_row] = record.boundary_settlements() else {
        panic!("write+exit fixture retains two settlement rows");
    };
    assert_eq!(write_row.machine, machine_id(97));
    assert_eq!(write_row.settlement.psi_operation, operation_id(98));
    assert!(write_row.settlement.scalar_arguments.is_empty());
    assert_eq!(write_row.settlement.arguments.len(), 1);
    assert_eq!(write_row.settlement.byte_sequence_arguments.len(), 1);
    assert!(write_row.settlement.completion_claim_sources.is_empty());
    assert!(write_row.settlement.completion_receipts.is_empty());
    assert!(write_row.settlement.completion_provider_custody.is_empty());
    assert_eq!(exit_row.machine, machine_id(97));
    assert_eq!(exit_row.settlement.psi_operation, operation_id(100));
    assert_eq!(exit_row.settlement.scalar_arguments.len(), 1);
    assert!(exit_row.settlement.arguments.is_empty());
    assert!(exit_row.settlement.byte_sequence_arguments.is_empty());

    let i32_integer =
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 32)
            .unwrap();
    let i32_type = semantic_vocabulary::ScalarType::Integer(i32_integer);
    let fabricated_scalar_argument = BoundaryScalarArgument {
        source_value: semantic_vocabulary::ValueId::new(99).unwrap(),
        scalar_type: i32_type,
        immediate: semantic_vocabulary::IntegerValue::Signed(9),
        destination: calling_conventions::MachineRegister::X86Rdi,
    };
    let fabricated_runtime_argument = machine_code::ForeignCallScalarArgumentRecord {
        parameter_index: 0,
        source: machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
            defining_operation: operation_id(98),
            source_value: semantic_vocabulary::ValueId::new(99).unwrap(),
            scalar_type: i32_integer,
            value: semantic_vocabulary::IntegerValue::Signed(9),
        },
        placement: ValuePlacement {
            shape: ValueShape::integer(4, 4),
            locations: Vec::new(),
        },
        code_offset: 0,
        byte_count: 4,
    };
    let fabricated_argument = StructuralArgument {
        access: StructuralAccess::Owned,
        place: PlaceId::new(98).unwrap(),
        path: Vec::new(),
    };
    let fabricated_receipt = CompletionReceipt {
        claim: ClaimId::new(71).unwrap(),
        argument_index: 0,
    };
    let scalar_result =
        machine_code::BoundaryResultRecord::Scalar(machine_code::BoundaryScalarResultRecord {
            value: semantic_vocabulary::ValueId::new(99).unwrap(),
            scalar_type: i32_type,
            placement: ValuePlacement {
                shape: ValueShape::integer(4, 4),
                locations: Vec::new(),
            },
            return_edge: edge_id(97),
        });

    let mismatch = |operation: u64| InstallationError::BoundaryRealizationMismatch {
        machine: machine_id(97),
        operation: operation_id(operation),
    };
    let bad_offset = |operation: u64| InstallationError::InvalidBoundarySettlementOffset {
        machine: machine_id(97),
        operation: operation_id(operation),
    };
    let provider_custody = |operation: u64| InstallationError::InvalidCompletionProviderCustody {
        machine: machine_id(97),
        operation: operation_id(operation),
    };
    let receipt_custody = |operation: u64| InstallationError::InvalidCompletionReceiptCustody {
        machine: machine_id(97),
        operation: operation_id(operation),
    };

    type ReplayMutation = (
        &'static str,
        usize,
        Box<dyn Fn(&mut image_emission::ObjectBoundarySettlement)>,
    );
    // Semantic identities no canonical record-shape join pins: the substituted
    // row still encodes and decodes, so rejection is the recomputed identity
    // and the independent image replay.
    let still_encodes: Vec<ReplayMutation> = vec![
        (
            "psi_operation",
            0,
            Box::new(|row| {
                row.settlement.psi_operation = operation_id(999);
            }),
        ),
        (
            "boundary",
            0,
            Box::new(|row| {
                row.settlement.boundary = BoundaryMachineId::new(999).unwrap();
            }),
        ),
        (
            "operation_ordinal",
            0,
            Box::new(|row| {
                row.settlement.operation_ordinal = 7;
            }),
        ),
        (
            "execution::provider_identity",
            0,
            Box::new(|row| {
                let machine_code::BoundaryExecutionRecord::AdmittedProvider(execution) =
                    &mut row.settlement.execution
                else {
                    panic!("write settlement retains admitted provider execution");
                };
                execution.provider_execution_report_identity += 1;
            }),
        ),
        (
            "byte_sequence_arguments::literal_operation",
            0,
            Box::new(|row| {
                row.settlement.byte_sequence_arguments[0].literal_operation = operation_id(999);
            }),
        ),
        (
            "byte_sequence_arguments::structural_type",
            0,
            Box::new(|row| {
                row.settlement.byte_sequence_arguments[0]
                    .structural_type
                    .identity = "test::SubstitutedBytes".into();
            }),
        ),
        (
            "scalar_arguments::source_value",
            1,
            Box::new(|row| {
                row.settlement.scalar_arguments[0].source_value =
                    semantic_vocabulary::ValueId::new(99).unwrap();
            }),
        ),
        (
            "scalar_arguments::immediate",
            1,
            Box::new(|row| {
                row.settlement.scalar_arguments[0].immediate =
                    semantic_vocabulary::IntegerValue::Signed(38);
            }),
        ),
    ];
    for (field, index, mutate) in still_encodes {
        let mut changed = record.clone();
        mutate(&mut changed.boundary_settlements_mut_for_test()[index]);
        assert_ne!(changed, record, "{field}: substitution changes the row");
        let bytes = encode_installation_record(&changed)
            .unwrap_or_else(|error| panic!("{field}: substituted row encodes: {error:?}"));
        let replayed = decode_installation_record(&bytes)
            .unwrap_or_else(|error| panic!("{field}: substituted row decodes: {error:?}"));
        assert_eq!(
            replayed, changed,
            "{field}: codec preserves the substituted row"
        );
        assert_ne!(
            installation_fingerprint(&replayed)
                .unwrap_or_else(|error| panic!("{field}: substituted fingerprint: {error:?}")),
            authentic_fingerprint,
            "{field}: recomputed identity differs from the authentic record"
        );
        assert_eq!(
            validate_installation_record(&replayed, &image),
            Err(InstallationError::ImageBindingMismatch),
            "{field}: independent replay rejects the substituted row"
        );
    }

    type RejectedMutation = (
        &'static str,
        usize,
        Box<dyn Fn(&mut image_emission::ObjectBoundarySettlement)>,
        InstallationError,
    );
    // Canonical record-shape joins reject every other one-field substitution
    // at encoding, before any identity or replay could accept it.
    let rejected: Vec<RejectedMutation> = vec![
        (
            "machine",
            0,
            Box::new(|row| {
                row.machine = machine_id(98);
            }),
            InstallationError::EffectMachineMissing(machine_id(98)),
        ),
        (
            "text_offset",
            0,
            Box::new(|row| {
                row.text_offset += 1;
            }),
            bad_offset(98),
        ),
        (
            "execution",
            0,
            Box::new(|row| {
                row.settlement.execution = machine_code::BoundaryExecutionRecord::CompilerBuiltin(
                    target_operations::CompilerBuiltinExecution::HostedExitProcessI32,
                );
            }),
            provider_custody(98),
        ),
        (
            "realization",
            0,
            Box::new(|row| {
                row.settlement.realization =
                    target_operations::HostedExitProcessI32Realization.into();
            }),
            provider_custody(98),
        ),
        (
            "scalar_arguments",
            0,
            Box::new(move |row| {
                row.settlement
                    .scalar_arguments
                    .push(fabricated_scalar_argument);
            }),
            mismatch(98),
        ),
        (
            "runtime_scalar_arguments",
            0,
            Box::new(move |row| {
                row.settlement
                    .runtime_scalar_arguments
                    .push(fabricated_runtime_argument.clone());
            }),
            mismatch(98),
        ),
        (
            "arguments",
            0,
            Box::new(move |row| {
                row.settlement.arguments.push(fabricated_argument.clone());
            }),
            mismatch(98),
        ),
        (
            "arguments::place",
            0,
            Box::new(|row| {
                row.settlement.arguments[0].place = PlaceId::new(98).unwrap();
            }),
            mismatch(98),
        ),
        (
            "byte_sequence_arguments",
            0,
            Box::new(|row| {
                row.settlement.byte_sequence_arguments.clear();
            }),
            mismatch(98),
        ),
        (
            "byte_sequence_arguments::bytes",
            0,
            Box::new(|row| {
                row.settlement.byte_sequence_arguments[0].bytes.push(4);
            }),
            mismatch(98),
        ),
        (
            "byte_sequence_arguments::code_offset",
            0,
            Box::new(|row| {
                row.settlement.byte_sequence_arguments[0].code_offset += 1;
            }),
            mismatch(98),
        ),
        (
            "completion_claim_sources",
            0,
            Box::new(|row| {
                let claim = ClaimId::new(71).unwrap();
                row.settlement
                    .completion_claim_sources
                    .push(CompletionClaimSource {
                        claim,
                        entry: Some(EntryClaim {
                            claim,
                            input: PlaceId::new(97).unwrap(),
                            path: Vec::new(),
                        }),
                        content: None,
                    });
            }),
            receipt_custody(98),
        ),
        (
            "completion_receipts",
            0,
            Box::new(move |row| {
                row.settlement.completion_receipts.push(fabricated_receipt);
            }),
            receipt_custody(98),
        ),
        (
            "completion_provider_custody",
            0,
            Box::new(|row| {
                let claim = ClaimId::new(71).unwrap();
                row.settlement.completion_provider_custody.push(
                    machine_code::CompletionProviderCustodyBinding {
                        source: CompletionClaimSource {
                            claim,
                            entry: Some(EntryClaim {
                                claim,
                                input: PlaceId::new(97).unwrap(),
                                path: Vec::new(),
                            }),
                            content: None,
                        },
                        receipt: CompletionReceipt {
                            claim,
                            argument_index: 0,
                        },
                        provider_execution: machine_code::ProviderExecutionRecord::new(
                            11, 12, 13, 14, 15,
                        )
                        .unwrap(),
                    },
                );
            }),
            provider_custody(98),
        ),
        (
            "native_result",
            0,
            Box::new(move |row| {
                row.settlement.native_result = scalar_result.clone();
            }),
            mismatch(98),
        ),
        (
            "code_offset",
            0,
            Box::new(|row| {
                row.settlement.code_offset += 1;
            }),
            bad_offset(98),
        ),
        (
            "byte_count",
            0,
            Box::new(|row| {
                row.settlement.byte_count += 1;
            }),
            mismatch(98),
        ),
        (
            "machine::exit",
            1,
            Box::new(|row| {
                row.machine = machine_id(98);
            }),
            InstallationError::EffectMachineMissing(machine_id(98)),
        ),
        (
            "text_offset::exit",
            1,
            Box::new(|row| {
                row.text_offset += 1;
            }),
            bad_offset(100),
        ),
        (
            "psi_operation::exit",
            1,
            Box::new(|row| {
                row.settlement.psi_operation = operation_id(98);
            }),
            InstallationError::DuplicateBoundarySettlementOperation {
                machine: machine_id(97),
                operation: operation_id(98),
            },
        ),
        (
            "execution::exit",
            1,
            Box::new(|row| {
                row.settlement.execution = machine_code::BoundaryExecutionRecord::AdmittedProvider(
                    machine_code::ProviderExecutionRecord::new(970, 971, 972, 973, 974).unwrap(),
                );
            }),
            provider_custody(100),
        ),
        (
            "realization::exit",
            1,
            Box::new(|row| {
                row.settlement.realization = target_operations::LinuxWriteLineRealization.into();
            }),
            provider_custody(100),
        ),
        (
            "scalar_arguments::exit",
            1,
            Box::new(|row| {
                row.settlement.scalar_arguments.clear();
            }),
            mismatch(100),
        ),
        (
            "scalar_arguments::scalar_type",
            1,
            Box::new(move |row| {
                row.settlement.scalar_arguments[0].scalar_type =
                    semantic_vocabulary::ScalarType::Boolean;
            }),
            mismatch(100),
        ),
        (
            "scalar_arguments::destination",
            1,
            Box::new(|row| {
                row.settlement.scalar_arguments[0].destination =
                    calling_conventions::MachineRegister::X86Rsi;
            }),
            mismatch(100),
        ),
        (
            "runtime_scalar_arguments::exit",
            1,
            Box::new(move |row| {
                row.settlement.runtime_scalar_arguments.push(
                    machine_code::ForeignCallScalarArgumentRecord {
                        parameter_index: 0,
                        source:
                            machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
                                defining_operation: operation_id(98),
                                source_value: semantic_vocabulary::ValueId::new(99).unwrap(),
                                scalar_type: i32_integer,
                                value: semantic_vocabulary::IntegerValue::Signed(9),
                            },
                        placement: ValuePlacement {
                            shape: ValueShape::integer(4, 4),
                            locations: Vec::new(),
                        },
                        code_offset: 0,
                        byte_count: 4,
                    },
                );
            }),
            mismatch(100),
        ),
        (
            "arguments::exit",
            1,
            Box::new(|row| {
                row.settlement.arguments.push(StructuralArgument {
                    access: StructuralAccess::Owned,
                    place: PlaceId::new(98).unwrap(),
                    path: Vec::new(),
                });
            }),
            mismatch(100),
        ),
        (
            "byte_sequence_arguments::exit",
            1,
            Box::new(|row| {
                row.settlement.byte_sequence_arguments.push(
                    machine_code::BoundaryByteSequenceArgumentRecord {
                        argument: StructuralArgument {
                            access: StructuralAccess::SharedBorrow,
                            place: PlaceId::new(98).unwrap(),
                            path: Vec::new(),
                        },
                        literal_operation: operation_id(97),
                        structural_type: terminal_psi::StructuralTypeDeclaration {
                            id: StructuralTypeId::new(98).unwrap(),
                            identity: "test::FabricatedBytes".into(),
                            shape: terminal_psi::StructuralTypeShape::ByteSequence(
                                terminal_psi::ByteSequenceCarrier::BorrowedView,
                            ),
                        },
                        bytes: vec![1, 2, 3],
                        code_offset: 0,
                        code_byte_count: 4,
                        data_offset: 4,
                        data_byte_count: 4,
                    },
                );
            }),
            mismatch(100),
        ),
        (
            "completion_claim_sources::exit",
            1,
            Box::new(|row| {
                let claim = ClaimId::new(71).unwrap();
                row.settlement
                    .completion_claim_sources
                    .push(CompletionClaimSource {
                        claim,
                        entry: Some(EntryClaim {
                            claim,
                            input: PlaceId::new(97).unwrap(),
                            path: Vec::new(),
                        }),
                        content: None,
                    });
            }),
            provider_custody(100),
        ),
        (
            "completion_receipts::exit",
            1,
            Box::new(|row| {
                row.settlement.completion_receipts.push(CompletionReceipt {
                    claim: ClaimId::new(71).unwrap(),
                    argument_index: 0,
                });
            }),
            InstallationError::InvalidCompletionReceiptArgumentIndex {
                machine: machine_id(97),
                operation: operation_id(100),
            },
        ),
        (
            "completion_provider_custody::exit",
            1,
            Box::new(|row| {
                let claim = ClaimId::new(71).unwrap();
                row.settlement.completion_provider_custody.push(
                    machine_code::CompletionProviderCustodyBinding {
                        source: CompletionClaimSource {
                            claim,
                            entry: Some(EntryClaim {
                                claim,
                                input: PlaceId::new(97).unwrap(),
                                path: Vec::new(),
                            }),
                            content: None,
                        },
                        receipt: CompletionReceipt {
                            claim,
                            argument_index: 0,
                        },
                        provider_execution: machine_code::ProviderExecutionRecord::new(
                            11, 12, 13, 14, 15,
                        )
                        .unwrap(),
                    },
                );
            }),
            provider_custody(100),
        ),
        (
            "native_result::exit",
            1,
            Box::new(|row| {
                row.settlement.native_result = machine_code::BoundaryResultRecord::Scalar(
                    machine_code::BoundaryScalarResultRecord {
                        value: semantic_vocabulary::ValueId::new(99).unwrap(),
                        scalar_type: semantic_vocabulary::ScalarType::Integer(
                            semantic_vocabulary::IntegerType::new(
                                semantic_vocabulary::IntegerSign::Signed,
                                32,
                            )
                            .unwrap(),
                        ),
                        placement: ValuePlacement {
                            shape: ValueShape::integer(4, 4),
                            locations: Vec::new(),
                        },
                        return_edge: edge_id(97),
                    },
                );
            }),
            mismatch(100),
        ),
        (
            "operation_ordinal::exit",
            1,
            Box::new(|row| {
                row.settlement.operation_ordinal += 1;
            }),
            mismatch(100),
        ),
        (
            "code_offset::exit",
            1,
            Box::new(|row| {
                row.settlement.code_offset += 1;
            }),
            bad_offset(100),
        ),
        (
            "byte_count::exit",
            1,
            Box::new(|row| {
                row.settlement.byte_count += 1;
            }),
            mismatch(100),
        ),
    ];
    for (field, index, mutate, expected) in rejected {
        let mut changed = record.clone();
        mutate(&mut changed.boundary_settlements_mut_for_test()[index]);
        assert_ne!(changed, record, "{field}: substitution changes the row");
        assert_eq!(
            encode_installation_record(&changed),
            Err(expected),
            "{field}: canonical encoding rejects the substitution"
        );
    }

    // Roster-level custody: dropping a row still encodes (no coverage join
    // binds settlements to operations), so only the image binding rejects it;
    // reordering the rows is rejected by the canonical
    // (machine, text_offset, operation_ordinal) order.
    let mut dropped_row = record.clone();
    dropped_row.boundary_settlements_mut_for_test().pop();
    let bytes = encode_installation_record(&dropped_row).expect("dropped row encodes");
    let replayed = decode_installation_record(&bytes).expect("dropped row decodes");
    assert_ne!(
        installation_fingerprint(&replayed).expect("dropped fingerprint"),
        authentic_fingerprint
    );
    assert_eq!(
        validate_installation_record(&replayed, &image),
        Err(InstallationError::ImageBindingMismatch)
    );
    let mut reordered = record.clone();
    reordered.boundary_settlements_mut_for_test().swap(0, 1);
    assert_eq!(
        encode_installation_record(&reordered),
        Err(InstallationError::NonCanonicalBoundarySettlementOrder)
    );
}
