//! Construction and canonical encoding of the closed Seatbelt policy.

mod definitions;
mod profile;

use crate::backend::ResolverExecutionAuthorityRoots;
use crate::model::ResolverExecutionPhase;
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::io;
use std::path::Path;

pub(super) const MACOS_NULL_DEVICE: &str = "/dev/null";

pub(super) struct SeatbeltPolicy {
    encoded: String,
    definitions: Vec<OsString>,
}

impl SeatbeltPolicy {
    pub(super) fn construct(
        executable: &Path,
        phase: ResolverExecutionPhase,
        roots: ResolverExecutionAuthorityRoots<'_>,
    ) -> io::Result<Self> {
        if phase.permits_network() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "host-routed resolver phase cannot receive a Seatbelt policy",
            ));
        }
        let encoded = profile::encode(phase);
        let definitions = definitions::encode(executable, roots);
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
