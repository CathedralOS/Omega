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
