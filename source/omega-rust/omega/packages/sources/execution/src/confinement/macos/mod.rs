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
mod tests;
