// A service permission binds the package's checked schema and requirement.
// The receiver classifies the selected native operation separately.

#[cfg(test)]
#[path = "host_cohorts/tests.rs"]
mod tests;

use build_providers::acceptance::ServiceTerminalAuthorityPermission;
use semantic_vocabulary::{
    PortableFilesystemAuthorityFacet,
    TerminalAuthorityDisposition,
};
use terminal_authority_receipts::{NormalizedForeignArgumentContract, TerminalMechanismIdentity};
use build_providers::plans::{ServiceMethod, ServiceSchema, ServiceSchemaDigest};

use super::TerminalAuthorityPolicyRow;

const DARWIN_CLOCK_TRAIT: &str = "DarwinClock";
const LIBSYSTEM_INSTALL_NAME: &[u8] = b"/usr/lib/libSystem.B.dylib";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnsettledFilesystemRequirement {
    UnknownRequirement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnsettledTimeHostRequirement {
    UnknownRequirement,
}

fn permitted_filesystem_facets(name: &str) -> Option<&'static [PortableFilesystemAuthorityFacet]> {
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
        "create" => CREATE,
        "open" | "open_create" | "open_at" | "open_path_handle" => ALL,
        "read" | "read_at" => CONTENT_READ,
        "write" | "write_at" | "set_len" => CONTENT_WRITE,
        "remove" | "remove_dir" | "unlink_at" | "rename" | "hard_link" | "symlink"
        | "create_hard_link" | "remove_name" | "remove_dir_name" => NAMESPACE_MUTATION,
        "create_dir" | "create_dir_name" => CREATE_DIR,
        "set_permissions"
        | "set_file_permissions"
        | "set_file_time"
        | "set_file_times"
        | "change_owner"
        | "change_owner_no_follow"
        | "change_file_owner" => METADATA_MUTATION,
        "canonicalize"
        | "read_metadata"
        | "read_file_metadata"
        | "read_symlink_metadata"
        | "final_path_name_by_handle"
        | "read_link" => METADATA_QUERY,
        "read_dir" | "find_first" | "find_next" => DIRECTORY_ENUMERATION,
        "sync" | "sync_data" | "lock_file" | "lock_file_ex" | "unlock_file" | "seek"
        | "get_osfhandle" | "duplicate" | "get_last_error" | "errno" => EMPTY,
        "close" | "find_close" | "close_handle" => EMPTY,
        _ => return None,
    })
}

pub(crate) fn filesystem_host_permission_row(
    schema: &ServiceSchema,
    service_schema: ServiceSchemaDigest,
    method: &ServiceMethod,
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
) -> Result<ServiceTerminalAuthorityPermission, UnsettledFilesystemRequirement> {
    let facets = permitted_filesystem_facets(&method.name)
        .ok_or(UnsettledFilesystemRequirement::UnknownRequirement)?;
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

fn settled_time_host_cohort(name: &str) -> bool {
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
        _ => None,
    }
}

pub(super) fn darwin_clock_locator(symbol: &[u8]) -> Option<target::ForeignLocatorIdentityDigest> {
    let locator = target::normalize_foreign_locator(
        target::ForeignLocatorCandidate::MachODylibSymbol {
            install_name: LIBSYSTEM_INSTALL_NAME.to_vec(),
            symbol: symbol.to_vec(),
        },
        target::TargetProfile::MacosArm64,
    ).ok()?;
    Some(locator.identity_digest())
}

fn is_reviewed_darwin_clock_import(mechanism: TerminalMechanismIdentity, symbol: &[u8]) -> bool {
    let TerminalMechanismIdentity::NormalizedForeign(foreign) = mechanism else {
        return false;
    };
    foreign.target() == target::TargetProfile::MacosArm64
        && foreign.argument_contract() == NormalizedForeignArgumentContract::AdmittedPlan
        && darwin_clock_locator(symbol) == Some(foreign.locator_identity())
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
