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

use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use cap_std::fs::Dir as CapabilityDirectory;

use super::traversal_observations::{SourceEntry, SourceEntryKind, SourceTreePolicy};
use crate::SourceResolveError;
use crate::error::{local_source_changed, too_many_files};
use crate::limits::{DEFAULT_BUILD_OUTPUT_DIRECTORY, LocalSourceLimits};
#[cfg(test)]
use crate::tree::filesystem::CapturedEntryObservation;
use crate::tree::filesystem::{
    IoResultAt, io_error, open_captured_directory, raw_os_bytes, require_unchanged_entry,
};

/// Cap retained capability directory handles so deep or wide trees never press
/// against host file-descriptor limits while still avoiding per-file root walks.
pub(super) const MAX_OPEN_DIRECTORIES: usize = 256;

/// Cap parallel OS filesystem workers. On Windows, kernel file-object creation
/// peaks around 4 threads and regresses past 6-8 threads due to contention.
pub(super) fn capture_worker_count(work_items: usize) -> usize {
    if work_items < 2 {
        return 1;
    }
    #[cfg(windows)]
    const MAX_WORKERS: usize = 4;
    #[cfg(not(windows))]
    const MAX_WORKERS: usize = 8;

    std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .clamp(1, MAX_WORKERS)
        .min(work_items)
}

/// Shared dynamic work stack for single-scope parallel tree traversal.
/// Workers pop work, push newly discovered children, and exit once no items
/// remain and no other worker is active.
pub(super) struct WorkQueue<T> {
    state: Mutex<QueueState<T>>,
    ready: Condvar,
}

struct QueueState<T> {
    items: Vec<T>,
    active: usize,
    closed: bool,
}

impl<T> WorkQueue<T> {
    pub(super) fn new(initial: Vec<T>) -> Self {
        Self {
            state: Mutex::new(QueueState {
                items: initial,
                active: 0,
                closed: false,
            }),
            ready: Condvar::new(),
        }
    }

    pub(super) fn push(&self, item: T) {
        let mut state = self.state.lock().expect("work queue lock poisoned");
        if !state.closed {
            state.items.push(item);
            self.ready.notify_one();
        }
    }

    pub(super) fn push_many(&self, items: impl IntoIterator<Item = T>) {
        let mut state = self.state.lock().expect("work queue lock poisoned");
        if !state.closed {
            let before = state.items.len();
            state.items.extend(items);
            let added = state.items.len() - before;
            if added == 1 {
                self.ready.notify_one();
            } else if added > 1 {
                self.ready.notify_all();
            }
        }
    }

    pub(super) fn cancel(&self) {
        let mut state = self.state.lock().expect("work queue lock poisoned");
        state.closed = true;
        state.items.clear();
        self.ready.notify_all();
    }

    pub(super) fn pop(&self) -> Option<T> {
        let mut state = self.state.lock().expect("work queue lock poisoned");
        loop {
            if state.closed {
                return None;
            }
            if let Some(item) = state.items.pop() {
                state.active += 1;
                return Some(item);
            }
            if state.active == 0 {
                state.closed = true;
                self.ready.notify_all();
                return None;
            }
            state = self.ready.wait(state).expect("work queue lock poisoned");
        }
    }

    pub(super) fn finish_one(&self) {
        let mut state = self.state.lock().expect("work queue lock poisoned");
        state.active -= 1;
        if state.active == 0 && state.items.is_empty() {
            state.closed = true;
            self.ready.notify_all();
        }
    }
}

pub(super) struct OpenDirectoryHandles {
    root: Arc<CapabilityDirectory>,
    entries: HashMap<PathBuf, Arc<CapabilityDirectory>>,
}

impl OpenDirectoryHandles {
    pub(super) fn new(
        root_directory: &CapabilityDirectory,
        root_display: &Path,
    ) -> Result<Self, SourceResolveError> {
        Ok(Self {
            root: Arc::new(root_directory.try_clone().at(root_display)?),
            entries: HashMap::new(),
        })
    }

