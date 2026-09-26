//! Build-directory ownership and occupancy records.
//!
//! A product compile publishes into a caller-visible build directory. Two
//! source roots defaulting to the same directory, one directory reached
//! through different spellings (symlinks, `..` segments, shared mounts), and
//! concurrent invocations all share the same output files. The owner record
//! names the canonical source root; the occupant record, created atomically,
//! names the live invocation. A second occupant is refused before any build
//! content is written rather than interleaving artifacts.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const OWNER_FILE: &str = ".omega-owner";
const OCCUPANT_FILE: &str = ".omega-occupant";
const OWNER_MAGIC: &str = "omega-build-directory-owner 1";
const OCCUPANT_MAGIC: &str = "omega-build-directory-occupant 1";

/// A build directory refused this invocation before any output was written.
#[derive(Debug, PartialEq, Eq)]
pub enum BuildDirConflict {
    /// The directory already records a different canonical source root.
    AliasedRoot {
        build_dir: PathBuf,
        recorded: String,
        requested: String,
    },
    /// A live marker says another invocation is compiling into it.
    Occupied { build_dir: PathBuf, detail: String },
    /// The spelled directory resolved to a different directory after admission
    /// than at admission — an alias (symlink, rename, mount) moved inside the
    /// admission window, or the admitted directory was renamed away and a new
    /// one created under its name — so record writes would land under a
    /// different identity than the one that was checked.
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

/// Removes the occupant record this invocation created. The owner record
/// persists: an interrupted compile still left artifacts attributed to its
/// root, and the next invocation must see that ownership.
#[derive(Debug)]
pub struct BuildDirOccupancy {
    occupant: PathBuf,
    /// The canonical directory identity this invocation admitted. Records and
    /// downstream writes bind to it, so a spelling swapped underneath the
    /// admission window cannot redirect or shadow them.
    directory: PathBuf,
}

impl BuildDirOccupancy {
    /// The canonical build directory the records and artifact writes belong to.
    pub fn directory(&self) -> &Path {
        &self.directory
    }
}

impl Drop for BuildDirOccupancy {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.occupant);
    }
}

/// Claim `build_dir` for one compile of `root_path`. Canonicalization folds
/// spelling aliases (`..`, `.`, symlinks) before ownership is compared, so the
/// same root keeps working under a different working-directory spelling while
/// a genuinely different root refuses. The owner and occupant records are
/// written through the canonical directory name, and the spelling is rechecked
/// after acquisition so an alias swapped in during admission cannot redirect
/// the records or shadow them from later artifact writes.
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
                // The record is ours but the spelling moved; leave the admitted
                // directory without a live marker rather than publish into an
                // identity the caller did not request.
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

/// A build directory's host identity: the canonical spelling plus the
/// filesystem object behind it, so renaming the admitted directory away and
/// recreating a fresh one at the same spelling still reads as a different
/// directory. A host that cannot report the object degrades to the canonical
/// spelling alone.
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

/// The directory `spelled` names must still be the one `admitted` recorded: a
/// symlink, rename, or mount swapped inside the admission window must not let
/// record writes follow an identity the caller never asked about.
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
                // Lost the creation race: the winner's record decides.
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

#[cfg(test)]
mod tests {
    use super::{BuildDirConflict, OCCUPANT_FILE, OWNER_FILE, acquire};
    use crate::temporary_directory::TemporaryDirectory;
    use std::path::PathBuf;

