/*
No-follow source operations and closing drift sweeps.

Capture retains the selected members' opening identities. Enumerated subtrees
also retain their complete member lists, including control names omitted by the
source policy. Closing checks compare those identities and member lists before
publication. Ancestors of explicitly selected files retain directory identities
without enumerating unrelated siblings. The final sweep checks member identities
first and reopens every selected directory without following symlinks, so a
replaced ancestor cannot redirect later metadata or membership observations.
*/

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use cap_std::fs::Dir as CapabilityDirectory;

use super::traversal_observations::{SourceEntry, SourceEntryKind, SourceTreePolicy};
use crate::SourceResolveError;
use crate::error::{local_source_changed, too_many_files};
use crate::limits::{DEFAULT_BUILD_OUTPUT_DIRECTORY, LocalSourceLimits};
use crate::tree::filesystem::{
    CapturedEntryObservation, IoResultAt, io_error, open_captured_directory, raw_os_bytes,
    require_unchanged_entry,
};

pub(super) struct CapturedDirectoryObservation {
    pub(super) relative_path: PathBuf,
    pub(super) metadata: cap_std::fs::Metadata,
    pub(super) listed_names: Option<Vec<OsString>>,
}

pub(super) struct CapturedMemberObservation {
    pub(super) relative_path: PathBuf,
    pub(super) metadata: cap_std::fs::Metadata,
}

#[derive(Default)]
pub(super) struct CapturedTreeObservations {
    pub(super) directories: Vec<CapturedDirectoryObservation>,
    pub(super) members: Vec<CapturedMemberObservation>,
}

pub(super) fn close_captured_directory(
    directory: &CapabilityDirectory,
    display_dir: &Path,
    opening_metadata: &cap_std::fs::Metadata,
    listed_names: &[OsString],
    observed_entries: &[CapturedEntryObservation],
) -> Result<(), SourceResolveError> {
    let closing_metadata = directory.dir_metadata().at(display_dir)?;
    require_unchanged_entry(opening_metadata, &closing_metadata, display_dir)?;

    let mut closed_names: Vec<OsString> = Vec::new();
    for entry in directory.entries().at(display_dir)? {
        closed_names.push(entry.at(display_dir)?.file_name());
    }
    closed_names.sort();
    if closed_names != listed_names {
        return Err(local_source_changed(display_dir));
    }

    for observation in observed_entries {
        let display_path = display_dir.join(&observation.name);
        let metadata = match directory.symlink_metadata(&observation.name) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(local_source_changed(&display_path));
            }
            Err(error) => return Err(io_error(&display_path, error)),
        };
        require_unchanged_entry(&observation.metadata, &metadata, &display_path)?;
    }
    Ok(())
}

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
                return Err(local_source_changed(&display_path));
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
        let closing_metadata = directory.dir_metadata().at(&display_path)?;
        require_unchanged_entry(&record.metadata, &closing_metadata, &display_path)?;

        if let Some(listed_names) = &record.listed_names {
            let mut closed_names: Vec<OsString> = Vec::new();
            for entry in directory.entries().at(&display_path)? {
                closed_names.push(entry.at(&display_path)?.file_name());
            }
            closed_names.sort();
            if closed_names != *listed_names {
                return Err(local_source_changed(&display_path));
            }
        }
    }
    Ok(())
}

pub(super) fn open_observed_directory(
    root_directory: &CapabilityDirectory,
    relative_path: &Path,
    display_path: &Path,
) -> Result<CapabilityDirectory, SourceResolveError> {
    let mut opened = root_directory.try_clone().at(display_path)?;
    for component in relative_path.components() {
        opened = match open_captured_directory(&opened, component.as_os_str(), display_path) {
            Ok(child) => child,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                ) =>
            {
                return Err(local_source_changed(display_path));
            }
            Err(error) => return Err(io_error(display_path, error)),
        };
    }
    Ok(opened)
}

pub(crate) fn excluded_from_capture(
    policy: SourceTreePolicy,
    logical_dir: &Path,
    name: &OsStr,
) -> bool {
    policy == SourceTreePolicy::LocalPackage
        && (name == ".git"
            || (logical_dir.as_os_str().is_empty()
                && (name == DEFAULT_BUILD_OUTPUT_DIRECTORY || is_root_control_file(name))))
}

pub(super) fn read_and_validate_symlink_target(
    root_directory: &CapabilityDirectory,
    root: &Path,
    directory: &CapabilityDirectory,
    logical_directory: &Path,
    name: &OsStr,
    link: &Path,
    policy: SourceTreePolicy,
) -> Result<PathBuf, SourceResolveError> {
    let raw_target = directory.read_link_contents(name).at(link)?;
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
    name.as_encoded_bytes().eq_ignore_ascii_case(b"omega.lock")
}

pub(super) fn push_entry(
    entries: &mut Vec<SourceEntry>,
    relative: PathBuf,
    kind: SourceEntryKind,
    limits: LocalSourceLimits,
) -> Result<(), SourceResolveError> {
    if entries.len() >= limits.max_entries {
        return Err(too_many_files(limits.max_entries));
    }
    entries.push(SourceEntry {
        relative_bytes: canonical_relative_path_bytes(&relative),
        relative_path: relative,
        kind,
    });
    Ok(())
}

pub(super) fn canonical_relative_path_bytes(relative: &Path) -> Vec<u8> {
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
    fn member_added_during_visit_rejects() {
        let fixture = DirectoryFixture::new("close-added", &["a.omg"]);
        let observation = fixture.observe();
        std::fs::write(fixture.root.join("late.omg"), b"late").expect("add member");
        assert!(matches!(
            fixture.close(&observation),
            Err(SourceResolveError::LocalSourceChanged { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn member_rewritten_in_place_with_restored_observations_rejects() {
        use std::io::Write;

        let fixture = DirectoryFixture::new("close-in-place", &["a.omg", "b.omg"]);
        let observation = fixture.observe();
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
    use cap_fs_ext::DirExt;
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};

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
                listed_names: Some(listed.clone()),
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
}
