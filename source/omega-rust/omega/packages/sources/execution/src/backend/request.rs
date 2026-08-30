use super::ResolverExecutionAuthorityRoots;
use crate::model::ResolverExecutionPhase;
use crate::request::{require_absolute, require_canonical_bounded_path, require_regular_file};
use std::io;
use std::path::Path;

pub(super) fn validate_launch_request(
    executable: &Path,
    phase: ResolverExecutionPhase,
    roots: ResolverExecutionAuthorityRoots<'_>,
) -> io::Result<()> {
    require_absolute(executable, "resolver executable")?;
    require_regular_file(executable, "resolver executable")?;
    require_canonical_bounded_path(executable, "resolver executable")?;

    match (phase.requires_mutable_root(), roots.mutable_root) {
        (true, Some(root)) => {
            require_absolute(root, "resolver mutable root")?;
            require_canonical_bounded_path(root, "resolver mutable root")?;
        }
        (true, None) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "mutating resolver phase has no mutable root",
            ));
        }
        (false, Some(_)) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "read-only resolver phase received a mutable root",
            ));
        }
        (false, None) => {}
    }
    match (phase, roots.inspection_read_root) {
        (ResolverExecutionPhase::RepositoryInspection, Some(root)) => {
            require_absolute(root, "resolver inspection read root")?;
            require_canonical_bounded_path(root, "resolver inspection read root")?;
        }
        (ResolverExecutionPhase::RepositoryInspection, None) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "repository inspection has no read root",
            ));
        }
        (_, Some(_)) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "noninspection resolver phase received an inspection read root",
            ));
        }
        (_, None) => {}
    }
    match (phase, roots.discovery_read_root) {
        (ResolverExecutionPhase::TransportDiscovery, Some(root)) => {
            require_absolute(root, "resolver discovery read root")?;
            require_canonical_bounded_path(root, "resolver discovery read root")?;
        }
        (ResolverExecutionPhase::TransportDiscovery, None) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "transport discovery has no read root",
            ));
        }
        (_, Some(_)) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "nondiscovery resolver phase received a discovery read root",
            ));
        }
        (_, None) => {}
    }

    Ok(())
}
