//! Retained native-handle query-release occurrence realization: the verified
//! build filesystem replay record derives occurrence-specific
//! ordinary-release contracts, and stale or substituted proof earns no row.

use std::collections::BTreeSet;

use super::super::{
    TerminalAuthorityPolicyBuildError, UnsettledFilesystemRequirement, filesystem_mechanism_row,
    filesystem_native_handle_query_release_contracts, filesystem_release_bound_mechanism,
    filesystem_release_mechanism_row, filesystem_release_occurrence_mechanism_rows,
    normalized_foreign_terminal_mechanism, terminal_authority_policy_with_rows,
};
use build_evaluation::{
    BuildFilesystemReplayRecordLimits, ReviewOnlyBuildFilesystemReplayRecord,
    capture_verified_build_filesystem_replay_record,
    recover_review_only_build_filesystem_replay_record,
};
use effects::{
    CheckedSyscallArgumentContractIdentity, NormalizedForeignTerminalMechanismIdentity,
    SyscallTerminalMechanismIdentity, TerminalMechanismIdentity,
    provider_plan::{
        EvaluatedForeignImport, ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceMethod,
        ServiceSchema,
    },
};

fn method(name: &str) -> ServiceMethod {
    ServiceMethod {
        name: name.to_owned(),
        requirement_owner: "test::FilesystemHost".to_owned(),
        requirement_owner_package_identity: None,
        requirement_identity: format!("test::FilesystemHost::{name}#exact"),
        ..ServiceMethod::default()
    }
}

fn release_bound_mechanism(
    number: u32,
    contract: super::super::FilesystemOrdinaryReleaseContract,
) -> TerminalMechanismIdentity {
    SyscallTerminalMechanismIdentity::new(
        target::TargetProfile::LinuxX64,
        number,
        contract.checked_argument_contract(),
    )
    .into()
}

/// The unconstrained Windows `CloseHandle` import: the admitted calling plan
/// is its whole contract.
fn close_handle_import() -> NormalizedForeignTerminalMechanismIdentity {
    let locator = target::normalize_foreign_locator(
        target::ForeignLocatorCandidate::PeByName {
            library: b"kernel32.dll".to_vec(),
            export: b"CloseHandle".to_vec(),
        },
        target::TargetProfile::WindowsX64,
    )
    .expect("the CloseHandle import normalizes");
    NormalizedForeignTerminalMechanismIdentity::from_normalized_locator(
        &locator,
        effects::provider_plan::BoundaryCallingPlanCommitment::from_digest([34; 32]),
    )
}

fn release_bound_import(
    contract: super::super::FilesystemOrdinaryReleaseContract,
) -> TerminalMechanismIdentity {
    close_handle_import()
        .with_checked_argument_contract(contract.checked_argument_contract())
        .into()
}

fn retained_record(occurrences: &[(&[u8], u64)]) -> ReviewOnlyBuildFilesystemReplayRecord {
    capture_verified_build_filesystem_replay_record(
        &build_evaluation::test_support::replayable_native_handle_query_chain_summary(occurrences),
        BuildFilesystemReplayRecordLimits::default(),
    )
    .expect("retained query-release chains encode")
    .expect("verified query-release chains retain replay custody")
}

fn release_contracts(
    record: &ReviewOnlyBuildFilesystemReplayRecord,
) -> Vec<super::super::FilesystemOrdinaryReleaseContract> {
    filesystem_native_handle_query_release_contracts(
        record,
        BuildFilesystemReplayRecordLimits::default(),
    )
    .expect("retained occurrences realize their contracts")
}

#[test]
fn retained_occurrence_derives_one_evidence_bound_empty_row() {
    let record = retained_record(&[(b"pkg/main.omg", 7)]);
    let contracts = release_contracts(&record);
    let [contract] = contracts.as_slice() else {
        panic!("one retained occurrence derives exactly one contract")
    };

    let bound = release_bound_mechanism(3, *contract);
    let row = filesystem_release_mechanism_row(bound, &method("close_handle"), *contract)
        .expect("the occurrence-bound mechanism earns the explicit empty row");
    assert_eq!(row.mechanism(), bound);
    assert!(row.disposition().is_authority_class_empty());

    let policy = terminal_authority_policy_with_rows(vec![row]).expect("exact release policy");
    assert!(
        policy
            .classify(bound)
            .expect("the bound mechanism classifies")
            .is_authority_class_empty()
    );
    // The same syscall number under an unconstrained contract stays
    // fail-closed: the row is bound to this occurrence's contract, not to
    // the requirement or the number.
    let unconstrained: TerminalMechanismIdentity = SyscallTerminalMechanismIdentity::new(
        target::TargetProfile::LinuxX64,
        3,
        CheckedSyscallArgumentContractIdentity::from_digest([4; 32]),
    )
    .into();
    assert!(policy.classify(unconstrained).is_err());
    // The generic emitter still refuses the bound mechanism: the contract
    // must arrive through the release path, not silently.
    assert_eq!(
        filesystem_mechanism_row(bound, &method("close_handle")),
        Err(UnsettledFilesystemRequirement::OrdinaryReleaseContract)
    );
}

