//! No-follow traversal and policy validation for source trees.
//!
//! Per-member checks (the bounded file read and each entry's first no-follow
//! observation) cover the window up to that member's own inspection. Before a
//! directory's visit returns, `close_captured_directory` re-lists its members
//! and re-observes each captured entry, so a member added, removed, replaced
//! or retargeted while its siblings were being processed still rejects. The
//! directory's own identity metadata participates on both sides, which also
//! rejects member churn that restores the same listing, while excluded policy
//! names keep counting toward membership: the capture premise is a quiescent
//! tree, not merely a quiescent captured subset. Ancestor directories apply
//! the same close sweep, so a replaced child directory cannot hide behind a
//! retained handle. After the deepest close returns,
//! `verify_captured_tree` re-observes every visited directory's member set
//! and every captured member once more before publication, so a mutation
//! landing in an already-closed subtree — or at the root after its own close
//! — still rejects at capture time rather than waiting for the resolver's
//! later live-tree comparison. The sweeps narrow drift detection to the
//! capture window; a mutation racing the final sweep itself remains the
//! boundary of the quiescent-tree premise.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use cap_fs_ext::DirExt;
use cap_std::fs::Dir as CapabilityDirectory;

use super::traversal_observations::{SourceEntry, SourceEntryKind, SourceTreePolicy};
use crate::SourceResolveError;
use crate::limits::{DEFAULT_BUILD_OUTPUT_DIRECTORY, LocalSourceLimits};
use crate::tree::filesystem::{
    CapturedEntryObservation, io_error, open_captured_directory, raw_os_bytes,
    read_capability_file_bounded, require_unchanged_entry,
};

/// One visited directory's opening record, retained for the final whole-tree
/// sweep: the member set listed at open plus the directory's own identity
/// metadata, both re-verified after the traversal completes.
pub(super) struct CapturedDirectoryObservation {
    relative_path: PathBuf,
    metadata: cap_std::fs::Metadata,
    listed_names: Vec<OsString>,
}

/// One captured member's first no-follow observation, keyed by its
/// root-relative path so the final sweep can re-observe it from the retained
/// root handle after every directory's own close has run.
pub(super) struct CapturedMemberObservation {
    relative_path: PathBuf,
    metadata: cap_std::fs::Metadata,
}

