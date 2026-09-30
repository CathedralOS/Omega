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

#[cfg(test)]
mod tests {
    use super::open_or_create_lock_file;
    use cap_std::fs::Dir as CapabilityDirectory;
    use std::path::PathBuf;
    use std::sync::Barrier;

    struct TemporaryDirectory(PathBuf);

    impl TemporaryDirectory {
        fn create(name: &str) -> Self {
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "omega-lock-file-{name}-{}-{stamp}",
                std::process::id()
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn open(&self) -> CapabilityDirectory {
            CapabilityDirectory::open_ambient_dir(&self.0, cap_std::ambient_authority()).unwrap()
        }
    }

    impl Drop for TemporaryDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn contended_creation_opens_the_one_created_file() {
        const CONTENDERS: usize = 8;
        const COLD_NAMES: usize = 32;
        let temporary = TemporaryDirectory::create("contended");
        let directory = temporary.open();

        for name in 0..COLD_NAMES {
            let name = format!("entry-{name}.lock");
            let start = Barrier::new(CONTENDERS);
            let opened = std::thread::scope(|scope| {
                let contenders = (0..CONTENDERS)
                    .map(|_| {
                        scope.spawn(|| {
                            start.wait();
                            open_or_create_lock_file(&directory, &name)
                        })
                    })
                    .collect::<Vec<_>>();
                contenders
                    .into_iter()
                    .map(|contender| contender.join().expect("contender finished"))
                    .collect::<Vec<_>>()
            });
            let named = directory.symlink_metadata(&name).unwrap();
            for result in opened {
                let file = result.unwrap_or_else(|error| panic!("{name}: {error}"));
                #[cfg(unix)]
                {
                    use cap_std::fs::MetadataExt;

                    let opened = file.metadata().unwrap();
                    assert_eq!((opened.dev(), opened.ino()), (named.dev(), named.ino()));
                }
                #[cfg(not(unix))]
                let _ = (file, &named);
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn creation_is_owner_only_and_reopening_keeps_contents() {
        use cap_std::fs::MetadataExt;
        use std::io::{Read, Write};

        let temporary = TemporaryDirectory::create("reopen");
        let directory = temporary.open();
        let mut created = open_or_create_lock_file(&directory, "entry.lock").unwrap();
        created.write_all(b"held").unwrap();
        assert_eq!(created.metadata().unwrap().mode() & 0o777, 0o600);

        let mut reopened = open_or_create_lock_file(&directory, "entry.lock").unwrap();
        let mut contents = String::new();
        reopened.read_to_string(&mut contents).unwrap();
        assert_eq!(contents, "held");
    }

    #[cfg(unix)]
    #[test]
    fn symbolic_links_are_rejected_not_followed() {
        let temporary = TemporaryDirectory::create("symlink");
        std::fs::write(temporary.0.join("target"), b"").unwrap();
        std::os::unix::fs::symlink("target", temporary.0.join("entry.lock")).unwrap();
        std::os::unix::fs::symlink("absent", temporary.0.join("dangling.lock")).unwrap();
        let directory = temporary.open();

        assert!(open_or_create_lock_file(&directory, "entry.lock").is_err());
        assert!(open_or_create_lock_file(&directory, "dangling.lock").is_err());
        assert!(!temporary.0.join("absent").exists());
    }
}
