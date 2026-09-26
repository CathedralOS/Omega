//! Selected provider plan fact tests.

use super::{InstallationReachResolution, ProviderPlan, SelectedProviderPlanFacts};
use crate::provider_plan::{
    EvaluatedBindingEvaluationDigest, EvaluatedBindingMaterializationDigest,
    EvaluatedBindingProducerClosureDigest, EvaluatedBindingReceipt, EvaluatedBindingUsage,
    EvaluatedForeignImport, ProviderBinding, ProviderPlanRow, ServiceMethod, ServiceSchema,
};

fn evaluated_import(locator: crate::NormalizedForeignLocator, seed: u8) -> EvaluatedForeignImport {
    let usage = EvaluatedBindingUsage::from_evaluator(7, 1, 10, 1_000, 0, 0, 4, 12, 3, 0).unwrap();
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
    .unwrap();
    EvaluatedForeignImport::from_retained_evidence(locator, receipt).unwrap()
}

fn candidate(name: &str, method: &str) -> ProviderPlan {
    ProviderPlan {
        name: name.into(),
        provider_type: format!("{name}Provider"),
        provider_type_package_identity: None,
        target: "x86_64-unknown-none".into(),
        schema: ServiceSchema {
            trait_name: format!("{name}Service"),
            trait_package_identity: None,
            methods: vec![ServiceMethod {
                name: method.into(),
                requirement_owner: format!("{name}Service"),
                requirement_owner_package_identity: None,
                requirement_identity: format!("{name}Service::{method}"),
                parameter_count: 0,
                parameter_type_identities: Vec::new(),
                entry_claims: Vec::new(),
                has_result: false,
                result_type_identity: None,
                result_claims: Vec::new(),
                service_reach: vec![format!("{name}Service")],
                synchronous_invocations: Vec::new(),
                may_suspend: false,
                may_block: false,
                terminates_guarantee: false,
                termination_premises: Vec::new(),
                calling_plan_report_fingerprint: None,
                calling_plan_commitment: None,
            }],
        },
        rows: vec![ProviderPlanRow {
            method: method.into(),
            requirement_identity: format!("{name}Service::{method}"),
            requirement_lifetime_partition: Vec::new(),
            binding: ProviderBinding::CheckedAdapter {
                machine_identity: format!("{name}Provider::{method}"),
                machine_package_identity: None,
            },
        }],
        origin_package_identity: None,
        origin_package: "test".into(),
    }
}

/// One opaque Windows import spelled through the evaluated locator route, as
/// the selected-plan row and as the matching opaque admission binding.
fn opaque_import(
    library: &[u8],
    export: &[u8],
    seed: u8,
) -> (ProviderBinding, crate::OpaqueInProcessBinding) {
    let locator = crate::normalize_foreign_locator(
        crate::ForeignLocatorCandidate::PeByName {
            library: library.to_vec(),
            export: export.to_vec(),
        },
        target::TargetProfile::WindowsX64,
    )
    .expect("normalized opaque import");
    let evaluated = evaluated_import(locator, seed);
    (
        ProviderBinding::Import {
            evaluated: evaluated.clone(),
        },
        crate::OpaqueInProcessBinding::Import { evaluated },
    )
}

fn opaque_candidate(
    name: &str,
    method: &str,
    library: &[u8],
    export: &[u8],
    seed: u8,
) -> (ProviderPlan, crate::OpaqueInProcessBinding) {
    let mut plan = candidate(name, method);
    plan.target = "windows_x86_64".into();
    let (binding, opaque_binding) = opaque_import(library, export, seed);
    plan.rows[0].binding = binding;
    (plan, opaque_binding)
}

