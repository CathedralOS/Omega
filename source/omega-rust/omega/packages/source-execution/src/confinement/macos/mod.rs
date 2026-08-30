//! macOS Seatbelt policy construction and native realization.

mod command;
mod executable;
mod policy;

pub(crate) use command::command;
pub(crate) use executable::{
    ExecutableMetadataIdentity, executable_metadata_identity, hash_executable,
    verify_owned_native_executable,
};

pub(crate) const MACOS_SANDBOX_EXECUTABLE: &str = "/usr/bin/sandbox-exec";

#[cfg(test)]
pub fn macos_confined_metadata_paths(
    executable: &std::path::Path,
    additional_executables: &[std::path::PathBuf],
    confined_read_roots: &[&std::path::Path],
) -> std::io::Result<Vec<std::path::PathBuf>> {
    policy::macos_confined_metadata_paths(executable, additional_executables, confined_read_roots)
}

#[cfg(test)]
pub fn macos_helper_metadata_roots(
    additional_executables: &[std::path::PathBuf],
) -> std::io::Result<Vec<std::path::PathBuf>> {
    policy::macos_helper_metadata_roots(additional_executables)
}

#[cfg(test)]
use policy::{
    MACOS_CONFINED_METADATA_PATH_LIMIT, MACOS_TLS_CONFIGURATION_ALIAS_ROOT,
    MACOS_TLS_CONFIGURATION_ROOT,
};

#[cfg(test)]
mod tests;
