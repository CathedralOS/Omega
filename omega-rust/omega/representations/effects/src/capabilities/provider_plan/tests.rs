//! Provider plan tests.

use super::{
    BoundaryCallingPlanCommitment, EvaluatedBindingEvaluationDigest,
    EvaluatedBindingMaterializationDigest, EvaluatedBindingProducerClosureDigest,
    EvaluatedBindingReceipt, EvaluatedBindingUsage, EvaluatedForeignImport, ProviderBinding,
    ProviderPlan, ProviderPlanRow, ServiceEntryAuthorityFlow, ServiceEntryClaim, ServiceMethod,
    ServiceProgressEstablishmentRoute, ServiceProgressEstablishmentRouteKind,
    ServiceProgressPremise, ServiceProgressSubject, ServiceResultClaim, ServiceSchema,
};
use crate::capabilities::foreign_locator::NormalizedForeignLocator;

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

fn normalized_windows_import(library: &[u8], export: &[u8]) -> ProviderBinding {
    let locator = crate::normalize_foreign_locator(
        crate::ForeignLocatorCandidate::PeByName {
            library: library.to_vec(),
            export: export.to_vec(),
        },
        target::TargetProfile::WindowsX64,
    )
    .expect("valid normalized Windows import");
    ProviderBinding::Import {
        evaluated: evaluated_import(locator, 11),
    }
}

fn normalized_macos_import(install_name: &[u8], symbol: &[u8]) -> ProviderBinding {
    let locator = crate::normalize_foreign_locator(
        crate::ForeignLocatorCandidate::MachODylibSymbol {
            install_name: install_name.to_vec(),
            symbol: symbol.to_vec(),
        },
        target::TargetProfile::MacosArm64,
    )
    .expect("valid normalized Mach-O import");
    ProviderBinding::Import {
        evaluated: evaluated_import(locator, 21),
    }
}

/// The built-in Console lowering, spelled as a ProviderPlan value --
/// the PRV4 relocation target (windows.rs insert_platform_lowering's
/// rows as data). Construction is free; nothing consumes this yet.
fn windows_console_plan() -> ProviderPlan {
    let schema = ServiceSchema {
        trait_name: "Console".to_owned(),
        trait_package_identity: None,
        methods: vec![
            ServiceMethod {
                name: "write_line".to_owned(),
                requirement_owner: "Console".to_owned(),
                requirement_owner_package_identity: None,
                requirement_identity: "Console::write_line".to_owned(),
                parameter_count: 1,
                parameter_type_identities: vec!["String".to_owned()],
                entry_claims: Vec::new(),
                has_result: false,
                result_type_identity: None,
                result_claims: Vec::new(),
                service_reach: vec!["Console".to_owned()],
                synchronous_invocations: Vec::new(),
                may_suspend: false,
                may_block: false,
                terminates_guarantee: false,
                termination_premises: Vec::new(),
                calling_plan_report_fingerprint: None,
                calling_plan_commitment: None,
            },
            ServiceMethod {
                name: "read_byte".to_owned(),
                requirement_owner: "Console".to_owned(),
                requirement_owner_package_identity: None,
                requirement_identity: "Console::read_byte".to_owned(),
                parameter_count: 0,
                parameter_type_identities: Vec::new(),
                entry_claims: Vec::new(),
                has_result: true,
                result_type_identity: Some("u8".to_owned()),
                result_claims: Vec::new(),
                service_reach: vec!["Console".to_owned()],
                synchronous_invocations: Vec::new(),
                may_suspend: true,
                may_block: false,
                terminates_guarantee: false,
                termination_premises: Vec::new(),
                calling_plan_report_fingerprint: None,
                calling_plan_commitment: None,
            },
            ServiceMethod {
                name: "exit_process".to_owned(),
                requirement_owner: "Console".to_owned(),
                requirement_owner_package_identity: None,
                requirement_identity: "Console::exit_process".to_owned(),
                parameter_count: 1,
                parameter_type_identities: vec!["i32".to_owned()],
                entry_claims: Vec::new(),
                has_result: false,
                result_type_identity: None,
                result_claims: Vec::new(),
                service_reach: vec!["Console".to_owned()],
                synchronous_invocations: Vec::new(),
                may_suspend: false,
                may_block: true,
                terminates_guarantee: false,
                termination_premises: Vec::new(),
                calling_plan_report_fingerprint: None,
                calling_plan_commitment: None,
            },
        ],
    };
    ProviderPlan {
        name: "omega::host::standard::console".to_owned(),
        provider_type: "StandardConsole".to_owned(),
        provider_type_package_identity: None,
        target: "windows_x86_64".to_owned(),
        schema,
        rows: vec![
            ProviderPlanRow {
                method: "write_line".to_owned(),
                requirement_identity: "Console::write_line".to_owned(),
                requirement_lifetime_partition: Vec::new(),
                binding: normalized_windows_import(b"kernel32.dll", b"WriteFile"),
            },
            ProviderPlanRow {
                method: "read_byte".to_owned(),
                requirement_identity: "Console::read_byte".to_owned(),
                requirement_lifetime_partition: Vec::new(),
                binding: normalized_windows_import(b"kernel32.dll", b"ReadFile"),
            },
            ProviderPlanRow {
                method: "exit_process".to_owned(),
                requirement_identity: "Console::exit_process".to_owned(),
                requirement_lifetime_partition: Vec::new(),
                binding: normalized_windows_import(b"kernel32.dll", b"ExitProcess"),
            },
        ],
        origin_package_identity: None,
        origin_package: "omega::language::std".to_owned(),
    }
}

