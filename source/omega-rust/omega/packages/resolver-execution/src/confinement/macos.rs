use crate::backend::{ResolverExecutionAuthorityRoots, ResolverExecutionBackend};
use crate::model::{
    ResolverExecutionBackendIdentity, ResolverExecutionNetworkTransport, ResolverExecutionPhase,
};
use crate::network::ResolverExecutionEndpointRoutePolicy;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::Command;

pub(crate) const MACOS_SANDBOX_EXECUTABLE: &str = "/usr/bin/sandbox-exec";
const MACOS_NULL_DEVICE: &str = "/dev/null";
const MACOS_DIRECTORY_LOOKUP_SERVICE: &str = "com.apple.system.opendirectoryd.libinfo";
const MACOS_HOSTNAME_SYSCTL: &str = "kern.hostname";
const MACOS_RUST_RUNTIME_PAGE_SIZE_SYSCTL: &str = "hw.pagesize_compat";
const MACOS_TLS_CONFIGURATION_ROOT: &str = "/private/etc/ssl";
const MACOS_TLS_CONFIGURATION_ALIAS_ROOT: &str = "/etc/ssl";
const MACOS_CONFINED_METADATA_PATH_LIMIT: usize = 1024;
const RESOLVER_EXECUTION_CANONICAL_BYTE_LIMIT: usize = 2 * 1024 * 1024;
const EXECUTABLE_BYTE_LIMIT: u64 = 256 * 1024 * 1024;

