// Opens or creates a persistent lock file directly beneath a retained
// `CapabilityDirectory` without following symlinks or truncating existing
// content.
//
// Why split `create_new(true)` from a fallback non-creating `open_with` instead
// of a single `create(true)` call? On Darwin 24.6 with APFS, when two processes
// race to create the same filename via `openat` relative to a directory
// descriptor, the loser fails with `ENOENT` roughly 50% of the time instead of
// opening the winner's file (the kernel recovers from lost creation races for
// path-based `open`, but not for directory-descriptor `openat`). Attempting an
// exclusive create (`O_CREAT | O_EXCL`) first either creates the file or
// returns `AlreadyExists`; the second `open_with` (without `O_CREAT` and with
// `FollowSymlinks::No`) then opens the winner's regular file or rejects a
// symlink at that name.

use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
#[cfg(unix)]
use cap_std::fs::OpenOptionsExt;
use cap_std::fs::{Dir as CapabilityDirectory, File as CapabilityFile, OpenOptions};
use std::{io, path::Path};

pub fn open_or_create_lock_file(
    directory: &CapabilityDirectory,
    name: impl AsRef<Path>,
) -> io::Result<CapabilityFile> {
    let name = name.as_ref();
    let mut create = OpenOptions::new();
    create
        .read(true)
        .write(true)
        .create_new(true)
        .follow(FollowSymlinks::No);
    #[cfg(unix)]
    create.mode(0o600);
    match directory.open_with(name, &create) {
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        created_or_failed => return created_or_failed,
    }
    let mut existing = OpenOptions::new();
    existing.read(true).write(true).follow(FollowSymlinks::No);
    directory.open_with(name, &existing)
}
