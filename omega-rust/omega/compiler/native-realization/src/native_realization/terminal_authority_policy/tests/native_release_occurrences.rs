//! Retained native-handle query-release occurrence realization: the verified
//! build filesystem replay record derives occurrence-specific
//! ordinary-release contracts, and stale or substituted proof earns no row.

use super::super::{
    UnsettledFilesystemRequirement, filesystem_mechanism_row,
    filesystem_native_handle_query_release_contracts, filesystem_release_mechanism_row,
    terminal_authority_policy_with_rows,
};
use build_evaluation::{
    BuildFilesystemReplayRecordLimits, ReviewOnlyBuildFilesystemReplayRecord,
    capture_verified_build_filesystem_replay_record,
    recover_review_only_build_filesystem_replay_record,
};
use effects::{
    CheckedSyscallArgumentContractIdentity, SyscallTerminalMechanismIdentity,
    TerminalMechanismIdentity, provider_plan::ServiceMethod,
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