/// The tree-wide observation record a traversal accumulates for
/// `verify_captured_tree`'s closing sweep.
#[derive(Default)]
pub(super) struct CapturedTreeObservations {
    directories: Vec<CapturedDirectoryObservation>,
    members: Vec<CapturedMemberObservation>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn visit_directory(
    root_directory: &CapabilityDirectory,
    directory: &CapabilityDirectory,
    display_dir: &Path,
    logical_dir: PathBuf,
    depth: usize,
    root: &Path,
    limits: LocalSourceLimits,
    policy: SourceTreePolicy,
    captured_file_bytes: &mut u64,
    entries: &mut Vec<SourceEntry>,
    observations: &mut CapturedTreeObservations,
) -> Result<(), SourceResolveError> {
    if depth > limits.max_depth {
        return Err(SourceResolveError::TooDeep {
            path: display_dir.to_path_buf(),
            limit: limits.max_depth,
        });
    }

    let remaining_entries = limits.max_entries.saturating_sub(entries.len());
    let excluded_entry_allowance = match policy {
        SourceTreePolicy::ExactMaterialized => 0,
        SourceTreePolicy::LocalPackage if logical_dir.as_os_str().is_empty() => 4,
        SourceTreePolicy::LocalPackage => 1,
    };
    let directory_listing_limit = remaining_entries.saturating_add(excluded_entry_allowance);
    let opening_metadata = directory
        .dir_metadata()
        .map_err(|error| io_error(display_dir, error))?;
    let mut entry_names = Vec::new();
    for entry in directory
        .entries()
        .map_err(|error| io_error(display_dir, error))?
    {
        if entry_names.len() >= directory_listing_limit {
            return Err(SourceResolveError::TooManyFiles {
                limit: limits.max_entries,
            });
        }
        entry_names.push(
            entry
                .map_err(|error| io_error(display_dir, error))?
                .file_name(),
        );
    }
    entry_names.sort();
    let entry_names_listed = entry_names;

    let mut observed_entries = Vec::new();
    for name in &entry_names_listed {
        if excluded_from_capture(policy, &logical_dir, name) {
            continue;
        }
        if entries.len() >= limits.max_entries {
            return Err(SourceResolveError::TooManyFiles {
                limit: limits.max_entries,
            });
        }
        let display_path = display_dir.join(name);
        let logical_path = logical_dir.join(name);
        let metadata = directory
            .symlink_metadata(name)
            .map_err(|error| io_error(&display_path, error))?;
        observed_entries.push(CapturedEntryObservation {
            name: name.clone(),
            metadata: metadata.clone(),
        });
        observations.members.push(CapturedMemberObservation {
            relative_path: logical_path.clone(),
            metadata: metadata.clone(),
        });
        if metadata.file_type().is_symlink() {
            let raw_target = read_and_validate_symlink_target(
                root_directory,
                root,
                directory,
                &logical_dir,
                name,
                &display_path,
                policy,
            )?;
            push_entry(
                entries,
                logical_path,
                SourceEntryKind::Symlink {
                    target_bytes: raw_os_bytes(raw_target.as_os_str()),
                },
                limits,
            )?;
        } else if metadata.is_dir() {
            let child = open_captured_directory(directory, name, &display_path)?;
            push_entry(
                entries,
                logical_path.clone(),
                SourceEntryKind::Directory,
                limits,
            )?;
            visit_directory(
                root_directory,
                &child,
                &display_path,
                logical_path,
                depth + 1,
                root,
                limits,
                policy,
                captured_file_bytes,
                entries,
                observations,
            )?;
        } else if metadata.is_file() {
            let remaining = limits.max_bytes.checked_sub(*captured_file_bytes).ok_or(
                SourceResolveError::TooManyBytes {
                    limit: limits.max_bytes,
                },
            )?;
            let (bytes, executable) = read_capability_file_bounded(
                directory,
                name,
                &display_path,
                remaining,
                limits.max_bytes,
            )?;
            *captured_file_bytes = captured_file_bytes.checked_add(bytes.len() as u64).ok_or(
                SourceResolveError::TooManyBytes {
                    limit: limits.max_bytes,
                },
            )?;
            push_entry(
                entries,
                logical_path,
                SourceEntryKind::File { bytes, executable },
                limits,
            )?;
        } else {
            return Err(SourceResolveError::UnsupportedFileType { path: display_path });
        }
    }
    close_captured_directory(
        directory,
        display_dir,
        &opening_metadata,
        &entry_names_listed,
        &observed_entries,
    )?;
    observations.directories.push(CapturedDirectoryObservation {
        relative_path: logical_dir,
        metadata: opening_metadata,
        listed_names: entry_names_listed,
    });
    Ok(())
}

/// Close one captured directory: the member set must still be exactly the
/// set listed at open — including names the policy excludes from capture,
/// since the isolation premise covers a quiescent tree, not a quiescent
/// captured subset — and every captured entry must still answer at its first
/// identity. An entry created or removed mid-visit changes the member set
/// (and usually the directory's own clocks); one replaced or retargeted in
/// place is caught by the identity compare. `NotFound` on the recheck is
/// drift, not an I/O failure, so it reports the same `LocalSourceChanged`.
fn close_captured_directory(
    directory: &CapabilityDirectory,
    display_dir: &Path,
    opening_metadata: &cap_std::fs::Metadata,
    listed_names: &[OsString],
    observed_entries: &[CapturedEntryObservation],
) -> Result<(), SourceResolveError> {
    let closing_metadata = directory
        .dir_metadata()
        .map_err(|error| io_error(display_dir, error))?;
    require_unchanged_entry(opening_metadata, &closing_metadata, display_dir)?;

    let mut closed_names: Vec<OsString> = Vec::new();
    for entry in directory
        .entries()
        .map_err(|error| io_error(display_dir, error))?
    {
        closed_names.push(
            entry
                .map_err(|error| io_error(display_dir, error))?
                .file_name(),
        );
    }
    closed_names.sort();
    if closed_names != listed_names {
        return Err(SourceResolveError::LocalSourceChanged {
            path: display_dir.to_path_buf(),
        });
    }

    for observation in observed_entries {
        let display_path = display_dir.join(&observation.name);
        let metadata = match directory.symlink_metadata(&observation.name) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(SourceResolveError::LocalSourceChanged { path: display_path });
            }
            Err(error) => return Err(io_error(&display_path, error)),
        };
        require_unchanged_entry(&observation.metadata, &metadata, &display_path)?;
    }
    Ok(())
}

