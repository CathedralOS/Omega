/*

Build-directory ownership and live occupancy locking (`admit_outputs`, `acquire`).

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
  touching any existing artifacts. If two processes for the same root race to
  create `.omega-owner` for the first time, the loser catches `AlreadyExists`
  and re-reads the winner's file.
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

pub(crate) struct TargetWork {
    pub profile: target::TargetProfile,
    pub directory: Result<PathBuf, crate::BuildError>,
}

pub(crate) fn admit_outputs(
    profiles: &[target::TargetProfile],
    base: &Path,
    root: &Path,
    per_target: bool,
    private: bool,
) -> (Vec<TargetWork>, Vec<BuildDirOccupancy>) {
    let mut work = Vec::with_capacity(profiles.len());
    let mut occupancies = Vec::new();
    for profile in profiles {
        let directory = if per_target {
            base.join(profile.target_name())
        } else {
            base.to_path_buf()
        };
        let directory = if private {
            Ok(directory)
        } else {
            match acquire(&directory, root) {
                Ok(occupancy) => {
                    let directory = occupancy.directory().to_path_buf();
                    occupancies.push(occupancy);
                    Ok(directory)
                }
                Err(error) => Err(crate::BuildError::BuildDirectory(error.to_string())),
            }
        };
        work.push(TargetWork {
            profile: *profile,
            directory,
        });
    }
    (work, occupancies)
}

#[derive(Debug, PartialEq, Eq)]
pub enum BuildDirConflict {
    AliasedRoot {
        build_dir: PathBuf,
        recorded: String,
        requested: String,
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
    check_owner(&admitted.canonical, &root)?;
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
            Ok(BuildDirOccupancy {
                occupant,
                directory: admitted.canonical,
            })
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
    let recorded = match fs::read_to_string(&owner) {
        Ok(contents) => contents
            .lines()
            .find_map(|line| line.strip_prefix("root "))
            .map(str::to_owned),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return match File::create_new(&owner) {
                Ok(mut marker) => {
                    let _ = writeln!(marker, "{OWNER_MAGIC}");
                    let _ = writeln!(marker, "root {root}");
                    Ok(())
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    check_owner(build_dir, root)
                }
                Err(_) => Err(occupied_io(build_dir)),
            };
        }
        Err(_) => return Err(occupied_io(build_dir)),
    };
    match recorded {
        None => Ok(()),
        Some(recorded) if recorded == root => Ok(()),
        Some(recorded) => Err(BuildDirConflict::AliasedRoot {
            build_dir: canonicalize_or_literal(build_dir),
            recorded,
            requested: root.to_owned(),
        }),
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
