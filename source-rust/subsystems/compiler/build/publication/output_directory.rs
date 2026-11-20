/*

Build-directory ownership and live occupancy locking.

A product compilation writes artifacts into a caller-visible build directory.
Two different source roots defaulting to the same output path, one directory
reached through different path spellings (symlinks, `..` segments, or shared
mounts), or two concurrent `omega` processes targeting the same directory would
otherwise overwrite or interleave each other's outputs.

We guard each non-private target directory with two marker files written inside
 the canonical directory:
- `.omega-owner` (`OWNER_FILE`): records the canonical `root <path>` of the
  source project that owns this build directory. This file persists across
  invocations (even if a compile is interrupted), so a later compile from a
  different source root is refused with `BuildDirConflict::AliasedRoot` before
  touching any existing artifacts. The process holding the occupancy marker
  reads or establishes ownership, so concurrent callers cannot read a partial record.
- `.omega-occupant` (`OCCUPANT_FILE`): created atomically with `File::create_new`
  for the duration of the compile and removed when `BuildDirOccupancy` drops. A
  second concurrent compile sees `AlreadyExists` and fails immediately with
  `BuildDirConflict::Occupied` (reporting the live PID, host, and timestamp from
  the marker).

*/

/*

Why we recheck `DirectoryIdentity` after creating `.omega-occupant` (`confirm_admitted`):

Between resolving `build_dir` and creating `.omega-occupant`, an external
process could rename the directory away and create a new one at the same path,
or retarget a symlink ancestor. We capture `DirectoryIdentity` (both the
canonical path spelling and the OS filesystem object `(dev, ino)` / file index
from `platform_custody::filesystem_object_identity`) before `create_new`, and
re-query `directory_identity(build_dir)` immediately after writing `.omega-occupant`.
If the identity changed (`BuildDirConflict::MovedDuringAdmission`), we unlink
our `.omega-occupant` marker from the old directory and abort before publishing
any artifacts into an unverified location.

*/

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const OWNER_FILE: &str = ".omega-owner";
const OCCUPANT_FILE: &str = ".omega-occupant";
const OWNER_MAGIC: &str = "omega-build-directory-owner 1";
const OCCUPANT_MAGIC: &str = "omega-build-directory-occupant 1";

/// Use a caller-selected directory or put the output beside the input program.
pub fn base(root: &Path, requested: Option<&Path>) -> PathBuf {
    requested
        .map(Path::to_path_buf)
        .unwrap_or_else(|| root.parent().unwrap_or_else(|| Path::new("")).join("build"))
}

#[derive(Debug, PartialEq, Eq)]
pub enum BuildDirConflict {
    AliasedRoot {
        build_dir: PathBuf,
        recorded: String,
        requested: String,
    },
    InvalidOwner {
        build_dir: PathBuf,
    },
    Occupied {
        build_dir: PathBuf,
        detail: String,
    },
    MovedDuringAdmission {
        build_dir: PathBuf,
        observed: PathBuf,
    },
}

impl std::fmt::Display for BuildDirConflict {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AliasedRoot {
                build_dir,
                recorded,
                requested,
            } => write!(
                formatter,
                "build directory `{}` is already the output of a different source root \
                 `{recorded}` (this invocation compiles `{requested}`); \
                 pass a separate --build-dir or remove the directory",
                build_dir.display()
            ),
            Self::InvalidOwner { build_dir } => write!(
                formatter,
                "build directory `{}` has an unreadable or incomplete owner record",
                build_dir.display()
            ),
            Self::Occupied { build_dir, detail } => write!(
                formatter,
                "another compilation is already using build directory `{}` ({detail}); \
                 if that compile is gone, remove `{OCCUPANT_FILE}`",
                build_dir.display()
            ),
            Self::MovedDuringAdmission {
                build_dir,
                observed,
            } => write!(
                formatter,
                "build directory `{}` moved or was replaced during acquisition \
                 (the spelling now resolves to `{}`); a renamed or symlinked \
                 ancestor changed underneath the admission check — retry, or \
                 use the canonical spelling directly",
                build_dir.display(),
                observed.display()
            ),
        }
    }
}

impl std::error::Error for BuildDirConflict {}

#[derive(Debug)]
pub struct BuildDirOccupancy {
    occupant: PathBuf,
    directory: PathBuf, // Canonical directory path that records and downstream artifact writes bind to.
}

impl BuildDirOccupancy {
    pub fn directory(&self) -> &Path {
        &self.directory
    }
}

impl Drop for BuildDirOccupancy {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.occupant);
    }
}