#[test]
fn evaluated_calling_plan_is_published_provider_identity() {
    let mut first = windows_console_plan();
    let baseline = first.report_fingerprint();
    first.schema.methods[0].calling_plan_report_fingerprint = Some(0x1234);
    first.schema.methods[0].calling_plan_commitment =
        Some(typed_trees::typed_trees::BoundaryCallingPlanCommitment::from_digest([1; 32]));
    assert_ne!(baseline, first.report_fingerprint());

    let mut refactored = first.clone();
    refactored.schema.methods[0].calling_plan_report_fingerprint = Some(0x1234);
    assert_eq!(first.report_fingerprint(), refactored.report_fingerprint());

    refactored.schema.methods[0].calling_plan_commitment =
        Some(typed_trees::typed_trees::BoundaryCallingPlanCommitment::from_digest([2; 32]));
    assert_eq!(first.report_fingerprint(), refactored.report_fingerprint());
    assert_ne!(first.identity_digest(), refactored.identity_digest());
}

#[test]
fn service_schema_digest_excludes_realization_rows_and_binds_schema_drift() {
    let original = windows_console_plan();
    let schema_identity = original.schema.identity_digest();

    let mut different_realization = original.clone();
    different_realization.rows[0].method = "different-row".to_owned();
    assert_eq!(
        different_realization.schema.identity_digest(),
        schema_identity,
        "provider realization rows are outside service-schema identity",
    );
    assert_ne!(
        different_realization.identity_digest(),
        original.identity_digest(),
    );

    let mut changed_schema = original;
    changed_schema.schema.methods[0].may_block = true;
    assert_ne!(changed_schema.schema.identity_digest(), schema_identity);
}

#[test]
fn provider_candidate_rejects_an_empty_calling_plan_commitment() {
    let mut plan = windows_console_plan();
    plan.schema.methods[0].calling_plan_report_fingerprint = Some(0x1234);
    plan.schema.methods[0].calling_plan_commitment =
        Some(BoundaryCallingPlanCommitment::from_digest([0; 32]));

    let diagnostics = plan.validate_candidate_against_schema();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.contains("empty calling-plan commitment")),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

#[test]
fn strong_provider_plan_identity_rejects_compact_equal_structural_substitution() {
    let original = windows_console_plan();
    let mut substituted = original.clone();
    substituted.schema.methods[0].requirement_owner = "OtherConsole".to_owned();

    assert_eq!(
        original.report_fingerprint(),
        substituted.report_fingerprint(),
        "the legacy compact renderer did not retain the readable requirement-owner field"
    );
    assert_ne!(original, substituted);
    assert_ne!(original.identity_digest(), substituted.identity_digest());
}

#[test]
fn exact_package_provenance_enters_provider_identity_but_legacy_label_does_not() {
    let mut first = windows_console_plan();
    first.origin_package_identity = semantic_vocabulary::PackageKeyIdentity::from_digest([1; 32]);
    let first_identity = first.report_fingerprint();

    let mut renamed_label = first.clone();
    renamed_label.origin_package = "misleading display label".to_owned();
    assert_eq!(renamed_label.report_fingerprint(), first_identity);

    let mut second = first;
    second.origin_package_identity = semantic_vocabulary::PackageKeyIdentity::from_digest([2; 32]);
    assert_ne!(second.report_fingerprint(), first_identity);

    let mut provider_type_owner = renamed_label.clone();
    provider_type_owner.provider_type_package_identity =
        semantic_vocabulary::PackageKeyIdentity::from_digest([3; 32]);
    assert_ne!(provider_type_owner.report_fingerprint(), first_identity);

    let mut schema_owner = renamed_label.clone();
    schema_owner.schema.trait_package_identity =
        semantic_vocabulary::PackageKeyIdentity::from_digest([4; 32]);
    assert_ne!(schema_owner.report_fingerprint(), first_identity);

    let mut requirement_owner = renamed_label;
    requirement_owner.schema.methods[0].requirement_owner_package_identity =
        semantic_vocabulary::PackageKeyIdentity::from_digest([5; 32]);
    assert_ne!(requirement_owner.report_fingerprint(), first_identity);

    let mut unbound_adapter = windows_console_plan();
    unbound_adapter.rows[0].binding = ProviderBinding::CheckedAdapter {
        machine_identity: "named-callable(path(ConsoleProvider::write))".to_owned(),
        machine_package_identity: None,
    };
    let unbound_identity = unbound_adapter.report_fingerprint();
    let mut bound_adapter = unbound_adapter.clone();
    let adapter_package = semantic_vocabulary::PackageKeyIdentity::from_digest([6; 32])
        .expect("nonzero package identity");
    bound_adapter.origin_package_identity = Some(adapter_package);
    bound_adapter.rows[0].binding = ProviderBinding::CheckedAdapter {
        machine_identity: "named-callable(path(ConsoleProvider::write))".to_owned(),
        machine_package_identity: Some(adapter_package),
    };
    assert_ne!(bound_adapter.report_fingerprint(), unbound_identity);

    let mut other_overload = unbound_adapter;
    other_overload.rows[0].binding = ProviderBinding::CheckedAdapter {
        machine_identity: "named-callable(path(ConsoleProvider::write))#other".to_owned(),
        machine_package_identity: None,
    };
    assert_ne!(other_overload.report_fingerprint(), unbound_identity);
}

