use super::{
    admitted_progress_receipt, entry_id, installed_code, installed_code_with_fill,
    progress_installation_fixture, provider_occurrence_binding, root_id,
};
use crate::{
    ComponentProgressDemandIdentity, ComponentProgressReceiptBinding,
    InstalledProviderOccurrenceId, InstalledRootLedger, ProgressProfileEstablishmentAttestation,
    ProgressProfileEstablishmentReceiptId, ProgressProfileGrantInvocationId,
};
use effects::provider_plan::ServiceProgressEstablishmentRoute;
use effects::{CheckedComponentProgressDemand, ComponentProgressManifest};

#[test]
fn component_progress_seals_against_distinct_exact_subject_and_issuer_occurrences() {
    let (selected, manifest, scheduler_plan, admission_plan, route) =
        progress_installation_fixture();
    let mut code = installed_code(54_000, entry_id(54_001));
    let mut ledger = InstalledRootLedger::claim(&mut code).expect("installation registry");
    ledger
        .seal_provider_occurrence_closure(
            &selected,
            [
                provider_occurrence_binding(
                    &code,
                    &selected,
                    scheduler_plan,
                    54_010,
                    54_020,
                    "SchedulerProvider",
                ),
                provider_occurrence_binding(
                    &code,
                    &selected,
                    admission_plan,
                    54_011,
                    54_021,
                    "SchedulerAdmissionProvider",
                ),
            ],
        )
        .expect("installed provider closure");
    let receipt = admitted_progress_receipt(
        &mut ledger,
        &code,
        54_020,
        54_021,
        admission_plan,
        54_030,
        route,
    );
    assert_ne!(receipt.subject(), receipt.issuer());

    let demand = ComponentProgressDemandIdentity::from_demand(&manifest.pending()[0]);
    let acceptance = ledger
        .seal_component_progress(
            manifest.clone(),
            [ComponentProgressReceiptBinding::new(
                demand,
                receipt.clone(),
            )],
        )
        .expect("exact component progress closure");
    assert_eq!(acceptance.manifest(), &manifest);
    assert_eq!(acceptance.receipts().collect::<Vec<_>>(), vec![&receipt]);
    assert!(acceptance.binds_installed_code(&code));
    let mut expected_provider_plans = vec![scheduler_plan, admission_plan];
    expected_provider_plans.sort_unstable();
    assert_eq!(
        acceptance.selected_provider_plan_report_identities(),
        expected_provider_plans.as_slice()
    );
    let colliding_code = installed_code_with_fill(54_000, entry_id(54_001), 1);
    assert!(!acceptance.binds_installed_code(&colliding_code));
    assert_ne!(acceptance.non_authoritative_report_fingerprint(), 0);
}

#[test]
fn component_progress_sealing_is_transactional_and_receipt_facts_are_reusable() {
    let (selected, _, scheduler_plan, admission_plan, route) = progress_installation_fixture();
    let demands = [(4, 1), (9, 2)]
        .into_iter()
        .map(
            |(statement_ordinal, call_ordinal)| CheckedComponentProgressDemand {
                provider_service_identity: "Scheduler".into(),
                provider_service_package_identity: None,
                requirement_identity: "Scheduler::wait#exact".into(),
                requirement_owner_package_identity: None,
                profile_identity: "SchedulerHandle::WeakFair".into(),
                subject_projections: vec!["queue".into()],
                origin_callable_identity: "Application::start".into(),
                origin_state_identity: "Application::start::entry".into(),
                statement_ordinal,
                call_ordinal,
            },
        )
        .collect::<Vec<_>>();
    let manifest = ComponentProgressManifest::bind("Application::start".into(), &selected, demands)
        .expect("two-demand manifest");
    let mut code = installed_code(55_000, entry_id(55_001));
    let mut ledger = InstalledRootLedger::claim(&mut code).expect("installation registry");
    ledger
        .seal_provider_occurrence_closure(
            &selected,
            [
                provider_occurrence_binding(
                    &code,
                    &selected,
                    scheduler_plan,
                    55_010,
                    55_020,
                    "SchedulerProvider",
                ),
                provider_occurrence_binding(
                    &code,
                    &selected,
                    admission_plan,
                    55_011,
                    55_021,
                    "SchedulerAdmissionProvider",
                ),
            ],
        )
        .expect("installed provider closure");
    let receipt = admitted_progress_receipt(
        &mut ledger,
        &code,
        55_020,
        55_021,
        admission_plan,
        55_030,
        route,
    );

    let missing = ledger
        .seal_component_progress(
            manifest.clone(),
            [ComponentProgressReceiptBinding::new(
                ComponentProgressDemandIdentity::from_demand(&manifest.pending()[0]),
                receipt.clone(),
            )],
        )
        .expect_err("partial closure must reject");
    assert!(missing.diagnostic().0.contains("exactly cover"));

    let bindings = manifest
        .pending()
        .iter()
        .map(|demand| {
            ComponentProgressReceiptBinding::new(
                ComponentProgressDemandIdentity::from_demand(demand),
                receipt.clone(),
            )
        })
        .collect::<Vec<_>>();
    let acceptance = ledger
        .seal_component_progress(manifest, bindings)
        .expect("corrected retry succeeds without burning acceptance");
    assert_eq!(acceptance.receipts().count(), 2);
}

