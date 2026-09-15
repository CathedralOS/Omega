//! Optimizer module role: policy table. Canonical settled `FilesystemHost` cohort dispositions.
//!
//! This module encodes the settled portable filesystem control/lifecycle
//! policy (`wiki/spec/build/permissions.md#portable-filesystem-control-and-lifecycle-authority`)
//! for the canonical `FilesystemHost` raw boundary declared in
//! `source/library/std/filesystem_host.omg`.
//!
//! Requirement names only locate rows inside the canonical checked schema; the
//! emitted permission and classification rows are still keyed by the exact
//! schema commitment, the complete normalized requirement identity, and the
//! exact role-tagged mechanism identity. A name outside the canonical cohort
//! is not a disposition: lookup fails closed. Demand completeness never
//! fabricates a broad union, and an ordinary-release cohort cannot emit a
//! mechanism row without its separately retained occurrence-specific release
//! contract. That contract binds into a direct-syscall mechanism's checked
//! argument-contract coordinate; a normalized foreign mechanism commits its
//! implementation contract to the boundary calling plan instead and has no
//! release coordinate yet.

use effects::{
    CheckedSyscallArgumentContractIdentity, PortableFilesystemAuthorityFacet,
    ServiceTerminalAuthorityPermission, TerminalAuthorityDisposition, TerminalMechanismIdentity,
    provider_plan::{ServiceMethod, ServiceSchema, ServiceSchemaDigest},
};
use sha2::{Digest, Sha256};

use super::TerminalAuthorityPolicyRow;

/// Domain separation inside the checked syscall argument-contract namespace
/// for the occurrence-specific ordinary-release contract.
const ORDINARY_RELEASE_CONTRACT_DOMAIN: &[u8] =
    b"omega.checked-syscall-argument-contract.filesystem-ordinary-release.v1\0";

/// Domain separation for one retained bounded Source native-handle
/// query-release occurrence inside a verified build filesystem replay
/// record. The occurrence commitment binds the verified record's own strong
/// commitment and the occurrence's ordinal position in the retained event
/// sequence, so one record can carry several proved occurrences without
/// letting them share a contract.
const NATIVE_HANDLE_QUERY_RELEASE_OCCURRENCE_DOMAIN: &[u8] =
    b"omega.filesystem-ordinary-release.native-handle-query-occurrence.v1\0";

/// Operation tags of the retained bounded Source native-handle query-release
/// chain in the checked filesystem replay grammar
/// (`FilesystemSourceNativeHandleQueryChainReplayRecord`): constrained
/// `open_path_handle`, admitted `final_path_name_by_handle`/`get_last_error`
/// observations, and the releasing `close_handle`.
const OPEN_PATH_HANDLE_TAG: u16 = 28;
const CLOSE_HANDLE_TAG: u16 = 29;
const FINAL_PATH_NAME_BY_HANDLE_TAG: u16 = 31;
const GET_LAST_ERROR_TAG: u16 = 35;

/// One canonical `FilesystemHost` requirement's settled cohort disposition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemCohortDisposition {
    /// The requirement's selected mechanism exercises exactly this portable
    /// facet set under the settled policy. An empty set is an explicit empty
    /// classification: service reach and exact review identity are retained
    /// while no closed class is exercised.
    Facets(&'static [PortableFilesystemAuthorityFacet]),
    /// Ordinary release narrows to the explicit empty set only under a
    /// separately retained occurrence-specific release contract. A generic
    /// unconstrained mechanism for this requirement earns no classification
    /// row; the proved constrained occurrence uses its own evidence-bound
    /// mechanism key.
    OrdinaryReleaseContract,
}

/// The named requirement has no settled canonical disposition usable for the
/// requested row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnsettledFilesystemRequirement {
    /// The name is not a canonical `FilesystemHost` cohort member. No
    /// disposition is inferred from a readable name.
    UnknownRequirement,
    /// The requirement's settled cohort is a concrete facet disposition, not
    /// the ordinary-release contract. Its mechanism row emits through
    /// `filesystem_mechanism_row` and needs no release contract.
    SettledFacetCohort,
    /// The requirement's only settled classification is the explicit empty
    /// set under a retained occurrence-specific ordinary-release contract.
    /// Consumer permission rows still emit an explicit empty grant; mechanism
    /// classification rows refuse until that contract is established.
    OrdinaryReleaseContract,
}

