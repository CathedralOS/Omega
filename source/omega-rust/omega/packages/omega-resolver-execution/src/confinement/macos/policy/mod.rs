//! Construction and canonical encoding of the closed Seatbelt policy.

mod definitions;
mod metadata;
mod profile;

use crate::backend::ResolverExecutionAuthorityRoots;
use crate::model::{ResolverExecutionNetworkTransport, ResolverExecutionPhase};
use crate::network::ResolverExecutionEndpointRoutePolicy;
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

pub(super) use metadata::{macos_confined_metadata_paths, macos_helper_metadata_roots};

pub(super) const MACOS_CONFINED_METADATA_PATH_LIMIT: usize = 1024;
pub(super) const MACOS_NULL_DEVICE: &str = "/dev/null";
pub(super) const MACOS_TLS_CONFIGURATION_ROOT: &str = "/private/etc/ssl";
pub(super) const MACOS_TLS_CONFIGURATION_ALIAS_ROOT: &str = "/etc/ssl";

pub(super) struct SeatbeltPolicy {
    encoded: String,
    definitions: Vec<OsString>,
}

impl SeatbeltPolicy {
    pub(super) fn construct(
        executable: &Path,
        additional_executables: &[PathBuf],
        phase: ResolverExecutionPhase,
        network_transport: Option<ResolverExecutionNetworkTransport>,
        endpoint_route: Option<&ResolverExecutionEndpointRoutePolicy>,
        roots: ResolverExecutionAuthorityRoots<'_>,
    ) -> io::Result<Self> {
        let confined_metadata = metadata::confined_metadata(
            executable,
            additional_executables,
            phase,
            network_transport,
            roots,
        )?;
        let encoded = profile::encode(
            additional_executables,
            phase,
            network_transport,
            endpoint_route.is_some(),
            confined_metadata.as_ref(),
        );
        let definitions = definitions::encode(
            executable,
            additional_executables,
            endpoint_route,
            roots,
            confined_metadata.as_ref(),
        );
        Ok(Self {
            encoded,
            definitions,
        })
    }

    pub(super) fn encoded(&self) -> &str {
        &self.encoded
    }

    pub(super) fn definitions(&self) -> &[OsString] {
        &self.definitions
    }

    pub(super) fn sha256(&self) -> String {
        Sha256::digest(self.encoded.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
}