/// The whole-tree counterpart of `close_captured_directory`, run once after
/// the traversal completes: every directory already re-verified itself before
/// its own visit returned, but a member created, removed, replaced or
/// retargeted inside a subtree after that subtree closed is invisible to its
/// parent's close. Re-observe every visited directory's member set and every
/// captured member's identity here so drift landing anywhere in the tree
/// during the capture window still rejects before publication. The per-member
/// re-observation runs first — each directory entry's own kind and identity
/// must still match before its handle is re-opened for the listing compare,
/// so a directory swapped for a link cannot redirect the listing under the
/// retained root capability.
pub(super) fn verify_captured_tree(
    root_directory: &CapabilityDirectory,
    root: &Path,
    observations: &CapturedTreeObservations,
) -> Result<(), SourceResolveError> {
    for member in &observations.members {
        let display_path = root.join(&member.relative_path);
        let metadata = match root_directory.symlink_metadata(&member.relative_path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(SourceResolveError::LocalSourceChanged { path: display_path });
            }
            Err(error) => return Err(io_error(&display_path, error)),
        };
        require_unchanged_entry(&member.metadata, &metadata, &display_path)?;
    }

    for record in &observations.directories {
        let display_path = root.join(&record.relative_path);
        let opened;
        let directory = if record.relative_path.as_os_str().is_empty() {
            root_directory
        } else {
            opened = open_observed_directory(root_directory, &record.relative_path, &display_path)?;
            &opened
        };
        let closing_metadata = directory
            .dir_metadata()
            .map_err(|error| io_error(&display_path, error))?;
        require_unchanged_entry(&record.metadata, &closing_metadata, &display_path)?;

        let mut closed_names: Vec<OsString> = Vec::new();
        for entry in directory
            .entries()
            .map_err(|error| io_error(&display_path, error))?
        {
            closed_names.push(
                entry
                    .map_err(|error| io_error(&display_path, error))?
                    .file_name(),
            );
        }
        closed_names.sort();
        if closed_names != record.listed_names {
            return Err(SourceResolveError::LocalSourceChanged { path: display_path });
        }
    }
    Ok(())
}

/// Re-open a visited directory under the retained root, no-follow at every
/// component: a vanishing or re-kinded component is drift, not an I/O
/// failure, so it reports the same `LocalSourceChanged` the per-directory
/// close uses.
fn open_observed_directory(
    root_directory: &CapabilityDirectory,
    relative_path: &Path,
    display_path: &Path,
) -> Result<CapabilityDirectory, SourceResolveError> {
    let mut opened = root_directory
        .try_clone()
        .map_err(|error| io_error(display_path, error))?;
    for component in relative_path.components() {
        opened = match opened.open_dir_nofollow(component.as_os_str()) {
            Ok(child) => child,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                ) =>
            {
                return Err(SourceResolveError::LocalSourceChanged {
                    path: display_path.to_path_buf(),
                });
            }
            Err(error) => return Err(io_error(display_path, error)),
        };
    }
    Ok(opened)
}

