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
//! retained handle. A mutation landing after a directory's own close is only
//! visible to the resolver's later live-tree comparison — the sweep narrows
//! the drift window, it does not make the whole tree atomic.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use cap_std::fs::Dir as CapabilityDirectory;

use super::traversal_observations::{SourceEntry, SourceEntryKind, SourceTreePolicy};
use crate::SourceResolveError;
use crate::limits::{DEFAULT_BUILD_OUTPUT_DIRECTORY, LocalSourceLimits};
use crate::tree::filesystem::{
    CapturedEntryObservation, io_error, open_captured_directory, raw_os_bytes,
    read_capability_file_bounded, require_unchanged_entry,
};

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
    )
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
    use super::*;
    use crate::test_support::temp_root;
    use cap_std::ambient_authority;

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
        let mut readonly = std::fs::metadata(root.join("child"))
            .expect("read child metadata")
            .permissions();
        readonly.set_readonly(true);
        std::fs::set_permissions(root.join("child"), readonly).expect("mark child readonly");
        assert!(matches!(
            fixture.close(&observation),
            Err(SourceResolveError::LocalSourceChanged { .. })
        ));
        let mut writable = std::fs::metadata(root.join("child"))
            .expect("read child metadata")
            .permissions();
        writable.set_readonly(false);
        std::fs::set_permissions(root.join("child"), writable).expect("restore child permissions");
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
}