#[test]
fn independent_operational_ceilings_enter_provider_identity() {
    let baseline = windows_console_plan();
    let baseline_identity = baseline.report_fingerprint();

    let mut suspending = baseline.clone();
    suspending.schema.methods[0].may_suspend = true;
    assert_ne!(suspending.report_fingerprint(), baseline_identity);

    let mut blocking = baseline;
    blocking.schema.methods[0].may_block = true;
    assert_ne!(blocking.report_fingerprint(), baseline_identity);

    let mut terminating = windows_console_plan();
    terminating.schema.methods[0].terminates_guarantee = true;
    assert_ne!(terminating.report_fingerprint(), baseline_identity);

    let mut premised = terminating.clone();
    premised.schema.methods[0].termination_premises = vec![ServiceProgressPremise {
        profile: "SchedulerHandle::WeakFair".to_owned(),
        subject: ServiceProgressSubject::Parameter(0),
        subject_projections: Vec::new(),
        establishment_routes: vec![ServiceProgressEstablishmentRoute {
            kind: ServiceProgressEstablishmentRouteKind::BoundaryRequirement,
            requirement_identity: "SchedulerAdmission::grant#exact".to_owned(),
        }],
    }];
    assert_ne!(
        premised.report_fingerprint(),
        terminating.report_fingerprint()
    );
    let mut changed_route = premised.clone();
    changed_route.schema.methods[0].termination_premises[0].establishment_routes[0]
        .requirement_identity = "SchedulerAdmission::grant_strong#exact".to_owned();
    assert_ne!(
        premised.report_fingerprint(),
        changed_route.report_fingerprint(),
        "the authorized establishment route must enter provider identity"
    );
    let mut two_routes = premised.clone();
    two_routes.schema.methods[0].termination_premises[0]
        .establishment_routes
        .push(ServiceProgressEstablishmentRoute {
            kind: ServiceProgressEstablishmentRouteKind::BoundaryRequirement,
            requirement_identity: "SchedulerAdmission::accept_weak#exact".to_owned(),
        });
    let mut reversed_routes = two_routes.clone();
    reversed_routes.schema.methods[0].termination_premises[0]
        .establishment_routes
        .reverse();
    assert_eq!(
        two_routes.report_fingerprint(),
        reversed_routes.report_fingerprint(),
        "route declaration order is presentation, not provider identity"
    );
    assert_ne!(
        suspending.report_fingerprint(),
        blocking.report_fingerprint()
    );
    assert_ne!(
        terminating.report_fingerprint(),
        suspending.report_fingerprint()
    );
    assert_ne!(
        terminating.report_fingerprint(),
        blocking.report_fingerprint()
    );
}

#[test]
fn progress_schema_rejects_missing_repeated_and_non_boundary_routes() {
    let mut plan = windows_console_plan();
    plan.schema.methods[0].terminates_guarantee = true;
    plan.schema.methods[0].termination_premises = vec![ServiceProgressPremise {
        profile: "SchedulerHandle::WeakFair".to_owned(),
        subject: ServiceProgressSubject::Parameter(0),
        subject_projections: Vec::new(),
        establishment_routes: Vec::new(),
    }];
    assert!(
        plan.validate_against_schema()
            .iter()
            .any(|error| error.contains("has no authorized establishment route"))
    );

    let route = ServiceProgressEstablishmentRoute {
        kind: ServiceProgressEstablishmentRouteKind::BoundaryRequirement,
        requirement_identity: "SchedulerAdmission::grant#exact".to_owned(),
    };
    plan.schema.methods[0].termination_premises[0].establishment_routes =
        vec![route.clone(), route];
    assert!(
        plan.validate_against_schema()
            .iter()
            .any(|error| error.contains("repeats an establishment route"))
    );

    plan.schema.methods[0].termination_premises[0].establishment_routes =
        vec![ServiceProgressEstablishmentRoute {
            kind: ServiceProgressEstablishmentRouteKind::CheckedRequirement,
            requirement_identity: "SchedulerAdmission::grant#exact".to_owned(),
        }];
    assert!(
        plan.validate_against_schema()
            .iter()
            .any(|error| error.contains("non-boundary establishment route"))
    );
}

#[test]
fn normalized_parameter_and_result_types_enter_provider_identity() {
    let baseline = windows_console_plan();
    let baseline_identity = baseline.report_fingerprint();

    let mut qualified_parameter = baseline.clone();
    qualified_parameter.schema.methods[0].parameter_type_identities[0] =
        "InterruptAcknowledgement in InterruptAcknowledgement::Pending".to_owned();
    assert_ne!(qualified_parameter.report_fingerprint(), baseline_identity);

    let mut changed_result = baseline;
    changed_result.schema.methods[1].result_type_identity = Some("u16".to_owned());
    assert_ne!(changed_result.report_fingerprint(), baseline_identity);
    assert_ne!(
        qualified_parameter.report_fingerprint(),
        changed_result.report_fingerprint()
    );
}

#[test]
fn structured_entry_claims_enter_provider_identity() {
    let baseline = windows_console_plan();
    let mut accepted = baseline.clone();
    accepted.schema.methods[0].entry_claims = vec![ServiceEntryClaim {
        parameter_index: 0,
        carrier_identity: "named(name(InterruptAcknowledgement))".to_owned(),
        domain: "InterruptAcknowledgement::Pending".to_owned(),
        predicate_body: language_semantics::DomainPredicateBody::Bodyless,
        effective_carry: language_semantics::CarryPolicy::STRICT,
        authority_flow: ServiceEntryAuthorityFlow::Accepts,
    }];

    assert_ne!(
        accepted.report_fingerprint(),
        baseline.report_fingerprint(),
        "the receipt identity must bind structured accepted authority, not only display types"
    );

    let mut redirected_carrier = accepted.clone();
    redirected_carrier.schema.methods[0].entry_claims[0].carrier_identity =
        "named(name(OtherAcknowledgement))".to_owned();
    assert_ne!(
        accepted.report_fingerprint(),
        redirected_carrier.report_fingerprint(),
        "the routed qualification's exact carrier is provider-plan identity"
    );

    let mut relaxed = accepted.clone();
    relaxed.schema.methods[0].entry_claims[0].effective_carry =
        language_semantics::CarryPolicy::PERMISSIVE;
    assert_ne!(
        accepted.report_fingerprint(),
        relaxed.report_fingerprint(),
        "the compiler-owned entry carry policy is receipt identity"
    );

    let mut predicate_bearing = accepted.clone();
    predicate_bearing.schema.methods[0].entry_claims[0].predicate_body =
        language_semantics::DomainPredicateBody::Present;
    assert_ne!(
        accepted.report_fingerprint(),
        predicate_bearing.report_fingerprint(),
        "predicate discharge is part of the selected provider contract"
    );
}

