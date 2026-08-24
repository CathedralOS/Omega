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
    AdmissionReceiptId, Artifact, ArtifactAdmissionEvidence, ArtifactContentId, ArtifactEntry,
    ArtifactId, CodePlacementAuthority, CodePlacementId, EntrySetId, FinalValidationCertificate,
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
use omega_terminal_image_emission::{
    bind_installed_terminal_artifact, build_terminal_installation_record_with_evidence,
    build_terminal_object_artifact, emit_terminal_executable_image,
};
use omega_terminal_installation_evidence::TerminalProviderExecutionEvidence;
use omega_terminal_machine_code::{
    TerminalBoundarySettlementRecord, TerminalMachineCodeFunction, TerminalMachineCodePlan,
    TerminalNativeFuelAttribution, TerminalNativeFuelSite,
};
use omega_terminal_target_operations::{
    TerminalBoundaryRealization, TerminalBoundaryScalarArgument,
    TerminalLinuxExitGroupI32Realization, TerminalProviderExecutionBinding,
    TerminalProviderPlanIdentity, TerminalPsiProvenance,
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
    fingerprint: u64,
    root: u64,
    boundary: u64,
}

impl TerminalProviderExecutionEvidence for TestProviderExecution {
    fn requirement_identity(&self) -> &str {
        "Scheduler::wait#exact"
    }

    fn provider_plan(&self) -> u64 {
        self.plan
    }

    fn provider_execution_identity(&self) -> u64 {
        self.execution
    }

    fn provider_execution_fingerprint(&self) -> u64 {
        self.fingerprint
    }

    fn normalized_root_identity(&self) -> u64 {
        self.root
    }

    fn boundary_contract_fingerprint(&self) -> u64 {
        self.boundary
    }
}

struct RunnableFixture {
    installed_code: InstalledCodeId,
    runnable: InstalledRunnableComponent,
}