/// Strong identity of one retained occurrence-specific ordinary-release
/// contract bound into a direct-syscall mechanism key.
///
/// Ordinary-release narrowing requires exact checked constraint evidence in
/// the mechanism key; this identity is the domain-separated commitment the
/// checking stage's retained evidence produces. It stays disjoint from the
/// conservative unconstrained argument contract and every other narrowing
/// derivation, so a constrained mechanism and an unconstrained one never
/// share a key or a classification row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilesystemOrdinaryReleaseContract(CheckedSyscallArgumentContractIdentity);

impl FilesystemOrdinaryReleaseContract {
    /// The checked argument-contract coordinate carried in the bound
    /// mechanism's `Syscall` identity.
    pub const fn checked_argument_contract(self) -> CheckedSyscallArgumentContractIdentity {
        self.0
    }
}

/// Bind one retained occurrence commitment into the ordinary-release contract
/// coordinate of a direct-syscall mechanism.
///
/// `occurrence_commitment` is the checking stage's strong commitment of the
/// retained constrained acquisition/observation/release lifecycle — for the
/// `FilesystemHost` native-handle chain, the checked query-release record's
/// own commitment. Minting this coordinate does not itself establish the
/// occurrence and grants no classification, permission, or admission: the
/// mechanism row still emits only through `filesystem_release_mechanism_row`.
pub fn filesystem_ordinary_release_contract(
    occurrence_commitment: [u8; 32],
) -> FilesystemOrdinaryReleaseContract {
    let mut digest = Sha256::new();
    digest.update(ORDINARY_RELEASE_CONTRACT_DOMAIN);
    digest.update(occurrence_commitment);
    FilesystemOrdinaryReleaseContract(CheckedSyscallArgumentContractIdentity::from_digest(
        digest.finalize().into(),
    ))
}

/// Realize every retained bounded Source native-handle query-release
/// occurrence in one verified build filesystem replay record into its
/// ordinary-release contract, in authored operation order.
///
/// The retained record replays through the verified build-evaluation
/// boundary: rehydration reconstructs each retained event through the
/// checked replay record constructors, so every attempt this routine
/// attributes to an occurrence already proved the exact constrained
/// acquisition/observation/release lifecycle — the constrained
/// `open_path_handle` contract, `Resolved` handle preservation through
/// admitted `final_path_name_by_handle`/`get_last_error` observations, and
/// one successful `close_handle` retiring the identity exactly once.
///
/// Each occurrence's commitment binds the verified record's own strong
/// commitment and the occurrence's ordinal position among the retained
/// query-release occurrences. Minting a contract establishes no
/// classification by itself: the row still emits only through
/// `filesystem_release_mechanism_row` for the direct-syscall mechanism
/// carrying the exact coordinate. Stale or substituted evidence fails
/// closed — a tampered record cannot rehydrate, a different record commits
/// differently, and a mechanism bound to one occurrence cannot inherit
/// another occurrence's contract.
///
/// Attempts that do not form a complete retained occurrence earn no
/// contract: failed acquisition, escapes, invalidating calls, early
/// retirement, deferred deletion, and missing or late release all fail the
/// checked constructors upstream, and a partial sequence is never an
/// occurrence here.
pub fn filesystem_native_handle_query_release_contracts(
    record: &build_evaluation::ReviewOnlyBuildFilesystemReplayRecord,
    limits: build_evaluation::BuildFilesystemReplayRecordLimits,
) -> Result<
    Vec<FilesystemOrdinaryReleaseContract>,
    build_evaluation::BuildFilesystemReplayRecordError,
