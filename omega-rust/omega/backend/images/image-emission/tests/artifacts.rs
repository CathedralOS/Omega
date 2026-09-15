use function_identity::{MachineFunctionIdentity, StateKey};
use image_emission::{
    INSTALLATION_FORMAT_MARKER, InstallationError, ObjectError,
    build_admitted_x86_fma_object_artifact, build_feature_required_x86_fma_object_artifact,
    build_installation_record, build_installation_record_with_evidence,
    build_installation_record_with_provider_executions,
    build_installation_record_with_selected_provider_plans_and_evidence, build_object_artifact,
    build_object_artifact_with_private_functions, can_emit_executable_image,
    decode_installation_record, derive_installation_stack_demand, derive_stack_demand,
    derive_unit_stack_demand, emit_executable_image, emit_object_container,
    encode_installation_record, installation_fingerprint, validate_installation_record,
};
use installation_evidence::{ComponentProgressAcceptanceEvidence, ProviderExecutionEvidence};
use machine_code::{
    Aarch64ReturnLinkEvidence, BoundarySettlementRecord, InternalCallRelocation,
    InternalUnitCallRecord, MachineCodeFunction, MachineCodePlan, PortEffectRecord,
    ScalarCallStackEvidence, ScalarCleanupPreservationEvidence, ScalarConditionalBranchEvidence,
    ScalarConditionalCondition, ScalarControlAffineCleanupRecord, ScalarControlFlowEvidence,
    ScalarDivisionBranchEvidence, ScalarStackEvidence, ScalarStackMutation,
    ScalarStackMutationKind, SemanticCodeAttribution, SemanticCodeSite, StackAdjustmentPair,
    UnitAffineCleanupRecord, UnitCallStackEvidence, UnitParameterHomeRecord, UnitParameterRecord,
    UnitStackEvidence, X86ScalarFmaFormat,
};
use object_file::{RelocationKind, RelocationOrigin, SectionKind, SymbolKind, object_symbol_name};
use semantic_vocabulary::{
    BoundaryMachineId, ClaimId, EdgeId, MachineId, OperationId, PlaceId, ProfileDecisionId,
    ServiceId, StructuralTypeId,
};
use symbols::SymbolHandle;
use target::{
    AdmittedX86ScalarFmaProvider, NativeTarget, TargetProfile, X86DeploymentFeatures,
    X86FeatureRequirement, X86ScalarFmaDifferentialReceipt, X86ScalarFmaSlot, X86TargetFeature,
};
use target_operations::{
    BoundaryRealization, BoundaryScalarArgument, CallSiteOwner, CompletionClaimSource,
    HostedExitProcessI32Realization, MetadataOnlyPortRealization, ProviderExecutionBinding,
    ProviderPlanReportIdentity, TerminalPsiProvenance,
};
use terminal_psi::{
    CompletionReceipt, EntryClaim, NominalAffineCleanup, SemanticFingerprint, StructuralAccess,
    StructuralArgument, StructuralMultiplicity, TerminalAffineCleanupAction, TerminalPsiIdentity,
    VocabularyMarker,
};

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
            emit_executable_image(&artifact, 3).is_err(),
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
        let image = emit_executable_image(&artifact, 3)
            .expect("feature-qualified generic FMA object should reach exact image emission");
        assert_eq!(image.x86_scalar_fma_provider(), Some(provider));
        image_emission::validate_executable_image(&artifact, &image)
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
    assert!(emit_executable_image(&feature_only, 3).is_err());
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
            let image = emit_executable_image(&object, 3).expect("import-free exit image");
            image_emission::validate_executable_image(&object, &image)
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

#[path = "artifacts/hosted_exit_runtime.rs"]
mod hosted_exit_runtime;

#[path = "artifacts/macho_fixups.rs"]
mod macho_fixups;
#[path = "artifacts/macho_storage.rs"]
mod macho_storage;

#[derive(Debug)]
struct WriteExitProvider(u64);

impl ProviderExecutionEvidence for WriteExitProvider {
    fn requirement_identity(&self) -> &str {
        match self.0 {
            970 => "Console::write_line",
            980 => "Console::exit_process",
            _ => "unexpected provider",
        }
    }

    fn provider_plan_report_identity(&self) -> u64 {
        self.0
    }
    fn provider_execution_report_identity(&self) -> u64 {
        self.0 + 1
    }
    fn provider_execution_report_fingerprint(&self) -> u64 {
        self.0 + 2
    }
    fn normalized_root_report_identity(&self) -> u64 {
        self.0 + 3
    }
    fn boundary_contract_report_fingerprint(&self) -> u64 {
        self.0 + 4
    }
}

fn write_exit_provider_binding(provider: &WriteExitProvider) -> ProviderExecutionBinding {
    ProviderExecutionBinding::from_execution_record(
        ProviderPlanReportIdentity::new(provider.provider_plan_report_identity()).unwrap(),
        provider.provider_execution_report_identity(),
        provider.provider_execution_report_fingerprint(),
        provider.normalized_root_report_identity(),
        provider.boundary_contract_report_fingerprint(),
    )
    .unwrap()
}

/// Linux x64 machine whose two boundary settlements cover both execution
/// roles: an admitted-provider `LinuxWriteLine` retaining byte-sequence
/// argument custody and a compiler-builtin `HostedExitProcessI32` retaining
/// one scalar argument.
fn linux_write_line_exit_plan(provider: &WriteExitProvider) -> MachineCodePlan {
    let target = NativeTarget::linux_x64();
    let machine = machine_id(97);
    let literal_operation = operation_id(97);
    let write_operation = operation_id(98);
    let constant_operation = operation_id(99);
    let exit_operation = operation_id(100);
    let return_edge = edge_id(97);
    let write_boundary = BoundaryMachineId::new(97).unwrap();
    let exit_boundary = BoundaryMachineId::new(98).unwrap();
    let literal_place = PlaceId::new(97).unwrap();
    let structural_type_id = StructuralTypeId::new(97).unwrap();
    let literal = vec![0, 0x80, 0xff];
    let structural_type = terminal_psi::StructuralTypeDeclaration {
        id: structural_type_id,
        identity: "test::BorrowedBytes".into(),
        shape: terminal_psi::StructuralTypeShape::ByteSequence(
            terminal_psi::ByteSequenceCarrier::BorrowedView,
        ),
    };
    let structural_argument = StructuralArgument {
        access: StructuralAccess::SharedBorrow,
        place: literal_place,
        path: Vec::new(),
    };
    let (write_bytes, data) = isa_x86_64::encode_linux_write_line_literal(&literal).unwrap();
    let exit_bytes = isa_x86_64::encode_hosted_exit_process_i32(37);
    let mut bytes = write_bytes.clone();
    let exit_offset = bytes.len();
    bytes.extend_from_slice(&exit_bytes);
    let return_offset = bytes.len();
    bytes.push(0xc3);
    let i32_type =
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 32)
            .unwrap();
    let exit_argument = BoundaryScalarArgument {
        source_value: semantic_vocabulary::ValueId::new(97).unwrap(),
        scalar_type: semantic_vocabulary::ScalarType::Integer(i32_type),
        immediate: semantic_vocabulary::IntegerValue::Signed(37),
        destination: calling_conventions::MachineRegister::X86Rdi,
    };
    MachineCodePlan {
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
                operations: vec![
                    literal_operation,
                    write_operation,
                    constant_operation,
                    exit_operation,
                ],
                edges: vec![return_edge],
            },
            bytes: bytes.clone(),
            x86_scalar_fma: Vec::new(),
            x86_scalar_fma_occurrences: Vec::new(),
            x86_floating_control: None,
            unit_stack: Some(UnitStackEvidence {
                frame: None,
                aarch64_return_link: None,
                stack_alignment: 16,
            }),
            unit_parameter_homes: Vec::new(),
            unit_parameters: Vec::new(),
            scalar_stack: None,
            internal_calls: Vec::new(),
            foreign_calls: Vec::new(),
            internal_unit_calls: Vec::new(),
            unit_continuations: Vec::new(),
            unit_affine_cleanup: Some(UnitAffineCleanupRecord {
                psi_edge: return_edge,
                structural_types: vec![structural_type.clone()].into(),
                locals: Vec::new(),
                actions: Vec::new(),
                code_offset: return_offset,
                byte_count: 1,
            }),
            semantic_code_attribution: vec![
                SemanticCodeAttribution {
                    site: SemanticCodeSite::Operation(literal_operation),
                    operation_ordinal: 0,
                    code_offset: 0,
                    byte_count: 0,
                },
                SemanticCodeAttribution {
                    site: SemanticCodeSite::Operation(write_operation),
                    operation_ordinal: 1,
                    code_offset: 0,
                    byte_count: write_bytes.len(),
                },
                SemanticCodeAttribution {
                    site: SemanticCodeSite::Operation(constant_operation),
                    operation_ordinal: 2,
                    code_offset: exit_offset,
                    byte_count: 0,
                },
                SemanticCodeAttribution {
                    site: SemanticCodeSite::Operation(exit_operation),
                    operation_ordinal: 3,
                    code_offset: exit_offset,
                    byte_count: exit_bytes.len(),
                },
                SemanticCodeAttribution {
                    site: SemanticCodeSite::Edge(return_edge),
                    operation_ordinal: 4,
                    code_offset: return_offset,
                    byte_count: 1,
                },
            ],
            port_effects: Vec::new(),
            boundary_settlements: vec![
                BoundarySettlementRecord {
                    psi_operation: write_operation,
                    boundary: write_boundary,
                    execution: machine_code::BoundaryExecutionRecord::AdmittedProvider(
                        write_exit_provider_binding(provider).into(),
                    ),
                    realization: target_operations::LinuxWriteLineRealization.into(),
                    scalar_arguments: Vec::new(),
                    runtime_scalar_arguments: Vec::new(),
                    arguments: vec![structural_argument.clone()],
                    byte_sequence_arguments: vec![
                        machine_code::BoundaryByteSequenceArgumentRecord {
                            argument: structural_argument,
                            literal_operation,
                            structural_type: structural_type.clone(),
                            bytes: literal.clone(),
                            code_offset: 0,
                            code_byte_count: data.start,
                            data_offset: data.start,
                            data_byte_count: data.len(),
                        },
                    ],
                    completion_claim_sources: Vec::new(),
                    completion_receipts: Vec::new(),
                    completion_provider_custody: Vec::new(),
                    native_result: machine_code::BoundaryResultRecord::Unit,
                    operation_ordinal: 1,
                    code_offset: 0,
                    byte_count: write_bytes.len(),
                },
                BoundarySettlementRecord {
                    psi_operation: exit_operation,
                    boundary: exit_boundary,
                    execution: machine_code::BoundaryExecutionRecord::CompilerBuiltin(
                        target_operations::CompilerBuiltinExecution::HostedExitProcessI32,
                    ),
                    realization: HostedExitProcessI32Realization.into(),
                    scalar_arguments: vec![exit_argument],
                    runtime_scalar_arguments: Vec::new(),
                    arguments: Vec::new(),
                    byte_sequence_arguments: Vec::new(),
                    completion_claim_sources: Vec::new(),
                    completion_receipts: Vec::new(),
                    completion_provider_custody: Vec::new(),
                    native_result: machine_code::BoundaryResultRecord::Unit,
                    operation_ordinal: 3,
                    code_offset: exit_offset,
                    byte_count: exit_bytes.len(),
                },
            ],
            scalar_affine_cleanup: None,
            scalar_control_affine_cleanups: Vec::new(),
            scalar_structural_parameters: Vec::new(),
            scalar_structural_parameter_homes: Vec::new(),
            structural_return: None,
        }],
    }
}

/// Linux x64 machine retaining two privileged `out` effects. The second is
/// consumed by an admitted-provider `MetadataOnlyPort` settlement while the
/// first stays unbound custody, so one-field mutation coverage reaches both
/// the replay-rejected representable axes and the axes pinned by the
/// settlement join at encoding.
fn port_effect_plan(provider: &WriteExitProvider) -> MachineCodePlan {
    let machine = machine_id(1);
    let free_operation = operation_id(1);
    let bound_operation = operation_id(2);
    let settlement_operation = operation_id(3);
    let return_edge = edge_id(1);
    let service = ServiceId::new(1).unwrap();
    let boundary = BoundaryMachineId::new(1).unwrap();
    let mut bytes = x86_encoding::encode_immediate_port_write(0x20, 0x20).to_vec();
    let bound_offset = bytes.len();
    bytes.extend_from_slice(&x86_encoding::encode_immediate_port_write(0x40, 0x50));
    let settlement_offset = bytes.len();
    bytes.push(0xc3);
    MachineCodePlan {
        psi: identity(),
        target: NativeTarget::linux_x64(),
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
                operations: vec![free_operation, bound_operation, settlement_operation],
                edges: vec![return_edge],
            },
            bytes,
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
                    site: SemanticCodeSite::Operation(free_operation),
                    operation_ordinal: 0,
                    code_offset: 0,
                    byte_count: 27,
                },
                SemanticCodeAttribution {
                    site: SemanticCodeSite::Operation(bound_operation),
                    operation_ordinal: 1,
                    code_offset: bound_offset,
                    byte_count: 27,
                },
                SemanticCodeAttribution {
                    site: SemanticCodeSite::Operation(settlement_operation),
                    operation_ordinal: 2,
                    code_offset: settlement_offset,
                    byte_count: 0,
                },
                SemanticCodeAttribution {
                    site: SemanticCodeSite::Edge(return_edge),
                    operation_ordinal: 3,
                    code_offset: settlement_offset,
                    byte_count: 1,
                },
            ],
            port_effects: vec![
                PortEffectRecord {
                    psi_operation: free_operation,
                    service,
                    port: 0x20,
                    value: 0x20,
                    operation_ordinal: 0,
                    code_offset: 0,
                    byte_count: 27,
                },
                PortEffectRecord {
                    psi_operation: bound_operation,
                    service,
                    port: 0x40,
                    value: 0x50,
                    operation_ordinal: 1,
                    code_offset: bound_offset,
                    byte_count: 27,
                },
            ],
            boundary_settlements: vec![BoundarySettlementRecord {
                psi_operation: settlement_operation,
                boundary,
                execution: machine_code::BoundaryExecutionRecord::AdmittedProvider(
                    write_exit_provider_binding(provider).into(),
                ),
                realization: MetadataOnlyPortRealization {
                    effect_operation: bound_operation,
                    service,
                    port: 0x40,
                    value: 0x50,
                }
                .into(),
                scalar_arguments: Vec::new(),
                runtime_scalar_arguments: Vec::new(),
                arguments: Vec::new(),
                byte_sequence_arguments: Vec::new(),
                completion_claim_sources: Vec::new(),
                completion_receipts: Vec::new(),
                completion_provider_custody: Vec::new(),
                native_result: machine_code::BoundaryResultRecord::Unit,
                operation_ordinal: 2,
                code_offset: settlement_offset,
                byte_count: 0,
            }],
            scalar_affine_cleanup: None,
            scalar_control_affine_cleanups: Vec::new(),
            scalar_structural_parameters: Vec::new(),
            scalar_structural_parameter_homes: Vec::new(),
            structural_return: None,
        }],
    }
}