#[test]
fn selected_plans_are_retained_in_canonical_order() {
    let alpha = candidate("Alpha", "read");
    let beta = candidate("Beta", "write");
    let candidates = vec![beta.clone(), alpha.clone()];

    let first =
        SelectedProviderPlanFacts::from_selection(&candidates, &["Beta".into(), "Alpha".into()])
            .expect("valid selection");
    let second =
        SelectedProviderPlanFacts::from_selection(&candidates, &["Alpha".into(), "Beta".into()])
            .expect("valid selection");

    assert_eq!(first, second);
    assert_eq!(
        first
            .plans()
            .iter()
            .map(|plan| plan.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Alpha", "Beta"]
    );
    assert_eq!(
        first
            .plan_by_report_fingerprint(alpha.report_fingerprint())
            .map(|plan| plan.name.as_str()),
        Some("Alpha")
    );
}

#[test]
fn exact_plan_lookup_rejects_a_substitute_claiming_the_same_report_identity() {
    let selected_plan = candidate("Alpha", "read");
    let mut substituted_plan = selected_plan.clone();
    substituted_plan.rows[0].binding = ProviderBinding::CheckedAdapter {
        machine_identity: "AlphaProvider::substituted_read".into(),
        machine_package_identity: None,
    };
    let report_identity = selected_plan.report_fingerprint();
    let selected = SelectedProviderPlanFacts::from_selected_plans(vec![selected_plan.clone()])
        .expect("selected provider plan");

    assert!(
        selected
            .plan_by_exact_evidence(report_identity, &substituted_plan)
            .is_none(),
        "claiming a collision-equal compact report identity cannot replace exact plan evidence"
    );
    assert_eq!(
        selected.plan_by_exact_evidence(report_identity, &selected_plan),
        Some(&selected_plan)
    );
}

#[test]
fn selected_closure_digest_distinguishes_compact_equal_calling_plans() {
    let mut first = candidate("Alpha", "read");
    first.schema.methods[0].calling_plan_report_fingerprint = Some(0x1234);
    first.schema.methods[0].calling_plan_commitment =
        Some(crate::provider_plan::BoundaryCallingPlanCommitment::from_digest([0x11; 32]));
    let mut substituted = first.clone();
    substituted.schema.methods[0].calling_plan_commitment =
        Some(crate::provider_plan::BoundaryCallingPlanCommitment::from_digest([0x22; 32]));

    assert_eq!(first.report_fingerprint(), substituted.report_fingerprint());
    assert_ne!(first.identity_digest(), substituted.identity_digest());
    let first = SelectedProviderPlanFacts::from_selected_plans(vec![first])
        .expect("first exact selected closure");
    let substituted = SelectedProviderPlanFacts::from_selected_plans(vec![substituted])
        .expect("substituted exact selected closure");
    assert_eq!(first.report_fingerprint(), substituted.report_fingerprint());
    assert_ne!(first.identity_digest(), substituted.identity_digest());
}

#[test]
fn resolved_selection_retains_same_spelled_plans_from_distinct_packages() {
    let first_package = semantic_vocabulary::PackageKeyIdentity::from_digest([0x31; 32])
        .expect("nonzero package identity");
    let second_package = semantic_vocabulary::PackageKeyIdentity::from_digest([0x32; 32])
        .expect("nonzero package identity");
    let mut first = candidate("Shared", "run");
    first.schema.trait_package_identity = Some(first_package);
    let mut second = first.clone();
    second.schema.trait_package_identity = Some(second_package);

    let selected =
        SelectedProviderPlanFacts::from_selected_plans(vec![second.clone(), first.clone()])
            .expect("package-qualified slots remain distinct after resolution");

    assert_eq!(selected.plans().len(), 2);
    assert!(selected.plans().contains(&first));
    assert!(selected.plans().contains(&second));
    assert!(
        SelectedProviderPlanFacts::from_selected_plans(vec![first.clone(), first.clone()])
            .expect_err("the same resolved plan cannot be retained twice")
            .contains("appears more than once")
    );
    assert!(
        SelectedProviderPlanFacts::from_selection(&[first, second], &["Shared".to_owned()])
            .expect_err("legacy name-only selection cannot choose between packages")
            .contains("matches 2 candidates")
    );
}

#[test]
fn identity_lookup_distinguishes_same_readable_name_across_packages() {
    let first_package = semantic_vocabulary::PackageKeyIdentity::from_digest([0x41; 32])
        .expect("nonzero package identity");
    let second_package = semantic_vocabulary::PackageKeyIdentity::from_digest([0x42; 32])
        .expect("nonzero package identity");
    let mut first = candidate("Shared", "run");
    first.origin_package_identity = Some(first_package);
    first.provider_type_package_identity = Some(first_package);
    first.schema.trait_package_identity = Some(first_package);
    first.schema.methods[0].requirement_owner_package_identity = Some(first_package);
    let ProviderBinding::CheckedAdapter {
        machine_package_identity,
        ..
    } = &mut first.rows[0].binding
    else {
        panic!("test candidate must use a checked adapter");
    };
    *machine_package_identity = Some(first_package);
    let mut second = first.clone();
    second.origin_package_identity = Some(second_package);
    second.provider_type_package_identity = Some(second_package);
    second.schema.trait_package_identity = Some(second_package);
    second.schema.methods[0].requirement_owner_package_identity = Some(second_package);
    let ProviderBinding::CheckedAdapter {
        machine_package_identity,
        ..
    } = &mut second.rows[0].binding
    else {
        panic!("test candidate must use a checked adapter");
    };
    *machine_package_identity = Some(second_package);

    let first_identity = first.report_fingerprint();
    let second_identity = second.report_fingerprint();
    assert_ne!(first_identity, second_identity);

    let selected = SelectedProviderPlanFacts::from_selected_plans(vec![first, second])
        .expect("package-qualified same-readable-name plans remain independently addressable");

    assert_eq!(
        selected
            .plan_by_report_fingerprint(first_identity)
            .and_then(|plan| plan.origin_package_identity),
        Some(first_package)
    );
    assert_eq!(
        selected
            .plan_by_report_fingerprint(second_identity)
            .and_then(|plan| plan.origin_package_identity),
        Some(second_package)
    );
}

#[test]
fn installation_reach_resolution_is_exact_bounded_selected_evidence() {
    let plan = candidate("Interrupt", "complete");
    let plan_identity = plan.report_fingerprint();
    let selected = SelectedProviderPlanFacts::from_selection(
        std::slice::from_ref(&plan),
        std::slice::from_ref(&plan.name),
    )
    .expect("selected provider");
    let base_identity = selected.report_fingerprint();
    let requirement_identity = plan.schema.methods[0].requirement_identity.clone();
    let resolved = selected
        .with_installation_reach_resolutions(vec![InstallationReachResolution {
            requirement_identity: requirement_identity.clone(),
            provider_plan_report_identity: plan_identity,
            upper_bound: vec!["PortIo".into(), "MachineControl".into()],
            resolved_row: vec!["PortIo".into()],
        }])
        .expect("selected row refines its bound");

    assert_ne!(resolved.report_fingerprint(), base_identity);
    let row = resolved
        .installation_reach_resolution(&requirement_identity)
        .expect("exact requirement resolution");
    assert_eq!(row.upper_bound, ["MachineControl", "PortIo"]);
    assert_eq!(row.resolved_row, ["PortIo"]);
    assert_eq!(
        resolved
            .resolve_installation_reach(
                &["InterruptCompletion".into(), "MachineControl".into()],
                std::slice::from_ref(&requirement_identity),
            )
            .expect("selected row closes the root"),
        ["InterruptCompletion", "MachineControl", "PortIo"]
    );
    assert!(
        resolved
            .resolve_installation_reach(&[], &["Missing::requirement".into()])
            .expect_err("final admission rejects unresolved rows")
            .contains("remains unresolved at final admission")
    );

    let outside = SelectedProviderPlanFacts::from_selection(
        std::slice::from_ref(&plan),
        std::slice::from_ref(&plan.name),
    )
    .expect("selected provider")
    .with_installation_reach_resolutions(vec![InstallationReachResolution {
        requirement_identity,
        provider_plan_report_identity: plan_identity,
        upper_bound: vec!["MachineControl".into()],
        resolved_row: vec!["FilesystemHost".into()],
    }])
    .expect_err("resolved row outside the bound must reject");
    assert!(outside.contains("exceeds its published upper bound"));
}

/// Two root traits inheriting `InterruptEntry` each carry the parent's exact
/// `InterruptEntry::enter` requirement identity in their provider-plan rows —
/// provenance keeps the declaring trait's identity so conformance matching
/// stays exact. Their installation-reach resolutions therefore share one
/// requirement identity and are distinguished only by the selected plan that
/// realized the row.
fn interrupt_root_candidate(name: &str, trait_name: &str) -> ProviderPlan {
    let mut plan = candidate(name, "enter");
    plan.schema.trait_name = trait_name.into();
    plan.schema.methods[0].requirement_owner = "InterruptEntry".into();
    plan.schema.methods[0].requirement_identity = "InterruptEntry::enter".into();
    plan.rows[0].requirement_identity = "InterruptEntry::enter".into();
    plan
}

#[test]
fn shared_inherited_requirement_resolves_per_selected_root_plan() {
    let fatal = interrupt_root_candidate("FatalExceptionProvider", "FatalExceptionRoot");
    let timer = interrupt_root_candidate("TimerProvider", "TimerRoot");
    let acknowledgement = candidate("Acknowledgement", "complete");
    let fatal_plan = fatal.report_fingerprint();
    let timer_plan = timer.report_fingerprint();
    let acknowledgement_plan = acknowledgement.report_fingerprint();
    let shared_requirement = "InterruptEntry::enter".to_owned();
    let acknowledgement_requirement = acknowledgement.schema.methods[0]
        .requirement_identity
        .clone();
    let build = || {
        SelectedProviderPlanFacts::from_selection(
            &[fatal.clone(), timer.clone(), acknowledgement.clone()],
            &[
                fatal.name.clone(),
                timer.name.clone(),
                acknowledgement.name.clone(),
            ],
        )
        .expect("distinct root boundary slots select together")
    };
    let resolutions = || {
        vec![
            InstallationReachResolution {
                requirement_identity: shared_requirement.clone(),
                provider_plan_report_identity: fatal_plan,
                upper_bound: vec!["MachineControl".into(), "PortIo".into()],
                resolved_row: vec!["PortIo".into()],
            },
            InstallationReachResolution {
                requirement_identity: shared_requirement.clone(),
                provider_plan_report_identity: timer_plan,
                upper_bound: vec!["MachineControl".into(), "PortIo".into()],
                resolved_row: vec!["MachineControl".into()],
            },
            InstallationReachResolution {
                requirement_identity: acknowledgement_requirement.clone(),
                provider_plan_report_identity: acknowledgement_plan,
                upper_bound: vec!["PortIo".into()],
                resolved_row: vec!["PortIo".into()],
            },
        ]
    };
    let selected = build()
        .with_installation_reach_resolutions(resolutions())
        .expect("one shared requirement identity resolves once per selected plan");

    assert!(
        selected
            .installation_reach_resolution(&shared_requirement)
            .is_none()
    );
    assert!(
        selected
            .resolve_installation_reach(&[], std::slice::from_ref(&shared_requirement))
            .expect_err("an ambiguous shared requirement cannot resolve unscoped")
            .contains("more than one selected provider plan")
    );
    assert!(
        selected
            .resolve_installation_reach(&[], &["Missing::requirement".into()])
            .expect_err("absence still rejects unscoped")
            .contains("remains unresolved at final admission")
    );

    assert_eq!(
        selected
            .installation_reach_resolution_for_plan(fatal_plan, &shared_requirement)
            .map(|row| row.resolved_row.as_slice()),
        Some(["PortIo".to_owned()].as_slice())
    );
    assert_eq!(
        selected
            .installation_reach_resolution_for_plan(timer_plan, &shared_requirement)
            .map(|row| row.resolved_row.as_slice()),
        Some(["MachineControl".to_owned()].as_slice())
    );
    assert_eq!(
        selected
            .resolve_installation_reach_for_plan(
                &["RootConcrete".into()],
                &[
                    shared_requirement.clone(),
                    acknowledgement_requirement.clone()
                ],
                timer_plan,
            )
            .expect("the timer root binds its own row plus singly-realized requirements"),
        ["MachineControl", "PortIo", "RootConcrete"]
    );
    assert!(
        selected
            .resolve_installation_reach_for_plan(&[], &["MachineControl::halt".into()], timer_plan,)
            .expect_err("a requirement absent from every plan still rejects")
            .contains("remains unresolved at final admission")
    );
    assert!(
        selected
            .resolve_installation_reach_for_plan(
                &[],
                std::slice::from_ref(&shared_requirement),
                acknowledgement_plan,
            )
            .expect_err("a shared requirement realized elsewhere is ambiguous for this root")
            .contains("none under provider plan")
    );

    assert!(
        build()
            .with_installation_reach_resolutions(vec![
                resolutions()[0].clone(),
                resolutions()[0].clone(),
            ])
            .expect_err("the exact requirement/plan pair still cannot repeat")
            .contains("more than one selected resolution under provider plan")
    );
}

#[test]
fn absent_duplicate_and_partial_selections_reject() {
    let complete = candidate("Complete", "run");
    assert!(
        SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(&complete),
            &["Missing".into()]
        )
        .expect_err("missing candidate must reject")
        .contains("absent")
    );
    assert!(
        SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(&complete),
            &["Complete".into(), "Complete".into()]
        )
        .expect_err("duplicate selection must reject")
        .contains("more than once")
    );

    let mut partial = candidate("Partial", "run");
    partial.rows.clear();
    assert!(
        SelectedProviderPlanFacts::from_selection(&[partial], &["Partial".into()])
            .expect_err("partial selected plan must reject")
            .contains("not fully covering")
    );

    let first = candidate("First", "run");
    let mut second = candidate("Second", "run");
    second.schema.trait_name = first.schema.trait_name.clone();
    assert!(
        SelectedProviderPlanFacts::from_selection(
            &[first, second],
            &["First".into(), "Second".into()]
        )
        .expect_err("one boundary slot cannot retain two selected plans")
        .contains("more than one selected provider plan")
    );
}

