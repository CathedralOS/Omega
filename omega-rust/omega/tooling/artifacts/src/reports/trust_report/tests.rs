//! Trust report identity, validation, and rendering regressions.

use super::{
    TrustCrashCause, TrustCrashRouteBucket, TrustCrashRouteGuard, TrustGenericAcceptedInstanceRow,
    TrustProgressPremiseRow, TrustProgressPremiseSubject, TrustProviderRealization,
    TrustProviderRequirementRow, TrustQualificationRow, TrustReport, TrustReportRow,
};
use crate::ArtifactWriter;
use effects::provider_plan::{
    EvaluatedBindingEvaluationDigest, EvaluatedBindingMaterializationDigest,
    EvaluatedBindingProducerClosureDigest, EvaluatedBindingReceipt, EvaluatedBindingUsage,
    EvaluatedForeignImport,
};
use effects::{ForeignLocatorCandidate, NormalizedForeignLocator, normalize_foreign_locator};
use target::TargetProfile;

fn evaluated_import(locator: NormalizedForeignLocator, seed: u8) -> EvaluatedForeignImport {
    let usage = EvaluatedBindingUsage::from_evaluator(7, 1, 10, 1_000, 0, 0, 4, 12, 3, 0)
        .expect("valid fixture usage");
    let receipt = EvaluatedBindingReceipt::from_evaluation(
        None,
        format!("fixture::producer::{seed}"),
        EvaluatedBindingProducerClosureDigest::from_bytes([seed; 32]).unwrap(),
        1,
        usage,
        EvaluatedBindingEvaluationDigest::from_bytes([seed.wrapping_add(1); 32]).unwrap(),
        1,
        EvaluatedBindingMaterializationDigest::from_bytes([seed.wrapping_add(2); 32]).unwrap(),
        locator.identity_digest(),
    )
    .expect("valid fixture receipt");
    EvaluatedForeignImport::from_retained_evidence(locator, receipt)
        .expect("receipt matches fixture locator")
}

fn normalized_windows_import(library: &[u8], export: &[u8]) -> TrustProviderRealization {
    let locator = normalize_foreign_locator(
        ForeignLocatorCandidate::PeByName {
            library: library.to_vec(),
            export: export.to_vec(),
        },
        TargetProfile::WindowsX64,
    )
    .expect("valid normalized Windows import");
    TrustProviderRealization::Import {
        evaluated: evaluated_import(locator, 11),
    }
}

fn normalized_macos_import(install_name: &[u8], symbol: &[u8]) -> TrustProviderRealization {
    let locator = normalize_foreign_locator(
        ForeignLocatorCandidate::MachODylibSymbol {
            install_name: install_name.to_vec(),
            symbol: symbol.to_vec(),
        },
        TargetProfile::MacosArm64,
    )
    .expect("valid normalized Mach-O import");
    TrustProviderRealization::Import {
        evaluated: evaluated_import(locator, 21),
    }
}

fn test_provider_plan_digest() -> effects::provider_plan::ProviderPlanDigest {
    effects::provider_plan::ProviderPlan::default().identity_digest()
}

fn test_selected_provider_closure_digest() -> effects::SelectedProviderClosureDigest {
    effects::SelectedProviderPlanFacts::default().identity_digest()
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
                    checked_trees::MachineContractCommitment::from_digest([0xab; 32]),
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
                    checked_trees::MachineContractCommitment::from_digest([0xbc; 32]),
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
            instance_contract_commitment: checked_trees::MachineContractCommitment::from_digest(
                [0xaa; 32],
            ),
            type_argument_identities: vec!["named(name(Card))".to_owned()],
            const_argument_identities: vec!["named(name(1))".to_owned()],
            machine_argument_contract_report_fingerprints: vec![0x3333],
            machine_argument_contract_commitments: vec![
                checked_trees::MachineContractCommitment::from_digest([0x33; 32]),
            ],
            conformance_argument_report_fingerprints: vec![0x4444],
            conformance_argument_commitments: vec![
                typed_trees::typed_trees::ClosedConformanceApplicationCommitment::from_digest(
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
        instance_contract_commitment: checked_trees::MachineContractCommitment::from_digest(
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
        checked_trees::MachineContractCommitment::from_digest([0x22; 32]);
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
            provider_type_package_identity: semantic_vocabulary::PackageKeyIdentity::from_digest(
                [0x5b; 32],
            ),
            target: "windows_x86_64".to_owned(),
            provider_origin_package_identity: semantic_vocabulary::PackageKeyIdentity::from_digest(
                [0x5a; 32],
            ),
            provider_origin_package: "omega::providers::root".to_owned(),
            service_schema: "Root".to_owned(),
            service_schema_package_identity: semantic_vocabulary::PackageKeyIdentity::from_digest(
                [0x5c; 32],
            ),
            calling_plan_report_fingerprint: Some(0xfeed),
            calling_plan_commitment: Some(
                typed_trees::typed_trees::BoundaryCallingPlanCommitment::from_digest([0xfe; 32]),
            ),
            selected: false,
            requirement_owner: "Base".to_owned(),
            requirement_owner_package_identity:
                semantic_vocabulary::PackageKeyIdentity::from_digest([0x5d; 32]),
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
            machine_package_identity: semantic_vocabulary::PackageKeyIdentity::from_digest(
                [0x5a; 32]
            ),
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
}

#[test]
fn macho_locator_trust_report_keeps_raw_install_name_and_symbol() {
    let baseline = normalized_macos_import(b"/usr/lib/libSystem.B.dylib", b"_write\xff");
    assert_ne!(
        baseline.foreign_locator_compatibility_report_identity(),
        normalized_macos_import(b"/usr/lib/libobjc.A.dylib", b"_write\xff")
            .foreign_locator_compatibility_report_identity(),
    );
    assert_ne!(
        baseline.foreign_locator_compatibility_report_identity(),
        normalized_macos_import(b"/usr/lib/libSystem.B.dylib", b"_read\xff")
            .foreign_locator_compatibility_report_identity(),
    );

    let text = baseline.report_text();
    assert!(text.contains("MachODylibSymbol ["));
    assert!(text.contains("target `macos_arm64`"));
    assert!(
        text.contains("install-name bytes 0x2f7573722f6c69622f6c696253797374656d2e422e64796c6962")
    );
    assert!(text.contains("symbol bytes 0x5f7772697465ff"));
    assert!(!text.contains("_write"));
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