/// Names the capture policy leaves out; identical for the opening listing and
/// the closing membership recheck is not required because the close compares
/// raw membership — an excluded member appearing or vanishing mid-traversal
/// is still drift under the quiescent-tree premise and rejects.
fn excluded_from_capture(policy: SourceTreePolicy, logical_dir: &Path, name: &OsStr) -> bool {
    policy == SourceTreePolicy::LocalPackage
        && (name == ".git"
            || (logical_dir.as_os_str().is_empty()
                && (name == DEFAULT_BUILD_OUTPUT_DIRECTORY || is_root_control_file(name))))
}

fn read_and_validate_symlink_target(
    root_directory: &CapabilityDirectory,
    root: &Path,
    directory: &CapabilityDirectory,
    logical_directory: &Path,
    name: &OsStr,
    link: &Path,
    policy: SourceTreePolicy,
) -> Result<PathBuf, SourceResolveError> {
    let raw_target = directory
        .read_link_contents(name)
        .map_err(|error| io_error(link, error))?;
    if raw_target.is_absolute() {
        return Err(SourceResolveError::SymlinkEscapesRoot {
            link: link.to_path_buf(),
            target: raw_target,
        });
    }
    let target_request = logical_directory.join(&raw_target);
    let target_display = root.join(&target_request);
    let relative_target = root_directory.canonicalize(&target_request).map_err(|_| {
        SourceResolveError::SymlinkEscapesRoot {
            link: link.to_path_buf(),
            target: target_display,
        }
    })?;
    if policy == SourceTreePolicy::LocalPackage
        && (relative_target
            .components()
            .any(|component| component.as_os_str() == ".git")
            || relative_target
                .components()
                .next()
                .is_some_and(|component| is_root_control_file(component.as_os_str())))
    {
        return Err(SourceResolveError::SymlinkTargetsExcludedMetadata {
            link: link.to_path_buf(),
            target: root.join(&relative_target),
        });
    }
    if policy == SourceTreePolicy::LocalPackage
        && relative_target
            .components()
            .next()
            .is_some_and(|component| component.as_os_str() == DEFAULT_BUILD_OUTPUT_DIRECTORY)
    {
        return Err(SourceResolveError::SymlinkTargetsExcludedBuildOutput {
            link: link.to_path_buf(),
            target: root.join(&relative_target),
        });
    }
    Ok(raw_target)
}

fn is_root_control_file(name: &OsStr) -> bool {
    // Package locks and compiler admission policy are not package source inputs.
    // Reserve case variants too so the rule agrees on case-folding filesystems.
    name.as_encoded_bytes().eq_ignore_ascii_case(b"omega.lock")
        || name
            .as_encoded_bytes()
            .eq_ignore_ascii_case(b"omega.admissions")
}

fn push_entry(
    entries: &mut Vec<SourceEntry>,
    relative: PathBuf,
    kind: SourceEntryKind,
    limits: LocalSourceLimits,
) -> Result<(), SourceResolveError> {
    if entries.len() >= limits.max_entries {
        return Err(SourceResolveError::TooManyFiles {
            limit: limits.max_entries,
        });
    }
    entries.push(SourceEntry {
        relative_bytes: canonical_relative_path_bytes(&relative),
        relative_path: relative,
        kind,
    });
    Ok(())
}

fn canonical_relative_path_bytes(relative: &Path) -> Vec<u8> {
    let mut encoded = Vec::new();
    for component in relative.components() {
        if !encoded.is_empty() {
            encoded.push(b'/');
        }
        encoded.extend_from_slice(raw_os_bytes(component.as_os_str()).as_slice());
    }
    encoded
}