#[test]
fn linux_write_line_then_exit_survives_object_image_and_installation_replay() {
    let write_provider = WriteExitProvider(970);
    let plan = linux_write_line_exit_plan(&write_provider);
    let object = build_object_artifact(&plan).expect("composed object validates");
    let image = emit_executable_image(&object, 3).expect("Linux image emits");
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
    let image = emit_executable_image(&artifact, 3).expect("settlement image");
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

/// Every representable field of an installed semantic-code attribution row is
/// an authenticated custody axis: a one-field substitution either cannot
/// encode canonically or still encodes, recomputes a distinct installation
/// fingerprint, and independent replay against the unchanged image rejects it.
/// Operation and edge site identities and in-bounds byte counts join only
/// against the retained image rows; the machine join, the canonical
/// (machine, operation_ordinal, text_offset) order, interval geometry, and
/// the boundary-joined nominal return edge are canonical record-shape
/// custody rejected at encoding.
#[test]
fn installation_semantic_code_attribution_rejects_every_one_field_substitution() {
    let write_provider = WriteExitProvider(970);
    let plan = linux_write_line_exit_plan(&write_provider);
    let artifact = build_object_artifact(&plan).expect("attribution artifact");
    let image = emit_executable_image(&artifact, 3).expect("attribution image");
    let record = build_installation_record_with_provider_executions(
        &image,
        ProfileDecisionId::new(97).unwrap(),
        [&write_provider],
    )
    .expect("attribution installation");
    validate_installation_record(&record, &image).expect("exact image binding");
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");
    let function = record
        .functions()
        .iter()
        .find(|function| function.machine == machine_id(97))
        .expect("attributed function row");
    let function_byte_count = function.byte_count;
    let [literal_row, write_row, constant_row, exit_row, return_row] =
        record.semantic_code_attribution()
    else {
        panic!("write+exit fixture retains five attribution rows");
    };
    assert_eq!(literal_row.machine, machine_id(97));
    assert_eq!(
        literal_row.attribution,
        SemanticCodeAttribution {
            site: SemanticCodeSite::Operation(operation_id(97)),
            operation_ordinal: 0,
            code_offset: 0,
            byte_count: 0,
        }
    );
    assert_eq!(literal_row.text_offset, function.text_offset);
    assert_eq!(
        write_row.attribution.site,
        SemanticCodeSite::Operation(operation_id(98))
    );
    assert_eq!(write_row.attribution.operation_ordinal, 1);
    assert_eq!(write_row.attribution.code_offset, 0);
    assert_eq!(
        constant_row.attribution.site,
        SemanticCodeSite::Operation(operation_id(99))
    );
    assert_eq!(constant_row.attribution.operation_ordinal, 2);
    assert_eq!(constant_row.attribution.byte_count, 0);
    assert_eq!(
        exit_row.attribution.site,
        SemanticCodeSite::Operation(operation_id(100))
    );
    assert_eq!(exit_row.attribution.operation_ordinal, 3);
    assert_eq!(
        return_row.attribution.site,
        SemanticCodeSite::Edge(edge_id(97))
    );
    assert_eq!(return_row.attribution.operation_ordinal, 4);
    assert_eq!(
        return_row
            .attribution
            .code_offset
            .checked_add(return_row.attribution.byte_count),
        Some(function_byte_count)
    );

    type ReplayMutation = (
        &'static str,
        usize,
        Box<dyn Fn(&mut image_emission::ObjectCodeAttribution)>,
    );
    // Semantic site identities and in-bounds byte counts have no canonical
    // record-shape join: the substituted row still encodes and decodes, so
    // rejection is the recomputed identity and the independent image replay.
    let still_encodes: Vec<ReplayMutation> = vec![
        (
            "site::operation_identity",
            0,
            Box::new(|row| {
                row.attribution.site = SemanticCodeSite::Operation(operation_id(199));
            }),
        ),
        (
            "site::edge_identity",
            4,
            Box::new(|row| {
                row.attribution.site = SemanticCodeSite::Edge(edge_id(199));
            }),
        ),
        (
            "byte_count::zero_marker_row",
            0,
            Box::new(|row| {
                row.attribution.byte_count = 1;
            }),
        ),
        (
            "byte_count::in_bounds_row",
            1,
            Box::new(|row| {
                row.attribution.byte_count -= 1;
            }),
        ),
    ];
    for (field, index, mutate) in still_encodes {
        let mut changed = record.clone();
        mutate(&mut changed.semantic_code_attribution_mut_for_test()[index]);
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

    let invalid_attribution =
        |site: SemanticCodeSite| InstallationError::InvalidSemanticCodeAttribution {
            machine: machine_id(97),
            site,
        };
    let exit_mismatch = || InstallationError::BoundaryRealizationMismatch {
        machine: machine_id(97),
        operation: operation_id(100),
    };
    // Canonical record-shape joins reject every other one-field substitution
    // at encoding, before any identity or replay could accept it.
    let rejected: Vec<(
        &'static str,
        usize,
        Box<dyn Fn(&mut image_emission::ObjectCodeAttribution)>,
        InstallationError,
    )> = vec![
        (
            "machine::unknown",
            4,
            Box::new(|row| {
                row.machine = machine_id(199);
            }),
            InstallationError::SemanticCodeAttributionMachineMissing(machine_id(199)),
        ),
        (
            "operation_ordinal::reorders_roster",
            0,
            Box::new(|row| {
                row.attribution.operation_ordinal = 5;
            }),
            InstallationError::NonCanonicalSemanticCodeAttributionOrder,
        ),
        (
            "code_offset::operation_row",
            1,
            Box::new(|row| {
                row.attribution.code_offset += 1;
            }),
            invalid_attribution(SemanticCodeSite::Operation(operation_id(98))),
        ),
        (
            "text_offset::operation_row",
            1,
            Box::new(|row| {
                row.text_offset += 1;
            }),
            invalid_attribution(SemanticCodeSite::Operation(operation_id(98))),
        ),
        (
            "byte_count::past_function_end",
            1,
            Box::new(move |row| {
                row.attribution.byte_count = function_byte_count + 1;
            }),
            invalid_attribution(SemanticCodeSite::Operation(operation_id(98))),
        ),
        (
            "site::tail_edge_to_operation",
            4,
            Box::new(|row| {
                row.attribution.site = SemanticCodeSite::Operation(operation_id(199));
            }),
            exit_mismatch(),
        ),
        (
            "operation_ordinal::tail_edge",
            4,
            Box::new(|row| {
                row.attribution.operation_ordinal = 5;
            }),
            exit_mismatch(),
        ),
        (
            "code_offset::tail_edge",
            4,
            Box::new(|row| {
                row.attribution.code_offset -= 1;
            }),
            invalid_attribution(SemanticCodeSite::Edge(edge_id(97))),
        ),
        (
            "byte_count::tail_edge",
            4,
            Box::new(|row| {
                row.attribution.byte_count = 0;
            }),
            exit_mismatch(),
        ),
    ];
    for (field, index, mutate, expected) in rejected {
        let mut changed = record.clone();
        mutate(&mut changed.semantic_code_attribution_mut_for_test()[index]);
        assert_ne!(changed, record, "{field}: substitution changes the row");
        assert_eq!(
            encode_installation_record(&changed),
            Err(expected),
            "{field}: canonical encoding rejects the substitution"
        );
    }

    // Roster-level custody: dropping an unjoined operation row still encodes,
    // so only the image binding rejects it; dropping the nominal return-edge
    // row breaks the boundary-joined tail at encoding, while a duplicated or
    // reordered roster hits the canonical ordering rule.
    let mut dropped_row = record.clone();
    dropped_row
        .semantic_code_attribution_mut_for_test()
        .remove(0);
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
    let mut dropped_edge_row = record.clone();
    dropped_edge_row
        .semantic_code_attribution_mut_for_test()
        .pop();
    assert_eq!(
        encode_installation_record(&dropped_edge_row),
        Err(exit_mismatch())
    );
    let mut duplicated_row = record.clone();
    let row = duplicated_row.semantic_code_attribution()[1].clone();
    duplicated_row
        .semantic_code_attribution_mut_for_test()
        .insert(2, row);
    assert_eq!(
        encode_installation_record(&duplicated_row),
        Err(InstallationError::NonCanonicalSemanticCodeAttributionOrder)
    );
    let mut reordered = record.clone();
    reordered
        .semantic_code_attribution_mut_for_test()
        .swap(1, 2);
    assert_eq!(
        encode_installation_record(&reordered),
        Err(InstallationError::NonCanonicalSemanticCodeAttributionOrder)
    );
}

/// Every representable field of an installed compiler-private callback row is
/// an authenticated custody axis: a one-field substitution either cannot
/// encode canonically or still encodes, recomputes a distinct installation
/// fingerprint, and independent replay against the unchanged image rejects it.
#[test]
fn installation_private_function_row_rejects_every_one_field_substitution() {
    let plan = callback_private_plan();
    let artifact =
        build_object_artifact_with_private_functions(&plan).expect("callback private artifact");
    let image = emit_executable_image(&artifact, 3).expect("callback private image");
    let record = build_installation_record(&image, ProfileDecisionId::new(53).expect("profile"))
        .expect("callback private installation");
    validate_installation_record(&record, &image).expect("exact image binding");
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");
    let [authentic] = record.private_functions() else {
        panic!("callback fixture retains one private row");
    };
    let text_offset = authentic.text_offset;
    let byte_count = authentic.byte_count;
    assert_eq!(authentic.machine, machine_id(97));
    assert_eq!(authentic.source_psi, identity());
    assert!(
        authentic
            .identity
            .callback_thunk_placement_index()
            .is_some()
    );

    type ReplayMutation = (
        &'static str,
        Box<dyn Fn(&mut image_emission::InstalledCompilerPrivateFunction)>,
    );
    // Semantic identities no canonical record-shape join pins: the substituted
    // row still encodes and decodes, so rejection is the recomputed identity
    // and the independent image replay.
    let still_encodes: Vec<ReplayMutation> = vec![
        (
            "identity::continuation",
            Box::new(|row| {
                row.identity = MachineFunctionIdentity::callback_thunk(
                    StateKey {
                        machine: SymbolHandle::from_parts(15, 2),
                        state: SymbolHandle::from_parts(13, 3),
                        segment_index: 0,
                    },
                    0,
                )
                .expect("substituted callback thunk identity");
            }),
        ),
        (
            "identity::placement_index",
            Box::new(|row| {
                row.identity = MachineFunctionIdentity::callback_thunk(
                    row.identity.associated_source_continuation(),
                    3,
                )
                .expect("substituted placement index");
            }),
        ),
        (
            "source_psi::program_fingerprint",
            Box::new(|row| {
                row.source_psi.program_fingerprint = SemanticFingerprint::from_bytes([7; 32]);
            }),
        ),
        (
            "machine",
            Box::new(|row| {
                row.machine = machine_id(98);
            }),
        ),
        (
            "scalar_abi::parameters::value",
            Box::new(|row| {
                row.scalar_abi.parameters[0].value =
                    semantic_vocabulary::ValueId::new(41).expect("substituted parameter value");
            }),
        ),
        (
            "scalar_abi::result::value",
            Box::new(|row| {
                row.scalar_abi.result.value =
                    semantic_vocabulary::ValueId::new(43).expect("substituted result value");
            }),
        ),
    ];
    for (field, mutate) in still_encodes {
        let mut changed = record.clone();
        mutate(&mut changed.private_functions_mut_for_test()[0]);
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
        Box<dyn Fn(&mut image_emission::InstalledCompilerPrivateFunction)>,
        InstallationError,
    );
    // Canonical record-shape joins reject every other one-field substitution
    // at encoding, before any identity or replay could accept it.
    let rejected: Vec<RejectedMutation> = vec![
        (
            "identity::source_kind",
            Box::new(|row| {
                row.identity =
                    MachineFunctionIdentity::source(row.identity.associated_source_continuation());
            }),
            InstallationError::InvalidCompilerPrivateFunction,
        ),
        (
            "text_offset",
            Box::new(move |row| {
                row.text_offset = text_offset + 1;
            }),
            InstallationError::InvalidCompilerPrivateFunction,
        ),
        (
            "byte_count::empty",
            Box::new(|row| {
                row.byte_count = 0;
            }),
            InstallationError::InvalidCompilerPrivateFunction,
        ),
        (
            "byte_count::extended",
            Box::new(move |row| {
                row.byte_count = byte_count + 1;
            }),
            InstallationError::InvalidImageSectionLayout,
        ),
        (
            "scalar_abi::parameters::placement",
            Box::new(|row| {
                row.scalar_abi.parameters[0].placement = row.scalar_abi.result.placement.clone();
            }),
            InstallationError::InvalidCompilerPrivateFunction,
        ),
        (
            "scalar_abi::result::collision",
            Box::new(|row| {
                row.scalar_abi.result.value = row.scalar_abi.parameters[0].value;
            }),
            InstallationError::InvalidCompilerPrivateFunction,
        ),
    ];
    for (field, mutate, expected) in rejected {
        let mut changed = record.clone();
        mutate(&mut changed.private_functions_mut_for_test()[0]);
        assert_ne!(changed, record, "{field}: substitution changes the row");
        assert_eq!(
            encode_installation_record(&changed),
            Err(expected),
            "{field}: canonical encoding rejects the substitution"
        );
    }

    // Roster-level custody: dropping the row leaves its text bytes unaccounted,
    // so the canonical section join rejects it at encoding.
    let mut dropped_row = record.clone();
    dropped_row.private_functions_mut_for_test().pop();
    assert_eq!(
        encode_installation_record(&dropped_row),
        Err(InstallationError::InvalidImageSectionLayout)
    );
}

#[test]
fn installation_dynamic_conformance_table_rejects_every_one_field_substitution() {
    let artifact = build_object_artifact(&dynamic_conformance_table_plan())
        .expect("dynamic-table object artifact");
    assert_eq!(artifact.dynamic_conformance_tables().len(), 1);
    assert_eq!(artifact.dynamic_conformance_tables()[0].slots.len(), 2);
    assert_eq!(
        artifact.dynamic_conformance_tables()[0].slots[0].target,
        Some(machine_id(2))
    );
    assert_eq!(
        artifact.dynamic_conformance_tables()[0].slots[1].target,
        None
    );
    let image = emit_executable_image(&artifact, 3).expect("dynamic-table image");
    let record = build_installation_record(&image, ProfileDecisionId::new(1).expect("profile"))
        .expect("dynamic-table installation");
    validate_installation_record(&record, &image).expect("authentic binding");
    assert_eq!(record.dynamic_conformance_tables().len(), 1);
    assert_eq!(record.dynamic_conformance_tables()[0].slots.len(), 2);
    assert_eq!(record.dynamic_calls().len(), 1);
    let authentic_fingerprint =
        installation_fingerprint(&record).expect("authentic installation fingerprint");

    let function_text_offset = record
        .functions()
        .iter()
        .find(|function| function.machine == machine_id(3))
        .expect("dynamic caller row")
        .text_offset;

    // These one-field substitutions remain representable: they encode and
    // decode canonically, recompute to a different installation identity, and
    // independent replay against the unchanged image rejects them.
    type StillEncodedMutation = (
        &'static str,
        Box<dyn Fn(&mut image_emission::InstallationRecord)>,
    );
    let still_encodes: Vec<StillEncodedMutation> = vec![
        (
            "table::application_report_fingerprint",
            Box::new(|record| {
                record.dynamic_conformance_tables_mut_for_test()[0]
                    .application_report_fingerprint = u64::MAX;
            }),
        ),
        (
            "table::slots[1]::target",
            Box::new(|record| {
                record.dynamic_conformance_tables_mut_for_test()[0].slots[1].target =
                    Some(machine_id(1));
            }),
        ),
        (
            "call::operation",
            Box::new(|record| {
                record.dynamic_calls_mut_for_test()[0].operation = operation_id(98);
            }),
        ),
        (
            "call::text_offset",
            Box::new(move |record| {
                record.dynamic_calls_mut_for_test()[0].text_offset = function_text_offset + 1;
            }),
        ),
        (
            "call::byte_count",
            Box::new(|record| {
                record.dynamic_calls_mut_for_test()[0].byte_count += 1;
            }),
        ),
    ];
    for (field, mutate) in still_encodes {
        let mut changed = record.clone();
        mutate(&mut changed);
        assert_ne!(changed, record, "{field}: substitution changes the record");
        let bytes = encode_installation_record(&changed)
            .unwrap_or_else(|error| panic!("{field}: substituted record encodes: {error:?}"));
        let replayed = decode_installation_record(&bytes)
            .unwrap_or_else(|error| panic!("{field}: substituted record decodes: {error:?}"));
        assert_eq!(
            replayed, changed,
            "{field}: codec preserves the substituted record"
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
            "{field}: independent replay rejects the substituted record"
        );
    }

    // Canonical record-shape joins reject every other one-field substitution
    // at encoding, before any identity or replay could accept it.
    let foreign_commitment =
        terminal_psi::ClosedConformanceApplicationCommitment::from_digest([7; 32]);
    type RejectedMutation = (
        &'static str,
        Box<dyn Fn(&mut image_emission::InstallationRecord)>,
        InstallationError,
    );
    let rejected: Vec<RejectedMutation> = vec![
        (
            "table::application_commitment",
            Box::new(move |record| {
                record.dynamic_conformance_tables_mut_for_test()[0].application_commitment =
                    foreign_commitment;
            }),
            InstallationError::InvalidDynamicCall(machine_id(3)),
        ),
        (
            "table::application_report_fingerprint::zero",
            Box::new(|record| {
                record.dynamic_conformance_tables_mut_for_test()[0]
                    .application_report_fingerprint = 0;
            }),
            InstallationError::InvalidDynamicConformanceTable,
        ),
        (
            "table::data_offset",
            Box::new(|record| {
                record.dynamic_conformance_tables_mut_for_test()[0].data_offset = 8;
            }),
            InstallationError::InvalidDynamicConformanceTable,
        ),
        (
            "table::byte_count",
            Box::new(|record| {
                record.dynamic_conformance_tables_mut_for_test()[0].byte_count = 8;
            }),
            InstallationError::InvalidDynamicConformanceTable,
        ),
        (
            "table::slots[0]::row_index",
            Box::new(|record| {
                record.dynamic_conformance_tables_mut_for_test()[0].slots[0].row_index = 1;
            }),
            InstallationError::InvalidDynamicConformanceTable,
        ),
        (
            "table::slots[0]::data_offset",
            Box::new(|record| {
                record.dynamic_conformance_tables_mut_for_test()[0].slots[0].data_offset = 8;
            }),
            InstallationError::InvalidDynamicConformanceTable,
        ),
        (
            "table::slots[0]::target::unknown",
            Box::new(|record| {
                record.dynamic_conformance_tables_mut_for_test()[0].slots[0].target =
                    Some(machine_id(98));
            }),
            InstallationError::InvalidDynamicConformanceTable,
        ),
        (
            "table::slots[0]::target::retargeted",
            Box::new(|record| {
                record.dynamic_conformance_tables_mut_for_test()[0].slots[0].target =
                    Some(machine_id(1));
            }),
            InstallationError::InvalidDynamicCall(machine_id(3)),
        ),
        (
            "table::slots[0]::target::erased",
            Box::new(|record| {
                record.dynamic_conformance_tables_mut_for_test()[0].slots[0].target = None;
            }),
            InstallationError::InvalidDynamicCall(machine_id(3)),
        ),
        (
            "table::slots[1]::row_index",
            Box::new(|record| {
                record.dynamic_conformance_tables_mut_for_test()[0].slots[1].row_index = 0;
            }),
            InstallationError::InvalidDynamicConformanceTable,
        ),
        (
            "table::slots[1]::data_offset",
            Box::new(|record| {
                record.dynamic_conformance_tables_mut_for_test()[0].slots[1].data_offset = 0;
            }),
            InstallationError::InvalidDynamicConformanceTable,
        ),
        (
            "table::slots[1]::target::unknown",
            Box::new(|record| {
                record.dynamic_conformance_tables_mut_for_test()[0].slots[1].target =
                    Some(machine_id(98));
            }),
            InstallationError::InvalidDynamicConformanceTable,
        ),
        (
            "call::machine::unknown",
            Box::new(|record| {
                record.dynamic_calls_mut_for_test()[0].machine = machine_id(98);
            }),
            InstallationError::InvalidDynamicCall(machine_id(98)),
        ),
        (
            "call::machine::other_function",
            Box::new(|record| {
                record.dynamic_calls_mut_for_test()[0].machine = machine_id(1);
            }),
            InstallationError::InvalidDynamicCall(machine_id(1)),
        ),
        (
            "call::application_commitment",
            Box::new(move |record| {
                record.dynamic_calls_mut_for_test()[0].application_commitment = foreign_commitment;
            }),
            InstallationError::InvalidDynamicCall(machine_id(3)),
        ),
        (
            "call::initial_source",
            Box::new(|record| {
                record.dynamic_calls_mut_for_test()[0].initial_source =
                    PlaceId::new(98).expect("substituted source");
            }),
            InstallationError::InvalidDynamicCall(machine_id(3)),
        ),
        (
            "call::rebound_source",
            Box::new(|record| {
                record.dynamic_calls_mut_for_test()[0].rebound_source =
                    PlaceId::new(98).expect("substituted source");
            }),
            InstallationError::InvalidDynamicCall(machine_id(3)),
        ),
        (
            "call::selected_table_byte_offset::unresolved_row",
            Box::new(|record| {
                record.dynamic_calls_mut_for_test()[0].selected_table_byte_offset = 8;
            }),
            InstallationError::InvalidDynamicCall(machine_id(3)),
        ),
        (
            "call::selected_table_byte_offset::misaligned",
            Box::new(|record| {
                record.dynamic_calls_mut_for_test()[0].selected_table_byte_offset = 7;
            }),
            InstallationError::InvalidDynamicCall(machine_id(3)),
        ),
        (
            "call::realization",
            Box::new(|record| {
                record.dynamic_calls_mut_for_test()[0].realization = machine_id(1);
            }),
            InstallationError::InvalidDynamicCall(machine_id(3)),
        ),
        (
            "call::text_offset::before_function",
            Box::new(move |record| {
                record.dynamic_calls_mut_for_test()[0].text_offset = function_text_offset - 1;
            }),
            InstallationError::InvalidDynamicCall(machine_id(3)),
        ),
        (
            "call::byte_count::empty",
            Box::new(|record| {
                record.dynamic_calls_mut_for_test()[0].byte_count = 0;
            }),
            InstallationError::InvalidDynamicCall(machine_id(3)),
        ),
    ];
    for (field, mutate, expected) in rejected {
        let mut changed = record.clone();
        mutate(&mut changed);
        assert_ne!(changed, record, "{field}: substitution changes the record");
        assert_eq!(
            encode_installation_record(&changed),
            Err(expected),
            "{field}: canonical encoding rejects the substitution"
        );
    }

    // Roster-level custody: reordering, dropping, or duplicating slots and
    // tables breaks canonical ordering, byte counts, or commitment closure
    // before any identity or replay could accept it.
    let mut swapped_slots = record.clone();
    swapped_slots.dynamic_conformance_tables_mut_for_test()[0]
        .slots
        .swap(0, 1);
    assert_eq!(
        encode_installation_record(&swapped_slots),
        Err(InstallationError::InvalidDynamicConformanceTable)
    );
    let mut dropped_slot = record.clone();
    dropped_slot.dynamic_conformance_tables_mut_for_test()[0]
        .slots
        .pop();
    assert_eq!(
        encode_installation_record(&dropped_slot),
        Err(InstallationError::InvalidDynamicConformanceTable)
    );
    let mut duplicated_slot = record.clone();
    let slot = duplicated_slot.dynamic_conformance_tables()[0].slots[1];
    duplicated_slot.dynamic_conformance_tables_mut_for_test()[0]
        .slots
        .push(slot);
    assert_eq!(
        encode_installation_record(&duplicated_slot),
        Err(InstallationError::InvalidDynamicConformanceTable)
    );
    let mut dropped_table = record.clone();
    dropped_table
        .dynamic_conformance_tables_mut_for_test()
        .pop();
    assert_eq!(
        encode_installation_record(&dropped_table),
        Err(InstallationError::InvalidImageSectionLayout)
    );
    let mut duplicated_table = record.clone();
    let table = duplicated_table.dynamic_conformance_tables()[0].clone();
    duplicated_table
        .dynamic_conformance_tables_mut_for_test()
        .push(table);
    assert_eq!(
        encode_installation_record(&duplicated_table),
        Err(InstallationError::InvalidDynamicConformanceTable)
    );
    let mut dropped_call = record.clone();
    dropped_call.dynamic_calls_mut_for_test().pop();
    assert_eq!(
        encode_installation_record(&dropped_call),
        Err(InstallationError::InvalidDynamicConformanceTable)
    );
    let mut duplicated_call = record.clone();
    let call = duplicated_call.dynamic_calls()[0];
    duplicated_call.dynamic_calls_mut_for_test().push(call);
    assert_eq!(
        encode_installation_record(&duplicated_call),
        Err(InstallationError::InvalidDynamicCall(machine_id(3)))
    );
}

#[test]
fn installation_dynamic_parameter_call_rejects_every_one_field_substitution() {
    let artifact = build_object_artifact(&dynamic_parameter_call_plan())
        .expect("dynamic-parameter object artifact");
    let image = emit_executable_image(&artifact, 3).expect("dynamic-parameter image");
    let record = build_installation_record(&image, ProfileDecisionId::new(1).expect("profile"))
        .expect("dynamic-parameter installation");
    validate_installation_record(&record, &image).expect("authentic binding");
    assert_eq!(record.dynamic_parameter_calls().len(), 1);
    let authentic = record.dynamic_parameter_calls()[0];
    assert_eq!(authentic.machine, machine_id(2));
    assert_eq!(authentic.operation, operation_id(2));
    assert_eq!(authentic.source_value, None);
    assert_eq!(authentic.requirement_slot, 0);
    let authentic_fingerprint =
        installation_fingerprint(&record).expect("authentic installation fingerprint");

    let caller = record
        .functions()
        .iter()
        .find(|function| function.machine == machine_id(2))
        .expect("dynamic-parameter caller row");
    let function_text_offset = caller.text_offset;
    let function_text_end = function_text_offset + caller.byte_count;
    assert_eq!(authentic.text_offset, function_text_offset);
    assert_eq!(authentic.byte_count, 8);

    // These one-field substitutions remain representable: they encode and
    // decode canonically, recompute to a different installation identity, and
    // independent replay against the unchanged image rejects them.
    type StillEncodedMutation = (
        &'static str,
        Box<dyn Fn(&mut image_emission::InstallationRecord)>,
    );
    let still_encodes: Vec<StillEncodedMutation> = vec![
        (
            "call::operation",
            Box::new(|record| {
                record.dynamic_parameter_calls_mut_for_test()[0].operation = operation_id(98);
            }),
        ),
        (
            "call::source_value",
            Box::new(|record| {
                record.dynamic_parameter_calls_mut_for_test()[0].source_value =
                    semantic_vocabulary::ValueId::new(98);
            }),
        ),
        (
            "call::requirement_slot",
            Box::new(|record| {
                record.dynamic_parameter_calls_mut_for_test()[0].requirement_slot = 1;
            }),
        ),
        (
            "call::text_offset",
            Box::new(move |record| {
                record.dynamic_parameter_calls_mut_for_test()[0].text_offset =
                    function_text_offset + 1;
            }),
        ),
        (
            "call::byte_count",
            Box::new(|record| {
                record.dynamic_parameter_calls_mut_for_test()[0].byte_count += 1;
            }),
        ),
    ];
    for (field, mutate) in still_encodes {
        let mut changed = record.clone();
        mutate(&mut changed);
        assert_ne!(changed, record, "{field}: substitution changes the record");
        let bytes = encode_installation_record(&changed)
            .unwrap_or_else(|error| panic!("{field}: substituted record encodes: {error:?}"));
        let replayed = decode_installation_record(&bytes)
            .unwrap_or_else(|error| panic!("{field}: substituted record decodes: {error:?}"));
        assert_eq!(
            replayed, changed,
            "{field}: codec preserves the substituted record"
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
            "{field}: independent replay rejects the substituted record"
        );
    }

    // Canonical record-shape joins reject every other one-field substitution
    // at encoding, before any identity or replay could accept it.
    type RejectedMutation = (
        &'static str,
        Box<dyn Fn(&mut image_emission::InstallationRecord)>,
        InstallationError,
    );
    let rejected: Vec<RejectedMutation> = vec![
        (
            "call::machine::unknown",
            Box::new(|record| {
                record.dynamic_parameter_calls_mut_for_test()[0].machine = machine_id(98);
            }),
            InstallationError::InvalidDynamicParameterCall(machine_id(98)),
        ),
        (
            "call::machine::other_function",
            Box::new(|record| {
                record.dynamic_parameter_calls_mut_for_test()[0].machine = machine_id(1);
            }),
            InstallationError::InvalidDynamicParameterCall(machine_id(1)),
        ),
        (
            "call::text_offset::before_function",
            Box::new(move |record| {
                record.dynamic_parameter_calls_mut_for_test()[0].text_offset =
                    function_text_offset - 1;
            }),
            InstallationError::InvalidDynamicParameterCall(machine_id(2)),
        ),
        (
            "call::text_offset::past_function",
            Box::new(move |record| {
                record.dynamic_parameter_calls_mut_for_test()[0].text_offset = function_text_end;
            }),
            InstallationError::InvalidDynamicParameterCall(machine_id(2)),
        ),
        (
            "call::byte_count::empty",
            Box::new(|record| {
                record.dynamic_parameter_calls_mut_for_test()[0].byte_count = 0;
            }),
            InstallationError::InvalidDynamicParameterCall(machine_id(2)),
        ),
        (
            "call::byte_count::past_function",
            Box::new(|record| {
                record.dynamic_parameter_calls_mut_for_test()[0].byte_count += 10;
            }),
            InstallationError::InvalidDynamicParameterCall(machine_id(2)),
        ),
    ];
    for (field, mutate, expected) in rejected {
        let mut changed = record.clone();
        mutate(&mut changed);
        assert_ne!(changed, record, "{field}: substitution changes the record");
        assert_eq!(
            encode_installation_record(&changed),
            Err(expected),
            "{field}: canonical encoding rejects the substitution"
        );
    }

    // Roster-level custody: dropping the row still encodes, but its recomputed
    // identity diverges and independent replay rejects it; duplicating the row
    // collides on the canonical (machine, operation) call site.
    let mut dropped_call = record.clone();
    dropped_call.dynamic_parameter_calls_mut_for_test().pop();
    let dropped_bytes =
        encode_installation_record(&dropped_call).expect("dropped dynamic-parameter call encodes");
    let dropped =
        decode_installation_record(&dropped_bytes).expect("dropped dynamic-parameter call decodes");
    assert_ne!(
        installation_fingerprint(&dropped).expect("dropped fingerprint"),
        authentic_fingerprint,
        "dropped row recomputes a different installation identity"
    );
    assert_eq!(
        validate_installation_record(&dropped, &image),
        Err(InstallationError::ImageBindingMismatch),
        "independent replay rejects the dropped row"
    );
    let mut duplicated_call = record.clone();
    let call = duplicated_call.dynamic_parameter_calls()[0];
    duplicated_call
        .dynamic_parameter_calls_mut_for_test()
        .push(call);
    assert_eq!(
        encode_installation_record(&duplicated_call),
        Err(InstallationError::InvalidDynamicParameterCall(machine_id(
            2
        )))
    );
}

#[test]
fn installation_stored_dynamic_call_rejects_every_one_field_substitution() {
    let artifact =
        build_object_artifact(&stored_dynamic_call_plan()).expect("stored dynamic object artifact");
    assert_eq!(artifact.dynamic_conformance_tables().len(), 1);
    assert_eq!(artifact.dynamic_conformance_tables()[0].slots.len(), 2);
    let image = emit_executable_image(&artifact, 3).expect("stored dynamic image");
    let record = build_installation_record(&image, ProfileDecisionId::new(1).expect("profile"))
        .expect("stored dynamic installation");
    validate_installation_record(&record, &image).expect("authentic binding");
    assert_eq!(record.dynamic_conformance_tables().len(), 1);
    assert_eq!(record.stored_dynamic_calls().len(), 1);
    let authentic = record.stored_dynamic_calls()[0];
    assert_eq!(authentic.machine, machine_id(3));
    assert_eq!(authentic.establishment_operation, operation_id(4));
    assert_eq!(authentic.operation, operation_id(5));
    assert_eq!(authentic.descriptor_ordinal, 0);
    assert_eq!(authentic.selection_ordinal, 0);
    assert_eq!(authentic.source, PlaceId::new(1).expect("place"));
    assert_eq!(authentic.descriptor_home_byte_offset, 0);
    assert_eq!(authentic.selected_table_byte_offset, 0);
    assert_eq!(authentic.realization, machine_id(2));
    let authentic_fingerprint =
        installation_fingerprint(&record).expect("authentic installation fingerprint");

    let caller = record
        .functions()
        .iter()
        .find(|function| function.machine == machine_id(3))
        .expect("stored dynamic caller row");
    let function_text_offset = caller.text_offset;
    let function_text_end = function_text_offset + caller.byte_count;
    assert_eq!(authentic.establishment_text_offset, function_text_offset);
    assert_eq!(authentic.establishment_byte_count, 25);
    assert_eq!(authentic.text_offset, function_text_offset + 25);
    assert_eq!(authentic.byte_count, 32);

    // These one-field substitutions remain representable: they encode and
    // decode canonically, recompute to a different installation identity, and
    // independent replay against the unchanged image rejects them.
    type StillEncodedMutation = (
        &'static str,
        Box<dyn Fn(&mut image_emission::InstallationRecord)>,
    );
    let still_encodes: Vec<StillEncodedMutation> = vec![
        (
            "table::application_report_fingerprint",
            Box::new(|record| {
                record.dynamic_conformance_tables_mut_for_test()[0]
                    .application_report_fingerprint = u64::MAX;
            }),
        ),
        (
            "table::slots[1]::target",
            Box::new(|record| {
                record.dynamic_conformance_tables_mut_for_test()[0].slots[1].target =
                    Some(machine_id(1));
            }),
        ),
        (
            "call::operation",
            Box::new(|record| {
                record.stored_dynamic_calls_mut_for_test()[0].operation = operation_id(98);
            }),
        ),
        (
            "call::establishment_operation",
            Box::new(|record| {
                record.stored_dynamic_calls_mut_for_test()[0].establishment_operation =
                    operation_id(98);
            }),
        ),
        (
            "call::descriptor_ordinal",
            Box::new(|record| {
                record.stored_dynamic_calls_mut_for_test()[0].descriptor_ordinal = 1;
            }),
        ),
        (
            "call::selection_ordinal",
            Box::new(|record| {
                record.stored_dynamic_calls_mut_for_test()[0].selection_ordinal = 1;
            }),
        ),
        (
            "call::descriptor_home_byte_offset",
            Box::new(|record| {
                record.stored_dynamic_calls_mut_for_test()[0].descriptor_home_byte_offset = 8;
            }),
        ),
        (
            "call::establishment_byte_count",
            Box::new(|record| {
                record.stored_dynamic_calls_mut_for_test()[0].establishment_byte_count -= 1;
            }),
        ),
        (
            "call::text_offset",
            Box::new(move |record| {
                record.stored_dynamic_calls_mut_for_test()[0].text_offset =
                    function_text_offset + 26;
            }),
        ),
        (
            "call::byte_count",
            Box::new(|record| {
                record.stored_dynamic_calls_mut_for_test()[0].byte_count -= 1;
            }),
        ),
    ];
    for (field, mutate) in still_encodes {
        let mut changed = record.clone();
        mutate(&mut changed);
        assert_ne!(changed, record, "{field}: substitution changes the record");
        let bytes = encode_installation_record(&changed)
            .unwrap_or_else(|error| panic!("{field}: substituted record encodes: {error:?}"));
        let replayed = decode_installation_record(&bytes)
            .unwrap_or_else(|error| panic!("{field}: substituted record decodes: {error:?}"));
        assert_eq!(
            replayed, changed,
            "{field}: codec preserves the substituted record"
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
            "{field}: independent replay rejects the substituted record"
        );
    }

    // Canonical record-shape joins reject every other one-field substitution
    // at encoding, before any identity or replay could accept it.
    let foreign_commitment =
        terminal_psi::ClosedConformanceApplicationCommitment::from_digest([7; 32]);
    type RejectedMutation = (
        &'static str,
        Box<dyn Fn(&mut image_emission::InstallationRecord)>,
        InstallationError,
    );
    let rejected: Vec<RejectedMutation> = vec![
        (
            "call::machine::unknown",
            Box::new(|record| {
                record.stored_dynamic_calls_mut_for_test()[0].machine = machine_id(98);
            }),
            InstallationError::InvalidStoredDynamicCall(machine_id(98)),
        ),
        (
            "call::machine::other_function",
            Box::new(|record| {
                record.stored_dynamic_calls_mut_for_test()[0].machine = machine_id(1);
            }),
            InstallationError::InvalidStoredDynamicCall(machine_id(1)),
        ),
        (
            "call::source::unknown_place",
            Box::new(|record| {
                record.stored_dynamic_calls_mut_for_test()[0].source =
                    PlaceId::new(98).expect("place");
            }),
            InstallationError::InvalidStoredDynamicCall(machine_id(3)),
        ),
        (
            "call::application_commitment",
            Box::new(move |record| {
                record.stored_dynamic_calls_mut_for_test()[0].application_commitment =
                    foreign_commitment;
            }),
            InstallationError::InvalidStoredDynamicCall(machine_id(3)),
        ),
        (
            "call::selected_table_byte_offset::unresolved_row",
            Box::new(|record| {
                record.stored_dynamic_calls_mut_for_test()[0].selected_table_byte_offset = 8;
            }),
            InstallationError::InvalidStoredDynamicCall(machine_id(3)),
        ),
        (
            "call::selected_table_byte_offset::misaligned",
            Box::new(|record| {
                record.stored_dynamic_calls_mut_for_test()[0].selected_table_byte_offset = 7;
            }),
            InstallationError::InvalidStoredDynamicCall(machine_id(3)),
        ),
        (
            "call::selected_table_byte_offset::past_rows",
            Box::new(|record| {
                record.stored_dynamic_calls_mut_for_test()[0].selected_table_byte_offset = 16;
            }),
            InstallationError::InvalidStoredDynamicCall(machine_id(3)),
        ),
        (
            "call::realization::not_selected",
            Box::new(|record| {
                record.stored_dynamic_calls_mut_for_test()[0].realization = machine_id(1);
            }),
            InstallationError::InvalidStoredDynamicCall(machine_id(3)),
        ),
        (
            "call::descriptor_home_byte_offset::misaligned",
            Box::new(|record| {
                record.stored_dynamic_calls_mut_for_test()[0].descriptor_home_byte_offset = 4;
            }),
            InstallationError::InvalidStoredDynamicCall(machine_id(3)),
        ),
        (
            "call::establishment_text_offset::before_function",
            Box::new(move |record| {
                record.stored_dynamic_calls_mut_for_test()[0].establishment_text_offset =
                    function_text_offset - 1;
            }),
            InstallationError::InvalidStoredDynamicCall(machine_id(3)),
        ),
        (
            "call::establishment_text_offset::inside",
            Box::new(move |record| {
                record.stored_dynamic_calls_mut_for_test()[0].establishment_text_offset =
                    function_text_offset + 1;
            }),
            InstallationError::InvalidStoredDynamicCall(machine_id(3)),
        ),
        (
            "call::establishment_byte_count::empty",
            Box::new(|record| {
                record.stored_dynamic_calls_mut_for_test()[0].establishment_byte_count = 0;
            }),
            InstallationError::InvalidStoredDynamicCall(machine_id(3)),
        ),
        (
            "call::establishment_byte_count::overlaps_call",
            Box::new(|record| {
                record.stored_dynamic_calls_mut_for_test()[0].establishment_byte_count += 1;
            }),
            InstallationError::InvalidStoredDynamicCall(machine_id(3)),
        ),
        (
            "call::text_offset::inside_establishment",
            Box::new(move |record| {
                record.stored_dynamic_calls_mut_for_test()[0].text_offset =
                    function_text_offset + 24;
            }),
            InstallationError::InvalidStoredDynamicCall(machine_id(3)),
        ),
        (
            "call::text_offset::past_function",
            Box::new(move |record| {
                record.stored_dynamic_calls_mut_for_test()[0].text_offset = function_text_end;
            }),
            InstallationError::InvalidStoredDynamicCall(machine_id(3)),
        ),
        (
            "call::byte_count::empty",
            Box::new(|record| {
                record.stored_dynamic_calls_mut_for_test()[0].byte_count = 0;
            }),
            InstallationError::InvalidStoredDynamicCall(machine_id(3)),
        ),
        (
            "call::byte_count::past_function",
            Box::new(|record| {
                record.stored_dynamic_calls_mut_for_test()[0].byte_count += 100;
            }),
            InstallationError::InvalidStoredDynamicCall(machine_id(3)),
        ),
    ];
    for (field, mutate, expected) in rejected {
        let mut changed = record.clone();
        mutate(&mut changed);
        assert_ne!(changed, record, "{field}: substitution changes the record");
        assert_eq!(
            encode_installation_record(&changed),
            Err(expected),
            "{field}: canonical encoding rejects the substitution"
        );
    }

    // Roster-level custody: dropping the only stored call leaves the emitted
    // conformance table unreferenced, a duplicated call collides on the
    // canonical establishment/dispatch ordering, and table roster mutations
    // hit the data-layout and commitment canonicality joins.
    let mut dropped_call = record.clone();
    dropped_call.stored_dynamic_calls_mut_for_test().pop();
    assert_eq!(
        encode_installation_record(&dropped_call),
        Err(InstallationError::InvalidDynamicConformanceTable)
    );
    let mut duplicated_call = record.clone();
    let call = duplicated_call.stored_dynamic_calls()[0];
    duplicated_call
        .stored_dynamic_calls_mut_for_test()
        .push(call);
    assert_eq!(
        encode_installation_record(&duplicated_call),
        Err(InstallationError::InvalidStoredDynamicCall(machine_id(3)))
    );
    let mut dropped_table = record.clone();
    dropped_table
        .dynamic_conformance_tables_mut_for_test()
        .pop();
    assert_eq!(
        encode_installation_record(&dropped_table),
        Err(InstallationError::InvalidImageSectionLayout)
    );
    let mut duplicated_table = record.clone();
    let table = duplicated_table.dynamic_conformance_tables()[0].clone();
    duplicated_table
        .dynamic_conformance_tables_mut_for_test()
        .push(table);
    assert_eq!(
        encode_installation_record(&duplicated_table),
        Err(InstallationError::InvalidDynamicConformanceTable)
    );
    let mut swapped_slots = record.clone();
    swapped_slots.dynamic_conformance_tables_mut_for_test()[0]
        .slots
        .swap(0, 1);
    assert_eq!(
        encode_installation_record(&swapped_slots),
        Err(InstallationError::InvalidDynamicConformanceTable)
    );
    let mut dropped_slot = record.clone();
    dropped_slot.dynamic_conformance_tables_mut_for_test()[0]
        .slots
        .pop();
    assert_eq!(
        encode_installation_record(&dropped_slot),
        Err(InstallationError::InvalidDynamicConformanceTable)
    );
    let mut duplicated_slot = record.clone();
    let slot = duplicated_slot.dynamic_conformance_tables()[0].slots[0];
    duplicated_slot.dynamic_conformance_tables_mut_for_test()[0]
        .slots
        .push(slot);
    assert_eq!(
        encode_installation_record(&duplicated_slot),
        Err(InstallationError::InvalidDynamicConformanceTable)
    );
}

#[test]
fn installation_forwarded_dynamic_parameter_call_rejects_every_one_field_substitution() {
    let artifact = build_object_artifact(&forwarded_dynamic_parameter_call_plan())
        .expect("forwarded dynamic-parameter object artifact");
    let image = emit_executable_image(&artifact, 3).expect("forwarded dynamic-parameter image");
    let record = build_installation_record(&image, ProfileDecisionId::new(1).expect("profile"))
        .expect("forwarded dynamic-parameter installation");
    validate_installation_record(&record, &image).expect("authentic binding");
    assert_eq!(record.dynamic_parameter_calls().len(), 1);
    assert_eq!(record.forwarded_dynamic_parameter_calls().len(), 1);
    let authentic = record.forwarded_dynamic_parameter_calls()[0];
    assert_eq!(authentic.machine, machine_id(2));
    assert_eq!(authentic.operation, operation_id(2));
    assert_eq!(authentic.callee, machine_id(1));
    assert_eq!(
        authentic.source_value,
        Some(semantic_vocabulary::ValueId::new(91).expect("forwarded result"))
    );
    assert_eq!(
        authentic.scalar_type,
        Some(semantic_vocabulary::ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 32)
                .expect("i32 scalar type")
        ))
    );
    assert_eq!(authentic.source_parameter_ordinal, 0);
    assert_eq!(authentic.target_parameter_ordinal, 0);
    let authentic_fingerprint =
        installation_fingerprint(&record).expect("authentic installation fingerprint");

    let caller = record
        .functions()
        .iter()
        .find(|function| function.machine == machine_id(2))
        .expect("forwarded dynamic-parameter caller row");
    let function_text_offset = caller.text_offset;
    let function_text_end = function_text_offset + caller.byte_count;
    assert_eq!(authentic.text_offset, function_text_offset);
    assert_eq!(authentic.byte_count, 7);

    // These one-field substitutions remain representable: they encode and
    // decode canonically, recompute to a different installation identity, and
    // independent replay against the unchanged image rejects them.
    type StillEncodedMutation = (
        &'static str,
        Box<dyn Fn(&mut image_emission::InstallationRecord)>,
    );
    let still_encodes: Vec<StillEncodedMutation> = vec![
        (
            "call::operation",
            Box::new(|record| {
                record.forwarded_dynamic_parameter_calls_mut_for_test()[0].operation =
                    operation_id(98);
            }),
        ),
        (
            "call::source_value",
            Box::new(|record| {
                record.forwarded_dynamic_parameter_calls_mut_for_test()[0].source_value =
                    semantic_vocabulary::ValueId::new(98);
            }),
        ),
        (
            "call::scalar_type",
            Box::new(|record| {
                record.forwarded_dynamic_parameter_calls_mut_for_test()[0].scalar_type =
                    Some(semantic_vocabulary::ScalarType::Boolean);
            }),
        ),
        (
            "call::callee::self_function",
            Box::new(|record| {
                record.forwarded_dynamic_parameter_calls_mut_for_test()[0].callee = machine_id(2);
            }),
        ),
        (
            "call::text_offset",
            Box::new(move |record| {
                record.forwarded_dynamic_parameter_calls_mut_for_test()[0].text_offset =
                    function_text_offset + 1;
            }),
        ),
        (
            "call::byte_count",
            Box::new(|record| {
                record.forwarded_dynamic_parameter_calls_mut_for_test()[0].byte_count += 1;
            }),
        ),
    ];
    for (field, mutate) in still_encodes {
        let mut changed = record.clone();
        mutate(&mut changed);
        assert_ne!(changed, record, "{field}: substitution changes the record");
        let bytes = encode_installation_record(&changed)
            .unwrap_or_else(|error| panic!("{field}: substituted record encodes: {error:?}"));
        let replayed = decode_installation_record(&bytes)
            .unwrap_or_else(|error| panic!("{field}: substituted record decodes: {error:?}"));
        assert_eq!(
            replayed, changed,
            "{field}: codec preserves the substituted record"
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
            "{field}: independent replay rejects the substituted record"
        );
    }

    // Canonical record-shape joins reject every other one-field substitution
    // at encoding, before any identity or replay could accept it.
    type RejectedMutation = (
        &'static str,
        Box<dyn Fn(&mut image_emission::InstallationRecord)>,
        InstallationError,
    );
    let rejected: Vec<RejectedMutation> = vec![
        (
            "call::machine::unknown",
            Box::new(|record| {
                record.forwarded_dynamic_parameter_calls_mut_for_test()[0].machine = machine_id(98);
            }),
            InstallationError::InvalidForwardedDynamicParameterCall(machine_id(98)),
        ),
        (
            "call::machine::other_function",
            Box::new(|record| {
                record.forwarded_dynamic_parameter_calls_mut_for_test()[0].machine = machine_id(1);
            }),
            InstallationError::InvalidForwardedDynamicParameterCall(machine_id(1)),
        ),
        (
            "call::callee::unknown",
            Box::new(|record| {
                record.forwarded_dynamic_parameter_calls_mut_for_test()[0].callee = machine_id(98);
            }),
            InstallationError::InvalidForwardedDynamicParameterCall(machine_id(2)),
        ),
        (
            "call::source_parameter_ordinal",
            Box::new(|record| {
                record.forwarded_dynamic_parameter_calls_mut_for_test()[0]
                    .source_parameter_ordinal = 1;
            }),
            InstallationError::InvalidForwardedDynamicParameterCall(machine_id(2)),
        ),
        (
            "call::target_parameter_ordinal",
            Box::new(|record| {
                record.forwarded_dynamic_parameter_calls_mut_for_test()[0]
                    .target_parameter_ordinal = 1;
            }),
            InstallationError::InvalidForwardedDynamicParameterCall(machine_id(2)),
        ),
        (
            "call::source_value::cleared",
            Box::new(|record| {
                record.forwarded_dynamic_parameter_calls_mut_for_test()[0].source_value = None;
            }),
            InstallationError::InvalidForwardedDynamicParameterCall(machine_id(2)),
        ),
        (
            "call::scalar_type::cleared",
            Box::new(|record| {
                record.forwarded_dynamic_parameter_calls_mut_for_test()[0].scalar_type = None;
            }),
            InstallationError::InvalidForwardedDynamicParameterCall(machine_id(2)),
        ),
        (
            "call::scalar_type::float",
            Box::new(|record| {
                record.forwarded_dynamic_parameter_calls_mut_for_test()[0].scalar_type =
                    Some(semantic_vocabulary::ScalarType::IeeeFloat(
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    ));
            }),
            InstallationError::InvalidForwardedDynamicParameterCall(machine_id(2)),
        ),
        (
            "call::text_offset::before_function",
            Box::new(move |record| {
                record.forwarded_dynamic_parameter_calls_mut_for_test()[0].text_offset =
                    function_text_offset - 1;
            }),
            InstallationError::InvalidForwardedDynamicParameterCall(machine_id(2)),
        ),
        (
            "call::text_offset::past_function",
            Box::new(move |record| {
                record.forwarded_dynamic_parameter_calls_mut_for_test()[0].text_offset =
                    function_text_end;
            }),
            InstallationError::InvalidForwardedDynamicParameterCall(machine_id(2)),
        ),
        (
            "call::byte_count::empty",
            Box::new(|record| {
                record.forwarded_dynamic_parameter_calls_mut_for_test()[0].byte_count = 0;
            }),
            InstallationError::InvalidForwardedDynamicParameterCall(machine_id(2)),
        ),
    ];
    for (field, mutate, expected) in rejected {
        let mut changed = record.clone();
        mutate(&mut changed);
        assert_ne!(changed, record, "{field}: substitution changes the record");
        assert_eq!(
            encode_installation_record(&changed),
            Err(expected),
            "{field}: canonical encoding rejects the substitution"
        );
    }

    // Roster-level custody: dropping the row still encodes, but its recomputed
    // identity diverges and independent replay rejects it; duplicating the row
    // collides on the canonical (machine, operation) call site.
    let mut dropped_call = record.clone();
    dropped_call
        .forwarded_dynamic_parameter_calls_mut_for_test()
        .pop();
    let dropped_bytes = encode_installation_record(&dropped_call)
        .expect("dropped forwarded dynamic-parameter call encodes");
    let dropped = decode_installation_record(&dropped_bytes)
        .expect("dropped forwarded dynamic-parameter call decodes");
    assert_ne!(
        installation_fingerprint(&dropped).expect("dropped fingerprint"),
        authentic_fingerprint,
        "dropped row recomputes a different installation identity"
    );
    assert_eq!(
        validate_installation_record(&dropped, &image),
        Err(InstallationError::ImageBindingMismatch),
        "independent replay rejects the dropped row"
    );
    let mut duplicated_call = record.clone();
    let call = duplicated_call.forwarded_dynamic_parameter_calls()[0];
    duplicated_call
        .forwarded_dynamic_parameter_calls_mut_for_test()
        .push(call);
    assert_eq!(
        encode_installation_record(&duplicated_call),
        Err(InstallationError::InvalidForwardedDynamicParameterCall(
            machine_id(2)
        ))
    );
}

#[test]
fn installation_forwarded_dynamic_descriptor_rejects_every_one_field_substitution() {
    let artifact = build_object_artifact(&forwarded_dynamic_descriptor_call_plan())
        .expect("forwarded dynamic-descriptor object artifact");
    let image = emit_executable_image(&artifact, 3).expect("forwarded dynamic-descriptor image");
    let record = build_installation_record(&image, ProfileDecisionId::new(1).expect("profile"))
        .expect("forwarded dynamic-descriptor installation");
    validate_installation_record(&record, &image).expect("authentic binding");
    assert_eq!(record.forwarded_dynamic_descriptor_adapters().len(), 1);
    assert_eq!(record.forwarded_dynamic_descriptor_tables().len(), 1);
    assert_eq!(record.forwarded_dynamic_descriptor_calls().len(), 1);
    let authentic_adapter = record.forwarded_dynamic_descriptor_adapters()[0];
    let authentic_table = &record.forwarded_dynamic_descriptor_tables()[0];
    let authentic_call = &record.forwarded_dynamic_descriptor_calls()[0];
    assert_eq!(authentic_adapter.row_index, 0);
    assert_eq!(authentic_adapter.realization, machine_id(2));
    assert_eq!(authentic_adapter.byte_count, 17);
    assert_eq!(authentic_table.data_offset, 0);
    assert_eq!(authentic_table.byte_count, 8);
    assert_eq!(authentic_table.slots.len(), 1);
    assert_eq!(authentic_table.slots[0].row_index, 0);
    assert_eq!(authentic_table.slots[0].realization, machine_id(2));
    assert_eq!(
        authentic_table.slots[0].adapter_text_offset,
        authentic_adapter.text_offset
    );
    assert_eq!(authentic_table.slots[0].data_offset, 0);
    assert_eq!(authentic_call.machine, machine_id(3));
    assert_eq!(authentic_call.operation, operation_id(4));
    assert_eq!(authentic_call.callee, machine_id(1));
    assert_eq!(
        authentic_call.application_commitment,
        authentic_table.application_commitment
    );
    assert_eq!(
        authentic_call.source,
        StructuralArgument {
            place: PlaceId::new(1).expect("place"),
            path: Vec::new(),
            access: StructuralAccess::SharedBorrow,
        }
    );
    assert_eq!(authentic_call.semantic_result, None);
    assert_eq!(authentic_call.result, None);
    let authentic_fingerprint =
        installation_fingerprint(&record).expect("authentic installation fingerprint");

    let caller = record
        .functions()
        .iter()
        .find(|function| function.machine == machine_id(3))
        .expect("forwarded dynamic-descriptor caller row");
    let function_text_offset = caller.text_offset;
    let function_text_end = function_text_offset + caller.byte_count;
    assert_eq!(authentic_call.text_offset, function_text_offset);
    assert_eq!(authentic_call.byte_count, 24);
    assert!(authentic_adapter.text_offset >= function_text_end);

    // These one-field substitutions remain representable: they encode and
    // decode canonically, recompute to a different installation identity, and
    // independent replay against the unchanged image rejects them.
    type StillEncodedMutation = (
        &'static str,
        Box<dyn Fn(&mut image_emission::InstallationRecord)>,
    );
    let still_encodes: Vec<StillEncodedMutation> = vec![
        (
            "table::application_report_fingerprint",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_tables_mut_for_test()[0]
                    .application_report_fingerprint = u64::MAX;
            }),
        ),
        (
            "call::operation",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_calls_mut_for_test()[0].operation =
                    operation_id(98);
            }),
        ),
        (
            "call::callee::other_function",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_calls_mut_for_test()[0].callee = machine_id(2);
            }),
        ),
        (
            "call::source::place",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_calls_mut_for_test()[0]
                    .source
                    .place = PlaceId::new(98).expect("place");
            }),
        ),
        (
            "call::source::access",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_calls_mut_for_test()[0]
                    .source
                    .access = StructuralAccess::Owned;
            }),
        ),
        (
            "call::text_offset",
            Box::new(move |record| {
                record.forwarded_dynamic_descriptor_calls_mut_for_test()[0].text_offset =
                    function_text_offset + 1;
            }),
        ),
        (
            "call::byte_count",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_calls_mut_for_test()[0].byte_count -= 1;
            }),
        ),
    ];
    for (field, mutate) in still_encodes {
        let mut changed = record.clone();
        mutate(&mut changed);
        assert_ne!(changed, record, "{field}: substitution changes the record");
        let bytes = encode_installation_record(&changed)
            .unwrap_or_else(|error| panic!("{field}: substituted record encodes: {error:?}"));
        let replayed = decode_installation_record(&bytes)
            .unwrap_or_else(|error| panic!("{field}: substituted record decodes: {error:?}"));
        assert_eq!(
            replayed, changed,
            "{field}: codec preserves the substituted record"
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
            "{field}: independent replay rejects the substituted record"
        );
    }

    // Canonical record-shape joins reject every other one-field substitution
    // at encoding, before any identity or replay could accept it.
    let foreign_commitment =
        terminal_psi::ClosedConformanceApplicationCommitment::from_digest([7; 32]);
    type RejectedMutation = (
        &'static str,
        Box<dyn Fn(&mut image_emission::InstallationRecord)>,
        InstallationError,
    );
    let rejected: Vec<RejectedMutation> = vec![
        (
            "adapter::application_commitment",
            Box::new(move |record| {
                record.forwarded_dynamic_descriptor_adapters_mut_for_test()[0]
                    .application_commitment = foreign_commitment;
            }),
            InstallationError::InvalidForwardedDynamicDescriptorTable,
        ),
        (
            "adapter::row_index",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_adapters_mut_for_test()[0].row_index = 1;
            }),
            InstallationError::InvalidForwardedDynamicDescriptorTable,
        ),
        (
            "adapter::realization",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_adapters_mut_for_test()[0].realization =
                    machine_id(1);
            }),
            InstallationError::InvalidForwardedDynamicDescriptorTable,
        ),
        (
            "adapter::text_offset",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_adapters_mut_for_test()[0].text_offset += 1;
            }),
            InstallationError::InvalidForwardedDynamicDescriptorTable,
        ),
        (
            "adapter::byte_count::empty",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_adapters_mut_for_test()[0].byte_count = 0;
            }),
            InstallationError::InvalidForwardedDynamicDescriptorAdapter,
        ),
        (
            "adapter::byte_count::past_text_end",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_adapters_mut_for_test()[0].byte_count += 1;
            }),
            InstallationError::InvalidImageSectionLayout,
        ),
        (
            "table::application_commitment",
            Box::new(move |record| {
                record.forwarded_dynamic_descriptor_tables_mut_for_test()[0]
                    .application_commitment = foreign_commitment;
            }),
            InstallationError::InvalidForwardedDynamicDescriptorTable,
        ),
        (
            "table::data_offset",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_tables_mut_for_test()[0].data_offset = 8;
            }),
            InstallationError::InvalidForwardedDynamicDescriptorTable,
        ),
        (
            "table::byte_count",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_tables_mut_for_test()[0].byte_count = 16;
            }),
            InstallationError::InvalidForwardedDynamicDescriptorTable,
        ),
        (
            "table::slots[0]::row_index",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_tables_mut_for_test()[0].slots[0].row_index = 1;
            }),
            InstallationError::InvalidForwardedDynamicDescriptorTable,
        ),
        (
            "table::slots[0]::realization::unknown",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_tables_mut_for_test()[0].slots[0].realization =
                    machine_id(98);
            }),
            InstallationError::InvalidForwardedDynamicDescriptorTable,
        ),
        (
            "table::slots[0]::realization::not_adapter",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_tables_mut_for_test()[0].slots[0].realization =
                    machine_id(1);
            }),
            InstallationError::InvalidForwardedDynamicDescriptorTable,
        ),
        (
            "table::slots[0]::adapter_text_offset",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_tables_mut_for_test()[0].slots[0]
                    .adapter_text_offset += 1;
            }),
            InstallationError::InvalidForwardedDynamicDescriptorTable,
        ),
        (
            "table::slots[0]::data_offset",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_tables_mut_for_test()[0].slots[0].data_offset =
                    8;
            }),
            InstallationError::InvalidForwardedDynamicDescriptorTable,
        ),
        (
            "call::machine::unknown",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_calls_mut_for_test()[0].machine =
                    machine_id(98);
            }),
            InstallationError::InvalidForwardedDynamicDescriptorCall(machine_id(98)),
        ),
        (
            "call::machine::other_function",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_calls_mut_for_test()[0].machine = machine_id(1);
            }),
            InstallationError::InvalidForwardedDynamicDescriptorCall(machine_id(1)),
        ),
        (
            "call::callee::unknown",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_calls_mut_for_test()[0].callee = machine_id(98);
            }),
            InstallationError::InvalidForwardedDynamicDescriptorCall(machine_id(3)),
        ),
        (
            "call::application_commitment",
            Box::new(move |record| {
                record.forwarded_dynamic_descriptor_calls_mut_for_test()[0]
                    .application_commitment = foreign_commitment;
            }),
            InstallationError::InvalidForwardedDynamicDescriptorCall(machine_id(3)),
        ),
        (
            "call::semantic_result::without_result",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_calls_mut_for_test()[0].semantic_result =
                    Some(abstract_operations::AbstractResult {
                        value: semantic_vocabulary::ValueId::new(98).expect("value"),
                        scalar_type: semantic_vocabulary::ScalarType::Boolean,
                    });
            }),
            InstallationError::InvalidForwardedDynamicDescriptorCall(machine_id(3)),
        ),
        (
            "call::text_offset::before_function",
            Box::new(move |record| {
                record.forwarded_dynamic_descriptor_calls_mut_for_test()[0].text_offset =
                    function_text_offset - 1;
            }),
            InstallationError::InvalidForwardedDynamicDescriptorCall(machine_id(3)),
        ),
        (
            "call::text_offset::past_function",
            Box::new(move |record| {
                record.forwarded_dynamic_descriptor_calls_mut_for_test()[0].text_offset =
                    function_text_end;
            }),
            InstallationError::InvalidForwardedDynamicDescriptorCall(machine_id(3)),
        ),
        (
            "call::byte_count::empty",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_calls_mut_for_test()[0].byte_count = 0;
            }),
            InstallationError::InvalidForwardedDynamicDescriptorCall(machine_id(3)),
        ),
        (
            "call::byte_count::past_function",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_calls_mut_for_test()[0].byte_count += 100;
            }),
            InstallationError::InvalidForwardedDynamicDescriptorCall(machine_id(3)),
        ),
    ];
    for (field, mutate, expected) in rejected {
        let mut changed = record.clone();
        mutate(&mut changed);
        assert_ne!(changed, record, "{field}: substitution changes the record");
        assert_eq!(
            encode_installation_record(&changed),
            Err(expected),
            "{field}: canonical encoding rejects the substitution"
        );
    }

    // Roster-level custody: dropping the only call leaves the forwarded table
    // unreferenced, a duplicated call collides on the canonical (machine,
    // operation) call site, and adapter/table/slot roster mutations each hit
    // their canonicality joins.
    let mut dropped_call = record.clone();
    dropped_call
        .forwarded_dynamic_descriptor_calls_mut_for_test()
        .pop();
    assert_eq!(
        encode_installation_record(&dropped_call),
        Err(InstallationError::InvalidForwardedDynamicDescriptorTable)
    );
    let mut duplicated_call = record.clone();
    let call = duplicated_call.forwarded_dynamic_descriptor_calls()[0].clone();
    duplicated_call
        .forwarded_dynamic_descriptor_calls_mut_for_test()
        .push(call);
    assert_eq!(
        encode_installation_record(&duplicated_call),
        Err(InstallationError::InvalidForwardedDynamicDescriptorCall(
            machine_id(3)
        ))
    );
    let mut dropped_adapter = record.clone();
    dropped_adapter
        .forwarded_dynamic_descriptor_adapters_mut_for_test()
        .pop();
    assert_eq!(
        encode_installation_record(&dropped_adapter),
        Err(InstallationError::InvalidForwardedDynamicDescriptorTable)
    );
    let mut duplicated_adapter = record.clone();
    let adapter = duplicated_adapter.forwarded_dynamic_descriptor_adapters()[0];
    duplicated_adapter
        .forwarded_dynamic_descriptor_adapters_mut_for_test()
        .push(adapter);
    assert_eq!(
        encode_installation_record(&duplicated_adapter),
        Err(InstallationError::InvalidForwardedDynamicDescriptorAdapter)
    );
    let mut dropped_table = record.clone();
    dropped_table
        .forwarded_dynamic_descriptor_tables_mut_for_test()
        .pop();
    assert_eq!(
        encode_installation_record(&dropped_table),
        Err(InstallationError::InvalidImageSectionLayout)
    );
    let mut duplicated_table = record.clone();
    let table = duplicated_table.forwarded_dynamic_descriptor_tables()[0].clone();
    duplicated_table
        .forwarded_dynamic_descriptor_tables_mut_for_test()
        .push(table);
    assert_eq!(
        encode_installation_record(&duplicated_table),
        Err(InstallationError::InvalidForwardedDynamicDescriptorTable)
    );
    let mut dropped_slot = record.clone();
    dropped_slot.forwarded_dynamic_descriptor_tables_mut_for_test()[0]
        .slots
        .pop();
    assert_eq!(
        encode_installation_record(&dropped_slot),
        Err(InstallationError::InvalidForwardedDynamicDescriptorTable)
    );
    let mut duplicated_slot = record.clone();
    let slot = duplicated_slot.forwarded_dynamic_descriptor_tables()[0].slots[0];
    duplicated_slot.forwarded_dynamic_descriptor_tables_mut_for_test()[0]
        .slots
        .push(slot);
    assert_eq!(
        encode_installation_record(&duplicated_slot),
        Err(InstallationError::InvalidForwardedDynamicDescriptorTable)
    );
}

