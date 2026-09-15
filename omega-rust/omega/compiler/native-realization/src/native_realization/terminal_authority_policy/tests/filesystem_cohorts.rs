//! Canonical settled `FilesystemHost` cohort table: partition, exact rows, refusals.

use super::super::{
    FilesystemCohortDisposition, TerminalAuthorityPolicyBuildError, UnsettledFilesystemRequirement,
    filesystem_host_permission_rows, filesystem_mechanism_row,
    filesystem_ordinary_release_contract, filesystem_release_mechanism_row,
    settled_filesystem_cohort, terminal_authority_policy_with_rows,
};
use effects::{
    CheckedPhysicalTerminalMechanismIdentity, CheckedSyscallArgumentContractIdentity,
    PortableFilesystemAuthorityFacet, SyscallTerminalMechanismIdentity, TerminalAuthorityClass,
    TerminalAuthorityDisposition, TerminalMechanismIdentity,
    provider_plan::{ServiceMethod, ServiceSchema},
};

/// The complete canonical `FilesystemHost` cohort in declaration order,
/// mirroring `source/library/std/filesystem_host.omg`.
const CANONICAL_METHODS: &[&str] = &[
    "create",
    "open",
    "open_create",
    "read",
    "write",
    "read_at",
    "write_at",
    "close",
    "remove",
    "seek",
    "create_dir",
    "remove_dir",
    "create_dir_name",
    "open_at",
    "unlink_at",
    "set_permissions",
    "set_file_permissions",
    "rename",
    "hard_link",
    "symlink",
    "read_link",
    "canonicalize",
    "read_dir",
    "find_first",
    "find_next",
    "find_close",
    "create_hard_link",
    "open_path_handle",
    "close_handle",
    "get_osfhandle",
    "final_path_name_by_handle",
    "set_file_time",
    "lock_file_ex",
    "unlock_file",
    "get_last_error",
    "remove_name",
    "remove_dir_name",
    "read_metadata",
    "read_file_metadata",
    "read_symlink_metadata",
    "set_len",
    "set_file_times",
    "sync",
    "sync_data",
    "duplicate",
    "lock_file",
    "change_owner",
    "change_owner_no_follow",
    "change_file_owner",
    "errno",
];

fn method(name: &str) -> ServiceMethod {
    ServiceMethod {
        name: name.to_owned(),
        requirement_owner: "test::FilesystemHost".to_owned(),
        requirement_owner_package_identity: None,
        requirement_identity: format!("test::FilesystemHost::{name}#exact"),
        ..ServiceMethod::default()
    }
}

fn schema(names: &[&str]) -> ServiceSchema {
    ServiceSchema {
        trait_name: "test::FilesystemHost".to_owned(),
        methods: names.iter().map(|name| method(name)).collect(),
        ..ServiceSchema::default()
    }
}

fn mechanism(number: u32, contract_byte: u8) -> TerminalMechanismIdentity {
    SyscallTerminalMechanismIdentity::new(
        target::TargetProfile::LinuxX64,
        number,
        CheckedSyscallArgumentContractIdentity::from_digest([contract_byte; 32]),
    )
    .into()
}

#[test]
fn cohort_table_partitions_the_complete_canonical_schema() {
    assert_eq!(CANONICAL_METHODS.len(), 50);
    let covered = CANONICAL_METHODS
        .iter()
        .filter(|name| settled_filesystem_cohort(name).is_some())
        .count();
    assert_eq!(covered, 50);
    for unknown in [
        "",
        "Read",
        "close_everything",
        "read_link_target",
        "remove_dir_all",
    ] {
        assert_eq!(settled_filesystem_cohort(unknown), None);
    }
}