#[test]
fn selection_rejects_name_only_requirement_rows() {
    let mut incomplete = candidate("Incomplete", "run");
    incomplete.rows[0].requirement_identity.clear();
    assert!(
        SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(&incomplete),
            std::slice::from_ref(&incomplete.name),
        )
        .expect_err("name-only provider rows must not enter the selected closure")
        .contains("no exact requirement identity")
    );

    let mut incomplete = candidate("IncompleteSchema", "run");
    incomplete.schema.methods[0].requirement_identity.clear();
    assert!(
        SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(&incomplete),
            std::slice::from_ref(&incomplete.name),
        )
        .expect_err("name-only provider schema methods must not enter the selected closure")
        .contains("no exact requirement identity")
    );
}

#[test]
fn opaque_selection_survives_a_checked_wrapper_as_attributed_incompleteness() {
    let checked_wrapper = candidate("CheckedWrapper", "read");
    let mut opaque_leaf = candidate("OpaqueLeaf", "read_raw");
    opaque_leaf.schema.trait_name = "RawStorage".into();
    opaque_leaf.target = "windows_x86_64".into();
    let locator = crate::normalize_foreign_locator(
        crate::ForeignLocatorCandidate::PeByName {
            library: b"vendor-storage.dll".to_vec(),
            export: b"read_raw".to_vec(),
        },
        target::TargetProfile::WindowsX64,
    )
    .expect("normalized opaque import");
    opaque_leaf.rows[0].binding = ProviderBinding::Import {
        evaluated: evaluated_import(locator.clone(), 11),
    };
    let selected = SelectedProviderPlanFacts::from_selection(
        &[checked_wrapper.clone(), opaque_leaf.clone()],
        &[checked_wrapper.name.clone(), opaque_leaf.name.clone()],
    )
    .expect("both transitive selections are exact");

    let manifest = selected.executable_tcb_manifest();
    assert_eq!(manifest.known_entries.len(), 1);
    let crate::ScopeCompleteness::Incomplete { causes, .. } = manifest.completeness else {
        panic!("opaque in-process selection must make the scope incomplete");
    };
    assert_eq!(causes.len(), 1);
    assert!(matches!(
        &causes[0],
        crate::IncompleteCause::SelectedOpaqueProvider {
            provider_plan_report_identity,
            binding: crate::OpaqueInProcessBinding::Import {
                evaluated: retained,
            },
            ..
        } if *provider_plan_report_identity == opaque_leaf.report_fingerprint()
            && retained.locator() == &locator
    ));
}

