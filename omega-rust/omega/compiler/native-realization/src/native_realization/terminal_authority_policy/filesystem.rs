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
//! contract.

use effects::{
    PortableFilesystemAuthorityFacet, ServiceTerminalAuthorityPermission,
    TerminalAuthorityDisposition, TerminalMechanismIdentity,
    provider_plan::{ServiceMethod, ServiceSchema, ServiceSchemaDigest},
};

use super::TerminalAuthorityPolicyRow;

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
    /// The requirement's only settled classification is the explicit empty
    /// set under a retained occurrence-specific ordinary-release contract.
    /// Consumer permission rows still emit an explicit empty grant; mechanism
    /// classification rows refuse until that contract is established.
    OrdinaryReleaseContract,
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
