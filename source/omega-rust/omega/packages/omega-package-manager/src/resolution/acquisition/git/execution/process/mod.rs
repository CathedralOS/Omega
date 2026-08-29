//! Sealed Git process construction and bounded execution.
//!
//! Command policy is assembled in [`command`], stable execution identities in
//! [`identity`], process capture and cleanup in [`capture`], Git-specific
//! invocation in [`invocation`], and joined outcome precedence in
//! [`reconciliation`].

mod capture;
mod command;
mod identity;
mod invocation;
mod reconciliation;

#[allow(unused_imports)] // Preserve the former package-internal process API.
pub(in crate::resolution::acquisition) use capture::{
    BoundedCommandOutput, StreamCaptureResult, command_cleanup_reserve, duration_millis,
    run_command_bounded_with_budget, run_command_bounded_with_stdin_and_budget,
};
#[cfg(test)]
pub(in crate::resolution::acquisition) use capture::{capture_stream_bounded, run_command_bounded};
#[cfg(test)]
pub(in crate::resolution::acquisition) use command::sealed_git_command;
#[allow(unused_imports)] // Preserve the former package-internal process API.
pub(in crate::resolution::acquisition) use command::{
    git_helper_path, null_device, sealed_git_command_with_route, sealed_ssh_command,
};
#[allow(unused_imports)] // Preserve the former package-internal process API.
pub(in crate::resolution::acquisition) use identity::{
    GitCommandStdinIdentity, git_batch_stdin_identity, git_command_configuration_identity,
};
#[allow(unused_imports)] // Preserve the former package-internal process API.
pub(in crate::resolution::acquisition) use invocation::{
    run_git, run_git_bytes_stdout, run_git_output, run_git_stdout,
};
#[allow(unused_imports)] // Preserve the former package-internal process API.
pub(in crate::resolution::acquisition) use reconciliation::{
    reconcile_git_cache_operation_result, reconcile_git_command_endpoint_result,
    reconcile_git_command_result,
};

pub(in crate::resolution::acquisition) fn format_sha256(bytes: &[u8]) -> String {
    identity::format_sha256(bytes)
}

pub(in crate::resolution::acquisition) fn format_hex(bytes: &[u8]) -> String {
    identity::format_hex(bytes)
}