#[test]
fn console_plan_constructs_and_covers_its_schema() {
    let plan = windows_console_plan();
    assert!(plan.covers_schema());
}

#[test]
fn validation_names_every_structural_defect() {
    // PRV2: missing binding, stray row, and duplicate binding each produce
    // a NAMED error.
    let mut plan = windows_console_plan();
    plan.rows.remove(0);
    plan.rows.push(ProviderPlanRow {
        method: "not_a_method".to_owned(),
        requirement_identity: "Console::not_a_method".to_owned(),
        requirement_lifetime_partition: Vec::new(),
        binding: ProviderBinding::Syscall { number: 1 },
    });
    plan.rows.push(ProviderPlanRow {
        method: "exit_process".to_owned(),
        requirement_identity: "Console::exit_process".to_owned(),
        requirement_lifetime_partition: Vec::new(),
        binding: ProviderBinding::Syscall { number: 0 },
    });
    let errors = plan.validate_against_schema();
    assert!(
        errors
            .iter()
            .any(|error| error.contains("does not bind `Console::write_line`")),
        "missing binding named: {errors:?}"
    );
    assert!(
        errors
            .iter()
            .any(|error| error.contains("not a `Console` method")),
        "stray row named: {errors:?}"
    );
    assert!(
        errors
            .iter()
            .any(|error| error.contains("binds `Console::exit_process` 2 times")),
        "duplicate named: {errors:?}"
    );
    assert!(windows_console_plan().validate_against_schema().is_empty());
}

#[test]
fn coverage_detects_missing_and_stray_rows() {
    let mut plan = windows_console_plan();
    plan.rows.pop();
    assert!(
        plan.validate_candidate_against_schema().is_empty(),
        "a partial candidate is structurally valid before slot selection"
    );
    assert!(
        !plan.covers_schema(),
        "a missing method row must fail coverage"
    );

    let mut plan = windows_console_plan();
    plan.rows.push(ProviderPlanRow {
        method: "not_in_schema".to_owned(),
        requirement_identity: "Console::not_in_schema".to_owned(),
        requirement_lifetime_partition: Vec::new(),
        binding: ProviderBinding::VtableSlot { index: 0 },
    });
    assert!(
        !plan.validate_candidate_against_schema().is_empty(),
        "a stray row is invalid even before coverage selection"
    );
    assert!(!plan.covers_schema(), "a stray row must fail coverage");
}

#[test]
fn result_overloaded_requirements_bind_by_exact_identity() {
    let mut plan = windows_console_plan();
    let template = plan.schema.methods[0].clone();
    plan.schema.trait_name = "Convert".to_owned();
    plan.schema.methods = vec![
        ServiceMethod {
            name: "convert".to_owned(),
            requirement_owner: "Convert".to_owned(),
            requirement_identity: "Convert::convert(i32)->i32".to_owned(),
            ..template.clone()
        },
        ServiceMethod {
            name: "convert".to_owned(),
            requirement_owner: "Convert".to_owned(),
            requirement_identity: "Convert::convert(i32)->i32 in Saturating".to_owned(),
            ..template
        },
    ];
    plan.rows = vec![
        ProviderPlanRow {
            method: "convert".to_owned(),
            requirement_identity: plan.schema.methods[0].requirement_identity.clone(),
            requirement_lifetime_partition: Vec::new(),
            binding: ProviderBinding::Syscall { number: 1 },
        },
        ProviderPlanRow {
            method: "convert".to_owned(),
            requirement_identity: plan.schema.methods[1].requirement_identity.clone(),
            requirement_lifetime_partition: Vec::new(),
            binding: ProviderBinding::Syscall { number: 2 },
        },
    ];

    assert!(plan.covers_schema());
    assert!(plan.validate_against_schema().is_empty());

    plan.rows[1].requirement_identity = plan.rows[0].requirement_identity.clone();
    assert!(!plan.covers_schema());
    assert!(
        plan.validate_against_schema()
            .iter()
            .any(|error| error.contains("binds `Convert::convert` 2 times")),
        "duplicating one overload identity must not cover the other: {:?}",
        plan.validate_against_schema()
    );

    plan.rows[1].requirement_identity.clear();
    assert!(
        !plan.covers_schema(),
        "a human name cannot select an overload"
    );
}

#[test]
fn schema_validation_requires_explicit_owner_without_using_it_for_selection() {
    let mut plan = windows_console_plan();
    plan.schema.trait_name = "DerivedConsole".to_owned();

    assert!(
        plan.validate_candidate_against_schema().is_empty(),
        "an inherited requirement owner may differ from its selected schema"
    );
    assert!(plan.covers_schema());

    plan.schema.methods[0].requirement_owner.clear();
    let errors = plan.validate_candidate_against_schema();
    assert!(errors.iter().any(|error| {
        error.contains("schema method `DerivedConsole::write_line` has no exact requirement owner")
    }));
    assert!(
        plan.covers_schema(),
        "readable owner metadata must not replace canonical overload selection"
    );

    plan.schema.methods[0].requirement_owner = "Console".to_owned();
    plan.schema.methods[0].requirement_identity.clear();
    assert!(!plan.covers_schema());
    let errors = plan.validate_candidate_against_schema();
    assert!(errors.iter().any(|error| {
        error.contains(
            "schema method `DerivedConsole::write_line` has no exact requirement identity",
        )
    }));
}