#[cfg(test)]
mod close_tests {
    #[cfg(unix)]
    use std::ffi::OsStr;
    use std::ffi::OsString;
    use std::path::PathBuf;

    use cap_std::ambient_authority;
    use cap_std::fs::Dir as CapabilityDirectory;

    use super::close_captured_directory;
    use crate::SourceResolveError;
    use crate::test_support::temp_root;
    use crate::tree::filesystem::CapturedEntryObservation;

    struct DirectoryFixture {
        root: PathBuf,
        directory: CapabilityDirectory,
    }

    impl DirectoryFixture {
        fn new(name: &str, members: &[&str]) -> Self {
            let root = temp_root(name);
            std::fs::create_dir_all(&root).expect("create source fixture");
            for member in members {
                std::fs::write(root.join(member), b"captured bytes").expect("write member");
            }
            let directory = CapabilityDirectory::open_ambient_dir(&root, ambient_authority())
                .expect("retain fixture directory");
            Self { root, directory }
        }

        fn observe(&self) -> DirectoryObservation {
            let opening = self.directory.dir_metadata().expect("observe directory");
            let mut listed: Vec<OsString> = self
                .directory
                .entries()
                .expect("list fixture")
                .map(|entry| entry.expect("read fixture entry").file_name())
                .collect();
            listed.sort();
            let observed: Vec<CapturedEntryObservation> = listed
                .iter()
                .map(|name| CapturedEntryObservation {
                    name: name.clone(),
                    metadata: self
                        .directory
                        .symlink_metadata(name)
                        .expect("observe fixture member"),
                })
                .collect();
            DirectoryObservation {
                opening,
                listed,
                observed,
            }
        }

        fn close(&self, observation: &DirectoryObservation) -> Result<(), SourceResolveError> {
            close_captured_directory(
                &self.directory,
                &self.root,
                &observation.opening,
                &observation.listed,
                &observation.observed,
            )
        }
    }

    impl Drop for DirectoryFixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    struct DirectoryObservation {
        opening: cap_std::fs::Metadata,
        listed: Vec<OsString>,
        observed: Vec<CapturedEntryObservation>,
    }

    #[test]
    fn unchanged_directory_closes_cleanly() {
        let fixture = DirectoryFixture::new("close-unchanged", &["a.omg", "b.omg"]);
        fixture
            .close(&fixture.observe())
            .expect("a quiescent directory closes");
    }

    #[test]
    fn member_added_during_visit_rejects() {
        let fixture = DirectoryFixture::new("close-added", &["a.omg"]);
        let observation = fixture.observe();
        std::fs::write(fixture.root.join("late.omg"), b"late").expect("add member");
        assert!(matches!(
            fixture.close(&observation),
            Err(SourceResolveError::LocalSourceChanged { .. })
        ));
    }

    #[test]
    fn member_removed_during_visit_rejects() {
        let fixture = DirectoryFixture::new("close-removed", &["a.omg", "b.omg"]);
        let observation = fixture.observe();
        std::fs::remove_file(fixture.root.join("b.omg")).expect("remove member");
        assert!(matches!(
            fixture.close(&observation),
            Err(SourceResolveError::LocalSourceChanged { .. })
        ));
    }

    #[test]
    fn member_replaced_in_place_rejects() {
        // The replacement rewrites the member with different content: hosts
        // that recycle inode numbers inside a timestamp tick cannot hide the
        // replacement behind an identical observation.
        let fixture = DirectoryFixture::new("close-replaced", &["a.omg", "b.omg"]);
        let observation = fixture.observe();
        std::fs::remove_file(fixture.root.join("b.omg")).expect("remove member");
        std::fs::write(fixture.root.join("b.omg"), b"replacement with new length")
            .expect("replace member");
        assert!(matches!(
            fixture.close(&observation),
            Err(SourceResolveError::LocalSourceChanged { .. })
        ));
    }