#[test]
fn selected_plan_identity_changes_with_normalized_import_coordinates() {
    fn selected(export: &[u8]) -> SelectedProviderPlanFacts {
        let mut plan = candidate("OpaqueLeaf", "read_raw");
        plan.target = "windows_x86_64".into();
        let locator = crate::normalize_foreign_locator(
            crate::ForeignLocatorCandidate::PeByName {
                library: b"vendor-storage.dll".to_vec(),
                export: export.to_vec(),
            },
            target::TargetProfile::WindowsX64,
        )
        .expect("normalized selected import");
        plan.rows[0].binding = ProviderBinding::Import {
            evaluated: evaluated_import(locator, 21),
        };
        SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(&plan),
            std::slice::from_ref(&plan.name),
        )
        .expect("selected normalized import")
    }

    assert_ne!(
        selected(b"read_raw").report_fingerprint(),
        selected(b"write_raw").report_fingerprint(),
    );
}

#[test]
fn selected_plan_identity_retains_macho_install_name_and_symbol() {
    fn selected(install_name: &[u8], symbol: &[u8]) -> SelectedProviderPlanFacts {
        let mut plan = candidate("OpaqueLeaf", "read_raw");
        plan.target = "macos_arm64".into();
        let locator = crate::normalize_foreign_locator(
            crate::ForeignLocatorCandidate::MachODylibSymbol {
                install_name: install_name.to_vec(),
                symbol: symbol.to_vec(),
            },
            target::TargetProfile::MacosArm64,
        )
        .expect("normalized selected Mach-O import");
        plan.rows[0].binding = ProviderBinding::Import {
            evaluated: evaluated_import(locator, 31),
        };
        SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(&plan),
            std::slice::from_ref(&plan.name),
        )
        .expect("selected normalized Mach-O import")
    }

    let baseline = selected(b"/usr/lib/libSystem.B.dylib", b"_read");
    assert_ne!(
        baseline.identity_digest(),
        selected(b"/usr/lib/libobjc.A.dylib", b"_read").identity_digest(),
    );
    assert_ne!(
        baseline.identity_digest(),
        selected(b"/usr/lib/libSystem.B.dylib", b"_write").identity_digest(),
    );
}

