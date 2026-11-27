//! Select and capture the requested source namespace in a single scoped pass.
//!
//! Explicitly named paths from a scoped `SourceCaptureRequest` are resolved
//! without enumerating unselected siblings; selected directories and their
//! files are then traversed through a shared `WorkQueue` inside one scoped
//! worker pool so there are no per-level thread-spawn barriers, while opening
//! regular files and subdirectories directly with no-follow handles.

use std::collections::{BTreeMap, HashSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::selection::{SourceCaptureObligation, SourceCaptureRequest, SourceSelectionError};
use cap_std::fs::Dir as CapabilityDirectory;

use super::traversal::{
    CapturedDirectoryObservation, CapturedMemberObservation, CapturedTreeObservations,
    OpenDirectoryHandles, WorkQueue, canonical_relative_path_bytes, capture_worker_count,
    excluded_from_capture, open_verified_child, push_entry, read_and_validate_symlink_target,
};
use super::traversal_observations::{SourceEntry, SourceEntryKind, SourceTreePolicy};
use crate::SourceResolveError;
use crate::error::{local_source_changed, too_many_bytes, too_many_files};
use crate::limits::{LocalSourceLimits, SOURCE_BYTE_ABSOLUTE_LIMIT};
use crate::tree::filesystem::{
    FILE_READ_BUFFER_BYTES, IoResultAt, io_error, read_capability_file_observed,
    require_unchanged_entry,
};

struct SharedBudget<'a> {
    root: &'a Path,
    max_entries: usize,
    row_limit: usize,
    path_byte_limit: usize,
    rows: AtomicUsize,
    path_bytes: AtomicUsize,
    captured_bytes: AtomicU64,
    max_bytes: u64,
}

impl<'a> SharedBudget<'a> {
    fn new(root: &'a Path, limits: LocalSourceLimits) -> Self {
        Self {
            root,
            max_entries: limits.max_entries,
            row_limit: limits.max_entries.saturating_add(1),
            path_byte_limit: SOURCE_BYTE_ABSOLUTE_LIMIT as usize,
            rows: AtomicUsize::new(1), // Root occupies row 0.
            path_bytes: AtomicUsize::new(0),
            captured_bytes: AtomicU64::new(0),
            max_bytes: limits.max_bytes,
        }
    }

    fn admit_path(&self, relative_bytes: &[u8]) -> Result<(), SourceResolveError> {
        let mut current = self.rows.load(Ordering::Relaxed);
        loop {
            if current >= self.row_limit {
                return Err(too_many_files(self.max_entries));
            }
            match self.rows.compare_exchange_weak(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => current = actual,
            }
        }
        if !native_capture_path_is_canonical(relative_bytes) {
            return Err(selection_io_error(
                self.root,
                SourceSelectionError::NoncanonicalPath {
                    path: relative_bytes.to_vec(),
                },
            ));
        }
        let add = relative_bytes.len();
        let mut bytes = self.path_bytes.load(Ordering::Relaxed);
        loop {
            let next = bytes
                .checked_add(add)
                .filter(|total| *total <= self.path_byte_limit)
                .ok_or_else(|| {
                    selection_io_error(
                        self.root,
                        SourceSelectionError::PathByteLimit {
                            limit: self.path_byte_limit,
                        },
                    )
                })?;
            match self.path_bytes.compare_exchange_weak(
                bytes,
                next,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Ok(()),
                Err(actual) => bytes = actual,
            }
        }
    }

    fn reserve_file_bytes(&self, expected_len: u64) -> Result<u64, SourceResolveError> {
        let mut current = self.captured_bytes.load(Ordering::Relaxed);
        loop {
            let next = current
                .checked_add(expected_len)
                .filter(|total| *total <= self.max_bytes)
                .ok_or(too_many_bytes(self.max_bytes))?;
            match self.captured_bytes.compare_exchange_weak(
                current,
                next,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Ok(self.max_bytes - current),
                Err(actual) => current = actual,
            }
        }
    }
}

fn selection_io_error(root: &Path, reason: SourceSelectionError) -> SourceResolveError {
    SourceResolveError::Io {
        path: root.to_path_buf(),
        message: reason.to_string(),
    }
}