#[test]
fn stale_or_substituted_occurrence_proof_earns_no_row() {
    let retained = retained_record(&[(b"pkg/main.omg", 7)]);
    let stale = retained_record(&[(b"pkg/other.omg", 7)]);
    let substituted = retained_record(&[(b"pkg/main.omg", 9)]);
    let retained_contracts = release_contracts(&retained);
    let [contract] = retained_contracts.as_slice() else {
        panic!("the retained occurrence derives one contract")
    };
    let stale_contracts = release_contracts(&stale);
    let [stale_contract] = stale_contracts.as_slice() else {
        panic!("the stale occurrence derives one contract")
    };
    let substituted_contracts = release_contracts(&substituted);
    let [substituted_contract] = substituted_contracts.as_slice() else {
        panic!("the substituted occurrence derives one contract")
    };
    assert_ne!(contract, stale_contract);
    assert_ne!(contract, substituted_contract);

    let bound = release_bound_mechanism(3, *contract);
    for other in [*stale_contract, *substituted_contract] {
        assert_eq!(
            filesystem_release_mechanism_row(bound, &method("close_handle"), other),
            Err(UnsettledFilesystemRequirement::OrdinaryReleaseContract),
            "a mechanism bound to this occurrence cannot inherit another \
             occurrence's contract"
        );
    }
    // And the foreign evidence's own mechanism cannot claim this occurrence's
    // row either: the coordinate is checked, not merely present.
    for other_mechanism in [
        release_bound_mechanism(3, *stale_contract),
        release_bound_mechanism(3, *substituted_contract),
    ] {
        assert_eq!(
            filesystem_release_mechanism_row(other_mechanism, &method("close_handle"), *contract),
            Err(UnsettledFilesystemRequirement::OrdinaryReleaseContract)
        );
    }
}

#[test]
fn each_retained_occurrence_binds_a_distinct_contract() {
    let record = retained_record(&[(b"pkg/main.omg", 7), (b"pkg/lib.omg", 8)]);
    let contracts = release_contracts(&record);
    let [first, second] = contracts.as_slice() else {
        panic!("two retained occurrences derive two contracts")
    };
    assert_ne!(
        first, second,
        "each occurrence earns its own evidence-bound contract"
    );
    for contract in [*first, *second] {
        let bound = release_bound_mechanism(3, contract);
        let row = filesystem_release_mechanism_row(bound, &method("close_handle"), contract)
            .expect("each occurrence-bound mechanism earns its empty row");
        assert!(row.disposition().is_authority_class_empty());
    }
}

#[test]
fn a_record_without_a_release_occurrence_derives_no_contracts() {
    let record = capture_verified_build_filesystem_replay_record(
        &build_evaluation::test_support::replayable_unknown_descriptor_summary(),
        BuildFilesystemReplayRecordLimits::default(),
    )
    .expect("the retained failure record encodes")
    .expect("the retained failure record keeps custody");
    assert!(
        release_contracts(&record).is_empty(),
        "a record without a query-release occurrence realizes no contracts"
    );
}