pub fn acquire(build_dir: &Path, root_path: &Path) -> Result<BuildDirOccupancy, BuildDirConflict> {
    if fs::create_dir_all(build_dir).is_err() {
        return Err(occupied_io(build_dir));
    }
    let root = display_path(canonicalize_or_literal(root_path));
    let admitted = directory_identity(build_dir);
    let occupant = admitted.canonical.join(OCCUPANT_FILE);
    match File::create_new(&occupant) {
        Ok(mut marker) => {
            let _ = writeln!(marker, "{OCCUPANT_MAGIC}");
            let _ = writeln!(marker, "root {root}");
            let _ = writeln!(marker, "pid {}", std::process::id());
            let _ = writeln!(marker, "host {}", host_name());
            let _ = writeln!(marker, "unix_ms {}", unix_millis());
            if let Err(conflict) = confirm_admitted(build_dir, &admitted) {
                let _ = fs::remove_file(&occupant);
                return Err(conflict);
            }
            let occupancy = BuildDirOccupancy {
                occupant,
                directory: admitted.canonical,
            };
            check_owner(occupancy.directory(), &root)?;
            Ok(occupancy)
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            let detail = fs::read_to_string(&occupant)
                .map(|contents| contents.lines().skip(1).collect::<Vec<_>>().join(", "))
                .unwrap_or_else(|_| "unreadable occupant record".to_owned());
            Err(BuildDirConflict::Occupied {
                build_dir: admitted.canonical,
                detail,
            })
        }
        Err(_) => Err(occupied_io(build_dir)),
    }
}

#[derive(Debug, PartialEq, Eq)]
struct DirectoryIdentity {
    canonical: PathBuf,
    object: Option<(u64, u64)>,
}

fn directory_identity(spelled: &Path) -> DirectoryIdentity {
    let canonical = canonicalize_or_literal(spelled);
    let object = directory_object(&canonical);
    DirectoryIdentity { canonical, object }
}

fn directory_object(path: &Path) -> Option<(u64, u64)> {
    platform_custody::filesystem_object_identity(path).ok()
}

fn confirm_admitted(spelled: &Path, admitted: &DirectoryIdentity) -> Result<(), BuildDirConflict> {
    let observed = directory_identity(spelled);
    if observed != *admitted {
        return Err(BuildDirConflict::MovedDuringAdmission {
            build_dir: admitted.canonical.clone(),
            observed: observed.canonical,
        });
    }
    Ok(())
}

fn check_owner(build_dir: &Path, root: &str) -> Result<(), BuildDirConflict> {
    let owner = build_dir.join(OWNER_FILE);
    let invalid = || BuildDirConflict::InvalidOwner {
        build_dir: build_dir.to_owned(),
    };
    let expected = format!("{OWNER_MAGIC}\nroot {root}\n");
    match fs::read_to_string(&owner) {
        Ok(contents) if contents == expected => Ok(()),
        Ok(contents) => {
            let recorded = contents
                .strip_prefix(OWNER_MAGIC)
                .and_then(|body| body.strip_prefix("\nroot "))
                .and_then(|path| path.strip_suffix('\n'))
                .filter(|path| !path.is_empty())
                .ok_or_else(invalid)?;
            Err(BuildDirConflict::AliasedRoot {
                build_dir: build_dir.to_owned(),
                recorded: recorded.to_owned(),
                requested: root.to_owned(),
            })
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let mut marker = File::create_new(owner).map_err(|_| invalid())?;
            marker.write_all(expected.as_bytes()).map_err(|_| invalid())
        }
        Err(_) => Err(invalid()),
    }
}

fn occupied_io(build_dir: &Path) -> BuildDirConflict {
    BuildDirConflict::Occupied {
        build_dir: build_dir.to_path_buf(),
        detail: "cannot create the occupancy record".to_owned(),
    }
}

fn canonicalize_or_literal(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn display_path(path: PathBuf) -> String {
    path.display().to_string()
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn host_name() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "-".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use platform_custody::temporary_directory::TemporaryDirectory;

    #[test]
    fn occupancy_is_temporary_and_the_source_owner_persists() {
        let temp = TemporaryDirectory::create("build-output-owner", false).unwrap();
        let root = temp.path().join("saved.psi");
        let directory = base(&root, None);
        assert_eq!(directory, temp.path().join("build"));
        let first = acquire(&directory, &root).unwrap();
        assert_eq!(first.directory(), directory.canonicalize().unwrap());
        assert!(matches!(
            acquire(&directory, &root),
            Err(BuildDirConflict::Occupied { .. })
        ));
        drop(first);
        let other = temp.path().join("other.psi");
        assert!(matches!(
            acquire(&directory, &other),
            Err(BuildDirConflict::AliasedRoot { .. })
        ));
        assert!(acquire(&directory, &root).is_ok());
        assert_eq!(base(&root, Some(&other)), other);
    }

    #[test]
    fn incomplete_owner_cannot_authorize_publication() {
        let temp = TemporaryDirectory::create("invalid-build-output-owner", false).unwrap();
        fs::write(temp.path().join(OWNER_FILE), OWNER_MAGIC).unwrap();
        assert!(matches!(
            acquire(temp.path(), Path::new("saved.psi")),
            Err(BuildDirConflict::InvalidOwner { .. })
        ));
        assert!(!temp.path().join(OCCUPANT_FILE).exists());
    }
}
