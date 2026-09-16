//! Provider, exit, port effect, function, callback, FMA and internal call
//! plan fixtures.

use super::scalar_plans::integer_return;
use super::{edge_id, identity, machine_id, operation_id};
use calling_conventions::ValueShape;
use function_identity::{MachineFunctionIdentity, StateKey};
use image_emission::{
    InstallationError, decode_installation_record, encode_installation_record,
    installation_fingerprint, validate_installation_record,
};
use installation_evidence::ProviderExecutionEvidence;
use machine_code::{
    BoundarySettlementRecord, InternalCallRelocation, MachineCodeFunction, MachineCodePlan,
    PortEffectRecord, SemanticCodeAttribution, SemanticCodeSite, UnitAffineCleanupRecord,
    UnitStackEvidence, X86ScalarFmaFormat,
};
use object_file::object_symbol_name;
use semantic_vocabulary::{BoundaryMachineId, ClaimId, PlaceId, ServiceId, StructuralTypeId};
use symbols::SymbolHandle;
use target::{
    AdmittedX86ScalarFmaProvider, NativeTarget, TargetProfile, X86DeploymentFeatures,
    X86FeatureRequirement, X86ScalarFmaDifferentialReceipt, X86ScalarFmaSlot, X86TargetFeature,
};
use target_operations::{
    BoundaryScalarArgument, CallSiteOwner, HostedExitProcessI32Realization,
    MetadataOnlyPortRealization, ProviderExecutionBinding, ProviderPlanReportIdentity,
    TerminalPsiProvenance,
};
use terminal_psi::{StructuralAccess, StructuralArgument, StructuralMultiplicity};

#[derive(Debug)]
pub(super) struct WriteExitProvider(pub(super) u64);

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