    #[test]
    fn membership_restored_after_churn_still_rejects() {
        // Create and remove a transient member, then restore the directory's
        // modified time: the closing listing equals the opening listing and
        // the directory clock is the only observation the churn can move.
        // (Churn completed inside one filesystem timestamp tick is invisible
        // to any metadata-level check and is not covered here.)
        let fixture = DirectoryFixture::new("close-churn", &["a.omg"]);
        let observation = fixture.observe();
        std::fs::write(fixture.root.join("transient.omg"), b"temp").expect("create transient");
        std::fs::remove_file(fixture.root.join("transient.omg")).expect("remove transient");
        let directory_handle = std::fs::File::open(&fixture.root).expect("open fixture dir");
        directory_handle
            .set_times(std::fs::FileTimes::new().set_modified(
                std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1),
            ))
            .expect("move directory clock");
        assert!(matches!(
            fixture.close(&observation),
            Err(SourceResolveError::LocalSourceChanged { .. })
        ));
    }

    #[test]
    fn renamed_member_rejects() {
        let fixture = DirectoryFixture::new("close-renamed", &["a.omg", "b.omg"]);
        let observation = fixture.observe();
        std::fs::rename(fixture.root.join("a.omg"), fixture.root.join("renamed.omg"))
            .expect("rename member");
        assert!(matches!(
            fixture.close(&observation),
            Err(SourceResolveError::LocalSourceChanged { .. })
        ));
    }

    #[test]
    fn replaced_child_directory_rejects() {
        let root = temp_root("close-replaced-dir");
        std::fs::create_dir_all(root.join("child")).expect("create child");
        std::fs::write(root.join("child").join("a.omg"), b"captured").expect("write member");
        let fixture = {
            let directory = CapabilityDirectory::open_ambient_dir(&root, ambient_authority())
                .expect("retain fixture directory");
            DirectoryFixture {
                root: root.clone(),
                directory,
            }
        };
        let observation = fixture.observe();
        std::fs::remove_dir_all(root.join("child")).expect("remove child");
        std::fs::create_dir_all(root.join("child")).expect("recreate child");
        std::fs::write(root.join("child").join("a.omg"), b"captured").expect("rewrite member");
        // Inode numbers and both clocks can repeat inside one filesystem
        // tick; a permissions change keeps the replacement visible on them.
        let original = std::fs::metadata(root.join("child"))
            .expect("read child metadata")
            .permissions();
        let mut readonly = original.clone();
        readonly.set_readonly(true);
        std::fs::set_permissions(root.join("child"), readonly).expect("mark child readonly");
        assert!(matches!(
            fixture.close(&observation),
            Err(SourceResolveError::LocalSourceChanged { .. })
        ));
        // Restore the mode the directory actually had. Clearing the readonly
        // bit instead would hand it back world-writable on Unix, which is
        // both wrong and what `permissions_set_readonly_false` rejects.
        std::fs::set_permissions(root.join("child"), original).expect("restore child permissions");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn retargeted_link_rejects() {
        let root = temp_root("close-retargeted-link");
        std::fs::create_dir_all(&root).expect("create fixture");
        std::fs::write(root.join("one.omg"), b"one").expect("write target");
        std::fs::write(root.join("longer-two.omg"), b"two").expect("write target");
        std::os::unix::fs::symlink("one.omg", root.join("link.omg")).expect("create link");
        let fixture = {
            let directory = CapabilityDirectory::open_ambient_dir(&root, ambient_authority())
                .expect("retain fixture directory");
            DirectoryFixture {
                root: root.clone(),
                directory,
            }
        };
        let observation = fixture.observe();
        std::fs::remove_file(root.join("link.omg")).expect("unlink");
        // A longer target keeps the test deterministic on hosts that recycle
        // inode numbers and timestamps inside a single clock tick.
        std::os::unix::fs::symlink("longer-two.omg", root.join("link.omg")).expect("retarget link");
        assert!(matches!(
            fixture.close(&observation),
            Err(SourceResolveError::LocalSourceChanged { .. })
        ));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn member_rewritten_in_place_with_restored_observations_rejects() {
        // A same-inode rewrite restoring every compared indicator — type,
        // length, permissions and the recorded modification time — still
        // moves the kernel-managed change time, so the closing entry
        // comparison rejects it. Editing a member's bytes does not move its
        // parent's clock, so only the per-entry recheck can see this drift.
        use std::io::Write;

        let fixture = DirectoryFixture::new("close-in-place", &["a.omg", "b.omg"]);
        let observation = fixture.observe();
        // Kernel change time moves only when the edit lands in a later
        // filesystem timestamp tick; coarse-clock hosts quantize metadata
        // timestamps to jiffies, so the mutation has to wait out a tick.
        std::thread::sleep(std::time::Duration::from_millis(50));
        let member = fixture.root.join("b.omg");
        let recorded = fixture
            .directory
            .symlink_metadata(OsStr::new("b.omg"))
            .expect("record member metadata");
        let mut writer = std::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&member)
            .expect("rewrite member in place");
        writer
            .write_all(b"replaced bytes")
            .expect("same-length replacement");
        writer
            .set_times(
                std::fs::FileTimes::new()
                    .set_modified(recorded.modified().expect("recorded modified").into_std()),
            )
            .expect("restore the recorded modification time");
        assert!(matches!(
            fixture.close(&observation),
            Err(SourceResolveError::LocalSourceChanged { .. })
        ));
    }

    /// The link-retarget and in-place-restore rejection legs above depend on
    /// unix symlink spelling and kernel change time; neither exists on this
    /// host. Record the absent coverage explicitly rather than letting a
    /// non-unix run drop it silently.
    #[cfg(not(unix))]
    #[test]
    fn unix_close_recheck_legs_recorded_as_unavailable_on_this_host() {
        eprintln!(
            "SKIP: link-retarget and in-place-restore close-recheck coverage \
             requires a unix host (symlink spelling and kernel change time)"
        );
    }
}

