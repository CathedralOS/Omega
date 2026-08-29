//! Canonical Git cache entry construction and object-format discovery.

use std::ffi::{OsStr, OsString};
use std::path::Path;

use cap_fs_ext::OpenOptionsFollowExt;

use crate::resolution::acquisition::{
    CacheCustodyKind, CapabilityDirectory, CapabilityOpenOptions, FollowSymlinks,
    GIT_CACHE_METADATA, GIT_CACHE_REPOSITORY, GIT_CONFIG_SHA1, GIT_CONFIG_SHA256,
    GitExecutionTransport, GitExecutor, GitObjectIdAlgorithm, LocalSourceLimits, PendingCacheEntry,
    ResolverExecutionPhase, SourceResolveError, Write, git_object_algorithm, io_error,
    is_object_id, reconcile_git_cache_operation_result, replace_canonical_git_control_file,
    run_git, run_git_bytes_stdout, verify_capability_cache_node_owner_and_mode,
};
#[cfg(unix)]
use crate::resolution::acquisition::{CapabilityOpenOptionsExt, CapabilityPermissionsExt};

use super::{VerifiedGitRepository, cache_invalid, git_cache_metadata};

pub(in crate::resolution::acquisition) fn create_git_cache_entry(
    executor: &GitExecutor,
    cache_dir: &Path,
    cache_directory: &CapabilityDirectory,
    entry_root: &Path,
    entry_name: &OsStr,
    cache_identity: &str,
    locator_identity: &str,
    fetch_locator: &str,
    requested_rev: &str,
    execution_transport: GitExecutionTransport,
    limits: LocalSourceLimits,
) -> Result<(), SourceResolveError> {
    let mut pending = PendingCacheEntry::create(cache_dir, cache_directory, cache_identity)?;
    let repository = pending.root.join(GIT_CACHE_REPOSITORY);
    let empty_template = pending.root.join("empty-template");
    pending.create_private_directory("empty-template", &empty_template)?;
    pending.verify_ambient_path_identity(cache_dir)?;
    let object_format_result =
        discover_git_object_format(executor, &pending.root, fetch_locator, requested_rev);
    let object_format = reconcile_git_cache_operation_result(
        object_format_result,
        pending.verify_ambient_path_identity(cache_dir),
        None,
    )?;
    let mut init_arguments = vec![
        OsString::from("init"),
        OsString::from("--quiet"),
        OsString::from("--bare"),
    ];
    if object_format == GitObjectIdAlgorithm::Sha256 {
        init_arguments.push(OsString::from("--object-format=sha256"));
    }
    init_arguments.push(OsString::from("--template"));
    init_arguments.push(empty_template.as_os_str().to_owned());
    init_arguments.push(repository.as_os_str().to_owned());
    pending.verify_ambient_path_identity(cache_dir)?;
    let init_result = run_git(
        executor,
        &pending.root,
        ResolverExecutionPhase::RepositoryInitialization,
        init_arguments.iter(),
    );
    reconcile_git_cache_operation_result(
        init_result,
        pending.verify_ambient_path_identity(cache_dir),
        None,
    )?;
    let canonical_config = match object_format {
        GitObjectIdAlgorithm::Sha1 => GIT_CONFIG_SHA1,
        GitObjectIdAlgorithm::Sha256 => GIT_CONFIG_SHA256,
    };
    pending.verify_ambient_path_identity(cache_dir)?;
    let config_result = replace_canonical_git_control_file(
        pending.directory()?,
        OsStr::new(GIT_CACHE_REPOSITORY),
        &repository,
        canonical_config,
    );
    reconcile_git_cache_operation_result(
        config_result,
        pending.verify_ambient_path_identity(cache_dir),
        None,
    )?;
    pending
        .directory()?
        .remove_dir("empty-template")
        .map_err(|error| io_error(&empty_template, error))?;

    let metadata_path = pending.root.join(GIT_CACHE_METADATA);
    let mut metadata_options = CapabilityOpenOptions::new();
    metadata_options
        .write(true)
        .create_new(true)
        .follow(FollowSymlinks::No);
    #[cfg(unix)]
    metadata_options.mode(0o600);
    let mut metadata = pending
        .directory()?
        .open_with(GIT_CACHE_METADATA, &metadata_options)
        .map_err(|error| io_error(&metadata_path, error))?;
    #[cfg(unix)]
    {
        let mut permissions = metadata
            .metadata()
            .map_err(|error| io_error(&metadata_path, error))?
            .permissions();
        permissions.set_mode(0o600);
        metadata
            .set_permissions(permissions)
            .map_err(|error| io_error(&metadata_path, error))?;
    }
    metadata
        .write_all(&git_cache_metadata(
            locator_identity,
            requested_rev,
            execution_transport,
        ))
        .map_err(|error| io_error(&metadata_path, error))?;
    metadata
        .sync_all()
        .map_err(|error| io_error(&metadata_path, error))?;
    let metadata_custody = metadata
        .metadata()
        .map_err(|error| io_error(&metadata_path, error))?;
    verify_capability_cache_node_owner_and_mode(
        CacheCustodyKind::Git,
        &metadata_path,
        &metadata_custody,
    )?;
    #[cfg(unix)]
    {
        use cap_fs_ext::OsMetadataExt;

        if metadata_custody.mode() & 0o777 != 0o600 {
            return Err(cache_invalid(
                &metadata_path,
                "resolver metadata does not have exact private mode 0600",
            ));
        }
    }

    pending.verify_ambient_path_identity(cache_dir)?;
    let verification_result = VerifiedGitRepository::open(
        &pending.parent,
        &pending.stage_name,
        &pending.root,
        locator_identity,
        requested_rev,
        execution_transport,
        limits,
    );
    reconcile_git_cache_operation_result(
        verification_result,
        pending.verify_ambient_path_identity(cache_dir),
        None,
    )?;
    pending.publish(cache_dir, entry_root, entry_name)?;
    Ok(())
}