#[test]
fn tampered_occurrence_evidence_cannot_realize_the_bound_contract() {
    let record = retained_record(&[(b"pkg/main.omg", 7)]);
    let contracts = release_contracts(&record);
    let [contract] = contracts.as_slice() else {
        panic!("the retained occurrence derives one contract")
    };
    let contract = *contract;

    // Substitute one byte of the returned final-path lane: the carrier's
    // retained post-state still holds the original path, so the checked
    // query record rejects the substitution on rehydration.
    let mut tampered = record.canonical_bytes().to_vec();
    let needle = b"C:\\pkg\\main.omg";
    let offset = tampered
        .windows(needle.len())
        .position(|window| window == needle)
        .expect("the retained query returns the resolved path");
    tampered[offset] ^= 1;

    match recover_review_only_build_filesystem_replay_record(
        &tampered,
        BuildFilesystemReplayRecordLimits::default(),
    ) {
        Err(_) => {}
        Ok(tampered_record) => {
            let tampered_contracts = filesystem_native_handle_query_release_contracts(
                &tampered_record,
                BuildFilesystemReplayRecordLimits::default(),
            );
            match tampered_contracts {
                Err(_) => {}
                Ok(contracts) => assert!(
                    contracts.iter().all(|candidate| *candidate != contract),
                    "substituted evidence never derives the original contract"
                ),
            }
        }
    }
}

#[test]
fn retained_occurrence_binds_a_normalized_foreign_release_row() {
    let record = retained_record(&[(b"pkg/main.omg", 7)]);
    let contracts = release_contracts(&record);
    let [contract] = contracts.as_slice() else {
        panic!("one retained occurrence derives exactly one contract")
    };

    // The constrained import occurrence carries the retained contract as its
    // checked coordinate beside the unchanged admitted plan and earns the
    // same evidence-bound empty row a direct syscall does.
    let bound = release_bound_import(*contract);
    let row = filesystem_release_mechanism_row(bound, &method("close_handle"), *contract)
        .expect("the occurrence-bound import earns the explicit empty row");
    assert_eq!(row.mechanism(), bound);
    assert!(row.disposition().is_authority_class_empty());

    let policy = terminal_authority_policy_with_rows(vec![row]).expect("exact release policy");
    assert!(
        policy
            .classify(bound)
            .expect("the bound import classifies")
            .is_authority_class_empty()
    );
    // The unconstrained import of the same symbol under the same admitted
    // plan is a different key: the row binds the occurrence, not the locator.
    let unconstrained: TerminalMechanismIdentity = close_handle_import().into();
    assert!(policy.classify(unconstrained).is_err());
    assert_eq!(
        filesystem_release_mechanism_row(unconstrained, &method("close_handle"), *contract),
        Err(UnsettledFilesystemRequirement::OrdinaryReleaseContract),
        "the admitted plan alone is not narrowing evidence"
    );
    // The generic emitter still refuses the bound import: the contract must
    // arrive through the release path.
    assert_eq!(
        filesystem_mechanism_row(bound, &method("close_handle")),
        Err(UnsettledFilesystemRequirement::OrdinaryReleaseContract)
    );
    // Another occurrence's contract neither fits this key nor claims its row.
    let stale = retained_record(&[(b"pkg/other.omg", 7)]);
    let stale_contracts = release_contracts(&stale);
    let [stale_contract] = stale_contracts.as_slice() else {
        panic!("the stale occurrence derives one contract")
    };
    assert_ne!(contract, stale_contract);
    assert_eq!(
        filesystem_release_mechanism_row(bound, &method("close_handle"), *stale_contract),
        Err(UnsettledFilesystemRequirement::OrdinaryReleaseContract)
    );
    assert_eq!(
        filesystem_release_mechanism_row(
            release_bound_import(*stale_contract),
            &method("close_handle"),
            *contract
        ),
        Err(UnsettledFilesystemRequirement::OrdinaryReleaseContract)
    );
    assert!(
        policy
            .classify(release_bound_import(*stale_contract))
            .is_err()
    );
}

#[test]
fn an_empty_checked_foreign_argument_contract_is_never_a_policy_key() {
    let empty: TerminalMechanismIdentity = close_handle_import()
        .with_checked_argument_contract(CheckedSyscallArgumentContractIdentity::from_digest(
            [0; 32],
        ))
        .into();
    let row = super::row(empty, []);
    assert_eq!(
        terminal_authority_policy_with_rows(vec![row]).err(),
        Some(TerminalAuthorityPolicyBuildError::EmptyCheckedForeignArgumentContract(empty))
    );
}