#[test]
fn schema_validation_requires_unique_exact_requirement_identities() {
    let mut valid = windows_console_plan();
    valid.schema.methods[0].name = "operation".to_owned();
    valid.rows[0].method = "operation".to_owned();
    valid.schema.methods[0].requirement_owner = "BaseConsole".to_owned();
    valid.schema.methods[1].name = "operation".to_owned();
    valid.rows[1].method = "operation".to_owned();
    valid.schema.methods[1].requirement_owner = "DerivedConsole".to_owned();
    assert!(
        valid.validate_candidate_against_schema().is_empty(),
        "duplicate readable names and inherited differing owners remain valid when exact overload identities differ"
    );

    let mut same_label = windows_console_plan();
    same_label
        .schema
        .methods
        .push(same_label.schema.methods[0].clone());
    assert!(
        same_label.covers_schema(),
        "one row previously appeared to cover two identical schema methods"
    );
    assert!(
        same_label
            .validate_candidate_against_schema()
            .iter()
            .any(|error| error.contains("repeat exact requirement identity `Console::write_line`"))
    );

    let mut different_label = windows_console_plan();
    let mut duplicate_identity = different_label.schema.methods[0].clone();
    duplicate_identity.name = "renamed_operation".to_owned();
    duplicate_identity.requirement_owner = "OtherConsole".to_owned();
    different_label.schema.methods.push(duplicate_identity);
    assert!(
        different_label
            .validate_candidate_against_schema()
            .iter()
            .any(|error| error.contains("repeat exact requirement identity `Console::write_line`"))
    );
}

#[test]
fn schema_validation_rejects_malformed_qualification_subjects_independently() {
    let valid_claim = ServiceEntryClaim {
        parameter_index: 0,
        carrier_identity: "named(name(Token))".to_owned(),
        domain: "Token::Granted".to_owned(),
        predicate_body: language_semantics::DomainPredicateBody::Bodyless,
        effective_carry: language_semantics::CarryPolicy::STRICT,
        authority_flow: ServiceEntryAuthorityFlow::Accepts,
    };
    let mut valid = windows_console_plan();
    valid.schema.methods[0].entry_claims = vec![valid_claim.clone()];
    assert!(valid.validate_candidate_against_schema().is_empty());

    let mut missing_parameter_type = valid.clone();
    missing_parameter_type.schema.methods[0]
        .parameter_type_identities
        .clear();
    assert!(
        missing_parameter_type
            .validate_candidate_against_schema()
            .iter()
            .any(|error| error.contains("declares 1 parameters but retains 0 exact parameter"))
    );

    let mut out_of_range = valid.clone();
    out_of_range.schema.methods[0].entry_claims[0].parameter_index = 1;
    assert!(
        out_of_range
            .validate_candidate_against_schema()
            .iter()
            .any(|error| error.contains("out-of-range parameter 1 of 1"))
    );

    let mut empty_entry_domain = valid.clone();
    empty_entry_domain.schema.methods[0].entry_claims[0]
        .domain
        .clear();
    assert!(
        empty_entry_domain
            .validate_candidate_against_schema()
            .iter()
            .any(|error| error
                .contains("entry claim for parameter 0 has no exact semantic domain"))
    );

    let mut empty_entry_carrier = valid.clone();
    empty_entry_carrier.schema.methods[0].entry_claims[0]
        .carrier_identity
        .clear();
    assert!(
        empty_entry_carrier
            .validate_candidate_against_schema()
            .iter()
            .any(|error| error.contains("has no exact carrier identity"))
    );

    let mut result_presence_mismatch = valid.clone();
    result_presence_mismatch.schema.methods[0].has_result = true;
    assert!(
        result_presence_mismatch
            .validate_candidate_against_schema()
            .iter()
            .any(|error| error.contains("result presence disagrees"))
    );

    let mut claim_without_result = valid.clone();
    claim_without_result.schema.methods[0].result_claims = vec![ServiceResultClaim {
        domain: "Token::Issued".to_owned(),
        effective_carry: language_semantics::CarryPolicy::STRICT,
    }];
    assert!(
        claim_without_result
            .validate_candidate_against_schema()
            .iter()
            .any(|error| error.contains("result claim without a real result"))
    );

    let mut empty_result_domain = valid;
    empty_result_domain.schema.methods[1].result_claims = vec![ServiceResultClaim {
        domain: String::new(),
        effective_carry: language_semantics::CarryPolicy::STRICT,
    }];
    assert!(
        empty_result_domain
            .validate_candidate_against_schema()
            .iter()
            .any(|error| error.contains("result claim has no exact semantic domain"))
    );
}