/// Every representable field of an installed privileged port-effect row is an
/// authenticated custody axis: a one-field substitution either cannot encode
/// canonically or still encodes, recomputes a distinct installation
/// fingerprint, and independent replay against the unchanged image rejects
/// it. The fixture retains one unbound effect and one effect consumed by an
/// admitted-provider `MetadataOnlyPort` settlement, so both the free semantic
/// axes and the axes pinned by the settlement join are exercised.
#[test]
fn installation_port_effect_rejects_every_one_field_substitution() {
    let provider = WriteExitProvider(7);
    let plan = port_effect_plan(&provider);
    let artifact = build_object_artifact(&plan).expect("port-effect artifact");
    let image = emit_executable_image(&artifact, 3).expect("port-effect image");
    let record = build_installation_record_with_provider_executions(
        &image,
        ProfileDecisionId::new(17).expect("profile"),
        [&provider],
    )
    .expect("port-effect installation");
    validate_installation_record(&record, &image).expect("exact image binding");
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");
    let [unbound, bound] = record.port_effects() else {
        panic!("port-effect fixture retains two rows");
    };
    assert_eq!(unbound.effect.psi_operation, operation_id(1));
    assert_eq!(bound.effect.psi_operation, operation_id(2));

    // The unbound row's semantic axes are independently representable: each
    // one-field substitution still encodes canonically, recomputes a distinct
    // installation fingerprint, and independent replay against the unchanged
    // image rejects it.
    let representable: [(&str, fn(&mut image_emission::ObjectPortEffect)); 5] = [
        ("psi_operation", |row| {
            row.effect.psi_operation = operation_id(96);
        }),
        ("service", |row| {
            row.effect.service = ServiceId::new(96).expect("drifted service");
        }),
        ("port", |row| {
            row.effect.port = 0x64;
        }),
        ("value", |row| {
            row.effect.value = 0x7f;
        }),
        ("operation_ordinal", |row| {
            row.effect.operation_ordinal = 9;
        }),
    ];
    for (field, mutate) in representable {
        let mut changed = record.clone();
        mutate(&mut changed.port_effects_mut_for_test()[0]);
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

    // Physical axes are canonical projections: the row must name an installed
    // machine, sit at `function.text_offset + code_offset`, and span exactly
    // the emitted `out` sequence. Drifting any of them fails at encoding.
    let unbound_encode_rejected: [(
        &str,
        fn(&mut image_emission::ObjectPortEffect),
        InstallationError,
    ); 4] = [
        ("machine", |row| row.machine = machine_id(96), {
            InstallationError::EffectMachineMissing(machine_id(96))
        }),
        ("code_offset", |row| row.effect.code_offset += 1, {
            InstallationError::InvalidPortEffectOffset {
                machine: machine_id(1),
                operation: operation_id(1),
            }
        }),
        ("byte_count", |row| row.effect.byte_count -= 1, {
            InstallationError::InvalidPortEffectOffset {
                machine: machine_id(1),
                operation: operation_id(1),
            }
        }),
        ("text_offset", |row| row.text_offset += 1, {
            InstallationError::InvalidPortEffectOffset {
                machine: machine_id(1),
                operation: operation_id(1),
            }
        }),
    ];
    for (field, mutate, expected) in unbound_encode_rejected {
        let mut changed = record.clone();
        mutate(&mut changed.port_effects_mut_for_test()[0]);
        assert_eq!(
            encode_installation_record(&changed),
            Err(expected),
            "{field}: non-canonical substitution rejected at encoding"
        );
    }

    // Every field of the settlement-consumed row is pinned: the physical axes
    // fail the same canonical projection joins, while the semantic axes fail
    // the `MetadataOnlyPort` realization's exact effect lookup.
    let bound_encode_rejected: [(
        &str,
        fn(&mut image_emission::ObjectPortEffect),
        InstallationError,
    ); 9] = [
        ("machine", |row| row.machine = machine_id(96), {
            InstallationError::EffectMachineMissing(machine_id(96))
        }),
        (
            "psi_operation",
            |row| row.effect.psi_operation = operation_id(96),
            {
                InstallationError::BoundaryRealizationMismatch {
                    machine: machine_id(1),
                    operation: operation_id(3),
                }
            },
        ),
        (
            "service",
            |row| row.effect.service = ServiceId::new(96).unwrap(),
            {
                InstallationError::BoundaryRealizationMismatch {
                    machine: machine_id(1),
                    operation: operation_id(3),
                }
            },
        ),
        ("port", |row| row.effect.port = 0x64, {
            InstallationError::BoundaryRealizationMismatch {
                machine: machine_id(1),
                operation: operation_id(3),
            }
        }),
        ("value", |row| row.effect.value = 0x7f, {
            InstallationError::BoundaryRealizationMismatch {
                machine: machine_id(1),
                operation: operation_id(3),
            }
        }),
        (
            "operation_ordinal",
            |row| row.effect.operation_ordinal = 9,
            {
                InstallationError::BoundaryRealizationMismatch {
                    machine: machine_id(1),
                    operation: operation_id(3),
                }
            },
        ),
        ("code_offset", |row| row.effect.code_offset += 1, {
            InstallationError::InvalidPortEffectOffset {
                machine: machine_id(1),
                operation: operation_id(2),
            }
        }),
        ("byte_count", |row| row.effect.byte_count -= 1, {
            InstallationError::InvalidPortEffectOffset {
                machine: machine_id(1),
                operation: operation_id(2),
            }
        }),
        ("text_offset", |row| row.text_offset += 1, {
            InstallationError::InvalidPortEffectOffset {
                machine: machine_id(1),
                operation: operation_id(2),
            }
        }),
    ];
    for (field, mutate, expected) in bound_encode_rejected {
        let mut changed = record.clone();
        mutate(&mut changed.port_effects_mut_for_test()[1]);
        assert_eq!(
            encode_installation_record(&changed),
            Err(expected),
            "{field}: non-canonical substitution rejected at encoding"
        );
    }

    // Dropping the unbound row still encodes: replay then rejects it because
    // the retained roster no longer matches the image. Dropping the consumed
    // row instead orphans the settlement join and fails at encoding, as do a
    // duplicated `(machine, psi_operation)` pair and a non-canonical order.
    let mut dropped_unbound = record.clone();
    dropped_unbound.port_effects_mut_for_test().remove(0);
    let bytes =
        encode_installation_record(&dropped_unbound).expect("dropped unbound effect still encodes");
    let replayed = decode_installation_record(&bytes).expect("dropped unbound effect decodes");
    assert_ne!(
        installation_fingerprint(&replayed).expect("substituted fingerprint"),
        authentic_fingerprint
    );
    assert_eq!(
        validate_installation_record(&replayed, &image),
        Err(InstallationError::ImageBindingMismatch),
        "dropped unbound row: independent replay rejects the roster"
    );
    let mut dropped_bound = record.clone();
    dropped_bound.port_effects_mut_for_test().remove(1);
    assert_eq!(
        encode_installation_record(&dropped_bound),
        Err(InstallationError::BoundaryRealizationMismatch {
            machine: machine_id(1),
            operation: operation_id(3),
        })
    );
    let mut duplicated_operation = record.clone();
    let mut duplicate = duplicated_operation.port_effects()[1].clone();
    duplicate.effect.psi_operation = operation_id(1);
    duplicate.effect.operation_ordinal = 5;
    duplicated_operation
        .port_effects_mut_for_test()
        .push(duplicate);
    assert_eq!(
        encode_installation_record(&duplicated_operation),
        Err(InstallationError::DuplicatePortEffectOperation {
            machine: machine_id(1),
            operation: operation_id(1),
        })
    );
    let mut reordered = record.clone();
    reordered.port_effects_mut_for_test().swap(0, 1);
    assert_eq!(
        encode_installation_record(&reordered),
        Err(InstallationError::NonCanonicalPortEffectOrder)
    );
}

#[test]
fn object_boundary_rejects_noncanonical_or_incomplete_machine_code_plans() {
    let mut reordered = two_function_plan();
    reordered.functions.swap(0, 1);
    assert_eq!(
        build_object_artifact(&reordered),
        Err(ObjectError::NonCanonicalFunctionOrder {
            previous: machine_id(2),
            current: machine_id(1),
        })
    );

    let mut missing_entry = two_function_plan();
    missing_entry.entry = machine_id(3);
    assert_eq!(
        build_object_artifact(&missing_entry),
        Err(ObjectError::EntryFunctionMissing(machine_id(3)))
    );

    let mut empty_function = two_function_plan();
    empty_function.functions[0].bytes.clear();
    assert_eq!(
        build_object_artifact(&empty_function),
        Err(ObjectError::EmptyFunction(machine_id(1)))
    );
}

#[test]
fn legacy_internal_call_rejects_an_installed_provider_origin_with_unchanged_bytes() {
    let mut plan = internal_call_plan(NativeTarget::linux_x64());
    account_x86_unit_call(&mut plan);
    build_object_artifact(&plan).expect("valid authored legacy call");
    let call = &mut plan.functions[1].internal_unit_calls[0];
    let boundary = semantic_vocabulary::BoundaryMachineId::new(1).unwrap();
    call.source = machine_code::InternalUnitCallSource::InstalledProvider {
        boundary,
        provider: Box::new(terminal_psi::ProviderCandidateConformance {
            boundary,
            requirement_identity: "requirement".into(),
            provider_identity: "provider".into(),
            candidate_identity: "candidate".into(),
            candidate: call.target,
            signature: terminal_psi::ProviderSignature {
                parameters: Vec::new(),
            },
            refinement: terminal_psi::ProviderRefinement {
                positional_parameters: Vec::new(),
                required_domains: Vec::new(),
                realized_service_ceiling: Vec::new(),
            },
        }),
        completion_claim_sources: Vec::new(),
        completion_receipts: Vec::new(),
    };
    assert_eq!(
        build_object_artifact(&plan),
        Err(ObjectError::InvalidInternalUnitCallEvidence(machine_id(2)))
    );
}

#[test]
fn legacy_call_free_function_rejects_an_incoming_pointer_role() {
    let mut plan = two_function_plan();
    build_object_artifact(&plan).expect("valid legacy leaf roster");
    plan.functions[0]
        .unit_parameter_homes
        .push(UnitParameterHomeRecord {
            place: semantic_vocabulary::PlaceId::new(1).unwrap(),
            structural_type: semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
            multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
            access: terminal_psi::StructuralAccess::Owned,
            shape: calling_conventions::ValueShape::integer(16, 8),
            source: calling_conventions::ValuePlacement {
                shape: calling_conventions::ValueShape::integer(16, 8),
                locations: Vec::new(),
            },
            location: machine_code::StructuralSourceLocation::IncomingIndirectPointer {
                register: calling_conventions::MachineRegister::X86Rcx,
            },
            indirect: true,
        });
    assert_eq!(
        build_object_artifact(&plan),
        Err(ObjectError::InvalidInternalUnitCallEvidence(machine_id(1)))
    );
    for location in [
        calling_conventions::IndirectPointerLocation::Register(
            calling_conventions::MachineRegister::X86Rcx,
        ),
        calling_conventions::IndirectPointerLocation::Stack {
            stack_byte_offset: 32,
            alignment: 8,
        },
    ] {
        plan.functions[0].unit_parameter_homes[0].location =
            machine_code::StructuralSourceLocation::IncomingBorrowedPointer { location };
        plan.functions[0].unit_parameter_homes[0].access =
            terminal_psi::StructuralAccess::WriteOnlyBorrow;
        assert_eq!(
            build_object_artifact(&plan),
            Err(ObjectError::InvalidInternalUnitCallEvidence(machine_id(1)))
        );
    }
}

#[test]
fn x86_internal_call_is_a_typed_relocation_and_the_only_final_text_mutation() {
    let mut plan = internal_call_plan(NativeTarget::linux_x64());
    account_x86_unit_call(&mut plan);
    let full_width_operation = operation_id(u64::from(u32::MAX) + 1);
    plan.functions[1].provenance.operations[0] = full_width_operation;
    plan.functions[1].internal_calls[0].owner =
        target_operations::CallSiteOwner::Operation(full_width_operation);
    plan.functions[1].internal_unit_calls[0].owner =
        target_operations::CallSiteOwner::Operation(full_width_operation);
    if let SemanticCodeSite::Operation(operation) =
        &mut plan.functions[1].semantic_code_attribution[0].site
    {
        *operation = full_width_operation;
    }
    let artifact = build_object_artifact(&plan).expect("terminal object artifact");
    assert_eq!(artifact.functions()[1].unit_stack.unwrap().frame_bytes, 0);
    assert_eq!(
        artifact.functions()[1].unit_stack.unwrap().local_peak_bytes,
        16
    );
    assert_eq!(artifact.functions()[1].unit_call_stacks.len(), 1);
    assert_eq!(
        artifact.functions()[1].unit_call_stacks[0].caller_live_bytes,
        16
    );

    assert_eq!(artifact.relocations().record_count(), 1);
    let (_, relocation) = artifact.relocations().records().next().expect("relocation");
    assert_eq!(relocation.kind, RelocationKind::X86_64Relative32);
    assert_eq!(relocation.section, SectionKind::Text);
    assert_eq!(relocation.offset, 11);
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

    let container = emit_object_container(&artifact);
    assert_eq!(container.output.relocations, 1);
    let image = emit_executable_image(&artifact, 3).expect("Linux x86-64 image");
    let output = image.output();
    assert_eq!(&output.final_text_bytes[11..15], &[0xf1, 0xff, 0xff, 0xff]);
    assert_eq!(output.final_image_relocations, 1);
    let evidence = output
        .compiler_text_validation
        .expect("relocation evidence");
    assert_ne!(
        evidence.encoded_text_report_fingerprint,
        evidence.final_compiler_text_report_fingerprint
    );
    assert_eq!(evidence.text_relocation_count, 1);

    let record = build_installation_record(&image, ProfileDecisionId::new(1).unwrap())
        .expect("installation record");
    validate_installation_record(&record, &image).expect("image binding");
}

#[test]
fn aarch64_internal_call_patches_only_the_branch_immediate() {
    let plan = internal_call_plan(NativeTarget::linux_arm64());
    let artifact = build_object_artifact(&plan).expect("terminal object artifact");
    let (_, relocation) = artifact.relocations().records().next().expect("relocation");
    assert_eq!(relocation.kind, RelocationKind::Aarch64Branch26);
    assert_eq!(relocation.offset, 8);

    let image = emit_executable_image(&artifact, 3).expect("Linux AArch64 image");
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
        build_object_artifact(&unknown_target),
        Err(ObjectError::UnknownInternalCallTarget {
            caller: machine_id(2),
            target: machine_id(3),
        })
    );

    let mut invalid_site = internal_call_plan(NativeTarget::linux_x64());
    invalid_site.functions[1].bytes[0] = 0x90;
    assert_eq!(
        build_object_artifact(&invalid_site),
        Err(ObjectError::InvalidInternalCallSite {
            caller: machine_id(2),
            owner: target_operations::CallSiteOwner::Operation(operation_id(2)),
            offset: 1,
        })
    );

    let mut duplicate_site = internal_call_plan(NativeTarget::linux_x64());
    let duplicate_call = duplicate_site.functions[1].internal_calls[0];
    duplicate_site.functions[1]
        .internal_calls
        .push(duplicate_call);
    assert_eq!(
        build_object_artifact(&duplicate_site),
        Err(ObjectError::NonCanonicalInternalCallOrder(machine_id(2)))
    );

    let mut missing_provenance = internal_call_plan(NativeTarget::linux_x64());
    missing_provenance.functions[1]
        .provenance
        .operations
        .clear();
    assert_eq!(
        build_object_artifact(&missing_provenance),
        Err(ObjectError::InternalCallOperationNotInProvenance {
            caller: machine_id(2),
            owner: target_operations::CallSiteOwner::Operation(operation_id(2)),
        })
    );

    let mut duplicate_operation = internal_call_plan(NativeTarget::linux_x64());
    duplicate_operation.functions[1].bytes = vec![0xe8, 0, 0, 0, 0, 0xe8, 0, 0, 0, 0, 0xc3];
    duplicate_operation.functions[1]
        .internal_calls
        .push(InternalCallRelocation {
            owner: target_operations::CallSiteOwner::Operation(operation_id(2)),
            target: machine_id(1),
            unit_stack: None,
            scalar_stack: None,
            offset: 6,
        });
    assert_eq!(
        build_object_artifact(&duplicate_operation),
        Err(ObjectError::DuplicateInternalCallOperation {
            caller: machine_id(2),
            owner: target_operations::CallSiteOwner::Operation(operation_id(2)),
        })
    );
}

#[test]
fn object_boundary_rejects_drifted_unit_stack_evidence() {
    let mut missing_call = internal_call_plan(NativeTarget::linux_x64());
    missing_call.functions[1].unit_stack = Some(UnitStackEvidence {
        frame: None,
        aarch64_return_link: None,
        stack_alignment: 16,
    });
    add_empty_unit_cleanup(&mut missing_call.functions[1]);
    assert_eq!(
        build_object_artifact(&missing_call),
        Err(ObjectError::MissingUnitCallStackEvidence {
            caller: machine_id(2),
            owner: target_operations::CallSiteOwner::Operation(operation_id(2)),
        })
    );

    let mut removed_allocation = internal_call_plan(NativeTarget::linux_x64());
    account_x86_unit_call(&mut removed_allocation);
    removed_allocation.functions[1].bytes.drain(0..4);
    removed_allocation.functions[1].internal_calls[0].offset -= 4;
    assert_eq!(
        build_object_artifact(&removed_allocation),
        Err(ObjectError::InvalidUnitStackEncoding {
            machine: machine_id(2),
            owner: Some(target_operations::CallSiteOwner::Operation(operation_id(2))),
            offset: 0,
        })
    );

    let mut missing_adjustment = internal_call_plan(NativeTarget::linux_x64());
    missing_adjustment.functions[1].unit_stack = Some(UnitStackEvidence {
        frame: None,
        aarch64_return_link: None,
        stack_alignment: 16,
    });
    add_empty_unit_cleanup(&mut missing_adjustment.functions[1]);
    missing_adjustment.functions[1].internal_calls[0].unit_stack =
        Some(UnitCallStackEvidence { outbound: None });
    assert_eq!(
        build_object_artifact(&missing_adjustment),
        Err(ObjectError::MissingX86UnitCallStackAdjustment {
            caller: machine_id(2),
            owner: target_operations::CallSiteOwner::Operation(operation_id(2)),
        })
    );

    let mut unclaimed_adjustment = internal_call_plan(NativeTarget::linux_x64());
    account_x86_unit_call(&mut unclaimed_adjustment);
    unclaimed_adjustment.functions[1].bytes.splice(
        13..13,
        [
            0x48, 0x83, 0xec, 0x08, // unclaimed sub rsp, 8
            0x48, 0x83, 0xc4, 0x08, // unclaimed add rsp, 8
        ],
    );
    let return_offset = unclaimed_adjustment.functions[1].bytes.len() - 1;
    let cleanup = unclaimed_adjustment.functions[1]
        .unit_affine_cleanup
        .as_mut()
        .unwrap();
    cleanup.code_offset = return_offset;
    unclaimed_adjustment.functions[1].semantic_code_attribution[1].code_offset = return_offset;
    assert_eq!(
        build_object_artifact(&unclaimed_adjustment),
        Err(ObjectError::UnclaimedUnitStackAdjustment {
            machine: machine_id(2),
            offset: 13,
        })
    );

    let mut unclaimed_aarch64_adjustment = internal_call_plan(NativeTarget::linux_arm64());
    account_aarch64_unit_call(&mut unclaimed_aarch64_adjustment);
    insert_aarch64_word(
        &mut unclaimed_aarch64_adjustment.functions[1].bytes,
        8,
        0xd100_43ff,
    );
    insert_aarch64_word(
        &mut unclaimed_aarch64_adjustment.functions[1].bytes,
        12,
        0x9100_43ff,
    );
    let caller = &mut unclaimed_aarch64_adjustment.functions[1];
    caller.internal_calls[0].offset = 16;
    caller.internal_unit_calls[0].byte_count = 12;
    caller.semantic_code_attribution[0].byte_count = 12;
    caller.semantic_code_attribution[1].code_offset = 20;
    let cleanup = caller.unit_affine_cleanup.as_mut().unwrap();
    cleanup.code_offset = 20;
    cleanup.byte_count = 12;
    let stack = caller.unit_stack.as_mut().expect("AArch64 Unit stack");
    stack
        .frame
        .as_mut()
        .expect("AArch64 Unit frame")
        .release_offset = 24;
    stack
        .aarch64_return_link
        .as_mut()
        .expect("AArch64 return link")
        .load_offset = 20;
    assert_eq!(
        build_object_artifact(&unclaimed_aarch64_adjustment),
        Err(ObjectError::UnclaimedUnitStackAdjustment {
            machine: machine_id(2),
            offset: 8,
        })
    );
}

#[test]
fn executable_nominal_cleanup_call_is_edge_owned_and_survives_installation() {
    let plan = two_call_edge_owned_cleanup_plan();
    let cleanup_edge = edge_id(3);
    let artifact = build_object_artifact(&plan).expect("edge-owned cleanup artifact");
    let caller = artifact
        .functions()
        .iter()
        .find(|function| function.machine == machine_id(3))
        .expect("caller function");
    assert_eq!(caller.unit_call_stacks.len(), 1);
    assert_eq!(
        caller.unit_call_stacks[0].owner,
        CallSiteOwner::CleanupAction {
            edge: cleanup_edge,
            action_ordinal: 0,
        }
    );
    assert_eq!(caller.unit_call_stacks[0].target, machine_id(1));
    assert_eq!(caller.unit_call_stacks[0].caller_live_bytes, 16);
    assert_eq!(
        derive_stack_demand(&artifact, machine_id(3))
            .expect("edge cleanup stack closure")
            .ceiling_bytes(),
        32
    );
    let drop = artifact
        .functions()
        .iter()
        .find(|function| function.machine == machine_id(1))
        .expect("drop function");
    assert_eq!(drop.unit_call_stacks.len(), 2);
    assert_eq!(
        drop.unit_call_stacks
            .iter()
            .map(|call| (call.owner, call.target))
            .collect::<Vec<_>>(),
        [
            (CallSiteOwner::Operation(operation_id(1)), machine_id(2)),
            (CallSiteOwner::Operation(operation_id(2)), machine_id(4)),
        ]
    );
    let relocation = artifact
        .relocations()
        .records()
        .map(|(_, relocation)| relocation)
        .find(|relocation| {
            matches!(
                relocation.origin,
                RelocationOrigin::SemanticEdge { edge_identity, .. }
                    if edge_identity == cleanup_edge.get()
            )
        })
        .expect("edge relocation");
    assert!(matches!(
        relocation.origin,
        RelocationOrigin::SemanticEdge { edge_identity, .. }
            if edge_identity == cleanup_edge.get()
    ));

    let image = emit_executable_image(&artifact, 3).expect("cleanup image");
    let installation =
        build_installation_record(&image, ProfileDecisionId::new(1).expect("profile"))
            .expect("cleanup installation");
    let installed = installation
        .internal_unit_calls()
        .iter()
        .find(|call| call.machine == machine_id(3))
        .expect("installed cleanup call");
    assert_eq!(
        installed.custody.owner,
        CallSiteOwner::CleanupAction {
            edge: cleanup_edge,
            action_ordinal: 0,
        }
    );
    assert!(installed.custody.arguments.is_empty());
    assert!(installed.custody.claim_transfers.is_empty());
    assert_eq!(installation.internal_unit_calls().len(), 3);
    let installed_caller = installation
        .functions()
        .iter()
        .find(|function| function.machine == machine_id(3))
        .expect("installed cleanup caller");
    assert_eq!(installed_caller.unit_stack, caller.unit_stack);
    assert_eq!(installed_caller.unit_call_stacks, caller.unit_call_stacks);
    let encoded = encode_installation_record(&installation).expect("encoded cleanup");
    let decoded = decode_installation_record(&encoded).expect("decoded cleanup");
    assert_eq!(decoded, installation);
    assert_eq!(
        derive_installation_stack_demand(&decoded, &image, machine_id(3))
            .expect("installed cleanup stack closure"),
        derive_stack_demand(&artifact, machine_id(3)).expect("object cleanup stack closure")
    );
    validate_installation_record(&installation, &image).expect("installed cleanup binding");

    let mut missing_call = plan;
    missing_call.functions[2].internal_calls.clear();
    missing_call.functions[2].internal_unit_calls.clear();
    assert_eq!(
        build_object_artifact(&missing_call),
        Err(ObjectError::InvalidUnitAffineCleanupEvidence(machine_id(3)))
    );

    let mut duplicate_helper = two_call_edge_owned_cleanup_plan();
    duplicate_helper.functions[0].internal_calls[1].target = machine_id(2);
    duplicate_helper.functions[0].internal_unit_calls[1].target = machine_id(2);
    assert_eq!(
        build_object_artifact(&duplicate_helper),
        Err(ObjectError::InvalidUnitAffineCleanupEvidence(machine_id(3)))
    );
}

#[test]
fn scalar_cleanup_custody_and_structural_homes_survive_image_installation() {
    let mut plan = edge_owned_cleanup_plan();
    let caller = &mut plan.functions[2];
    promote_x86_cleanup_to_scalar(caller);

    let artifact = build_object_artifact(&plan).expect("scalar cleanup object");
    let object_caller = artifact
        .functions()
        .iter()
        .find(|function| function.machine == machine_id(3))
        .expect("scalar cleanup caller");
    assert!(object_caller.unit_affine_cleanup.is_none());
    assert!(object_caller.scalar_affine_cleanup.is_some());
    assert!(object_caller.unit_stack.is_none());
    assert_eq!(object_caller.scalar_stack.unwrap().local_peak_bytes, 32);
    assert_eq!(object_caller.scalar_structural_parameter_homes.len(), 1);
    assert!(object_caller.unit_parameters.is_empty());

    let image = emit_executable_image(&artifact, 3).expect("scalar cleanup image");
    let installation =
        build_installation_record(&image, ProfileDecisionId::new(1).expect("profile"))
            .expect("scalar cleanup installation");
    let installed = installation
        .functions()
        .iter()
        .find(|function| function.machine == machine_id(3))
        .expect("installed scalar cleanup caller");
    assert!(!installed.unit_body);
    assert!(installed.scalar_affine_cleanup.is_some());
    assert_eq!(installed.scalar_stack, object_caller.scalar_stack);
    assert_eq!(
        installed.scalar_call_stacks,
        object_caller.scalar_call_stacks
    );
    assert_eq!(installed.scalar_structural_parameter_homes.len(), 1);
    let encoded = encode_installation_record(&installation).expect("canonical install");
    let decoded = decode_installation_record(&encoded).expect("decoded install");
    assert_eq!(decoded, installation);
    validate_installation_record(&installation, &image).expect("scalar cleanup image binding");

    let demand =
        derive_stack_demand(&artifact, machine_id(3)).expect("scalar cleanup stack closure");
    assert_eq!(demand.ceiling_bytes(), 48);
    assert_eq!(
        derive_installation_stack_demand(&decoded, &image, machine_id(3))
            .expect("installed scalar stack closure"),
        demand
    );
    assert_eq!(
        demand
            .contributing_machines()
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        [machine_id(1), machine_id(2), machine_id(3)]
    );

    let cleanup_owner = plan.functions[2].internal_calls[0].owner;
    let mut missing_function_evidence = plan.clone();
    missing_function_evidence.functions[2].scalar_stack = None;
    assert_eq!(
        build_object_artifact(&missing_function_evidence),
        Err(ObjectError::UnexpectedScalarCallStackEvidence {
            caller: machine_id(3),
            owner: cleanup_owner,
        })
    );

    let mut missing_call_evidence = plan.clone();
    missing_call_evidence.functions[2].internal_calls[0].scalar_stack = None;
    assert_eq!(
        build_object_artifact(&missing_call_evidence),
        Err(ObjectError::MissingScalarCallStackEvidence {
            caller: machine_id(3),
            owner: cleanup_owner,
        })
    );

    let mut conflicting_domains = plan.clone();
    conflicting_domains.functions[2].unit_stack = Some(UnitStackEvidence {
        frame: None,
        aarch64_return_link: None,
        stack_alignment: 16,
    });
    assert_eq!(
        build_object_artifact(&conflicting_domains),
        Err(ObjectError::ConflictingTerminalStackEvidence(machine_id(3)))
    );

    let mut missing_preservation = plan.clone();
    missing_preservation.functions[2]
        .scalar_stack
        .as_mut()
        .expect("scalar stack")
        .cleanup_preservation = None;
    assert_eq!(
        build_object_artifact(&missing_preservation),
        Err(ObjectError::InvalidUnitAffineCleanupEvidence(machine_id(3)))
    );

    let mut corrupted_result_store = plan.clone();
    let store_offset = corrupted_result_store.functions[2]
        .scalar_stack
        .as_ref()
        .and_then(|stack| stack.cleanup_preservation)
        .expect("cleanup preservation")
        .result_store_offset;
    corrupted_result_store.functions[2].bytes[store_offset + 1] = 0x8b;
    assert_eq!(
        build_object_artifact(&corrupted_result_store),
        Err(ObjectError::InvalidUnitAffineCleanupEvidence(machine_id(3)))
    );

    let mut overflowed_preservation = plan.clone();
    overflowed_preservation.functions[2]
        .scalar_stack
        .as_mut()
        .and_then(|stack| stack.cleanup_preservation.as_mut())
        .expect("cleanup preservation")
        .result_load_offset = usize::MAX;
    assert_eq!(
        build_object_artifact(&overflowed_preservation),
        Err(ObjectError::InvalidUnitAffineCleanupEvidence(machine_id(3)))
    );

    let mut call_outside_cleanup = plan;
    call_outside_cleanup.functions[2]
        .scalar_affine_cleanup
        .as_mut()
        .expect("scalar cleanup")
        .code_offset = 10;
    assert_eq!(
        build_object_artifact(&call_outside_cleanup),
        Err(ObjectError::InvalidUnitAffineCleanupEvidence(machine_id(3)))
    );
}

#[test]
fn mixed_no_code_and_nominal_cleanup_is_scalar_only_and_keeps_action_ordinal() {
    let mixed_unit = mixed_edge_owned_cleanup_plan();
    assert_eq!(
        build_object_artifact(&mixed_unit),
        Err(ObjectError::InvalidUnitAffineCleanupEvidence(machine_id(3))),
        "the scalar-only mixed slice must not widen Unit artifact admission",
    );

    let mut scalar = mixed_unit;
    let caller = &mut scalar.functions[2];
    promote_x86_cleanup_to_scalar(caller);

    let artifact = build_object_artifact(&scalar).expect("mixed scalar cleanup object");
    let object_caller = artifact
        .functions()
        .iter()
        .find(|function| function.machine == machine_id(3))
        .expect("mixed scalar cleanup caller");
    assert_eq!(
        object_caller.internal_unit_calls[0].owner,
        CallSiteOwner::CleanupAction {
            edge: edge_id(3),
            action_ordinal: 1,
        },
        "the no-code action retains ordinal zero without renumbering the call",
    );
    let image = emit_executable_image(&artifact, 3).expect("mixed scalar cleanup image");
    let installation =
        build_installation_record(&image, ProfileDecisionId::new(1).expect("profile"))
            .expect("mixed scalar cleanup installation");
    validate_installation_record(&installation, &image)
        .expect("mixed scalar cleanup image binding");

    let caller = &mut scalar.functions[2];
    caller.internal_calls[0].owner = CallSiteOwner::CleanupAction {
        edge: edge_id(3),
        action_ordinal: 0,
    };
    caller.internal_unit_calls[0].owner = caller.internal_calls[0].owner;
    assert!(
        build_object_artifact(&scalar).is_err(),
        "a cleanup call cannot claim the no-code action's ordinal",
    );
}

#[test]
fn x86_unit_stack_scan_uses_instruction_boundaries_not_immediate_substrings() {
    let mut plan = two_function_plan();
    plan.functions[0].bytes = vec![
        0x48, 0xb8, // mov rax, imm64
        0x48, 0x83, 0xec, 0x08, 0x48, 0x83, 0xc4, 0x08, // immediate payload
        0xc3,
    ];
    plan.functions[0].unit_stack = Some(UnitStackEvidence {
        frame: None,
        aarch64_return_link: None,
        stack_alignment: 16,
    });
    add_empty_unit_cleanup(&mut plan.functions[0]);
    build_object_artifact(&plan).expect("stack-like immediate bytes are not stack instructions");
}