    struct Fixture {
        directories: Vec<TemporaryDirectory>,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                directories: Vec::new(),
            }
        }

        fn directory(&mut self, label: &str) -> PathBuf {
            let directory = TemporaryDirectory::create(label, false).unwrap();
            let path = directory.path().to_path_buf();
            self.directories.push(directory);
            path
        }

        fn root(&mut self, label: &str) -> PathBuf {
            let directory = self.directory(label);
            let root = directory.join("main.omg");
            std::fs::write(&root, b"").unwrap();
            root
        }
    }

    #[test]
    fn released_occupancy_lets_the_same_root_compile_again() {
        let mut fixture = Fixture::new();
        let build = fixture.directory("build");
        let root = fixture.root("source-a");
        let first = acquire(&build, &root).unwrap();
        drop(first);
        assert!(acquire(&build, &root).is_ok());
    }

    #[test]
    fn held_occupancy_refuses_a_second_invocation_of_the_same_root() {
        let mut fixture = Fixture::new();
        let build = fixture.directory("build");
        let root = fixture.root("source-a");
        let first = acquire(&build, &root).unwrap();
        match acquire(&build, &root) {
            Err(BuildDirConflict::Occupied { detail, .. }) => {
                assert!(detail.contains("pid "), "{detail}");
                assert!(detail.contains("root "), "{detail}");
            }
            other => panic!("expected Occupied, got {other:?}"),
        }
        drop(first);
        assert!(acquire(&build, &root).is_ok());
    }

    #[test]
    fn a_different_source_root_aliasing_the_directory_refuses() {
        let mut fixture = Fixture::new();
        let build = fixture.directory("build");
        let first_root = fixture.root("source-a");
        let second_root = fixture.root("source-b");
        let first = acquire(&build, &first_root).unwrap();
        drop(first);
        match acquire(&build, &second_root) {
            Err(BuildDirConflict::AliasedRoot {
                recorded,
                requested,
                ..
            }) => {
                assert!(recorded.ends_with("main.omg"), "{recorded}");
                assert_ne!(recorded, requested);
            }
            other => panic!("expected AliasedRoot, got {other:?}"),
        }
    }

    #[test]
    fn aliased_spelling_of_the_same_root_is_not_an_alias() {
        let mut fixture = Fixture::new();
        let build = fixture.directory("build");
        let root = fixture.root("source-a");
        let spelled = root
            .parent()
            .unwrap()
            .join("nested")
            .join("..")
            .join("main.omg");
        std::fs::create_dir_all(spelled.parent().unwrap().parent().unwrap()).unwrap();
        let first = acquire(&build, &root).unwrap();
        drop(first);
        assert!(acquire(&build, &spelled).is_ok());
    }

    #[test]
    fn a_stale_occupant_record_is_reported_with_its_contents() {
        let mut fixture = Fixture::new();
        let build = fixture.directory("build");
        let root = fixture.root("source-a");
        std::fs::write(
            build.join(OCCUPANT_FILE),
            "omega-build-directory-occupant 1\npid 424242\nhost test-host\n",
        )
        .unwrap();
        match acquire(&build, &root) {
            Err(BuildDirConflict::Occupied { detail, .. }) => {
                assert!(detail.contains("pid 424242"), "{detail}");
                assert!(detail.contains("test-host"), "{detail}");
            }
            other => panic!("expected Occupied, got {other:?}"),
        }
    }

    #[test]
    fn dropping_the_guard_removes_only_the_occupant_record() {
        let mut fixture = Fixture::new();
        let build = fixture.directory("build");
        let root = fixture.root("source-a");
        let guard = acquire(&build, &root).unwrap();
        drop(guard);
        assert!(build.join(OWNER_FILE).is_file());
        assert!(!build.join(OCCUPANT_FILE).exists());
    }

    #[test]
    fn acquired_directory_is_the_canonical_spelling() {
        let mut fixture = Fixture::new();
        let build = fixture.directory("build");
        let root = fixture.root("source-a");
        let spelled = build.join("nested").join("..");
        std::fs::create_dir_all(&spelled).unwrap();
        let guard = acquire(&spelled, &root).unwrap();
        assert_eq!(guard.directory(), &std::fs::canonicalize(&build).unwrap());
        drop(guard);
    }

    #[test]
    fn a_spelling_that_no_longer_names_the_admitted_directory_is_refused() {
        let mut fixture = Fixture::new();
        let parent = fixture.directory("parent");
        let build = parent.join("build");
        let moved = parent.join("moved");
        std::fs::create_dir_all(&build).unwrap();
        let admitted = super::directory_identity(&build);
        // The admission-time identity leaves the spelling entirely; the
        // object lookup fails, so the observed identity can no longer match.
        std::fs::rename(&build, &moved).unwrap();
        match super::confirm_admitted(&build, &admitted) {
            Err(BuildDirConflict::MovedDuringAdmission { build_dir, .. }) => {
                assert_eq!(build_dir, admitted.canonical);
            }
            other => panic!("expected MovedDuringAdmission, got {other:?}"),
        }
        // A fresh directory taking over the same spelling is still a different
        // directory: its filesystem object no longer matches the admitted one.
        std::fs::create_dir_all(&build).unwrap();
        assert!(matches!(
            super::confirm_admitted(&build, &admitted),
            Err(BuildDirConflict::MovedDuringAdmission { .. })
        ));
        // Re-admitted under the same name, the new directory checks out.
        assert!(super::confirm_admitted(&build, &super::directory_identity(&build)).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn records_bind_the_canonical_directory_under_a_symlinked_spelling() {
        use std::os::unix::fs::symlink;
        let mut fixture = Fixture::new();
        let real = fixture.directory("real-build");
        let links = fixture.directory("links");
        let alias = links.join("alias");
        symlink(&real, &alias).unwrap();
        let root = fixture.root("source-a");
        let guard = acquire(&alias, &root).unwrap();
        // Records and the admitted directory name the target, not the link.
        assert!(alias.is_symlink());
        assert!(real.join(OWNER_FILE).is_file());
        assert!(real.join(OCCUPANT_FILE).is_file());
        assert_eq!(guard.directory(), &std::fs::canonicalize(&real).unwrap());
        drop(guard);
        // A stale marker left at the link itself does not shadow the target's.
        std::fs::write(
            real.join(OCCUPANT_FILE),
            "omega-build-directory-occupant 1\npid 9\nhost h\n",
        )
        .unwrap();
        assert!(acquire(&alias, &root).is_err());
    }
}