#[test]
fn pinned_opaque_entry_remains_incomplete_without_executable_closure_evidence() {
    let (opaque, opaque_binding) =
        opaque_candidate("Opaque", "read", b"vendor-storage", b"read", 41);
    let plan_identity = opaque.report_fingerprint();
    let selected = SelectedProviderPlanFacts::from_selection(
        std::slice::from_ref(&opaque),
        std::slice::from_ref(&opaque.name),
    )
    .expect("selected opaque provider")
    .with_opaque_executable_admissions([crate::OpaqueExecutableAdmissionCandidate {
        provider_plan_report_identity: plan_identity,
        provider_plan_digest: opaque.identity_digest(),
        method: "read".into(),
        requirement_identity: opaque.schema.methods[0].requirement_identity.clone(),
        binding: opaque_binding.clone(),
        executable_identity: "sha256:0123456789abcdef".into(),
        implementation_evidence_identity: "receipt:vendor-storage-v1".into(),
        execution_scope: crate::ExecutionScope::CallerAddressSpace,
        containment: vec![crate::ContainmentEvidence {
            guarantee: crate::ContainmentGuarantee::FaultContainment,
            evidence_identity: "receipt:fault-boundary-v1".into(),
        }],
        executable_closure_evidence_identity: None,
    }])
    .expect("exact opaque admission");

    let manifest = selected.executable_tcb_manifest();
    assert_eq!(manifest.known_entries.len(), 1);
    assert!(matches!(
        manifest.known_entries[0].executable_identity,
        crate::ExecutableIdentity::PinnedOpaqueArtifact(ref identity)
            if identity == "sha256:0123456789abcdef"
    ));
    assert!(matches!(
        manifest.completeness,
        crate::ScopeCompleteness::Incomplete { ref causes, .. } if causes.len() == 1
    ));
}