pub(crate) fn command(
    backend: &ResolverExecutionBackend,
    executable: &Path,
    additional_executables: &[PathBuf],
    phase: ResolverExecutionPhase,
    network_transport: Option<ResolverExecutionNetworkTransport>,
    endpoint_route: Option<&ResolverExecutionEndpointRoutePolicy>,
    roots: ResolverExecutionAuthorityRoots<'_>,
) -> io::Result<(Command, Option<String>)> {
    let ResolverExecutionBackendIdentity::MacosSeatbelt {
        executable: sandbox_executable,
        ..
    } = &backend.identity
    else {
        return Err(io::Error::other(
            "macOS resolver selected a non-Seatbelt backend",
        ));
    };
    let mut profile = "(version 1) (deny default) ".to_owned();
    if phase.permits_descendant_processes() {
        profile.push_str("(allow process-fork) ");
    }
    profile.push_str("(allow signal) ");
    let confines_content_reads = matches!(
        phase,
        ResolverExecutionPhase::RepositoryInitialization
            | ResolverExecutionPhase::RepositoryInspection
    ) || (phase == ResolverExecutionPhase::Fetch
        && network_transport == Some(ResolverExecutionNetworkTransport::Https))
        || (phase == ResolverExecutionPhase::TransportDiscovery
            && network_transport == Some(ResolverExecutionNetworkTransport::Https));
    let confined_metadata = match (phase, network_transport) {
        (ResolverExecutionPhase::RepositoryInitialization, _) => {
            let mutable_root = roots.mutable_root.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "repository initialization requires its compiler-owned mutable root",
                )
            })?;
            Some((
                "MUTABLE_ROOT",
                false,
                Vec::new(),
                macos_confined_metadata_paths(executable, additional_executables, &[mutable_root])?,
            ))
        }
        (ResolverExecutionPhase::RepositoryInspection, _) => {
            let inspection_read_root = roots.inspection_read_root.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "repository inspection requires its compiler-owned read root",
                )
            })?;
            Some((
                "INSPECTION_READ_ROOT",
                false,
                Vec::new(),
                macos_confined_metadata_paths(
                    executable,
                    additional_executables,
                    &[inspection_read_root],
                )?,
            ))
        }
        (
            ResolverExecutionPhase::TransportDiscovery,
            Some(ResolverExecutionNetworkTransport::Https),
        ) => {
            let discovery_read_root = roots.discovery_read_root.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "HTTPS transport discovery requires its compiler-owned read root",
                )
            })?;
            Some((
                "DISCOVERY_READ_ROOT",
                true,
                macos_helper_metadata_roots(additional_executables)?,
                macos_confined_metadata_paths(
                    executable,
                    additional_executables,
                    &[
                        discovery_read_root,
                        Path::new(MACOS_TLS_CONFIGURATION_ROOT),
                        Path::new(MACOS_TLS_CONFIGURATION_ALIAS_ROOT),
                    ],
                )?,
            ))
        }
        (ResolverExecutionPhase::Fetch, Some(ResolverExecutionNetworkTransport::Https)) => {
            let mutable_root = roots.mutable_root.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "HTTPS fetch requires its compiler-owned mutable root",
                )
            })?;
            Some((
                "MUTABLE_ROOT",
                true,
                macos_helper_metadata_roots(additional_executables)?,
                macos_confined_metadata_paths(
                    executable,
                    additional_executables,
                    &[
                        mutable_root,
                        Path::new(MACOS_TLS_CONFIGURATION_ROOT),
                        Path::new(MACOS_TLS_CONFIGURATION_ALIAS_ROOT),
                    ],
                )?,
            ))
        }
        (ResolverExecutionPhase::TransportDiscovery | ResolverExecutionPhase::Fetch, _) => None,
    };
    if confines_content_reads {
        let read_root_parameter = match phase {
            ResolverExecutionPhase::TransportDiscovery => "DISCOVERY_READ_ROOT",
            ResolverExecutionPhase::RepositoryInspection => "INSPECTION_READ_ROOT",
            ResolverExecutionPhase::RepositoryInitialization | ResolverExecutionPhase::Fetch => {
                "MUTABLE_ROOT"
            }
        };
        if let Some((
            metadata_root_parameter,
            includes_tls_root,
            metadata_subpaths,
            metadata_paths,
        )) = &confined_metadata
        {
            profile.push_str("(allow file-read-metadata file-test-existence (subpath (param \"");
            profile.push_str(metadata_root_parameter);
            profile.push_str("\"))");
            if *includes_tls_root {
                profile.push_str(&format!(
                    " (subpath \"{MACOS_TLS_CONFIGURATION_ROOT}\") \
                         (subpath \"{MACOS_TLS_CONFIGURATION_ALIAS_ROOT}\")"
                ));
            }
            for index in 0..metadata_subpaths.len() {
                profile.push_str(&format!(" (subpath (param \"METADATA_SUBPATH_{index}\"))"));
            }
            for index in 0..metadata_paths.len() {
                profile.push_str(&format!(" (literal (param \"METADATA_PATH_{index}\"))"));
            }
            profile.push_str(") ");
        } else {
            profile.push_str("(allow file-read-metadata) ");
        }
        profile.push_str("(allow file-read-data (subpath (param \"");
        profile.push_str(read_root_parameter);
        profile.push_str("\")) (literal (param \"EXECUTABLE_0\"))");
        for index in 0..additional_executables.len() {
            profile.push_str(&format!(" (literal (param \"EXECUTABLE_{}\"))", index + 1));
        }
        if matches!(
            phase,
            ResolverExecutionPhase::TransportDiscovery | ResolverExecutionPhase::Fetch
        ) && network_transport == Some(ResolverExecutionNetworkTransport::Https)
        {
            profile.push_str(&format!(" (subpath \"{MACOS_TLS_CONFIGURATION_ROOT}\")"));
        }
        profile.push_str(&format!(
            " (literal \"{}\") (literal \"{MACOS_NULL_DEVICE}\")) ",
            std::path::MAIN_SEPARATOR
        ));
    } else {
        profile.push_str("(allow file-read*) ");
    }
    profile.push_str(&format!(
        "(allow file-test-existence file-write-data (literal \"{MACOS_NULL_DEVICE}\")) \
             (allow process-exec (literal (param \"EXECUTABLE_0\"))"
    ));
    for index in 0..additional_executables.len() {
        profile.push_str(&format!(" (literal (param \"EXECUTABLE_{}\"))", index + 1));
    }
    profile.push(')');
    if endpoint_route.is_some() {
        profile.push_str(" (allow network-outbound (remote tcp (param \"BROKER_ENDPOINT\")))");
    }
    if network_transport == Some(ResolverExecutionNetworkTransport::Ssh) {
        profile.push_str(&format!(
            " (allow mach-lookup (global-name \"{MACOS_DIRECTORY_LOOKUP_SERVICE}\")) \
                 (allow sysctl-read (sysctl-name \"{MACOS_HOSTNAME_SYSCTL}\")) \
                 (allow sysctl-read (sysctl-name \"{MACOS_RUST_RUNTIME_PAGE_SIZE_SYSCTL}\"))"
        ));
    }
    if phase.requires_mutable_root() {
        profile.push_str(" (allow file-write* (subpath (param \"MUTABLE_ROOT\")))");
    }

    let mut command = Command::new(sandbox_executable);
    command
        .arg("-D")
        .arg(definition_argument("EXECUTABLE_0", executable));
    for (index, helper) in additional_executables.iter().enumerate() {
        command.arg("-D").arg(definition_argument(
            &format!("EXECUTABLE_{}", index + 1),
            helper,
        ));
    }
    if let Some(root) = roots.mutable_root {
        command
            .arg("-D")
            .arg(definition_argument("MUTABLE_ROOT", root));
    }
    if let Some(root) = roots.inspection_read_root {
        command
            .arg("-D")
            .arg(definition_argument("INSPECTION_READ_ROOT", root));
    }
    if let Some(root) = roots.discovery_read_root {
        command
            .arg("-D")
            .arg(definition_argument("DISCOVERY_READ_ROOT", root));
    }
    if let Some((_, _, metadata_subpaths, metadata_paths)) = &confined_metadata {
        for (index, path) in metadata_subpaths.iter().enumerate() {
            command.arg("-D").arg(definition_argument(
                &format!("METADATA_SUBPATH_{index}"),
                path,
            ));
        }
        for (index, path) in metadata_paths.iter().enumerate() {
            command
                .arg("-D")
                .arg(definition_argument(&format!("METADATA_PATH_{index}"), path));
        }
    }
    if let Some(route) = endpoint_route {
        command.arg("-D").arg(format!(
            "BROKER_ENDPOINT=localhost:{}",
            route.broker_endpoint().port()
        ));
    }
    let profile_sha256 = format_sha256(Sha256::digest(profile.as_bytes()).as_slice());
    command.arg("-p").arg(profile).arg(executable);
    Ok((command, Some(profile_sha256)))
}