fn runnable_fixture(seed: u64) -> RunnableFixture {
    let route = ServiceProgressEstablishmentRoute {
        kind: ServiceProgressEstablishmentRouteKind::BoundaryRequirement,
        requirement_identity: "Scheduler::grant_weak_fair#exact".into(),
    };
    let provider = ProviderPlan {
        name: "scheduler-plan".into(),
        provider_type: "SchedulerProvider".into(),
        target: "test".into(),
        schema: ServiceSchema {
            trait_name: "Scheduler".into(),
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
                binding: ProviderBinding::CompilerIntrinsic {
                    machine: "TestScheduler::wait".into(),
                },
            },
            ProviderPlanRow {
                method: "grant_weak_fair".into(),
                requirement_identity: route.requirement_identity.clone(),
                binding: ProviderBinding::CompilerIntrinsic {
                    machine: "TestScheduler::grant_weak_fair".into(),
                },
            },
        ],
        origin_package: "omega::test".into(),
    };
    let provider_plan = provider.identity_fingerprint();
    let selected =
        SelectedProviderPlanFacts::from_selection(&[provider], &["scheduler-plan".into()])
            .expect("selected provider plan");
    let manifest = ComponentProgressManifest::bind(
        "Application::start".into(),
        &selected,
        vec![CheckedComponentProgressDemand {
            provider_service_identity: "Scheduler".into(),
            requirement_identity: "Scheduler::wait#exact".into(),
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
        fingerprint: seed + 2,
        root: seed + 3,
        boundary: seed + 4,
    };
    let (object, image) = terminal_image(&provider_execution);
    let mut installed = install_terminal_text(&object, seed + 20, seed + 21);
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
    let installation = build_terminal_installation_record_with_evidence(
        &image,
        ProfileDecisionId::new(seed + 40).expect("profile decision"),
        [&provider_execution],
        Some(&progress),
    )
    .expect("terminal installation record");
    let artifact = bind_installed_terminal_artifact(&object, &image, installation, installed)
        .expect("installed terminal artifact");
    let runnable = bind_installed_runnable_component(artifact, Some(progress))
        .expect("installed runnable component");
    RunnableFixture {
        installed_code,
        runnable,
    }
}

fn terminal_image(
    execution: &TestProviderExecution,
) -> (
    omega_terminal_image_emission::TerminalObjectArtifact,
    omega_terminal_image_emission::TerminalExecutableImage,
) {
    let machine = MachineId::new(1).expect("machine");
    let operation = OperationId::new(1).expect("operation");
    let edge = EdgeId::new(1).expect("edge");
    let bytes = omega_terminal_isa_x86_64::encode_linux_exit_group_i32(0);
    let provider = TerminalProviderExecutionBinding::from_execution_record(
        TerminalProviderPlanIdentity::new(execution.plan).expect("provider plan"),
        execution.execution,
        execution.fingerprint,
        execution.root,
        execution.boundary,
    )
    .expect("provider execution binding");
    let scalar_type = psi_core::ScalarType::Integer(
        psi_core::IntegerType::new(psi_core::IntegerSign::Signed, 32).expect("i32"),
    );
    let plan = TerminalMachineCodePlan {
        terminal_psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
        },
        target: omega_target::NativeTarget::linux_x64(),
        entry: machine,
        functions: vec![TerminalMachineCodeFunction {
            machine,
            attachment: None,
            provenance: TerminalPsiProvenance {
                operations: vec![operation],
                edges: vec![edge],
            },
            bytes: bytes.clone(),
            unit_stack: None,
            unit_parameter_homes: Vec::new(),
            unit_parameters: Vec::new(),
            scalar_stack: None,
            internal_calls: Vec::new(),
            internal_unit_calls: Vec::new(),
            unit_affine_cleanup: None,
            fuel_attribution: vec![
                TerminalNativeFuelAttribution {
                    schedule: psi_terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
                    site: TerminalNativeFuelSite::Operation(operation),
                    units: 1,
                    operation_ordinal: 0,
                    code_offset: 0,
                    byte_count: bytes.len(),
                },
                TerminalNativeFuelAttribution {
                    schedule: psi_terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
                    site: TerminalNativeFuelSite::Edge(edge),
                    units: 1,
                    operation_ordinal: 1,
                    code_offset: bytes.len(),
                    byte_count: 0,
                },
            ],
            port_effects: Vec::new(),
            boundary_settlements: vec![TerminalBoundarySettlementRecord {
                psi_operation: operation,
                boundary: BoundaryMachineId::new(1).expect("boundary"),
                provider_execution: provider.into(),
                realization: TerminalBoundaryRealization::LinuxExitGroupI32(
                    TerminalLinuxExitGroupI32Realization,
                ),
                scalar_arguments: vec![TerminalBoundaryScalarArgument {
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
            structural_return: None,
        }],
    };
    let object = build_terminal_object_artifact(&plan).expect("terminal object");
    let image = emit_terminal_executable_image(&object, 3).expect("terminal image");
    (object, image)
}

fn install_terminal_text(
    object: &omega_terminal_image_emission::TerminalObjectArtifact,
    artifact_identity: u64,
    installed_identity: u64,
) -> InstalledCode {
    let scope = ArtifactInstallationScopeId::from_normalized_identity(1).expect("scope");
    let constraints = PlacementConstraints::new(None, 16, PlacementPhase::Load, None, Some(scope))
        .expect("constraints");
    let entry = psi_layout_plans::EntryStubId::from_normalized_identity(1).expect("entry");
    let artifact = Artifact::from_canonical_decode(
        install_id(artifact_identity, ArtifactId::from_normalized_identity),
        install_id(
            artifact_identity + 1,
            ArtifactContentId::from_normalized_identity,
        ),
        object.target().architecture,
        object.text_bytes().to_vec(),
        install_id(2, MachineContractSetId::from_normalized_identity),
        install_id(3, MachineFootprintId::from_normalized_identity),
        install_id(4, PlacementPlanId::from_normalized_identity),
        constraints,
        install_id(5, EntrySetId::from_normalized_identity),
        vec![ArtifactEntry::from_canonical_decode(entry, 0)],
        install_id(6, RelocationSetId::from_normalized_identity),
        Vec::new(),
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
    .mint(0x1000, 4096)
    .expect("placement extent");
    let placement = CodePlacementAuthority::from_admitted_provider(
        install_id(8, CodePlacementId::from_normalized_identity),
        install_id(1, InstallationScopeId::from_normalized_identity),
        InstallationAudience::DormantLocal,
        &extent,
        rights,
        constraints,
        PlacementSite {
            base_address: 0x1000,
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
                selected_provider_closure_identity: closure,
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

fn candidate(era: u64, installed: InstalledCodeId) -> ComponentEraCandidate {
    ComponentEraCandidate {
        era_identity: era,
        artifact_instance_identity: installed.normalized_identity(),
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

    let first_candidate = candidate(10, first.installed_code);
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

    let second_candidate = candidate(11, second.installed_code);
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
    let (artifact, progress) = retired.into_parts();
    let (_, installed) = artifact.into_parts();
    assert!(progress.is_some());
    assert_eq!(installed.identity(), first.installed_code);
}

#[test]
fn runnable_publication_rejection_returns_candidate_receipt_and_opaque_evidence() {
    let fixture = runnable_fixture(30_000);
    let mut ledger = lifecycle();
    let mut wrong = candidate(10, fixture.installed_code);
    wrong.artifact_instance_identity ^= 1;
    let receipt =
        ComponentEraPublicationReceipt::from_runtime(300, ledger.lifecycle(), &wrong, true, false);
    let error = ledger
        .publish(wrong, receipt, fixture.runnable)
        .expect_err("artifact occurrence substitution rejects");
    assert!(error.diagnostic().contains("different installed artifact"));
    let (_, _, runnable) = error.into_parts();
    let (artifact, progress) = runnable.into_parts();
    let (_, installed) = artifact.into_parts();
    assert!(progress.is_some());
    assert_eq!(installed.identity(), fixture.installed_code);
    assert_eq!(ledger.current_era(), None);
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
