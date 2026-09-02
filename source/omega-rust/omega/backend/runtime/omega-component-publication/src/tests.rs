use super::*;

use omega_calling_conventions::{
    ArrivalContextId, ArrivalContextRealization, CallSignature, CallingPolicy, EntryStackEpoch,
    EntryStackRealization, EntryStackStage, MachineRegister, MachineState, MachineStateSet,
    ProviderExitRealization, RegisterSet, StackDomainRef, StateFootprintEvidence,
    ValidatedBoundaryEntryPlan, ValueLocation, ValueShape, evaluate_call_plan,
    evaluate_ordinary_boundary_entry_plan, validate_entry_stack_realization,
};
use omega_effects::provider_plan::{
    ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceMethod,
    ServiceProgressEstablishmentRoute, ServiceProgressEstablishmentRouteKind,
    ServiceProgressPremise, ServiceProgressSubject, ServiceSchema,
};
use omega_effects::{
    CheckedComponentProgressDemand, ComponentEraLedgerId, ComponentProgressManifest,
    ExecutableTcbManifest, ExecutableTcbProfile, ExecutableTcbProfileAcceptance, ExecutionScope,
    IncompleteScopePolicy, ScopeCompleteness, SelectedProviderPlanFacts,
    evaluate_executable_tcb_profile,
};
use omega_executable_installation::{
    AdmissionReceiptId, Artifact, ArtifactAdmissionEvidence, ArtifactEntry, ArtifactId,
    CodePlacementAuthority, CodePlacementId, EntrySetId, FinalValidationCertificate,
    FinalValidationId, InstallAuthority, InstallationAudience, InstallationReceipt,
    InstallationScopeId, InstalledCode, InstalledCodeId, MachineContractSetId, MachineFootprintId,
    MaterializationReceipt, PlacementPlanId, RelocationSetId, WxEnforcement, admit_executable,
    install_validated, materialize_admitted_artifact, materialize_and_freeze,
    validate_final_placement,
};
use omega_external_roots::{
    AcknowledgementPolicyId, AdmittedOpaqueArrivalContextSet, BoundEpochStackComposition,
    ComponentArtifactId, ComponentContractId, ComponentProgressDemandIdentity,
    ComponentProgressReceiptBinding, ComponentProviderId, ComponentVersionPin,
    ComponentVersionPinId, ExternalRootCandidate, ExternalRootDiagnostic, ExternalRootId,
    FixedFuelCall, FixedFuelProviderSummary, FuelProvisionId, FuelScheduleIdentity,
    InstalledProviderOccurrenceId, InstalledRootLedger, LogicalFuelResourceColumn,
    MachineStateResourceColumn, NestingRelationId, OpaqueCallbackProviderId,
    OpaqueCallbackRegistrationCapacityOccurrence, OpaqueCallbackRegistrationCapacityOccurrenceId,
    OpaqueCallbackRegistrationId, OpaqueCallbackRegistrationReceipt,
    OpaqueCallbackRegistrationReceiptId, OpaqueCallbackUnregistrationContractId,
    OpaqueCallbackUnregistrationReceipt, OpaqueCallbackUnregistrationReceiptId,
    OpaqueProviderExitAssurance, ProgressProfileEstablishmentAttestation,
    ProgressProfileEstablishmentReceiptId, ProgressProfileGrantInvocationId, ProviderExecution,
    ProviderExecutionId, ProviderFuelSummaryId, ProviderFuelValidationReceiptId,
    ProviderOccurrenceInstallationReceipt, ProviderOccurrenceInstallationReceiptId,
    ProviderOccurrencePlanBinding, ProviderPlanId, ProviderStackSummary, ResolvedRootServiceReach,
    RootAdmission, RootAdmissionId, RootEffectId, RootProviderId, RootRemovalReceipt,
    RootRemovalReceiptId, RootSlotAuthority, RootSlotId, RootSlotOwnerId, StackNestingRelation,
    StackResourceColumn, StackValidationReceiptId, TrustReceiptId,
    admit_opaque_arrival_context_set, bind_opaque_adapter_stack_realization,
    compose_bound_entry_stack_epochs, compose_fixed_fuel, validate_external_root,
};
use omega_function_identity::{MachineFunctionIdentity, StateKey};
use omega_image_emission::{
    bind_installed_artifact, bind_installed_compiler_private_function_entry,
    build_installation_record_with_evidence, build_object_artifact_with_private_functions,
    emit_executable_image,
};
use omega_installation_evidence::ProviderExecutionEvidence;
use omega_machine_code::{
    BoundarySettlementRecord, CompilerPrivateMachineCodeFunction, MachineCodeFunction,
    MachineCodePlan, MachineCodePlanWithPrivateFunctions, SemanticCodeAttribution,
    SemanticCodeSite,
};
use omega_target_operations::{
    BoundaryRealization, BoundaryScalarArgument, ClaimCompletionOnlyRealization,
    FixedIntegerScalarAbiValue, FixedIntegerScalarFunctionAbi, LinuxExitGroupI32Realization,
    ProviderExecutionBinding, ProviderPlanReportIdentity, ScalarParameterLocation, TargetFunction,
    TargetOperation, TargetOperationPlan, TerminalPsiProvenance,
};
use psi_core::{
    BoundaryMachineId, EdgeId, IntegerSign, IntegerType, MachineId, OperationId, ProfileDecisionId,
    ValueId,
};
use psi_extents::{
    AddressSpaceId, ExtentDiagnostic, ExtentLineageId, ExtentProvenanceId, ExtentRightId,
    ExtentRights, ExtentRootGrant, MappingEraId,
};
use psi_layout_plans::{
    ArtifactInstallationScopeId, EntryStubId, PlacementConstraints, PlacementPhase, PlacementSite,
};
use psi_symbols::SymbolHandle;
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

#[derive(Debug)]
struct TestProviderExecution {
    plan: u64,
    execution: u64,
    report_fingerprint: u64,
    root: u64,
    boundary: u64,
}

impl ProviderExecutionEvidence for TestProviderExecution {
    fn requirement_identity(&self) -> &str {
        "Scheduler::wait#exact"
    }

    fn provider_plan_report_identity(&self) -> u64 {
        self.plan
    }

    fn provider_execution_report_identity(&self) -> u64 {
        self.execution
    }

    fn provider_execution_report_fingerprint(&self) -> u64 {
        self.report_fingerprint
    }

    fn normalized_root_report_identity(&self) -> u64 {
        self.root
    }

    fn boundary_contract_report_fingerprint(&self) -> u64 {
        self.boundary
    }
}

struct RunnableFixture {
    installed_code: InstalledCodeId,
    runnable: InstalledRunnableComponent,
}

fn runnable_fixture(seed: u64) -> RunnableFixture {
    runnable_fixture_at(seed, 0x1000)
}