#[test]
fn object_replays_linear_scalar_stack_peaks_and_rejects_mutations() {
    let mut x86 = two_function_plan();
    x86.functions.truncate(1);
    x86.entry = machine_id(1);
    x86.functions[0].bytes = vec![
        0x50, // push rax
        0x52, // push rdx
        0x5a, // pop rdx
        0x58, // pop rax
        0xc3, // ret
    ];
    x86.functions[0].scalar_stack = Some(ScalarStackEvidence {
        mutations: vec![
            scalar_mutation(0, 1, ScalarStackMutationKind::X86Push),
            scalar_mutation(1, 1, ScalarStackMutationKind::X86Push),
            scalar_mutation(2, 1, ScalarStackMutationKind::X86Pop),
            scalar_mutation(3, 1, ScalarStackMutationKind::X86Pop),
        ],
        control_flow: ScalarControlFlowEvidence::Linear,
        stack_alignment: 16,
        cleanup_preservation: None,
    });
    let artifact = build_object_artifact(&x86).expect("x86 scalar stack artifact");
    assert_eq!(
        artifact.functions()[0]
            .scalar_stack
            .unwrap()
            .local_peak_bytes,
        16
    );
    assert_eq!(
        image_emission::derive_stack_demand(&artifact, machine_id(1))
            .expect("scalar stack demand")
            .ceiling_bytes(),
        16
    );

    let mut removed_push = x86.clone();
    removed_push.functions[0].bytes.remove(0);
    assert_eq!(
        build_object_artifact(&removed_push),
        Err(ObjectError::InvalidScalarStackEvidence {
            machine: machine_id(1),
            offset: 1,
        })
    );

    let mut injected_push = x86.clone();
    injected_push.functions[0].bytes.insert(4, 0x50);
    assert_eq!(
        build_object_artifact(&injected_push),
        Err(ObjectError::UnclaimedScalarStackMutation {
            machine: machine_id(1),
            offset: 4,
        })
    );

    let mut branch = x86.clone();
    branch.functions[0].bytes.splice(4..4, [0xeb, 0]);
    assert_eq!(
        build_object_artifact(&branch),
        Err(ObjectError::NonLinearScalarControlFlow {
            machine: machine_id(1),
            offset: 4,
        })
    );

    let mut unsupported_lea = x86.clone();
    unsupported_lea.functions[0].bytes = vec![
        0x48, 0x8d, 0x64, 0x24, 0x08, // lea rsp, [rsp + 8]
        0xc3,
    ];
    unsupported_lea.functions[0]
        .scalar_stack
        .as_mut()
        .expect("scalar evidence")
        .mutations
        .clear();
    assert_eq!(
        build_object_artifact(&unsupported_lea),
        Err(ObjectError::UnsupportedScalarStackMutation {
            machine: machine_id(1),
            offset: 0,
        })
    );

    let mut call = internal_call_plan(NativeTarget::linux_x64());
    call.functions[1].scalar_stack = Some(ScalarStackEvidence {
        mutations: Vec::new(),
        control_flow: ScalarControlFlowEvidence::Linear,
        stack_alignment: 16,
        cleanup_preservation: None,
    });
    assert_eq!(
        build_object_artifact(&call),
        Err(ObjectError::MissingScalarCallStackEvidence {
            caller: machine_id(2),
            owner: CallSiteOwner::Operation(operation_id(2)),
        })
    );

    let mut aarch64 = two_function_plan();
    aarch64.target = NativeTarget::linux_arm64();
    aarch64.functions.truncate(1);
    aarch64.entry = machine_id(1);
    aarch64.functions[0].bytes = aarch64_words(&[
        0xd100_43ff, // sub sp, sp, #16
        0xd100_43ff, // sub sp, sp, #16
        0x9100_43ff, // add sp, sp, #16
        0x9100_43ff, // add sp, sp, #16
        0xd65f_03c0, // ret
    ]);
    aarch64.functions[0].scalar_stack = Some(ScalarStackEvidence {
        mutations: vec![
            scalar_mutation(0, 4, ScalarStackMutationKind::Allocate { byte_size: 16 }),
            scalar_mutation(4, 4, ScalarStackMutationKind::Allocate { byte_size: 16 }),
            scalar_mutation(8, 4, ScalarStackMutationKind::Release { byte_size: 16 }),
            scalar_mutation(12, 4, ScalarStackMutationKind::Release { byte_size: 16 }),
        ],
        control_flow: ScalarControlFlowEvidence::Linear,
        stack_alignment: 16,
        cleanup_preservation: None,
    });
    let artifact = build_object_artifact(&aarch64).expect("AArch64 scalar stack artifact");
    assert_eq!(
        artifact.functions()[0]
            .scalar_stack
            .unwrap()
            .local_peak_bytes,
        32
    );

    let mut injected_aarch64 = aarch64;
    insert_aarch64_word(&mut injected_aarch64.functions[0].bytes, 16, 0xd100_43ff);
    assert_eq!(
        build_object_artifact(&injected_aarch64),
        Err(ObjectError::UnclaimedScalarStackMutation {
            machine: machine_id(1),
            offset: 16,
        })
    );

    let mut unsupported_aarch64 = two_function_plan();
    unsupported_aarch64.target = NativeTarget::linux_arm64();
    unsupported_aarch64.functions.truncate(1);
    unsupported_aarch64.entry = machine_id(1);
    unsupported_aarch64.functions[0].bytes = aarch64_words(&[
        0xa9bf_7bfd, // stp x29, x30, [sp, #-16]!
        0xd65f_03c0, // ret
    ]);
    unsupported_aarch64.functions[0].scalar_stack = Some(ScalarStackEvidence {
        mutations: Vec::new(),
        control_flow: ScalarControlFlowEvidence::Linear,
        stack_alignment: 16,
        cleanup_preservation: None,
    });
    assert_eq!(
        build_object_artifact(&unsupported_aarch64),
        Err(ObjectError::UnsupportedScalarStackMutation {
            machine: machine_id(1),
            offset: 0,
        })
    );
}

#[test]
fn object_replays_x86_division_diamond_stack_paths_and_rejects_forgery() {
    let mut plan = two_function_plan();
    plan.functions.truncate(1);
    plan.entry = machine_id(1);
    plan.functions[0].bytes = vec![
        0x48, 0x89, 0xf8, // mov rax, rdi
        0x50, // push rax
        0x48, 0x89, 0xf0, // mov rax, rsi
        0x41, 0x5a, // pop r10
        0x50, // push rax
        0x4c, 0x89, 0xd0, // mov rax, r10
        0x48, 0x83, 0x3c, 0x24, 0xff, // cmp qword [rsp], -1
        0x0f, 0x85, 0x0c, 0x00, 0x00, 0x00, // jne ordinary
        0x48, 0xf7, 0xd8, // neg rax
        0x48, 0x83, 0xc4, 0x08, // add rsp, 8
        0xe9, 0x0a, 0x00, 0x00, 0x00, // jmp merge
        0x48, 0x99, // cqo
        0x48, 0xf7, 0x3c, 0x24, // idiv qword [rsp]
        0x48, 0x83, 0xc4, 0x08, // add rsp, 8
        0xc3, // ret
    ];
    plan.functions[0].scalar_stack = Some(ScalarStackEvidence {
        mutations: vec![
            scalar_mutation(3, 1, ScalarStackMutationKind::X86Push),
            scalar_mutation(7, 2, ScalarStackMutationKind::X86Pop),
            scalar_mutation(9, 1, ScalarStackMutationKind::X86Push),
            scalar_mutation(27, 4, ScalarStackMutationKind::Release { byte_size: 8 }),
            scalar_mutation(42, 4, ScalarStackMutationKind::Release { byte_size: 8 }),
        ],
        control_flow: ScalarControlFlowEvidence::LinearWithDivisionBranches {
            branches: vec![ScalarDivisionBranchEvidence {
                branch_offset: 18,
                branch_byte_count: 6,
                ordinary_arm_offset: 36,
                join_offset: 31,
                join_byte_count: 5,
                merge_offset: 46,
            }],
        },
        stack_alignment: 16,
        cleanup_preservation: None,
    });
    let artifact = build_object_artifact(&plan).expect("division stack artifact");
    assert_eq!(
        artifact.functions()[0]
            .scalar_stack
            .expect("scalar stack")
            .local_peak_bytes,
        8
    );

    let mut forged_branch = plan.clone();
    let ScalarControlFlowEvidence::LinearWithDivisionBranches { branches } = &mut forged_branch
        .functions[0]
        .scalar_stack
        .as_mut()
        .expect("stack evidence")
        .control_flow
    else {
        unreachable!()
    };
    branches[0].ordinary_arm_offset = 35;
    assert_eq!(
        build_object_artifact(&forged_branch),
        Err(ObjectError::InvalidScalarConditionalEvidence {
            machine: machine_id(1),
            offset: 18,
        })
    );

    let mut forged_join = plan;
    forged_join.functions[0].bytes[32] = 0x09;
    assert_eq!(
        build_object_artifact(&forged_join),
        Err(ObjectError::InvalidScalarConditionalEvidence {
            machine: machine_id(1),
            offset: 18,
        })
    );
}

#[test]
fn scalar_stack_decoder_ignores_stack_opcode_bytes_inside_immediates() {
    let mut plan = two_function_plan();
    plan.functions.truncate(1);
    plan.entry = machine_id(1);
    plan.functions[0].bytes = vec![
        0x48, 0xb8, // mov rax, imm64
        0x50, 0x48, 0x83, 0xec, 8, 0x58, 0x90, 0x90, // immediate payload
        0xc3,
    ];
    plan.functions[0].scalar_stack = Some(ScalarStackEvidence {
        mutations: Vec::new(),
        control_flow: ScalarControlFlowEvidence::Linear,
        stack_alignment: 16,
        cleanup_preservation: None,
    });
    build_object_artifact(&plan).expect("stack opcodes inside an immediate are not instructions");
}

#[test]
fn scalar_direct_calls_compose_pending_temporaries_and_fail_closed() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let plan = scalar_call_plan(target);
        let artifact = build_object_artifact(&plan).expect("typed scalar call artifact");
        let caller = &artifact.functions()[1];
        assert_eq!(caller.scalar_call_stacks.len(), 1);
        assert_eq!(
            caller.scalar_call_stacks[0].caller_live_bytes,
            16 * if target.architecture == target::Architecture::X86_64 {
                1
            } else {
                2
            }
        );
        assert_eq!(
            derive_stack_demand(&artifact, machine_id(2))
                .expect("acyclic scalar call demand")
                .ceiling_bytes(),
            u64::from(caller.scalar_call_stacks[0].caller_live_bytes)
        );

        let mut missing = plan.clone();
        missing.functions[1].internal_calls[0].scalar_stack = None;
        assert_eq!(
            build_object_artifact(&missing),
            Err(ObjectError::MissingScalarCallStackEvidence {
                caller: machine_id(2),
                owner: CallSiteOwner::Operation(operation_id(2)),
            })
        );

        let mut untyped = plan.clone();
        untyped.functions[1].internal_calls.clear();
        let call_offset = if target.architecture == target::Architecture::X86_64 {
            1
        } else {
            12
        };
        assert_eq!(
            build_object_artifact(&untyped),
            Err(ObjectError::UntypedScalarInternalCall {
                machine: machine_id(2),
                offset: call_offset,
            })
        );

        let mut unaccounted = plan.clone();
        unaccounted.functions[0].scalar_stack = None;
        let artifact = build_object_artifact(&unaccounted)
            .expect("object construction does not invent callee evidence");
        assert_eq!(
            derive_stack_demand(&artifact, machine_id(2)),
            Err(ObjectError::UnaccountedTerminalStack(machine_id(1)))
        );

        let mut cycle = plan;
        cycle.functions[0] = cycle.functions[1].clone();
        cycle.functions[0].machine = machine_id(1);
        cycle.functions[0].provenance.operations = vec![operation_id(1)];
        cycle.functions[0].internal_calls[0].owner =
            target_operations::CallSiteOwner::Operation(operation_id(1));
        cycle.functions[0].internal_calls[0].target = machine_id(2);
        let artifact = build_object_artifact(&cycle).expect("typed scalar call cycle");
        assert_eq!(
            derive_stack_demand(&artifact, machine_id(2)),
            Err(ObjectError::TerminalStackCycle(machine_id(2)))
        );
    }

    let mut mutation = scalar_call_plan(NativeTarget::linux_x64());
    mutation.functions[1]
        .scalar_stack
        .as_mut()
        .expect("scalar evidence")
        .mutations
        .remove(0);
    assert_eq!(
        build_object_artifact(&mutation),
        Err(ObjectError::UnclaimedScalarStackMutation {
            machine: machine_id(2),
            offset: 0,
        })
    );

    let mut forged = scalar_call_plan(NativeTarget::linux_arm64());
    forged.functions[1].internal_calls[0]
        .scalar_stack
        .as_mut()
        .expect("scalar call evidence")
        .outbound
        .as_mut()
        .expect("AArch64 outbound evidence")
        .byte_size = 32;
    assert!(matches!(
        build_object_artifact(&forged),
        Err(ObjectError::InvalidScalarCallStackEvidence { .. })
    ));

    let mut link_mutation = scalar_call_plan(NativeTarget::linux_arm64());
    link_mutation.functions[1].bytes[8..12].copy_from_slice(&0xd503_201f_u32.to_le_bytes());
    assert!(matches!(
        build_object_artifact(&link_mutation),
        Err(ObjectError::InvalidScalarCallStackEvidence { .. })
    ));
}

#[test]
fn scalar_two_return_conditional_replays_each_arm_and_rejects_forgery() {
    let x86 = scalar_two_return_conditional_plan(NativeTarget::linux_x64());
    let artifact = build_object_artifact(&x86).expect("x86 conditional stack artifact");
    assert_eq!(
        artifact.functions()[0]
            .scalar_stack
            .expect("x86 scalar stack")
            .local_peak_bytes,
        0
    );

    let mut x86_division = scalar_two_return_conditional_plan(NativeTarget::linux_x64());
    x86_division.functions[0].bytes = vec![
        0x89, 0xf8, // mov eax, edi
        0x85, 0xc0, // test eax, eax
        0x0f, 0x84, 15, 0, 0, 0, // jz false arm
        0xb8, 8, 0, 0, 0, // true: mov eax, 8
        0x31, 0xd2, // xor edx, edx
        0xb9, 2, 0, 0, 0, // mov ecx, 2
        0xf7, 0xf1, // div ecx
        0xc3, // ret
        0xb8, 3, 0, 0, 0,    // false: mov eax, 3
        0xc3, // ret
    ];
    x86_division.functions[0]
        .scalar_stack
        .as_mut()
        .expect("scalar evidence")
        .control_flow = conditional_tree(ScalarConditionalCondition::Parameter, 4, 6, 25);
    let artifact = build_object_artifact(&x86_division)
        .expect("branch-free x86 division replays inside one conditional arm");
    assert_eq!(
        artifact.functions()[0]
            .scalar_stack
            .expect("division scalar stack")
            .local_peak_bytes,
        0
    );
    let mut forged_inner_branch = x86_division;
    forged_inner_branch.functions[0].bytes[22] = 0x75; // jne rel8, not div
    assert_eq!(
        build_object_artifact(&forged_inner_branch),
        Err(ObjectError::NonLinearScalarControlFlow {
            machine: machine_id(1),
            offset: 22,
        })
    );

    let aarch64 = scalar_two_return_conditional_plan(NativeTarget::linux_arm64());
    let artifact = build_object_artifact(&aarch64).expect("AArch64 conditional stack artifact");
    assert_eq!(
        artifact.functions()[0]
            .scalar_stack
            .expect("AArch64 scalar stack")
            .local_peak_bytes,
        32,
        "sequential arms take a maximum, not a sum"
    );
    assert_eq!(
        derive_stack_demand(&artifact, machine_id(1))
            .expect("conditional stack demand")
            .ceiling_bytes(),
        32
    );

    let mut forged_target = x86.clone();
    forged_target.functions[0]
        .scalar_stack
        .as_mut()
        .expect("scalar evidence")
        .control_flow = conditional_tree(ScalarConditionalCondition::Parameter, 4, 6, 20);
    assert_eq!(
        build_object_artifact(&forged_target),
        Err(ObjectError::InvalidScalarConditionalEvidence {
            machine: machine_id(1),
            offset: 4,
        })
    );

    let mut wrong_polarity = x86.clone();
    wrong_polarity.functions[0].bytes[5] = 0x85; // jne instead of canonical je
    assert_eq!(
        build_object_artifact(&wrong_polarity),
        Err(ObjectError::InvalidScalarConditionalEvidence {
            machine: machine_id(1),
            offset: 4,
        })
    );

    let mut missing_evidence = x86;
    missing_evidence.functions[0]
        .scalar_stack
        .as_mut()
        .expect("scalar evidence")
        .control_flow = ScalarControlFlowEvidence::Linear;
    assert_eq!(
        build_object_artifact(&missing_evidence),
        Err(ObjectError::NonLinearScalarControlFlow {
            machine: machine_id(1),
            offset: 4,
        })
    );

    let mut call_claim = scalar_two_return_conditional_plan(NativeTarget::linux_x64());
    call_claim.functions[0]
        .bytes
        .splice(0..0, [0xe8, 0, 0, 0, 0]);
    call_claim.functions[0]
        .scalar_stack
        .as_mut()
        .expect("scalar evidence")
        .control_flow = conditional_tree(ScalarConditionalCondition::Parameter, 9, 6, 24);
    call_claim.functions[0]
        .internal_calls
        .push(InternalCallRelocation {
            owner: target_operations::CallSiteOwner::Operation(operation_id(1)),
            target: machine_id(1),
            unit_stack: None,
            scalar_stack: Some(ScalarCallStackEvidence {
                outbound: None,
                aarch64_return_link: None,
            }),
            offset: 1,
        });
    assert_eq!(
        build_object_artifact(&call_claim),
        Err(ObjectError::ScalarConditionalCallOutsideArm {
            machine: machine_id(1),
            operation: operation_id(1),
            offset: 0,
        })
    );

    let mut extra_branch = aarch64.clone();
    extra_branch.functions[0].bytes[4..8].copy_from_slice(&0x1400_0000_u32.to_le_bytes());
    assert_eq!(
        build_object_artifact(&extra_branch),
        Err(ObjectError::NonLinearScalarControlFlow {
            machine: machine_id(1),
            offset: 4,
        })
    );

    let mut crash_arm = aarch64.clone();
    crash_arm.functions[0].bytes[4..8].copy_from_slice(&0xd420_0000_u32.to_le_bytes()); // brk #0
    crash_arm.functions[0]
        .scalar_stack
        .as_mut()
        .expect("scalar evidence")
        .mutations
        .remove(0);
    assert_eq!(
        build_object_artifact(&crash_arm),
        Err(ObjectError::NonLinearScalarControlFlow {
            machine: machine_id(1),
            offset: 4,
        })
    );

    let mut missing_return = aarch64.clone();
    missing_return.functions[0].bytes[12..16].copy_from_slice(&0xd503_201f_u32.to_le_bytes());
    assert_eq!(
        build_object_artifact(&missing_return),
        Err(ObjectError::MissingBalancedScalarReturn(machine_id(1)))
    );

    let mut unclaimed = aarch64.clone();
    unclaimed.functions[0]
        .scalar_stack
        .as_mut()
        .expect("scalar evidence")
        .mutations
        .remove(0);
    assert_eq!(
        build_object_artifact(&unclaimed),
        Err(ObjectError::UnclaimedScalarStackMutation {
            machine: machine_id(1),
            offset: 4,
        })
    );

    let mut crossed = scalar_two_return_conditional_plan(NativeTarget::linux_arm64());
    crossed.functions[0].bytes = aarch64_words(&[
        0x3400_0060, // cbz w0, false arm at byte 12
        0xd100_43ff, // true: sub sp, sp, #16
        0xd65f_03c0, // true: ret while still allocated
        0x9100_43ff, // false: add sp, sp, #16
        0xd65f_03c0, // false: ret
    ]);
    crossed.functions[0].scalar_stack = Some(ScalarStackEvidence {
        mutations: vec![
            scalar_mutation(4, 4, ScalarStackMutationKind::Allocate { byte_size: 16 }),
            scalar_mutation(12, 4, ScalarStackMutationKind::Release { byte_size: 16 }),
        ],
        control_flow: conditional_tree(ScalarConditionalCondition::Parameter, 0, 4, 12),
        stack_alignment: 16,
        cleanup_preservation: None,
    });
    assert_eq!(
        build_object_artifact(&crossed),
        Err(ObjectError::MissingBalancedScalarReturn(machine_id(1))),
        "allocations may not balance against a different arm"
    );
}

#[test]
fn scalar_three_leaf_cleanup_object_custody_rejects_corruption() {
    let plan = scalar_three_leaf_cleanup_plan();
    let artifact = build_object_artifact(&plan).expect("three-leaf cleanup object");
    let function = &artifact.functions()[0];
    assert_eq!(function.scalar_control_affine_cleanups.len(), 3);
    assert_eq!(
        function
            .scalar_control_affine_cleanups
            .iter()
            .map(|record| record.cleanup.psi_edge)
            .collect::<Vec<_>>(),
        [edge_id(10), edge_id(11), edge_id(12)]
    );

    let invalid = Err(ObjectError::InvalidUnitAffineCleanupEvidence(machine_id(1)));
    let mut reordered = plan.clone();
    reordered.functions[0]
        .scalar_control_affine_cleanups
        .swap(0, 1);
    assert_eq!(build_object_artifact(&reordered), invalid);

    let mut duplicate_edge = plan.clone();
    duplicate_edge.functions[0].scalar_control_affine_cleanups[1]
        .cleanup
        .psi_edge = edge_id(10);
    assert_eq!(build_object_artifact(&duplicate_edge), invalid);

    let mut crossed_interval = plan.clone();
    crossed_interval.functions[0].scalar_control_affine_cleanups[0]
        .cleanup
        .byte_count += 1;
    assert_eq!(build_object_artifact(&crossed_interval), invalid);

    let mut forged_preservation = plan.clone();
    forged_preservation.functions[0].scalar_control_affine_cleanups[2]
        .preservation
        .result_store_offset += 1;
    assert_eq!(build_object_artifact(&forged_preservation), invalid);

    let mut forged_control = plan;
    let ScalarControlFlowEvidence::ConditionalTree { decisions, .. } = &mut forged_control
        .functions[0]
        .scalar_stack
        .as_mut()
        .expect("scalar stack")
        .control_flow
    else {
        unreachable!()
    };
    decisions[1].branch_offset = decisions[0].false_arm_offset;
    assert!(matches!(
        build_object_artifact(&forged_control),
        Err(ObjectError::InvalidScalarConditionalEvidence { .. })
            | Err(ObjectError::InvalidUnitAffineCleanupEvidence(_))
    ));
}

#[test]
fn scalar_expression_conditionals_replay_balanced_prefix_and_validate_branch_kind() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let plan = scalar_expression_two_return_conditional_plan(target);
        let artifact =
            build_object_artifact(&plan).expect("balanced expression-condition scalar artifact");
        assert_eq!(
            derive_stack_demand(&artifact, machine_id(1))
                .expect("expression-condition stack demand")
                .ceiling_bytes(),
            16
        );

        let mut wrong_branch = plan.clone();
        match target.architecture {
            target::Architecture::X86_64 => {
                wrong_branch.functions[0].bytes[12] = 0x85; // jne, not canonical je
            }
            target::Architecture::Aarch64 => {
                wrong_branch.functions[0].bytes[12..16]
                    .copy_from_slice(&0x5400_0041_u32.to_le_bytes()); // b.ne, not b.eq
            }
        }
        assert!(matches!(
            build_object_artifact(&wrong_branch),
            Err(ObjectError::InvalidScalarConditionalEvidence { .. })
        ));

        let mut forged_kind = plan.clone();
        let ScalarControlFlowEvidence::ConditionalTree { decisions, .. } = &mut forged_kind
            .functions[0]
            .scalar_stack
            .as_mut()
            .expect("scalar evidence")
            .control_flow
        else {
            unreachable!()
        };
        decisions[0].condition = ScalarConditionalCondition::Parameter;
        assert!(matches!(
            build_object_artifact(&forged_kind),
            Err(ObjectError::InvalidScalarConditionalEvidence { .. })
        ));

        let call_plan = scalar_expression_condition_call_plan(target);
        let call_artifact =
            build_object_artifact(&call_plan).expect("typed condition-prefix call should validate");
        assert_eq!(
            derive_stack_demand(&call_artifact, call_plan.entry)
                .expect("condition-prefix call demand")
                .ceiling_bytes(),
            16
        );
        let mut untyped_call = call_plan;
        untyped_call.functions[1].internal_calls.clear();
        assert!(matches!(
            build_object_artifact(&untyped_call),
            Err(ObjectError::UntypedScalarInternalCall { .. })
        ));
    }

    let mut clobbered_flags =
        scalar_expression_two_return_conditional_plan(NativeTarget::linux_x64());
    clobbered_flags.functions[0].bytes[6..11].copy_from_slice(&[0x48, 0x83, 0xc4, 16, 0x90]); // add rsp, 16; nop
    assert_eq!(
        build_object_artifact(&clobbered_flags),
        Err(ObjectError::InvalidScalarStackEvidence {
            machine: machine_id(1),
            offset: 6,
        })
    );

    let mut unbalanced = scalar_expression_two_return_conditional_plan(NativeTarget::linux_arm64());
    unbalanced.functions[0].bytes[8..12].copy_from_slice(&0xd503_201f_u32.to_le_bytes());
    assert!(matches!(
        build_object_artifact(&unbalanced),
        Err(ObjectError::InvalidScalarStackEvidence { .. })
            | Err(ObjectError::MissingBalancedScalarReturn(_))
    ));
}

#[test]
fn scalar_conditional_calls_replay_per_arm_and_compose_by_maximum() {
    for (target, expected_peak) in [
        (NativeTarget::linux_x64(), 16),
        (NativeTarget::linux_arm64(), 32),
    ] {
        let plan = scalar_conditional_call_plan(target);
        let artifact = build_object_artifact(&plan).expect("typed conditional-call artifact");
        let caller = &artifact.functions()[1];
        assert_eq!(caller.scalar_call_stacks.len(), 2);
        assert_eq!(
            caller
                .scalar_stack
                .expect("conditional scalar stack")
                .local_peak_bytes,
            expected_peak,
            "arm peaks take a maximum rather than summing sequential arms"
        );
        assert_eq!(
            derive_stack_demand(&artifact, machine_id(2))
                .expect("conditional call closure")
                .ceiling_bytes(),
            u64::from(expected_peak)
        );

        let mut source_distributed = plan.clone();
        source_distributed.functions[1].internal_calls[1].owner =
            CallSiteOwner::Operation(operation_id(2));
        build_object_artifact(&source_distributed)
            .expect("one semantic call may be source-distributed across mutually exclusive leaves");

        let mut missing = plan.clone();
        missing.functions[1].internal_calls[0].scalar_stack = None;
        assert_eq!(
            build_object_artifact(&missing),
            Err(ObjectError::MissingScalarCallStackEvidence {
                caller: machine_id(2),
                owner: CallSiteOwner::Operation(operation_id(2)),
            })
        );

        let mut untyped = plan.clone();
        let call_start = match target.architecture {
            target::Architecture::X86_64 => 11,
            target::Architecture::Aarch64 => 16,
        };
        untyped.functions[1].internal_calls.remove(0);
        assert_eq!(
            build_object_artifact(&untyped),
            Err(ObjectError::UntypedScalarInternalCall {
                machine: machine_id(2),
                offset: call_start,
            })
        );

        let mut unaccounted = plan;
        unaccounted.functions[0].scalar_stack = None;
        let artifact = build_object_artifact(&unaccounted)
            .expect("object retains no invented callee stack evidence");
        assert_eq!(
            derive_stack_demand(&artifact, machine_id(2)),
            Err(ObjectError::UnaccountedTerminalStack(machine_id(1)))
        );
    }

    let mut forged_link = scalar_conditional_call_plan(NativeTarget::linux_arm64());
    forged_link.functions[1].internal_calls[0]
        .scalar_stack
        .as_mut()
        .expect("scalar call evidence")
        .aarch64_return_link
        .as_mut()
        .expect("AArch64 link evidence")
        .store_offset = 8;
    assert!(matches!(
        build_object_artifact(&forged_link),
        Err(ObjectError::InvalidScalarCallStackEvidence { .. })
    ));

    let mut opposite_arm = scalar_conditional_call_plan(NativeTarget::linux_x64());
    opposite_arm.functions[1].internal_calls[0].scalar_stack =
        opposite_arm.functions[1].internal_calls[1].scalar_stack;
    assert!(matches!(
        build_object_artifact(&opposite_arm),
        Err(ObjectError::InvalidScalarCallStackEvidence { .. })
    ));

    let mut cycle = scalar_conditional_call_plan(NativeTarget::linux_x64());
    cycle.functions[0] = cycle.functions[1].clone();
    cycle.functions[0].machine = machine_id(1);
    for call in &mut cycle.functions[0].internal_calls {
        call.target = machine_id(2);
    }
    let artifact = build_object_artifact(&cycle).expect("conditional call cycle object");
    assert_eq!(
        derive_stack_demand(&artifact, machine_id(2)),
        Err(ObjectError::TerminalStackCycle(machine_id(2)))
    );
}

#[test]
fn terminal_unit_stack_demand_composes_the_exact_call_closure() {
    let mut plan = internal_call_plan(NativeTarget::linux_x64());
    plan.functions[0].unit_stack = Some(UnitStackEvidence {
        frame: None,
        aarch64_return_link: None,
        stack_alignment: 16,
    });
    add_empty_unit_cleanup(&mut plan.functions[0]);
    account_x86_unit_call(&mut plan);
    let artifact = build_object_artifact(&plan).expect("accounted Unit artifact");
    let demand = derive_unit_stack_demand(&artifact, machine_id(2))
        .expect("acyclic Unit closure stack demand");
    assert_eq!(demand.psi(), plan.psi);
    assert_eq!(demand.target(), plan.target);
    assert_eq!(demand.entry(), machine_id(2));
    assert_eq!(demand.ceiling_bytes(), 16);
    assert_eq!(demand.stack_alignment(), 16);
    assert_eq!(
        demand
            .contributing_machines()
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        [machine_id(1), machine_id(2)]
    );

    let mut unaccounted = internal_call_plan(NativeTarget::linux_x64());
    account_x86_unit_call(&mut unaccounted);
    let artifact = build_object_artifact(&unaccounted).expect("partly accounted artifact");
    assert_eq!(
        derive_unit_stack_demand(&artifact, machine_id(2)),
        Err(ObjectError::UnaccountedTerminalStack(machine_id(1)))
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
            target::Architecture::X86_64 => integer_return(7),
            target::Architecture::Aarch64 => {
                vec![0xe0, 0x00, 0x80, 0x52, 0xc0, 0x03, 0x5f, 0xd6]
            }
        };
        let machine = machine_id(1);
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
                    operations: vec![operation_id(1)],
                    edges: vec![edge_id(1)],
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
                semantic_code_attribution: Vec::new(),
                port_effects: Vec::new(),
                boundary_settlements: Vec::new(),
                scalar_affine_cleanup: None,
                scalar_control_affine_cleanups: Vec::new(),
                scalar_structural_parameters: Vec::new(),
                scalar_structural_parameter_homes: Vec::new(),
                structural_return: None,
            }],
        };
        let artifact = build_object_artifact(&plan).expect("artifact");
        let image = emit_executable_image(&artifact, 3)
            .unwrap_or_else(|error| panic!("{target:?} image failed: {error}"));
        assert_eq!(image.psi(), plan.psi);
        let installation = build_installation_record(&image, ProfileDecisionId::new(1).unwrap())
            .expect("installation record");
        assert_eq!(
            installation.subsystem(),
            matches!(target.object_format, target::ObjectFormat::Coff).then_some(3)
        );
        let installation_bytes =
            encode_installation_record(&installation).expect("installation bytes");
        assert_eq!(
            decode_installation_record(&installation_bytes),
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
            evidence.encoded_text_report_fingerprint,
            evidence.final_compiler_text_report_fingerprint
        );
        assert!(evidence.has_valid_derivation_digest());
        assert_ne!(
            evidence.encoded_text_digest.as_bytes(),
            evidence.final_compiler_text_digest.as_bytes(),
            "distinct digest domains remain separate even for identical bytes"
        );
        assert_eq!(evidence.text_relocation_count, 0);
        assert_eq!(evidence.checked_instruction_validation_count, 0);
    }
}

#[test]
fn installation_record_is_canonical_and_binds_exact_image_and_target_facts() {
    let plan = two_function_plan();
    let artifact = build_object_artifact(&plan).expect("artifact");
    let image = emit_executable_image(&artifact, 3).expect("Linux image");
    let record = build_installation_record(
        &image,
        ProfileDecisionId::new(11).expect("profile decision"),
    )
    .expect("installation record");

    assert_eq!(record.psi(), plan.psi);
    assert_eq!(record.target(), plan.target);
    assert_eq!(record.subsystem(), None);
    assert!(record.selected_provider_plans().is_empty());
    let bytes = encode_installation_record(&record).expect("canonical bytes");
    assert_eq!(&bytes[..8], b"PSIINST\0");
    assert_eq!(
        u16::from_le_bytes(bytes[8..10].try_into().unwrap()),
        INSTALLATION_FORMAT_MARKER
    );
    assert_eq!(decode_installation_record(&bytes), Ok(record.clone()));
    validate_installation_record(&record, &image).expect("exact image binding");
    // This fixture contains neither an owned stack-pointer home nor an X8 result.
    // Its format-96 payload differs from format 95 only in the marker.
    // Reconstruct framing independently of the production helper and pin both.
    use sha2::{Digest, Sha256};
    let independent_fingerprint = |payload: &[u8]| {
        let mut digest = Sha256::new();
        digest.update(b"omega-installation-record\0");
        digest.update(u64::try_from(payload.len()).unwrap().to_le_bytes());
        digest.update(payload);
        format!("{:x}", digest.finalize())
    };
    let mut predecessor_payload = bytes.clone();
    predecessor_payload[8..10].copy_from_slice(&95_u16.to_le_bytes());
    assert_eq!(
        independent_fingerprint(&predecessor_payload),
        "d09ebbb7d6ea88e268c8336e9c41d7c82ab2068edbdfb8cef5a2418e74b8a6b7"
    );
    assert_eq!(
        decode_installation_record(&predecessor_payload),
        Err(InstallationError::UnsupportedFormatMarker(95))
    );
    assert_eq!(
        independent_fingerprint(&bytes),
        "d289ec57299e77c36a06adb8a2f78117fd49ae51aca38163d6c340e31713a5ec"
    );
    assert_eq!(
        installation_fingerprint(&record)
            .expect("installation fingerprint")
            .to_string(),
        "d289ec57299e77c36a06adb8a2f78117fd49ae51aca38163d6c340e31713a5ec"
    );
    // Format 82 adds an explicit continuation count to every function row,
    // including these empty rosters. Changing only the header is not a
    // conversion back to the previous wire format.
    assert!(
        record
            .functions()
            .iter()
            .all(|function| function.unit_continuations.is_empty())
    );
    let mut previous_bytes = bytes.clone();
    previous_bytes[8..10].copy_from_slice(&81_u16.to_le_bytes());
    assert_eq!(
        decode_installation_record(&previous_bytes),
        Err(InstallationError::UnsupportedFormatMarker(81))
    );

    let mut changed_plan = plan;
    changed_plan.functions[1].bytes = integer_return(8);
    let changed_artifact = build_object_artifact(&changed_plan).expect("changed artifact");
    let changed_image = emit_executable_image(&changed_artifact, 3).expect("changed Linux image");
    assert_eq!(
        validate_installation_record(&record, &changed_image),
        Err(InstallationError::ImageBindingMismatch)
    );
    assert!(matches!(
        derive_installation_stack_demand(&record, &changed_image, machine_id(2)),
        Err(image_emission::InstallationStackError::Installation(
            InstallationError::ImageBindingMismatch
        ))
    ));
}

/// Shared one-field-substitution driver for installation-header coverage: a
/// representable substitution still encodes and round-trips, the recomputed
/// installation fingerprint differs from the authentic record's published
/// identity, and replay against the unchanged image rejects the substitution.
fn assert_header_substitution_rejected(
    record: &image_emission::InstallationRecord,
    image: &image_emission::ExecutableImage,
    authentic: image_emission::InstallationFingerprint,
    field: &str,
    mutate: impl Fn(&mut image_emission::InstallationRecord),
) {
    let mut changed = record.clone();
    mutate(&mut changed);
    assert_ne!(changed, *record, "{field}: substitution changes the record");
    let bytes = encode_installation_record(&changed)
        .unwrap_or_else(|error| panic!("{field}: substituted record encodes: {error:?}"));
    let replayed = decode_installation_record(&bytes)
        .unwrap_or_else(|error| panic!("{field}: substituted record decodes: {error:?}"));
    assert_eq!(
        replayed, changed,
        "{field}: codec preserves the substituted record"
    );
    assert_ne!(
        installation_fingerprint(&replayed)
            .unwrap_or_else(|error| panic!("{field}: substituted fingerprint: {error:?}")),
        authentic,
        "{field}: recomputed identity differs from the authentic record"
    );
    assert_eq!(
        validate_installation_record(&replayed, image),
        Err(InstallationError::ImageBindingMismatch),
        "{field}: independent replay rejects the substitution"
    );
}

