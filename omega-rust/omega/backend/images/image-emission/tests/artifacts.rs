//! Fixtures shared by the artifact tests: object and installation plans,
//! unit call accounts, cleanup plans and identities.

#[path = "artifacts/hosted_exit_runtime.rs"]
mod hosted_exit_runtime;
#[path = "artifacts/installation_field_substitutions.rs"]
mod installation_field_substitutions;
#[path = "artifacts/installation_records.rs"]
mod installation_records;
#[path = "artifacts/installed_artifact.rs"]
mod installed_artifact;
#[path = "artifacts/macho_fixups.rs"]
mod macho_fixups;
#[path = "artifacts/macho_storage.rs"]
mod macho_storage;
#[path = "artifacts/object_custody.rs"]
mod object_custody;
#[path = "artifacts/object_replays.rs"]
mod object_replays;
#[path = "artifacts/provider_execution.rs"]
mod provider_execution;

use function_identity::{MachineFunctionIdentity, StateKey};
use image_emission::{
    InstallationError, decode_installation_record, encode_installation_record,
    installation_fingerprint, validate_installation_record,
};
use installation_evidence::{ComponentProgressAcceptanceEvidence, ProviderExecutionEvidence};
use machine_code::{
    Aarch64ReturnLinkEvidence, BoundarySettlementRecord, InternalCallRelocation,
    InternalUnitCallRecord, MachineCodeFunction, MachineCodePlan, PortEffectRecord,
    ScalarCallStackEvidence, ScalarCleanupPreservationEvidence, ScalarConditionalBranchEvidence,
    ScalarConditionalCondition, ScalarControlAffineCleanupRecord, ScalarControlFlowEvidence,
    ScalarStackEvidence, ScalarStackMutation, ScalarStackMutationKind, SemanticCodeAttribution,
    SemanticCodeSite, StackAdjustmentPair, UnitAffineCleanupRecord, UnitCallStackEvidence,
    UnitParameterHomeRecord, UnitParameterRecord, UnitStackEvidence, X86ScalarFmaFormat,
};
use object_file::object_symbol_name;
use semantic_vocabulary::{
    BoundaryMachineId, ClaimId, EdgeId, MachineId, OperationId, PlaceId, ServiceId,
    StructuralTypeId,
};
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
use terminal_psi::{
    NominalAffineCleanup, SemanticFingerprint, StructuralAccess, StructuralArgument,
    StructuralMultiplicity, TerminalAffineCleanupAction, TerminalPsiIdentity, VocabularyMarker,
};

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

/// One machine per admitted structural-return custody lane beside the ordinary
/// entry machine: machine 1 carries the claim-free affine identity return and
/// machine 3 the claim-bearing linear return. Both records describe the same
/// emitted `mov rax, rdi; ret` interval, so object replay regenerates every
/// byte from the retained record rather than trusting the row.
fn structural_return_plan() -> MachineCodePlan {
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
