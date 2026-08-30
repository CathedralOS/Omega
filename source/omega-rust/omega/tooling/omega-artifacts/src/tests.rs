//! Artifact construction and projection regression tests.

use std::collections::BTreeSet;

use omega_calling_conventions::{
    ArrivalContextId, ArrivalContextRealization, CallSignature, CallingPolicy, EntryStack,
    EntryStackEpoch, EntryStackRealization, EntryStackStage, MachineRegister, MachineState,
    MachineStateSet, Preemption, ProviderExitRealization, RegisterSet, StackDomainRef,
    StateFootprintEvidence, ValueShape, evaluate_ordinary_boundary_entry_plan,
    validate_entry_stack_realization,
};
use omega_effects::{ForeignLocatorCandidate, normalize_foreign_locator};
use omega_executable_installation::{
    AdmissionReceiptId, Artifact, ArtifactAdmissionEvidence, ArtifactAuthorityCommitments,
    ArtifactEntry, ArtifactId, CodePlacementAuthority, CodePlacementId, ContainerLimits,
    DecodedArtifactContainer, EntrySetId, FinalValidationCertificate, FinalValidationId,
    InstallAuthority, InstallationAudience, InstallationDiagnostic, InstallationReceipt,
    InstallationScopeId, InstalledCode, InstalledCodeId, MachineContractSetId, MachineFootprintId,
    MaterializationReceipt, PlacementPlanId, RelocationSetId, WxEnforcement, admit_executable,
    decode_executable_container, install_validated, materialize_admitted_artifact,
    materialize_and_freeze, normalized_decoded_content_digest, validate_final_placement,
};
use omega_external_roots::{
    AcknowledgementPolicyId, ComponentArtifactId, ComponentContractId, ComponentProviderId,
    ComponentVersionPin, ComponentVersionPinId, ExternalRootDiagnostic, ExternalRootId,
    FixedFuelCall, FixedFuelProviderSummary, FuelProvisionId, FuelScheduleIdentity,
    FuelValidationReceiptId, InstalledRootRecord, LogicalFuelResourceColumn,
    MachineStateResourceColumn, NestingRelationId, OpaqueProviderExitAssurance,
    ProviderExecutionId, ProviderFuelSummaryId, ProviderFuelValidationReceiptId, ProviderPlanId,
    ProviderStackSummary, RootAdmissionId, RootEffectId, RootProviderId, RootSlotId,
    RootSlotOwnerId, StackNestingRelation, StackResourceColumn, StackValidationReceiptId,
    StateValidationReceiptId, TrustReceiptId, bind_opaque_adapter_stack_realization,
    compose_bound_entry_stack_epochs, compose_fixed_fuel,
};
use omega_target::{Architecture, TargetProfile};
use psi_extents::{
    AddressSpaceId, ExtentDiagnostic, ExtentLineageId, ExtentProvenanceId, ExtentRightId,
    ExtentRights, ExtentRootGrant, MappingEraId,
};
use psi_layout_plans::{
    ArtifactInstallationScopeId, EntryStubId, PlacementAddressRange, PlacementConstraints,
    PlacementPhase, PlacementSite,
};

use super::external_root_report::external_root_records_manifest_json;
use super::{
    ArtifactWriter, TrustCrashCause, TrustCrashRouteBucket, TrustCrashRouteGuard,
    TrustGenericAcceptedInstanceRow, TrustProgressPremiseRow, TrustProgressPremiseSubject,
    TrustProviderRealization, TrustProviderRequirementRow, TrustQualificationRow, TrustReport,
    TrustReportRow, value_placement_json,
};

fn normalized_windows_import(library: &[u8], export: &[u8]) -> TrustProviderRealization {
    TrustProviderRealization::Import {
        locator: normalize_foreign_locator(
            ForeignLocatorCandidate::PeByName {
                library: library.to_vec(),
                export: export.to_vec(),
            },
            TargetProfile::WindowsX64,
        )
        .expect("valid normalized Windows import"),
    }
}

fn test_provider_plan_digest() -> omega_effects::provider_plan::ProviderPlanDigest {
    omega_effects::provider_plan::ProviderPlan::default().identity_digest()
}

fn test_selected_provider_closure_digest() -> omega_effects::SelectedProviderClosureDigest {
    omega_effects::SelectedProviderPlanFacts::default().identity_digest()
}

fn trust_provider_requirement(
    target: &str,
    realization: TrustProviderRealization,
) -> TrustProviderRequirementRow {
    TrustProviderRequirementRow {
        provider_plan: "ForeignProvider::satisfies::Foreign".to_owned(),
        provider_plan_report_fingerprint: 0x1234,
        provider_plan_digest: test_provider_plan_digest(),
        provider_type: String::new(),
        provider_type_package_identity: None,
        target: target.to_owned(),
        provider_origin_package_identity: None,
        provider_origin_package: String::new(),
        service_schema: "Foreign".to_owned(),
        service_schema_package_identity: None,
        calling_plan_report_fingerprint: None,
        calling_plan_commitment: None,
        selected: true,
        requirement_owner: "Foreign".to_owned(),
        requirement_owner_package_identity: None,
        requirement_identity: "Foreign::invoke".to_owned(),
        method: "invoke".to_owned(),
        parameter_type_identities: Vec::new(),
        result_type_identity: None,
        service_reach: Vec::new(),
        synchronous_invocations: Vec::new(),
        may_suspend: false,
        may_block: false,
        terminates_guarantee: false,
        termination_premises: Vec::new(),
        realization,
        provenance: "root grant (build.omg)".to_owned(),
        grant_selectors: vec!["Foreign".to_owned()],
        standing_warning: false,
    }
}

