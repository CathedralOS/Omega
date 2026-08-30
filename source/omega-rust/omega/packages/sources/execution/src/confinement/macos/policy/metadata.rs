use super::{
    MACOS_CONFINED_METADATA_PATH_LIMIT, MACOS_NULL_DEVICE, MACOS_TLS_CONFIGURATION_ALIAS_ROOT,
    MACOS_TLS_CONFIGURATION_ROOT,
};
use crate::backend::ResolverExecutionAuthorityRoots;
use crate::model::{ResolverExecutionNetworkTransport, ResolverExecutionPhase};
use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};

const RESOLVER_EXECUTION_CANONICAL_BYTE_LIMIT: usize = 2 * 1024 * 1024;

pub(super) struct ConfinedMetadata {
    pub(super) root_parameter: &'static str,
    pub(super) includes_tls_root: bool,
    pub(super) subpaths: Vec<PathBuf>,
    pub(super) paths: Vec<PathBuf>,
}

pub(super) fn confined_metadata(
    executable: &Path,
    additional_executables: &[PathBuf],
    phase: ResolverExecutionPhase,
    network_transport: Option<ResolverExecutionNetworkTransport>,
    roots: ResolverExecutionAuthorityRoots<'_>,
) -> io::Result<Option<ConfinedMetadata>> {
    let metadata = match (phase, network_transport) {
        (ResolverExecutionPhase::RepositoryInitialization, _) => {
            let mutable_root = roots.mutable_root.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "repository initialization requires its compiler-owned mutable root",
                )
            })?;
            ConfinedMetadata {
                root_parameter: "MUTABLE_ROOT",
                includes_tls_root: false,
                subpaths: Vec::new(),
                paths: macos_confined_metadata_paths(
                    executable,
                    additional_executables,
                    &[mutable_root],
                )?,
            }
        }
        (ResolverExecutionPhase::RepositoryInspection, _) => {
            let inspection_read_root = roots.inspection_read_root.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "repository inspection requires its compiler-owned read root",
                )
            })?;
            ConfinedMetadata {
                root_parameter: "INSPECTION_READ_ROOT",
                includes_tls_root: false,
                subpaths: Vec::new(),
                paths: macos_confined_metadata_paths(
                    executable,
                    additional_executables,
                    &[inspection_read_root],
                )?,
            }
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
            ConfinedMetadata {
                root_parameter: "DISCOVERY_READ_ROOT",
                includes_tls_root: true,
                subpaths: macos_helper_metadata_roots(additional_executables)?,
                paths: macos_confined_metadata_paths(
                    executable,
                    additional_executables,
                    &[
                        discovery_read_root,
                        Path::new(MACOS_TLS_CONFIGURATION_ROOT),
                        Path::new(MACOS_TLS_CONFIGURATION_ALIAS_ROOT),
                    ],
                )?,
            }
        }
        (ResolverExecutionPhase::Fetch, Some(ResolverExecutionNetworkTransport::Https)) => {
            let mutable_root = roots.mutable_root.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "HTTPS fetch requires its compiler-owned mutable root",
                )
            })?;
            ConfinedMetadata {
                root_parameter: "MUTABLE_ROOT",
                includes_tls_root: true,
                subpaths: macos_helper_metadata_roots(additional_executables)?,
                paths: macos_confined_metadata_paths(
                    executable,
                    additional_executables,
                    &[
                        mutable_root,
                        Path::new(MACOS_TLS_CONFIGURATION_ROOT),
                        Path::new(MACOS_TLS_CONFIGURATION_ALIAS_ROOT),
                    ],
                )?,
            }
        }
        (ResolverExecutionPhase::TransportDiscovery | ResolverExecutionPhase::Fetch, _) => {
            return Ok(None);
        }
    };
    Ok(Some(metadata))
}

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