#[test]
fn schema_validation_requires_nonempty_semantic_type_identities() {
    let mut valid = windows_console_plan();
    valid.schema.methods[0].entry_claims = vec![ServiceEntryClaim {
        parameter_index: 0,
        carrier_identity: "named(name(Token))".to_owned(),
        domain: "Token::Granted".to_owned(),
        predicate_body: language_semantics::DomainPredicateBody::Bodyless,
        effective_carry: language_semantics::CarryPolicy::STRICT,
        authority_flow: ServiceEntryAuthorityFlow::Accepts,
    }];
    valid.schema.methods[1].result_claims = vec![ServiceResultClaim {
        domain: "Token::Issued".to_owned(),
        effective_carry: language_semantics::CarryPolicy::STRICT,
    }];
    assert!(
        valid.validate_candidate_against_schema().is_empty(),
        "exact type identities and independently retained domains are orthogonal"
    );

    let mut blank_parameter = valid.clone();
    blank_parameter.schema.methods[0].parameter_type_identities[0].clear();
    assert!(
        blank_parameter
            .validate_candidate_against_schema()
            .iter()
            .any(|error| error.contains("parameter 0 has no exact semantic type identity"))
    );

    let mut blank_result = valid;
    blank_result.schema.methods[1]
        .result_type_identity
        .as_mut()
        .expect("read_byte has a real result")
        .clear();
    assert!(
        blank_result
            .validate_candidate_against_schema()
            .iter()
            .any(|error| error.contains("result has no exact semantic type identity"))
    );
}

#[test]
fn schema_validation_requires_canonical_born_strict_claims() {
    fn entry_claim(
        parameter_index: usize,
        domain: &str,
        predicate_body: language_semantics::DomainPredicateBody,
    ) -> ServiceEntryClaim {
        ServiceEntryClaim {
            parameter_index,
            carrier_identity: "named(name(Token))".to_owned(),
            domain: domain.to_owned(),
            predicate_body,
            effective_carry: language_semantics::CarryPolicy::STRICT,
            authority_flow: ServiceEntryAuthorityFlow::Accepts,
        }
    }

    let mut valid = windows_console_plan();
    valid.schema.methods[0].parameter_count = 2;
    valid.schema.methods[0].parameter_type_identities =
        vec!["FirstToken".to_owned(), "SecondToken".to_owned()];
    valid.schema.methods[0].entry_claims = vec![
        entry_claim(
            0,
            "Domain::Alpha",
            language_semantics::DomainPredicateBody::Bodyless,
        ),
        entry_claim(
            0,
            "Domain::Beta",
            language_semantics::DomainPredicateBody::Present,
        ),
        entry_claim(
            1,
            "Domain::Alpha",
            language_semantics::DomainPredicateBody::Bodyless,
        ),
    ];
    valid.schema.methods[1].result_claims = vec![
        ServiceResultClaim {
            domain: "Domain::Alpha".to_owned(),
            effective_carry: language_semantics::CarryPolicy::STRICT,
        },
        ServiceResultClaim {
            domain: "Domain::Beta".to_owned(),
            effective_carry: language_semantics::CarryPolicy::STRICT,
        },
    ];
    assert!(
        valid.validate_candidate_against_schema().is_empty(),
        "canonical claims allow multiple domains per parameter and the same domain at different positions"
    );
    assert!(
        windows_console_plan()
            .validate_candidate_against_schema()
            .is_empty(),
        "empty claim vectors remain valid"
    );

    let mut permissive_entry = valid.clone();
    permissive_entry.schema.methods[0].entry_claims[0].effective_carry =
        language_semantics::CarryPolicy::PERMISSIVE;
    assert!(
        permissive_entry
            .validate_candidate_against_schema()
            .iter()
            .any(|error| error.contains("entry claim") && error.contains("not born-strict"))
    );

    let mut permissive_result = valid.clone();
    permissive_result.schema.methods[1].result_claims[0].effective_carry =
        language_semantics::CarryPolicy::PERMISSIVE;
    assert!(
        permissive_result
            .validate_candidate_against_schema()
            .iter()
            .any(|error| error.contains("result claim") && error.contains("not born-strict"))
    );

    let mut duplicate_entry = valid.clone();
    duplicate_entry.schema.methods[0].entry_claims[1] =
        duplicate_entry.schema.methods[0].entry_claims[0].clone();
    assert!(
        duplicate_entry
            .validate_candidate_against_schema()
            .iter()
            .any(|error| error.contains("entry claims are not strictly increasing"))
    );

    let mut out_of_order_entry = valid.clone();
    out_of_order_entry.schema.methods[0].entry_claims.swap(0, 1);
    assert!(
        out_of_order_entry
            .validate_candidate_against_schema()
            .iter()
            .any(|error| error.contains("entry claims are not strictly increasing"))
    );

    let mut duplicate_result = valid.clone();
    duplicate_result.schema.methods[1].result_claims[1] =
        duplicate_result.schema.methods[1].result_claims[0].clone();
    assert!(
        duplicate_result
            .validate_candidate_against_schema()
            .iter()
            .any(|error| error.contains("result claims are not strictly increasing"))
    );

    let mut out_of_order_result = valid;
    out_of_order_result.schema.methods[1]
        .result_claims
        .swap(0, 1);
    assert!(
        out_of_order_result
            .validate_candidate_against_schema()
            .iter()
            .any(|error| error.contains("result claims are not strictly increasing"))
    );
}