struct ActiveDirectory {
    relative_path: PathBuf,
    directory: Arc<CapabilityDirectory>,
    metadata: cap_std::fs::Metadata,
}

#[derive(Clone, Copy)]
enum ChildKindHint {
    File,
    Directory,
    Other,
}

struct ChildWork {
    parent_directory: Arc<CapabilityDirectory>,
    parent_relative: PathBuf,
    relative_path: PathBuf,
    leaf_name: OsString,
    hint: ChildKindHint,
    expected_opening: Option<cap_std::fs::Metadata>,
}

enum CaptureTask {
    Directory(ActiveDirectory),
    LeafBatch(Vec<ChildWork>),
}

#[derive(Default)]
struct WorkerOutput {
    entries: Vec<SourceEntry>,
    directories: Vec<CapturedDirectoryObservation>,
    members: Vec<CapturedMemberObservation>,
}

pub(super) fn capture_members(
    root_directory: &CapabilityDirectory,
    root: &Path,
    limits: LocalSourceLimits,
    policy: SourceTreePolicy,
    request: &SourceCaptureRequest,
    entries: &mut Vec<SourceEntry>,
    observations: &mut CapturedTreeObservations,
) -> Result<(), SourceResolveError> {
    let root_metadata = root_directory.dir_metadata().at(root)?;
    if !root_metadata.is_dir() {
        return Err(selection_io_error(
            root,
            SourceSelectionError::RootNotDirectory,
        ));
    }
    let root_arc = Arc::new(root_directory.try_clone().at(root)?);
    let budget = SharedBudget::new(root, limits);
    let mut seen_scoped: HashSet<Vec<u8>> = HashSet::from([Vec::new()]);
    let mut partial_ancestors: BTreeMap<PathBuf, cap_std::fs::Metadata> = BTreeMap::new();
    let mut initial_dirs: Vec<ActiveDirectory> = Vec::new();
    let mut explicit_leaves: Vec<ChildWork> = Vec::new();
    let mut enumerated_root = false;

    let mut open_dirs = OpenDirectoryHandles::new(root_directory, root)?;
    for (declared, obligation) in request.entries() {
        if declared.is_empty() {
            enumerated_root = true;
            initial_dirs.push(ActiveDirectory {
                relative_path: PathBuf::new(),
                directory: Arc::clone(&root_arc),
                metadata: root_metadata.clone(),
            });
            continue;
        }

        let declared_path = source_path(declared, root)?;
        if path_is_excluded(policy, &declared_path) {
            if obligation == SourceCaptureObligation::Required {
                return Err(selection_io_error(
                    root,
                    SourceSelectionError::MissingRequired {
                        path: declared.to_vec(),
                    },
                ));
            }
            continue;
        }

        let mut reachable = true;
        for (end, _) in declared
            .iter()
            .enumerate()
            .filter(|(_, byte)| **byte == b'/')
        {
            let ancestor_bytes = &declared[..end];
            if seen_scoped.contains(ancestor_bytes) {
                let ancestor_path = source_path(ancestor_bytes, root)?;
                if !partial_ancestors.contains_key(&ancestor_path) {
                    reachable = false;
                    break;
                }
                continue;
            }
            let ancestor_path = source_path(ancestor_bytes, root)?;
            let parent = ancestor_path.parent().unwrap_or(Path::new(""));
            let leaf = ancestor_path
                .file_name()
                .expect("nonempty ancestor relative path");
            let display = root.join(&ancestor_path);
            let parent_dir = open_dirs.open(parent, root)?;
            let metadata = match parent_dir.symlink_metadata(leaf) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    reachable = false;
                    break;
                }
                Err(error) => return Err(io_error(&display, error)),
            };
            if !metadata.is_dir() {
                reachable = false;
                break;
            }
            if ancestor_path.components().count() > limits.max_depth {
                return Err(SourceResolveError::TooDeep {
                    path: display,
                    limit: limits.max_depth,
                });
            }
            budget.admit_path(ancestor_bytes)?;
            seen_scoped.insert(ancestor_bytes.to_vec());
            observations.members.push(CapturedMemberObservation {
                relative_path: ancestor_path.clone(),
                metadata: metadata.clone(),
            });
            partial_ancestors.insert(ancestor_path.clone(), metadata);
            push_entry(entries, ancestor_path, SourceEntryKind::Directory, limits)?;
        }

        let metadata = if reachable {
            let parent = declared_path.parent().unwrap_or(Path::new(""));
            let leaf = declared_path
                .file_name()
                .expect("nonempty declared relative path");
            let display = root.join(&declared_path);
            let parent_dir = open_dirs.open(parent, root)?;
            match parent_dir.symlink_metadata(leaf) {
                Ok(metadata) => Some((
                    parent_dir,
                    parent.to_path_buf(),
                    leaf.to_os_string(),
                    metadata,
                )),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => return Err(io_error(&display, error)),
            }
        } else {
            None
        };

        match metadata {
            Some((parent_dir, parent_relative, leaf_name, opening)) => {
                if opening.is_dir() && declared_path.components().count() > limits.max_depth {
                    return Err(SourceResolveError::TooDeep {
                        path: root.join(&declared_path),
                        limit: limits.max_depth,
                    });
                }
                if !seen_scoped.insert(declared.to_vec()) {
                    return Err(selection_io_error(
                        root,
                        SourceSelectionError::DuplicatePath {
                            path: declared.to_vec(),
                        },
                    ));
                }
                budget.admit_path(declared)?;
                if opening.is_dir() {
                    let display = root.join(&declared_path);
                    let child_dir = open_verified_child(&parent_dir, &leaf_name, &display)?;
                    let dir_meta = child_dir.dir_metadata().at(&display)?;
                    require_unchanged_entry(&opening, &dir_meta, &display)?;
                    observations.members.push(CapturedMemberObservation {
                        relative_path: declared_path.clone(),
                        metadata: dir_meta.clone(),
                    });
                    push_entry(
                        entries,
                        declared_path.clone(),
                        SourceEntryKind::Directory,
                        limits,
                    )?;
                    initial_dirs.push(ActiveDirectory {
                        relative_path: declared_path,
                        directory: child_dir,
                        metadata: dir_meta,
                    });
                } else {
                    let hint = if opening.is_file() {
                        ChildKindHint::File
                    } else {
                        ChildKindHint::Other
                    };
                    explicit_leaves.push(ChildWork {
                        parent_directory: parent_dir,
                        parent_relative,
                        relative_path: declared_path,
                        leaf_name,
                        hint,
                        expected_opening: Some(opening),
                    });
                }
            }
            None if obligation == SourceCaptureObligation::Required => {
                return Err(selection_io_error(
                    root,
                    SourceSelectionError::MissingRequired {
                        path: declared.to_vec(),
                    },
                ));
            }
            None => {}
        }
    }
    drop(open_dirs);

    if !enumerated_root {
        partial_ancestors.insert(PathBuf::new(), root_metadata);
    }
    for (relative_path, metadata) in partial_ancestors {
        observations.directories.push(CapturedDirectoryObservation {
            relative_path,
            metadata,
            listed_names: None,
        });
    }

    // Seed the queue by expanding a single root directory on the calling thread
    // so tiny fixtures never pay thread-spawn overhead, while larger trees fan
    // out immediately across one persistent worker pool.
    let mut initial_tasks: Vec<CaptureTask> = Vec::new();
    let mut seed_output = WorkerOutput::default();
    let has_nested_dirs;
    let mut pending_leaves = explicit_leaves.len();

    if !explicit_leaves.is_empty() {
        for chunk in explicit_leaves.chunks(8) {
            let mut batch = Vec::with_capacity(chunk.len());
            for item in chunk {
                batch.push(ChildWork {
                    parent_directory: Arc::clone(&item.parent_directory),
                    parent_relative: item.parent_relative.clone(),
                    relative_path: item.relative_path.clone(),
                    leaf_name: item.leaf_name.clone(),
                    hint: item.hint,
                    expected_opening: item.expected_opening.clone(),
                });
            }
            initial_tasks.push(CaptureTask::LeafBatch(batch));
        }
    }

    if initial_dirs.len() == 1 {
        let dir = initial_dirs.pop().unwrap();
        let (child_dirs, leaves) =
            list_and_partition_directory(dir, root, limits, policy, &budget, &mut seed_output)?;
        has_nested_dirs = !child_dirs.is_empty();
        pending_leaves += leaves.len();
        if !leaves.is_empty() {
            push_leaf_batches(leaves, &mut initial_tasks);
        }
        if !child_dirs.is_empty() {
            push_leaf_batches(child_dirs, &mut initial_tasks);
        }
    } else {
        has_nested_dirs = !initial_dirs.is_empty();
        for dir in initial_dirs {
            initial_tasks.push(CaptureTask::Directory(dir));
        }
    }

    let workers = if !has_nested_dirs && pending_leaves < 16 {
        1
    } else {
        capture_worker_count(usize::MAX)
    };

    if workers <= 1 {
        let mut scratch = vec![0_u8; FILE_READ_BUFFER_BYTES];
        let queue = WorkQueue::new(initial_tasks);
        while let Some(task) = queue.pop() {
            let outcome = run_capture_task(
                task,
                root_directory,
                root,
                limits,
                policy,
                &budget,
                &queue,
                &mut scratch,
                &mut seed_output,
                false,
            );
            queue.finish_one();
            outcome?;
        }
        merge_worker_output(seed_output, entries, observations);
    } else {
        let queue = WorkQueue::new(initial_tasks);
        let failed = AtomicBool::new(false);
        let errors: Mutex<Vec<SourceResolveError>> = Mutex::new(Vec::new());
        let worker_outputs = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..workers)
                .map(|_| {
                    scope.spawn(|| {
                        let mut scratch = vec![0_u8; FILE_READ_BUFFER_BYTES];
                        let mut output = WorkerOutput::default();
                        while let Some(task) = queue.pop() {
                            if !failed.load(Ordering::Relaxed)
                                && let Err(error) = run_capture_task(
                                    task,
                                    root_directory,
                                    root,
                                    limits,
                                    policy,
                                    &budget,
                                    &queue,
                                    &mut scratch,
                                    &mut output,
                                    true,
                                )
                            {
                                failed.store(true, Ordering::Relaxed);
                                queue.cancel();
                                errors.lock().expect("error lock poisoned").push(error);
                            }
                            queue.finish_one();
                        }
                        output
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().expect("source capture worker panicked"))
                .collect::<Vec<_>>()
        });
        let mut errors = errors.into_inner().expect("error lock poisoned");
        if !errors.is_empty() {
            return Err(errors.remove(0));
        }
        merge_worker_output(seed_output, entries, observations);
        for output in worker_outputs {
            merge_worker_output(output, entries, observations);
        }
    }

    if entries.len() > limits.max_entries {
        return Err(too_many_files(limits.max_entries));
    }
    Ok(())
}