    pub(super) fn open(
        &mut self,
        relative_path: &Path,
        root_display: &Path,
    ) -> Result<Arc<CapabilityDirectory>, SourceResolveError> {
        if relative_path.as_os_str().is_empty() {
            return Ok(Arc::clone(&self.root));
        }
        if let Some(directory) = self.entries.get(relative_path) {
            return Ok(Arc::clone(directory));
        }
        if self.entries.len() >= MAX_OPEN_DIRECTORIES {
            self.entries.clear();
        }
        let parent = relative_path.parent().unwrap_or(Path::new(""));
        let parent_directory = self.open(parent, root_display)?;
        let leaf = relative_path
            .file_name()
            .expect("nonempty directory relative path");
        let display_path = root_display.join(relative_path);
        let child = open_verified_child(&parent_directory, leaf, &display_path)?;
        self.entries
            .insert(relative_path.to_path_buf(), Arc::clone(&child));
        Ok(child)
    }
}

pub(super) fn open_verified_child(
    parent_directory: &CapabilityDirectory,
    leaf: &OsStr,
    display_path: &Path,
) -> Result<Arc<CapabilityDirectory>, SourceResolveError> {
    match open_captured_directory(parent_directory, leaf, display_path) {
        Ok(child) => Ok(Arc::new(child)),
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
            ) =>
        {
            Err(local_source_changed(display_path))
        }
        Err(error) => Err(io_error(display_path, error)),
    }
}

fn open_from_root(
    root_directory: &CapabilityDirectory,
    relative_path: &Path,
    display_path: &Path,
) -> Result<Arc<CapabilityDirectory>, SourceResolveError> {
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
    Ok(Arc::new(opened))
}

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

#[cfg(test)]
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

enum VerifyTask<'a> {
    Directory {
        parent_directory: Option<Arc<CapabilityDirectory>>,
        record: &'a CapturedDirectoryObservation,
    },
    MemberBatch {
        directory: Arc<CapabilityDirectory>,
        members: Vec<&'a CapturedMemberObservation>,
    },
}

pub(super) fn verify_captured_tree(
    root_directory: &CapabilityDirectory,
    root: &Path,
    observations: &CapturedTreeObservations,
) -> Result<(), SourceResolveError> {
    let mut members_by_parent: HashMap<&Path, Vec<&CapturedMemberObservation>> = HashMap::new();
    for member in &observations.members {
        let parent = member.relative_path.parent().unwrap_or(Path::new(""));
        members_by_parent.entry(parent).or_default().push(member);
    }

    let mut dir_set: HashMap<&Path, &CapturedDirectoryObservation> = HashMap::new();
    for record in &observations.directories {
        dir_set.insert(record.relative_path.as_path(), record);
    }
    let mut children_by_parent: HashMap<&Path, Vec<&CapturedDirectoryObservation>> = HashMap::new();
    let mut roots: Vec<&CapturedDirectoryObservation> = Vec::new();
    for record in &observations.directories {
        if record.relative_path.as_os_str().is_empty() {
            roots.push(record);
        } else {
            let parent = record.relative_path.parent().unwrap_or(Path::new(""));
            if dir_set.contains_key(parent) {
                children_by_parent.entry(parent).or_default().push(record);
            } else {
                roots.push(record);
            }
        }
    }

    let root_arc = Arc::new(root_directory.try_clone().at(root)?);
    let mut initial_tasks: Vec<VerifyTask<'_>> = roots
        .into_iter()
        .map(|record| VerifyTask::Directory {
            parent_directory: None,
            record,
        })
        .collect();

    // Handle any orphan member observations whose parent was not recorded in
    // `observations.directories` (only possible in synthetic unit fixtures).
    let mut orphan_members: Vec<(&Path, Vec<&CapturedMemberObservation>)> = Vec::new();
    for (parent, members) in &members_by_parent {
        if !dir_set.contains_key(parent) {
            orphan_members.push((*parent, members.clone()));
        }
    }

    let total_work = observations.directories.len() + observations.members.len();
    let workers = if total_work < 32 {
        1
    } else {
        capture_worker_count(total_work)
    };

    let context = VerifyContext {
        root_directory,
        root_arc: &root_arc,
        root,
        children_by_parent: &children_by_parent,
        members_by_parent: &members_by_parent,
        parallel_members: workers > 1,
    };

    if workers <= 1 {
        let queue = WorkQueue::new(initial_tasks);
        while let Some(task) = queue.pop() {
            let outcome = process_verify_task(task, &queue, &context);
            queue.finish_one();
            outcome?;
        }
    } else {
        // Reversing keeps the first root at the top of the Vec stack.
        initial_tasks.reverse();
        let queue = WorkQueue::new(initial_tasks);
        let failed = AtomicBool::new(false);
        let errors: Mutex<Vec<SourceResolveError>> = Mutex::new(Vec::new());
        std::thread::scope(|scope| {
            for _ in 0..workers {
                scope.spawn(|| {
                    while let Some(task) = queue.pop() {
                        if !failed.load(Ordering::Relaxed)
                            && let Err(error) = process_verify_task(task, &queue, &context)
                        {
                            failed.store(true, Ordering::Relaxed);
                            queue.cancel();
                            errors.lock().expect("error lock poisoned").push(error);
                        }
                        queue.finish_one();
                    }
                });
            }
        });
        let mut errors = errors.into_inner().expect("error lock poisoned");
        if !errors.is_empty() {
            return Err(errors.remove(0));
        }
    }

    if !orphan_members.is_empty() {
        let mut open_dirs = OpenDirectoryHandles::new(root_directory, root)?;
        for (parent, members) in orphan_members {
            let directory = open_dirs.open(parent, root)?;
            verify_member_slice(&directory, root, &members)?;
        }
    }
    Ok(())
}