> {
    let replay =
        build_evaluation::rehydrate_review_only_build_filesystem_replay_record(record, limits)?;
    let attempts = replay.attempts();
    let mut contracts = Vec::new();
    let mut cursor = 0;
    while cursor < attempts.len() {
        if attempts[cursor].operation_tag() != OPEN_PATH_HANDLE_TAG {
            cursor += 1;
            continue;
        }
        cursor += 1;
        let observations_start = cursor;
        while cursor < attempts.len()
            && matches!(
                attempts[cursor].operation_tag(),
                FINAL_PATH_NAME_BY_HANDLE_TAG | GET_LAST_ERROR_TAG
            )
        {
            cursor += 1;
        }
        let complete = cursor < attempts.len()
            && attempts[cursor].operation_tag() == CLOSE_HANDLE_TAG
            && attempts[observations_start..cursor]
                .iter()
                .any(|attempt| attempt.operation_tag() == FINAL_PATH_NAME_BY_HANDLE_TAG);
        if !complete {
            // Successful rehydration already admitted the record's event
            // structure, so an incomplete run is not a retained occurrence.
            continue;
        }
        let mut digest = Sha256::new();
        digest.update(NATIVE_HANDLE_QUERY_RELEASE_OCCURRENCE_DOMAIN);
        digest.update(record.commitment());
        digest.update(
            u64::try_from(contracts.len())
                .expect("bounded occurrence ordinals fit u64")
                .to_le_bytes(),
        );
        let occurrence_commitment: [u8; 32] = digest.finalize().into();
        contracts.push(filesystem_ordinary_release_contract(occurrence_commitment));
        cursor += 1;
    }
    Ok(contracts)
}

/// Look up the settled cohort for one canonical `FilesystemHost` requirement
/// name. The name locates the human-reviewed row; it is never a key in the
/// emitted artifacts.
pub fn settled_filesystem_cohort(name: &str) -> Option<FilesystemCohortDisposition> {
    use FilesystemCohortDisposition::Facets;
    use PortableFilesystemAuthorityFacet as Facet;
    const ALL: &[Facet] = &Facet::ALL;
    const CONTENT_READ: &[Facet] = &[Facet::ContentRead];
    const CONTENT_WRITE: &[Facet] = &[Facet::ContentWrite];
    const METADATA_QUERY: &[Facet] = &[Facet::MetadataQuery];
    const DIRECTORY_ENUMERATION: &[Facet] = &[Facet::DirectoryEnumeration];
    const NAMESPACE_MUTATION: &[Facet] = &[Facet::NamespaceMutation];
    const METADATA_MUTATION: &[Facet] = &[Facet::MetadataMutation];
    const CREATE: &[Facet] = &[
        Facet::ContentWrite,
        Facet::NamespaceMutation,
        Facet::MetadataMutation,
    ];
    const CREATE_DIR: &[Facet] = &[Facet::NamespaceMutation, Facet::MetadataMutation];
    const EMPTY: &[Facet] = &[];
    Some(match name {
        "create" => Facets(CREATE),
        "open" | "open_create" | "open_at" | "open_path_handle" => Facets(ALL),
        "read" | "read_at" => Facets(CONTENT_READ),
        "write" | "write_at" | "set_len" => Facets(CONTENT_WRITE),
        "remove" | "remove_dir" | "unlink_at" | "rename" | "hard_link" | "symlink"
        | "create_hard_link" | "remove_name" | "remove_dir_name" => Facets(NAMESPACE_MUTATION),
        "create_dir" | "create_dir_name" => Facets(CREATE_DIR),
        "set_permissions" | "set_file_permissions" | "set_file_time" | "set_file_times"
        | "change_owner" | "change_owner_no_follow" | "change_file_owner" => {
            Facets(METADATA_MUTATION)
        }
        "canonicalize" | "read_metadata" | "read_file_metadata" | "read_symlink_metadata"
        | "final_path_name_by_handle" | "read_link" => Facets(METADATA_QUERY),
        "read_dir" | "find_first" | "find_next" => Facets(DIRECTORY_ENUMERATION),
        // Durability-only completion of existing changes.
        "sync" | "sync_data"
        // Ordinary locking/unlocking: there is no coordination class.
        | "lock_file" | "lock_file_ex" | "unlock_file"
        // Accepted position, handle-representation, and alias operations.
        | "seek" | "get_osfhandle" | "duplicate"
        // Recognized error-state observation, independent of declaring service.
        | "get_last_error" | "errno" => Facets(EMPTY),
        "close" | "find_close" | "close_handle" => {
            FilesystemCohortDisposition::OrdinaryReleaseContract
        }
        _ => return None,
    })
}