fn runnable_fixture_at(seed: u64, placement_base: u64) -> RunnableFixture {
    let route = ServiceProgressEstablishmentRoute {
        kind: ServiceProgressEstablishmentRouteKind::BoundaryRequirement,
        requirement_identity: "Scheduler::grant_weak_fair#exact".into(),
    };
    let provider = ProviderPlan {
        name: "scheduler-plan".into(),
        provider_type: "SchedulerProvider".into(),
        provider_type_package_identity: None,
        target: "test".into(),
        schema: ServiceSchema {
            trait_name: "Scheduler".into(),
            trait_package_identity: None,
            methods: vec![
                ServiceMethod {
                    name: "wait".into(),
                    requirement_owner: "Scheduler".into(),
                    requirement_identity: "Scheduler::wait#exact".into(),
                    service_reach: vec!["Scheduler".into()],
                    may_suspend: true,
                    terminates_guarantee: true,
                    termination_premises: vec![ServiceProgressPremise {
                        profile: "SchedulerHandle::WeakFair".into(),
                        subject: ServiceProgressSubject::ProviderReceiver,
                        subject_projections: vec!["queue".into()],
                        establishment_routes: vec![route.clone()],
                    }],
                    ..ServiceMethod::default()
                },
                ServiceMethod {
                    name: "grant_weak_fair".into(),
                    requirement_owner: "Scheduler".into(),
                    requirement_identity: route.requirement_identity.clone(),
                    parameter_count: 1,
                    parameter_type_identities: vec!["SchedulerHandle".into()],
                    has_result: true,
                    result_type_identity: Some("SchedulerHandle in WeakFair".into()),
                    service_reach: vec!["Scheduler".into()],
                    terminates_guarantee: true,
                    ..ServiceMethod::default()
                },
            ],
        },
        rows: vec![
            ProviderPlanRow {
                method: "wait".into(),
                requirement_identity: "Scheduler::wait#exact".into(),
                requirement_lifetime_partition: Vec::new(),
                binding: ProviderBinding::CompilerIntrinsic {
                    machine: "TestScheduler::wait".into(),
                },
            },
            ProviderPlanRow {
                method: "grant_weak_fair".into(),
                requirement_identity: route.requirement_identity.clone(),
                requirement_lifetime_partition: Vec::new(),
                binding: ProviderBinding::CompilerIntrinsic {
                    machine: "TestScheduler::grant_weak_fair".into(),
                },
            },
        ],
        origin_package_identity: None,
        origin_package: "omega::test".into(),
    };
    let provider_plan = provider.report_fingerprint();
    let selected =
        SelectedProviderPlanFacts::from_selection(&[provider], &["scheduler-plan".into()])
            .expect("selected provider plan");
    let manifest = ComponentProgressManifest::bind(
        "Application::start".into(),
        &selected,
        vec![CheckedComponentProgressDemand {
            provider_service_identity: "Scheduler".into(),
            provider_service_package_identity: None,
            requirement_identity: "Scheduler::wait#exact".into(),
            requirement_owner_package_identity: None,
            profile_identity: "SchedulerHandle::WeakFair".into(),
            subject_projections: vec!["queue".into()],
            origin_callable_identity: "Application::start".into(),
            origin_state_identity: "Application::start::entry".into(),
            statement_ordinal: 1,
            call_ordinal: 0,
        }],
    )
    .expect("component progress manifest");

    let provider_execution = TestProviderExecution {
        plan: provider_plan,
        execution: seed + 1,
        report_fingerprint: seed + 2,
        root: seed + 3,
        boundary: seed + 4,
    };
    let (object, image) = terminal_image(&provider_execution);
    let mut installed = install_terminal_text(&object, seed + 20, seed + 21, placement_base);
    let installed_code = installed.identity();
    let mut root_ledger =
        InstalledRootLedger::claim(&mut installed).expect("installation registry");
    let occurrence = root_id(
        seed + 30,
        InstalledProviderOccurrenceId::from_normalized_identity,
    );
    root_ledger
        .seal_provider_occurrence_closure(
            &selected,
            [ProviderOccurrencePlanBinding::new(
                provider_plan,
                selected
                    .plan_by_report_fingerprint(provider_plan)
                    .expect("selected fixture plan")
                    .clone(),
                ProviderOccurrenceInstallationReceipt::from_provider(
                    root_id(
                        seed + 31,
                        ProviderOccurrenceInstallationReceiptId::from_normalized_identity,
                    ),
                    &installed,
                    occurrence,
                    "SchedulerProvider",
                ),
            )],
        )
        .expect("provider occurrence closure");
    let progress_receipt = root_ledger
        .admit_progress_profile_establishment(
            ProgressProfileEstablishmentAttestation::from_provider(
                root_id(
                    seed + 32,
                    ProgressProfileEstablishmentReceiptId::from_normalized_identity,
                ),
                &installed,
                occurrence,
                occurrence,
                provider_plan,
                root_id(
                    seed + 33,
                    ProgressProfileGrantInvocationId::from_normalized_identity,
                ),
                "SchedulerHandle::WeakFair",
                vec!["queue".into()],
                route,
            ),
        )
        .expect("progress establishment receipt");
    let progress = root_ledger
        .seal_component_progress(
            manifest.clone(),
            [ComponentProgressReceiptBinding::new(
                ComponentProgressDemandIdentity::from_demand(&manifest.pending()[0]),
                progress_receipt,
            )],
        )
        .expect("component progress acceptance");
    let installation = build_installation_record_with_evidence(
        &image,
        ProfileDecisionId::new(seed + 40).expect("profile decision"),
        [&provider_execution],
        Some(&progress),
    )
    .expect("terminal installation record");
    let artifact = bind_installed_artifact(object, image, installation, installed)
        .expect("installed terminal artifact");
    let runnable = bind_installed_runnable_component(artifact, root_ledger, Some(progress))
        .expect("installed runnable component");
    RunnableFixture {
        installed_code,
        runnable,
    }
}

fn terminal_image(
    execution: &TestProviderExecution,
) -> (
    omega_image_emission::ObjectArtifact,
    omega_image_emission::ExecutableImage,
) {
    let machine = MachineId::new(1).expect("machine");
    let provider_operation = OperationId::new(1).expect("provider operation");
    let exit_operation = OperationId::new(2).expect("exit operation");
    let edge = EdgeId::new(1).expect("edge");
    let bytes = omega_isa_x86_64::encode_linux_exit_group_i32(0);
    let provider = ProviderExecutionBinding::from_execution_record(
        ProviderPlanReportIdentity::new(execution.plan).expect("provider plan"),
        execution.execution,
        execution.report_fingerprint,
        execution.root,
        execution.boundary,
    )
    .expect("provider execution binding");
    let scalar_type = psi_core::ScalarType::Integer(
        psi_core::IntegerType::new(psi_core::IntegerSign::Signed, 32).expect("i32"),
    );
    let plan = MachineCodePlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
        },
        target: omega_target::NativeTarget::linux_x64(),
        entry: machine,
        functions: vec![MachineCodeFunction {
            machine,
            attachment: None,
            fixed_integer_scalar_abi: None,
            mixed_structural_scalar_abi: None,
            structural_call_scalar_return: None,
            unit_scalar_abi: None,
            provenance: TerminalPsiProvenance {
                operations: vec![provider_operation, exit_operation],
                edges: vec![edge],
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
            internal_unit_scalar_calls: Vec::new(),
            installed_provider_unit_scalar_calls: Vec::new(),
            dynamic_calls: Vec::new(),
            dynamic_parameter_calls: Vec::new(),
            forwarded_dynamic_descriptor_calls: Vec::new(),
            unit_scalar_homes: Vec::new(),
            unit_integer_constants: Vec::new(),
            unit_affine_scalar_records: Vec::new(),
            unit_structural_scalar_field_stores: Vec::new(),
            scalar_structural_scalar_field_stores: Vec::new(),
            unit_affine_cleanup: None,
            semantic_code_attribution: vec![
                SemanticCodeAttribution {
                    site: SemanticCodeSite::Operation(provider_operation),
                    operation_ordinal: 0,
                    code_offset: 0,
                    byte_count: 0,
                },
                SemanticCodeAttribution {
                    site: SemanticCodeSite::Operation(exit_operation),
                    operation_ordinal: 1,
                    code_offset: 0,
                    byte_count: bytes.len(),
                },
                SemanticCodeAttribution {
                    site: SemanticCodeSite::Edge(edge),
                    operation_ordinal: 2,
                    code_offset: bytes.len(),
                    byte_count: 0,
                },
            ],
            port_effects: Vec::new(),
            boundary_settlements: vec![
                BoundarySettlementRecord {
                    psi_operation: provider_operation,
                    boundary: BoundaryMachineId::new(1).expect("provider boundary"),
                    execution: omega_machine_code::BoundaryExecutionRecord::AdmittedProvider(
                        provider.into(),
                    ),
                    realization: BoundaryRealization::ClaimCompletionOnly(
                        ClaimCompletionOnlyRealization,
                    ),
                    scalar_arguments: Vec::new(),
                    runtime_scalar_arguments: Vec::new(),
                    arguments: Vec::new(),
                    byte_sequence_arguments: Vec::new(),
                    completion_claim_sources: Vec::new(),
                    completion_receipts: Vec::new(),
                    completion_provider_custody: Vec::new(),
                    native_result: None,
                    operation_ordinal: 0,
                    code_offset: 0,
                    byte_count: 0,
                },
                BoundarySettlementRecord {
                    psi_operation: exit_operation,
                    boundary: BoundaryMachineId::new(2).expect("exit boundary"),
                    execution: omega_machine_code::BoundaryExecutionRecord::CompilerBuiltin(
                        omega_target_operations::CompilerBuiltinExecution::LinuxExitGroupI32,
                    ),
                    realization: BoundaryRealization::LinuxExitGroupI32(
                        LinuxExitGroupI32Realization,
                    ),
                    scalar_arguments: vec![BoundaryScalarArgument {
                        source_value: psi_core::ValueId::new(1).expect("value"),
                        scalar_type,
                        immediate: psi_core::IntegerValue::Signed(0),
                        destination: omega_calling_conventions::MachineRegister::X86Rdi,
                    }],
                    runtime_scalar_arguments: Vec::new(),
                    arguments: Vec::new(),
                    byte_sequence_arguments: Vec::new(),
                    completion_claim_sources: Vec::new(),
                    completion_receipts: Vec::new(),
                    completion_provider_custody: Vec::new(),
                    native_result: None,
                    operation_ordinal: 1,
                    code_offset: 0,
                    byte_count: bytes.len(),
                },
            ],
            scalar_affine_cleanup: None,
            scalar_control_affine_cleanups: Vec::new(),
            scalar_structural_parameters: Vec::new(),
            scalar_structural_parameter_homes: Vec::new(),
            ranked_u32_countdown: None,
            structural_return: None,
        }],
    };
    let private_plan = callback_private_operation_plan();
    let private_source_psi = private_plan.psi;
    let assigned_private =
        omega_target_operations_to_assigned_target_operations::assign_registers(&private_plan)
            .expect("private callback register assignment");
    let private_machine = omega_machine_emission::emit_machine_code(&assigned_private)
        .expect("private callback machine emission");
    let [private_function] = private_machine.functions.as_slice() else {
        panic!("one private callback machine function expected")
    };
    let private_identity = callback_private_function_identity();
    let object =
        build_object_artifact_with_private_functions(&MachineCodePlanWithPrivateFunctions {
            plan,
            private_functions: vec![CompilerPrivateMachineCodeFunction {
                identity: private_identity,
                private_symbol: "__omega_component_callback".into(),
                source_psi: private_source_psi,
                function: private_function.clone(),
            }],
        })
        .expect("terminal object with private callback");
    let image = emit_executable_image(&object, 3).expect("terminal image");
    (object, image)
}