#[test]
fn settled_cohorts_carry_their_exact_policy_dispositions() {
    use FilesystemCohortDisposition::Facets;
    use PortableFilesystemAuthorityFacet as Facet;
    assert_eq!(
        settled_filesystem_cohort("read_link"),
        Some(Facets(&[Facet::MetadataQuery]))
    );
    for empty in [
        "seek",
        "get_osfhandle",
        "duplicate",
        "lock_file",
        "lock_file_ex",
        "unlock_file",
        "get_last_error",
        "errno",
        "sync",
        "sync_data",
    ] {
        assert_eq!(
            settled_filesystem_cohort(empty),
            Some(Facets(&[])),
            "{empty} is an explicit empty disposition"
        );
    }
    for release in ["close", "find_close", "close_handle"] {
        assert_eq!(
            settled_filesystem_cohort(release),
            Some(FilesystemCohortDisposition::OrdinaryReleaseContract),
            "{release} narrows only under the retained occurrence-specific release contract"
        );
    }
    assert_eq!(
        settled_filesystem_cohort("open"),
        Some(Facets(&PortableFilesystemAuthorityFacet::ALL))
    );
    assert_eq!(
        settled_filesystem_cohort("create"),
        Some(Facets(&[
            Facet::ContentWrite,
            Facet::NamespaceMutation,
            Facet::MetadataMutation,
        ]))
    );
    assert_eq!(
        settled_filesystem_cohort("create_dir"),
        Some(Facets(&[Facet::NamespaceMutation, Facet::MetadataMutation]))
    );
    assert_eq!(
        settled_filesystem_cohort("find_next"),
        Some(Facets(&[Facet::DirectoryEnumeration]))
    );
}

#[test]
fn permission_rows_cover_the_complete_schema_with_exact_keys() {
    let schema = schema(CANONICAL_METHODS);
    let rows = filesystem_host_permission_rows(&schema).expect("complete permission table");
    assert_eq!(rows.len(), CANONICAL_METHODS.len());
    for method in &schema.methods {
        let matching = rows
            .iter()
            .filter(|row| row.requirement_identity() == method.requirement_identity)
            .collect::<Vec<_>>();
        let [row] = matching.as_slice() else {
            panic!("permission table lost exact row for {}", method.name);
        };
        assert_eq!(row.service_schema(), schema.identity_digest());
        let expected = match settled_filesystem_cohort(&method.name) {
            Some(FilesystemCohortDisposition::Facets(facets)) => {
                TerminalAuthorityDisposition::from_filesystem_facets(facets.iter().copied())
            }
            // Ordinary-release cohorts retain service reach and exact review
            // identity through an explicit empty grant.
            Some(FilesystemCohortDisposition::OrdinaryReleaseContract) => {
                TerminalAuthorityDisposition::from_classes([])
            }
            None => unreachable!("canonical method resolved no cohort"),
        };
        assert_eq!(row.permitted(), &expected, "{}", method.name);
    }
}

#[test]
fn permission_rows_reject_unknown_schema_members_without_partial_table() {
    let mut names = CANONICAL_METHODS.to_vec();
    names.push("invented_operation");
    assert_eq!(
        filesystem_host_permission_rows(&schema(&names)),
        Err(UnsettledFilesystemRequirement::UnknownRequirement)
    );
}

#[test]
fn mechanism_rows_emit_exact_dispositions_and_refuse_unproved_release() {
    let sync = mechanism(74, 9);
    let row = filesystem_mechanism_row(sync, &method("sync"))
        .expect("durability-only completion is an explicit empty row");
    assert_eq!(row.mechanism(), sync);
    assert!(row.disposition().is_authority_class_empty());

    let read = mechanism(0, 7);
    let row = filesystem_mechanism_row(read, &method("read"))
        .expect("content-read cohort classifies exactly");
    assert_eq!(
        row.disposition().classes(),
        &[TerminalAuthorityClass::FilesystemContentRead]
    );

    let read_link = mechanism(89, 8);
    let row = filesystem_mechanism_row(read_link, &method("read_link"))
        .expect("stored link-target query is metadata query");
    assert_eq!(
        row.disposition().classes(),
        &[TerminalAuthorityClass::FilesystemMetadataQuery]
    );

    for release in ["close", "find_close", "close_handle"] {
        assert_eq!(
            filesystem_mechanism_row(mechanism(3, 4), &method(release)),
            Err(UnsettledFilesystemRequirement::OrdinaryReleaseContract),
            "a generic {release} mechanism earns no classification row"
        );
    }
    assert_eq!(
        filesystem_mechanism_row(mechanism(3, 4), &method("definitely_not_close")),
        Err(UnsettledFilesystemRequirement::UnknownRequirement)
    );
}

