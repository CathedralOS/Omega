//! Proof-pair preparation and companion naming for requested products.
//!
//! Prepare and validate requested pairs before staging. Stage every file before
//! `platform_custody::file_publication` makes destinations visible and replays
//! installed bytes. That owner also handles platform replacement and cleanup.
//! `terminal_codec` owns Psi pair preparation; `psi_pair` selects paths and stages it.
//! Native receipt identities and proof validation belong to `native_artifact`.

mod psi_pair;

pub(crate) use psi_pair::{psi_pair_receipt, stage_psi_pair};

use std::path::PathBuf;

/// Remove one companion this publication is not refreshing. The publication
/// contract forbids associating a stale sidecar with newly written bytes, so
/// an unrequested artifact/`.proof` pair cannot survive beside the artifact
/// it no longer commits to. Absence is the expected common case; every other
/// removal failure aborts publication before any new bytes are installed.
pub(crate) fn remove_stale_companion(path: &std::path::Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "failed to remove stale proof companion {}: {error}",
            path.display()
        )),
    }
}

/// The adjacent `.proof`/`.psi` companion path beside `path`, formed by
/// appending `suffix` to the complete artifact filename per the contract.
pub(crate) fn appended_file_name_path(path: &std::path::Path, suffix: &str) -> PathBuf {
    let mut name = path
        .file_name()
        .expect("a publication path always has a file name")
        .to_os_string();
    name.push(suffix);
    path.with_file_name(name)
}