fn callback_private_operation_plan() -> TargetOperationPlan {
    let target = omega_target::NativeTarget::linux_x64();
    let machine = MachineId::new(2).expect("private machine");
    let parameter = ValueId::new(20).expect("private parameter");
    let result = ValueId::new(21).expect("private result");
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let shape = ValueShape::integer(8, 8);
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![shape],
            result: Some(shape),
        },
    )
    .expect("private callback ABI");
    let parameter_placement = call_plan.parameters[0].clone();
    let result_placement = call_plan.result.clone().expect("private result placement");
    let location = match parameter_placement.locations.as_slice() {
        [ValueLocation::Register { register, .. }] => ScalarParameterLocation::Register(*register),
        _ => panic!("one-u64 parameter must use one register"),
    };
    TargetOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0x52; 32]),
        },
        target,
        entry: machine,
        functions: vec![TargetFunction {
            machine,
            attachment: None,
            fixed_integer_scalar_abi: Some(FixedIntegerScalarFunctionAbi {
                call_plan,
                parameters: vec![FixedIntegerScalarAbiValue {
                    value: parameter,
                    scalar_type,
                    placement: parameter_placement,
                }],
                result: FixedIntegerScalarAbiValue {
                    value: result,
                    scalar_type,
                    placement: result_placement,
                },
            }),
            mixed_structural_scalar_abi: None,
            provenance: TerminalPsiProvenance {
                operations: Vec::new(),
                edges: vec![EdgeId::new(2).expect("private return edge")],
            },
            operation: TargetOperation::ReturnIntegerParameter {
                psi_edge: EdgeId::new(2).expect("private return edge"),
                source_value: result,
                scalar_type,
                parameter_index: 0,
                location,
            },
        }],
    }
}

fn callback_private_function_identity() -> MachineFunctionIdentity {
    MachineFunctionIdentity::callback_thunk(
        StateKey {
            machine: SymbolHandle::from_parts(91, 2),
            state: SymbolHandle::from_parts(93, 3),
            segment_index: 5,
        },
        0,
    )
    .expect("private callback identity")
}

fn install_terminal_text(
    object: &omega_image_emission::ObjectArtifact,
    artifact_identity: u64,
    installed_identity: u64,
    placement_base: u64,
) -> InstalledCode {
    let scope = ArtifactInstallationScopeId::from_normalized_identity(1).expect("scope");
    let constraints = PlacementConstraints::new(None, 16, PlacementPhase::Load, None, Some(scope))
        .expect("constraints");
    let entry = EntryStubId::from_normalized_identity(1).expect("entry");
    let private_entry = EntryStubId::from_normalized_identity(2).expect("private entry");
    let [private_function] = object.private_functions() else {
        panic!("one private callback expected")
    };
    let contracts = install_id(2, MachineContractSetId::from_normalized_identity);
    let footprint = install_id(3, MachineFootprintId::from_normalized_identity);
    let artifact = Artifact::from_canonical_decode(
        install_id(artifact_identity, ArtifactId::from_normalized_identity),
        object.target().architecture,
        object.text_bytes().to_vec(),
        contracts,
        footprint,
        install_id(4, PlacementPlanId::from_normalized_identity),
        constraints,
        install_id(5, EntrySetId::from_normalized_identity),
        vec![
            ArtifactEntry::from_canonical_decode(entry, 0),
            ArtifactEntry::from_canonical_decode(
                private_entry,
                u64::try_from(private_function.function.text_offset)
                    .expect("private callback offset"),
            ),
        ],
        install_id(6, RelocationSetId::from_normalized_identity),
        Vec::new(),
        omega_executable_installation::ArtifactAuthorityCommitments::from_canonical_evidence(
            contracts,
            b"test-machine-contracts-v1",
            footprint,
            b"test-machine-footprint-v1",
            None,
            Some((scope, b"test-installation-scope-v1")),
        ),
    )
    .expect("artifact");
    let admitted = admit_executable(
        &artifact,
        ArtifactAdmissionEvidence::from_validator(
            install_id(7, AdmissionReceiptId::from_normalized_identity),
            &artifact,
            true,
        ),
    )
    .expect("admitted artifact");
    let rights = ExtentRights::from_normalized_identities([extent_id(
        1,
        ExtentRightId::from_normalized_identity,
    )]);
    let extent = ExtentRootGrant::from_admitted_provider(
        psi_extents::ExtentProviderIssuance::from_normalized_identities([
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13,
        ])
        .expect("extent issuance"),
        extent_id(2, ExtentLineageId::from_normalized_identity),
        extent_id(3, AddressSpaceId::from_normalized_identity),
        rights.clone(),
        extent_id(4, ExtentProvenanceId::from_normalized_identity),
        extent_id(5, MappingEraId::from_normalized_identity),
    )
    .mint(placement_base, 4096)
    .expect("placement extent");
    let placement = CodePlacementAuthority::from_admitted_provider(
        install_id(8, CodePlacementId::from_normalized_identity),
        install_id(1, InstallationScopeId::from_normalized_identity),
        InstallationAudience::DormantLocal,
        &extent,
        rights,
        constraints,
        PlacementSite {
            base_address: placement_base,
            phase: PlacementPhase::Load,
            machine_regime: None,
            installation_scope: Some(scope),
        },
    )
    .claim(extent)
    .expect("placement");
    let materialized = materialize_admitted_artifact(&admitted, &placement, |_| None)
        .expect("materialized artifact");
    let frozen = materialize_and_freeze(
        &admitted,
        placement,
        materialized.clone(),
        MaterializationReceipt::from_materialized(
            &materialized,
            install_id(9, MachineFootprintId::from_normalized_identity),
            true,
        ),
    )
    .expect("frozen artifact");
    let validation = FinalValidationCertificate::from_validator(
        install_id(10, FinalValidationId::from_normalized_identity),
        &frozen,
        true,
    );
    let validated = validate_final_placement(frozen, &validation).expect("validated artifact");
    let authority = InstallAuthority::from_admitted_provider(&validated);
    let receipt = InstallationReceipt::from_provider(
        install_id(
            installed_identity,
            InstalledCodeId::from_normalized_identity,
        ),
        &validated,
        true,
        WxEnforcement::HardwareEnforced,
    );
    install_validated(validated, authority, receipt).expect("installed code")
}