#[test]
fn emitted_mechanism_rows_enter_policy_and_duplicates_still_reject() {
    let rows = vec![
        filesystem_mechanism_row(mechanism(0, 7), &method("read")).unwrap(),
        filesystem_mechanism_row(mechanism(74, 9), &method("sync")).unwrap(),
        filesystem_mechanism_row(mechanism(89, 8), &method("read_link")).unwrap(),
    ];
    let policy = terminal_authority_policy_with_rows(rows).expect("exact cohort policy");
    assert_eq!(
        policy.classify(mechanism(0, 7)).unwrap().classes(),
        &[TerminalAuthorityClass::FilesystemContentRead]
    );
    assert!(
        policy
            .classify(mechanism(0, 7))
            .unwrap()
            .contains_all(&TerminalAuthorityDisposition::from_classes([]))
    );
    // A substituted contract digest is an unknown mechanism, not a synonym.
    assert!(policy.classify(mechanism(0, 8)).is_err());
    // The unclassified release mechanism stays fail-closed.
    assert!(policy.classify(mechanism(3, 4)).is_err());

    let duplicate = filesystem_mechanism_row(mechanism(0, 7), &method("read")).unwrap();
    assert_eq!(
        terminal_authority_policy_with_rows(vec![
            filesystem_mechanism_row(mechanism(0, 7), &method("read")).unwrap(),
            duplicate,
        ]),
        Err(TerminalAuthorityPolicyBuildError::DuplicateMechanism(
            mechanism(0, 7)
        ))
    );
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

#[test]
fn ordinary_release_contract_binds_exactly_one_syscall_mechanism() {
    let contract = filesystem_ordinary_release_contract([7; 32]);
    // The release domain cannot reproduce its retained occurrence commitment
    // or a differently committed occurrence's contract.
    assert_ne!(contract.checked_argument_contract().as_bytes(), [7; 32]);
    assert_ne!(filesystem_ordinary_release_contract([8; 32]), contract);

    let bound = release_bound_mechanism(3, contract);
    for release in ["close", "find_close", "close_handle"] {
        let row = filesystem_release_mechanism_row(bound, &method(release), contract)
            .expect("the occurrence-bound mechanism earns the explicit empty row");
        assert_eq!(row.mechanism(), bound);
        assert!(row.disposition().is_authority_class_empty());
    }

    // The same syscall number under any other contract is a different,
    // unconstrained key: it cannot inherit the bound contract's row.
    assert_eq!(
        filesystem_release_mechanism_row(mechanism(3, 4), &method("close"), contract),
        Err(UnsettledFilesystemRequirement::OrdinaryReleaseContract)
    );
    // Non-syscall roles carry no release contract coordinate and refuse.
    let port_write: TerminalMechanismIdentity =
        CheckedPhysicalTerminalMechanismIdentity::port_write(target::TargetProfile::LinuxX64, 0x60)
            .into();
    assert_eq!(
        filesystem_release_mechanism_row(port_write, &method("close"), contract),
        Err(UnsettledFilesystemRequirement::OrdinaryReleaseContract)
    );
    // Faceted and unknown requirements never take a release row.
    assert_eq!(
        filesystem_release_mechanism_row(bound, &method("read"), contract),
        Err(UnsettledFilesystemRequirement::SettledFacetCohort)
    );
    assert_eq!(
        filesystem_release_mechanism_row(bound, &method("not_a_requirement"), contract),
        Err(UnsettledFilesystemRequirement::UnknownRequirement)
    );
    // The generic emitter refuses even the bound mechanism: the contract must
    // arrive through the release path, not silently.
    assert_eq!(
        filesystem_mechanism_row(bound, &method("close")),
        Err(UnsettledFilesystemRequirement::OrdinaryReleaseContract)
    );
}

#[test]
fn occurrence_bound_release_row_classifies_in_an_exact_policy() {
    let contract = filesystem_ordinary_release_contract([7; 32]);
    let bound = release_bound_mechanism(3, contract);
    let policy = terminal_authority_policy_with_rows(vec![
        filesystem_mechanism_row(mechanism(0, 7), &method("read")).unwrap(),
        filesystem_release_mechanism_row(bound, &method("close"), contract).unwrap(),
    ])
    .expect("exact cohort policy with the release row");
    assert!(policy.classify(bound).unwrap().is_authority_class_empty());
    // A substituted contract is an unknown mechanism, not a synonym, and an
    // unconstrained close stays fail-closed under the same table.
    assert!(policy.classify(mechanism(3, 4)).is_err());
    assert!(policy.classify(mechanism(3, 0)).is_err());
}
