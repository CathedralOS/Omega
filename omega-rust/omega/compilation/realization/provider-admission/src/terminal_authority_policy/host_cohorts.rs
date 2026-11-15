/*
Canonical settled host-cohort dispositions for `FilesystemHost`, `TimeHost`,
`Console`, `ProcessExit`, and `DarwinClock`.

Requirement names only locate rows inside the canonical checked schema; emitted
permission and classification rows remain keyed by the exact schema commitment,
the complete normalized requirement identity, and the exact role-tagged
mechanism identity. Names outside a canonical cohort fail closed, and
ordinary-release filesystem requirements (`close`, `find_close`, `close_handle`)
refuse unconstrained mechanism rows until an occurrence-specific release
contract is established.
*/

use effects::{
    NormalizedForeignArgumentContract, PortableFilesystemAuthorityFacet,
    ServiceTerminalAuthorityPermission, TerminalAuthorityClass, TerminalAuthorityDisposition,
    TerminalMechanismIdentity,
    provider_plan::{ServiceMethod, ServiceSchema, ServiceSchemaDigest},
};

use super::TerminalAuthorityPolicyRow;

const DARWIN_CLOCK_TRAIT: &str = "DarwinClock";
const LIBSYSTEM_INSTALL_NAME: &[u8] = b"/usr/lib/libSystem.B.dylib";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnsettledFilesystemRequirement {
    UnknownRequirement,
    SettledFacetCohort,
    OrdinaryReleaseContract,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FilesystemCohortDisposition {
    Facets(&'static [PortableFilesystemAuthorityFacet]),
    OrdinaryReleaseContract,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnsettledTimeHostRequirement {
    UnknownRequirement,
}

pub(crate) fn settled_filesystem_cohort(name: &str) -> Option<FilesystemCohortDisposition> {
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
        "set_permissions"
        | "set_file_permissions"
        | "set_file_time"
        | "set_file_times"
        | "change_owner"
        | "change_owner_no_follow"
        | "change_file_owner" => Facets(METADATA_MUTATION),
        "canonicalize"
        | "read_metadata"
        | "read_file_metadata"
        | "read_symlink_metadata"
        | "final_path_name_by_handle"
        | "read_link" => Facets(METADATA_QUERY),
        "read_dir" | "find_first" | "find_next" => Facets(DIRECTORY_ENUMERATION),
        "sync" | "sync_data" | "lock_file" | "lock_file_ex" | "unlock_file" | "seek"
        | "get_osfhandle" | "duplicate" | "get_last_error" | "errno" => Facets(EMPTY),
        "close" | "find_close" | "close_handle" => {
            FilesystemCohortDisposition::OrdinaryReleaseContract
        }
        _ => return None,
    })
}

pub(crate) fn filesystem_host_permission_row(
    schema: &ServiceSchema,
    service_schema: ServiceSchemaDigest,
    method: &ServiceMethod,
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
) -> Result<ServiceTerminalAuthorityPermission, UnsettledFilesystemRequirement> {
    let facets = match settled_filesystem_cohort(&method.name) {
        Some(FilesystemCohortDisposition::Facets(facets)) => facets,
        Some(FilesystemCohortDisposition::OrdinaryReleaseContract) => &[],
        None => return Err(UnsettledFilesystemRequirement::UnknownRequirement),
    };
    Ok(ServiceTerminalAuthorityPermission::for_filesystem_facets(
        service_schema,
        identities.intern(schema.identity_text(method.requirement_identity)),
        facets.iter().copied(),
    ))
}

pub fn filesystem_host_permission_rows(
    schema: &ServiceSchema,
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
) -> Result<Vec<ServiceTerminalAuthorityPermission>, UnsettledFilesystemRequirement> {
    let digest = schema.identity_digest();
    schema
        .declaration
        .methods
        .iter()
        .map(|method| filesystem_host_permission_row(schema, digest, method, identities))
        .collect()
}

pub(crate) fn filesystem_mechanism_row(
    mechanism: TerminalMechanismIdentity,
    method: &ServiceMethod,
) -> Option<TerminalAuthorityPolicyRow> {
    let FilesystemCohortDisposition::Facets(facets) = settled_filesystem_cohort(&method.name)?
    else {
        return None;
    };
    Some(TerminalAuthorityPolicyRow::new(
        mechanism,
        TerminalAuthorityDisposition::from_filesystem_facets(facets.iter().copied()),
    ))
}

pub(crate) fn settled_time_host_cohort(name: &str) -> bool {
    matches!(
        name,
        "monotonic_ticks"
            | "monotonic_ticks_per_second"
            | "wall_clock_raw"
            | "wall_clock_units_per_second"
            | "wall_clock_epoch_offset_seconds"
            | "sleep"
    )
}

pub(crate) fn time_host_permission_row(
    schema: &ServiceSchema,
    service_schema: ServiceSchemaDigest,
    method: &ServiceMethod,
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
) -> Result<ServiceTerminalAuthorityPermission, UnsettledTimeHostRequirement> {
    if !settled_time_host_cohort(&method.name) {
        return Err(UnsettledTimeHostRequirement::UnknownRequirement);
    }
    Ok(ServiceTerminalAuthorityPermission::new(
        service_schema,
        identities.intern(schema.identity_text(method.requirement_identity)),
        TerminalAuthorityDisposition::from_classes([]),
    ))
}

pub fn time_host_permission_rows(
    schema: &ServiceSchema,
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
) -> Result<Vec<ServiceTerminalAuthorityPermission>, UnsettledTimeHostRequirement> {
    let digest = schema.identity_digest();
    schema
        .declaration
        .methods
        .iter()
        .map(|method| time_host_permission_row(schema, digest, method, identities))
        .collect()
}

pub(crate) fn time_host_mechanism_row(
    mechanism: TerminalMechanismIdentity,
    method: &ServiceMethod,
) -> Option<TerminalAuthorityPolicyRow> {
    settled_time_host_cohort(&method.name).then(|| {
        TerminalAuthorityPolicyRow::new(mechanism, TerminalAuthorityDisposition::from_classes([]))
    })
}

pub(crate) fn settled_console_cohort(name: &str) -> Option<TerminalAuthorityClass> {
    match name {
        "exit_process" => Some(TerminalAuthorityClass::ProcessTermination),
        "write" | "write_byte" | "write_line" => Some(TerminalAuthorityClass::ProcessOutput),
        "read_byte" | "read_line" => Some(TerminalAuthorityClass::ProcessInput),
        _ => None,
    }
}

pub(crate) fn console_mechanism_row(
    mechanism: TerminalMechanismIdentity,
    method: &ServiceMethod,
) -> Option<TerminalAuthorityPolicyRow> {
    let class = settled_console_cohort(&method.name)?;
    Some(TerminalAuthorityPolicyRow::new(
        mechanism,
        TerminalAuthorityDisposition::from_classes([class]),
    ))
}

fn reviewed_darwin_clock_symbol(
    schema: &ServiceSchema,
    method: &ServiceMethod,
) -> Option<&'static [u8]> {
    if schema.identity_text(schema.declaration.trait_name) != DARWIN_CLOCK_TRAIT
        || method.requirement_owner.rsplit("::").next() != Some(DARWIN_CLOCK_TRAIT)
    {
        return None;
    }
    match method.name.as_str() {
        "clock_ns" => Some(b"_clock_gettime_nsec_np"),
        "mach_absolute_time" => Some(b"_mach_absolute_time"),
        "poll" => Some(b"_poll"),
        _ => None,
    }
}

