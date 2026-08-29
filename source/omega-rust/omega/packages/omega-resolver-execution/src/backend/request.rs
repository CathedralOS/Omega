use super::ResolverExecutionAuthorityRoots;
use crate::model::{ResolverExecutionNetworkTransport, ResolverExecutionPhase};
use crate::network::ResolverExecutionEndpointRoute;
use crate::request::{
    require_absolute, require_canonical_bounded_path, require_regular_file,
    RESOLVER_EXECUTION_ADDITIONAL_EXECUTABLE_LIMIT,
};
use std::io;
use std::path::{Path, PathBuf};

pub(super) fn validate_launch_request(
    executable: &Path,
    additional_executables: &[PathBuf],
    phase: ResolverExecutionPhase,
    network_transport: Option<ResolverExecutionNetworkTransport>,
    endpoint_route: Option<&ResolverExecutionEndpointRoute>,
    roots: ResolverExecutionAuthorityRoots<'_>,
) -> io::Result<Vec<PathBuf>> {
    require_absolute(executable, "resolver executable")?;
    require_regular_file(executable, "resolver executable")?;
    if additional_executables.len() > RESOLVER_EXECUTION_ADDITIONAL_EXECUTABLE_LIMIT {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "resolver helper executable set exceeds its fixed limit",
        ));
    }
    require_canonical_bounded_path(executable, "resolver executable")?;
    for helper in additional_executables {
        require_absolute(helper, "resolver helper executable")?;
        require_canonical_bounded_path(helper, "resolver helper executable")?;
        require_regular_file(helper, "resolver helper executable")?;
    }
    let mut additional_executables = additional_executables.to_vec();
    additional_executables.retain(|helper| helper != executable);
    additional_executables.sort();
    additional_executables.dedup();
    match (phase.permits_network(), network_transport, endpoint_route) {
        (true, Some(_), Some(_)) | (false, None, None) => {}
        (true, None, _) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "networked resolver phase has no closed transport authority",
            ));
        }
        (true, Some(_), None) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "networked resolver phase has no endpoint route",
            ));
        }
        (false, Some(_), _) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "nonnetwork resolver phase received transport authority",
            ));
        }
        (false, None, Some(_)) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "nonnetwork resolver phase received an endpoint route",
            ));
        }
    }
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

    Ok(additional_executables)
}