fn tcb_acceptance(name: &str, closure: u64) -> ExecutableTcbProfileAcceptance {
    evaluate_executable_tcb_profile(
        &ExecutableTcbManifest {
            known_entries: Vec::new(),
            completeness: ScopeCompleteness::Complete {
                scope: ExecutionScope::CallerAddressSpace,
                selected_provider_closure_report_identity: closure,
                opaque_closure_evidence: Vec::new(),
                runtime_closure_evidence: Vec::new(),
            },
        },
        &ExecutableTcbProfile {
            name: name.into(),
            scope: ExecutionScope::CallerAddressSpace,
            allow_static_current_artifact_checked_bodies: true,
            exact_allowances: Vec::new(),
            incomplete_scope: IncompleteScopePolicy::Reject,
        },
    )
    .expect("TCB acceptance")
}

fn lifecycle() -> RunnableComponentEraLedger {
    RunnableComponentEraLedger::new(
        ComponentEraEntryLedger::new(
            ComponentEraLedgerId::from_normalized_identity(1).expect("ledger"),
            "CodecBinding/v1".into(),
            "CodecEntry/v1".into(),
            2,
            tcb_acceptance("platform", 1),
        )
        .expect("component lifecycle"),
    )
}

fn candidate(era: u64, runnable: &InstalledRunnableComponent) -> ComponentEraCandidate {
    ComponentEraCandidate {
        era_identity: era,
        artifact_occurrence_digest: runnable.installed().occurrence_digest(),
        artifact_instance_compatibility_report_identity: runnable
            .installed_code()
            .normalized_identity(),
        binding_contract_identity: "CodecBinding/v1".into(),
        entry_contract_identity: "CodecEntry/v1".into(),
        entry_plan_identity: format!("entry-plan:{era}"),
        entry_plan_admission_receipt_identity: format!("entry-plan-receipt:{era}"),
        executable_tcb_acceptance: tcb_acceptance(format!("era-{era}").as_str(), era),
    }
}

#[test]
fn runnable_publication_retains_opaque_progress_until_successful_retirement() {
    let first = runnable_fixture(10_000);
    let second = runnable_fixture(20_000);
    let mut ledger = lifecycle();

    let first_candidate = candidate(10, &first.runnable);
    let first_receipt = ComponentEraPublicationReceipt::from_runtime(
        100,
        ledger.lifecycle(),
        &first_candidate,
        true,
        false,
    );
    ledger
        .publish(first_candidate, first_receipt, first.runnable)
        .expect("first runnable era");
    assert!(ledger.retained_component(10).unwrap().progress().is_some());

    let rejected_retirement =
        ComponentEraRetirementReceipt::from_runtime(200, ledger.lifecycle(), 10, true);
    let error = ledger
        .retire(rejected_retirement)
        .expect_err("current era cannot retire");
    assert!(error.diagnostic().contains("noncurrent"));
    assert!(ledger.retained_component(10).is_some());

    let second_candidate = candidate(11, &second.runnable);
    let second_receipt = ComponentEraPublicationReceipt::from_runtime(
        101,
        ledger.lifecycle(),
        &second_candidate,
        true,
        true,
    );
    ledger
        .publish(second_candidate, second_receipt, second.runnable)
        .expect("replacement runnable era");
    let quiescence = ComponentEraQuiescenceReceipt::from_runtime(ledger.lifecycle(), 10, 0, true);
    ledger
        .establish_quiescence(quiescence)
        .expect("old era quiescent");
    let retirement = ComponentEraRetirementReceipt::from_runtime(201, ledger.lifecycle(), 10, true);
    let retired = ledger.retire(retirement).expect("retire old runnable era");
    assert!(retired.progress().is_some());
    assert_eq!(retired.installed().identity(), first.installed_code);
    assert!(ledger.retained_component(10).is_none());
    assert_eq!(
        ledger
            .retained_component(11)
            .unwrap()
            .installed()
            .identity(),
        second.installed_code
    );
    let (artifact, _, progress) = retired.into_parts();
    let (_, _, _, installed) = artifact.into_parts();
    assert!(progress.is_some());
    assert_eq!(installed.identity(), first.installed_code);
}

#[test]
fn runnable_publication_rejection_returns_candidate_receipt_and_opaque_evidence() {
    let fixture = runnable_fixture(30_000);
    let mut ledger = lifecycle();
    let mut wrong = candidate(10, &fixture.runnable);
    wrong.artifact_instance_compatibility_report_identity ^= 1;
    let receipt =
        ComponentEraPublicationReceipt::from_runtime(300, ledger.lifecycle(), &wrong, true, false);
    let error = ledger
        .publish(wrong, receipt, fixture.runnable)
        .expect_err("artifact occurrence substitution rejects");
    assert!(error.diagnostic().contains("different installed artifact"));
    let (_, _, runnable) = error.into_parts();
    assert!(runnable.progress().is_some());
    assert_eq!(runnable.installed().identity(), fixture.installed_code);
    assert_eq!(ledger.current_era(), None);
}

#[test]
fn runnable_publication_rejects_compact_equal_different_artifact_occurrence() {
    let fixture = runnable_fixture(31_000);
    let substituted = runnable_fixture(32_000);
    let mut ledger = lifecycle();
    let mut wrong = candidate(10, &fixture.runnable);
    wrong.artifact_occurrence_digest = substituted.runnable.installed().occurrence_digest();
    assert_eq!(
        wrong.artifact_instance_compatibility_report_identity,
        fixture.installed_code.normalized_identity()
    );
    assert_ne!(
        wrong.artifact_occurrence_digest,
        fixture.runnable.installed().occurrence_digest()
    );
    let receipt =
        ComponentEraPublicationReceipt::from_runtime(301, ledger.lifecycle(), &wrong, true, false);

    let error = ledger
        .publish(wrong, receipt, fixture.runnable)
        .expect_err("compact-equal artifact occurrence substitution rejects");
    assert!(error.diagnostic().contains("different installed artifact"));
    assert_eq!(ledger.current_era(), None);
}