fn merge_worker_output(
    output: WorkerOutput,
    entries: &mut Vec<SourceEntry>,
    observations: &mut CapturedTreeObservations,
) {
    entries.extend(output.entries);
    observations.directories.extend(output.directories);
    observations.members.extend(output.members);
}

fn push_leaf_batches(mut items: Vec<ChildWork>, target: &mut Vec<CaptureTask>) {
    const BATCH_SIZE: usize = 8;
    while !items.is_empty() {
        let split_at = items.len().saturating_sub(BATCH_SIZE);
        let batch = items.split_off(split_at);
        target.push(CaptureTask::LeafBatch(batch));
    }
}

fn list_and_partition_directory(
    dir: ActiveDirectory,
    root: &Path,
    limits: LocalSourceLimits,
    policy: SourceTreePolicy,
    budget: &SharedBudget<'_>,
    output: &mut WorkerOutput,
) -> Result<(Vec<ChildWork>, Vec<ChildWork>), SourceResolveError> {
    let display = root.join(&dir.relative_path);
    let allowance = if policy == SourceTreePolicy::LocalPackage {
        if dir.relative_path.as_os_str().is_empty() {
            4
        } else {
            1
        }
    } else {
        0
    };
    let mut raw_entries: Vec<(OsString, Option<cap_std::fs::FileType>)> = Vec::new();
    for member in dir.directory.entries().at(&display)? {
        if raw_entries.len() >= limits.max_entries.saturating_add(allowance) {
            return Err(too_many_files(limits.max_entries));
        }
        let entry = member.at(&display)?;
        let name = entry.file_name();
        let file_type = entry.file_type().ok();
        raw_entries.push((name, file_type));
    }
    raw_entries.sort_by(|(left, _), (right, _)| left.cmp(right));
    let all_names: Vec<OsString> = raw_entries.iter().map(|(name, _)| name.clone()).collect();
    for pair in all_names.windows(2) {
        if pair[0] == pair[1] {
            let child_bytes = canonical_relative_path_bytes(&dir.relative_path.join(&pair[0]));
            return Err(selection_io_error(
                root,
                SourceSelectionError::DuplicatePath { path: child_bytes },
            ));
        }
    }
    output.directories.push(CapturedDirectoryObservation {
        relative_path: dir.relative_path.clone(),
        metadata: dir.metadata,
        listed_names: Some(all_names),
    });

    let child_depth = dir.relative_path.components().count() + 1;
    let mut child_dirs = Vec::new();
    let mut child_leaves = Vec::new();
    for (name, file_type) in raw_entries {
        if excluded_from_capture(policy, &dir.relative_path, &name) {
            continue;
        }
        let child_path = dir.relative_path.join(&name);
        let child_bytes = canonical_relative_path_bytes(&child_path);
        budget.admit_path(&child_bytes)?;
        let hint = match file_type {
            Some(ft) if ft.is_symlink() => ChildKindHint::Other,
            Some(ft) if ft.is_dir() => {
                if child_depth > limits.max_depth {
                    return Err(SourceResolveError::TooDeep {
                        path: root.join(&child_path),
                        limit: limits.max_depth,
                    });
                }
                ChildKindHint::Directory
            }
            Some(ft) if ft.is_file() => ChildKindHint::File,
            _ => ChildKindHint::Other,
        };
        let work = ChildWork {
            parent_directory: Arc::clone(&dir.directory),
            parent_relative: dir.relative_path.clone(),
            relative_path: child_path,
            leaf_name: name,
            hint,
            expected_opening: None,
        };
        if matches!(hint, ChildKindHint::Directory) {
            child_dirs.push(work);
        } else {
            child_leaves.push(work);
        }
    }
    Ok((child_dirs, child_leaves))
}

