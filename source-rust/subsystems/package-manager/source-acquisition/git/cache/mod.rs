/*
Git repository cache creation, retained custody verification, and invalidation.

We store cached bare Git repositories in per-locator, per-revision,
per-transport cache entries (`identity`). `creation` initializes new entries
with an empty template and compiler-owned canonical configuration (`SHA-1` or
`SHA-256`), `repository` opens and verifies retained entries (`VerifiedGitRepository`)
while rejecting external object indirection (`objects/info/alternates`,
`objects/info/http-alternates`, `commondir`), and `snapshots` retains the
immutable `snapshots/` publication collection inside each entry.
*/

/*
Why invalidation removes metadata rather than trusting partial state:

When a cache entry fails custody or object authentication, `invalidation`
removes `GIT_CACHE_METADATA` through the retained capability directory so
subsequent resolutions treat the entry as incomplete. On a later acquisition
with network access (`retry`), `discard_incomplete_entry` verifies custody of
the incomplete entry and removes the entire entry directory rather than
reconstructing metadata over unverified objects.
*/

pub(crate) mod creation;
pub(crate) mod custody;
pub(crate) mod identity;
pub(crate) mod invalidation;
pub(crate) mod repository;
pub(crate) mod retry;
pub(crate) mod snapshots;

use crate::custody::platform::same_capability_file_identity;
use crate::error::{SourceResolveError, cache_invalid};
use crate::limits::GIT_CONFIG_SHA256;
use crate::tree::filesystem::IoResultAt;
use cap_fs_ext::DirExt;
use cap_std::fs::Dir as CapabilityDirectory;
use platform_custody::record_file::{RecordFileLimits, RecordFileRoot};
use std::ffi::OsStr;
use std::path::Path;

// Atomic restoration of the compiler-owned bare Git `config` control file
// (`GIT_CONFIG_SHA1` or `GIT_CONFIG_SHA256`) inside a retained repository
// directory via `platform_custody::record_file::RecordFileRoot`.

pub(crate) fn replace_canonical_git_control_file(
    entry: &CapabilityDirectory,
    repository_name: &OsStr,
    repository_path: &Path,
    canonical_config: &[u8],
) -> Result<(), SourceResolveError> {
    let classified = entry
        .symlink_metadata(repository_name)
        .at(repository_path)?;
    if classified.file_type().is_symlink() || !classified.is_dir() {
        return Err(cache_invalid(
            repository_path,
            "Git repository is not a concrete directory",
        ));
    }
    let directory = entry
        .open_dir_nofollow(repository_name)
        .map_err(|error| cache_invalid(repository_path, error.to_string()))?;
    let opened = directory.dir_metadata().at(repository_path)?;
    if !same_capability_file_identity(&classified, &opened) {
        return Err(cache_invalid(
            repository_path,
            "Git repository changed while opening it for configuration replacement",
        ));
    }
    replace_canonical_git_control_file_from_open_repository(
        &directory,
        repository_path,
        canonical_config,
    )
}

pub(crate) fn replace_canonical_git_control_file_from_open_repository(
    repository: &CapabilityDirectory,
    repository_path: &Path,
    canonical_config: &[u8],
) -> Result<(), SourceResolveError> {
    let config_path = repository_path.join("config");
    let directory = repository.try_clone().at(repository_path)?;
    let root = RecordFileRoot::from_directory(directory, repository_path.to_path_buf()).map_err(
        |error| {
            cache_invalid(
                repository_path,
                format!("failed to bind Git configuration directory custody: {error:?}"),
            )
        },
    )?;
    root.replace_existing(
        Path::new("config"),
        canonical_config,
        RecordFileLimits {
            maximum_bytes: GIT_CONFIG_SHA256.len(),
        },
    )
    .map_err(|error| {
        cache_invalid(
            &config_path,
            format!("failed to atomically restore canonical Git configuration: {error:?}"),
        )
    })
}