#[test]
fn registered_private_callback_preserves_exact_runtime_custody_through_retry() {
    let private_entry = EntryStubId::from_normalized_identity(2).expect("private entry");
    let process_entry = EntryStubId::from_normalized_identity(1).expect("process entry");
    let private_function = callback_private_function_identity();
    let mut fixture = runnable_fixture(800);
    let other = runnable_fixture_at(800, 0x9000);
    let attribution = bind_installed_compiler_private_function_entry(
        fixture.runnable.installed_artifact(),
        private_function,
        private_entry,
    )
    .expect("private callback attribution");
    let other_attribution = bind_installed_compiler_private_function_entry(
        other.runnable.installed_artifact(),
        private_function,
        private_entry,
    )
    .expect("other installed occurrence attribution");
    assert!(
        bind_installed_compiler_private_function_entry(
            fixture.runnable.installed_artifact(),
            private_function,
            process_entry,
        )
        .expect_err("process entry cannot substitute for private callback")
        .diagnostic()
        .contains("text offset")
    );

    let candidate = callback_root_candidate(fixture.runnable.installed(), private_entry);
    let boundary = callback_boundary();
    let validated = validate_external_root(candidate, &boundary).expect("callback root");
    let slot = RootSlotAuthority::from_admitted_owner(
        root_id(720, RootSlotId::from_normalized_identity),
        root_id(721, RootSlotOwnerId::from_normalized_identity),
    );
    let execution = ProviderExecution::from_admitted_provider(
        root_id(754, ProviderExecutionId::from_normalized_identity),
        &validated,
        Some(OpaqueProviderExitAssurance::AcceptedClaim {
            realization: ProviderExitRealization {
                control: validated.boundary().call.entry_control,
                restored_state: validated.boundary().state.restored_state,
            },
            validation_receipt: root_id(704, TrustReceiptId::from_normalized_identity),
        }),
    )
    .expect("callback provider execution");
    let admission = RootAdmission::from_admitted_provider(
        root_id(722, RootAdmissionId::from_normalized_identity),
        &validated,
        &execution,
        fixture.runnable.installed(),
        &slot,
        validated.candidate().trust_receipts.iter().copied(),
    )
    .expect("callback root admission");
    let mut runtime = fixture.runnable.external_root_runtime();
    let root = runtime
        .install(validated, slot, admission)
        .expect("installed callback root");
    let root_identity = root.root();
    let not_quiesced = RootRemovalReceipt::from_provider(
        root_id(780, RootRemovalReceiptId::from_normalized_identity),
        &root,
        true,
        false,
    );
    let quiesced = RootRemovalReceipt::from_provider(
        root_id(781, RootRemovalReceiptId::from_normalized_identity),
        &root,
        true,
        true,
    );
    let provider = root_id(784, OpaqueCallbackProviderId::from_normalized_identity);
    let capacity_identity = root_id(
        789,
        OpaqueCallbackRegistrationCapacityOccurrenceId::from_normalized_identity,
    );
    let capacity =
        OpaqueCallbackRegistrationCapacityOccurrence::from_provider(capacity_identity, provider);
    let failed_provider_receipt = OpaqueCallbackRegistrationReceipt::from_provider(
        root_id(
            782,
            OpaqueCallbackRegistrationReceiptId::from_normalized_identity,
        ),
        root_id(783, OpaqueCallbackRegistrationId::from_normalized_identity),
        provider,
        root_id(
            785,
            OpaqueCallbackUnregistrationContractId::from_normalized_identity,
        ),
        &root,
        &capacity,
        false,
    );
    let error = runtime
        .admit_compiler_private_callback(other_attribution, root, failed_provider_receipt, capacity)
        .expect_err("cross-occurrence attribution rejects before provider success");
    let (other_attribution, root, _failed_provider_receipt, capacity) = (*error).into_parts();
    assert_eq!(other_attribution.entry(), private_entry);
    assert_eq!(capacity.identity(), capacity_identity);
    assert_eq!(root.root(), root_identity);

    let failed_provider_receipt = OpaqueCallbackRegistrationReceipt::from_provider(
        root_id(
            786,
            OpaqueCallbackRegistrationReceiptId::from_normalized_identity,
        ),
        root_id(783, OpaqueCallbackRegistrationId::from_normalized_identity),
        provider,
        root_id(
            785,
            OpaqueCallbackUnregistrationContractId::from_normalized_identity,
        ),
        &root,
        &capacity,
        false,
    );
    let error = runtime
        .admit_compiler_private_callback(attribution, root, failed_provider_receipt, capacity)
        .expect_err("provider rejection establishes no registration");
    let (attribution, root, _failed_provider_receipt, capacity) = (*error).into_parts();
    assert_eq!(attribution.entry(), private_entry);
    assert_eq!(capacity.identity(), capacity_identity);

    let accepted_receipt = OpaqueCallbackRegistrationReceipt::from_provider(
        root_id(
            787,
            OpaqueCallbackRegistrationReceiptId::from_normalized_identity,
        ),
        root_id(783, OpaqueCallbackRegistrationId::from_normalized_identity),
        provider,
        root_id(
            785,
            OpaqueCallbackUnregistrationContractId::from_normalized_identity,
        ),
        &root,
        &capacity,
        true,
    );
    let collision_capacity =
        OpaqueCallbackRegistrationCapacityOccurrence::from_provider(capacity_identity, provider);
    let error = runtime
        .admit_compiler_private_callback(attribution, root, accepted_receipt, collision_capacity)
        .expect_err("collision-equal capacity occurrence rejects");
    let (attribution, root, accepted_receipt, collision_capacity) = (*error).into_parts();
    assert_eq!(collision_capacity.identity(), capacity_identity);
    let registration = runtime
        .admit_compiler_private_callback(attribution, root, accepted_receipt, capacity)
        .expect("exact provider registration");
    assert_eq!(registration.attribution().entry(), private_entry);
    assert_eq!(
        registration.registration().capacity().identity(),
        capacity_identity
    );

    let unsuccessful = OpaqueCallbackUnregistrationReceipt::from_provider(
        root_id(
            788,
            OpaqueCallbackUnregistrationReceiptId::from_normalized_identity,
        ),
        registration.registration(),
        false,
    );
    let error = registration
        .unregister_and_quiesce(&mut runtime, unsuccessful, not_quiesced)
        .expect_err("unsuccessful unregister retains callback custody");
    let (registration, _, not_quiesced) = (*error).into_parts();
    assert_eq!(registration.attribution().entry(), private_entry);
    assert_eq!(
        registration.registration().capacity().identity(),
        capacity_identity
    );

    let successful = OpaqueCallbackUnregistrationReceipt::from_provider(
        root_id(
            790,
            OpaqueCallbackUnregistrationReceiptId::from_normalized_identity,
        ),
        registration.registration(),
        true,
    );
    let error = registration
        .unregister_and_quiesce(&mut runtime, successful, not_quiesced)
        .expect_err("provider success without quiescence retains callback custody");
    let (registration, _, _) = (*error).into_parts();
    assert_eq!(registration.attribution().entry(), private_entry);

    let successful = OpaqueCallbackUnregistrationReceipt::from_provider(
        root_id(
            791,
            OpaqueCallbackUnregistrationReceiptId::from_normalized_identity,
        ),
        registration.registration(),
        true,
    );
    let completed = registration
        .unregister_and_quiesce(&mut runtime, successful, quiesced)
        .expect("provider unregister plus root quiescence");
    let (attribution, completion) = completed.into_parts();
    assert_eq!(attribution.entry(), private_entry);
    let (returned_slot, returned_capacity) = completion.into_parts();
    assert_eq!(
        returned_slot.slot(),
        root_id(720, RootSlotId::from_normalized_identity)
    );
    assert_eq!(returned_capacity.identity(), capacity_identity);
}

#[test]
fn registered_private_callback_rejects_a_different_admitted_root_entry() {
    let private_entry = EntryStubId::from_normalized_identity(2).expect("private entry");
    let process_entry = EntryStubId::from_normalized_identity(1).expect("process entry");
    let mut fixture = runnable_fixture(900);
    let attribution = bind_installed_compiler_private_function_entry(
        fixture.runnable.installed_artifact(),
        callback_private_function_identity(),
        private_entry,
    )
    .expect("private callback attribution");
    let candidate = callback_root_candidate(fixture.runnable.installed(), process_entry);
    let boundary = callback_boundary();
    let validated = validate_external_root(candidate, &boundary).expect("process-entry root");
    let slot = RootSlotAuthority::from_admitted_owner(
        root_id(820, RootSlotId::from_normalized_identity),
        root_id(821, RootSlotOwnerId::from_normalized_identity),
    );
    let execution = ProviderExecution::from_admitted_provider(
        root_id(854, ProviderExecutionId::from_normalized_identity),
        &validated,
        Some(OpaqueProviderExitAssurance::AcceptedClaim {
            realization: ProviderExitRealization {
                control: validated.boundary().call.entry_control,
                restored_state: validated.boundary().state.restored_state,
            },
            validation_receipt: root_id(704, TrustReceiptId::from_normalized_identity),
        }),
    )
    .expect("process-entry provider execution");
    let admission = RootAdmission::from_admitted_provider(
        root_id(822, RootAdmissionId::from_normalized_identity),
        &validated,
        &execution,
        fixture.runnable.installed(),
        &slot,
        validated.candidate().trust_receipts.iter().copied(),
    )
    .expect("process-entry root admission");
    let mut runtime = fixture.runnable.external_root_runtime();
    let root = runtime
        .install(validated, slot, admission)
        .expect("installed process-entry root");
    let provider = root_id(884, OpaqueCallbackProviderId::from_normalized_identity);
    let capacity = OpaqueCallbackRegistrationCapacityOccurrence::from_provider(
        root_id(
            889,
            OpaqueCallbackRegistrationCapacityOccurrenceId::from_normalized_identity,
        ),
        provider,
    );
    let receipt = OpaqueCallbackRegistrationReceipt::from_provider(
        root_id(
            882,
            OpaqueCallbackRegistrationReceiptId::from_normalized_identity,
        ),
        root_id(883, OpaqueCallbackRegistrationId::from_normalized_identity),
        provider,
        root_id(
            885,
            OpaqueCallbackUnregistrationContractId::from_normalized_identity,
        ),
        &root,
        &capacity,
        true,
    );
    let error = runtime
        .admit_compiler_private_callback(attribution, root, receipt, capacity)
        .expect_err("an admitted process entry cannot substitute for the private entry");
    let (attribution, root, _, capacity) = (*error).into_parts();
    assert_eq!(attribution.entry(), private_entry);
    assert_eq!(
        root.root(),
        root_id(701, ExternalRootId::from_normalized_identity)
    );
    assert_eq!(capacity.provider(), provider);
}