#[test]
fn exact_closure_and_containment_receipts_complete_the_opaque_scope() {
    let (opaque, opaque_binding) = opaque_candidate("Opaque", "read", b"platform", b"read", 42);
    let plan_identity = opaque.report_fingerprint();
    let selected = SelectedProviderPlanFacts::from_selection(
        std::slice::from_ref(&opaque),
        std::slice::from_ref(&opaque.name),
    )
    .expect("selected opaque provider")
    .with_opaque_executable_admissions([crate::OpaqueExecutableAdmissionCandidate {
        provider_plan_report_identity: plan_identity,
        provider_plan_digest: opaque.identity_digest(),
        method: "read".into(),
        requirement_identity: opaque.schema.methods[0].requirement_identity.clone(),
        binding: opaque_binding.clone(),
        executable_identity: "platform-baseline:read-v1".into(),
        implementation_evidence_identity: "receipt:platform-read-v1".into(),
        execution_scope: crate::ExecutionScope::CallerAddressSpace,
        containment: vec![
            crate::ContainmentEvidence {
                guarantee: crate::ContainmentGuarantee::BoundedResources,
                evidence_identity: "receipt:quota-v1".into(),
            },
            crate::ContainmentEvidence {
                guarantee: crate::ContainmentGuarantee::MemoryIsolation,
                evidence_identity: "receipt:memory-v1".into(),
            },
        ],
        executable_closure_evidence_identity: Some("receipt:closed-loader-v1".into()),
    }])
    .expect("exact opaque admission");

    let manifest = selected.executable_tcb_manifest();
    assert_eq!(manifest.known_entries[0].containment.len(), 2);
    let crate::ScopeCompleteness::Complete {
        opaque_closure_evidence,
        ..
    } = manifest.completeness
    else {
        panic!("closed executable envelope should complete the scope");
    };
    assert_eq!(opaque_closure_evidence.len(), 1);
    assert_eq!(
        opaque_closure_evidence[0].evidence_identity,
        "receipt:closed-loader-v1"
    );
}