/// One selected `FilesystemHost` provider plan serving `methods`; each row
/// binds `binding` in order and carries the method's exact requirement
/// identity.
fn filesystem_plan(
    profile: target::TargetProfile,
    methods: &[&str],
    bindings: Vec<ProviderBinding>,
) -> ProviderPlan {
    ProviderPlan {
        name: "test::filesystem-provider".into(),
        target: profile.target_name().into(),
        schema: ServiceSchema {
            trait_name: "test::FilesystemHost".into(),
            methods: methods.iter().map(|name| method(name)).collect(),
            ..ServiceSchema::default()
        },
        rows: methods
            .iter()
            .zip(bindings)
            .map(|(name, binding)| ProviderPlanRow {
                method: (*name).to_owned(),
                requirement_identity: format!("test::FilesystemHost::{name}#exact"),
                requirement_lifetime_partition: Vec::new(),
                binding,
            })
            .collect(),
        ..ProviderPlan::default()
    }
}

fn evaluated_close_handle_import() -> EvaluatedForeignImport {
    let locator = target::normalize_foreign_locator(
        target::ForeignLocatorCandidate::PeByName {
            library: b"kernel32.dll".to_vec(),
            export: b"CloseHandle".to_vec(),
        },
        target::TargetProfile::WindowsX64,
    )
    .expect("the CloseHandle import normalizes");
    let usage = effects::provider_plan::EvaluatedBindingUsage::from_evaluator(
        7, 1, 10, 1_000, 0, 0, 4, 12, 3, 0,
    )
    .expect("valid fixture usage");
    let receipt = effects::provider_plan::EvaluatedBindingReceipt::from_evaluation(
        None,
        "fixture::producer".to_owned(),
        effects::provider_plan::EvaluatedBindingProducerClosureDigest::from_bytes([11; 32])
            .unwrap(),
        1,
        usage,
        effects::provider_plan::EvaluatedBindingEvaluationDigest::from_bytes([12; 32]).unwrap(),
        1,
        effects::provider_plan::EvaluatedBindingMaterializationDigest::from_bytes([13; 32])
            .unwrap(),
        locator.identity_digest(),
    )
    .expect("valid fixture receipt");
    EvaluatedForeignImport::from_retained_evidence(locator, receipt)
        .expect("receipt matches fixture locator")
}

#[test]
fn occurrence_rows_mint_only_for_demanded_release_cohort_selections() {
    let record = retained_record(&[(b"pkg/main.omg", 7), (b"pkg/lib.omg", 8)]);
    let contracts = release_contracts(&record);
    let [first, second] = contracts.as_slice() else {
        panic!("two retained occurrences derive two contracts")
    };
    let close = "test::FilesystemHost::close_handle#exact".to_owned();
    let read = "test::FilesystemHost::read#exact".to_owned();
    let undemanded_close = "test::FilesystemHost::close#exact".to_owned();
    let plan = filesystem_plan(
        target::TargetProfile::LinuxX64,
        &["close_handle", "read", "close"],
        vec![
            ProviderBinding::Syscall { number: 3 },
            ProviderBinding::Syscall { number: 4 },
            ProviderBinding::Syscall { number: 5 },
        ],
    );
    let selected = effects::SelectedProviderPlanFacts::from_selected_plans(vec![plan])
        .expect("selected plans");
    let demanded = BTreeSet::from([close.clone(), read.clone()]);

    let rows = filesystem_release_occurrence_mechanism_rows(&contracts, &demanded, &selected, &[])
        .expect("demanded release cohort mints its bound rows");
    // Each retained occurrence mints exactly one row for the demanded
    // release-cohort row: two occurrences, two distinct bound keys. The
    // demanded facet cohort (`read`) and the undemanded release cohort
    // (`close`) mint nothing.
    assert_eq!(rows.len(), 2);
    for (row, contract) in rows.iter().zip([first, second]) {
        let bound = release_bound_mechanism(3, *contract);
        assert_eq!(row.mechanism(), bound);
        assert!(row.disposition().is_authority_class_empty());
    }
    let policy = terminal_authority_policy_with_rows(rows).expect("exact release rows");
    assert!(
        policy
            .classify(release_bound_mechanism(3, *first))
            .is_ok_and(|disposition| disposition.is_authority_class_empty())
    );
    // The conservative mechanism for the same number was never rowed.
    assert!(
        policy
            .classify(SyscallTerminalMechanismIdentity::new(
                target::TargetProfile::LinuxX64,
                3,
                CheckedSyscallArgumentContractIdentity::from_digest([9; 32]),
            ))
            .is_err()
    );

    // Undemanded release cohorts mint nothing even with retained contracts;
    // demanding `close` itself mints its own pair, so the earlier exclusion
    // was demand, not the method.
    let undemanded = BTreeSet::from([read.clone()]);
    assert!(
        filesystem_release_occurrence_mechanism_rows(&contracts, &undemanded, &selected, &[])
            .expect("undemanded release cohorts mint nothing")
            .is_empty()
    );
    let close_demanded = BTreeSet::from([undemanded_close]);
    assert_eq!(
        filesystem_release_occurrence_mechanism_rows(&contracts, &close_demanded, &selected, &[])
            .expect("a demanded release cohort mints its rows")
            .len(),
        2
    );
    // Without retained contracts nothing mints either: the occurrence's own
    // evidence is the only input that can produce these keys.
    assert!(
        filesystem_release_occurrence_mechanism_rows(&[], &demanded, &selected, &[])
            .expect("no contracts mint no rows")
            .is_empty()
    );
    // A stale record's contracts mint rows under different keys entirely.
    let stale = retained_record(&[(b"pkg/other.omg", 7)]);
    let stale_contracts = release_contracts(&stale);
    let stale_rows =
        filesystem_release_occurrence_mechanism_rows(&stale_contracts, &demanded, &selected, &[])
            .expect("stale contracts still mint their own rows");
    assert_eq!(stale_rows.len(), 1);
    assert!(
        policy.classify(stale_rows[0].mechanism()).is_err(),
        "a stale occurrence's bound key has no row in this record's policy"
    );
}