fn journal_acceptance() -> ComponentDeploymentAcceptanceSnapshot {
    ComponentDeploymentAcceptanceSnapshot::new(
        "envelope:codec-v1",
        b"canonical-envelope-v1".to_vec(),
        vec![
            ComponentDeploymentAdmissionRecord::new(
                "opaque-native",
                "CodecEntry/v1",
                "admission:7",
            )
            .expect("admission row"),
        ],
    )
    .expect("acceptance snapshot")
}

#[test]
fn deployment_journal_roundtrips_all_phases_and_leaves_recovery_policy_open() {
    let fixture = runnable_fixture(40_000);
    let fixture_occurrence_digest = *fixture.runnable.installed().occurrence_digest().as_bytes();
    let mut ledger = lifecycle();
    let current_candidate = candidate(20, &fixture.runnable);
    let receipt = ComponentEraPublicationReceipt::from_runtime(
        400,
        ledger.lifecycle(),
        &current_candidate,
        true,
        false,
    );
    let prepared = prepare_component_deployment(
        900,
        &ledger,
        current_candidate,
        receipt,
        fixture.runnable,
        journal_acceptance(),
    )
    .expect("prepared deployment");
    let prepared_bytes =
        encode_component_deployment_journal(prepared.record()).expect("canonical Prepared journal");
    let decoded =
        decode_component_deployment_journal(&prepared_bytes).expect("decode Prepared journal");
    assert_eq!(&decoded, prepared.record());
    assert_eq!(
        decoded.candidate().artifact_occurrence_digest(),
        fixture_occurrence_digest
    );
    assert_eq!(
        reconcile_component_deployment_restart(&decoded, 900, "CodecBinding/v1", "CodecEntry/v1",)
            .expect("Prepared reconciliation"),
        ComponentDeploymentRestartReconciliation::PolicyRequired {
            phase: ComponentDeploymentJournalPhase::Prepared,
            choices: vec![ComponentDeploymentRecoveryChoice::RollForwardCandidate],
        }
    );

    let activated = prepared
        .activate(&decoded, &mut ledger)
        .expect("activate exact durable predecessor");
    let activated_bytes = encode_component_deployment_journal(activated.record())
        .expect("canonical Activated journal");
    let activated_durable =
        decode_component_deployment_journal(&activated_bytes).expect("decode Activated journal");
    assert!(matches!(
        reconcile_component_deployment_restart(
            &activated_durable,
            900,
            "CodecBinding/v1",
            "CodecEntry/v1",
        ),
        Ok(ComponentDeploymentRestartReconciliation::PolicyRequired {
            phase: ComponentDeploymentJournalPhase::Activated,
            ..
        })
    ));

    let finalized = activated
        .finalize(&activated_durable, &ledger)
        .expect("finalize exact live occurrence");
    let finalized_bytes = encode_component_deployment_journal(finalized.record())
        .expect("canonical Finalized journal");
    let finalized_durable =
        decode_component_deployment_journal(&finalized_bytes).expect("decode Finalized journal");
    assert!(matches!(
        reconcile_component_deployment_restart(
            &finalized_durable,
            900,
            "CodecBinding/v1",
            "CodecEntry/v1",
        ),
        Ok(ComponentDeploymentRestartReconciliation::Complete { .. })
    ));

    let next = runnable_fixture(45_000);
    let next_candidate = candidate(21, &next.runnable);
    let next_receipt = ComponentEraPublicationReceipt::from_runtime(
        401,
        ledger.lifecycle(),
        &next_candidate,
        true,
        true,
    );
    let next_prepared = prepare_component_deployment(
        901,
        &ledger,
        next_candidate,
        next_receipt,
        next.runnable,
        journal_acceptance(),
    )
    .expect("replacement preparation derives current slot history");
    assert_eq!(
        next_prepared
            .record()
            .prior()
            .map(|value| value.era_identity()),
        Some(20)
    );
    assert_eq!(next_prepared.record().live_eras_before().len(), 1);
    assert_eq!(
        next_prepared.record().live_eras_before()[0]
            .occurrence()
            .era_identity(),
        20
    );
}

#[test]
fn deployment_journal_rejects_tamper_and_failed_activation_returns_custody() {
    let fixture = runnable_fixture(50_000);
    let mut ledger = lifecycle();
    let wrong = candidate(30, &fixture.runnable);
    let receipt =
        ComponentEraPublicationReceipt::from_runtime(500, ledger.lifecycle(), &wrong, false, false);
    let prepared = prepare_component_deployment(
        901,
        &ledger,
        wrong,
        receipt,
        fixture.runnable,
        journal_acceptance(),
    )
    .expect("preparation records candidate before publication");
    let durable = prepared.record().clone();
    let error = prepared
        .activate(&durable, &mut ledger)
        .expect_err("runtime receipt that did not publish rejects activation");
    assert!(!error.diagnostic().is_empty());
    let recovered = error.into_prepared();
    assert_eq!(recovered.record(), &durable);
    assert_eq!(ledger.current_era(), None);

    let mut bytes = encode_component_deployment_journal(&durable).expect("canonical journal bytes");
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    assert!(decode_component_deployment_journal(&bytes).is_err());
    assert!(
        reconcile_component_deployment_restart(&durable, 902, "CodecBinding/v1", "CodecEntry/v1",)
            .is_err()
    );
}

#[test]
fn deployment_journal_finalization_rejects_collision_equal_installed_substitution() {
    let first = runnable_fixture_at(52_000, 0x1000);
    let substituted = runnable_fixture_at(52_000, 0x3000);
    assert_eq!(first.installed_code, substituted.installed_code);
    assert_eq!(first.runnable.artifact(), substituted.runnable.artifact());
    assert_ne!(
        first.runnable.installed().receipt_context(),
        substituted.runnable.installed().receipt_context(),
        "different exact placements must remain distinct despite equal compact report IDs"
    );

    let mut first_ledger = lifecycle();
    let first_candidate = candidate(32, &first.runnable);
    let first_receipt = ComponentEraPublicationReceipt::from_runtime(
        520,
        first_ledger.lifecycle(),
        &first_candidate,
        true,
        false,
    );
    let first_prepared = prepare_component_deployment(
        952,
        &first_ledger,
        first_candidate,
        first_receipt,
        first.runnable,
        journal_acceptance(),
    )
    .expect("first prepared deployment");
    let first_durable = first_prepared.record().clone();
    let first_activated = first_prepared
        .activate(&first_durable, &mut first_ledger)
        .expect("first activated deployment");
    let first_activated_durable = first_activated.record().clone();

    let mut substituted_ledger = lifecycle();
    let substituted_candidate = candidate(32, &substituted.runnable);
    let substituted_receipt = ComponentEraPublicationReceipt::from_runtime(
        520,
        substituted_ledger.lifecycle(),
        &substituted_candidate,
        true,
        false,
    );
    let substituted_prepared = prepare_component_deployment(
        953,
        &substituted_ledger,
        substituted_candidate,
        substituted_receipt,
        substituted.runnable,
        journal_acceptance(),
    )
    .expect("substituted prepared deployment");
    let substituted_durable = substituted_prepared.record().clone();
    let _substituted_activated = substituted_prepared
        .activate(&substituted_durable, &mut substituted_ledger)
        .expect("substituted activated deployment");

    let error = first_activated
        .finalize(&first_activated_durable, &substituted_ledger)
        .expect_err("equal report IDs cannot substitute another installed occurrence");
    assert!(
        error
            .diagnostic()
            .contains("exact activated installed-code")
    );
    error
        .into_activated()
        .finalize(&first_activated_durable, &first_ledger)
        .expect("failed substitution preserves exact activated evidence for retry");
}