#[allow(clippy::too_many_arguments)]
fn run_capture_task(
    task: CaptureTask,
    root_directory: &CapabilityDirectory,
    root: &Path,
    limits: LocalSourceLimits,
    policy: SourceTreePolicy,
    budget: &SharedBudget<'_>,
    queue: &WorkQueue<CaptureTask>,
    scratch: &mut [u8],
    output: &mut WorkerOutput,
    parallel: bool,
) -> Result<(), SourceResolveError> {
    match task {
        CaptureTask::Directory(dir) => {
            let (child_dirs, leaves) =
                list_and_partition_directory(dir, root, limits, policy, budget, output)?;
            if parallel && leaves.len() >= 16 {
                let mut batches = Vec::new();
                push_leaf_batches(leaves, &mut batches);
                queue.push_many(batches);
            } else {
                for leaf in leaves {
                    process_child(
                        leaf,
                        root_directory,
                        root,
                        limits,
                        policy,
                        budget,
                        queue,
                        scratch,
                        output,
                    )?;
                }
            }
            for child_dir in child_dirs {
                process_child(
                    child_dir,
                    root_directory,
                    root,
                    limits,
                    policy,
                    budget,
                    queue,
                    scratch,
                    output,
                )?;
            }
            Ok(())
        }
        CaptureTask::LeafBatch(items) => {
            for item in items {
                process_child(
                    item,
                    root_directory,
                    root,
                    limits,
                    policy,
                    budget,
                    queue,
                    scratch,
                    output,
                )?;
            }
            Ok(())
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn process_child(
    item: ChildWork,
    root_directory: &CapabilityDirectory,
    root: &Path,
    limits: LocalSourceLimits,
    policy: SourceTreePolicy,
    budget: &SharedBudget<'_>,
    queue: &WorkQueue<CaptureTask>,
    scratch: &mut [u8],
    output: &mut WorkerOutput,
) -> Result<(), SourceResolveError> {
    let display = root.join(&item.relative_path);
    match item.hint {
        ChildKindHint::File => {
            let (metadata, bytes, executable) = read_capability_file_observed(
                &item.parent_directory,
                &item.leaf_name,
                &display,
                item.expected_opening.as_ref(),
                |len| budget.reserve_file_bytes(len),
                limits.max_bytes,
                scratch,
            )?;
            output.members.push(CapturedMemberObservation {
                relative_path: item.relative_path.clone(),
                metadata,
            });
            output.entries.push(SourceEntry {
                relative_bytes: canonical_relative_path_bytes(&item.relative_path),
                relative_path: item.relative_path,
                kind: SourceEntryKind::File { bytes, executable },
            });
            Ok(())
        }
        ChildKindHint::Directory => {
            let child_dir = open_verified_child(&item.parent_directory, &item.leaf_name, &display)?;
            let dir_meta = child_dir.dir_metadata().at(&display)?;
            if let Some(opening) = &item.expected_opening {
                require_unchanged_entry(opening, &dir_meta, &display)?;
            }
            output.members.push(CapturedMemberObservation {
                relative_path: item.relative_path.clone(),
                metadata: dir_meta.clone(),
            });
            output.entries.push(SourceEntry {
                relative_bytes: canonical_relative_path_bytes(&item.relative_path),
                relative_path: item.relative_path.clone(),
                kind: SourceEntryKind::Directory,
            });
            queue.push(CaptureTask::Directory(ActiveDirectory {
                relative_path: item.relative_path,
                directory: child_dir,
                metadata: dir_meta,
            }));
            Ok(())
        }
        ChildKindHint::Other => {
            let opening = match item.parent_directory.symlink_metadata(&item.leaf_name) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    return Err(local_source_changed(&display));
                }
                Err(error) => return Err(io_error(&display, error)),
            };
            if let Some(expected) = &item.expected_opening {
                require_unchanged_entry(expected, &opening, &display)?;
            }
            if opening.file_type().is_symlink() {
                let target = read_and_validate_symlink_target(
                    root_directory,
                    root,
                    &item.parent_directory,
                    &item.parent_relative,
                    &item.leaf_name,
                    &display,
                    policy,
                )?;
                output.members.push(CapturedMemberObservation {
                    relative_path: item.relative_path.clone(),
                    metadata: opening,
                });
                output.entries.push(SourceEntry {
                    relative_bytes: canonical_relative_path_bytes(&item.relative_path),
                    relative_path: item.relative_path,
                    kind: SourceEntryKind::Symlink {
                        target_bytes: crate::tree::filesystem::raw_os_bytes(target.as_os_str()),
                    },
                });
                Ok(())
            } else if opening.is_dir() {
                if item.relative_path.components().count() > limits.max_depth {
                    return Err(SourceResolveError::TooDeep {
                        path: display,
                        limit: limits.max_depth,
                    });
                }
                let child_dir =
                    open_verified_child(&item.parent_directory, &item.leaf_name, &display)?;
                let dir_meta = child_dir.dir_metadata().at(&display)?;
                require_unchanged_entry(&opening, &dir_meta, &display)?;
                output.members.push(CapturedMemberObservation {
                    relative_path: item.relative_path.clone(),
                    metadata: dir_meta.clone(),
                });
                output.entries.push(SourceEntry {
                    relative_bytes: canonical_relative_path_bytes(&item.relative_path),
                    relative_path: item.relative_path.clone(),
                    kind: SourceEntryKind::Directory,
                });
                queue.push(CaptureTask::Directory(ActiveDirectory {
                    relative_path: item.relative_path,
                    directory: child_dir,
                    metadata: dir_meta,
                }));
                Ok(())
            } else if opening.is_file() {
                let (metadata, bytes, executable) = read_capability_file_observed(
                    &item.parent_directory,
                    &item.leaf_name,
                    &display,
                    Some(&opening),
                    |len| budget.reserve_file_bytes(len),
                    limits.max_bytes,
                    scratch,
                )?;
                output.members.push(CapturedMemberObservation {
                    relative_path: item.relative_path.clone(),
                    metadata,
                });
                output.entries.push(SourceEntry {
                    relative_bytes: canonical_relative_path_bytes(&item.relative_path),
                    relative_path: item.relative_path,
                    kind: SourceEntryKind::File { bytes, executable },
                });
                Ok(())
            } else {
                Err(SourceResolveError::UnsupportedFileType { path: display })
            }
        }
    }
}

fn path_is_excluded(policy: SourceTreePolicy, path: &Path) -> bool {
    let mut prefix = PathBuf::new();
    for component in path.components() {
        if excluded_from_capture(policy, &prefix, component.as_os_str()) {
            return true;
        }
        prefix.push(component.as_os_str());
    }
    false
}

fn native_capture_path_is_canonical(path: &[u8]) -> bool {
    #[cfg(unix)]
    {
        !path.is_empty()
            && !path.starts_with(b"/")
            && !path.contains(&0)
            && !path
                .split(|byte| *byte == b'/')
                .any(|component| component.is_empty() || matches!(component, b"." | b".."))
    }
    #[cfg(not(unix))]
    crate::selection::canonical_filesystem_metadata_path_is_canonical(path, false)
}

#[cfg(unix)]
fn source_path(bytes: &[u8], _root: &Path) -> Result<PathBuf, SourceResolveError> {
    use std::os::unix::ffi::OsStringExt;
    Ok(std::ffi::OsString::from_vec(bytes.to_vec()).into())
}

#[cfg(not(unix))]
fn source_path(bytes: &[u8], root: &Path) -> Result<PathBuf, SourceResolveError> {
    String::from_utf8(bytes.to_vec())
        .map(PathBuf::from)
        .map_err(|error| {
            io_error(
                root,
                std::io::Error::new(std::io::ErrorKind::InvalidData, error),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::selection::SourceCaptureObligation;
    use crate::tree::capture::traversal::verify_captured_tree;
    use crate::tree::filesystem::open_canonical_source_root;

    fn observed_selection(name: &str) -> (PathBuf, CapabilityDirectory, CapturedTreeObservations) {
        let root = crate::test_support::temp_root(name);
        std::fs::create_dir_all(root.join("private")).unwrap();
        std::fs::write(root.join("private/main.omg"), "selected source").unwrap();
        std::fs::write(root.join("private/unselected.txt"), "outside capture").unwrap();
        let directory = open_canonical_source_root(&root).unwrap();
        let request = SourceCaptureRequest::new([(
            b"private/main.omg".to_vec(),
            SourceCaptureObligation::Required,
        )])
        .unwrap();
        let mut observations = CapturedTreeObservations::default();
        capture_members(
            &directory,
            &root,
            LocalSourceLimits::default(),
            SourceTreePolicy::LocalPackage,
            &request,
            &mut Vec::new(),
            &mut observations,
        )
        .unwrap();
        (root, directory, observations)
    }

    #[test]
    fn closing_sweep_rejects_selected_file_drift() {
        let (root, directory, observations) = observed_selection("selected-file-drift");
        std::fs::write(
            root.join("private/main.omg"),
            "different selected source bytes",
        )
        .unwrap();
        assert!(matches!(
            verify_captured_tree(&directory, &root, &observations),
            Err(SourceResolveError::LocalSourceChanged { .. })
        ));
        drop(directory);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn closing_sweep_preserves_capture_when_only_unselected_file_contents_change() {
        let (root, directory, observations) = observed_selection("unselected-file-drift");
        std::fs::write(
            root.join("private/unselected.txt"),
            "changed outside source inventory",
        )
        .unwrap();
        verify_captured_tree(&directory, &root, &observations).unwrap();
        drop(directory);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn closing_sweep_rejects_replaced_partial_ancestor_without_following_it() {
        let (root, directory, observations) = observed_selection("selected-ancestor-drift");
        std::fs::rename(root.join("private"), root.join("retained")).unwrap();
        std::os::unix::fs::symlink("retained", root.join("private")).unwrap();
        assert!(matches!(
            verify_captured_tree(&directory, &root, &observations),
            Err(SourceResolveError::LocalSourceChanged { .. })
        ));
        drop(directory);
        std::fs::remove_dir_all(root).unwrap();
    }
}