/// Every representable installation-header axis — program identity, target,
/// subsystem, profile decision, the committed component-progress projection,
/// the bound image fingerprint and section layout, and the compiler
/// text-validation receipt — is authenticated custody: a one-field
/// substitution either cannot encode canonically or still encodes, recomputes
/// a distinct installation fingerprint, and independent replay rejects it.
/// Axes bound to the emitted image reject through `validate_installation_record`.
/// The admission-owned axes — the caller-supplied profile decision and the
/// component-progress identities — are not image facts, so the image join
/// cannot see them; their custody is the published record identity that a
/// deployment journal replays against its pinned fingerprint.
#[test]
fn installation_header_rejects_every_one_field_substitution() {
    let plan = two_function_plan();
    let artifact = build_object_artifact(&plan).expect("artifact");
    let image = emit_executable_image(&artifact, 3).expect("Linux image");
    let record = build_installation_record_with_evidence(
        &image,
        ProfileDecisionId::new(11).expect("profile decision"),
        std::iter::empty::<&dyn ProviderExecutionEvidence>(),
        Some(&TestComponentProgressAcceptance {
            manifest: 0x1122,
            acceptance: 0x3344,
        }),
    )
    .expect("installation record");
    validate_installation_record(&record, &image).expect("exact image binding");
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");
    let authentic_evidence = record.compiler_text_validation();
    let authentic_progress = record
        .component_progress()
        .expect("committed component progress");
    assert_eq!(authentic_progress.manifest_identity(), 0x1122);
    assert_eq!(authentic_progress.acceptance_identity(), 0x3344);
    assert_eq!(record.subsystem(), None);

    // A relocation-bearing sibling image supplies well-typed foreign values
    // for the bound image fingerprint and the receipt digest axes.
    let mut other_plan = internal_call_plan(NativeTarget::linux_x64());
    account_x86_unit_call(&mut other_plan);
    let other_artifact = build_object_artifact(&other_plan).expect("other artifact");
    let other_image = emit_executable_image(&other_artifact, 3).expect("other image");
    let other =
        build_installation_record(&other_image, ProfileDecisionId::new(7).expect("profile"))
            .expect("other record");
    let other_fingerprint = other.image();
    let other_evidence = other.compiler_text_validation();
    assert_ne!(other_fingerprint, record.image());
    assert_ne!(
        other_evidence.encoded_text_digest, authentic_evidence.encoded_text_digest,
        "sibling text differs"
    );
    assert_ne!(
        other_evidence.final_compiler_text_digest, authentic_evidence.final_compiler_text_digest,
        "sibling final text differs"
    );
    assert_ne!(
        other_evidence.relocation_envelope_digest, authentic_evidence.relocation_envelope_digest,
        "a retained relocation changes the envelope commitment"
    );
    assert_ne!(
        other_evidence.derivation_digest, authentic_evidence.derivation_digest,
        "sibling derivation identity differs"
    );

    // Component-progress projections differing in exactly one committed
    // identity.
    let manifest_only = build_installation_record_with_evidence(
        &image,
        ProfileDecisionId::new(11).expect("profile decision"),
        std::iter::empty::<&dyn ProviderExecutionEvidence>(),
        Some(&TestComponentProgressAcceptance {
            manifest: 0x5566,
            acceptance: 0x3344,
        }),
    )
    .expect("manifest-substituted record")
    .component_progress();
    let acceptance_only = build_installation_record_with_evidence(
        &image,
        ProfileDecisionId::new(11).expect("profile decision"),
        std::iter::empty::<&dyn ProviderExecutionEvidence>(),
        Some(&TestComponentProgressAcceptance {
            manifest: 0x1122,
            acceptance: 0x7788,
        }),
    )
    .expect("acceptance-substituted record")
    .component_progress();
    assert_ne!(manifest_only, record.component_progress());
    assert_ne!(acceptance_only, record.component_progress());

    // Axes joined to the emitted image: each substitution still encodes and
    // round-trips, recomputes a distinct installation fingerprint, and replay
    // against the unchanged image rejects it.
    let image_bound: Vec<(&str, Box<dyn Fn(&mut image_emission::InstallationRecord)>)> = vec![
        (
            "psi.program_fingerprint",
            Box::new(|record| {
                record.psi_mut_for_test().program_fingerprint =
                    SemanticFingerprint::from_bytes([0xa5; 32]);
            }),
        ),
        (
            "target.architecture",
            Box::new(|record| {
                record.target_mut_for_test().architecture = target::Architecture::Aarch64;
            }),
        ),
        (
            "target",
            Box::new(|record| {
                *record.target_mut_for_test() = NativeTarget::macos_arm64();
            }),
        ),
        (
            "image",
            Box::new(move |record| {
                *record.image_mut_for_test() = other_fingerprint;
            }),
        ),
        (
            "image_sections.layout.text_address",
            Box::new(|record| {
                record.image_sections_mut_for_test().layout.text_address += 0x1000;
            }),
        ),
        (
            "image_sections.layout.data_address",
            Box::new(|record| {
                record.image_sections_mut_for_test().layout.data_address += 0x1000;
            }),
        ),
        (
            "image_sections.layout.bss_address",
            Box::new(|record| {
                record.image_sections_mut_for_test().layout.bss_address += 0x1000;
            }),
        ),
        (
            "compiler_text_validation",
            Box::new(move |record| {
                *record.compiler_text_validation_mut_for_test() = other_evidence;
            }),
        ),
        // `derivation_report_fingerprint` is report compatibility only: it is
        // outside the derivation-digest join, so the substitution is
        // representable without recomputing the receipt's own identity.
        (
            "compiler_text_validation.derivation_report_fingerprint",
            Box::new(|record| {
                record
                    .compiler_text_validation_mut_for_test()
                    .derivation_report_fingerprint += 1;
            }),
        ),
    ];
    for (field, mutate) in image_bound {
        assert_header_substitution_rejected(&record, &image, authentic_fingerprint, field, mutate);
    }

    // Every receipt input joined by the derivation digest remains
    // representable once the containing identity is honestly recomputed:
    // encoding accepts the consistent receipt and replay still rejects it
    // against the unchanged image.
    let recomputed_receipt: Vec<(&str, Box<dyn Fn(&mut image_emission::InstallationRecord)>)> = vec![
        (
            "compiler_text_validation.encoded_text_digest",
            Box::new(move |record| {
                let evidence = record.compiler_text_validation_mut_for_test();
                evidence.encoded_text_digest = other_evidence.encoded_text_digest;
                evidence.derivation_digest = evidence.recomputed_derivation_digest();
            }),
        ),
        (
            "compiler_text_validation.final_compiler_text_digest",
            Box::new(move |record| {
                let evidence = record.compiler_text_validation_mut_for_test();
                evidence.final_compiler_text_digest = other_evidence.final_compiler_text_digest;
                evidence.derivation_digest = evidence.recomputed_derivation_digest();
            }),
        ),
        (
            "compiler_text_validation.relocation_envelope_digest",
            Box::new(move |record| {
                let evidence = record.compiler_text_validation_mut_for_test();
                evidence.relocation_envelope_digest = other_evidence.relocation_envelope_digest;
                evidence.derivation_digest = evidence.recomputed_derivation_digest();
            }),
        ),
        (
            "compiler_text_validation.encoded_text_report_fingerprint",
            Box::new(|record| {
                let evidence = record.compiler_text_validation_mut_for_test();
                evidence.encoded_text_report_fingerprint += 1;
                evidence.derivation_digest = evidence.recomputed_derivation_digest();
            }),
        ),
        (
            "compiler_text_validation.final_compiler_text_report_fingerprint",
            Box::new(|record| {
                let evidence = record.compiler_text_validation_mut_for_test();
                evidence.final_compiler_text_report_fingerprint += 1;
                evidence.derivation_digest = evidence.recomputed_derivation_digest();
            }),
        ),
        (
            "compiler_text_validation.relocation_envelope_report_fingerprint",
            Box::new(|record| {
                let evidence = record.compiler_text_validation_mut_for_test();
                evidence.relocation_envelope_report_fingerprint += 1;
                evidence.derivation_digest = evidence.recomputed_derivation_digest();
            }),
        ),
        (
            "compiler_text_validation.checked_instruction_validation_report_fingerprint",
            Box::new(|record| {
                let evidence = record.compiler_text_validation_mut_for_test();
                evidence.checked_instruction_validation_report_fingerprint += 1;
                evidence.derivation_digest = evidence.recomputed_derivation_digest();
            }),
        ),
        (
            "compiler_text_validation.checked_instruction_footprint_report_fingerprint",
            Box::new(|record| {
                let evidence = record.compiler_text_validation_mut_for_test();
                evidence.checked_instruction_footprint_report_fingerprint += 1;
                evidence.derivation_digest = evidence.recomputed_derivation_digest();
            }),
        ),
        (
            "compiler_text_validation.text_relocation_count",
            Box::new(|record| {
                let evidence = record.compiler_text_validation_mut_for_test();
                evidence.text_relocation_count += 1;
                evidence.derivation_digest = evidence.recomputed_derivation_digest();
            }),
        ),
        (
            "compiler_text_validation.checked_instruction_validation_count",
            Box::new(|record| {
                let evidence = record.compiler_text_validation_mut_for_test();
                evidence.checked_instruction_validation_count += 1;
                evidence.derivation_digest = evidence.recomputed_derivation_digest();
            }),
        ),
    ];
    for (field, mutate) in recomputed_receipt {
        assert_header_substitution_rejected(&record, &image, authentic_fingerprint, field, mutate);
    }

    // Admission-owned axes are not image facts: the image join cannot see
    // them, so the substitution only breaks the published record identity —
    // the recomputed-fingerprint mismatch a deployment journal applies.
    let admission_bound: Vec<(&str, Box<dyn Fn(&mut image_emission::InstallationRecord)>)> = vec![
        (
            "profile_decision",
            Box::new(|record| {
                *record.profile_decision_mut_for_test() =
                    ProfileDecisionId::new(12).expect("profile decision");
            }),
        ),
        (
            "component_progress.manifest",
            Box::new(move |record| {
                *record.component_progress_mut_for_test() = manifest_only;
            }),
        ),
        (
            "component_progress.acceptance",
            Box::new(move |record| {
                *record.component_progress_mut_for_test() = acceptance_only;
            }),
        ),
        (
            "component_progress",
            Box::new(|record| {
                *record.component_progress_mut_for_test() = None;
            }),
        ),
    ];
    for (field, mutate) in admission_bound {
        let mut changed = record.clone();
        mutate(&mut changed);
        assert_ne!(changed, record, "{field}: substitution changes the record");
        let bytes = encode_installation_record(&changed)
            .unwrap_or_else(|error| panic!("{field}: substituted record encodes: {error:?}"));
        let replayed = decode_installation_record(&bytes)
            .unwrap_or_else(|error| panic!("{field}: substituted record decodes: {error:?}"));
        assert_eq!(
            replayed, changed,
            "{field}: codec preserves the substituted record"
        );
        assert_ne!(
            installation_fingerprint(&replayed)
                .unwrap_or_else(|error| panic!("{field}: substituted fingerprint: {error:?}")),
            authentic_fingerprint,
            "{field}: recomputed identity differs from the authentic record"
        );
        assert_eq!(
            validate_installation_record(&replayed, &image),
            Ok(()),
            "{field}: admission-owned axes sit outside the image join"
        );
    }

    // The remaining header axes are not independently representable:
    // canonical encoding rejects them before any identity or replay check.
    let encode_rejected: Vec<(
        &str,
        Box<dyn Fn(&mut image_emission::InstallationRecord)>,
        InstallationError,
    )> = vec![
        // ELF and Mach-O records carry no subsystem fact.
        (
            "subsystem",
            Box::new(|record| {
                *record.subsystem_mut_for_test() = Some(3);
            }),
            InstallationError::UnexpectedSubsystem,
        ),
        // `object_format` has no representable substitution on an x86-64
        // record: Mach-O emits only for AArch64 and COFF requires a
        // subsystem.
        (
            "target.object_format",
            Box::new(|record| {
                record.target_mut_for_test().object_format = target::ObjectFormat::MachO;
            }),
            InstallationError::UnsupportedTarget(NativeTarget {
                architecture: target::Architecture::X86_64,
                object_format: target::ObjectFormat::MachO,
                pointer_size: 8,
                pointer_alignment: 8,
            }),
        ),
        (
            "target.object_format",
            Box::new(|record| {
                record.target_mut_for_test().object_format = target::ObjectFormat::Coff;
            }),
            InstallationError::MissingCoffSubsystem,
        ),
        (
            "target.pointer_size",
            Box::new(|record| {
                record.target_mut_for_test().pointer_size = 4;
            }),
            InstallationError::UnsupportedTarget(NativeTarget {
                architecture: target::Architecture::X86_64,
                object_format: target::ObjectFormat::Elf,
                pointer_size: 4,
                pointer_alignment: 8,
            }),
        ),
        (
            "target.pointer_alignment",
            Box::new(|record| {
                record.target_mut_for_test().pointer_alignment = 16;
            }),
            InstallationError::UnsupportedTarget(NativeTarget {
                architecture: target::Architecture::X86_64,
                object_format: target::ObjectFormat::Elf,
                pointer_size: 8,
                pointer_alignment: 16,
            }),
        ),
        // The section projection re-derives its facts from the retained
        // function roster and the initialized-data prefix.
        (
            "image_sections.layout.text_address",
            Box::new(|record| {
                record.image_sections_mut_for_test().layout.text_address = 0;
            }),
            InstallationError::InvalidImageSectionLayout,
        ),
        (
            "image_sections.text_byte_count",
            Box::new(|record| {
                record.image_sections_mut_for_test().text_byte_count += 8;
            }),
            InstallationError::InvalidImageSectionLayout,
        ),
        (
            "image_sections.data_byte_count",
            Box::new(|record| {
                record.image_sections_mut_for_test().data_byte_count += 8;
            }),
            InstallationError::InvalidImageSectionLayout,
        ),
        (
            "image_sections.final_data_fingerprint",
            Box::new(|record| {
                record.image_sections_mut_for_test().final_data_fingerprint =
                    image_emission::InitializedDataFingerprint::for_test([0x33; 32]);
            }),
            InstallationError::InvalidImageSectionLayout,
        ),
        // Every receipt input bound by the derivation digest — and the digest
        // itself — is non-canonical without a consistent recomputation.
        (
            "compiler_text_validation.encoded_text_digest",
            Box::new(move |record| {
                record
                    .compiler_text_validation_mut_for_test()
                    .encoded_text_digest = other_evidence.encoded_text_digest;
            }),
            InstallationError::InvalidCompilerTextDerivationDigest,
        ),
        (
            "compiler_text_validation.final_compiler_text_digest",
            Box::new(move |record| {
                record
                    .compiler_text_validation_mut_for_test()
                    .final_compiler_text_digest = other_evidence.final_compiler_text_digest;
            }),
            InstallationError::InvalidCompilerTextDerivationDigest,
        ),
        (
            "compiler_text_validation.relocation_envelope_digest",
            Box::new(move |record| {
                record
                    .compiler_text_validation_mut_for_test()
                    .relocation_envelope_digest = other_evidence.relocation_envelope_digest;
            }),
            InstallationError::InvalidCompilerTextDerivationDigest,
        ),
        (
            "compiler_text_validation.derivation_digest",
            Box::new(move |record| {
                record
                    .compiler_text_validation_mut_for_test()
                    .derivation_digest = other_evidence.derivation_digest;
            }),
            InstallationError::InvalidCompilerTextDerivationDigest,
        ),
        (
            "compiler_text_validation.encoded_text_report_fingerprint",
            Box::new(|record| {
                record
                    .compiler_text_validation_mut_for_test()
                    .encoded_text_report_fingerprint += 1;
            }),
            InstallationError::InvalidCompilerTextDerivationDigest,
        ),
        (
            "compiler_text_validation.final_compiler_text_report_fingerprint",
            Box::new(|record| {
                record
                    .compiler_text_validation_mut_for_test()
                    .final_compiler_text_report_fingerprint += 1;
            }),
            InstallationError::InvalidCompilerTextDerivationDigest,
        ),
        (
            "compiler_text_validation.relocation_envelope_report_fingerprint",
            Box::new(|record| {
                record
                    .compiler_text_validation_mut_for_test()
                    .relocation_envelope_report_fingerprint += 1;
            }),
            InstallationError::InvalidCompilerTextDerivationDigest,
        ),
        (
            "compiler_text_validation.checked_instruction_validation_report_fingerprint",
            Box::new(|record| {
                record
                    .compiler_text_validation_mut_for_test()
                    .checked_instruction_validation_report_fingerprint += 1;
            }),
            InstallationError::InvalidCompilerTextDerivationDigest,
        ),
        (
            "compiler_text_validation.checked_instruction_footprint_report_fingerprint",
            Box::new(|record| {
                record
                    .compiler_text_validation_mut_for_test()
                    .checked_instruction_footprint_report_fingerprint += 1;
            }),
            InstallationError::InvalidCompilerTextDerivationDigest,
        ),
        (
            "compiler_text_validation.text_relocation_count",
            Box::new(|record| {
                record
                    .compiler_text_validation_mut_for_test()
                    .text_relocation_count += 1;
            }),
            InstallationError::InvalidCompilerTextDerivationDigest,
        ),
        (
            "compiler_text_validation.checked_instruction_validation_count",
            Box::new(|record| {
                record
                    .compiler_text_validation_mut_for_test()
                    .checked_instruction_validation_count += 1;
            }),
            InstallationError::InvalidCompilerTextDerivationDigest,
        ),
    ];
    for (field, mutate, expected) in encode_rejected {
        let mut changed = record.clone();
        mutate(&mut changed);
        assert_ne!(changed, record, "{field}: substitution changes the record");
        assert_eq!(
            encode_installation_record(&changed),
            Err(expected),
            "{field}: substituted record is rejected at canonical encoding"
        );
    }

    // The subsystem axis is representable only where the writer records it:
    // on a PE/COFF image the substituted subsystem still encodes and replay
    // against the unchanged image rejects it.
    let mut coff_plan = two_function_plan();
    coff_plan.target = NativeTarget::windows_x64();
    let coff_artifact = build_object_artifact(&coff_plan).expect("COFF artifact");
    let coff_image = emit_executable_image(&coff_artifact, 3).expect("PE image");
    let coff_record =
        build_installation_record(&coff_image, ProfileDecisionId::new(19).expect("profile"))
            .expect("COFF record");
    assert_eq!(coff_record.subsystem(), Some(3));
    validate_installation_record(&coff_record, &coff_image).expect("COFF binding");
    let coff_fingerprint = installation_fingerprint(&coff_record).expect("COFF fingerprint");
    assert_header_substitution_rejected(
        &coff_record,
        &coff_image,
        coff_fingerprint,
        "subsystem",
        |record| {
            *record.subsystem_mut_for_test() = Some(4);
        },
    );
    for (field, mutate, expected) in [
        (
            "subsystem",
            Box::new(|record: &mut image_emission::InstallationRecord| {
                *record.subsystem_mut_for_test() = None;
            }) as Box<dyn Fn(&mut image_emission::InstallationRecord)>,
            InstallationError::MissingCoffSubsystem,
        ),
        // A retained subsystem is non-canonical on every non-COFF target.
        (
            "target",
            Box::new(|record: &mut image_emission::InstallationRecord| {
                *record.target_mut_for_test() = NativeTarget::linux_x64();
            }) as Box<dyn Fn(&mut image_emission::InstallationRecord)>,
            InstallationError::UnexpectedSubsystem,
        ),
        (
            "target.object_format",
            Box::new(|record: &mut image_emission::InstallationRecord| {
                record.target_mut_for_test().object_format = target::ObjectFormat::MachO;
            }) as Box<dyn Fn(&mut image_emission::InstallationRecord)>,
            InstallationError::UnsupportedTarget(NativeTarget {
                architecture: target::Architecture::X86_64,
                object_format: target::ObjectFormat::MachO,
                pointer_size: 8,
                pointer_alignment: 8,
            }),
        ),
    ] {
        let mut changed = coff_record.clone();
        mutate(&mut changed);
        assert_ne!(
            changed, coff_record,
            "{field}: substitution changes the record"
        );
        assert_eq!(
            encode_installation_record(&changed),
            Err(expected),
            "{field}: substituted COFF record is rejected at canonical encoding"
        );
    }

    // Axes without an in-memory representation still reject at the wire: the
    // fixed magic and format marker, the single unstable vocabulary marker,
    // unknown enum tags, the reserved field, zero profile and committed
    // progress identities, and presence-flag lies are alternate or malformed
    // encodings, never a canonical record.
    let canonical = encode_installation_record(&record).expect("canonical bytes");
    let mut foreign_magic = canonical.clone();
    foreign_magic[0] ^= 0xff;
    assert_eq!(
        decode_installation_record(&foreign_magic),
        Err(InstallationError::InvalidMagic)
    );
    let mut foreign_format = canonical.clone();
    foreign_format[8..10].copy_from_slice(&(INSTALLATION_FORMAT_MARKER + 1).to_le_bytes());
    assert_eq!(
        decode_installation_record(&foreign_format),
        Err(InstallationError::UnsupportedFormatMarker(
            INSTALLATION_FORMAT_MARKER + 1
        ))
    );
    let mut foreign_marker = canonical.clone();
    foreign_marker[10..12].copy_from_slice(&(VocabularyMarker::CURRENT.get() + 1).to_le_bytes());
    assert_eq!(
        decode_installation_record(&foreign_marker),
        Err(InstallationError::UnsupportedVocabularyMarker(
            VocabularyMarker::CURRENT.get() + 1
        ))
    );
    let mut foreign_architecture = canonical.clone();
    foreign_architecture[44] = 0;
    assert_eq!(
        decode_installation_record(&foreign_architecture),
        Err(InstallationError::InvalidArchitectureTag(0))
    );
    let mut foreign_object_format = canonical.clone();
    foreign_object_format[45] = 0;
    assert_eq!(
        decode_installation_record(&foreign_object_format),
        Err(InstallationError::InvalidObjectFormatTag(0))
    );
    let mut nonzero_reserved = canonical.clone();
    nonzero_reserved[66] = 1;
    assert_eq!(
        decode_installation_record(&nonzero_reserved),
        Err(InstallationError::NonzeroReservedField)
    );
    let mut zero_profile = canonical.clone();
    zero_profile[68..76].fill(0);
    assert_eq!(
        decode_installation_record(&zero_profile),
        Err(InstallationError::ZeroProfileDecision)
    );
    // The committed progress identities sit at fixed header offsets behind
    // the presence flag; pinning the bytes before zeroing keeps the axes
    // aligned with the encoder layout.
    assert_eq!(&canonical[76..84], &0x1122_u64.to_le_bytes());
    assert_eq!(&canonical[84..92], &0x3344_u64.to_le_bytes());
    let mut zero_manifest = canonical.clone();
    zero_manifest[76..84].fill(0);
    assert_eq!(
        decode_installation_record(&zero_manifest),
        Err(InstallationError::ZeroComponentProgressManifestIdentity)
    );
    let mut zero_acceptance = canonical.clone();
    zero_acceptance[84..92].fill(0);
    assert_eq!(
        decode_installation_record(&zero_acceptance),
        Err(InstallationError::ZeroComponentProgressAcceptanceIdentity)
    );
    let mut hidden_subsystem = canonical.clone();
    hidden_subsystem[64..66].copy_from_slice(&3_u16.to_le_bytes());
    assert_eq!(
        decode_installation_record(&hidden_subsystem),
        Err(InstallationError::NonCanonicalSubsystem)
    );
    // A presence flag without its payload decodes an out-of-shape record.
    let mut claimed_subsystem = canonical.clone();
    claimed_subsystem[46] = 1;
    assert_eq!(
        decode_installation_record(&claimed_subsystem),
        Err(InstallationError::UnexpectedSubsystem)
    );
    let mut dropped_progress = canonical.clone();
    dropped_progress[47] = 0;
    assert_eq!(
        decode_installation_record(&dropped_progress),
        Err(InstallationError::InvalidCallSiteOwnerTag(0)),
        "an absent progress flag cannot absorb the committed identity bytes"
    );
}

/// Every representable scalar field of an installed function row is an
/// authenticated custody axis: a one-field substitution still encodes,
/// recomputes a distinct installation fingerprint, and independent replay
/// against the unchanged image rejects it. Roster-level rows (call stacks,
/// homes, continuations) are covered by their producing fixtures elsewhere.
#[test]
fn installation_function_row_rejects_every_one_field_substitution() {
    let plan = two_function_plan();
    let artifact = build_object_artifact(&plan).expect("artifact");
    let image = emit_executable_image(&artifact, 3).expect("Linux image");
    let record = build_installation_record(&image, ProfileDecisionId::new(11).expect("profile"))
        .expect("installation record");
    validate_installation_record(&record, &image).expect("exact image binding");
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");
    let [_, authentic] = record.functions() else {
        panic!("two-function fixture retains two rows");
    };
    assert_eq!(authentic.attachment, None);
    assert!(!authentic.unit_body);
    assert_eq!(authentic.unit_stack, None);
    assert_eq!(authentic.scalar_stack, None);

    let mutations: [(&str, fn(&mut image_emission::InstalledFunction)); 4] = [
        ("machine", |row| {
            row.machine = MachineId::new(row.machine.get() + 100).expect("drifted machine");
        }),
        ("attachment", |row| {
            row.attachment = Some(StructuralTypeId::new(7).expect("drifted attachment"));
        }),
        ("unit_stack", |row| {
            row.unit_stack = Some(image_emission::ObjectUnitStack {
                frame_bytes: 16,
                local_peak_bytes: 0,
                stack_alignment: 16,
            });
        }),
        ("scalar_stack", |row| {
            row.scalar_stack = Some(image_emission::ObjectScalarStack {
                local_peak_bytes: 0,
                stack_alignment: 16,
            });
        }),
    ];
    for (field, mutate) in mutations {
        let mut changed = record.clone();
        mutate(&mut changed.functions_mut_for_test()[1]);
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

    // Text intervals are not independently representable: canonical rows are
    // contiguous and exhaust the text section, so the encoder rejects a shifted
    // offset, a shortened interval, or a dropped row before any identity or
    // replay could accept it.
    let mut shifted_offset = record.clone();
    shifted_offset.functions_mut_for_test()[1].text_offset += 1;
    assert_eq!(
        encode_installation_record(&shifted_offset),
        Err(InstallationError::NonCanonicalInstalledFunctions)
    );
    let mut shortened = record.clone();
    shortened.functions_mut_for_test()[1].byte_count -= 1;
    assert_eq!(
        encode_installation_record(&shortened),
        Err(InstallationError::InvalidImageSectionLayout)
    );
    let mut dropped_row = record.clone();
    dropped_row.functions_mut_for_test().pop();
    assert_eq!(
        encode_installation_record(&dropped_row),
        Err(InstallationError::InvalidImageSectionLayout)
    );
    // `unit_body` is a projection of the retained affine cleanup; flipping it
    // alone is likewise rejected at encoding.
    let mut flipped_body = record.clone();
    flipped_body.functions_mut_for_test()[1].unit_body = true;
    assert_eq!(
        encode_installation_record(&flipped_body),
        Err(InstallationError::InvalidUnitAffineCleanup(machine_id(2)))
    );
}

/// Every representable field of an installed internal Unit-call row is an
/// authenticated custody axis: a one-field substitution either cannot encode
/// canonically or still encodes, recomputes a distinct installation
/// fingerprint, and independent replay against the unchanged image rejects it.
#[test]
fn installation_internal_unit_call_row_rejects_every_one_field_substitution() {
    let plan = two_call_edge_owned_cleanup_plan();
    let artifact = build_object_artifact(&plan).expect("cleanup artifact");
    let image = emit_executable_image(&artifact, 3).expect("cleanup image");
    let record = build_installation_record(&image, ProfileDecisionId::new(41).expect("profile"))
        .expect("cleanup installation");
    validate_installation_record(&record, &image).expect("exact image binding");
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");
    assert_eq!(record.internal_unit_calls().len(), 3);
    let authentic = &record.internal_unit_calls()[0];
    assert_eq!(authentic.machine, machine_id(1));
    assert_eq!(
        authentic.custody.owner,
        CallSiteOwner::Operation(operation_id(1))
    );
    assert_eq!(authentic.custody.target, machine_id(2));
    assert_eq!(authentic.custody.result, None);
    assert_eq!(authentic.custody.semantic_result, None);
    assert_eq!(authentic.custody.structural_result, None);
    assert!(authentic.custody.scalar_arguments.is_empty());
    assert!(authentic.custody.arguments.is_empty());
    assert!(authentic.custody.claim_transfers.is_empty());
    assert_eq!(
        record.internal_unit_calls()[2].custody.owner,
        CallSiteOwner::CleanupAction {
            edge: edge_id(3),
            action_ordinal: 0,
        }
    );

    let i32_type =
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 32)
            .expect("i32");
    let empty_placement = ValuePlacement {
        shape: ValueShape::integer(0, 1),
        locations: Vec::new(),
    };
    let scalar_placement = ValuePlacement {
        shape: ValueShape::integer(4, 4),
        locations: Vec::new(),
    };
    let provider_source = machine_code::InternalUnitCallSource::InstalledProvider {
        boundary: semantic_vocabulary::BoundaryMachineId::new(7).unwrap(),
        provider: Box::new(terminal_psi::ProviderCandidateConformance {
            boundary: semantic_vocabulary::BoundaryMachineId::new(7).unwrap(),
            requirement_identity: "requirement".into(),
            provider_identity: "provider".into(),
            candidate_identity: "candidate".into(),
            candidate: machine_id(4),
            signature: terminal_psi::ProviderSignature {
                parameters: Vec::new(),
            },
            refinement: terminal_psi::ProviderRefinement {
                positional_parameters: Vec::new(),
                required_domains: Vec::new(),
                realized_service_ceiling: Vec::new(),
            },
        }),
        completion_claim_sources: Vec::new(),
        completion_receipts: Vec::new(),
    };
    let affine_structural_result = machine_code::InternalStructuralCallResult {
        operation_result: terminal_psi::StructuralOperationResult {
            place: PlaceId::new(31).unwrap(),
            structural_type: StructuralTypeId::new(31).unwrap(),
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        },
        result_home: None,
        function_result: terminal_psi::StructuralResultDeclaration {
            place: PlaceId::new(31).unwrap(),
            structural_type: StructuralTypeId::new(31).unwrap(),
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            reference_sources: Vec::new(),
        },
        returned_claim_transfers: Vec::new(),
        returned_claims: Vec::new(),
        caller_result_placement: empty_placement.clone(),
        callee_result_placement: empty_placement.clone(),
    };
    let scalar_argument = machine_code::InternalUnitScalarCallArgumentRecord {
        parameter_index: 0,
        source: machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
            defining_operation: operation_id(1),
            source_value: semantic_vocabulary::ValueId::new(31).unwrap(),
            scalar_type: i32_type,
            value: semantic_vocabulary::IntegerValue::Signed(7),
        },
        destination: scalar_placement.clone(),
        code_offset: 0,
        byte_count: 4,
    };
    let structural_argument = machine_code::InternalUnitCallArgumentRecord {
        place: PlaceId::new(31).unwrap(),
        access: StructuralAccess::Owned,
        path: Vec::new(),
        root_structural_type: StructuralTypeId::new(31).unwrap(),
        structural_type: StructuralTypeId::new(31).unwrap(),
        shape: ValueShape::integer(0, 1),
        source_byte_offset: 0,
        source_location: machine_code::StructuralSourceLocation::Stack { byte_offset: 0 },
        call_stack_bytes: 0,
        fixed_array_length: None,
        element_stride: None,
        source: machine_code::InternalUnitStructuralArgumentSourceRecord::Placement(
            empty_placement.clone(),
        ),
        destination: empty_placement,
        code_offset: 0,
        byte_count: 0,
        bytes: Vec::new(),
    };

    use std::rc::Rc;
    let provider_source = Rc::new(provider_source);
    let affine_structural_result = Rc::new(affine_structural_result);
    let scalar_argument = Rc::new(scalar_argument);
    let structural_argument = Rc::new(structural_argument);

    // A cleanup-owned call's `byte_count` is the one field no record-shape
    // join pins: the cleanup action's extent is carried by the cleanup
    // record itself, so the substituted row still encodes. It recomputes a
    // distinct installation fingerprint, and independent replay against the
    // unchanged image rejects it.
    let mut changed = record.clone();
    changed.internal_unit_calls_mut_for_test()[2]
        .custody
        .byte_count += 1;
    let bytes =
        encode_installation_record(&changed).expect("substituted cleanup byte_count encodes");
    let replayed =
        decode_installation_record(&bytes).expect("substituted cleanup byte_count decodes");
    assert_eq!(replayed, changed);
    assert_ne!(
        installation_fingerprint(&replayed).expect("substituted fingerprint"),
        authentic_fingerprint,
        "byte_count: recomputed identity differs from the authentic record"
    );
    assert_eq!(
        validate_installation_record(&replayed, &image),
        Err(InstallationError::ImageBindingMismatch),
        "byte_count: independent replay rejects the substituted row"
    );

    // Every other field on every row is a canonical projection bound by the
    // caller's attribution and cleanup joins or the callee's own call shape,
    // so a one-field substitution is rejected at encoding before any identity
    // or replay could accept it.
    let provider_source_owned = provider_source.clone();
    let provider_source_cleanup = provider_source.clone();
    let affine_structural_result_owned = affine_structural_result.clone();
    let affine_structural_result_cleanup = affine_structural_result.clone();
    let scalar_argument_owned = scalar_argument.clone();
    let scalar_argument_cleanup = scalar_argument.clone();
    let scalar_argument_second = scalar_argument.clone();
    let structural_argument_owned = structural_argument.clone();
    let structural_argument_cleanup = structural_argument.clone();

    type CallMutation = (
        &'static str,
        usize,
        Box<dyn Fn(&mut image_emission::InstalledInternalUnitCall)>,
        InstallationError,
    );
    let cleanup_join = InstallationError::InvalidUnitAffineCleanup(machine_id(3));
    let mutations: Vec<CallMutation> = vec![
        (
            "machine",
            0,
            Box::new(|row| {
                row.machine = machine_id(2);
            }),
            cleanup_join.clone(),
        ),
        (
            "text_offset",
            0,
            Box::new(|row| {
                row.text_offset += 1;
            }),
            InstallationError::InvalidInternalUnitCall(machine_id(1)),
        ),
        (
            "owner",
            0,
            Box::new(|row| {
                row.custody.owner = CallSiteOwner::Operation(operation_id(7));
            }),
            InstallationError::InvalidInternalUnitCall(machine_id(1)),
        ),
        (
            "owner::cleanup_action",
            2,
            Box::new(|row| {
                row.custody.owner = CallSiteOwner::CleanupAction {
                    edge: edge_id(4),
                    action_ordinal: 1,
                };
            }),
            cleanup_join.clone(),
        ),
        (
            "source",
            0,
            Box::new(move |row| {
                row.custody.source = (*provider_source_owned).clone();
            }),
            InstallationError::InvalidInternalUnitCall(machine_id(1)),
        ),
        (
            "target",
            0,
            Box::new(|row| {
                row.custody.target = machine_id(4);
            }),
            cleanup_join.clone(),
        ),
        (
            "result",
            0,
            Box::new(|row| {
                row.custody.result = Some(semantic_vocabulary::ScalarType::Boolean);
            }),
            cleanup_join.clone(),
        ),
        (
            "structural_result",
            0,
            Box::new(move |row| {
                row.custody.structural_result = Some((*affine_structural_result_owned).clone());
            }),
            InstallationError::InvalidUnitAffineCleanup(machine_id(1)),
        ),
        (
            "scalar_arguments",
            0,
            Box::new(move |row| {
                row.custody
                    .scalar_arguments
                    .push((*scalar_argument_owned).clone());
            }),
            InstallationError::InvalidInternalUnitCall(machine_id(1)),
        ),
        (
            "arguments",
            0,
            Box::new(move |row| {
                row.custody
                    .arguments
                    .push((*structural_argument_owned).clone());
            }),
            cleanup_join.clone(),
        ),
        (
            "claim_transfers",
            0,
            Box::new(|row| {
                row.custody
                    .claim_transfers
                    .push(terminal_psi::ClaimTransfer {
                        claim: ClaimId::new(31).unwrap(),
                        argument_index: 0,
                    });
            }),
            cleanup_join.clone(),
        ),
        (
            "operation_ordinal",
            0,
            Box::new(|row| {
                row.custody.operation_ordinal += 1;
            }),
            cleanup_join.clone(),
        ),
        (
            "code_offset",
            0,
            Box::new(|row| {
                row.custody.code_offset += 1;
            }),
            cleanup_join.clone(),
        ),
        (
            "code_offset::second_call",
            1,
            Box::new(|row| {
                row.custody.code_offset += 1;
            }),
            InstallationError::InvalidInternalUnitCall(machine_id(1)),
        ),
        (
            "byte_count",
            0,
            Box::new(|row| {
                row.custody.byte_count += 1;
            }),
            cleanup_join.clone(),
        ),
        (
            "byte_count::second_call",
            1,
            Box::new(|row| {
                row.custody.byte_count += 1;
            }),
            InstallationError::InvalidInternalUnitCall(machine_id(1)),
        ),
        (
            "machine::cleanup_call",
            2,
            Box::new(|row| {
                row.machine = machine_id(4);
            }),
            cleanup_join.clone(),
        ),
        (
            "text_offset::cleanup_call",
            2,
            Box::new(|row| {
                row.text_offset += 1;
            }),
            InstallationError::InvalidInternalUnitCall(machine_id(3)),
        ),
        (
            "code_offset::cleanup_call",
            2,
            Box::new(|row| {
                row.custody.code_offset += 1;
            }),
            cleanup_join.clone(),
        ),
        (
            "operation_ordinal::cleanup_call",
            2,
            Box::new(|row| {
                row.custody.operation_ordinal += 1;
            }),
            InstallationError::InvalidInternalUnitCall(machine_id(3)),
        ),
        (
            "source::cleanup_call",
            2,
            Box::new(move |row| {
                row.custody.source = (*provider_source_cleanup).clone();
            }),
            InstallationError::InvalidInternalUnitCall(machine_id(3)),
        ),
        (
            "result::cleanup_call",
            2,
            Box::new(|row| {
                row.custody.result = Some(semantic_vocabulary::ScalarType::Boolean);
            }),
            InstallationError::InvalidInternalUnitCall(machine_id(3)),
        ),
        (
            "scalar_arguments::cleanup_call",
            2,
            Box::new(move |row| {
                row.custody
                    .scalar_arguments
                    .push((*scalar_argument_cleanup).clone());
            }),
            InstallationError::InvalidInternalUnitCall(machine_id(3)),
        ),
        (
            "arguments::cleanup_call",
            2,
            Box::new(move |row| {
                row.custody
                    .arguments
                    .push((*structural_argument_cleanup).clone());
            }),
            cleanup_join.clone(),
        ),
        (
            "structural_result::cleanup_call",
            2,
            Box::new(move |row| {
                row.custody.structural_result = Some((*affine_structural_result_cleanup).clone());
            }),
            cleanup_join.clone(),
        ),
        (
            "target::second_call",
            1,
            Box::new(|row| {
                row.custody.target = machine_id(2);
            }),
            cleanup_join.clone(),
        ),
        (
            "operation_ordinal::second_call",
            1,
            Box::new(|row| {
                row.custody.operation_ordinal += 1;
            }),
            cleanup_join.clone(),
        ),
        (
            "scalar_arguments::second_call",
            1,
            Box::new(move |row| {
                row.custody
                    .scalar_arguments
                    .push((*scalar_argument_second).clone());
            }),
            InstallationError::InvalidInternalUnitCall(machine_id(1)),
        ),
        (
            "target::cleanup_call",
            2,
            Box::new(|row| {
                row.custody.target = machine_id(4);
            }),
            cleanup_join.clone(),
        ),
        (
            "claim_transfers::cleanup_call",
            2,
            Box::new(|row| {
                row.custody
                    .claim_transfers
                    .push(terminal_psi::ClaimTransfer {
                        claim: ClaimId::new(31).unwrap(),
                        argument_index: 0,
                    });
            }),
            cleanup_join.clone(),
        ),
        (
            "dropped_row",
            usize::MAX,
            Box::new(|_| {}),
            cleanup_join.clone(),
        ),
    ];
    for (field, row_index, mutate, expected) in mutations {
        let mut changed = record.clone();
        if row_index == usize::MAX {
            changed.internal_unit_calls_mut_for_test().remove(0);
        } else {
            mutate(&mut changed.internal_unit_calls_mut_for_test()[row_index]);
        }
        assert_ne!(changed, record, "{field}: substitution changes the row");
        assert_eq!(
            encode_installation_record(&changed),
            Err(expected),
            "{field}: substituted row is rejected at canonical encoding"
        );
    }

    // `semantic_result` is a canonical projection of `result`: a value whose
    // scalar type does not equal the retained ABI result is rejected at
    // encoding rather than reaching replay.
    let mut changed = record.clone();
    changed.internal_unit_calls_mut_for_test()[0]
        .custody
        .semantic_result = Some(abstract_operations::AbstractResult {
        value: semantic_vocabulary::ValueId::new(31).unwrap(),
        scalar_type: semantic_vocabulary::ScalarType::Boolean,
    });
    assert_eq!(
        encode_installation_record(&changed),
        Err(InstallationError::InvalidInternalUnitCall(machine_id(1)))
    );
}

