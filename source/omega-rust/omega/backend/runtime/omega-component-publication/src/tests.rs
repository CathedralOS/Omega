use super::*;

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
    ComponentProgressDemandIdentity, ComponentProgressReceiptBinding, ExternalRootDiagnostic,
    InstalledProviderOccurrenceId, InstalledRootLedger, ProgressProfileEstablishmentAttestation,
    ProgressProfileEstablishmentReceiptId, ProgressProfileGrantInvocationId,
    ProviderOccurrenceInstallationReceipt, ProviderOccurrenceInstallationReceiptId,
    ProviderOccurrencePlanBinding,
};
use omega_image_emission::{
    bind_installed_artifact, build_installation_record_with_evidence, build_object_artifact,
    emit_executable_image,
};
use omega_installation_evidence::ProviderExecutionEvidence;
use omega_machine_code::{
    BoundarySettlementRecord, MachineCodeFunction, MachineCodePlan, SemanticCodeAttribution,
    SemanticCodeSite,
};
use omega_target_operations::{
    BoundaryRealization, BoundaryScalarArgument, LinuxExitGroupI32Realization,
    ProviderExecutionBinding, ProviderPlanReportIdentity, TerminalPsiProvenance,
};
use psi_core::{BoundaryMachineId, EdgeId, MachineId, OperationId, ProfileDecisionId};
use psi_extents::{
    AddressSpaceId, ExtentDiagnostic, ExtentLineageId, ExtentProvenanceId, ExtentRightId,
    ExtentRights, ExtentRootGrant, MappingEraId,
};
use psi_layout_plans::{
    ArtifactInstallationScopeId, PlacementConstraints, PlacementPhase, PlacementSite,
};
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
    let operation = OperationId::new(1).expect("operation");
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
                operations: vec![operation],
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
            dynamic_scalar_calls: Vec::new(),
            dynamic_parameter_scalar_calls: Vec::new(),
            forwarded_dynamic_descriptor_calls: Vec::new(),
            unit_scalar_homes: Vec::new(),
            unit_integer_constants: Vec::new(),
            unit_structural_scalar_field_stores: Vec::new(),
            unit_affine_cleanup: None,
            semantic_code_attribution: vec![
                SemanticCodeAttribution {
                    site: SemanticCodeSite::Operation(operation),
                    operation_ordinal: 0,
                    code_offset: 0,
                    byte_count: bytes.len(),
                },
                SemanticCodeAttribution {
                    site: SemanticCodeSite::Edge(edge),
                    operation_ordinal: 1,
                    code_offset: bytes.len(),
                    byte_count: 0,
                },
            ],
            port_effects: Vec::new(),
            boundary_settlements: vec![BoundarySettlementRecord {
                psi_operation: operation,
                boundary: BoundaryMachineId::new(1).expect("boundary"),
                execution: omega_machine_code::BoundaryExecutionRecord::AdmittedProvider(
                    provider.into(),
                ),
                realization: BoundaryRealization::LinuxExitGroupI32(LinuxExitGroupI32Realization),
                scalar_arguments: vec![BoundaryScalarArgument {
                    source_value: psi_core::ValueId::new(1).expect("value"),
                    scalar_type,
                    immediate: psi_core::IntegerValue::Signed(0),
                    destination: omega_calling_conventions::MachineRegister::X86Rdi,
                }],
                arguments: Vec::new(),
                byte_sequence_arguments: Vec::new(),
                completion_claim_sources: Vec::new(),
                completion_receipts: Vec::new(),
                completion_provider_custody: Vec::new(),
                native_result: None,
                operation_ordinal: 0,
                code_offset: 0,
                byte_count: bytes.len(),
            }],
            scalar_affine_cleanup: None,
            scalar_control_affine_cleanups: Vec::new(),
            scalar_structural_parameters: Vec::new(),
            scalar_structural_parameter_homes: Vec::new(),
            ranked_u32_countdown: None,
            structural_return: None,
        }],
    };
    let object = build_object_artifact(&plan).expect("terminal object");
    let image = emit_executable_image(&object, 3).expect("terminal image");
    (object, image)
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
    let entry = psi_layout_plans::EntryStubId::from_normalized_identity(1).expect("entry");
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
        vec![ArtifactEntry::from_canonical_decode(entry, 0)],
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