#[test]
fn occurrence_rows_mint_the_bound_normalized_foreign_key() {
    let record = retained_record(&[(b"pkg/main.omg", 7)]);
    let contracts = release_contracts(&record);
    let [contract] = contracts.as_slice() else {
        panic!("one retained occurrence derives one contract")
    };
    let contract = *contract;
    let close = "test::FilesystemHost::close_handle#exact".to_owned();
    let evaluated = evaluated_close_handle_import();
    let boundary_plan = calling_conventions::evaluate_ordinary_boundary_entry_plan(
        calling_conventions::CallingPolicy::native_for_target(target::NativeTarget::windows_x64()),
        &calling_conventions::CallSignature {
            parameters: vec![calling_conventions::ValueShape::integer(8, 8)],
            result: None,
        },
    )
    .expect("admitted calling plan")
    .plan()
    .clone();
    let plan = filesystem_plan(
        target::TargetProfile::WindowsX64,
        &["close_handle"],
        vec![ProviderBinding::Import {
            evaluated: evaluated.clone(),
        }],
    );
    let selected = effects::SelectedProviderPlanFacts::from_selected_plans(vec![plan])
        .expect("selected plans");
    let external = calling_conventions::ExternalBindingRow {
        target_name: target::TargetProfile::WindowsX64.target_name().into(),
        trait_name: "test::FilesystemHost".into(),
        method: "close_handle".into(),
        requirement_identity: close.clone(),
        table_type: String::new(),
        boundary_entry_plan: Some(boundary_plan.clone()),
        binding: calling_conventions::ExternalBindingKind::Import {
            locator: evaluated.locator().clone(),
        },
    };
    let demanded = BTreeSet::from([close.clone()]);

    let rows = filesystem_release_occurrence_mechanism_rows(
        &contracts,
        &demanded,
        &selected,
        std::slice::from_ref(&external),
    )
    .expect("the demanded import mints its bound row");
    let [row] = rows.as_slice() else {
        panic!("one retained occurrence mints exactly one bound import row")
    };
    let base = normalized_foreign_terminal_mechanism(evaluated.locator(), &boundary_plan)
        .expect("admitted plan derives the unconstrained foreign key");
    let bound = filesystem_release_bound_mechanism(base, contract)
        .expect("a normalized foreign key accepts the checked coordinate");
    assert_eq!(row.mechanism(), bound);
    assert!(row.disposition().is_authority_class_empty());

    // The admitted calling plan alone mints nothing: the external row's
    // retained boundary contract is required to form the key.
    let mut no_plan = external.clone();
    no_plan.boundary_entry_plan = None;
    assert!(
        filesystem_release_occurrence_mechanism_rows(
            &contracts,
            &demanded,
            &selected,
            std::slice::from_ref(&no_plan),
        )
        .expect("a missing boundary contract mints no row")
        .is_empty()
    );
    assert!(
        filesystem_release_occurrence_mechanism_rows(&contracts, &demanded, &selected, &[])
            .expect("a missing external row mints no row")
            .is_empty()
    );
}