pub(super) fn write_exit_provider_binding(
    provider: &WriteExitProvider,
) -> ProviderExecutionBinding {
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
pub(super) fn linux_write_line_exit_plan(provider: &WriteExitProvider) -> MachineCodePlan {
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

/// Windows x64 Unit entry whose single semantic operation is one returning
/// zero-argument PE import call. The lifetime frame reserves the call's MXCSR
/// slot, the evaluated Microsoft x64 plan leaves shadow space plus call
/// alignment as its outbound custody, and the admitted same-stack contribution
/// carries the opaque provider leaf. Every retained interval replays against
/// the emitted bytes, so object construction, the PE image, and the
/// installation record's foreign-call stack row are all genuinely derived.
pub(super) fn windows_foreign_call_plan(provider: &WriteExitProvider) -> MachineCodePlan {
    let target = NativeTarget::windows_x64();
    let machine = machine_id(61);
    let call_operation = operation_id(61);
    let return_edge = edge_id(61);
    let locator = target::normalize_foreign_locator(
        target::ForeignLocatorCandidate::PeByName {
            library: b"kernel32.dll".to_vec(),
            export: b"ExitProcess".to_vec(),
        },
        TargetProfile::WindowsX64,
    )
    .expect("normalized PE import");
    let signature = calling_conventions::CallSignature {
        parameters: Vec::new(),
        result: None,
    };
    let call_plan = calling_conventions::evaluate_call_plan(
        calling_conventions::CallingPolicy::native_for_target(target),
        &signature,
    )
    .expect("native Windows x64 call plan");
    let boundary_entry_plan = calling_conventions::evaluate_ordinary_boundary_entry_plan(
        calling_conventions::CallingPolicy::native_for_target(target),
        &signature,
    )
    .expect("ordinary boundary entry plan")
    .plan()
    .clone();
    let save_bytes = isa_x86_64::encode_stmxcsr_rsp_displacement(16).expect("MXCSR save encoding");
    let restore_bytes =
        isa_x86_64::encode_ldmxcsr_rsp_displacement(16).expect("MXCSR restore encoding");
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&[0x48, 0x83, 0xec, 0x20]); // sub rsp, 32
    let save_offset = bytes.len();
    bytes.extend_from_slice(&save_bytes);
    let outbound_allocation_offset = bytes.len();
    bytes.extend_from_slice(&[0x48, 0x83, 0xec, 0x28]); // sub rsp, 40
    bytes.push(0xe8);
    let call_offset = bytes.len();
    bytes.extend_from_slice(&[0; 4]);
    let outbound_release_offset = bytes.len();
    bytes.extend_from_slice(&[0x48, 0x83, 0xc4, 0x28]); // add rsp, 40
    let restore_offset = bytes.len();
    bytes.extend_from_slice(&restore_bytes);
    let frame_release_offset = bytes.len();
    bytes.extend_from_slice(&[0x48, 0x83, 0xc4, 0x20]); // add rsp, 32
    let return_offset = bytes.len();
    bytes.push(0xc3);
    let provider_plan_commitment =
        task_plans::SameStackProviderPlanCommitment::from_digest([7; 32]);
    let same_stack_contribution = task_plans::admit_same_stack_contribution(
        task_plans::SameStackContributionAdmissionCandidate {
            provider_plan_report_identity: provider.provider_plan_report_identity(),
            provider_plan_commitment,
            requirement_identity: "kernel32::ExitProcess".to_string(),
            receipt: task_plans::SameStackContributionAdmissionReceiptId::from_normalized_identity(
                0x7757_494e_0001,
            )
            .expect("same-stack admission receipt"),
            bytes: 64,
            alignment: 16,
        },
        provider.provider_plan_report_identity(),
        provider_plan_commitment,
        "kernel32::ExitProcess",
    )
    .expect("admitted same-stack contribution");
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
                operations: vec![call_operation],
                edges: vec![return_edge],
            },
            bytes,
            x86_scalar_fma: Vec::new(),
            x86_scalar_fma_occurrences: Vec::new(),
            x86_floating_control: None,
            unit_stack: Some(UnitStackEvidence {
                frame: Some(machine_code::StackAdjustmentPair {
                    byte_size: 32,
                    allocation_offset: 0,
                    allocation_byte_count: 4,
                    release_offset: frame_release_offset,
                    release_byte_count: 4,
                }),
                aarch64_return_link: None,
                stack_alignment: 16,
            }),
            unit_parameter_homes: Vec::new(),
            unit_parameters: Vec::new(),
            scalar_stack: None,
            internal_calls: Vec::new(),
            foreign_calls: vec![machine_code::ForeignCallRelocation {
                owner: CallSiteOwner::Operation(call_operation),
                operation_ordinal: 0,
                offset: call_offset,
                locator,
                provider_execution: write_exit_provider_binding(provider).into(),
                boundary_entry_plan,
                call_plan,
                scalar_arguments: Vec::new(),
                callback_address: None,
                scalar_result: None,
                x86_floating_control: Some(machine_code::X86ForeignCallFloatingControlRecord {
                    target,
                    saved_slot_byte_offset: 16,
                    save_offset,
                    save_byte_count: save_bytes.len(),
                    restore_offset,
                    restore_byte_count: restore_bytes.len(),
                }),
                aarch64_floating_control: None,
                unit_stack: machine_code::UnitCallStackEvidence {
                    outbound: Some(machine_code::StackAdjustmentPair {
                        byte_size: 40,
                        allocation_offset: outbound_allocation_offset,
                        allocation_byte_count: 4,
                        release_offset: outbound_release_offset,
                        release_byte_count: 4,
                    }),
                },
                same_stack_contribution,
            }],
            internal_unit_calls: Vec::new(),
            unit_continuations: Vec::new(),
            unit_affine_cleanup: Some(UnitAffineCleanupRecord {
                psi_edge: return_edge,
                structural_types: Vec::new().into(),
                locals: Vec::new(),
                actions: Vec::new(),
                code_offset: return_offset,
                byte_count: 1,
            }),
            semantic_code_attribution: vec![
                SemanticCodeAttribution {
                    site: SemanticCodeSite::Operation(call_operation),
                    operation_ordinal: 0,
                    code_offset: save_offset,
                    byte_count: frame_release_offset - save_offset,
                },
                SemanticCodeAttribution {
                    site: SemanticCodeSite::Edge(return_edge),
                    operation_ordinal: 1,
                    code_offset: return_offset,
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
        }],
    }
}