#[test]
fn schema_validation_requires_canonical_independent_service_axes() {
    let mut valid = windows_console_plan();
    valid.schema.methods[0].service_reach = vec!["Console".to_owned(), "Storage".to_owned()];
    valid.schema.methods[0].synchronous_invocations =
        vec!["Clock".to_owned(), "TaskRuntime".to_owned()];
    valid.schema.methods[2].service_reach.clear();
    assert!(
        valid.validate_candidate_against_schema().is_empty(),
        "reach and direct invocation are distinct canonical sets, and either may be empty"
    );

    let mut empty_reach = valid.clone();
    empty_reach.schema.methods[0].service_reach[0].clear();
    assert!(
        empty_reach
            .validate_candidate_against_schema()
            .iter()
            .any(|error| error.contains("service-reach identity at index 0 is empty"))
    );

    let mut duplicate_reach = valid.clone();
    duplicate_reach.schema.methods[0].service_reach =
        vec!["Console".to_owned(), "Console".to_owned()];
    assert!(
        duplicate_reach
            .validate_candidate_against_schema()
            .iter()
            .any(|error| error.contains("service-reach identities are not strictly increasing"))
    );

    let mut out_of_order_reach = valid.clone();
    out_of_order_reach.schema.methods[0].service_reach =
        vec!["Storage".to_owned(), "Console".to_owned()];
    assert!(
        out_of_order_reach
            .validate_candidate_against_schema()
            .iter()
            .any(|error| error.contains("service-reach identities are not strictly increasing"))
    );

    let mut empty_invocation = valid.clone();
    empty_invocation.schema.methods[0].synchronous_invocations[0].clear();
    assert!(
        empty_invocation
            .validate_candidate_against_schema()
            .iter()
            .any(|error| error.contains("synchronous-invocation identity at index 0 is empty"))
    );

    let mut duplicate_invocation = valid.clone();
    duplicate_invocation.schema.methods[0].synchronous_invocations =
        vec!["Clock".to_owned(), "Clock".to_owned()];
    assert!(
        duplicate_invocation
            .validate_candidate_against_schema()
            .iter()
            .any(|error| error
                .contains("synchronous-invocation identities are not strictly increasing"))
    );

    let mut out_of_order_invocation = valid;
    out_of_order_invocation.schema.methods[0].synchronous_invocations =
        vec!["TaskRuntime".to_owned(), "Clock".to_owned()];
    assert!(
        out_of_order_invocation
            .validate_candidate_against_schema()
            .iter()
            .any(|error| error
                .contains("synchronous-invocation identities are not strictly increasing"))
    );
}

#[test]
fn schema_validation_requires_selection_and_readable_names() {
    let mut valid = windows_console_plan();
    valid.target.clear();
    valid.provider_type.clear();
    // An evaluated import is bound to its normalized target, so a free
    // universal plan carries target-free leaves.
    for row in &mut valid.rows {
        row.binding = ProviderBinding::Syscall { number: 60 };
    }
    valid.schema.trait_name = "DerivedConsole".to_owned();
    valid.schema.methods[0].requirement_owner = "BaseConsole".to_owned();
    valid.schema.methods[0].name = "operation".to_owned();
    valid.rows[0].method = "operation".to_owned();
    valid.schema.methods[1].name = "operation".to_owned();
    valid.rows[1].method = "operation".to_owned();
    assert!(
        valid.validate_candidate_against_schema().is_empty(),
        "free universal plans, inherited owners, and duplicate readable overload names remain valid"
    );
    assert!(
        valid.covers_schema(),
        "exact overload identity still selects"
    );

    let mut blank_plan = valid.clone();
    blank_plan.name.clear();
    assert!(
        blank_plan
            .validate_candidate_against_schema()
            .iter()
            .any(|error| error.contains("no exact selection name"))
    );

    let mut blank_schema = valid.clone();
    blank_schema.schema.trait_name.clear();
    assert!(
        blank_schema
            .validate_candidate_against_schema()
            .iter()
            .any(|error| error.contains("no exact boundary-slot identity"))
    );

    let mut blank_method = valid;
    blank_method.schema.methods[0].name.clear();
    blank_method.rows[0].method.clear();
    assert!(
        blank_method.covers_schema(),
        "a matching blank label still proves why a separate presence fence is required"
    );
    assert!(
        blank_method
            .validate_candidate_against_schema()
            .iter()
            .any(|error| error.contains("method with no readable drift name"))
    );
}

