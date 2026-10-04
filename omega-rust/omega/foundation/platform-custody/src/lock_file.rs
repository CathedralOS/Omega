//! Persistent lock files opened beneath a retained directory.

use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
#[cfg(unix)]
use cap_std::fs::OpenOptionsExt;
use cap_std::fs::{Dir as CapabilityDirectory, File as CapabilityFile, OpenOptions};
use std::io;
use std::path::Path;

/// Open the lock file `name` directly beneath `directory` for reading and
/// writing, creating it owner-only (mode 0600 on Unix) when it is absent.
///
/// A final symbolic link is never followed and an existing file is never
/// truncated. Callers keep lock files in place and still verify the opened
/// file's identity, owner, and mode; this only chooses how the open is made.
///
/// Creation is exclusive, and a name that already exists is reopened without
/// `O_CREAT`. The single open-or-create call this replaces is wrong on macOS:
/// when two processes create one name at once through `openat` relative to a
/// directory descriptor, the one that loses the creation race fails with
/// `ENOENT` instead of opening the winner's file. On Darwin 24.6 with APFS
/// that happens for roughly half of all contended creations, while the same
/// contention through `open` by absolute path or relative to the working
/// directory never fails: the kernel recovers from a lost creation race for
/// those lookups but not for a directory-descriptor lookup, and a capability
/// open has only the descriptor. An exclusive create never enters that
/// recovery: it either creates the file or reports `AlreadyExists`, and the
/// second open then finds the file that exists.
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
    // An existing symbolic link also reports `AlreadyExists`; this no-follow
    // open then rejects it rather than opening its target.
    let mut existing = OpenOptions::new();
    existing.read(true).write(true).follow(FollowSymlinks::No);
    directory.open_with(name, &existing)
}