#[test]
fn exact_closure_evidence_survives_an_unrelated_incomplete_row() {
    let (closed, closed_binding) =
        opaque_candidate("Closed", "read", b"closed-platform", b"read", 43);
    let (open, _open_binding) = opaque_candidate("Open", "write", b"open-vendor", b"write", 44);
    let closed_identity = closed.report_fingerprint();
    let selected = SelectedProviderPlanFacts::from_selection(
        &[closed.clone(), open.clone()],
        &[closed.name.clone(), open.name.clone()],
    )
    .expect("two distinct selected slots")
    .with_opaque_executable_admissions([crate::OpaqueExecutableAdmissionCandidate {
        provider_plan_report_identity: closed_identity,
        provider_plan_digest: closed.identity_digest(),
        method: "read".into(),
        requirement_identity: closed.schema.methods[0].requirement_identity.clone(),
        binding: closed_binding.clone(),
        executable_identity: "platform-baseline:closed-read-v1".into(),
        implementation_evidence_identity: "receipt:closed-read-v1".into(),
        execution_scope: crate::ExecutionScope::CallerAddressSpace,
        containment: Vec::new(),
        executable_closure_evidence_identity: Some("receipt:closed-loader-v1".into()),
    }])
    .expect("closed row admission");

    let manifest = selected.executable_tcb_manifest();
    let crate::ScopeCompleteness::Incomplete {
        causes,
        opaque_closure_evidence,
        ..
    } = manifest.completeness
    else {
        panic!("unadmitted opaque row keeps scope incomplete");
    };
    assert_eq!(causes.len(), 1);
    assert!(matches!(
        &causes[0],
        crate::IncompleteCause::SelectedOpaqueProvider {
            provider_plan_report_identity,
            ..
        } if *provider_plan_report_identity == open.report_fingerprint()
    ));
    assert_eq!(opaque_closure_evidence.len(), 1);
    assert_eq!(
        opaque_closure_evidence[0].evidence_identity,
        "receipt:closed-loader-v1"
    );
}