fn discover_git_object_format(
    executor: &GitExecutor,
    working_directory: &Path,
    url: &str,
    requested_rev: &str,
) -> Result<GitObjectIdAlgorithm, SourceResolveError> {
    if is_object_id(requested_rev) {
        return git_object_algorithm(requested_rev);
    }
    let output = run_git_bytes_stdout(
        executor,
        working_directory,
        ResolverExecutionPhase::TransportDiscovery,
        [
            OsStr::new("ls-remote"),
            OsStr::new("--symref"),
            OsStr::new("--"),
            OsStr::new(url),
            OsStr::new("HEAD"),
            OsStr::new(requested_rev),
        ],
    )?;
    parse_git_remote_object_format(&output, working_directory)
}

pub(in crate::resolution::acquisition) fn parse_git_remote_object_format(
    output: &[u8],
    working_directory: &Path,
) -> Result<GitObjectIdAlgorithm, SourceResolveError> {
    let mut selected = None;
    for line in output.split(|byte| *byte == b'\n') {
        if line.is_empty() || line.starts_with(b"ref: ") {
            continue;
        }
        let Some(separator) = line.iter().position(|byte| *byte == b'\t') else {
            return Err(cache_invalid(
                working_directory,
                "Git object-format discovery returned a malformed row",
            ));
        };
        let oid = std::str::from_utf8(&line[..separator]).map_err(|_| {
            cache_invalid(
                working_directory,
                "Git object-format discovery returned a non-ASCII object ID",
            )
        })?;
        let algorithm = git_object_algorithm(oid)?;
        if selected.is_some_and(|selected| selected != algorithm) {
            return Err(cache_invalid(
                working_directory,
                "Git object-format discovery returned mixed hash algorithms",
            ));
        }
        selected = Some(algorithm);
    }
    selected.ok_or_else(|| {
        cache_invalid(
            working_directory,
            "Git object-format discovery returned no selected object ID",
        )
    })
}