#[cfg(test)]
mod tree_tests {
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};

    use cap_fs_ext::DirExt;
    use cap_std::ambient_authority;
    use cap_std::fs::Dir as CapabilityDirectory;

    use super::{
        CapturedDirectoryObservation, CapturedMemberObservation, CapturedTreeObservations,
        verify_captured_tree,
    };
    use crate::SourceResolveError;
    use crate::test_support::temp_root;

    struct TreeFixture {
        root: PathBuf,
        root_directory: CapabilityDirectory,
    }

    impl TreeFixture {
        /// Builds `root/` with `a.omg` at the top level and
        /// `sub/{b.omg,nested/c.omg}` beneath it.
        fn new(name: &str) -> Self {
            let root = temp_root(name);
            std::fs::create_dir_all(root.join("sub/nested")).expect("create nested fixture");
            std::fs::write(root.join("a.omg"), b"root bytes").expect("write root member");
            std::fs::write(root.join("sub/b.omg"), b"nested bytes").expect("write nested member");
            std::fs::write(root.join("sub/nested/c.omg"), b"deep bytes")
                .expect("write deep member");
            let root_directory = CapabilityDirectory::open_ambient_dir(&root, ambient_authority())
                .expect("retain fixture root");
            Self {
                root,
                root_directory,
            }
        }

        /// Mirrors `visit_directory`'s accumulation: every visited directory's
        /// opening metadata and member listing, and every member's first
        /// no-follow observation, in traversal order.
        fn observe(&self) -> CapturedTreeObservations {
            let mut observations = CapturedTreeObservations::default();
            Self::observe_directory(&self.root_directory, Path::new(""), &mut observations);
            observations
        }

        fn observe_directory(
            directory: &CapabilityDirectory,
            relative: &Path,
            observations: &mut CapturedTreeObservations,
        ) {
            let mut listed: Vec<OsString> = directory
                .entries()
                .expect("list directory")
                .map(|entry| entry.expect("read directory entry").file_name())
                .collect();
            listed.sort();
            observations.directories.push(CapturedDirectoryObservation {
                relative_path: relative.to_path_buf(),
                metadata: directory.dir_metadata().expect("observe directory"),
                listed_names: listed.clone(),
            });
            for name in listed {
                let child_relative = relative.join(&name);
                let metadata = directory.symlink_metadata(&name).expect("observe member");
                observations.members.push(CapturedMemberObservation {
                    relative_path: child_relative.clone(),
                    metadata: metadata.clone(),
                });
                if metadata.is_dir() {
                    let child = directory.open_dir_nofollow(&name).expect("open member dir");
                    Self::observe_directory(&child, &child_relative, observations);
                }
            }
        }

        fn verify(
            &self,
            observations: &CapturedTreeObservations,
        ) -> Result<(), SourceResolveError> {
            verify_captured_tree(&self.root_directory, &self.root, observations)
        }
    }

    impl Drop for TreeFixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn unchanged_tree_verifies() {
        let fixture = TreeFixture::new("tree-unchanged");
        let observations = fixture.observe();
        fixture
            .verify(&observations)
            .expect("quiescent tree verifies");
    }

    #[test]
    fn member_added_to_already_closed_subtree_rejects() {
        // The subtree's own close already ran: only the final whole-tree
        // sweep can see the new member.
        let fixture = TreeFixture::new("tree-added-subtree");
        let observations = fixture.observe();
        std::fs::write(fixture.root.join("sub/new.omg"), b"late member")
            .expect("add member after close");
        assert!(matches!(
            fixture.verify(&observations),
            Err(SourceResolveError::LocalSourceChanged { .. })
        ));
    }

    #[test]
    fn member_removed_from_root_rejects() {
        let fixture = TreeFixture::new("tree-removed-root");
        let observations = fixture.observe();
        std::fs::remove_file(fixture.root.join("a.omg")).expect("remove root member");
        assert!(matches!(
            fixture.verify(&observations),
            Err(SourceResolveError::LocalSourceChanged { .. })
        ));
    }

    #[test]
    fn member_rewritten_in_already_closed_subtree_rejects() {
        // Rewriting bytes in place keeps the member's name and kind; the
        // moved modification/change indicators carry the drift.
        let fixture = TreeFixture::new("tree-rewritten-subtree");
        let observations = fixture.observe();
        std::fs::write(fixture.root.join("sub/b.omg"), b"rewritten bytes longer")
            .expect("rewrite member in place");
        assert!(matches!(
            fixture.verify(&observations),
            Err(SourceResolveError::LocalSourceChanged { .. })
        ));
    }

    #[test]
    fn child_directory_replaced_after_its_close_rejects() {
        let fixture = TreeFixture::new("tree-replaced-dir");
        let observations = fixture.observe();
        std::fs::remove_dir_all(fixture.root.join("sub/nested")).expect("remove nested dir");
        std::fs::create_dir_all(fixture.root.join("sub/nested")).expect("recreate nested dir");
        assert!(matches!(
            fixture.verify(&observations),
            Err(SourceResolveError::LocalSourceChanged { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn link_retargeted_in_already_closed_subtree_rejects() {
        let fixture = TreeFixture::new("tree-retargeted-link");
        std::os::unix::fs::symlink("../a.omg", fixture.root.join("sub/linked.omg"))
            .expect("create fixture link");
        let observations = fixture.observe();
        std::fs::remove_file(fixture.root.join("sub/linked.omg")).expect("remove link");
        std::os::unix::fs::symlink("b.omg", fixture.root.join("sub/linked.omg"))
            .expect("retarget link");
        assert!(matches!(
            fixture.verify(&observations),
            Err(SourceResolveError::LocalSourceChanged { .. })
        ));
    }
}