#[derive(Debug)]
struct TestComponentProgressAcceptance {
    manifest: u64,
    acceptance: u64,
}

impl ComponentProgressAcceptanceEvidence for TestComponentProgressAcceptance {
    fn component_progress_manifest_identity(&self) -> u64 {
        self.manifest
    }

    fn component_progress_acceptance_identity(&self) -> u64 {
        self.acceptance
    }
}

#[test]
fn installation_record_fingerprints_component_progress_acceptance() {
    let artifact = build_object_artifact(&two_function_plan()).expect("artifact");
    let image = emit_executable_image(&artifact, 3).expect("image");
    let profile = ProfileDecisionId::new(12).expect("profile decision");
    let plain = build_installation_record(&image, profile).expect("plain record");
    let acceptance = TestComponentProgressAcceptance {
        manifest: 0x1122,
        acceptance: 0x3344,
    };
    let committed = build_installation_record_with_evidence(
        &image,
        profile,
        std::iter::empty::<&dyn ProviderExecutionEvidence>(),
        Some(&acceptance),
    )
    .expect("progress-bound record");

    let progress = committed
        .component_progress()
        .expect("component progress projection");
    assert_eq!(progress.manifest_identity(), 0x1122);
    assert_eq!(progress.acceptance_identity(), 0x3344);
    assert_ne!(
        installation_fingerprint(&plain).expect("plain fingerprint"),
        installation_fingerprint(&committed).expect("committed fingerprint")
    );
    let bytes = encode_installation_record(&committed).expect("canonical bytes");
    assert_eq!(decode_installation_record(&bytes), Ok(committed));

    let zero = TestComponentProgressAcceptance {
        manifest: 0,
        acceptance: 0x3344,
    };
    assert_eq!(
        build_installation_record_with_evidence(
            &image,
            profile,
            std::iter::empty::<&dyn ProviderExecutionEvidence>(),
            Some(&zero),
        ),
        Err(InstallationError::ZeroComponentProgressManifestIdentity)
    );
}

#[test]
fn installation_record_retains_selected_provider_plan_without_execution() {
    let artifact = build_object_artifact(&two_function_plan()).expect("artifact");
    let image = emit_executable_image(&artifact, 3).expect("image");
    let profile = ProfileDecisionId::new(13).expect("profile decision");
    let record = build_installation_record_with_selected_provider_plans_and_evidence(
        &image,
        profile,
        [91],
        std::iter::empty::<&dyn ProviderExecutionEvidence>(),
        None,
    )
    .expect("selected but unexecuted provider plan remains installation identity");

    assert_eq!(record.selected_provider_plans()[0].get(), 91);
    let bytes = encode_installation_record(&record).expect("canonical bytes");
    assert_eq!(decode_installation_record(&bytes), Ok(record));

    assert_eq!(
        build_installation_record_with_selected_provider_plans_and_evidence(
            &image,
            profile,
            [91, 91],
            std::iter::empty::<&dyn ProviderExecutionEvidence>(),
            None,
        ),
        Err(InstallationError::DuplicateProviderPlan)
    );
}

/// The selected provider-plan roster is admission-owned custody. The executed
/// subset is pinned by the boundary settlements' admitted-provider execution
/// records: substituting or dropping the executed plan, clearing or reordering
/// the roster, or duplicating an entry is rejected at canonical encoding. The
/// unexecuted remainder is representable — exact selection authority stays
/// outside the decodable record — so each substitution still encodes,
/// recomputes a distinct installation fingerprint, and is rejected by the
/// published record identity a deployment journal replays rather than by the
/// image join. Malformed identities and non-canonical wire order reject at
/// decode.
#[test]
fn installation_selected_provider_plan_rejects_every_one_field_substitution() {
    let provider = WriteExitProvider(7);
    let plan = port_effect_plan(&provider);
    let artifact = build_object_artifact(&plan).expect("port-effect artifact");
    let image = emit_executable_image(&artifact, 3).expect("port-effect image");
    let profile = ProfileDecisionId::new(23).expect("profile decision");
    // The settlement's admitted-provider execution requires plan 7; plan 42 is
    // selected but never executes in this image.
    let record = build_installation_record_with_selected_provider_plans_and_evidence(
        &image,
        profile,
        [7, 42],
        [&provider],
        None,
    )
    .expect("selected closure with an unexecuted plan");
    assert_eq!(
        record
            .selected_provider_plans()
            .iter()
            .map(|plan| plan.get())
            .collect::<Vec<_>>(),
        [7, 42]
    );
    validate_installation_record(&record, &image).expect("exact image binding");
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");

    // Every unexecuted-selection substitution is independently representable:
    // the mutated roster is itself a genuine admitted selection producing the
    // same record, the substitution still encodes and round-trips, and the
    // recomputed installation fingerprint differs from the authentic record —
    // the published identity a deployment journal replays. The image join
    // cannot see the change, so image replay still accepts.
    let representable: [(&str, &[u64]); 3] = [
        ("unexecuted element", &[7, 43]),
        ("extended selection", &[7, 42, 99]),
        ("dropped unexecuted", &[7]),
    ];
    for (field, plans) in representable {
        let mut changed = record.clone();
        *changed.selected_provider_plans_mut_for_test() = plans
            .iter()
            .map(|plan| {
                image_emission::SelectedProviderPlanReportIdentity::new(*plan)
                    .expect("nonzero plan identity")
            })
            .collect();
        assert_ne!(changed, record, "{field}: substitution changes the record");
        let admitted = build_installation_record_with_selected_provider_plans_and_evidence(
            &image,
            profile,
            plans.iter().copied(),
            [&provider],
            None,
        )
        .unwrap_or_else(|error| panic!("{field}: substituted roster admits: {error:?}"));
        assert_eq!(
            changed, admitted,
            "{field}: substitution is itself an admitted selection"
        );
        let bytes = encode_installation_record(&changed)
            .unwrap_or_else(|error| panic!("{field}: substituted record encodes: {error:?}"));
        let replayed = decode_installation_record(&bytes)
            .unwrap_or_else(|error| panic!("{field}: substituted record decodes: {error:?}"));
        assert_eq!(
            replayed, changed,
            "{field}: codec preserves the substituted record"
        );
        assert_ne!(
            installation_fingerprint(&replayed)
                .unwrap_or_else(|error| panic!("{field}: substituted fingerprint: {error:?}")),
            authentic_fingerprint,
            "{field}: recomputed identity differs from the authentic record"
        );
        assert_eq!(
            validate_installation_record(&replayed, &image),
            Ok(()),
            "{field}: the selected closure sits outside the image join"
        );
    }

    // The executed subset is bound by the record-shape closure join: a
    // substitution that loses the required plan, an emptied roster, a
    // non-canonical order, or a duplicated entry all fail at encoding before
    // any identity or replay check.
    let encode_rejected: [(
        &str,
        fn(&mut image_emission::InstallationRecord),
        InstallationError,
    ); 6] = [
        (
            "executed element",
            |record| {
                record.selected_provider_plans_mut_for_test()[0] =
                    image_emission::SelectedProviderPlanReportIdentity::new(8)
                        .expect("nonzero plan identity");
            },
            InstallationError::ProviderSettlementClosureMismatch,
        ),
        (
            "dropped executed",
            |record| {
                record.selected_provider_plans_mut_for_test().remove(0);
            },
            InstallationError::ProviderSettlementClosureMismatch,
        ),
        (
            "cleared roster",
            |record| {
                record.selected_provider_plans_mut_for_test().clear();
            },
            InstallationError::ProviderSettlementClosureMismatch,
        ),
        (
            "reordered roster",
            |record| {
                record.selected_provider_plans_mut_for_test().swap(0, 1);
            },
            InstallationError::NonCanonicalProviderPlanOrder,
        ),
        (
            "duplicated element",
            |record| {
                let plans = record.selected_provider_plans_mut_for_test();
                plans.insert(1, plans[0]);
            },
            InstallationError::NonCanonicalProviderPlanOrder,
        ),
        (
            "out-of-order substitution",
            |record| {
                record.selected_provider_plans_mut_for_test()[1] =
                    image_emission::SelectedProviderPlanReportIdentity::new(3)
                        .expect("nonzero plan identity");
            },
            InstallationError::NonCanonicalProviderPlanOrder,
        ),
    ];
    for (field, mutate, expected) in encode_rejected {
        let mut changed = record.clone();
        mutate(&mut changed);
        assert_ne!(changed, record, "{field}: substitution changes the record");
        assert_eq!(
            encode_installation_record(&changed),
            Err(expected),
            "{field}: non-canonical substitution rejected at encoding"
        );
    }

    // Admission binds the roster both ways: every reported execution must sit
    // inside the selected closure, and the reported closure must match the
    // image's retained executions exactly.
    assert_eq!(
        build_installation_record_with_selected_provider_plans_and_evidence(
            &image,
            profile,
            [42],
            [&provider],
            None,
        ),
        Err(InstallationError::ProviderExecutionOutsideSelectedClosure)
    );
    assert_eq!(
        build_installation_record_with_selected_provider_plans_and_evidence(
            &image,
            profile,
            [7, 970],
            [&provider, &WriteExitProvider(970)],
            None,
        ),
        Err(InstallationError::ProviderExecutionClosureMismatch)
    );
    assert_eq!(
        build_installation_record_with_selected_provider_plans_and_evidence(
            &image,
            profile,
            [7, 0],
            [&provider],
            None,
        ),
        Err(InstallationError::ZeroProviderPlan)
    );

    // Axes without an in-memory representation still reject at the wire: a
    // zero identity, a non-canonical or duplicated order, and a count the
    // remaining bytes cannot carry are malformed encodings, never a canonical
    // record.
    let canonical = encode_installation_record(&record).expect("canonical bytes");
    let plan_pair = [7_u64.to_le_bytes(), 42_u64.to_le_bytes()].concat();
    let plan_offset = canonical
        .windows(plan_pair.len())
        .position(|window| window == plan_pair.as_slice())
        .expect("adjacent provider-plan identities");
    let count_offset = plan_offset - 4;
    assert_eq!(
        u32::from_le_bytes(canonical[count_offset..plan_offset].try_into().unwrap()),
        2
    );
    let mut zero_identity = canonical.clone();
    zero_identity[plan_offset..plan_offset + 8].fill(0);
    assert_eq!(
        decode_installation_record(&zero_identity),
        Err(InstallationError::ZeroProviderPlan)
    );
    let mut wire_reordered = canonical.clone();
    wire_reordered[plan_offset..plan_offset + 8].copy_from_slice(&42_u64.to_le_bytes());
    wire_reordered[plan_offset + 8..plan_offset + 16].copy_from_slice(&7_u64.to_le_bytes());
    assert_eq!(
        decode_installation_record(&wire_reordered),
        Err(InstallationError::NonCanonicalProviderPlanOrder)
    );
    let mut wire_duplicated = canonical.clone();
    wire_duplicated[plan_offset + 8..plan_offset + 16].copy_from_slice(&7_u64.to_le_bytes());
    assert_eq!(
        decode_installation_record(&wire_duplicated),
        Err(InstallationError::NonCanonicalProviderPlanOrder)
    );
    let mut inflated_count = canonical.clone();
    inflated_count[count_offset..plan_offset].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        decode_installation_record(&inflated_count),
        Err(InstallationError::UnexpectedEnd)
    );
}

#[test]
fn installation_decoder_rejects_alternate_and_malformed_encodings() {
    let artifact = build_object_artifact(&two_function_plan()).expect("artifact");
    let image = emit_executable_image(&artifact, 3).expect("image");
    let record =
        build_installation_record(&image, ProfileDecisionId::new(1).unwrap()).expect("record");
    let bytes = encode_installation_record(&record).expect("bytes");

    let mut future = bytes.clone();
    let future_marker = INSTALLATION_FORMAT_MARKER + 1;
    future[8..10].copy_from_slice(&future_marker.to_le_bytes());
    assert_eq!(
        decode_installation_record(&future),
        Err(InstallationError::UnsupportedFormatMarker(future_marker))
    );

    let mut wrong_pointer_width = bytes.clone();
    wrong_pointer_width[48..56].copy_from_slice(&4_u64.to_le_bytes());
    assert!(matches!(
        decode_installation_record(&wrong_pointer_width),
        Err(InstallationError::UnsupportedTarget(_))
    ));

    let mut zero_profile = bytes.clone();
    zero_profile[68..76].copy_from_slice(&0_u64.to_le_bytes());
    assert_eq!(
        decode_installation_record(&zero_profile),
        Err(InstallationError::ZeroProfileDecision)
    );

    let mut changed_text_digest = bytes.clone();
    let compiler_text_validation = record.compiler_text_validation();
    let encoded_text_digest = compiler_text_validation.encoded_text_digest.as_bytes();
    let digest_offset = changed_text_digest
        .windows(encoded_text_digest.len())
        .position(|window| window == encoded_text_digest)
        .expect("encoded compiler-text digest");
    changed_text_digest[digest_offset] ^= 1;
    assert_eq!(
        decode_installation_record(&changed_text_digest),
        Err(InstallationError::InvalidCompilerTextDerivationDigest)
    );

    assert_eq!(
        decode_installation_record(&bytes[..bytes.len() - 1]),
        Err(InstallationError::UnexpectedEnd)
    );

    let mut trailing = bytes;
    trailing.push(0);
    assert_eq!(
        decode_installation_record(&trailing),
        Err(InstallationError::TrailingBytes(1))
    );
}

#[test]
fn privileged_effect_and_exact_provider_execution_survive_installation() {
    let port_operation = operation_id(1);
    let settlement_operation = operation_id(2);
    let service = ServiceId::new(1).unwrap();
    let boundary = BoundaryMachineId::new(1).unwrap();
    let provider_plan = ProviderPlanReportIdentity::new(7).unwrap();
    let provider_execution =
        ProviderExecutionBinding::from_execution_record(provider_plan, 8, 9, 10, 11).unwrap();
    let realization = MetadataOnlyPortRealization {
        effect_operation: port_operation,
        service,
        port: 0x20,
        value: 0x20,
    };
    let mut bytes = x86_encoding::encode_immediate_port_write(0x20, 0x20).to_vec();
    bytes.push(0xc3);
    let plan = MachineCodePlan {
        psi: identity(),
        target: NativeTarget::linux_x64(),
        entry: machine_id(1),
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
            machine: machine_id(1),
            attachment: None,
            provenance: TerminalPsiProvenance {
                operations: vec![port_operation, settlement_operation],
                edges: vec![edge_id(1)],
            },
            bytes,
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
                    site: SemanticCodeSite::Operation(port_operation),
                    operation_ordinal: 0,
                    code_offset: 0,
                    byte_count: 27,
                },
                SemanticCodeAttribution {
                    site: SemanticCodeSite::Operation(settlement_operation),
                    operation_ordinal: 1,
                    code_offset: 27,
                    byte_count: 0,
                },
                SemanticCodeAttribution {
                    site: SemanticCodeSite::Edge(edge_id(1)),
                    operation_ordinal: 2,
                    code_offset: 27,
                    byte_count: 1,
                },
            ],
            port_effects: vec![PortEffectRecord {
                psi_operation: port_operation,
                service,
                port: 0x20,
                value: 0x20,
                operation_ordinal: 0,
                code_offset: 0,
                byte_count: 27,
            }],
            boundary_settlements: vec![BoundarySettlementRecord {
                psi_operation: settlement_operation,
                boundary,
                execution: machine_code::BoundaryExecutionRecord::AdmittedProvider(
                    provider_execution.into(),
                ),
                realization: realization.into(),
                scalar_arguments: Vec::new(),
                runtime_scalar_arguments: Vec::new(),
                arguments: Vec::new(),
                byte_sequence_arguments: Vec::new(),
                completion_claim_sources: Vec::new(),
                completion_receipts: Vec::new(),
                completion_provider_custody: Vec::new(),
                native_result: machine_code::BoundaryResultRecord::Unit,
                operation_ordinal: 1,
                code_offset: 27,
                byte_count: 0,
            }],
            scalar_affine_cleanup: None,
            scalar_control_affine_cleanups: Vec::new(),
            scalar_structural_parameters: Vec::new(),
            scalar_structural_parameter_homes: Vec::new(),
            structural_return: None,
        }],
    };
    let artifact = build_object_artifact(&plan).expect("effect artifact");
    assert_eq!(artifact.semantic_code_attribution().len(), 3);
    assert_eq!(
        artifact.semantic_code_attribution()[1]
            .attribution
            .byte_count,
        0
    );
    assert_eq!(artifact.port_effects()[0].effect.service, service);
    assert_eq!(
        artifact.boundary_settlements()[0].settlement.realization,
        realization.into()
    );
    let image = emit_executable_image(&artifact, 3).expect("effect image");
    assert_eq!(
        image.semantic_code_attribution(),
        artifact.semantic_code_attribution()
    );
    assert_eq!(
        build_installation_record(&image, ProfileDecisionId::new(1).unwrap()),
        Err(InstallationError::ProviderExecutionClosureMismatch)
    );

    let mut wrong_bytes = plan.clone();
    wrong_bytes.functions[0].bytes[0] ^= 1;
    assert!(matches!(
        build_object_artifact(&wrong_bytes),
        Err(ObjectError::PortEffectBytesMismatch { .. })
    ));
    let mut duplicate_completion_claim = plan.clone();
    duplicate_completion_claim.functions[0].boundary_settlements[0].arguments = vec![
        StructuralArgument {
            access: StructuralAccess::Owned,
            place: PlaceId::new(1).unwrap(),
            path: Vec::new(),
        },
        StructuralArgument {
            access: StructuralAccess::Owned,
            place: PlaceId::new(2).unwrap(),
            path: Vec::new(),
        },
    ];
    duplicate_completion_claim.functions[0].boundary_settlements[0].completion_receipts = vec![
        CompletionReceipt {
            claim: ClaimId::new(1).unwrap(),
            argument_index: 0,
        },
        CompletionReceipt {
            claim: ClaimId::new(1).unwrap(),
            argument_index: 1,
        },
    ];
    duplicate_completion_claim.functions[0].boundary_settlements[0].completion_claim_sources =
        vec![CompletionClaimSource {
            claim: ClaimId::new(1).unwrap(),
            entry: Some(EntryClaim {
                claim: ClaimId::new(1).unwrap(),
                input: PlaceId::new(1).unwrap(),
                path: Vec::new(),
            }),
            content: None,
        }];
    assert_eq!(
        build_object_artifact(&duplicate_completion_claim),
        Err(ObjectError::InvalidCompletionReceiptCustody {
            machine: machine_id(1),
            operation: settlement_operation,
        })
    );
    let mut wrong_realization = plan;
    wrong_realization.functions[0].boundary_settlements[0].realization =
        MetadataOnlyPortRealization {
            value: 0x21,
            ..realization
        }
        .into();
    assert!(matches!(
        build_object_artifact(&wrong_realization),
        Err(ObjectError::BoundaryRealizationMismatch { .. })
    ));
}

#[test]
fn image_boundary_rejects_noncanonical_pointer_facts() {
    let mut plan = two_function_plan();
    plan.target.pointer_size = 4;
    assert!(!can_emit_executable_image(plan.target));
    let artifact = build_object_artifact(&plan).expect("owned artifact");
    assert!(emit_executable_image(&artifact, 3).is_err());
}

fn artifact_symbol(artifact: &image_emission::ObjectArtifact) -> &str {
    object_symbol_name(artifact.object(), artifact.entry_function().symbol)
}

fn two_function_plan() -> MachineCodePlan {
    MachineCodePlan {
        psi: identity(),
        target: NativeTarget::linux_x64(),
        entry: machine_id(2),
        functions: vec![
            MachineCodeFunction {
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
                machine: machine_id(1),
                attachment: None,
                provenance: TerminalPsiProvenance {
                    operations: vec![operation_id(1)],
                    edges: vec![edge_id(1)],
                },
                bytes: integer_return(3),
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
                semantic_code_attribution: Vec::new(),
                port_effects: Vec::new(),
                boundary_settlements: Vec::new(),
                scalar_affine_cleanup: None,
                scalar_control_affine_cleanups: Vec::new(),
                scalar_structural_parameters: Vec::new(),
                scalar_structural_parameter_homes: Vec::new(),
                structural_return: None,
            },
            MachineCodeFunction {
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
                machine: machine_id(2),
                attachment: None,
                provenance: TerminalPsiProvenance {
                    operations: vec![operation_id(2)],
                    edges: vec![edge_id(2)],
                },
                bytes: integer_return(7),
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
                semantic_code_attribution: Vec::new(),
                port_effects: Vec::new(),
                boundary_settlements: Vec::new(),
                scalar_affine_cleanup: None,
                scalar_control_affine_cleanups: Vec::new(),
                scalar_structural_parameters: Vec::new(),
                scalar_structural_parameter_homes: Vec::new(),
                structural_return: None,
            },
        ],
    }
}

fn callback_private_plan() -> machine_code::MachineCodePlanWithPrivateFunctions {
    let target = NativeTarget::linux_x64();
    let scalar_type = semantic_vocabulary::ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
            .expect("u64"),
    );
    let shape = ValueShape::integer(8, 8);
    let call_plan = calling_conventions::evaluate_call_plan(
        calling_conventions::CallingPolicy::native_for_target(target),
        &calling_conventions::CallSignature {
            parameters: vec![shape],
            result: Some(shape),
        },
    )
    .expect("one-u64 callback ABI");
    let mut function = two_function_plan().functions.remove(0);
    function.machine = machine_id(97);
    function.scalar_abi = Some(target_operations::ScalarFunctionAbi {
        parameters: vec![target_operations::ScalarAbiValue {
            value: semantic_vocabulary::ValueId::new(19).expect("parameter value"),
            scalar_type,
            placement: call_plan.parameters[0].clone(),
        }],
        result: target_operations::ScalarAbiValue {
            value: semantic_vocabulary::ValueId::new(23).expect("result value"),
            scalar_type,
            placement: call_plan.result.clone().expect("result placement"),
        },
        call_plan,
    });
    function.bytes = vec![0x48, 0x89, 0xf8, 0xc3];
    machine_code::MachineCodePlanWithPrivateFunctions {
        plan: two_function_plan(),
        private_functions: vec![machine_code::CompilerPrivateMachineCodeFunction {
            identity: MachineFunctionIdentity::callback_thunk(
                StateKey {
                    machine: SymbolHandle::from_parts(11, 2),
                    state: SymbolHandle::from_parts(13, 3),
                    segment_index: 0,
                },
                0,
            )
            .expect("callback thunk identity"),
            private_symbol: "__omega_test_callback_thunk".into(),
            source_psi: identity(),
            function,
        }],
    }
}

fn x86_fma_plan(profile: TargetProfile, format: X86ScalarFmaFormat) -> MachineCodePlan {
    let requirement = X86FeatureRequirement::scalar_fma(profile).expect("x86 FMA profile");
    let emitted = machine_emission::emit_feature_required_x86_scalar_fma(
        requirement,
        profile.native_target(),
        format,
        calling_conventions::MachineRegister::X86Xmm(0),
        calling_conventions::MachineRegister::X86Xmm(1),
        calling_conventions::MachineRegister::X86Xmm(2),
        0,
    )
    .expect("source-free scalar FMA emission");
    let mut plan = two_function_plan();
    plan.target = profile.native_target();
    plan.entry = machine_id(1);
    plan.functions.truncate(1);
    plan.functions[0].bytes = emitted.bytes.into_iter().chain([0xc3]).collect();
    plan.functions[0].x86_scalar_fma = vec![emitted.custody];
    plan
}

fn admitted_x86_fma_provider(profile: TargetProfile) -> AdmittedX86ScalarFmaProvider {
    let requirement = X86FeatureRequirement::scalar_fma(profile).unwrap();
    let deployment = X86DeploymentFeatures::scalar_fma(
        profile,
        &[X86TargetFeature::Avx, X86TargetFeature::Fma3],
    )
    .unwrap();
    let differential_receipts = [
        X86ScalarFmaDifferentialReceipt::admit(
            X86ScalarFmaSlot::Binary32,
            [0x3f80_0001, 0x3f7f_fffe, 0xbf80_0000],
            0xa880_0000,
            0,
        )
        .unwrap(),
        X86ScalarFmaDifferentialReceipt::admit(
            X86ScalarFmaSlot::Binary64,
            [
                0x3ff0_0000_0000_0001,
                0x3fef_ffff_ffff_fffe,
                0xbff0_0000_0000_0000,
            ],
            0xb970_0000_0000_0000,
            0,
        )
        .unwrap(),
    ];
    AdmittedX86ScalarFmaProvider::admit(requirement, deployment, differential_receipts).unwrap()
}

fn refresh_x86_fma_identity(fragment: &mut machine_code::X86ScalarFmaFragment) {
    fragment.identity = fragment
        .recomputed_identity()
        .expect("mutated test fragment remains structurally identity-bearing");
}

fn internal_call_plan(target: NativeTarget) -> MachineCodePlan {
    let (callee, caller, call_offset) = match target.architecture {
        target::Architecture::X86_64 => (integer_return(3), vec![0xe8, 0, 0, 0, 0, 0xc3], 1),
        target::Architecture::Aarch64 => (
            vec![0x60, 0x00, 0x80, 0x52, 0xc0, 0x03, 0x5f, 0xd6],
            vec![0x00, 0x00, 0x00, 0x94, 0xc0, 0x03, 0x5f, 0xd6],
            0,
        ),
    };
    MachineCodePlan {
        psi: identity(),
        target,
        entry: machine_id(2),
        functions: vec![
            MachineCodeFunction {
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
                machine: machine_id(1),
                attachment: None,
                provenance: TerminalPsiProvenance {
                    operations: vec![operation_id(1)],
                    edges: vec![edge_id(1)],
                },
                bytes: callee,
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
                semantic_code_attribution: Vec::new(),
                port_effects: Vec::new(),
                boundary_settlements: Vec::new(),
                scalar_affine_cleanup: None,
                scalar_control_affine_cleanups: Vec::new(),
                scalar_structural_parameters: Vec::new(),
                scalar_structural_parameter_homes: Vec::new(),
                structural_return: None,
            },
            MachineCodeFunction {
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
                machine: machine_id(2),
                attachment: None,
                provenance: TerminalPsiProvenance {
                    operations: vec![operation_id(2)],
                    edges: vec![edge_id(2)],
                },
                bytes: caller,
                x86_scalar_fma: Vec::new(),
                x86_scalar_fma_occurrences: Vec::new(),
                x86_floating_control: None,
                unit_stack: None,
                unit_parameter_homes: Vec::new(),
                unit_parameters: Vec::new(),
                scalar_stack: None,
                internal_calls: vec![InternalCallRelocation {
                    owner: target_operations::CallSiteOwner::Operation(operation_id(2)),
                    target: machine_id(1),
                    unit_stack: None,
                    scalar_stack: None,
                    offset: call_offset,
                }],
                foreign_calls: Vec::new(),
                internal_unit_calls: Vec::new(),
                unit_continuations: Vec::new(),
                unit_affine_cleanup: None,
                semantic_code_attribution: Vec::new(),
                port_effects: Vec::new(),
                boundary_settlements: Vec::new(),
                scalar_affine_cleanup: None,
                scalar_control_affine_cleanups: Vec::new(),
                scalar_structural_parameters: Vec::new(),
                scalar_structural_parameter_homes: Vec::new(),
                structural_return: None,
            },
        ],
    }
}

fn scalar_two_return_conditional_plan(target: NativeTarget) -> MachineCodePlan {
    let mut plan = two_function_plan();
    plan.target = target;
    plan.entry = machine_id(1);
    plan.functions.truncate(1);
    let function = &mut plan.functions[0];
    match target.architecture {
        target::Architecture::X86_64 => {
            function.bytes = vec![
                0x89, 0xf8, // mov eax, edi
                0x85, 0xc0, // test eax, eax
                0x0f, 0x84, 9, 0, 0, 0, // jz false arm
                0x48, 0x89, 0xf0, // true arm
                0x25, 0xff, 0, 0, 0, 0xc3, // ret
                0x48, 0x89, 0xd0, // false arm
                0x25, 0xff, 0, 0, 0, 0xc3, // ret
            ];
            function.scalar_stack = Some(ScalarStackEvidence {
                mutations: Vec::new(),
                control_flow: conditional_tree(ScalarConditionalCondition::Parameter, 4, 6, 19),
                stack_alignment: 16,
                cleanup_preservation: None,
            });
        }
        target::Architecture::Aarch64 => {
            function.bytes = aarch64_words(&[
                0x3400_0080, // cbz w0, false arm at byte 16
                0xd100_43ff, // true: sub sp, sp, #16
                0x9100_43ff, // true: add sp, sp, #16
                0xd65f_03c0, // true: ret
                0xd100_83ff, // false: sub sp, sp, #32
                0x9100_83ff, // false: add sp, sp, #32
                0xd65f_03c0, // false: ret
            ]);
            function.scalar_stack = Some(ScalarStackEvidence {
                mutations: vec![
                    scalar_mutation(4, 4, ScalarStackMutationKind::Allocate { byte_size: 16 }),
                    scalar_mutation(8, 4, ScalarStackMutationKind::Release { byte_size: 16 }),
                    scalar_mutation(16, 4, ScalarStackMutationKind::Allocate { byte_size: 32 }),
                    scalar_mutation(20, 4, ScalarStackMutationKind::Release { byte_size: 32 }),
                ],
                control_flow: conditional_tree(ScalarConditionalCondition::Parameter, 0, 4, 16),
                stack_alignment: 16,
                cleanup_preservation: None,
            });
        }
    }
    plan
}

fn scalar_three_leaf_cleanup_plan() -> MachineCodePlan {
    let mut plan = two_function_plan();
    plan.entry = machine_id(1);
    plan.functions.truncate(1);
    let function = &mut plan.functions[0];
    function.provenance.edges = vec![
        edge_id(1),
        edge_id(2),
        edge_id(3),
        edge_id(4),
        edge_id(10),
        edge_id(11),
        edge_id(12),
    ];
    function.bytes = vec![
        0x85, 0xc0, // test eax, eax
        0x0f, 0x84, 0x38, 0, 0, 0, // root jz third leaf at 64
        0x85, 0xc0, // nested test eax, eax
        0x0f, 0x84, 0x18, 0, 0, 0, // nested jz second leaf at 40
        0xb8, 1, 0, 0, 0, // first result
        0x48, 0x83, 0xec, 16, // first preservation allocation
        0x48, 0x89, 0x44, 0x24, 0, // first result store
        0x48, 0x8b, 0x44, 0x24, 0, // first result load
        0x48, 0x83, 0xc4, 16, 0xc3, // first release/return
        0xb8, 0, 0, 0, 0, // second result
        0x48, 0x83, 0xec, 16, 0x48, 0x89, 0x44, 0x24, 0, 0x48, 0x8b, 0x44, 0x24, 0, 0x48, 0x83,
        0xc4, 16, 0xc3, 0xb8, 1, 0, 0, 0, // third result
        0x48, 0x83, 0xec, 16, 0x48, 0x89, 0x44, 0x24, 0, 0x48, 0x8b, 0x44, 0x24, 0, 0x48, 0x83,
        0xc4, 16, 0xc3,
    ];
    let leaf = |edge: u64, cleanup_start: usize, end: usize| ScalarControlAffineCleanupRecord {
        cleanup: UnitAffineCleanupRecord {
            psi_edge: edge_id(edge),
            structural_types: Vec::new().into(),
            locals: Vec::new(),
            actions: vec![TerminalAffineCleanupAction::DiscardRoot(
                PlaceId::new(1).unwrap(),
            )],
            code_offset: cleanup_start,
            byte_count: end - cleanup_start,
        },
        preservation: ScalarCleanupPreservationEvidence {
            frame: StackAdjustmentPair {
                byte_size: 16,
                allocation_offset: cleanup_start,
                allocation_byte_count: 4,
                release_offset: end - 5,
                release_byte_count: 4,
            },
            result_byte_offset: 0,
            result_store_offset: cleanup_start + 4,
            result_load_offset: end - 10,
            aarch64_return_link: None,
        },
    };
    function.scalar_control_affine_cleanups =
        vec![leaf(10, 21, 40), leaf(11, 45, 64), leaf(12, 69, 88)];
    function.scalar_stack = Some(ScalarStackEvidence {
        mutations: [21, 45, 69]
            .into_iter()
            .flat_map(|start| {
                [
                    scalar_mutation(
                        start,
                        4,
                        ScalarStackMutationKind::Allocate { byte_size: 16 },
                    ),
                    scalar_mutation(
                        start + 14,
                        4,
                        ScalarStackMutationKind::Release { byte_size: 16 },
                    ),
                ]
            })
            .collect(),
        control_flow: ScalarControlFlowEvidence::ConditionalTree {
            decisions: vec![
                ScalarConditionalBranchEvidence {
                    condition: ScalarConditionalCondition::Parameter,
                    branch_offset: 2,
                    branch_byte_count: 6,
                    false_arm_offset: 64,
                },
                ScalarConditionalBranchEvidence {
                    condition: ScalarConditionalCondition::Parameter,
                    branch_offset: 10,
                    branch_byte_count: 6,
                    false_arm_offset: 40,
                },
            ],
            crash_leaves: vec![false; 3],
            branches: Vec::new(),
        },
        stack_alignment: 16,
        cleanup_preservation: None,
    });
    function.scalar_structural_parameters = vec![UnitParameterRecord {
        place: PlaceId::new(1).unwrap(),
        structural_type: StructuralTypeId::new(1).unwrap(),
        multiplicity: StructuralMultiplicity::Affine,
        access: terminal_psi::StructuralAccess::Owned,
        shape: ValueShape::integer(0, 1),
    }];
    function.scalar_structural_parameter_homes = vec![UnitParameterHomeRecord {
        place: PlaceId::new(1).unwrap(),
        structural_type: StructuralTypeId::new(1).unwrap(),
        multiplicity: StructuralMultiplicity::Affine,
        access: terminal_psi::StructuralAccess::Owned,
        shape: ValueShape::integer(0, 1),
        source: ValuePlacement {
            shape: ValueShape::integer(0, 1),
            locations: Vec::new(),
        },
        location: machine_code::StructuralSourceLocation::Stack { byte_offset: 0 },
        indirect: false,
    }];
    function.semantic_code_attribution = [(10, 21, 19), (11, 45, 19), (12, 69, 19)]
        .into_iter()
        .enumerate()
        .map(
            |(ordinal, (edge, code_offset, byte_count))| SemanticCodeAttribution {
                site: SemanticCodeSite::Edge(edge_id(edge)),
                operation_ordinal: ordinal,
                code_offset,
                byte_count,
            },
        )
        .collect();
    plan
}

fn scalar_expression_two_return_conditional_plan(target: NativeTarget) -> MachineCodePlan {
    let mut plan = two_function_plan();
    plan.target = target;
    plan.entry = machine_id(1);
    plan.functions.truncate(1);
    let function = &mut plan.functions[0];
    match target.architecture {
        target::Architecture::X86_64 => {
            function.bytes = vec![
                0x48, 0x83, 0xec, 16, // sub rsp, 16
                0x85, 0xc0, // test eax, eax
                0x48, 0x8d, 0x64, 0x24, 16, // lea rsp, [rsp + 16]
                0x0f, 0x84, 1, 0, 0, 0,    // jz false arm
                0xc3, // true: ret
                0xc3, // false: ret
            ];
            function.scalar_stack = Some(ScalarStackEvidence {
                mutations: vec![
                    scalar_mutation(0, 4, ScalarStackMutationKind::Allocate { byte_size: 16 }),
                    scalar_mutation(
                        6,
                        5,
                        ScalarStackMutationKind::X86ReleasePreservingFlags { byte_size: 16 },
                    ),
                ],
                control_flow: conditional_tree(ScalarConditionalCondition::Expression, 11, 6, 18),
                stack_alignment: 16,
                cleanup_preservation: None,
            });
        }
        target::Architecture::Aarch64 => {
            function.bytes = aarch64_words(&[
                0xd100_43ff, // sub sp, sp, #16
                0x7100_001f, // cmp w0, #0
                0x9100_43ff, // add sp, sp, #16
                0x5400_0040, // b.eq false arm at byte 20
                0xd65f_03c0, // true: ret
                0xd65f_03c0, // false: ret
            ]);
            function.scalar_stack = Some(ScalarStackEvidence {
                mutations: vec![
                    scalar_mutation(0, 4, ScalarStackMutationKind::Allocate { byte_size: 16 }),
                    scalar_mutation(8, 4, ScalarStackMutationKind::Release { byte_size: 16 }),
                ],
                control_flow: conditional_tree(ScalarConditionalCondition::Expression, 12, 4, 20),
                stack_alignment: 16,
                cleanup_preservation: None,
            });
        }
    }
    plan
}