#[test]
fn durable_deployment_journal_storage_is_no_clobber_and_replays_exact_bytes() {
    let fixture = runnable_fixture(55_000);
    let ledger = lifecycle();
    let candidate = candidate(35, &fixture.runnable);
    let receipt = ComponentEraPublicationReceipt::from_runtime(
        550,
        ledger.lifecycle(),
        &candidate,
        true,
        false,
    );
    let prepared = prepare_component_deployment(
        955,
        &ledger,
        candidate,
        receipt,
        fixture.runnable,
        journal_acceptance(),
    )
    .expect("prepared journal fixture");
    let record = prepared.record().clone();
    let root = std::env::temp_dir().join(format!(
        "omega-component-journal-storage-{}-{}",
        std::process::id(),
        record.journal_identity(),
    ));
    std::fs::create_dir(&root).expect("fresh journal storage directory");
    let path = root.join("prepared.journal");
    let stored = durably_store_component_deployment_journal(record.clone(), path.clone())
        .expect("canonical record stores durably");
    assert_eq!(stored.record(), &record);
    assert_eq!(stored.path(), path);
    assert_ne!(stored.byte_compatibility_report_fingerprint(), 0);
    stored.validate().expect("stored record replays exactly");
    let loaded = load_durable_component_deployment_journal(path.clone())
        .expect("restart loads canonical durable record");
    assert_eq!(loaded.record(), &record);
    assert_eq!(loaded.byte_count(), stored.byte_count());
    assert_eq!(
        loaded.byte_compatibility_report_fingerprint(),
        stored.byte_compatibility_report_fingerprint()
    );

    let occupied = root.join("occupied.journal");
    std::fs::write(&occupied, b"caller-owned sentinel").expect("occupied destination fixture");
    let error = durably_store_component_deployment_journal(record.clone(), occupied.clone())
        .expect_err("an existing destination must never be replaced");
    assert_eq!(
        error.state(),
        ComponentDeploymentJournalStorageState::Unpublished
    );
    assert_eq!(error.record(), &record);
    assert_eq!(error.path(), occupied);
    assert_eq!(
        std::fs::read(&occupied).expect("read preserved destination"),
        b"caller-owned sentinel"
    );

    let mut corrupted = std::fs::read(&path).expect("read durable bytes");
    let last = corrupted.len() - 1;
    corrupted[last] ^= 1;
    std::fs::write(&path, corrupted).expect("mutate journal after publication");
    assert!(stored.validate().is_err());
    assert!(load_durable_component_deployment_journal(path.clone()).is_err());

    std::fs::remove_dir_all(&root).expect("remove owned journal storage fixture");
}

#[test]
fn cathedral_selected_restart_recovery_joins_prior_and_candidate_runtime_occurrences() {
    let prior = runnable_fixture(56_000);
    let candidate_fixture = runnable_fixture(57_000);
    let mut ledger = lifecycle();

    let prior_candidate = candidate(40, &prior.runnable);
    let prior_receipt = ComponentEraPublicationReceipt::from_runtime(
        560,
        ledger.lifecycle(),
        &prior_candidate,
        true,
        false,
    );
    ledger
        .publish(prior_candidate, prior_receipt, prior.runnable)
        .expect("publish prior runtime occurrence");

    let next_candidate = candidate(41, &candidate_fixture.runnable);
    let next_receipt = ComponentEraPublicationReceipt::from_runtime(
        570,
        ledger.lifecycle(),
        &next_candidate,
        true,
        true,
    );
    let prepared = prepare_component_deployment(
        960,
        &ledger,
        next_candidate,
        next_receipt,
        candidate_fixture.runnable,
        journal_acceptance(),
    )
    .expect("prepare replacement journal");
    let durable_record = prepared.record().clone();
    let root = std::env::temp_dir().join(format!(
        "omega-component-journal-recovery-{}-{}",
        std::process::id(),
        durable_record.journal_identity(),
    ));
    std::fs::create_dir(&root).expect("fresh restart recovery directory");
    let path = root.join("prepared.journal");
    let stored = durably_store_component_deployment_journal(durable_record.clone(), path)
        .expect("store exact Prepared journal");

    let rollback = join_component_deployment_restart_to_runtime(
        stored,
        ComponentDeploymentRecoveryChoice::RollBackToPrior,
        960,
        "CodecBinding/v1",
        "CodecEntry/v1",
        ledger,
    )
    .expect("caller-selected rollback rejoins the current prior occurrence");
    assert_eq!(rollback.occurrence().era_identity(), 40);
    rollback
        .validate()
        .expect("rollback continuation retains exact live custody");
    let (stored, choice, mut ledger) = rollback.into_parts();
    assert_eq!(choice, ComponentDeploymentRecoveryChoice::RollBackToPrior);

    let _activated = prepared
        .activate(&durable_record, &mut ledger)
        .expect("activate candidate after durable Prepared record");
    let roll_forward = join_component_deployment_restart_to_runtime(
        stored,
        ComponentDeploymentRecoveryChoice::RollForwardCandidate,
        960,
        "CodecBinding/v1",
        "CodecEntry/v1",
        ledger,
    )
    .expect("caller-selected roll-forward rejoins the current candidate occurrence");
    assert_eq!(roll_forward.occurrence().era_identity(), 41);
    roll_forward
        .validate()
        .expect("roll-forward continuation retains exact live custody");
    drop(roll_forward);
    std::fs::remove_dir_all(&root).expect("remove owned restart recovery fixture");
}

#[test]
fn restart_recovery_returns_inputs_on_occurrence_mismatch_and_durable_tamper() {
    let expected = runnable_fixture_at(58_000, 0x1000);
    let substituted = runnable_fixture_at(58_000, 0x3000);
    assert_eq!(expected.installed_code, substituted.installed_code);
    assert_eq!(
        expected.runnable.artifact(),
        substituted.runnable.artifact()
    );
    assert_ne!(
        expected.runnable.installed().occurrence_digest(),
        substituted.runnable.installed().occurrence_digest(),
        "fixture must collide only in compact report identities"
    );

    let expected_ledger = lifecycle();
    let expected_candidate = candidate(42, &expected.runnable);
    let expected_receipt = ComponentEraPublicationReceipt::from_runtime(
        580,
        expected_ledger.lifecycle(),
        &expected_candidate,
        true,
        false,
    );
    let prepared = prepare_component_deployment(
        961,
        &expected_ledger,
        expected_candidate,
        expected_receipt,
        expected.runnable,
        journal_acceptance(),
    )
    .expect("prepare expected occurrence journal");
    let record = prepared.record().clone();
    let root = std::env::temp_dir().join(format!(
        "omega-component-journal-recovery-mismatch-{}-{}",
        std::process::id(),
        record.journal_identity(),
    ));
    std::fs::create_dir(&root).expect("fresh recovery mismatch directory");
    let path = root.join("prepared.journal");
    let stored = durably_store_component_deployment_journal(record, path.clone())
        .expect("store expected occurrence journal");

    let mut substituted_ledger = lifecycle();
    let substituted_candidate = candidate(42, &substituted.runnable);
    let substituted_receipt = ComponentEraPublicationReceipt::from_runtime(
        581,
        substituted_ledger.lifecycle(),
        &substituted_candidate,
        true,
        false,
    );
    substituted_ledger
        .publish(
            substituted_candidate,
            substituted_receipt,
            substituted.runnable,
        )
        .expect("publish collision-equal substituted occurrence");

    let error = join_component_deployment_restart_to_runtime(
        stored,
        ComponentDeploymentRecoveryChoice::RollForwardCandidate,
        961,
        "CodecBinding/v1",
        "CodecEntry/v1",
        substituted_ledger,
    )
    .expect_err("compact report identity cannot substitute another live occurrence");
    assert!(error.diagnostic().contains("exact era occurrence"));
    let (stored, choice, substituted_ledger) = error.into_parts();
    assert_eq!(
        choice,
        ComponentDeploymentRecoveryChoice::RollForwardCandidate
    );

    let mut corrupted = std::fs::read(&path).expect("read durable recovery journal");
    let last = corrupted.len() - 1;
    corrupted[last] ^= 1;
    std::fs::write(&path, corrupted).expect("tamper durable recovery journal");
    let error = join_component_deployment_restart_to_runtime(
        stored,
        choice,
        961,
        "CodecBinding/v1",
        "CodecEntry/v1",
        substituted_ledger,
    )
    .expect_err("tampered durable bytes reject before the runtime join");
    assert!(error.diagnostic().contains("does not validate"));
    let (stored, returned_choice, _substituted_ledger) = error.into_parts();
    assert_eq!(returned_choice, choice);
    assert_eq!(stored.path(), path);
    drop(stored);
    std::fs::remove_dir_all(&root).expect("remove owned recovery mismatch fixture");
}