#[test]
fn provider_occurrence_and_progress_route_admission_fail_closed() {
    let (selected, _, scheduler_plan, admission_plan, route) = progress_installation_fixture();
    let mut code = installed_code(56_000, entry_id(56_001));
    let other_code = installed_code_with_fill(56_000, entry_id(56_001), 1);
    let mut ledger = InstalledRootLedger::claim(&mut code).expect("installation registry");
    let error = ledger
        .seal_provider_occurrence_closure(
            &selected,
            [
                provider_occurrence_binding(
                    &other_code,
                    &selected,
                    scheduler_plan,
                    56_010,
                    56_020,
                    "SchedulerProvider",
                ),
                provider_occurrence_binding(
                    &code,
                    &selected,
                    admission_plan,
                    56_011,
                    56_021,
                    "SchedulerAdmissionProvider",
                ),
            ],
        )
        .expect_err("colliding compact IDs cannot substitute exact installed evidence");
    assert!(error.0.contains("different installed-code"));

    ledger
        .seal_provider_occurrence_closure(
            &selected,
            [
                provider_occurrence_binding(
                    &code,
                    &selected,
                    scheduler_plan,
                    56_010,
                    56_020,
                    "SchedulerProvider",
                ),
                provider_occurrence_binding(
                    &code,
                    &selected,
                    admission_plan,
                    56_011,
                    56_021,
                    "SchedulerAdmissionProvider",
                ),
            ],
        )
        .expect("failed closure did not mutate the ledger");
    let wrong_route = ServiceProgressEstablishmentRoute {
        requirement_identity: "SchedulerAdmission::grant_other#exact".into(),
        ..route
    };
    let error = ledger
        .admit_progress_profile_establishment(
            ProgressProfileEstablishmentAttestation::from_provider(
                root_id(
                    56_030,
                    ProgressProfileEstablishmentReceiptId::from_normalized_identity,
                ),
                &code,
                root_id(
                    56_020,
                    InstalledProviderOccurrenceId::from_normalized_identity,
                ),
                root_id(
                    56_021,
                    InstalledProviderOccurrenceId::from_normalized_identity,
                ),
                admission_plan,
                root_id(
                    56_031,
                    ProgressProfileGrantInvocationId::from_normalized_identity,
                ),
                "SchedulerHandle::WeakFair",
                vec!["queue".into()],
                wrong_route,
            ),
        )
        .expect_err("issuer plan must realize the exact route");
    assert!(error.diagnostic().0.contains("exact requirement"));
}

#[test]
fn progress_receipt_identity_and_grant_invocation_cannot_be_rebound() {
    let (selected, _, scheduler_plan, admission_plan, route) = progress_installation_fixture();
    let mut code = installed_code(57_000, entry_id(57_001));
    let mut ledger = InstalledRootLedger::claim(&mut code).expect("installation registry");
    ledger
        .seal_provider_occurrence_closure(
            &selected,
            [
                provider_occurrence_binding(
                    &code,
                    &selected,
                    scheduler_plan,
                    57_010,
                    57_020,
                    "SchedulerProvider",
                ),
                provider_occurrence_binding(
                    &code,
                    &selected,
                    admission_plan,
                    57_011,
                    57_021,
                    "SchedulerAdmissionProvider",
                ),
            ],
        )
        .expect("installed provider closure");
    let receipt_identity = root_id(
        57_030,
        ProgressProfileEstablishmentReceiptId::from_normalized_identity,
    );
    let invocation = root_id(
        57_031,
        ProgressProfileGrantInvocationId::from_normalized_identity,
    );
    let attestation = |receipt, invocation, profile: &str| {
        ProgressProfileEstablishmentAttestation::from_provider(
            receipt,
            &code,
            root_id(
                57_020,
                InstalledProviderOccurrenceId::from_normalized_identity,
            ),
            root_id(
                57_021,
                InstalledProviderOccurrenceId::from_normalized_identity,
            ),
            admission_plan,
            invocation,
            profile,
            vec!["queue".into()],
            route.clone(),
        )
    };
    let admitted = ledger
        .admit_progress_profile_establishment(attestation(
            receipt_identity,
            invocation,
            "SchedulerHandle::WeakFair",
        ))
        .expect("first receipt admission");
    assert_eq!(
        ledger
            .admit_progress_profile_establishment(attestation(
                receipt_identity,
                invocation,
                "SchedulerHandle::WeakFair",
            ))
            .expect("exact replay is idempotent"),
        admitted
    );
    let divergent = ledger
        .admit_progress_profile_establishment(attestation(
            receipt_identity,
            invocation,
            "SchedulerHandle::StrongFair",
        ))
        .expect_err("one receipt identity cannot name another profile");
    assert!(divergent.diagnostic().0.contains("divergent evidence"));
    let second_receipt = root_id(
        57_032,
        ProgressProfileEstablishmentReceiptId::from_normalized_identity,
    );
    let duplicate_invocation = ledger
        .admit_progress_profile_establishment(attestation(
            second_receipt,
            invocation,
            "SchedulerHandle::WeakFair",
        ))
        .expect_err("one grant invocation cannot mint another receipt");
    assert!(
        duplicate_invocation
            .diagnostic()
            .0
            .contains("grant invocation")
    );
}
