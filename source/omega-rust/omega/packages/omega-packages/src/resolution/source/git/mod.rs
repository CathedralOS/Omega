//! Git request validation through authenticated immutable publication.

pub(super) mod cache;
pub(super) mod execution;
pub(super) mod objects;
pub(super) mod request;
pub(super) mod resolve;
pub(super) mod snapshot;

#[cfg(test)]
pub(super) use cache::invalidate_git_cache_entry_from_retained_parent;
#[allow(unused_imports)] // Preserve the former source-internal Git facade explicitly.
pub(super) use cache::{
    RetainedGitSnapshots, VerifiedGitRepository, append_framed_bytes, cache_invalid,
    create_git_cache_entry, git_cache_identity, git_cache_metadata,
    invalidate_git_cache_entry_from_open_parent, local_snapshot_invalid,
    parse_git_remote_object_format,
};
#[cfg(unix)]
#[allow(unused_imports)]
pub(super) use execution::verify_macos_open_executable_acl_custody;
#[allow(unused_imports)] // Preserve the former source-internal Git facade explicitly.
pub(super) use execution::{
    BoundedCommandOutput, CapturedOutputLimitExceeded, GitCapturedOutputBudget,
    GitCommandStdinIdentity, GitExecutableMetadataIdentity, GitExecutor,
    GitTransportExecutableObservation, StreamCaptureResult, command_cleanup_reserve,
    duration_millis, format_hex, format_sha256, git_batch_stdin_identity,
    git_command_configuration_identity, git_helper_path, null_device,
    open_git_transport_executable, open_https_transport_executable, process_group_already_absent,
    reconcile_git_cache_operation_result, reconcile_git_command_endpoint_result,
    reconcile_git_command_result, resolver_connect_helper_path, run_command_bounded_with_budget,
    run_command_bounded_with_stdin_and_budget, run_git, run_git_bytes_stdout, run_git_output,
    run_git_stdout, sealed_git_command_with_route, sealed_ssh_command, system_git_candidates,
    verify_git_transport_executable,
};
#[cfg(test)]
#[allow(unused_imports)]
pub(super) use execution::{
    capture_stream_bounded, run_command_bounded, sealed_git_command, test_file_network_endpoint,
    test_system_git_executor,
};
#[cfg(test)]
pub(super) use objects::read_git_blobs_batch_from_path;
#[allow(unused_imports)] // Preserve the former source-internal Git facade explicitly.
pub(super) use objects::{
    GitBlobBytes, GitTreeEntry, GitTreeEntryKind, PendingGitBatchRequest, assign_git_batch_output,
    authenticate_git_commit, authenticate_git_commit_payload, authenticate_git_tree,
    finalize_checked_sha1, git_batch_output_limit, git_directory_paths, git_object_algorithm,
    git_object_identity, git_object_invalid, git_tree_invalid, hex_digit, inspect_git_tree,
    is_object_id, parse_git_tree_entries, validate_git_symlink_target, verify_exact_git_revision,
    verify_git_object_identity,
};
#[allow(unused_imports)] // Preserve the former source-internal Git facade explicitly.
pub(super) use request::{
    GitExecutionTransport, GitSourceRequest, GitSourceRequestError, GitTransportProfile,
};
#[allow(unused_imports)] // Preserve the former source-internal Git facade explicitly.
pub(super) use resolve::{
    bounded_git_fetch_arguments, replace_canonical_git_control_file,
    replace_canonical_git_control_file_from_open_repository, requested_network_endpoint,
    resolve_git_source, resolve_git_source_in_lane, resolve_verified_git_cache_entry,
    validate_pending_git_request, verify_pending_git_snapshot,
};
#[cfg(test)]
#[allow(unused_imports)]
pub(super) use snapshot::{
    GitSnapshotMetadata, git_snapshot_metadata, make_snapshot_read_only, make_tree_owner_writable,
    preflight_git_snapshot,
};
#[allow(unused_imports)] // Preserve the former source-internal Git facade explicitly.
pub(super) use snapshot::{
    PendingMaterializedSnapshot, create_snapshot_symlink_from_open_root, local_snapshot_metadata,
    make_open_snapshot_read_only, make_open_tree_owner_writable, open_or_create_snapshot_directory,
    resolve_git_snapshot, verify_local_snapshot, verify_open_snapshot_tree_modes,
    write_snapshot_file_from_open_root,
};