#[cfg(target_os = "macos")]
pub(crate) fn macos_helper_metadata_roots(
    additional_executables: &[PathBuf],
) -> io::Result<Vec<PathBuf>> {
    let mut roots = BTreeSet::new();
    for executable in additional_executables {
        let parent = executable.parent().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "resolver helper executable has no metadata root",
            )
        })?;
        if parent == Path::new(std::path::MAIN_SEPARATOR_STR) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "resolver helper metadata root cannot be the filesystem root",
            ));
        }
        roots.insert(parent.to_path_buf());
    }
    bounded_macos_metadata_paths(roots, "resolver helper metadata roots")
}

#[cfg(target_os = "macos")]
pub(crate) fn macos_confined_metadata_paths(
    executable: &Path,
    additional_executables: &[PathBuf],
    confined_read_roots: &[&Path],
) -> io::Result<Vec<PathBuf>> {
    let mut paths = BTreeSet::new();
    for path in confined_read_roots
        .iter()
        .copied()
        .chain(std::iter::once(executable))
        .chain(additional_executables.iter().map(PathBuf::as_path))
        .chain(std::iter::once(Path::new(MACOS_NULL_DEVICE)))
    {
        for ancestor in path.ancestors() {
            paths.insert(ancestor.to_path_buf());
        }
    }
    bounded_macos_metadata_paths(paths, "resolver confined metadata paths")
}