fn scalar_expression_condition_call_plan(target: NativeTarget) -> MachineCodePlan {
    let mut plan = internal_call_plan(target);
    plan.functions[0].bytes = match target.architecture {
        target::Architecture::X86_64 => vec![0xc3],
        target::Architecture::Aarch64 => aarch64_words(&[0xd65f_03c0]),
    };
    plan.functions[0].scalar_stack = Some(ScalarStackEvidence {
        mutations: Vec::new(),
        control_flow: ScalarControlFlowEvidence::Linear,
        stack_alignment: 16,
        cleanup_preservation: None,
    });
    let caller = &mut plan.functions[1];
    caller.provenance.operations = vec![operation_id(2)];
    match target.architecture {
        target::Architecture::X86_64 => {
            caller.bytes = vec![
                0x48, 0x83, 0xec, 8, // outbound call area
                0xe8, 0, 0, 0, 0, // typed condition call
                0x48, 0x83, 0xc4, 8, // release call area
                0x85, 0xc0, // test returned Boolean
                0x0f, 0x84, 1, 0, 0, 0,    // jz false arm
                0xc3, // true: ret
                0xc3, // false: ret
            ];
            caller.scalar_stack = Some(ScalarStackEvidence {
                mutations: vec![
                    scalar_mutation(0, 4, ScalarStackMutationKind::Allocate { byte_size: 8 }),
                    scalar_mutation(9, 4, ScalarStackMutationKind::Release { byte_size: 8 }),
                ],
                control_flow: conditional_tree(ScalarConditionalCondition::Expression, 15, 6, 22),
                stack_alignment: 16,
                cleanup_preservation: None,
            });
            caller.internal_calls = vec![InternalCallRelocation {
                owner: target_operations::CallSiteOwner::Operation(operation_id(2)),
                target: machine_id(1),
                unit_stack: None,
                scalar_stack: Some(ScalarCallStackEvidence {
                    outbound: Some(StackAdjustmentPair {
                        byte_size: 8,
                        allocation_offset: 0,
                        allocation_byte_count: 4,
                        release_offset: 9,
                        release_byte_count: 4,
                    }),
                    aarch64_return_link: None,
                }),
                offset: 5,
            }];
        }
        target::Architecture::Aarch64 => {
            caller.bytes = aarch64_words(&[
                0xd100_43ff, // outbound call area
                0xf900_03fe, // save x30
                0x9400_0000, // typed condition call
                0xf940_03fe, // restore x30
                0x9100_43ff, // release call area
                0x7100_001f, // cmp w0, #0
                0x5400_0040, // b.eq false arm at byte 32
                0xd65f_03c0, // true: ret
                0xd65f_03c0, // false: ret
            ]);
            caller.scalar_stack = Some(ScalarStackEvidence {
                mutations: vec![
                    scalar_mutation(0, 4, ScalarStackMutationKind::Allocate { byte_size: 16 }),
                    scalar_mutation(16, 4, ScalarStackMutationKind::Release { byte_size: 16 }),
                ],
                control_flow: conditional_tree(ScalarConditionalCondition::Expression, 24, 4, 32),
                stack_alignment: 16,
                cleanup_preservation: None,
            });
            caller.internal_calls = vec![InternalCallRelocation {
                owner: target_operations::CallSiteOwner::Operation(operation_id(2)),
                target: machine_id(1),
                unit_stack: None,
                scalar_stack: Some(ScalarCallStackEvidence {
                    outbound: Some(StackAdjustmentPair {
                        byte_size: 16,
                        allocation_offset: 0,
                        allocation_byte_count: 4,
                        release_offset: 16,
                        release_byte_count: 4,
                    }),
                    aarch64_return_link: Some(Aarch64ReturnLinkEvidence {
                        frame_byte_offset: 0,
                        store_offset: 4,
                        load_offset: 12,
                    }),
                }),
                offset: 8,
            }];
        }
    }
    plan
}

fn scalar_conditional_call_plan(target: NativeTarget) -> MachineCodePlan {
    let mut plan = internal_call_plan(target);
    plan.functions[0].bytes = match target.architecture {
        target::Architecture::X86_64 => vec![0xc3],
        target::Architecture::Aarch64 => aarch64_words(&[0xd65f_03c0]),
    };
    plan.functions[0].scalar_stack = Some(ScalarStackEvidence {
        mutations: Vec::new(),
        control_flow: ScalarControlFlowEvidence::Linear,
        stack_alignment: 16,
        cleanup_preservation: None,
    });
    let caller = &mut plan.functions[1];
    caller.provenance.operations = vec![operation_id(2), operation_id(3)];
    match target.architecture {
        target::Architecture::X86_64 => {
            caller.bytes = vec![
                0x89, 0xf8, // mov eax, edi
                0x85, 0xc0, // test eax, eax
                0x0f, 0x84, 8, 0, 0, 0,    // jz false arm at byte 18
                0x50, // true: pending temporary
                0xe8, 0, 0, 0, 0,    // true: call
                0x58, // true: restore temporary
                0xc3, // true: ret
                0x48, 0x83, 0xec, 8, // false: outbound allocation
                0xe8, 0, 0, 0, 0, // false: call
                0x48, 0x83, 0xc4, 8,    // false: release
                0xc3, // false: ret
            ];
            caller.scalar_stack = Some(ScalarStackEvidence {
                mutations: vec![
                    scalar_mutation(10, 1, ScalarStackMutationKind::X86Push),
                    scalar_mutation(16, 1, ScalarStackMutationKind::X86Pop),
                    scalar_mutation(18, 4, ScalarStackMutationKind::Allocate { byte_size: 8 }),
                    scalar_mutation(27, 4, ScalarStackMutationKind::Release { byte_size: 8 }),
                ],
                control_flow: conditional_tree(ScalarConditionalCondition::Parameter, 4, 6, 18),
                stack_alignment: 16,
                cleanup_preservation: None,
            });
            caller.internal_calls = vec![
                InternalCallRelocation {
                    owner: target_operations::CallSiteOwner::Operation(operation_id(2)),
                    target: machine_id(1),
                    unit_stack: None,
                    scalar_stack: Some(ScalarCallStackEvidence {
                        outbound: None,
                        aarch64_return_link: None,
                    }),
                    offset: 12,
                },
                InternalCallRelocation {
                    owner: target_operations::CallSiteOwner::Operation(operation_id(3)),
                    target: machine_id(1),
                    unit_stack: None,
                    scalar_stack: Some(ScalarCallStackEvidence {
                        outbound: Some(StackAdjustmentPair {
                            byte_size: 8,
                            allocation_offset: 18,
                            allocation_byte_count: 4,
                            release_offset: 27,
                            release_byte_count: 4,
                        }),
                        aarch64_return_link: None,
                    }),
                    offset: 23,
                },
            ];
        }
        target::Architecture::Aarch64 => {
            caller.bytes = aarch64_words(&[
                0x3400_0120, // cbz w0, false arm at byte 36
                0xd100_43ff, // true: pending frame
                0xd100_43ff, // true: call area
                0xf900_03fe, // true: save x30
                0x9400_0000, // true: call
                0xf940_03fe, // true: restore x30
                0x9100_43ff, // true: release call area
                0x9100_43ff, // true: release pending frame
                0xd65f_03c0, // true: ret
                0xd100_43ff, // false: call area
                0xf900_03fe, // false: save x30
                0x9400_0000, // false: call
                0xf940_03fe, // false: restore x30
                0x9100_43ff, // false: release call area
                0xd65f_03c0, // false: ret
            ]);
            caller.scalar_stack = Some(ScalarStackEvidence {
                mutations: vec![
                    scalar_mutation(4, 4, ScalarStackMutationKind::Allocate { byte_size: 16 }),
                    scalar_mutation(8, 4, ScalarStackMutationKind::Allocate { byte_size: 16 }),
                    scalar_mutation(24, 4, ScalarStackMutationKind::Release { byte_size: 16 }),
                    scalar_mutation(28, 4, ScalarStackMutationKind::Release { byte_size: 16 }),
                    scalar_mutation(36, 4, ScalarStackMutationKind::Allocate { byte_size: 16 }),
                    scalar_mutation(52, 4, ScalarStackMutationKind::Release { byte_size: 16 }),
                ],
                control_flow: conditional_tree(ScalarConditionalCondition::Parameter, 0, 4, 36),
                stack_alignment: 16,
                cleanup_preservation: None,
            });
            caller.internal_calls = vec![
                InternalCallRelocation {
                    owner: target_operations::CallSiteOwner::Operation(operation_id(2)),
                    target: machine_id(1),
                    unit_stack: None,
                    scalar_stack: Some(ScalarCallStackEvidence {
                        outbound: Some(StackAdjustmentPair {
                            byte_size: 16,
                            allocation_offset: 8,
                            allocation_byte_count: 4,
                            release_offset: 24,
                            release_byte_count: 4,
                        }),
                        aarch64_return_link: Some(Aarch64ReturnLinkEvidence {
                            frame_byte_offset: 0,
                            store_offset: 12,
                            load_offset: 20,
                        }),
                    }),
                    offset: 16,
                },
                InternalCallRelocation {
                    owner: target_operations::CallSiteOwner::Operation(operation_id(3)),
                    target: machine_id(1),
                    unit_stack: None,
                    scalar_stack: Some(ScalarCallStackEvidence {
                        outbound: Some(StackAdjustmentPair {
                            byte_size: 16,
                            allocation_offset: 36,
                            allocation_byte_count: 4,
                            release_offset: 52,
                            release_byte_count: 4,
                        }),
                        aarch64_return_link: Some(Aarch64ReturnLinkEvidence {
                            frame_byte_offset: 0,
                            store_offset: 40,
                            load_offset: 48,
                        }),
                    }),
                    offset: 44,
                },
            ];
        }
    }
    plan
}

fn scalar_call_plan(target: NativeTarget) -> MachineCodePlan {
    let mut plan = internal_call_plan(target);
    plan.functions[0].bytes = match target.architecture {
        target::Architecture::X86_64 => vec![0xc3],
        target::Architecture::Aarch64 => aarch64_words(&[0xd65f_03c0]),
    };
    plan.functions[0].scalar_stack = Some(ScalarStackEvidence {
        mutations: Vec::new(),
        control_flow: ScalarControlFlowEvidence::Linear,
        stack_alignment: 16,
        cleanup_preservation: None,
    });
    let caller = &mut plan.functions[1];
    match target.architecture {
        target::Architecture::X86_64 => {
            caller.bytes = vec![
                0x50, // pending expression temporary
                0xe8, 0, 0, 0, 0,    // call rel32
                0x58, // restore expression temporary
                0xc3, // ret
            ];
            caller.scalar_stack = Some(ScalarStackEvidence {
                mutations: vec![
                    scalar_mutation(0, 1, ScalarStackMutationKind::X86Push),
                    scalar_mutation(6, 1, ScalarStackMutationKind::X86Pop),
                ],
                control_flow: ScalarControlFlowEvidence::Linear,
                stack_alignment: 16,
                cleanup_preservation: None,
            });
            caller.internal_calls[0].offset = 2;
            caller.internal_calls[0].scalar_stack = Some(ScalarCallStackEvidence {
                outbound: None,
                aarch64_return_link: None,
            });
        }
        target::Architecture::Aarch64 => {
            caller.bytes = aarch64_words(&[
                0xd100_43ff, // pending expression frame: sub sp, sp, #16
                0xd100_43ff, // call area: sub sp, sp, #16
                0xf900_03fe, // str x30, [sp]
                0x9400_0000, // bl #0
                0xf940_03fe, // ldr x30, [sp]
                0x9100_43ff, // release call area
                0x9100_43ff, // release expression frame
                0xd65f_03c0, // ret
            ]);
            caller.scalar_stack = Some(ScalarStackEvidence {
                mutations: vec![
                    scalar_mutation(0, 4, ScalarStackMutationKind::Allocate { byte_size: 16 }),
                    scalar_mutation(4, 4, ScalarStackMutationKind::Allocate { byte_size: 16 }),
                    scalar_mutation(20, 4, ScalarStackMutationKind::Release { byte_size: 16 }),
                    scalar_mutation(24, 4, ScalarStackMutationKind::Release { byte_size: 16 }),
                ],
                control_flow: ScalarControlFlowEvidence::Linear,
                stack_alignment: 16,
                cleanup_preservation: None,
            });
            caller.internal_calls[0].offset = 12;
            caller.internal_calls[0].scalar_stack = Some(ScalarCallStackEvidence {
                outbound: Some(StackAdjustmentPair {
                    byte_size: 16,
                    allocation_offset: 4,
                    allocation_byte_count: 4,
                    release_offset: 20,
                    release_byte_count: 4,
                }),
                aarch64_return_link: Some(Aarch64ReturnLinkEvidence {
                    frame_byte_offset: 0,
                    store_offset: 8,
                    load_offset: 16,
                }),
            });
        }
    }
    plan
}

fn account_x86_unit_call(plan: &mut MachineCodePlan) {
    let caller = &mut plan.functions[1];
    caller.bytes = vec![
        0x48, 0x83, 0xec, 0x08, // sub rsp, 8
        0xe8, 0, 0, 0, 0, // call rel32
        0x48, 0x83, 0xc4, 0x08, // add rsp, 8
        0xc3, // ret
    ];
    caller.unit_stack = Some(UnitStackEvidence {
        frame: None,
        aarch64_return_link: None,
        stack_alignment: 16,
    });
    add_empty_unit_cleanup(caller);
    caller.internal_calls[0].offset = 5;
    caller.internal_calls[0].unit_stack = Some(UnitCallStackEvidence {
        outbound: Some(StackAdjustmentPair {
            byte_size: 8,
            allocation_offset: 0,
            allocation_byte_count: 4,
            release_offset: 9,
            release_byte_count: 4,
        }),
    });
    caller.internal_unit_calls = vec![InternalUnitCallRecord {
        source: machine_code::InternalUnitCallSource::Authored,
        owner: caller.internal_calls[0].owner,
        target: caller.internal_calls[0].target,
        result: None,
        semantic_result: None,
        structural_result: None,
        scalar_arguments: Vec::new(),
        arguments: Vec::new(),
        claim_transfers: Vec::new(),
        operation_ordinal: 0,
        code_offset: 0,
        byte_count: 13,
    }];
    caller.semantic_code_attribution = vec![
        SemanticCodeAttribution {
            site: SemanticCodeSite::Operation(
                caller.internal_calls[0]
                    .owner
                    .operation()
                    .expect("ordinary call owner"),
            ),
            operation_ordinal: 0,
            code_offset: 0,
            byte_count: 13,
        },
        SemanticCodeAttribution {
            site: SemanticCodeSite::Edge(caller.provenance.edges[0]),
            operation_ordinal: 1,
            code_offset: 13,
            byte_count: 1,
        },
    ];
}

fn scalar_mutation(
    offset: usize,
    byte_count: usize,
    kind: ScalarStackMutationKind,
) -> ScalarStackMutation {
    ScalarStackMutation {
        offset,
        byte_count,
        kind,
    }
}

fn conditional_tree(
    condition: ScalarConditionalCondition,
    branch_offset: usize,
    branch_byte_count: usize,
    false_arm_offset: usize,
) -> ScalarControlFlowEvidence {
    ScalarControlFlowEvidence::ConditionalTree {
        decisions: vec![ScalarConditionalBranchEvidence {
            condition,
            branch_offset,
            branch_byte_count,
            false_arm_offset,
        }],
        crash_leaves: vec![false; 2],
        branches: Vec::new(),
    }
}

fn promote_x86_cleanup_to_scalar(caller: &mut MachineCodeFunction) {
    let prefix_len = 5;
    caller.bytes.splice(0..0, [0xb8, 1, 0, 0, 0]);
    let cleanup_start = prefix_len;
    caller.bytes.splice(
        cleanup_start..cleanup_start,
        [
            0x48, 0x83, 0xec, 16, // sub rsp, 16
            0x48, 0x89, 0x44, 0x24, 0, // mov [rsp], rax
        ],
    );
    let inserted_prefix = prefix_len + 9;
    let relocation = &mut caller.internal_calls[0];
    relocation.offset += inserted_prefix;
    let outbound = relocation
        .unit_stack
        .take()
        .and_then(|stack| stack.outbound)
        .expect("x86 cleanup call stack pair");
    let outbound = StackAdjustmentPair {
        allocation_offset: outbound.allocation_offset + inserted_prefix,
        release_offset: outbound.release_offset + inserted_prefix,
        ..outbound
    };
    relocation.scalar_stack = Some(ScalarCallStackEvidence {
        outbound: Some(outbound),
        aarch64_return_link: None,
    });
    caller.internal_unit_calls[0].code_offset += inserted_prefix;
    let original_ret = caller.bytes.pop();
    assert_eq!(original_ret, Some(0xc3));
    let result_load_offset = caller.bytes.len();
    caller.bytes.extend_from_slice(&[
        0x48, 0x8b, 0x44, 0x24, 0, // mov rax, [rsp]
        0x48, 0x83, 0xc4, 16, // add rsp, 16
        0xc3,
    ]);
    let frame_release_offset = result_load_offset + 5;
    caller.unit_stack = None;
    caller.scalar_stack = Some(ScalarStackEvidence {
        mutations: vec![
            scalar_mutation(
                cleanup_start,
                4,
                ScalarStackMutationKind::Allocate { byte_size: 16 },
            ),
            scalar_mutation(
                outbound.allocation_offset,
                outbound.allocation_byte_count,
                ScalarStackMutationKind::Allocate {
                    byte_size: outbound.byte_size,
                },
            ),
            scalar_mutation(
                outbound.release_offset,
                outbound.release_byte_count,
                ScalarStackMutationKind::Release {
                    byte_size: outbound.byte_size,
                },
            ),
            scalar_mutation(
                frame_release_offset,
                4,
                ScalarStackMutationKind::Release { byte_size: 16 },
            ),
        ],
        control_flow: ScalarControlFlowEvidence::Linear,
        stack_alignment: 16,
        cleanup_preservation: Some(ScalarCleanupPreservationEvidence {
            frame: StackAdjustmentPair {
                byte_size: 16,
                allocation_offset: cleanup_start,
                allocation_byte_count: 4,
                release_offset: frame_release_offset,
                release_byte_count: 4,
            },
            result_byte_offset: 0,
            result_store_offset: cleanup_start + 4,
            result_load_offset,
            aarch64_return_link: None,
        }),
    });
    let cleanup = caller
        .unit_affine_cleanup
        .take()
        .expect("Unit cleanup fixture");
    caller.scalar_affine_cleanup = Some(UnitAffineCleanupRecord {
        code_offset: cleanup_start,
        byte_count: caller.bytes.len() - cleanup_start,
        ..cleanup
    });
    caller.scalar_structural_parameters = std::mem::take(&mut caller.unit_parameters);
    caller.scalar_structural_parameter_homes = std::mem::take(&mut caller.unit_parameter_homes);
    caller.semantic_code_attribution[0].code_offset += inserted_prefix;
    caller.semantic_code_attribution[0].byte_count += 9;
    let cleanup_fuel = caller
        .semantic_code_attribution
        .last_mut()
        .expect("cleanup edge fuel");
    cleanup_fuel.code_offset = cleanup_start;
    cleanup_fuel.byte_count = caller.bytes.len() - cleanup_start;
}

fn account_aarch64_unit_call(plan: &mut MachineCodePlan) {
    let frame = StackAdjustmentPair {
        byte_size: 16,
        allocation_offset: 0,
        allocation_byte_count: 4,
        release_offset: 12,
        release_byte_count: 4,
    };
    let link = Aarch64ReturnLinkEvidence {
        frame_byte_offset: 0,
        store_offset: 4,
        load_offset: 8,
    };
    plan.functions[0].bytes = aarch64_words(&[
        0xd100_43ff, // sub sp, sp, #16
        0xf900_03fe, // str x30, [sp]
        0xf940_03fe, // ldr x30, [sp]
        0x9100_43ff, // add sp, sp, #16
        0xd65f_03c0, // ret
    ]);
    plan.functions[0].unit_stack = Some(UnitStackEvidence {
        frame: Some(frame),
        aarch64_return_link: Some(link),
        stack_alignment: 16,
    });
    add_empty_unit_cleanup(&mut plan.functions[0]);

    let caller = &mut plan.functions[1];
    caller.bytes = aarch64_words(&[
        0xd100_43ff, // sub sp, sp, #16
        0xf900_03fe, // str x30, [sp]
        0x9400_0000, // bl immediate
        0xf940_03fe, // ldr x30, [sp]
        0x9100_43ff, // add sp, sp, #16
        0xd65f_03c0, // ret
    ]);
    caller.unit_stack = Some(UnitStackEvidence {
        frame: Some(StackAdjustmentPair {
            release_offset: 16,
            ..frame
        }),
        aarch64_return_link: Some(Aarch64ReturnLinkEvidence {
            load_offset: 12,
            ..link
        }),
        stack_alignment: 16,
    });
    add_empty_unit_cleanup(caller);
    caller.internal_calls[0].offset = 8;
    caller.internal_calls[0].unit_stack = Some(UnitCallStackEvidence { outbound: None });
    caller.internal_unit_calls = vec![InternalUnitCallRecord {
        source: machine_code::InternalUnitCallSource::Authored,
        owner: caller.internal_calls[0].owner,
        target: caller.internal_calls[0].target,
        result: None,
        semantic_result: None,
        structural_result: None,
        scalar_arguments: Vec::new(),
        arguments: Vec::new(),
        claim_transfers: Vec::new(),
        operation_ordinal: 0,
        code_offset: 8,
        byte_count: 4,
    }];
    caller.semantic_code_attribution = vec![
        SemanticCodeAttribution {
            site: SemanticCodeSite::Operation(
                caller.internal_calls[0]
                    .owner
                    .operation()
                    .expect("ordinary call owner"),
            ),
            operation_ordinal: 0,
            code_offset: 8,
            byte_count: 4,
        },
        SemanticCodeAttribution {
            site: SemanticCodeSite::Edge(caller.provenance.edges[0]),
            operation_ordinal: 1,
            code_offset: 12,
            byte_count: 12,
        },
    ];
}