/// Linux x64 machine retaining two privileged `out` effects. The second is
/// consumed by an admitted-provider `MetadataOnlyPort` settlement while the
/// first stays unbound custody, so one-field mutation coverage reaches both
/// the replay-rejected representable axes and the axes pinned by the
/// settlement join at encoding.
pub(super) fn port_effect_plan(provider: &WriteExitProvider) -> MachineCodePlan {
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

/// Shared one-field-substitution driver for installation-header coverage: a
/// representable substitution still encodes and round-trips, the recomputed
/// installation fingerprint differs from the authentic record's published
/// identity, and replay against the unchanged image rejects the substitution.
pub(super) fn assert_header_substitution_rejected(
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

pub(super) fn artifact_symbol(artifact: &image_emission::ObjectArtifact) -> &str {
    object_symbol_name(artifact.object(), artifact.entry_function().symbol)
}

pub(super) fn two_function_plan() -> MachineCodePlan {
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

/// One machine per admitted structural-return custody lane beside the ordinary
/// entry machine: machine 1 carries the claim-free affine identity return and
/// machine 3 the claim-bearing linear return. Both records describe the same
/// emitted `mov rax, rdi; ret` interval, so object replay regenerates every
/// byte from the retained record rather than trusting the row.
pub(super) fn structural_return_plan() -> MachineCodePlan {
    let target = NativeTarget::linux_x64();
    let shape = ValueShape::integer(8, 8);
    let call_plan = calling_conventions::evaluate_call_plan(
        calling_conventions::CallingPolicy::native_for_target(target),
        &calling_conventions::CallSignature {
            parameters: vec![shape],
            result: Some(shape),
        },
    )
    .expect("one-u64 structural ABI");
    let source_placement = call_plan
        .parameters
        .first()
        .expect("parameter placement")
        .clone();
    let result_placement = call_plan.result.clone().expect("result placement");
    let structural_type = StructuralTypeId::new(41).expect("structural type");
    let install = |function: &mut MachineCodeFunction,
                   edge: u64,
                   multiplicity: StructuralMultiplicity,
                   source_place: u64,
                   result_place: u64,
                   returned_claims: Vec<ClaimId>| {
        let edge = edge_id(edge);
        let source = terminal_psi::StructuralParameterDeclaration {
            place: PlaceId::new(source_place).expect("source place"),
            position: 0,
            is_self: false,
            structural_type,
            multiplicity,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        };
        function.bytes = vec![0x48, 0x89, 0xf8, 0xc3];
        function.provenance = TerminalPsiProvenance {
            operations: Vec::new(),
            edges: vec![edge],
        };
        function.semantic_code_attribution = vec![SemanticCodeAttribution {
            site: SemanticCodeSite::Edge(edge),
            operation_ordinal: 0,
            code_offset: 0,
            byte_count: 4,
        }];
        function.structural_return = Some(machine_code::StructuralReturnRecord {
            psi_edge: edge,
            scalar_parameters: Vec::new(),
            parameters: vec![source.clone()],
            parameter_placements: vec![source_placement.clone()],
            source,
            result: terminal_psi::StructuralResultDeclaration {
                place: PlaceId::new(result_place).expect("result place"),
                structural_type,
                multiplicity,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                reference_sources: Vec::new(),
            },
            shape,
            source_placement: source_placement.clone(),
            result_placement: result_placement.clone(),
            returned_claims,
            trivial_affine_locals: Vec::new(),
            trivial_affine_discards: Vec::new(),
            code_offset: 0,
            byte_count: 4,
        });
    };
    let mut plan = two_function_plan();
    install(
        &mut plan.functions[0],
        10,
        StructuralMultiplicity::Affine,
        41,
        42,
        Vec::new(),
    );
    let mut linear = plan.functions[0].clone();
    linear.machine = machine_id(3);
    install(
        &mut linear,
        30,
        StructuralMultiplicity::Linear,
        51,
        52,
        vec![ClaimId::new(31).expect("returned claim")],
    );
    plan.functions.push(linear);
    plan
}

pub(super) fn callback_private_plan() -> machine_code::MachineCodePlanWithPrivateFunctions {
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

pub(super) fn x86_fma_plan(profile: TargetProfile, format: X86ScalarFmaFormat) -> MachineCodePlan {
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

pub(super) fn admitted_x86_fma_provider(profile: TargetProfile) -> AdmittedX86ScalarFmaProvider {
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

pub(super) fn refresh_x86_fma_identity(fragment: &mut machine_code::X86ScalarFmaFragment) {
    fragment.identity = fragment
        .recomputed_identity()
        .expect("mutated test fragment remains structurally identity-bearing");
}

pub(super) fn internal_call_plan(target: NativeTarget) -> MachineCodePlan {
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