struct VerifyContext<'a, 'b> {
    root_directory: &'b CapabilityDirectory,
    root_arc: &'b Arc<CapabilityDirectory>,
    root: &'b Path,
    children_by_parent: &'b HashMap<&'a Path, Vec<&'a CapturedDirectoryObservation>>,
    members_by_parent: &'b HashMap<&'a Path, Vec<&'a CapturedMemberObservation>>,
    parallel_members: bool,
}

fn process_verify_task<'a>(
    task: VerifyTask<'a>,
    queue: &WorkQueue<VerifyTask<'a>>,
    context: &VerifyContext<'a, '_>,
) -> Result<(), SourceResolveError> {
    match task {
        VerifyTask::Directory {
            parent_directory,
            record,
        } => {
            let display_path = context.root.join(&record.relative_path);
            let directory = if record.relative_path.as_os_str().is_empty() {
                Arc::clone(context.root_arc)
            } else if let Some(parent_directory) = parent_directory {
                let leaf = record
                    .relative_path
                    .file_name()
                    .expect("nonempty directory relative path");
                open_verified_child(&parent_directory, leaf, &display_path)?
            } else {
                open_from_root(context.root_directory, &record.relative_path, &display_path)?
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

            if let Some(children) = context
                .children_by_parent
                .get(record.relative_path.as_path())
            {
                queue.push_many(children.iter().map(|child| VerifyTask::Directory {
                    parent_directory: Some(Arc::clone(&directory)),
                    record: child,
                }));
            }

            if let Some(members) = context
                .members_by_parent
                .get(record.relative_path.as_path())
            {
                if context.parallel_members && members.len() >= 32 {
                    queue.push_many(members.chunks(16).map(|batch| VerifyTask::MemberBatch {
                        directory: Arc::clone(&directory),
                        members: batch.to_vec(),
                    }));
                } else {
                    verify_member_slice(&directory, context.root, members)?;
                }
            }
            Ok(())
        }
        VerifyTask::MemberBatch { directory, members } => {
            verify_member_slice(&directory, context.root, &members)
        }
    }
}

fn verify_member_slice(
    directory: &CapabilityDirectory,
    root: &Path,
    members: &[&CapturedMemberObservation],
) -> Result<(), SourceResolveError> {
    for member in members {
        let display_path = root.join(&member.relative_path);
        let name = member
            .relative_path
            .file_name()
            .expect("nonempty observed member path");
        let metadata = match directory.symlink_metadata(name) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(local_source_changed(&display_path));
            }
            Err(error) => return Err(io_error(&display_path, error)),
        };
        require_unchanged_entry(&member.metadata, &metadata, &display_path)?;
    }
    Ok(())
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

pub(crate) fn canonical_relative_path_bytes(relative: &Path) -> Vec<u8> {
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