#[test]
fn schema_validation_requires_canonical_binding_payloads() {
    fn plan_with_binding(binding: ProviderBinding) -> ProviderPlan {
        let mut plan = windows_console_plan();
        plan.rows[0].binding = binding;
        plan
    }

    let valid_bindings = [
        normalized_windows_import(b"kernel32.dll", b"WriteFile"),
        ProviderBinding::Syscall { number: 0 },
        ProviderBinding::Syscall {
            number: i64::from(u32::MAX),
        },
        ProviderBinding::CompilerIntrinsic {
            machine: "Console::write_line".to_owned(),
        },
        ProviderBinding::VtableSlot { index: 0 },
        ProviderBinding::VtableField {
            table: "StandardConsole".to_owned(),
            field: "write_line".to_owned(),
        },
        ProviderBinding::TableFunction {
            table: "StandardConsole".to_owned(),
            field: "write_line".to_owned(),
        },
        ProviderBinding::CheckedAdapter {
            machine_identity: "write_line_adapter".to_owned(),
            machine_package_identity: None,
        },
    ];
    for binding in valid_bindings {
        assert!(
            plan_with_binding(binding)
                .validate_candidate_against_schema()
                .is_empty(),
            "every closed binding family accepts its exact canonical payload"
        );
    }

    let mut wrong_target =
        plan_with_binding(normalized_windows_import(b"kernel32.dll", b"WriteFile"));
    wrong_target.target = "linux_x86_64".to_owned();
    assert!(
        wrong_target
            .validate_candidate_against_schema()
            .iter()
            .any(|error| error.contains("normalized import targets `windows_x86_64`"))
    );

    for binding in [
        ProviderBinding::Syscall { number: 0 },
        ProviderBinding::CompilerIntrinsic {
            machine: "Console::write_line".to_owned(),
        },
        ProviderBinding::VtableSlot { index: 0 },
    ] {
        let mut free = plan_with_binding(binding);
        free.target.clear();
        free.provider_type.clear();
        for row in &mut free.rows[1..] {
            row.binding = ProviderBinding::Syscall { number: 60 };
        }
        assert!(
            free.validate_candidate_against_schema().is_empty(),
            "irreducible non-table leaves remain valid without a target or nominal provider type"
        );
    }

    let corruptions = [
        (ProviderBinding::Syscall { number: -1 }, "syscall number -1"),
        (
            ProviderBinding::Syscall {
                number: i64::from(u32::MAX) + 1,
            },
            "target syscall plan requires a value in 0..=4294967295",
        ),
        (
            ProviderBinding::CompilerIntrinsic {
                machine: String::new(),
            },
            "compiler intrinsic has no exact realization-machine identity",
        ),
        (
            ProviderBinding::VtableSlot { index: -1 },
            "vtable slot index -1 is negative",
        ),
        (
            ProviderBinding::VtableField {
                table: String::new(),
                field: "write_line".to_owned(),
            },
            "without an attached provider data type",
        ),
        (
            ProviderBinding::VtableField {
                table: "StandardConsole".to_owned(),
                field: String::new(),
            },
            "table binding has no exact field identity",
        ),
        (
            ProviderBinding::VtableField {
                table: "OtherConsole".to_owned(),
                field: "write_line".to_owned(),
            },
            "table owner `OtherConsole` does not match nominal provider type `StandardConsole`",
        ),
        (
            ProviderBinding::TableFunction {
                table: String::new(),
                field: "write_line".to_owned(),
            },
            "without an attached provider data type",
        ),
        (
            ProviderBinding::TableFunction {
                table: "StandardConsole".to_owned(),
                field: String::new(),
            },
            "table binding has no exact field identity",
        ),
        (
            ProviderBinding::TableFunction {
                table: "OtherConsole".to_owned(),
                field: "write_line".to_owned(),
            },
            "table owner `OtherConsole` does not match nominal provider type `StandardConsole`",
        ),
        (
            ProviderBinding::CheckedAdapter {
                machine_identity: String::new(),
                machine_package_identity: None,
            },
            "checked adapter has no exact machine identity",
        ),
    ];
    for (binding, expected) in corruptions {
        let errors = plan_with_binding(binding).validate_candidate_against_schema();
        assert!(
            errors.iter().any(|error| error.contains(expected)),
            "missing `{expected}` in {errors:?}"
        );
    }

    for binding in [
        ProviderBinding::VtableField {
            table: "StandardConsole".to_owned(),
            field: "write_line".to_owned(),
        },
        ProviderBinding::TableFunction {
            table: "StandardConsole".to_owned(),
            field: "write_line".to_owned(),
        },
    ] {
        let mut free_table = plan_with_binding(binding);
        free_table.provider_type.clear();
        let errors = free_table.validate_candidate_against_schema();
        assert!(
            errors
                .iter()
                .any(|error| error.contains("table binding has no nominal provider type")),
            "missing nominal table-owner rejection in {errors:?}"
        );
    }

    let mut free_adapter = plan_with_binding(ProviderBinding::CheckedAdapter {
        machine_identity: "write_line_adapter".to_owned(),
        machine_package_identity: None,
    });
    free_adapter.provider_type.clear();
    let errors = free_adapter.validate_candidate_against_schema();
    assert!(
        errors
            .iter()
            .any(|error| error.contains("has no nominal provider type")),
        "missing nominal-provider rejection in {errors:?}"
    );
}

#[test]
fn normalized_import_identity_enters_provider_plan_identity_atomically() {
    let mut baseline = windows_console_plan();
    baseline.rows[0].binding = normalized_windows_import(b"kernel32.dll", b"WriteFile");
    let baseline_identity = baseline.report_fingerprint();

    let mut changed_library = baseline.clone();
    changed_library.rows[0].binding = normalized_windows_import(b"kernelbase.dll", b"WriteFile");
    assert_ne!(baseline_identity, changed_library.report_fingerprint());

    let mut changed_export = baseline.clone();
    changed_export.rows[0].binding = normalized_windows_import(b"kernel32.dll", b"ReadFile");
    assert_ne!(baseline_identity, changed_export.report_fingerprint());
}

#[test]
fn evaluated_receipt_enters_provider_plan_identity_atomically() {
    let locator = crate::normalize_foreign_locator(
        crate::ForeignLocatorCandidate::PeByName {
            library: b"kernel32.dll".to_vec(),
            export: b"WriteFile".to_vec(),
        },
        target::TargetProfile::WindowsX64,
    )
    .expect("valid normalized Windows import");
    let mut baseline = windows_console_plan();
    baseline.rows[0].binding = ProviderBinding::Import {
        evaluated: evaluated_import(locator.clone(), 11),
    };
    let mut changed_receipt = baseline.clone();
    changed_receipt.rows[0].binding = ProviderBinding::Import {
        evaluated: evaluated_import(locator, 21),
    };

    assert_ne!(
        baseline.report_fingerprint(),
        changed_receipt.report_fingerprint()
    );
    assert_ne!(
        baseline.identity_digest(),
        changed_receipt.identity_digest()
    );
}

#[test]
fn macho_locator_coordinates_enter_provider_plan_identity_atomically() {
    let mut baseline = windows_console_plan();
    baseline.target = "macos_arm64".to_owned();
    baseline.rows[0].binding = normalized_macos_import(b"/usr/lib/libSystem.B.dylib", b"_write");
    let baseline_identity = baseline.identity_digest();

    let mut changed_install_name = baseline.clone();
    changed_install_name.rows[0].binding =
        normalized_macos_import(b"/usr/lib/libobjc.A.dylib", b"_write");
    assert_ne!(baseline_identity, changed_install_name.identity_digest());

    let mut changed_symbol = baseline;
    changed_symbol.rows[0].binding =
        normalized_macos_import(b"/usr/lib/libSystem.B.dylib", b"_read");
    assert_ne!(baseline_identity, changed_symbol.identity_digest());
}