/// Emit the exact consumer permission row for one `FilesystemHost` schema
/// method. `method.name` locates the settled cohort; the emitted row is keyed
/// by the supplied schema commitment and the method's complete normalized
/// requirement identity.
///
/// Ordinary-release cohorts emit an explicit empty permission: the consumer
/// grants reach while permitting no closed class. That empty row is what the
/// proved constrained release occurrence later fits inside; it is not a claim
/// that a generic release mechanism classifies empty.
pub fn filesystem_host_permission_row(
    service_schema: ServiceSchemaDigest,
    method: &ServiceMethod,
) -> Result<ServiceTerminalAuthorityPermission, UnsettledFilesystemRequirement> {
    let facets = match settled_filesystem_cohort(&method.name) {
        Some(FilesystemCohortDisposition::Facets(facets)) => facets,
        Some(FilesystemCohortDisposition::OrdinaryReleaseContract) => &[],
        None => return Err(UnsettledFilesystemRequirement::UnknownRequirement),
    };
    Ok(ServiceTerminalAuthorityPermission::for_filesystem_facets(
        service_schema,
        method.requirement_identity.clone(),
        facets.iter().copied(),
    ))
}

/// Emit the complete consumer permission table for one canonical
/// `FilesystemHost` schema. Every method must resolve to a settled cohort;
/// unknown members fail the whole table rather than emitting a partial one.
pub fn filesystem_host_permission_rows(
    schema: &ServiceSchema,
) -> Result<Vec<ServiceTerminalAuthorityPermission>, UnsettledFilesystemRequirement> {
    let digest = schema.identity_digest();
    schema
        .methods
        .iter()
        .map(|method| filesystem_host_permission_row(digest, method))
        .collect()
}

/// Emit one exact receiving-policy row classifying the mechanism realization
/// admitted for the named `FilesystemHost` requirement. The row is keyed by
/// the exact mechanism identity; `method.name` only selects the settled
/// cohort's disposition.
///
/// Ordinary-release cohorts refuse: an unconstrained generic mechanism cannot
/// inherit the occurrence-specific release proof, and no broad union is
/// fabricated to complete the table.
pub fn filesystem_mechanism_row(
    mechanism: TerminalMechanismIdentity,
    method: &ServiceMethod,
) -> Result<TerminalAuthorityPolicyRow, UnsettledFilesystemRequirement> {
    match settled_filesystem_cohort(&method.name) {
        Some(FilesystemCohortDisposition::Facets(facets)) => Ok(TerminalAuthorityPolicyRow::new(
            mechanism,
            TerminalAuthorityDisposition::from_filesystem_facets(facets.iter().copied()),
        )),
        Some(FilesystemCohortDisposition::OrdinaryReleaseContract) => {
            Err(UnsettledFilesystemRequirement::OrdinaryReleaseContract)
        }
        None => Err(UnsettledFilesystemRequirement::UnknownRequirement),
    }
}

/// Emit the evidence-bound explicit-empty mechanism row for one
/// ordinary-release cohort requirement whose proved constrained occurrence
/// carries `contract` in the mechanism's checked contract coordinate.
///
/// Only a direct-syscall mechanism can carry the contract today: a normalized
/// foreign locator commits its implementation contract to the admitted
/// boundary calling plan and has no release coordinate, so foreign
/// `close`/`find_close`/`close_handle` realizations still refuse rather than
/// fabricating a union. A mechanism carrying any other contract is an
/// unconstrained key and earns no row.
pub fn filesystem_release_mechanism_row(
    mechanism: TerminalMechanismIdentity,
    method: &ServiceMethod,
    contract: FilesystemOrdinaryReleaseContract,
) -> Result<TerminalAuthorityPolicyRow, UnsettledFilesystemRequirement> {
    match settled_filesystem_cohort(&method.name) {
        Some(FilesystemCohortDisposition::OrdinaryReleaseContract) => {
            let TerminalMechanismIdentity::Syscall(syscall) = mechanism else {
                return Err(UnsettledFilesystemRequirement::OrdinaryReleaseContract);
            };
            if syscall.checked_argument_contract() != contract.checked_argument_contract() {
                return Err(UnsettledFilesystemRequirement::OrdinaryReleaseContract);
            }
            Ok(TerminalAuthorityPolicyRow::new(
                mechanism,
                TerminalAuthorityDisposition::from_filesystem_facets([]),
            ))
        }
        Some(FilesystemCohortDisposition::Facets(_)) => {
            Err(UnsettledFilesystemRequirement::SettledFacetCohort)
        }
        None => Err(UnsettledFilesystemRequirement::UnknownRequirement),
    }
}