#[test]
fn accepted_machine_service_reach_distinguishes_public_empty_from_non_machine_rows() {
    let root = std::env::temp_dir().join(format!(
        "omega-trust-accepted-reach-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let writer = ArtifactWriter::new(&root).expect("artifact writer");
    let report = TrustReport {
        rows: vec![
            TrustReportRow {
                commitment: "accepted fact: quiet_axiom".to_owned(),
                provenance: "root grant (build.omg)".to_owned(),
                machine_contract_report_fingerprint: Some(0xabcd),
                machine_contract_commitment: Some(
                    psi_checked_trees::MachineContractCommitment::from_digest([0xab; 32]),
                ),
                machine_template_report_fingerprint: None,
                machine_service_reach: Some(Vec::new()),
                machine_synchronous_invocations: Some(Vec::new()),
                machine_may_suspend: Some(false),
                machine_may_block: Some(false),
                machine_terminates_guarantee: Some(false),
                machine_crash_routes: Some(Vec::new()),
                standing_warning: false,
            },
            TrustReportRow {
                commitment: "provider plan: console".to_owned(),
                provenance: "root grant (build.omg)".to_owned(),
                machine_contract_report_fingerprint: None,
                machine_contract_commitment: None,
                machine_template_report_fingerprint: None,
                machine_service_reach: None,
                machine_synchronous_invocations: None,
                machine_may_suspend: None,
                machine_may_block: None,
                machine_terminates_guarantee: None,
                machine_crash_routes: None,
                standing_warning: false,
            },
            TrustReportRow {
                commitment: "accepted fact: guarded_axiom".to_owned(),
                provenance: "root grant (build.omg)".to_owned(),
                machine_contract_report_fingerprint: Some(0xbcde),
                machine_contract_commitment: Some(
                    psi_checked_trees::MachineContractCommitment::from_digest([0xbc; 32]),
                ),
                machine_template_report_fingerprint: None,
                machine_service_reach: Some(Vec::new()),
                machine_synchronous_invocations: Some(Vec::new()),
                machine_may_suspend: Some(false),
                machine_may_block: Some(false),
                machine_terminates_guarantee: Some(false),
                machine_crash_routes: Some(vec![
                    TrustCrashRouteBucket {
                        cause: TrustCrashCause::Trap,
                        alternative_guards: vec![
                            TrustCrashRouteGuard::PredicateIdentity(vec![1]),
                            TrustCrashRouteGuard::PredicateIdentity(vec![2]),
                        ],
                    },
                    TrustCrashRouteBucket {
                        cause: TrustCrashCause::Abort,
                        alternative_guards: vec![TrustCrashRouteGuard::Truth],
                    },
                ]),
                standing_warning: false,
            },
        ],
        generic_accepted_instances: vec![TrustGenericAcceptedInstanceRow {
            template_commitment: "admitted".to_owned(),
            template_report_fingerprint: 0x1111,
            instance_report_fingerprint: 0x2222,
            instance_contract_report_fingerprint: 0xaaaa,
            instance_contract_commitment: psi_checked_trees::MachineContractCommitment::from_digest(
                [0xaa; 32],
            ),
            type_argument_identities: vec!["named(name(Card))".to_owned()],
            const_argument_identities: vec!["named(name(1))".to_owned()],
            machine_argument_contract_report_fingerprints: vec![0x3333],
            machine_argument_contract_commitments: vec![
                psi_checked_trees::MachineContractCommitment::from_digest([0x33; 32]),
            ],
            conformance_argument_report_fingerprints: vec![0x4444],
            conformance_argument_commitments: vec![
                psi_typed_trees::typed_trees::ClosedConformanceApplicationCommitment::from_digest(
                    [0x44; 32],
                ),
            ],
        }],
        ..Default::default()
    };

    writer
        .write_trust_report(&report)
        .expect("trust report output");
    let output =
        std::fs::read_to_string(root.join("trust_report.md")).expect("written trust report");
    let accepted = output
        .lines()
        .find(|line| line.contains("accepted fact: quiet_axiom"))
        .expect("accepted fact row");
    let provider = output
        .lines()
        .find(|line| line.contains("provider plan: console"))
        .expect("provider row");
    let guarded = output
        .lines()
        .find(|line| line.contains("accepted fact: guarded_axiom"))
        .expect("guarded accepted fact row");

    assert!(accepted.contains("service reach: none"));
    assert!(accepted.contains("synchronous invocations: none"));
    assert!(accepted.contains("may suspend: no"));
    assert!(accepted.contains("may block: no"));
    assert!(accepted.contains("termination guarantee: no"));
    assert!(accepted.contains("crash routes: none"));
    assert!(guarded.contains("crash routes: Trap[0x01 | 0x02], Abort[true]"));
    assert!(output.contains("accepted template: admitted"));
    assert!(output.contains("template report fingerprint: 0000000000001111"));
    assert!(output.contains("instance report fingerprint: 0000000000002222"));
    assert!(output.contains("instance contract report fingerprint: 000000000000aaaa"));
    assert!(output.contains(&format!(
        "instance contract commitment: 0x{}",
        "aa".repeat(32)
    )));
    assert!(output.contains("type argument identities: named(name(Card))"));
    assert!(output.contains("const argument identities: named(name(1))"));
    assert!(output.contains("machine argument contract report fingerprints: 0000000000003333"));
    assert!(output.contains(&format!(
        "machine argument contract commitments: 0x{}",
        "33".repeat(32)
    )));
    assert!(output.contains("conformance argument report fingerprints: 0000000000004444"));
    assert!(output.contains(&format!(
        "conformance argument commitments: 0x{}",
        "44".repeat(32)
    )));
    assert!(!provider.contains("service reach:"));
    assert!(!provider.contains("synchronous invocations:"));
    assert!(!provider.contains("may suspend:"));
    assert!(!provider.contains("may block:"));
    assert!(!provider.contains("termination guarantee:"));
    assert!(!provider.contains("crash routes:"));
    std::fs::remove_dir_all(root).expect("remove test artifact directory");
}

#[test]
fn compact_equal_generic_instances_render_distinct_strong_commitments() {
    let root = std::env::temp_dir().join(format!(
        "omega-trust-compact-equal-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let writer = ArtifactWriter::new(&root).expect("artifact writer");
    let row = TrustGenericAcceptedInstanceRow {
        template_commitment: "accepted_generic".to_owned(),
        template_report_fingerprint: 7,
        instance_report_fingerprint: 9,
        instance_contract_report_fingerprint: 11,
        instance_contract_commitment: psi_checked_trees::MachineContractCommitment::from_digest(
            [0x11; 32],
        ),
        type_argument_identities: vec!["named(name(First))".to_owned()],
        const_argument_identities: Vec::new(),
        machine_argument_contract_report_fingerprints: Vec::new(),
        machine_argument_contract_commitments: Vec::new(),
        conformance_argument_report_fingerprints: Vec::new(),
        conformance_argument_commitments: Vec::new(),
    };
    let mut substitute = row.clone();
    substitute.instance_contract_commitment =
        psi_checked_trees::MachineContractCommitment::from_digest([0x22; 32]);
    substitute.type_argument_identities = vec!["named(name(Substitute))".to_owned()];
    assert_eq!(
        row.instance_contract_report_fingerprint,
        substitute.instance_contract_report_fingerprint
    );
    assert_ne!(
        row.instance_contract_commitment,
        substitute.instance_contract_commitment
    );

    writer
        .write_trust_report(&TrustReport {
            generic_accepted_instances: vec![row, substitute],
            ..Default::default()
        })
        .expect("trust report output");
    let output =
        std::fs::read_to_string(root.join("trust_report.md")).expect("written trust report");
    assert!(output.contains(&format!(
        "instance contract commitment: 0x{}",
        "11".repeat(32)
    )));
    assert!(output.contains(&format!(
        "instance contract commitment: 0x{}",
        "22".repeat(32)
    )));
    assert!(output.contains("named(name(First))"));
    assert!(output.contains("named(name(Substitute))"));
    std::fs::remove_dir_all(root).expect("remove test artifact directory");
}

#[test]
fn trust_report_keeps_inherited_requirement_owner_separate_from_overload_identity() {
    let root = std::env::temp_dir().join(format!(
        "omega-trust-owner-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let writer = ArtifactWriter::new(&root).expect("artifact writer");
    let report = TrustReport {
        selected_provider_closure_report_fingerprint: 0xabcd,
        selected_provider_closure_digest: test_selected_provider_closure_digest(),
        rows: Vec::new(),
        generic_accepted_instances: Vec::new(),
        provider_requirements: Vec::new(),
        qualifications: vec![TrustQualificationRow {
            provider_plan: "RootProvider::satisfies::Root".to_owned(),
            provider_plan_report_fingerprint: 0x1234,
            provider_plan_digest: test_provider_plan_digest(),
            provider_type: "RootProvider".to_owned(),
            provider_type_package_identity: psi_core::PackageKeyIdentity::from_digest([0x5b; 32]),
            target: "windows_x86_64".to_owned(),
            provider_origin_package_identity: psi_core::PackageKeyIdentity::from_digest([0x5a; 32]),
            provider_origin_package: "omega::providers::root".to_owned(),
            service_schema: "Root".to_owned(),
            service_schema_package_identity: psi_core::PackageKeyIdentity::from_digest([0x5c; 32]),
            calling_plan_report_fingerprint: Some(0xfeed),
            calling_plan_commitment: Some(
                psi_typed_trees::typed_trees::BoundaryCallingPlanCommitment::from_digest(
                    [0xfe; 32],
                ),
            ),
            selected: false,
            requirement_owner: "Base".to_owned(),
            requirement_owner_package_identity: psi_core::PackageKeyIdentity::from_digest(
                [0x5d; 32],
            ),
            requirement_identity: "named-callable(path(Base::enter), parameters(), result(none))"
                .to_owned(),
            method: "enter".to_owned(),
            subject: "parameter:0".to_owned(),
            authority_flow: "accepts".to_owned(),
            domain: "Token::Granted".to_owned(),
            effective_carry: "strict".to_owned(),
            predicate_discharge_required: false,
            provenance: "own-package (dev-active)".to_owned(),
            grant_selectors: Vec::new(),
            standing_warning: true,
        }],
    };

    writer
        .write_trust_report(&report)
        .expect("trust report output");
    let output =
        std::fs::read_to_string(root.join("trust_report.md")).expect("written trust report");

    assert!(output.contains("requirement owner: Base"));
    assert!(output.contains("selected provider closure report fingerprint: 000000000000abcd"));
    assert!(output.contains("selected provider closure digest: 0x"));
    assert!(output.contains("plan report fingerprint: 0000000000001234"));
    assert!(output.contains("plan digest: 0x"));
    assert!(output.contains("provider type: RootProvider"));
    assert!(output.contains("target: windows_x86_64"));
    assert!(output.contains("provider origin package: omega::providers::root"));
    assert!(output.contains(&format!("provider package key: {}", "5a".repeat(32))));
    assert!(output.contains(&format!("provider type package: {}", "5b".repeat(32))));
    assert!(output.contains(&format!("service schema package: {}", "5c".repeat(32))));
    assert!(output.contains(&format!("requirement owner package: {}", "5d".repeat(32))));
    assert!(output.contains("own-package (dev-active)"));
    assert!(output.contains("service schema: Root"));
    assert!(output.contains("calling plan report fingerprint: 000000000000feed"));
    assert!(output.contains(&format!("calling plan commitment: 0x{}", "fe".repeat(32))));
    assert!(output.contains("selected: no"));
    assert!(output.contains("requirement identity: named-callable(path(Base::enter)"));
    assert!(!output.contains("requirement owner: Root"));
    std::fs::remove_dir_all(root).expect("remove test artifact directory");
}

#[test]
fn trust_report_keeps_claim_free_provider_requirement_blast_radius_exact() {
    let root = std::env::temp_dir().join(format!(
        "omega-trust-provider-requirement-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let writer = ArtifactWriter::new(&root).expect("artifact writer");
    let report = TrustReport {
        selected_provider_closure_report_fingerprint: 0x5678,
        selected_provider_closure_digest: test_selected_provider_closure_digest(),
        rows: Vec::new(),
        generic_accepted_instances: Vec::new(),
        provider_requirements: vec![TrustProviderRequirementRow {
            provider_plan: "RootProvider::satisfies::Root".to_owned(),
            provider_plan_report_fingerprint: 0x1234,
            provider_plan_digest: test_provider_plan_digest(),
            provider_type: String::new(),
            provider_type_package_identity: None,
            target: String::new(),
            provider_origin_package_identity: None,
            provider_origin_package: String::new(),
            service_schema: "Root".to_owned(),
            service_schema_package_identity: None,
            calling_plan_report_fingerprint: None,
            calling_plan_commitment: None,
            selected: true,
            requirement_owner: "Base".to_owned(),
            requirement_owner_package_identity: None,
            requirement_identity: "named-callable(path(Base::enter), parameters(), result(none))"
                .to_owned(),
            method: "enter".to_owned(),
            parameter_type_identities: vec!["Token in Granted".to_owned()],
            result_type_identity: Some("Token".to_owned()),
            service_reach: vec!["Clock".to_owned(), "Storage".to_owned()],
            synchronous_invocations: vec!["Callback".to_owned()],
            may_suspend: true,
            may_block: false,
            terminates_guarantee: true,
            termination_premises: vec![
                TrustProgressPremiseRow {
                    profile: "SchedulerHandle::WeakFair".to_owned(),
                    subject: TrustProgressPremiseSubject::ProviderReceiver,
                    subject_projections: vec!["Scheduler::handle".to_owned()],
                },
                TrustProgressPremiseRow {
                    profile: "Buffer::EventuallyReady".to_owned(),
                    subject: TrustProgressPremiseSubject::Parameter(0),
                    subject_projections: Vec::new(),
                },
            ],
            realization: TrustProviderRealization::VtableSlot { index: 4 },
            provenance: "root grant (build.omg)".to_owned(),
            grant_selectors: vec!["Root".to_owned()],
            standing_warning: false,
        }],
        qualifications: Vec::new(),
    };

    writer
        .write_trust_report(&report)
        .expect("trust report output");
    let output =
        std::fs::read_to_string(root.join("trust_report.md")).expect("written trust report");

    assert!(output.contains("provider requirements: 1"));
    assert!(output.contains("selected provider closure report fingerprint: 0000000000005678"));
    assert!(output.contains(
        "provider plan: RootProvider::satisfies::Root -- plan report fingerprint: 0000000000001234"
    ));
    assert!(output.contains("provider type: <free external>"));
    assert!(output.contains("target: <all>"));
    assert!(output.contains("provider origin package: <none>"));
    assert!(output.contains("service schema: Root"));
    assert!(output.contains("calling plan report fingerprint: <none>"));
    assert!(output.contains("calling plan commitment: <none>"));
    assert!(output.contains("selected: yes"));
    assert!(output.contains("requirement owner: Base"));
    assert!(output.contains("requirement identity: named-callable(path(Base::enter)"));
    assert!(output.contains("method: enter"));
    assert!(output.contains("parameter types: Token in Granted"));
    assert!(output.contains("result type: Token"));
    assert!(output.contains("service reach: Clock, Storage"));
    assert!(output.contains("synchronous invocations: Callback"));
    assert!(output.contains("may suspend: yes"));
    assert!(output.contains("may block: no"));
    assert!(output.contains("termination guarantee: yes"));
    assert!(output.contains(
        "progress premises: SchedulerHandle::WeakFair(provider-receiver(build-bound).Scheduler::handle), Buffer::EventuallyReady(parameter:0)"
    ));
    assert!(output.contains("realization: vtable slot 4"));
    assert!(output.contains("root grant (build.omg)"));
    assert!(output.contains("grant selectors: Root"));
    assert!(!output.contains("requirement owner: Root"));
    assert!(!output.contains("STANDING WARNING"));
    std::fs::remove_dir_all(root).expect("remove test artifact directory");
}

#[test]
fn trust_provider_realizations_distinguish_checked_and_opaque_leaves() {
    assert_eq!(
        TrustProviderRealization::CheckedAdapter {
            machine_identity: "ConsoleProvider::write".to_owned(),
            machine_package_identity: psi_core::PackageKeyIdentity::from_digest([0x5a; 32]),
        }
        .report_text(),
        "checked adapter `ConsoleProvider::write`"
    );
    assert_eq!(
        TrustProviderRealization::Syscall { number: 60 }.report_text(),
        "syscall 60"
    );
}

#[test]
fn normalized_foreign_locator_mutations_change_trust_identity_and_exact_output() {
    let baseline = normalized_windows_import(b"opaque\xff.dll", b"invoke_raw");
    let changed_library = normalized_windows_import(b"opaque\xfe.dll", b"invoke_raw");
    let changed_export = normalized_windows_import(b"opaque\xff.dll", b"invoke_next");

    assert_ne!(
        baseline.foreign_locator_compatibility_report_identity(),
        changed_library.foreign_locator_compatibility_report_identity(),
    );
    assert_ne!(
        baseline.foreign_locator_compatibility_report_identity(),
        changed_export.foreign_locator_compatibility_report_identity(),
    );

    let text = baseline.report_text();
    let identity = baseline
        .foreign_locator_compatibility_report_identity()
        .expect("normalized import identity");
    assert!(text.contains(&format!("PeByName [{identity:016x}]")));
    assert!(text.contains("target `windows_x86_64`"));
    assert!(text.contains("library bytes 0x6f7061717565ff2e646c6c"));
    assert!(text.contains("export bytes 0x696e766f6b655f726177"));
    assert!(!text.contains("opaque.dll"));

    let bootstrap = TrustProviderRealization::StringBackedImportBootstrap {
        library: "opaque.dll".to_owned(),
        symbol: "invoke_raw".to_owned(),
    };
    assert_eq!(
        bootstrap.foreign_locator_compatibility_report_identity(),
        None
    );
    assert!(
        bootstrap
            .report_text()
            .starts_with("string-backed import bootstrap")
    );
}

#[test]
fn trust_report_rejects_normalized_locator_under_a_different_target() {
    let root = std::env::temp_dir().join(format!(
        "omega-trust-foreign-target-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let writer = ArtifactWriter::new(&root).expect("artifact writer");
    let report = TrustReport {
        provider_requirements: vec![trust_provider_requirement(
            "linux_x86_64",
            normalized_windows_import(b"opaque.dll", b"invoke_raw"),
        )],
        ..Default::default()
    };

    let diagnostic = writer
        .write_trust_report(&report)
        .expect_err("mismatched target must fail before artifact installation");
    assert!(diagnostic.message.contains("targets `windows_x86_64`"));
    assert!(diagnostic.message.contains("reports target `linux_x86_64`"));
    assert!(!root.join("trust_report.md").exists());
    std::fs::remove_dir_all(root).expect("remove test artifact directory");
}

#[test]
fn value_placement_json_retains_indirect_copy_geometry() {
    let boundary = evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::MicrosoftX64,
        &CallSignature {
            parameters: vec![ValueShape::integer(16, 8)],
            result: None,
        },
    )
    .expect("Microsoft x64 aggregate placement");
    let json = value_placement_json(&boundary.plan().call.parameters[0]);

    assert!(json.contains("\"indirect\""));
    assert!(json.contains("\"copy_stack_byte_offset\": 32"));
    assert!(json.contains("\"byte_size\": 16"));
}

fn root_id<T>(identity: u64, constructor: fn(u64) -> Result<T, ExternalRootDiagnostic>) -> T {
    constructor(identity).expect("normalized root identity")
}

fn fuel_schedule() -> FuelScheduleIdentity {
    FuelScheduleIdentity::new(1).expect("canonical test fuel schedule")
}

fn install_id<T>(identity: u64, constructor: fn(u64) -> Result<T, InstallationDiagnostic>) -> T {
    constructor(identity).expect("normalized installation identity")
}

fn extent_id<T>(identity: u64, constructor: fn(u64) -> Result<T, ExtentDiagnostic>) -> T {
    constructor(identity).expect("normalized extent identity")
}

fn extent_provider_issuance(seed: u64) -> psi_extents::ExtentProviderIssuance {
    let base = seed * 16;
    psi_extents::ExtentProviderIssuance::from_normalized_identities([
        base + 1,
        base + 2,
        base + 3,
        base + 4,
        base + 5,
        base + 6,
        base + 7,
        base + 8,
        base + 9,
        base + 10,
        base + 11,
        base + 12,
        base + 13,
    ])
    .expect("normalized provider issuance")
}

fn installed_code_fixture(entry: EntryStubId) -> InstalledCode {
    let scope =
        ArtifactInstallationScopeId::from_normalized_identity(61).expect("installation scope");
    let constraints = PlacementConstraints::new(
        Some(PlacementAddressRange::new(0x1000, 0x1_0000).expect("placement range")),
        4096,
        PlacementPhase::PostHandoff,
        None,
        Some(scope),
    )
    .expect("placement constraints");
    let contracts = install_id(30, MachineContractSetId::from_normalized_identity);
    let footprint = install_id(31, MachineFootprintId::from_normalized_identity);
    let authority_commitments = ArtifactAuthorityCommitments::from_canonical_evidence(
        contracts,
        b"test imported contract set",
        footprint,
        b"test declared footprint",
        None,
        Some((scope, b"test installation scope")),
    );
    let artifact = Artifact::from_canonical_decode(
        install_id(3, ArtifactId::from_normalized_identity),
        Architecture::X86_64,
        vec![0; 64],
        contracts,
        footprint,
        install_id(32, PlacementPlanId::from_normalized_identity),
        constraints.clone(),
        install_id(33, EntrySetId::from_normalized_identity),
        vec![ArtifactEntry::from_canonical_decode(entry, 16)],
        install_id(34, RelocationSetId::from_normalized_identity),
        Vec::new(),
        authority_commitments,
    )
    .expect("artifact");
    let admitted = admit_executable(
        &artifact,
        ArtifactAdmissionEvidence::from_validator(
            install_id(40, AdmissionReceiptId::from_normalized_identity),
            &artifact,
            true,
        ),
    )
    .expect("admitted artifact");
    let rights = ExtentRights::from_normalized_identities([extent_id(
        51,
        ExtentRightId::from_normalized_identity,
    )]);
    let extent = ExtentRootGrant::from_admitted_provider(
        extent_provider_issuance(100),
        extent_id(100, ExtentLineageId::from_normalized_identity),
        extent_id(50, AddressSpaceId::from_normalized_identity),
        rights.clone(),
        extent_id(52, ExtentProvenanceId::from_normalized_identity),
        extent_id(53, MappingEraId::from_normalized_identity),
    )
    .mint(0x1000, 4096)
    .expect("placement extent");
    let placement = CodePlacementAuthority::from_admitted_provider(
        install_id(100, CodePlacementId::from_normalized_identity),
        install_id(61, InstallationScopeId::from_normalized_identity),
        InstallationAudience::FutureFetcher,
        &extent,
        rights,
        constraints,
        PlacementSite {
            base_address: 0x1000,
            phase: PlacementPhase::PostHandoff,
            machine_regime: None,
            installation_scope: Some(scope),
        },
    )
    .claim(extent)
    .expect("placement");
    let materialized = materialize_admitted_artifact(&admitted, &placement, |_| None)
        .expect("artifact without relocations materializes");
    let frozen = materialize_and_freeze(
        &admitted,
        placement,
        materialized.clone(),
        MaterializationReceipt::from_materialized(
            &materialized,
            install_id(71, MachineFootprintId::from_normalized_identity),
            true,
        ),
    )
    .expect("frozen placement");
    let certificate = FinalValidationCertificate::from_validator(
        install_id(180, FinalValidationId::from_normalized_identity),
        &frozen,
        true,
    );
    let validated = validate_final_placement(frozen, &certificate).expect("validated placement");
    let install_authority = InstallAuthority::from_admitted_provider(&validated);
    let receipt = InstallationReceipt::from_provider(
        install_id(300, InstalledCodeId::from_normalized_identity),
        &validated,
        true,
        WxEnforcement::HardwareEnforced,
    );
    install_validated(validated, install_authority, receipt).expect("installed code")
}

fn entry_id(identity: u64) -> EntryStubId {
    EntryStubId::from_normalized_identity(identity).expect("normalized entry identity")
}

fn executable_container_fixture() -> Artifact {
    let artifact_id = install_id(900, ArtifactId::from_normalized_identity);
    let contracts = install_id(901, MachineContractSetId::from_normalized_identity);
    let footprint = install_id(902, MachineFootprintId::from_normalized_identity);
    let placement_plan = install_id(903, PlacementPlanId::from_normalized_identity);
    let entry_set = install_id(904, EntrySetId::from_normalized_identity);
    let relocation_set = install_id(905, RelocationSetId::from_normalized_identity);
    let code = vec![0xc3];
    let entries = vec![ArtifactEntry::from_canonical_decode(entry_id(906), 0)];
    let placement_constraints =
        PlacementConstraints::new(None, 1, PlacementPhase::Load, None, None)
            .expect("placement constraints");
    let authority_commitments = ArtifactAuthorityCommitments::from_canonical_evidence(
        contracts,
        b"test imported contract set",
        footprint,
        b"test declared footprint",
        None,
        None,
    );
    let mut decoded = DecodedArtifactContainer {
        format_marker: omega_executable_installation::OMEGA_EXECUTABLE_CONTAINER_MARKER,
        total_length: 1,
        artifact: artifact_id,
        content_fingerprint:
            omega_executable_installation::NonAuthoritativeContainerFingerprint64::from_compatibility_value(1)
                .unwrap(),
        architecture: Architecture::X86_64,
        code_length: code.len() as u64,
        code: code.clone(),
        contracts,
        declared_footprint: footprint,
        placement_plan,
        placement_constraints,
        entry_set,
        entries: entries.clone(),
        relocation_set,
        relocations: Vec::new(),
        proof_payload: omega_executable_installation::normalized_proof_payload_digest(b""),
        proof: Vec::new(),
        authority_commitments: Some(authority_commitments),
        sections: Vec::new(),
    };
    decoded.content_fingerprint =
        omega_executable_installation::non_authoritative_decoded_container_fingerprint(&decoded)
            .expect("normalized content fingerprint");
    let content = normalized_decoded_content_digest(&decoded).expect("normalized content digest");
    let artifact = Artifact::from_canonical_decode(
        artifact_id,
        Architecture::X86_64,
        code,
        contracts,
        footprint,
        placement_plan,
        placement_constraints,
        entry_set,
        entries,
        relocation_set,
        Vec::new(),
        authority_commitments,
    )
    .expect("canonical artifact");
    assert_eq!(artifact.content(), content);
    artifact
}

#[test]
fn writes_canonical_executable_container_atomically() {
    let root = std::env::temp_dir().join(format!(
        "omega-artifact-container-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let writer = ArtifactWriter::new(&root).expect("artifact writer");
    let limits = ContainerLimits {
        max_total_bytes: 64 * 1024,
        max_sections: 16,
        max_section_bytes: 32 * 1024,
        max_relocations: 64,
    };
    let artifact = executable_container_fixture();

    let path = writer
        .write_executable_container("program.omega-artifact", &artifact, b"proof", limits)
        .expect("canonical artifact output");
    let bytes = std::fs::read(&path).expect("written artifact bytes");
    let decoded =
        decode_executable_container(&bytes, limits).expect("written bytes remain canonical");

    assert_eq!(decoded.artifact(), &artifact);
    assert_eq!(decoded.proof(), b"proof");
    assert!(!root.join(".program.omega-artifact.tmp").exists());
    std::fs::remove_dir_all(root).expect("remove test artifact directory");
}

#[test]
fn external_root_manifest_is_complete_normalized_and_address_free() {
    let boundary = evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::SystemVAMD64,
        &CallSignature {
            parameters: vec![ValueShape::integer(8, 8)],
            result: None,
        },
    )
    .expect("boundary plan");
    let leaf = FixedFuelProviderSummary::from_admitted_provider(
        root_id(21, ProviderFuelSummaryId::from_normalized_identity),
        root_id(22, RootProviderId::from_normalized_identity),
        fuel_schedule(),
        4,
        BTreeSet::new(),
        root_id(
            23,
            ProviderFuelValidationReceiptId::from_normalized_identity,
        ),
    );
    let work_root = FixedFuelProviderSummary::from_admitted_provider(
        root_id(20, ProviderFuelSummaryId::from_normalized_identity),
        root_id(8, RootProviderId::from_normalized_identity),
        fuel_schedule(),
        3,
        BTreeSet::from([FixedFuelCall {
            callee: leaf.identity,
            maximum_invocations: 2,
        }]),
        root_id(
            24,
            ProviderFuelValidationReceiptId::from_normalized_identity,
        ),
    );
    let composed_fuel =
        compose_fixed_fuel(work_root.identity, [&work_root, &leaf]).expect("fixed fuel");
    let root_identity = root_id(1, ExternalRootId::from_normalized_identity);
    let nesting_identity = root_id(11, NestingRelationId::from_normalized_identity);
    let entry = entry_id(2);
    let installed_code = installed_code_fixture(entry);
    let stack_summary = ProviderStackSummary::from_admitted_provider(
        root_identity,
        root_id(8, RootProviderId::from_normalized_identity),
        EntryStack::ProviderSelected,
        2048,
        16,
        root_id(29, StackValidationReceiptId::from_normalized_identity),
    );
    let stack_realization = validate_entry_stack_realization(EntryStackRealization {
        contexts: vec![ArrivalContextRealization {
            context: ArrivalContextId::new(1).expect("arrival context"),
            epochs: vec![EntryStackEpoch {
                stage: EntryStackStage::Body,
                active_domain: StackDomainRef::Interrupted,
                occupancy_by_domain: Vec::new(),
                nesting: Preemption::NotApplicable,
            }],
        }],
    })
    .expect("entry stack realization");
    let arrival_contexts = omega_external_roots::admit_opaque_arrival_context_set(
        &stack_summary,
        &boundary,
        &installed_code,
        entry,
        vec![ArrivalContextId::new(1).expect("arrival context")],
        root_id(30, StackValidationReceiptId::from_normalized_identity),
    )
    .expect("arrival-context admission");
    let bound_stack = bind_opaque_adapter_stack_realization(
        &stack_summary,
        &boundary,
        &installed_code,
        entry,
        stack_realization,
        arrival_contexts,
    )
    .expect("bound stack realization");
    let composed_stack = compose_bound_entry_stack_epochs(
        &StackNestingRelation {
            identity: nesting_identity,
            edges: BTreeSet::new(),
        },
        [&bound_stack],
    )
    .expect("stack composition");
    let record = InstalledRootRecord {
        root: root_identity,
        normalized_root_report_identity: 0x101,
        entry,
        installed_code: installed_code.identity(),
        artifact: installed_code.artifact(),
        slot: root_id(5, RootSlotId::from_normalized_identity),
        owner: root_id(6, RootSlotOwnerId::from_normalized_identity),
        admission: root_id(7, RootAdmissionId::from_normalized_identity),
        provider_execution: root_id(30, ProviderExecutionId::from_normalized_identity),
        provider_execution_report_fingerprint: 0x3030,
        provider_exit_assurance: OpaqueProviderExitAssurance::AcceptedClaim {
            realization: ProviderExitRealization {
                control: boundary.plan().call.entry_control,
                restored_state: boundary.plan().state.restored_state,
            },
            validation_receipt: root_id(10, TrustReceiptId::from_normalized_identity),
        },
        provider_exit_assurance_report_fingerprint: 0x3031,
        provider_plan: root_id(31, ProviderPlanId::from_normalized_identity),
        requirement_identity: "TestRoot::entry".into(),
        entry_claims: Vec::new(),
        acknowledgement_parameter_index: None,
        interrupt_mask_guard_claim: None,
        service_reach: vec!["MachineControl".into()],
        selected_provider_closure_report_fingerprint: 0x3033,
        selected_provider_closure_digest: omega_effects::SelectedProviderPlanFacts::default()
            .identity_digest(),
        installation_reach_resolutions: Vec::new(),
        boundary_contract_report_fingerprint: boundary.contract_report_fingerprint(),
        boundary: boundary.plan().clone(),
        provider: root_id(8, RootProviderId::from_normalized_identity),
        effects: BTreeSet::from([root_id(9, RootEffectId::from_normalized_identity)]),
        trust_receipts: BTreeSet::from([root_id(10, TrustReceiptId::from_normalized_identity)]),
        nesting_relation: nesting_identity,
        acknowledgement_policy: Some(root_id(
            12,
            AcknowledgementPolicyId::from_normalized_identity,
        )),
        stack: StackResourceColumn {
            ceiling_bytes: 8192,
            realization: composed_stack,
            validation_receipt: root_id(25, StackValidationReceiptId::from_normalized_identity),
        },
        logical_fuel: LogicalFuelResourceColumn {
            schedule: fuel_schedule(),
            provision: root_id(28, FuelProvisionId::from_normalized_identity),
            ceiling_units: 64,
            realization: composed_fuel,
            validation_receipt: root_id(26, FuelValidationReceiptId::from_normalized_identity),
        },
        machine_state: MachineStateResourceColumn {
            realization: StateFootprintEvidence::new(
                RegisterSet::new([MachineRegister::X86Rax]),
                MachineStateSet::new([MachineState::Flags]),
            ),
            validation_receipt: root_id(27, StateValidationReceiptId::from_normalized_identity),
        },
        component_pins: BTreeSet::from([ComponentVersionPin {
            contract: root_id(13, ComponentContractId::from_normalized_identity),
            artifact: root_id(14, ComponentArtifactId::from_normalized_identity),
            provider: root_id(15, ComponentProviderId::from_normalized_identity),
            version: root_id(16, ComponentVersionPinId::from_normalized_identity),
        }]),
    };

    let first = external_root_records_manifest_json(0x202, &[&record]);
    let second = external_root_records_manifest_json(0x202, &[&record]);
    let parsed: serde_json::Value = serde_json::from_str(&first).expect("valid JSON manifest");

    assert_eq!(first, second);
    assert_eq!(parsed["root_count"], 1);
    assert_eq!(
        parsed["roots"][0]["normalized_root_report_identity"],
        "0x0000000000000101"
    );
    assert_eq!(parsed["roots"][0]["entry"], "0x0000000000000002");
    assert_eq!(
        parsed["roots"][0]["provider_execution"],
        "0x000000000000001e"
    );
    assert_eq!(parsed["roots"][0]["provider_plan"], "0x000000000000001f");
    assert_eq!(
        parsed["roots"][0]["selected_provider_closure_report_fingerprint"],
        "0x0000000000003033"
    );
    assert_eq!(
        parsed["roots"][0]["selected_provider_closure_digest"]
            .as_str()
            .expect("selected closure digest")
            .len(),
        66
    );
    assert_eq!(
        parsed["roots"][0]["boundary_plan"]["call"]["policy"],
        "system_v_amd64"
    );
    assert_eq!(
        parsed["roots"][0]["boundary_plan"]["state"]["stack"],
        "provider_selected"
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["stack"]["composed_wcsu_bytes"],
        2048
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["stack"]["summary_evidence"][0]["origin"],
        "admitted_provider"
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["stack"]["composed_domains"][0]["domain"]["kind"],
        "interrupted"
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["stack"]["summary_evidence"][0]["arrival_contexts"][0]["epochs"]
            [0]["stage"],
        "body"
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["stack"]["summary_evidence"][0]["entry_installed_code"],
        "0x000000000000012c"
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["stack"]["summary_evidence"][0]["adapter_origin"],
        "opaque_provider"
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["stack"]["summary_evidence"][0]["arrival_origin"],
        "opaque_provider"
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["stack"]["summary_evidence"][0]["target_arrival_rule_fingerprint"],
        serde_json::Value::Null
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["stack"]["summary_evidence"][0]["body_domains"][0]["context"],
        "0x0000000000000001"
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["stack"]["summary_evidence"][0]["body_domains"][0]["domain"]
            ["kind"],
        "interrupted"
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["stack"]["summary_evidence"][0]["opaque_validation_receipt"],
        "0x000000000000001e"
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["logical_fuel"]["composed_units"],
        11
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["logical_fuel"]["schedule_marker"],
        1
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["logical_fuel"]["provision"],
        "0x000000000000001c"
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["logical_fuel"]["summary_evidence"][0]["origin"],
        "admitted_provider"
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["logical_fuel"]["summary_evidence"][1]["origin"],
        "admitted_provider"
    );
    assert!(
        parsed["roots"][0]["resources"]
            .get("structural_work")
            .is_none()
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["machine_state"]["realized_registers"][0],
        "x86_rax"
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["machine_state"]["ceiling"]["permitted_transitive_use_bits"],
        "0x0007"
    );
    assert_eq!(
        parsed["roots"][0]["resources"]["stack"]["validation_receipt"],
        "0x0000000000000019"
    );
    assert_eq!(parsed["roots"][0]["effects"][0], "0x0000000000000009");
    assert_eq!(
        parsed["roots"][0]["component_pins"][0]["version"],
        "0x0000000000000010"
    );
    assert!(!first.contains("entry_address"));
    assert!(!first.contains("code_address"));
    assert!(!first.contains("ranking"));
    assert!(!first.contains("codegen"));
}