#[cfg(target_os = "macos")]
fn bounded_macos_metadata_paths(paths: BTreeSet<PathBuf>, name: &str) -> io::Result<Vec<PathBuf>> {
    if paths.len() > MACOS_CONFINED_METADATA_PATH_LIMIT {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} exceed compiler count limit"),
        ));
    }
    let encoded_bytes = paths.iter().try_fold(0_usize, |total, path| {
        total.checked_add(path.as_os_str().as_encoded_bytes().len())
    });
    if !matches!(encoded_bytes, Some(total) if total <= RESOLVER_EXECUTION_CANONICAL_BYTE_LIMIT) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} exceed compiler byte limit"),
        ));
    }
    Ok(paths.into_iter().collect())
}

#[cfg(target_os = "macos")]
fn definition_argument(name: &str, value: &Path) -> OsString {
    let mut argument = OsString::from(name);
    argument.push("=");
    argument.push(value.as_os_str());
    argument
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExecutableMetadataIdentity {
    length: u64,
    device: u64,
    inode: u64,
    mode: u32,
}

#[cfg(target_os = "macos")]
pub(crate) fn executable_metadata_identity(path: &Path) -> io::Result<ExecutableMetadataIdentity> {
    use std::os::unix::fs::MetadataExt;

    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "native resolver backend is not a concrete regular file",
        ));
    }
    Ok(ExecutableMetadataIdentity {
        length: metadata.len(),
        device: metadata.dev(),
        inode: metadata.ino(),
        mode: metadata.mode(),
    })
}

#[cfg(target_os = "macos")]
pub(crate) fn verify_owned_native_executable(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt;

    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.uid() != 0
        || metadata.mode() & 0o022 != 0
        || metadata.mode() & 0o6000 != 0
        || metadata.mode() & 0o111 == 0
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "native resolver backend lacks root-owned executable custody",
        ));
    }
    let executable = File::open(path)?;
    if omega_platform_custody::open_file_extended_acl_has_allow_entry(&executable)? {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "native resolver backend has an extended ACL allow entry",
        ));
    }
    for ancestor in path
        .parent()
        .ok_or_else(|| io::Error::other("native resolver backend has no parent"))?
        .ancestors()
    {
        let metadata = std::fs::symlink_metadata(ancestor)?;
        if metadata.file_type().is_symlink()
            || !metadata.is_dir()
            || metadata.uid() != 0
            || metadata.mode() & 0o022 != 0 && metadata.mode() & 0o1000 == 0
            || omega_platform_custody::extended_acl_has_allow_entry(
                ancestor,
                omega_platform_custody::SymbolicLinkBehavior::Follow,
            )?
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "native resolver backend ancestry lacks root-owned custody",
            ));
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn hash_executable(path: &Path) -> io::Result<String> {
    let metadata = std::fs::metadata(path)?;
    if metadata.len() > EXECUTABLE_BYTE_LIMIT {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "native resolver backend exceeds its executable byte ceiling",
        ));
    }
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut observed = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        observed = observed
            .checked_add(u64::try_from(count).unwrap_or(u64::MAX))
            .ok_or_else(|| io::Error::other("native resolver backend length overflowed"))?;
        if observed > EXECUTABLE_BYTE_LIMIT {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "native resolver backend exceeds its executable byte ceiling",
            ));
        }
        hasher.update(&buffer[..count]);
    }
    if observed != metadata.len() {
        return Err(io::Error::other(
            "native resolver backend changed while hashing",
        ));
    }
    Ok(format_sha256(&hasher.finalize()))
}

#[cfg(target_os = "macos")]
fn format_sha256(digest: &[u8]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests;