fn aarch64_words(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

fn insert_aarch64_word(bytes: &mut Vec<u8>, offset: usize, word: u32) {
    bytes.splice(offset..offset, word.to_le_bytes());
}

fn integer_return(value: u8) -> Vec<u8> {
    vec![0xb8, value, 0, 0, 0, 0xc3]
}

fn edge_owned_cleanup_plan() -> MachineCodePlan {
    let structural_type = StructuralTypeId::new(1).expect("type");
    let place = PlaceId::new(1).expect("place");
    let empty_shape = ValueShape::integer(0, 1);
    let empty_placement = ValuePlacement {
        shape: empty_shape,
        locations: Vec::new(),
    };
    let unit_stack = Some(UnitStackEvidence {
        frame: None,
        aarch64_return_link: None,
        stack_alignment: 16,
    });
    let empty_return = |edge| UnitAffineCleanupRecord {
        structural_types: Vec::new().into(),
        psi_edge: edge,
        locals: Vec::new(),
        actions: Vec::new(),
        code_offset: 13,
        byte_count: 1,
    };
    let x86_empty_call_bytes = vec![
        0x48, 0x83, 0xec, 0x08, 0xe8, 0, 0, 0, 0, 0x48, 0x83, 0xc4, 0x08, 0xc3,
    ];
    let stack_pair = StackAdjustmentPair {
        byte_size: 8,
        allocation_offset: 0,
        allocation_byte_count: 4,
        release_offset: 9,
        release_byte_count: 4,
    };
    let operation_call = |operation, target| InternalCallRelocation {
        owner: CallSiteOwner::Operation(operation),
        target,
        unit_stack: Some(UnitCallStackEvidence {
            outbound: Some(stack_pair),
        }),
        scalar_stack: None,
        offset: 5,
    };
    let operation_custody = |operation, target| InternalUnitCallRecord {
        source: machine_code::InternalUnitCallSource::Authored,
        owner: CallSiteOwner::Operation(operation),
        target,
        result: None,
        semantic_result: None,
        structural_result: None,
        scalar_arguments: Vec::new(),
        arguments: Vec::new(),
        claim_transfers: Vec::new(),
        operation_ordinal: 0,
        code_offset: 0,
        byte_count: 13,
    };
    MachineCodePlan {
        psi: identity(),
        target: NativeTarget::linux_x64(),
        entry: machine_id(3),
        functions: vec![
            MachineCodeFunction {
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
                machine: machine_id(1),
                attachment: Some(structural_type),
                provenance: TerminalPsiProvenance {
                    operations: vec![operation_id(1)],
                    edges: vec![edge_id(1)],
                },
                bytes: x86_empty_call_bytes.clone(),
                x86_scalar_fma: Vec::new(),
                x86_scalar_fma_occurrences: Vec::new(),
                x86_floating_control: None,
                unit_stack,
                unit_parameter_homes: Vec::new(),
                unit_parameters: Vec::new(),
                scalar_stack: None,
                internal_calls: vec![operation_call(operation_id(1), machine_id(2))],
                foreign_calls: Vec::new(),
                internal_unit_calls: vec![operation_custody(operation_id(1), machine_id(2))],
                unit_continuations: Vec::new(),
                unit_affine_cleanup: Some(empty_return(edge_id(1))),
                semantic_code_attribution: vec![
                    SemanticCodeAttribution {
                        site: SemanticCodeSite::Operation(operation_id(1)),
                        operation_ordinal: 0,
                        code_offset: 0,
                        byte_count: 13,
                    },
                    SemanticCodeAttribution {
                        site: SemanticCodeSite::Edge(edge_id(1)),
                        operation_ordinal: 1,
                        code_offset: 13,
                        byte_count: 1,
                    },
                ],
                port_effects: Vec::new(),
                boundary_settlements: Vec::new(),
                scalar_affine_cleanup: None,
                scalar_control_affine_cleanups: Vec::new(),
                scalar_structural_parameters: Vec::new(),
                scalar_structural_parameter_homes: Vec::new(),
                structural_return: None,
            },
            MachineCodeFunction {
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
                machine: machine_id(2),
                attachment: Some(StructuralTypeId::new(2).expect("helper type")),
                provenance: TerminalPsiProvenance {
                    operations: Vec::new(),
                    edges: vec![edge_id(2)],
                },
                bytes: vec![0xc3],
                x86_scalar_fma: Vec::new(),
                x86_scalar_fma_occurrences: Vec::new(),
                x86_floating_control: None,
                unit_stack,
                unit_parameter_homes: Vec::new(),
                unit_parameters: Vec::new(),
                scalar_stack: None,
                internal_calls: Vec::new(),
                foreign_calls: Vec::new(),
                internal_unit_calls: Vec::new(),
                unit_continuations: Vec::new(),
                unit_affine_cleanup: Some(UnitAffineCleanupRecord {
                    structural_types: Vec::new().into(),
                    code_offset: 0,
                    byte_count: 1,
                    ..empty_return(edge_id(2))
                }),
                semantic_code_attribution: vec![SemanticCodeAttribution {
                    site: SemanticCodeSite::Edge(edge_id(2)),
                    operation_ordinal: 0,
                    code_offset: 0,
                    byte_count: 1,
                }],
                port_effects: Vec::new(),
                boundary_settlements: Vec::new(),
                scalar_affine_cleanup: None,
                scalar_control_affine_cleanups: Vec::new(),
                scalar_structural_parameters: Vec::new(),
                scalar_structural_parameter_homes: Vec::new(),
                structural_return: None,
            },
            MachineCodeFunction {
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
                machine: machine_id(3),
                attachment: None,
                provenance: TerminalPsiProvenance {
                    operations: Vec::new(),
                    edges: vec![edge_id(3)],
                },
                bytes: x86_empty_call_bytes,
                x86_scalar_fma: Vec::new(),
                x86_scalar_fma_occurrences: Vec::new(),
                x86_floating_control: None,
                unit_stack,
                unit_parameter_homes: vec![UnitParameterHomeRecord {
                    place,
                    structural_type,
                    multiplicity: StructuralMultiplicity::Affine,
                    access: terminal_psi::StructuralAccess::Owned,
                    shape: empty_shape,
                    source: empty_placement.clone(),
                    location: machine_code::StructuralSourceLocation::Stack { byte_offset: 0 },
                    indirect: false,
                }],
                unit_parameters: vec![UnitParameterRecord {
                    place,
                    structural_type,
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    shape: empty_shape,
                }],
                scalar_stack: None,
                internal_calls: vec![InternalCallRelocation {
                    owner: CallSiteOwner::CleanupAction {
                        edge: edge_id(3),
                        action_ordinal: 0,
                    },
                    target: machine_id(1),
                    unit_stack: Some(UnitCallStackEvidence {
                        outbound: Some(stack_pair),
                    }),
                    scalar_stack: None,
                    offset: 5,
                }],
                foreign_calls: Vec::new(),
                internal_unit_calls: vec![InternalUnitCallRecord {
                    source: machine_code::InternalUnitCallSource::Authored,
                    owner: CallSiteOwner::CleanupAction {
                        edge: edge_id(3),
                        action_ordinal: 0,
                    },
                    target: machine_id(1),
                    result: None,
                    semantic_result: None,
                    structural_result: None,
                    scalar_arguments: Vec::new(),
                    arguments: Vec::new(),
                    claim_transfers: Vec::new(),
                    operation_ordinal: 0,
                    code_offset: 0,
                    byte_count: 13,
                }],
                unit_continuations: Vec::new(),
                unit_affine_cleanup: Some(UnitAffineCleanupRecord {
                    structural_types: Vec::new().into(),
                    psi_edge: edge_id(3),
                    locals: Vec::new(),
                    actions: vec![TerminalAffineCleanupAction::InvokeNominal(
                        NominalAffineCleanup {
                            place,
                            structural_type,
                            cleanup_machine: machine_id(1),
                            cleanup_receiver: None,
                            requirement_obligations: Vec::new(),
                        },
                    )],
                    code_offset: 0,
                    byte_count: 14,
                }),
                semantic_code_attribution: vec![SemanticCodeAttribution {
                    site: SemanticCodeSite::Edge(edge_id(3)),
                    operation_ordinal: 0,
                    code_offset: 0,
                    byte_count: 14,
                }],
                port_effects: Vec::new(),
                boundary_settlements: Vec::new(),
                scalar_affine_cleanup: None,
                scalar_control_affine_cleanups: Vec::new(),
                scalar_structural_parameters: Vec::new(),
                scalar_structural_parameter_homes: Vec::new(),
                structural_return: None,
            },
        ],
    }
}

/// The edge-owned cleanup caller extended with one byte-exact rebound dynamic
/// call so object construction materializes an ordinary dynamic conformance
/// table with one resolved and one unresolved slot.
fn dynamic_conformance_table_plan() -> MachineCodePlan {
    let mut plan = edge_owned_cleanup_plan();
    let owner = machine_id(3);
    let operation = operation_id(4);
    let place = PlaceId::new(1).expect("dynamic source place");
    let structural_type = StructuralTypeId::new(1).expect("structural type");
    let empty_shape = calling_conventions::ValueShape::integer(0, 1);
    let empty_placement = calling_conventions::ValuePlacement {
        shape: empty_shape,
        locations: Vec::new(),
    };
    // The two-word descriptor travels as one indirect pointer argument.
    let descriptor_placement = calling_conventions::ValuePlacement {
        shape: calling_conventions::ValueShape::integer(16, 8),
        locations: vec![calling_conventions::ValueLocation::Indirect {
            pointer: calling_conventions::IndirectPointerLocation::Register(
                calling_conventions::MachineRegister::X86Rdi,
            ),
            copy_stack_byte_offset: None,
            byte_size: 16,
            alignment: 8,
        }],
    };
    let mut application = terminal_psi::ClosedConformanceApplication {
        owner,
        declaration_identity: "closed.decl".to_string(),
        telescope: Vec::new(),
        subject_identity: None,
        trait_identity: "closed.trait".to_string(),
        trait_lifetime_arguments: Vec::new(),
        trait_arguments: Vec::new(),
        realization_callables: vec![terminal_psi::ClosedConformanceRealizationCallable {
            source_callable_identity: "closed.callable.a".to_string(),
            machine: machine_id(2),
            result: terminal_psi::ClosedConformanceCallableResult::Unit,
        }],
        rows: vec![
            terminal_psi::ClosedConformanceRow {
                declaring_trait_identity: "closed.trait".to_string(),
                public_requirement_identity: "closed.req.a".to_string(),
                requirement_identity: "closed.req.a.impl".to_string(),
                realization_identity: "closed.real.a".to_string(),
                realization_callable_identity: Some("closed.callable.a".to_string()),
            },
            terminal_psi::ClosedConformanceRow {
                declaring_trait_identity: "closed.trait".to_string(),
                public_requirement_identity: "closed.req.b".to_string(),
                requirement_identity: "closed.req.b.impl".to_string(),
                realization_identity: "closed.real.b".to_string(),
                realization_callable_identity: None,
            },
        ],
        report_fingerprint: 0,
        commitment: terminal_psi::ClosedConformanceApplicationCommitment::default(),
    };
    application.report_fingerprint =
        terminal_psi::closed_conformance_application_report_fingerprint(&application);
    application.commitment = terminal_psi::closed_conformance_application_commitment(&application);
    assert_ne!(application.report_fingerprint, 0);
    assert!(!application.commitment.is_zero());
    let selection_source = terminal_psi::StructuralArgument {
        place,
        path: Vec::new(),
        access: StructuralAccess::SharedBorrow,
    };
    let initial = terminal_psi::TerminalDynamicConformanceSelection {
        owner,
        ordinal: 0,
        source: selection_source.clone(),
        conformance_application_report_fingerprint: application.report_fingerprint,
        conformance_application_commitment: application.commitment,
    };
    let rebound = terminal_psi::TerminalDynamicConformanceSelection {
        ordinal: 1,
        ..initial.clone()
    };
    let instance_source = target_operations::TargetStructuralArgument {
        place,
        access: StructuralAccess::SharedBorrow,
        path: Vec::new(),
        root_structural_type: structural_type,
        structural_type,
        shape: empty_shape,
        source_byte_offset: 0,
        fixed_array_length: None,
        element_stride: None,
        source: target_operations::TargetStructuralArgumentSource::Placement(
            empty_placement.clone(),
        ),
        destination: descriptor_placement.clone(),
    };
    let dynamic_call = machine_code::DynamicCallRecord {
        psi_operation: operation,
        dynamic_dispatch: abstract_operations::AbstractReboundDynamicDispatch {
            initial,
            rebound,
            descriptor: terminal_psi::TerminalReboundDynamicDescriptor {
                owner,
                ordinal: 0,
                initial_selection_ordinal: 0,
                rebound_selection_ordinal: 1,
            },
            initial_application: application.clone(),
            application,
            dispatch: terminal_psi::TerminalIndirectDynamicDispatch {
                owner,
                operation,
                descriptor_ordinal: 0,
                declaring_trait_identity: "closed.trait".to_string(),
                public_requirement_identity: "closed.req.a".to_string(),
                requirement_identity: "closed.req.a.impl".to_string(),
                realization_identity: "closed.real.a".to_string(),
                realization_callable_identity: "closed.callable.a".to_string(),
                realization: machine_id(2),
            },
        },
        call_plan: calling_conventions::CallPlan {
            policy: calling_conventions::CallingPolicy::SystemVAMD64,
            parameters: vec![descriptor_placement.clone()],
            result: None,
            callback_materializations: Vec::new(),
            ordinary_clobbers: calling_conventions::RegisterSet::new([]),
            stack_alignment: 16,
            shadow_bytes: 0,
            entry_control: calling_conventions::EntryControl::CallReturn,
        },
        result: None,
        descriptor_abi: machine_code::DynamicTraitDescriptorAbiRecord {
            instance_byte_offset: 0,
            table_byte_offset: 8,
            word_byte_size: 8,
            total_byte_size: 16,
            byte_alignment: 8,
        },
        descriptor_home_byte_offset: 0,
        initial_instance: machine_code::DynamicInstanceMaterializationRecord {
            selection_ordinal: 0,
            source: instance_source.clone(),
            source_home_byte_offset: 0,
            source_home_indirect: false,
            code_offset: 4,
            byte_count: 9,
        },
        table_address: machine_code::DynamicTableAddressMaterialization {
            code_offset: 22,
            byte_count: 12,
            encoding: machine_code::DynamicTableAddressEncoding::X86_64Relative32 {
                relocation_offset: 25,
            },
        },
        rebound_instance: machine_code::DynamicInstanceMaterializationRecord {
            selection_ordinal: 1,
            source: instance_source,
            source_home_byte_offset: 0,
            source_home_indirect: false,
            code_offset: 13,
            byte_count: 9,
        },
        argument: machine_code::InternalUnitCallArgumentRecord {
            place,
            access: StructuralAccess::SharedBorrow,
            path: Vec::new(),
            root_structural_type: structural_type,
            structural_type,
            shape: empty_shape,
            source_byte_offset: 0,
            source_location: machine_code::StructuralSourceLocation::Stack { byte_offset: 0 },
            call_stack_bytes: 24,
            fixed_array_length: None,
            element_stride: None,
            source: machine_code::InternalUnitStructuralArgumentSourceRecord::Placement(
                empty_placement,
            ),
            destination: descriptor_placement,
            code_offset: 38,
            byte_count: 5,
            bytes: vec![0x48, 0x8b, 0x7c, 0x24, 0x18],
        },
        selected_table_byte_offset: 0,
        indirect_call_offset: 51,
        indirect_call_byte_count: 3,
        unit_stack: UnitCallStackEvidence {
            outbound: Some(StackAdjustmentPair {
                byte_size: 24,
                allocation_offset: 34,
                allocation_byte_count: 4,
                release_offset: 54,
                release_byte_count: 4,
            }),
        },
        operation_ordinal: 0,
        code_offset: 0,
        byte_count: 58,
    };

    let caller = &mut plan.functions[2];
    caller.attachment = Some(structural_type);
    caller.provenance.operations = vec![operation];
    caller.bytes = vec![
        // sub rsp, 16 — function frame holding the dynamic descriptor home.
        0x48, 0x83, 0xec, 0x10, //
        // lea r11, [rsp]; mov [rsp], r11 — initial instance word.
        0x4c, 0x8d, 0x1c, 0x24, 0x4c, 0x89, 0x5c, 0x24, 0x00, //
        // lea r11, [rsp]; mov [rsp], r11 — rebound instance word.
        0x4c, 0x8d, 0x1c, 0x24, 0x4c, 0x89, 0x5c, 0x24, 0x00, //
        // lea r10, [rip+rel32]; mov [rsp+8], r10 — table address word.
        0x4c, 0x8d, 0x15, 0x00, 0x00, 0x00, 0x00, 0x4c, 0x89, 0x54, 0x24, 0x08, //
        // sub rsp, 24 — outbound call frame.
        0x48, 0x83, 0xec, 0x18, //
        // mov rdi, [rsp+24] — argument descriptor pointer.
        0x48, 0x8b, 0x7c, 0x24, 0x18, //
        // mov r11, [rsp+32]; mov r11, [r11]; call r11.
        0x4c, 0x8b, 0x5c, 0x24, 0x20, 0x4d, 0x8b, 0x1b, 0x41, 0xff, 0xd3, //
        // add rsp, 24 — outbound release completes the operation span.
        0x48, 0x83, 0xc4, 0x18, //
        // Original edge-owned cleanup tail, shifted by 58 bytes.
        0x48, 0x83, 0xec, 0x08, 0xe8, 0x00, 0x00, 0x00, 0x00, 0x48, 0x83, 0xc4, 0x08, //
        // add rsp, 16; ret — frame release and return close the edge span.
        0x48, 0x83, 0xc4, 0x10, 0xc3,
    ];
    caller.unit_stack = Some(UnitStackEvidence {
        frame: Some(StackAdjustmentPair {
            byte_size: 16,
            allocation_offset: 0,
            allocation_byte_count: 4,
            release_offset: 71,
            release_byte_count: 4,
        }),
        aarch64_return_link: None,
        stack_alignment: 16,
    });
    caller.dynamic_calls = vec![dynamic_call];
    caller.internal_calls[0].offset = 63;
    caller.internal_calls[0].unit_stack = Some(UnitCallStackEvidence {
        outbound: Some(StackAdjustmentPair {
            byte_size: 8,
            allocation_offset: 58,
            allocation_byte_count: 4,
            release_offset: 67,
            release_byte_count: 4,
        }),
    });
    caller.internal_unit_calls[0].code_offset = 58;
    caller.internal_unit_calls[0].operation_ordinal = 1;
    let cleanup = caller
        .unit_affine_cleanup
        .as_mut()
        .expect("Unit cleanup fixture");
    cleanup.code_offset = 58;
    cleanup.byte_count = 18;
    caller.semantic_code_attribution = vec![
        SemanticCodeAttribution {
            site: SemanticCodeSite::Operation(operation),
            operation_ordinal: 0,
            code_offset: 0,
            byte_count: 58,
        },
        SemanticCodeAttribution {
            site: SemanticCodeSite::Edge(edge_id(3)),
            operation_ordinal: 1,
            code_offset: 58,
            byte_count: 18,
        },
    ];
    plan
}

/// One scalar Terminal function whose body performs a single indirect dispatch
/// through an existential descriptor parameter: the descriptor's table word
/// arrives in `rsi`, so `call qword ptr [rsi]` selects requirement slot zero.
/// The record carries the complete semantic, ABI, register, and byte custody
/// that object replay must re-derive from the emitted bytes alone.
fn dynamic_parameter_call_plan() -> MachineCodePlan {
    let target = NativeTarget::linux_x64();
    let mut plan = internal_call_plan(target);
    let caller = &mut plan.functions[1];
    caller.bytes = vec![
        0x50, // push rax — outbound call frame keeps the call site 16-aligned
        0xff, 0x96, 0, 0, 0, 0,    // call qword ptr [rsi] — descriptor table slot zero
        0x58, // pop rax
        0xc3, // ret
    ];
    caller.internal_calls = Vec::new();
    caller.scalar_stack = Some(ScalarStackEvidence {
        mutations: vec![
            scalar_mutation(0, 1, ScalarStackMutationKind::X86Push),
            scalar_mutation(7, 1, ScalarStackMutationKind::X86Pop),
        ],
        control_flow: ScalarControlFlowEvidence::Linear,
        stack_alignment: 16,
        cleanup_preservation: None,
    });
    let pointer = ValueShape::integer(8, 8);
    let policy = calling_conventions::CallingPolicy::native_for_target(target);
    let function_call_plan = calling_conventions::evaluate_call_plan(
        policy,
        &calling_conventions::CallSignature {
            parameters: vec![pointer, pointer],
            result: None,
        },
    )
    .expect("descriptor-parameter entry ABI");
    let dispatch_call_plan = calling_conventions::evaluate_call_plan(
        policy,
        &calling_conventions::CallSignature {
            parameters: vec![pointer],
            result: None,
        },
    )
    .expect("erased adapter ABI");
    let requirement = terminal_psi::TerminalDynamicRequirement {
        slot: 0,
        declaring_trait_identity: "dyn.trait".to_string(),
        public_requirement_identity: "dyn.req".to_string(),
        result: terminal_psi::ClosedConformanceCallableResult::Unit,
    };
    caller.dynamic_parameter_calls = vec![machine_code::DynamicParameterCallRecord {
        psi_edge: edge_id(2),
        psi_operation: operation_id(2),
        source_value: None,
        scalar_type: None,
        parameter: terminal_psi::TerminalDynamicDescriptorParameter {
            owner: machine_id(2),
            ordinal: 0,
            source_position: 0,
            trait_identity: "dyn.trait".to_string(),
            access: StructuralAccess::SharedBorrow,
            requirements: vec![requirement.clone()],
        },
        requirement,
        function_call_plan,
        dispatch_call_plan,
        instance: calling_conventions::MachineRegister::X86Rdi,
        table: calling_conventions::MachineRegister::X86Rsi,
        table_slot_byte_offset: 0,
        mechanism: machine_code::DynamicParameterCallMechanismRecord::X86MemoryIndirect {
            table: calling_conventions::MachineRegister::X86Rsi,
        },
        indirect_call_offset: 1,
        indirect_call_byte_count: 6,
        call_stack: ScalarCallStackEvidence {
            outbound: None,
            aarch64_return_link: None,
        },
        operation_ordinal: 0,
        code_offset: 0,
        byte_count: 8,
    }];
    plan
}

/// The edge-owned cleanup caller extended with one stored-descriptor
/// establishment and one later dispatch through the same aggregate slot. The
/// frame holds the descriptor pair at offset zero and the signed i32 result
/// home at offset sixteen, so every retained field below is re-derived from
/// these bytes during installation.
fn stored_dynamic_call_plan() -> MachineCodePlan {
    let mut plan = edge_owned_cleanup_plan();
    let owner = machine_id(3);
    let establishment_operation = operation_id(4);
    let call_operation = operation_id(5);
    let place = PlaceId::new(1).expect("dynamic source place");
    let structural_type = StructuralTypeId::new(1).expect("structural type");
    let i32_type = semantic_vocabulary::ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 32)
            .expect("i32 scalar type"),
    );
    let result_shape = ValueShape::integer(4, 4);
    let empty_shape = ValueShape::integer(0, 1);
    let empty_placement = ValuePlacement {
        shape: empty_shape,
        locations: Vec::new(),
    };
    let descriptor_placement = ValuePlacement {
        shape: ValueShape::integer(16, 8),
        locations: vec![calling_conventions::ValueLocation::Indirect {
            pointer: calling_conventions::IndirectPointerLocation::Register(
                calling_conventions::MachineRegister::X86Rdi,
            ),
            copy_stack_byte_offset: None,
            byte_size: 16,
            alignment: 8,
        }],
    };
    let result_placement = ValuePlacement {
        shape: result_shape,
        locations: vec![calling_conventions::ValueLocation::Register {
            register: calling_conventions::MachineRegister::X86Rax,
            value_byte_offset: 0,
            byte_size: 4,
        }],
    };
    let mut application = terminal_psi::ClosedConformanceApplication {
        owner,
        declaration_identity: "closed.decl".to_string(),
        telescope: Vec::new(),
        subject_identity: None,
        trait_identity: "closed.trait".to_string(),
        trait_lifetime_arguments: Vec::new(),
        trait_arguments: Vec::new(),
        realization_callables: vec![terminal_psi::ClosedConformanceRealizationCallable {
            source_callable_identity: "closed.callable.a".to_string(),
            machine: machine_id(2),
            result: terminal_psi::ClosedConformanceCallableResult::I32,
        }],
        rows: vec![
            terminal_psi::ClosedConformanceRow {
                declaring_trait_identity: "closed.trait".to_string(),
                public_requirement_identity: "closed.req.a".to_string(),
                requirement_identity: "closed.req.a.impl".to_string(),
                realization_identity: "closed.real.a".to_string(),
                realization_callable_identity: Some("closed.callable.a".to_string()),
            },
            terminal_psi::ClosedConformanceRow {
                declaring_trait_identity: "closed.trait".to_string(),
                public_requirement_identity: "closed.req.b".to_string(),
                requirement_identity: "closed.req.b.impl".to_string(),
                realization_identity: "closed.real.b".to_string(),
                realization_callable_identity: None,
            },
        ],
        report_fingerprint: 0,
        commitment: terminal_psi::ClosedConformanceApplicationCommitment::default(),
    };
    application.report_fingerprint =
        terminal_psi::closed_conformance_application_report_fingerprint(&application);
    application.commitment = terminal_psi::closed_conformance_application_commitment(&application);
    let stored = abstract_operations::AbstractStoredDynamicDescriptor {
        selection: terminal_psi::TerminalDynamicConformanceSelection {
            owner,
            ordinal: 0,
            source: StructuralArgument {
                place,
                path: Vec::new(),
                access: StructuralAccess::SharedBorrow,
            },
            conformance_application_report_fingerprint: application.report_fingerprint,
            conformance_application_commitment: application.commitment,
        },
        descriptor: terminal_psi::TerminalStoredDynamicDescriptor {
            owner,
            ordinal: 0,
            establishment_operation,
            selection_ordinal: 0,
            aggregate_type_identity: "closed.aggregate".to_string(),
            field_identity: "closed.field".to_string(),
        },
        application,
    };
    let instance_source = target_operations::TargetStructuralArgument {
        place,
        access: StructuralAccess::SharedBorrow,
        path: Vec::new(),
        root_structural_type: structural_type,
        structural_type,
        shape: empty_shape,
        source_byte_offset: 0,
        fixed_array_length: None,
        element_stride: None,
        source: target_operations::TargetStructuralArgumentSource::Placement(
            empty_placement.clone(),
        ),
        destination: descriptor_placement.clone(),
    };
    let result_home = machine_code::UnitScalarHomeRecord {
        defining_operation: call_operation,
        source_value: semantic_vocabulary::ValueId::new(80).expect("stored result value"),
        scalar_type: i32_type,
        shape: result_shape,
        byte_offset: 16,
    };
    let stored_call = machine_code::StoredDynamicCallRecord {
        establishment: machine_code::StoredDynamicDescriptorMaterializationRecord {
            psi_operation: establishment_operation,
            stored: stored.clone(),
            descriptor_abi: machine_code::DynamicTraitDescriptorAbiRecord {
                instance_byte_offset: 0,
                table_byte_offset: 8,
                word_byte_size: 8,
                total_byte_size: 16,
                byte_alignment: 8,
            },
            descriptor_home_byte_offset: 0,
            instance: machine_code::DynamicInstanceMaterializationRecord {
                selection_ordinal: 0,
                source: instance_source,
                source_home_byte_offset: 0,
                source_home_indirect: false,
                code_offset: 4,
                byte_count: 9,
            },
            table_address: machine_code::DynamicTableAddressMaterialization {
                code_offset: 13,
                byte_count: 12,
                encoding: machine_code::DynamicTableAddressEncoding::X86_64Relative32 {
                    relocation_offset: 16,
                },
            },
            operation_ordinal: 0,
            code_offset: 0,
            byte_count: 25,
        },
        psi_operation: call_operation,
        dynamic_dispatch: abstract_operations::AbstractStoredDynamicDispatch {
            stored,
            dispatch: terminal_psi::TerminalStoredDynamicDispatch {
                owner,
                operation: call_operation,
                descriptor_ordinal: 0,
                declaring_trait_identity: "closed.trait".to_string(),
                public_requirement_identity: "closed.req.a".to_string(),
                requirement_identity: "closed.req.a.impl".to_string(),
                realization_identity: "closed.real.a".to_string(),
                realization_callable_identity: "closed.callable.a".to_string(),
                realization: machine_id(2),
            },
        },
        call_plan: calling_conventions::CallPlan {
            policy: calling_conventions::CallingPolicy::SystemVAMD64,
            parameters: vec![descriptor_placement.clone()],
            result: Some(result_placement.clone()),
            callback_materializations: Vec::new(),
            ordinary_clobbers: calling_conventions::RegisterSet::new([]),
            stack_alignment: 16,
            shadow_bytes: 0,
            entry_control: calling_conventions::EntryControl::CallReturn,
        },
        result: machine_code::InternalUnitScalarCallResultRecord {
            home: result_home,
            source: result_placement,
            code_offset: 49,
            byte_count: 8,
        },
        argument: machine_code::InternalUnitCallArgumentRecord {
            place,
            access: StructuralAccess::SharedBorrow,
            path: Vec::new(),
            root_structural_type: structural_type,
            structural_type,
            shape: empty_shape,
            source_byte_offset: 0,
            source_location: machine_code::StructuralSourceLocation::Stack { byte_offset: 0 },
            call_stack_bytes: 24,
            fixed_array_length: None,
            element_stride: None,
            source: machine_code::InternalUnitStructuralArgumentSourceRecord::Placement(
                empty_placement,
            ),
            destination: descriptor_placement,
            code_offset: 29,
            byte_count: 5,
            bytes: vec![0x48, 0x8b, 0x7c, 0x24, 0x18],
        },
        selected_table_byte_offset: 0,
        indirect_call_offset: 42,
        indirect_call_byte_count: 3,
        unit_stack: UnitCallStackEvidence {
            outbound: Some(StackAdjustmentPair {
                byte_size: 24,
                allocation_offset: 25,
                allocation_byte_count: 4,
                release_offset: 45,
                release_byte_count: 4,
            }),
        },
        operation_ordinal: 1,
        code_offset: 25,
        byte_count: 32,
    };
    let caller = &mut plan.functions[2];
    caller.attachment = Some(structural_type);
    caller.provenance.operations = vec![establishment_operation, call_operation];
    caller.bytes = vec![
        // sub rsp, 32 — frame holding the stored descriptor and result home.
        0x48, 0x83, 0xec, 0x20, //
        // lea r11, [rsp]; mov [rsp], r11 — descriptor instance word.
        0x4c, 0x8d, 0x1c, 0x24, 0x4c, 0x89, 0x5c, 0x24, 0x00, //
        // lea r10, [rip+rel32]; mov [rsp+8], r10 — descriptor table word.
        0x4c, 0x8d, 0x15, 0x00, 0x00, 0x00, 0x00, 0x4c, 0x89, 0x54, 0x24, 0x08, //
        // sub rsp, 24 — outbound call frame.
        0x48, 0x83, 0xec, 0x18, //
        // mov rdi, [rsp+24] — descriptor instance word argument.
        0x48, 0x8b, 0x7c, 0x24, 0x18, //
        // mov r11, [rsp+32]; mov r11, [r11]; call r11 — selected slot dispatch.
        0x4c, 0x8b, 0x5c, 0x24, 0x20, 0x4d, 0x8b, 0x1b, 0x41, 0xff, 0xd3, //
        // add rsp, 24 — outbound release.
        0x48, 0x83, 0xc4, 0x18, //
        // movsxd rax, eax; mov [rsp+16], rax — i32 result home store.
        0x48, 0x63, 0xc0, 0x48, 0x89, 0x44, 0x24, 0x10, //
        // Edge-owned cleanup tail: sub rsp, 8; call rel32; add rsp, 8.
        0x48, 0x83, 0xec, 0x08, 0xe8, 0x00, 0x00, 0x00, 0x00, 0x48, 0x83, 0xc4, 0x08, //
        // add rsp, 32; ret — frame release and return close the edge span.
        0x48, 0x83, 0xc4, 0x20, 0xc3,
    ];
    caller.unit_stack = Some(UnitStackEvidence {
        frame: Some(StackAdjustmentPair {
            byte_size: 32,
            allocation_offset: 0,
            allocation_byte_count: 4,
            release_offset: 70,
            release_byte_count: 4,
        }),
        aarch64_return_link: None,
        stack_alignment: 16,
    });
    caller.unit_scalar_homes = vec![result_home];
    caller.stored_dynamic_calls = vec![stored_call];
    caller.internal_calls[0].offset = 62;
    caller.internal_calls[0].unit_stack = Some(UnitCallStackEvidence {
        outbound: Some(StackAdjustmentPair {
            byte_size: 8,
            allocation_offset: 57,
            allocation_byte_count: 4,
            release_offset: 66,
            release_byte_count: 4,
        }),
    });
    caller.internal_unit_calls[0].code_offset = 57;
    caller.internal_unit_calls[0].operation_ordinal = 2;
    let cleanup = caller
        .unit_affine_cleanup
        .as_mut()
        .expect("Unit cleanup fixture");
    cleanup.code_offset = 57;
    cleanup.byte_count = 18;
    caller.semantic_code_attribution = vec![
        SemanticCodeAttribution {
            site: SemanticCodeSite::Operation(establishment_operation),
            operation_ordinal: 0,
            code_offset: 0,
            byte_count: 25,
        },
        SemanticCodeAttribution {
            site: SemanticCodeSite::Operation(call_operation),
            operation_ordinal: 1,
            code_offset: 25,
            byte_count: 32,
        },
        SemanticCodeAttribution {
            site: SemanticCodeSite::Edge(edge_id(3)),
            operation_ordinal: 2,
            code_offset: 57,
            byte_count: 18,
        },
    ];
    plan
}

/// One transparent descriptor-parameter helper (machine 2) forwarding its own
/// parameter unchanged into machine 1's existential dispatch body. Both the
/// helper's forwarded-parameter custody and the callee's dispatch custody are
/// retained for installation.
fn forwarded_dynamic_parameter_call_plan() -> MachineCodePlan {
    let target = NativeTarget::linux_x64();
    let mut plan = internal_call_plan(target);
    let i32_type = semantic_vocabulary::ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 32)
            .expect("i32 scalar type"),
    );
    let pointer = ValueShape::integer(8, 8);
    let policy = calling_conventions::CallingPolicy::native_for_target(target);
    let call_plan = calling_conventions::evaluate_call_plan(
        policy,
        &calling_conventions::CallSignature {
            parameters: vec![pointer, pointer],
            result: Some(ValueShape::integer(4, 4)),
        },
    )
    .expect("forwarded descriptor ABI");
    let dispatch_plan = calling_conventions::evaluate_call_plan(
        policy,
        &calling_conventions::CallSignature {
            parameters: vec![pointer],
            result: Some(ValueShape::integer(4, 4)),
        },
    )
    .expect("parameter dispatch ABI");
    let requirement = terminal_psi::TerminalDynamicRequirement {
        slot: 0,
        declaring_trait_identity: "dyn.trait".to_string(),
        public_requirement_identity: "dyn.req".to_string(),
        result: terminal_psi::ClosedConformanceCallableResult::I32,
    };
    // The callee owns one existential descriptor parameter and dispatches its
    // requirement slot zero through the inbound table register.
    let callee_parameter = terminal_psi::TerminalDynamicDescriptorParameter {
        owner: machine_id(1),
        ordinal: 0,
        source_position: 0,
        trait_identity: "dyn.trait".to_string(),
        access: StructuralAccess::SharedBorrow,
        requirements: vec![requirement.clone()],
    };
    let callee = &mut plan.functions[0];
    callee.bytes = vec![
        0x50, // push rax — outbound call frame keeps the call site 16-aligned
        0xff, 0x96, 0, 0, 0, 0,    // call qword ptr [rsi] — descriptor table slot zero
        0x58, // pop rax
        0xc3, // ret
    ];
    callee.scalar_stack = Some(ScalarStackEvidence {
        mutations: vec![
            scalar_mutation(0, 1, ScalarStackMutationKind::X86Push),
            scalar_mutation(7, 1, ScalarStackMutationKind::X86Pop),
        ],
        control_flow: ScalarControlFlowEvidence::Linear,
        stack_alignment: 16,
        cleanup_preservation: None,
    });
    callee.dynamic_parameter_calls = vec![machine_code::DynamicParameterCallRecord {
        psi_edge: edge_id(1),
        psi_operation: operation_id(1),
        source_value: Some(semantic_vocabulary::ValueId::new(90).expect("dispatch result value")),
        scalar_type: Some(i32_type),
        parameter: callee_parameter.clone(),
        requirement: requirement.clone(),
        function_call_plan: call_plan.clone(),
        dispatch_call_plan: dispatch_plan,
        instance: calling_conventions::MachineRegister::X86Rdi,
        table: calling_conventions::MachineRegister::X86Rsi,
        table_slot_byte_offset: 0,
        mechanism: machine_code::DynamicParameterCallMechanismRecord::X86MemoryIndirect {
            table: calling_conventions::MachineRegister::X86Rsi,
        },
        indirect_call_offset: 1,
        indirect_call_byte_count: 6,
        call_stack: ScalarCallStackEvidence {
            outbound: None,
            aarch64_return_link: None,
        },
        operation_ordinal: 0,
        code_offset: 0,
        byte_count: 8,
    }];
    // The forwarder exposes the same descriptor parameter and passes it
    // unchanged to the callee through one aligned direct call.
    let forwarder_parameter = terminal_psi::TerminalDynamicDescriptorParameter {
        owner: machine_id(2),
        ordinal: 0,
        source_position: 0,
        trait_identity: "dyn.trait".to_string(),
        access: StructuralAccess::SharedBorrow,
        requirements: vec![requirement],
    };
    let call_stack = ScalarCallStackEvidence {
        outbound: None,
        aarch64_return_link: None,
    };
    let forwarder = &mut plan.functions[1];
    forwarder.bytes = vec![
        0x50, // push rax — outbound call frame keeps the call site 16-aligned
        0xe8, 0, 0, 0, 0,    // call machine 1
        0x58, // pop rax
        0xc3, // ret
    ];
    forwarder.scalar_stack = Some(ScalarStackEvidence {
        mutations: vec![
            scalar_mutation(0, 1, ScalarStackMutationKind::X86Push),
            scalar_mutation(6, 1, ScalarStackMutationKind::X86Pop),
        ],
        control_flow: ScalarControlFlowEvidence::Linear,
        stack_alignment: 16,
        cleanup_preservation: None,
    });
    forwarder.internal_calls = vec![InternalCallRelocation {
        owner: CallSiteOwner::Operation(operation_id(2)),
        target: machine_id(1),
        unit_stack: None,
        scalar_stack: Some(call_stack),
        offset: 2,
    }];
    forwarder.forwarded_dynamic_parameter_calls =
        vec![machine_code::ForwardedDynamicParameterCallRecord {
            psi_edge: edge_id(2),
            psi_operation: operation_id(2),
            source_value: Some(semantic_vocabulary::ValueId::new(91).expect("forwarded result")),
            scalar_type: Some(i32_type),
            callee: machine_id(1),
            argument: abstract_operations::AbstractDynamicDescriptorArgument {
                argument: terminal_psi::TerminalDynamicDescriptorArgument {
                    owner: machine_id(2),
                    operation: operation_id(2),
                    parameter_ordinal: 0,
                    source: terminal_psi::TerminalDynamicDescriptorSource::Parameter { ordinal: 0 },
                },
                target: callee_parameter,
                source: abstract_operations::AbstractDynamicDescriptorSource::Parameter(
                    forwarder_parameter.clone(),
                ),
            },
            parameter: forwarder_parameter,
            function_call_plan: call_plan.clone(),
            callee_call_plan: call_plan,
            instance: calling_conventions::MachineRegister::X86Rdi,
            table: calling_conventions::MachineRegister::X86Rsi,
            instance_destination: calling_conventions::MachineRegister::X86Rdi,
            table_destination: calling_conventions::MachineRegister::X86Rsi,
            direct_call_offset: 1,
            direct_call_byte_count: 5,
            call_stack: machine_code::ForwardedDynamicParameterCallStackEvidence::Scalar(
                call_stack,
            ),
            operation_ordinal: 0,
            code_offset: 0,
            byte_count: 7,
        }];
    forwarder.semantic_code_attribution = vec![
        SemanticCodeAttribution {
            site: SemanticCodeSite::Operation(operation_id(2)),
            operation_ordinal: 0,
            code_offset: 0,
            byte_count: 7,
        },
        SemanticCodeAttribution {
            site: SemanticCodeSite::Edge(edge_id(2)),
            operation_ordinal: 1,
            code_offset: 7,
            byte_count: 1,
        },
    ];
    plan
}

/// The edge-owned cleanup caller extended with one forwarded existential
/// descriptor argument whose single-row closed application emits one adapter
/// plus one forwarded table. The call itself has a Unit requirement result.
fn forwarded_dynamic_descriptor_call_plan() -> MachineCodePlan {
    let mut plan = edge_owned_cleanup_plan();
    let owner = machine_id(3);
    let operation = operation_id(4);
    let place = PlaceId::new(1).expect("forwarded source place");
    let structural_type = StructuralTypeId::new(1).expect("structural type");
    let pointer = ValueShape::integer(8, 8);
    let policy = calling_conventions::CallingPolicy::native_for_target(NativeTarget::linux_x64());
    let call_plan = calling_conventions::evaluate_call_plan(
        policy,
        &calling_conventions::CallSignature {
            parameters: vec![pointer, pointer],
            result: None,
        },
    )
    .expect("forwarded descriptor ABI");
    let adapter_call_plan = calling_conventions::evaluate_call_plan(
        policy,
        &calling_conventions::CallSignature {
            parameters: vec![pointer],
            result: None,
        },
    )
    .expect("adapter ABI");
    let requirement = terminal_psi::TerminalDynamicRequirement {
        slot: 0,
        declaring_trait_identity: "fwd.trait".to_string(),
        public_requirement_identity: "fwd.req".to_string(),
        result: terminal_psi::ClosedConformanceCallableResult::Unit,
    };
    let mut application = terminal_psi::ClosedConformanceApplication {
        owner,
        declaration_identity: "fwd.decl".to_string(),
        telescope: Vec::new(),
        subject_identity: None,
        trait_identity: "fwd.trait".to_string(),
        trait_lifetime_arguments: Vec::new(),
        trait_arguments: Vec::new(),
        realization_callables: vec![terminal_psi::ClosedConformanceRealizationCallable {
            source_callable_identity: "fwd.callable.a".to_string(),
            machine: machine_id(2),
            result: terminal_psi::ClosedConformanceCallableResult::Unit,
        }],
        rows: vec![terminal_psi::ClosedConformanceRow {
            declaring_trait_identity: "fwd.trait".to_string(),
            public_requirement_identity: "fwd.req".to_string(),
            requirement_identity: "fwd.req.impl".to_string(),
            realization_identity: "fwd.real.a".to_string(),
            realization_callable_identity: Some("fwd.callable.a".to_string()),
        }],
        report_fingerprint: 0,
        commitment: terminal_psi::ClosedConformanceApplicationCommitment::default(),
    };
    application.report_fingerprint =
        terminal_psi::closed_conformance_application_report_fingerprint(&application);
    application.commitment = terminal_psi::closed_conformance_application_commitment(&application);
    let adapter = machine_code::ForwardedDynamicDescriptorAdapterRecord {
        identity: machine_code::ForwardedDynamicDescriptorAdapterIdentity {
            application: application.commitment,
            row_index: 0,
            realization: machine_id(2),
        },
        requirement_identity: "fwd.req.impl".to_string(),
        realization_identity: "fwd.real.a".to_string(),
        realization_callable_identity: "fwd.callable.a".to_string(),
        result: terminal_psi::ClosedConformanceCallableResult::Unit,
        erased_call_plan: adapter_call_plan.clone(),
        realization_call_plan: adapter_call_plan,
        source_shape: pointer,
        bytes: vec![
            // mov rdi, [rdi] — load the shared-borrow instance word.
            0x48, 0x8b, 0x3f, //
            // sub rsp, 8; call rel32; add rsp, 8 — aligned adapter tail call.
            0x48, 0x83, 0xec, 0x08, 0xe8, 0x00, 0x00, 0x00, 0x00, 0x48, 0x83, 0xc4, 0x08, //
            0xc3,
        ],
        argument_code_offset: 0,
        argument_byte_count: 3,
        direct_call_offset: 7,
        direct_call_byte_count: 5,
        return_offset: 16,
        return_byte_count: 1,
    };
    let argument = machine_code::ForwardedDynamicDescriptorArgumentRecord {
        custody: abstract_operations::AbstractDynamicDescriptorArgument {
            argument: terminal_psi::TerminalDynamicDescriptorArgument {
                owner,
                operation,
                parameter_ordinal: 0,
                source: terminal_psi::TerminalDynamicDescriptorSource::Selection { ordinal: 0 },
            },
            target: terminal_psi::TerminalDynamicDescriptorParameter {
                owner: machine_id(1),
                ordinal: 0,
                source_position: 0,
                trait_identity: "fwd.trait".to_string(),
                access: StructuralAccess::SharedBorrow,
                requirements: vec![requirement],
            },
            source: abstract_operations::AbstractDynamicDescriptorSource::Selection {
                selection: terminal_psi::TerminalDynamicConformanceSelection {
                    owner,
                    ordinal: 0,
                    source: StructuralArgument {
                        place,
                        path: Vec::new(),
                        access: StructuralAccess::SharedBorrow,
                    },
                    conformance_application_report_fingerprint: application.report_fingerprint,
                    conformance_application_commitment: application.commitment,
                },
                application,
            },
        },
        instance: target_operations::TargetDynamicDescriptorInstanceArgument {
            place,
            access: StructuralAccess::SharedBorrow,
            path: Vec::new(),
            root_structural_type: structural_type,
            structural_type,
            shape: pointer,
            source_byte_offset: 0,
            source: ValuePlacement {
                shape: pointer,
                locations: vec![calling_conventions::ValueLocation::Stack {
                    stack_byte_offset: 0,
                    value_byte_offset: 0,
                    byte_size: 8,
                    alignment: 8,
                }],
            },
            destination: ValuePlacement {
                shape: pointer,
                locations: vec![calling_conventions::ValueLocation::Register {
                    register: calling_conventions::MachineRegister::X86Rdi,
                    value_byte_offset: 0,
                    byte_size: 8,
                }],
            },
        },
        instance_destination: calling_conventions::MachineRegister::X86Rdi,
        table_destination: calling_conventions::MachineRegister::X86Rsi,
        source_home_byte_offset: 0,
        source_home_indirect: false,
        instance_code_offset: 7,
        instance_byte_count: 4,
        table_address: machine_code::DynamicTableAddressMaterialization {
            code_offset: 0,
            byte_count: 7,
            encoding: machine_code::DynamicTableAddressEncoding::X86_64Relative32 {
                relocation_offset: 3,
            },
        },
        adapters: vec![adapter],
    };
    let caller = &mut plan.functions[2];
    caller.attachment = Some(structural_type);
    caller.provenance.operations = vec![operation];
    caller.bytes = vec![
        // lea rsi, [rip+rel32] — forwarded table address (relocation at +3).
        0x48, 0x8d, 0x35, 0x00, 0x00, 0x00, 0x00, //
        // lea rdi, [rsp] — instance pointer materialization.
        0x48, 0x8d, 0x3c, 0x24, //
        // sub rsp, 8; call rel32; add rsp, 8 — aligned direct call to the callee.
        0x48, 0x83, 0xec, 0x08, 0xe8, 0x00, 0x00, 0x00, 0x00, 0x48, 0x83, 0xc4, 0x08, //
        // Edge-owned cleanup tail: sub rsp, 8; call rel32; add rsp, 8.
        0x48, 0x83, 0xec, 0x08, 0xe8, 0x00, 0x00, 0x00, 0x00, 0x48, 0x83, 0xc4, 0x08, //
        0xc3,
    ];
    caller.internal_calls = vec![
        InternalCallRelocation {
            owner: CallSiteOwner::Operation(operation),
            target: machine_id(1),
            unit_stack: Some(UnitCallStackEvidence {
                outbound: Some(StackAdjustmentPair {
                    byte_size: 8,
                    allocation_offset: 11,
                    allocation_byte_count: 4,
                    release_offset: 20,
                    release_byte_count: 4,
                }),
            }),
            scalar_stack: None,
            offset: 16,
        },
        InternalCallRelocation {
            owner: CallSiteOwner::CleanupAction {
                edge: edge_id(3),
                action_ordinal: 0,
            },
            target: machine_id(1),
            unit_stack: Some(UnitCallStackEvidence {
                outbound: Some(StackAdjustmentPair {
                    byte_size: 8,
                    allocation_offset: 24,
                    allocation_byte_count: 4,
                    release_offset: 33,
                    release_byte_count: 4,
                }),
            }),
            scalar_stack: None,
            offset: 29,
        },
    ];
    caller.internal_unit_calls[0].code_offset = 24;
    caller.internal_unit_calls[0].operation_ordinal = 1;
    let cleanup = caller
        .unit_affine_cleanup
        .as_mut()
        .expect("Unit cleanup fixture");
    cleanup.code_offset = 24;
    cleanup.byte_count = 14;
    caller.forwarded_dynamic_descriptor_calls =
        vec![machine_code::ForwardedDynamicDescriptorCallRecord {
            psi_operation: operation,
            semantic_result: None,
            result: None,
            callee: machine_id(1),
            call_plan,
            dynamic_arguments: vec![argument],
            claim_transfers: Vec::new(),
            direct_call_offset: 15,
            direct_call_byte_count: 5,
            unit_stack: UnitCallStackEvidence {
                outbound: Some(StackAdjustmentPair {
                    byte_size: 8,
                    allocation_offset: 11,
                    allocation_byte_count: 4,
                    release_offset: 20,
                    release_byte_count: 4,
                }),
            },
            operation_ordinal: 0,
            code_offset: 0,
            byte_count: 24,
        }];
    caller.semantic_code_attribution = vec![
        SemanticCodeAttribution {
            site: SemanticCodeSite::Operation(operation),
            operation_ordinal: 0,
            code_offset: 0,
            byte_count: 24,
        },
        SemanticCodeAttribution {
            site: SemanticCodeSite::Edge(edge_id(3)),
            operation_ordinal: 1,
            code_offset: 24,
            byte_count: 14,
        },
    ];
    plan
}

fn mixed_edge_owned_cleanup_plan() -> MachineCodePlan {
    let mut plan = edge_owned_cleanup_plan();
    let caller = &mut plan.functions[2];
    let trivial_place = PlaceId::new(2).expect("trivial place");
    let trivial_type = StructuralTypeId::new(3).expect("trivial type");
    let mut trivial_parameter = caller.unit_parameters[0];
    trivial_parameter.place = trivial_place;
    trivial_parameter.structural_type = trivial_type;
    let mut trivial_home = caller.unit_parameter_homes[0].clone();
    trivial_home.place = trivial_place;
    trivial_home.structural_type = trivial_type;
    caller.unit_parameters.push(trivial_parameter);
    caller.unit_parameter_homes.push(trivial_home);
    caller
        .unit_affine_cleanup
        .as_mut()
        .expect("Unit cleanup fixture")
        .actions
        .insert(0, TerminalAffineCleanupAction::DiscardRoot(trivial_place));
    caller.internal_calls[0].owner = CallSiteOwner::CleanupAction {
        edge: edge_id(3),
        action_ordinal: 1,
    };
    caller.internal_unit_calls[0].owner = caller.internal_calls[0].owner;
    plan
}

fn two_call_edge_owned_cleanup_plan() -> MachineCodePlan {
    let mut plan = edge_owned_cleanup_plan();
    let second_operation = operation_id(2);
    let second_helper = machine_id(4);
    let second_edge = edge_id(4);
    let second_call_bytes = [
        0x48, 0x83, 0xec, 0x08, 0xe8, 0, 0, 0, 0, 0x48, 0x83, 0xc4, 0x08,
    ];
    let drop = &mut plan.functions[0];
    drop.bytes.splice(13..13, second_call_bytes);
    drop.provenance.operations.push(second_operation);
    drop.internal_calls.push(InternalCallRelocation {
        owner: CallSiteOwner::Operation(second_operation),
        target: second_helper,
        unit_stack: Some(UnitCallStackEvidence {
            outbound: Some(StackAdjustmentPair {
                byte_size: 8,
                allocation_offset: 13,
                allocation_byte_count: 4,
                release_offset: 22,
                release_byte_count: 4,
            }),
        }),
        scalar_stack: None,
        offset: 18,
    });
    drop.internal_unit_calls.push(InternalUnitCallRecord {
        source: machine_code::InternalUnitCallSource::Authored,
        owner: CallSiteOwner::Operation(second_operation),
        target: second_helper,
        result: None,
        semantic_result: None,
        structural_result: None,
        scalar_arguments: Vec::new(),
        arguments: Vec::new(),
        claim_transfers: Vec::new(),
        operation_ordinal: 1,
        code_offset: 13,
        byte_count: 13,
    });
    drop.semantic_code_attribution.insert(
        1,
        SemanticCodeAttribution {
            site: SemanticCodeSite::Operation(second_operation),
            operation_ordinal: 1,
            code_offset: 13,
            byte_count: 13,
        },
    );
    drop.semantic_code_attribution[2].operation_ordinal = 2;
    drop.semantic_code_attribution[2].code_offset = 26;
    let drop_return = drop.unit_affine_cleanup.as_mut().expect("drop return");
    drop_return.code_offset = 26;

    let mut helper = plan.functions[1].clone();
    helper.machine = second_helper;
    helper.attachment = Some(StructuralTypeId::new(2).expect("second helper type"));
    helper.provenance.edges = vec![second_edge];
    let helper_return = helper.unit_affine_cleanup.as_mut().expect("helper return");
    helper_return.psi_edge = second_edge;
    helper.semantic_code_attribution[0].site = SemanticCodeSite::Edge(second_edge);
    plan.functions.push(helper);
    plan
}

fn add_empty_unit_cleanup(function: &mut MachineCodeFunction) {
    let byte_count = if function.bytes.ends_with(&0xd65f_03c0_u32.to_le_bytes()) {
        4
    } else {
        1
    };
    let code_offset = function.bytes.len() - byte_count;
    function.unit_affine_cleanup = Some(UnitAffineCleanupRecord {
        structural_types: Vec::new().into(),
        psi_edge: function.provenance.edges[0],
        locals: Vec::new(),
        actions: Vec::new(),
        code_offset,
        byte_count,
    });
    if function.semantic_code_attribution.is_empty() {
        function
            .semantic_code_attribution
            .push(SemanticCodeAttribution {
                site: SemanticCodeSite::Edge(function.provenance.edges[0]),
                operation_ordinal: 0,
                code_offset,
                byte_count,
            });
    }
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
use calling_conventions::{ValuePlacement, ValueShape};