fn root_id<T>(identity: u64, constructor: fn(u64) -> Result<T, ExternalRootDiagnostic>) -> T {
    constructor(identity).expect("normalized external-root identity")
}

fn install_id<T>(
    identity: u64,
    constructor: fn(u64) -> Result<T, omega_executable_installation::InstallationDiagnostic>,
) -> T {
    constructor(identity).expect("normalized installation identity")
}

fn extent_id<T>(identity: u64, constructor: fn(u64) -> Result<T, ExtentDiagnostic>) -> T {
    constructor(identity).expect("normalized extent identity")
}

fn callback_boundary() -> ValidatedBoundaryEntryPlan {
    evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::SystemVAMD64,
        &CallSignature {
            parameters: vec![ValueShape::integer(8, 8)],
            result: None,
        },
    )
    .expect("callback boundary")
}

fn callback_fuel(_root: ExternalRootId) -> omega_external_roots::ComposedFuelDemand {
    let leaf_identity = root_id(731, ProviderFuelSummaryId::from_normalized_identity);
    let owner_identity = root_id(730, ProviderFuelSummaryId::from_normalized_identity);
    let leaf = FixedFuelProviderSummary::from_admitted_provider(
        leaf_identity,
        root_id(712, RootProviderId::from_normalized_identity),
        FuelScheduleIdentity::new(1).expect("fuel schedule"),
        5,
        std::collections::BTreeSet::new(),
        root_id(
            741,
            ProviderFuelValidationReceiptId::from_normalized_identity,
        ),
    );
    let owner = FixedFuelProviderSummary::from_admitted_provider(
        owner_identity,
        root_id(702, RootProviderId::from_normalized_identity),
        FuelScheduleIdentity::new(1).expect("fuel schedule"),
        2,
        std::collections::BTreeSet::from([FixedFuelCall {
            callee: leaf_identity,
            maximum_invocations: 1,
        }]),
        root_id(
            740,
            ProviderFuelValidationReceiptId::from_normalized_identity,
        ),
    );
    compose_fixed_fuel(owner_identity, [&owner, &leaf]).expect("callback fuel")
}

fn callback_stack(
    root: ExternalRootId,
    provider: RootProviderId,
    relation: NestingRelationId,
    boundary: &ValidatedBoundaryEntryPlan,
    code: &InstalledCode,
    entry: EntryStubId,
) -> BoundEpochStackComposition {
    let active_domain = StackDomainRef::Interrupted;
    let realization = validate_entry_stack_realization(EntryStackRealization {
        contexts: vec![ArrivalContextRealization {
            context: ArrivalContextId::new(1).expect("arrival context"),
            epochs: vec![EntryStackEpoch {
                stage: EntryStackStage::Body,
                active_domain,
                occupancy_by_domain: Vec::new(),
                nesting: boundary.plan().state.preemption,
            }],
        }],
    })
    .expect("callback stack realization");
    let summary = ProviderStackSummary::from_admitted_provider(
        root,
        provider,
        boundary.plan().state.stack,
        2048,
        16,
        root_id(749, StackValidationReceiptId::from_normalized_identity),
    );
    let contexts: AdmittedOpaqueArrivalContextSet = admit_opaque_arrival_context_set(
        &summary,
        boundary,
        code,
        entry,
        vec![ArrivalContextId::new(1).expect("arrival context")],
        root_id(748, StackValidationReceiptId::from_normalized_identity),
    )
    .expect("callback arrival context");
    let bound = bind_opaque_adapter_stack_realization(
        &summary,
        boundary,
        code,
        entry,
        realization,
        contexts,
    )
    .expect("callback stack binding");
    compose_bound_entry_stack_epochs(
        &StackNestingRelation {
            identity: relation,
            edges: std::collections::BTreeSet::new(),
        },
        [&bound],
    )
    .expect("callback stack composition")
}

fn callback_root_candidate(code: &InstalledCode, entry: EntryStubId) -> ExternalRootCandidate {
    let root = root_id(701, ExternalRootId::from_normalized_identity);
    let provider = root_id(702, RootProviderId::from_normalized_identity);
    let relation = root_id(706, NestingRelationId::from_normalized_identity);
    let boundary = callback_boundary();
    ExternalRootCandidate {
        identity: root,
        entry,
        provider,
        provider_plan: root_id(755, ProviderPlanId::from_normalized_identity),
        provider_plan_digest: ProviderPlan::default().identity_digest(),
        requirement_identity: "Callback::entry".into(),
        entry_claims: Vec::new(),
        acknowledgement_parameter_index: None,
        interrupt_mask_guard_claim: None,
        service_reach: ResolvedRootServiceReach::from_selected_provider_closure(
            Vec::new(),
            Vec::new(),
            &SelectedProviderPlanFacts::default(),
        )
        .expect("empty callback service reach"),
        effects: [root_id(703, RootEffectId::from_normalized_identity)]
            .into_iter()
            .collect(),
        trust_receipts: [root_id(704, TrustReceiptId::from_normalized_identity)]
            .into_iter()
            .collect(),
        nesting_relation: relation,
        acknowledgement_policy: Some(root_id(
            707,
            AcknowledgementPolicyId::from_normalized_identity,
        )),
        stack: StackResourceColumn {
            ceiling_bytes: 8192,
            realization: callback_stack(root, provider, relation, &boundary, code, entry),
            validation_receipt: root_id(750, StackValidationReceiptId::from_normalized_identity),
        },
        logical_fuel: LogicalFuelResourceColumn {
            schedule: FuelScheduleIdentity::new(1).expect("fuel schedule"),
            provision: root_id(753, FuelProvisionId::from_normalized_identity),
            ceiling_units: 64,
            realization: callback_fuel(root),
            validation_receipt: root_id(
                751,
                omega_external_roots::FuelValidationReceiptId::from_normalized_identity,
            ),
        },
        machine_state: MachineStateResourceColumn {
            realization: StateFootprintEvidence::new(
                RegisterSet::new([MachineRegister::X86Rax]),
                MachineStateSet::new([MachineState::Flags]),
            ),
            validation_receipt: root_id(
                752,
                omega_external_roots::StateValidationReceiptId::from_normalized_identity,
            ),
        },
        component_pins: [ComponentVersionPin {
            contract: root_id(708, ComponentContractId::from_normalized_identity),
            artifact: root_id(709, ComponentArtifactId::from_normalized_identity),
            provider: root_id(710, ComponentProviderId::from_normalized_identity),
            version: root_id(711, ComponentVersionPinId::from_normalized_identity),
        }]
        .into_iter()
        .collect(),
    }
}