#[test]
fn opaque_admission_rejects_binding_drift_and_duplicate_containment_axes() {
    let (opaque, opaque_binding) = opaque_candidate("Opaque", "read", b"platform", b"read", 45);
    let (_, drifted_binding) = opaque_import(b"other", b"read", 45);
    let plan_identity = opaque.report_fingerprint();
    let selected = SelectedProviderPlanFacts::from_selection(
        std::slice::from_ref(&opaque),
        std::slice::from_ref(&opaque.name),
    )
    .expect("selected opaque provider");
    let candidate = crate::OpaqueExecutableAdmissionCandidate {
        provider_plan_report_identity: plan_identity,
        provider_plan_digest: opaque.identity_digest(),
        method: "read".into(),
        requirement_identity: opaque.schema.methods[0].requirement_identity.clone(),
        binding: drifted_binding,
        executable_identity: "sha256:0123456789abcdef".into(),
        implementation_evidence_identity: "receipt:opaque-v1".into(),
        execution_scope: crate::ExecutionScope::CallerAddressSpace,
        containment: Vec::new(),
        executable_closure_evidence_identity: None,
    };
    let mut compact_equal_wrong_digest = candidate.clone();
    let mut substituted_plan = opaque.clone();
    substituted_plan.name = "compact-equal-substitution".into();
    compact_equal_wrong_digest.provider_plan_digest = substituted_plan.identity_digest();
    assert!(
        selected
            .clone()
            .with_opaque_executable_admissions([compact_equal_wrong_digest])
            .expect_err("compact report identity cannot select a different exact plan")
            .contains("unselected provider plan")
    );
    assert!(
        selected
            .clone()
            .with_opaque_executable_admissions([candidate.clone()])
            .expect_err("binding drift")
            .contains("binding drift")
    );

    let mut candidate = candidate;
    candidate.binding = opaque_binding;
    candidate.containment = vec![
        crate::ContainmentEvidence {
            guarantee: crate::ContainmentGuarantee::FaultContainment,
            evidence_identity: "receipt:fault-a".into(),
        },
        crate::ContainmentEvidence {
            guarantee: crate::ContainmentGuarantee::FaultContainment,
            evidence_identity: "receipt:fault-b".into(),
        },
    ];
    assert!(
        selected
            .with_opaque_executable_admissions([candidate])
            .expect_err("duplicate containment axis")
            .contains("repeats one containment guarantee")
    );
}

#[test]
fn checked_and_intrinsic_entries_are_derived_only_from_selected_plans() {
    let checked = candidate("Checked", "run");
    let mut intrinsic = candidate("Intrinsic", "halt");
    intrinsic.schema.trait_name = "MachineControl".into();
    intrinsic.rows[0].binding = ProviderBinding::CompilerIntrinsic {
        machine: "MachineControl::halt".into(),
    };
    let unselected = candidate("Unselected", "skip");
    let selected = SelectedProviderPlanFacts::from_selection(
        &[checked.clone(), intrinsic.clone(), unselected],
        &[intrinsic.name.clone(), checked.name.clone()],
    )
    .expect("selected closure");

    let manifest = selected.executable_tcb_manifest();
    assert_eq!(manifest.known_entries.len(), 2);
    assert!(matches!(
        manifest.completeness,
        crate::ScopeCompleteness::Complete {
            selected_provider_closure_report_identity,
            ..
        } if selected_provider_closure_report_identity == selected.report_fingerprint()
    ));
    assert!(manifest.known_entries.iter().all(|entry| {
        entry.origin == crate::ExecutableEntryOrigin::StaticSelection
            && entry.execution_scope == crate::ExecutionScope::CallerAddressSpace
            && entry.selected_requirement.is_some()
    }));
}

#[test]
fn executable_manifest_keeps_same_named_overload_rows_distinct() {
    let mut overloaded = candidate("Convert", "convert");
    let first_identity = "named-callable:path=ConvertService::convert;result=Ordinary";
    let second_identity = "named-callable:path=ConvertService::convert;result=Saturating";
    let mut second_method = overloaded.schema.methods[0].clone();
    overloaded.schema.methods[0].requirement_identity = first_identity.into();
    second_method.requirement_identity = second_identity.into();
    overloaded.schema.methods.push(second_method);
    overloaded.rows[0].requirement_identity = first_identity.into();
    overloaded.rows.push(ProviderPlanRow {
        method: "convert".into(),
        requirement_identity: second_identity.into(),
        requirement_lifetime_partition: Vec::new(),
        binding: ProviderBinding::CheckedAdapter {
            machine_identity: "ConvertProvider::convert".into(),
            machine_package_identity: None,
        },
    });

    let selected = SelectedProviderPlanFacts::from_selection(&[overloaded], &["Convert".into()])
        .expect("same-named exact overload rows cover distinct requirements");
    let manifest = selected.executable_tcb_manifest();
    assert_eq!(
        manifest.known_entries.len(),
        2,
        "one shared executable must not collapse distinct selected requirement rows"
    );
    let mut identities = manifest
        .known_entries
        .iter()
        .map(|entry| {
            let requirement = entry
                .selected_requirement
                .as_ref()
                .expect("static selected row identity");
            assert_eq!(requirement.method, "convert");
            requirement.requirement_identity.as_str()
        })
        .collect::<Vec<_>>();
    identities.sort_unstable();
    assert_eq!(identities, [first_identity, second_identity]);
}