fn is_reviewed_darwin_clock_import(mechanism: TerminalMechanismIdentity, symbol: &[u8]) -> bool {
    let TerminalMechanismIdentity::NormalizedForeign(foreign) = mechanism else {
        return false;
    };
    let Ok(reviewed) = target::normalize_foreign_locator(
        target::ForeignLocatorCandidate::MachODylibSymbol {
            install_name: LIBSYSTEM_INSTALL_NAME.to_vec(),
            symbol: symbol.to_vec(),
        },
        target::TargetProfile::MacosArm64,
    ) else {
        return false;
    };
    foreign.target() == reviewed.target()
        && foreign.locator_identity() == reviewed.identity_digest()
        && foreign.argument_contract() == NormalizedForeignArgumentContract::AdmittedPlan
}

pub(crate) fn darwin_clock_mechanism_row(
    mechanism: TerminalMechanismIdentity,
    schema: &ServiceSchema,
    method: &ServiceMethod,
) -> Option<TerminalAuthorityPolicyRow> {
    let symbol = reviewed_darwin_clock_symbol(schema, method)?;
    is_reviewed_darwin_clock_import(mechanism, symbol).then(|| {
        TerminalAuthorityPolicyRow::new(mechanism, TerminalAuthorityDisposition::from_classes([]))
    })
}
