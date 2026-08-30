//! Construction and canonical encoding of the closed Seatbelt policy.

mod definitions;
mod profile;

use crate::backend::ResolverExecutionAuthorityRoots;
use crate::model::{ResolverExecutionNetworkTransport, ResolverExecutionPhase};
use crate::network::ResolverExecutionEndpointRoutePolicy;
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

pub(super) const MACOS_NULL_DEVICE: &str = "/dev/null";

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
        let encoded = profile::encode(
            additional_executables,
            phase,
            network_transport,
            endpoint_route.is_some(),
        );
        let definitions =
            definitions::encode(executable, additional_executables, endpoint_route, roots);
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
