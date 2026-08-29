//! Git object parsing, graph authentication, and bounded blob transfer.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::resolution::source) struct GitTreeEntry {
    pub(in crate::resolution::source) relative_bytes: Vec<u8>,
    pub(in crate::resolution::source) relative_path: PathBuf,
    pub(in crate::resolution::source) oid: String,
    pub(in crate::resolution::source) size: u64,
    pub(in crate::resolution::source) kind: GitTreeEntryKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::resolution::source) enum GitTreeEntryKind {
    Tree,
    File {
        executable: bool,
        bytes: GitBlobBytes,
    },
    Symlink {
        target_bytes: GitBlobBytes,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::resolution::source) struct GitBlobBytes {
    pub(in crate::resolution::source) batch: Arc<Vec<u8>>,
    pub(in crate::resolution::source) start: usize,
    pub(in crate::resolution::source) end: usize,
}

impl GitBlobBytes {
    pub(in crate::resolution::source) fn empty() -> Self {
        Self {
            batch: Arc::new(Vec::new()),
            start: 0,
            end: 0,
        }
    }

    pub(in crate::resolution::source) fn as_slice(&self) -> &[u8] {
        &self.batch[self.start..self.end]
    }
}

#[derive(Debug)]
enum AuthenticatedGitTreeNode {
    Blob {
        mode: &'static [u8],
        oid: String,
    },
    Tree {
        expected_oid: String,
        directory: AuthenticatedGitDirectory,
    },
}

#[derive(Debug, Default)]
struct AuthenticatedGitDirectory {
    entries: BTreeMap<Vec<u8>, AuthenticatedGitTreeNode>,
}

pub(in crate::resolution::source) fn inspect_git_tree(
    executor: &GitExecutor,
    repository: &VerifiedGitRepository,
    tree: &str,
    limits: LocalSourceLimits,
) -> Result<Vec<GitTreeEntry>, SourceResolveError> {
    if !is_object_id(tree) {
        return Err(cache_invalid(
            repository.path(),
            "Git returned an invalid tree object ID",
        ));
    }
    let listing = repository.run_git_bytes_stdout(
        executor,
        [
            OsStr::new("ls-tree"),
            OsStr::new("--full-tree"),
            OsStr::new("-r"),
            OsStr::new("-t"),
            OsStr::new("-l"),
            OsStr::new("-z"),
            OsStr::new(tree),
        ],
    )?;
    let mut entries = parse_git_tree_entries(&listing, repository.path(), limits)?;
    read_git_blobs_batch(executor, repository, &mut entries, limits)?;
    Ok(entries)
}

pub(in crate::resolution::source) fn verify_exact_git_revision(
    requested_rev: &str,
    selected_commit: &str,
) -> Result<(), SourceResolveError> {
    if is_object_id(requested_rev) && !requested_rev.eq_ignore_ascii_case(selected_commit) {
        return Err(git_object_invalid(
            selected_commit,
            "selected commit does not match the exact requested revision",
        ));
    }
    Ok(())
}

pub(in crate::resolution::source) fn authenticate_git_commit(
    executor: &GitExecutor,
    repository: &VerifiedGitRepository,
    commit: &str,
    tree: &str,
) -> Result<(), SourceResolveError> {
    let payload = repository.run_git_bytes_stdout(
        executor,
        [
            OsStr::new("cat-file"),
            OsStr::new("commit"),
            OsStr::new(commit),
        ],
    )?;
    authenticate_git_commit_payload(commit, tree, &payload)
}

pub(in crate::resolution::source) fn authenticate_git_commit_payload(
    commit: &str,
    reported_tree: &str,
    payload: &[u8],
) -> Result<(), SourceResolveError> {
    let algorithm = git_object_algorithm(commit)?;
    if git_object_algorithm(reported_tree)? != algorithm {
        return Err(git_object_invalid(
            commit,
            "commit and root tree use different object formats",
        ));
    }
    verify_git_object_identity(commit, b"commit", payload, algorithm)?;

    let first_line = payload
        .split(|byte| *byte == b'\n')
        .next()
        .unwrap_or(payload);
    let Some(commit_tree) = first_line.strip_prefix(b"tree ") else {
        return Err(git_object_invalid(
            commit,
            "commit payload does not begin with one root tree edge",
        ));
    };
    let commit_tree = std::str::from_utf8(commit_tree)
        .map_err(|_| git_object_invalid(commit, "commit tree ID is not ASCII"))?;
    if git_object_algorithm(commit_tree)? != algorithm || commit_tree != reported_tree {
        return Err(git_object_invalid(
            commit,
            "commit root tree edge does not match the selected tree",
        ));
    }
    Ok(())
}

pub(in crate::resolution::source) fn authenticate_git_tree(
    expected_tree: &str,
    entries: &[GitTreeEntry],
) -> Result<(), SourceResolveError> {
    let algorithm = git_object_algorithm(expected_tree)?;
    let mut root = AuthenticatedGitDirectory::default();
    for entry in entries {
        match &entry.kind {
            GitTreeEntryKind::Tree => {}
            GitTreeEntryKind::File { bytes, .. } => {
                verify_git_object_identity(&entry.oid, b"blob", bytes.as_slice(), algorithm)?;
            }
            GitTreeEntryKind::Symlink { target_bytes } => {
                verify_git_object_identity(
                    &entry.oid,
                    b"blob",
                    target_bytes.as_slice(),
                    algorithm,
                )?;
            }
        }
        insert_authenticated_git_entry(&mut root, entry)?;
    }
    let actual_tree = authenticate_git_directory(&root, algorithm)?;
    if actual_tree != expected_tree {
        return Err(git_object_invalid(
            expected_tree,
            "authenticated tree graph does not reconstruct the selected root tree",
        ));
    }
    Ok(())
}

fn insert_authenticated_git_entry(
    directory: &mut AuthenticatedGitDirectory,
    entry: &GitTreeEntry,
) -> Result<(), SourceResolveError> {
    let components = entry
        .relative_bytes
        .split(|byte| *byte == b'/')
        .collect::<Vec<_>>();
    insert_authenticated_git_components(directory, &components, entry)
}

fn insert_authenticated_git_components(
    directory: &mut AuthenticatedGitDirectory,
    components: &[&[u8]],
    entry: &GitTreeEntry,
) -> Result<(), SourceResolveError> {
    let Some((name, rest)) = components.split_first() else {
        return Err(git_tree_invalid(
            &entry.relative_bytes,
            "authenticated tree entry has no path component",
        ));
    };
    if rest.is_empty() {
        let node = match entry.kind {
            GitTreeEntryKind::Tree => AuthenticatedGitTreeNode::Tree {
                expected_oid: entry.oid.clone(),
                directory: AuthenticatedGitDirectory::default(),
            },
            GitTreeEntryKind::File {
                executable: false, ..
            } => AuthenticatedGitTreeNode::Blob {
                mode: b"100644".as_slice(),
                oid: entry.oid.clone(),
            },
            GitTreeEntryKind::File {
                executable: true, ..
            } => AuthenticatedGitTreeNode::Blob {
                mode: b"100755".as_slice(),
                oid: entry.oid.clone(),
            },
            GitTreeEntryKind::Symlink { .. } => AuthenticatedGitTreeNode::Blob {
                mode: b"120000".as_slice(),
                oid: entry.oid.clone(),
            },
        };
        if directory.entries.insert(name.to_vec(), node).is_some() {
            return Err(git_tree_invalid(
                &entry.relative_bytes,
                "authenticated tree contains a duplicate path",
            ));
        }
        return Ok(());
    }

    let Some(node) = directory.entries.get_mut(*name) else {
        return Err(git_tree_invalid(
            &entry.relative_bytes,
            "authenticated tree path has no declared parent-tree edge",
        ));
    };
    let AuthenticatedGitTreeNode::Tree {
        directory: child, ..
    } = node
    else {
        return Err(git_tree_invalid(
            &entry.relative_bytes,
            "authenticated tree path traverses a blob",
        ));
    };
    insert_authenticated_git_components(child, rest, entry)
}

fn authenticate_git_directory(
    directory: &AuthenticatedGitDirectory,
    algorithm: GitObjectIdAlgorithm,
) -> Result<String, SourceResolveError> {
    let mut ordered = directory.entries.iter().collect::<Vec<_>>();
    ordered.sort_by(git_tree_entry_order);
    let mut payload = Vec::new();
    for (name, node) in ordered {
        let (mode, oid) = match node {
            AuthenticatedGitTreeNode::Blob { mode, oid } => (*mode, oid.clone()),
            AuthenticatedGitTreeNode::Tree {
                expected_oid,
                directory,
            } => {
                if git_object_algorithm(expected_oid)? != algorithm {
                    return Err(git_object_invalid(
                        expected_oid,
                        "child tree uses a different hash algorithm than its graph",
                    ));
                }
                let actual_oid = authenticate_git_directory(directory, algorithm)?;
                if actual_oid != *expected_oid {
                    return Err(git_object_invalid(
                        expected_oid,
                        "child tree bytes do not match the declared tree edge",
                    ));
                }
                (b"40000".as_slice(), actual_oid)
            }
        };
        payload.extend_from_slice(mode);
        payload.push(b' ');
        payload.extend_from_slice(name);
        payload.push(0);
        payload.extend_from_slice(&decode_git_object_id(&oid, algorithm)?);
    }
    git_object_identity(b"tree", &payload, algorithm)
}

fn git_tree_entry_order(
    left: &(&Vec<u8>, &AuthenticatedGitTreeNode),
    right: &(&Vec<u8>, &AuthenticatedGitTreeNode),
) -> std::cmp::Ordering {
    let common = left.0.len().min(right.0.len());
    let prefix = left.0[..common].cmp(&right.0[..common]);
    if prefix != std::cmp::Ordering::Equal {
        return prefix;
    }
    let left_next = left.0.get(common).copied().unwrap_or({
        if matches!(left.1, AuthenticatedGitTreeNode::Tree { .. }) {
            b'/'
        } else {
            0
        }
    });
    let right_next = right.0.get(common).copied().unwrap_or({
        if matches!(right.1, AuthenticatedGitTreeNode::Tree { .. }) {
            b'/'
        } else {
            0
        }
    });
    left_next.cmp(&right_next)
}

pub(in crate::resolution::source) fn verify_git_object_identity(
    expected: &str,
    kind: &[u8],
    payload: &[u8],
    algorithm: GitObjectIdAlgorithm,
) -> Result<(), SourceResolveError> {
    if git_object_algorithm(expected)? != algorithm {
        return Err(git_object_invalid(
            expected,
            "object ID uses a different hash algorithm than its graph",
        ));
    }
    if git_object_identity(kind, payload, algorithm)? != expected {
        return Err(git_object_invalid(
            expected,
            "object bytes do not match the declared object ID",
        ));
    }
    Ok(())
}

pub(in crate::resolution::source) fn git_object_identity(
    kind: &[u8],
    payload: &[u8],
    algorithm: GitObjectIdAlgorithm,
) -> Result<String, SourceResolveError> {
    let length = payload.len().to_string();
    match algorithm {
        GitObjectIdAlgorithm::Sha1 => {
            let mut hasher = CheckedSha1::new();
            hasher.update(kind);
            hasher.update(b" ");
            hasher.update(length.as_bytes());
            hasher.update([0]);
            hasher.update(payload);
            finalize_checked_sha1(hasher)
        }
        GitObjectIdAlgorithm::Sha256 => {
            let mut hasher = Sha256::new();
            hasher.update(kind);
            hasher.update(b" ");
            hasher.update(length.as_bytes());
            hasher.update([0]);
            hasher.update(payload);
            Ok(format_hex(&hasher.finalize()))
        }
    }
}

pub(in crate::resolution::source) fn finalize_checked_sha1(
    hasher: CheckedSha1,
) -> Result<String, SourceResolveError> {
    let result = hasher.try_finalize();
    if result.has_collision() {
        return Err(git_object_invalid(
            "sha1-collision",
            "Git object bytes match a known SHA-1 collision attack",
        ));
    }
    Ok(format_hex(result.hash()))
}

pub(in crate::resolution::source) fn git_object_algorithm(
    oid: &str,
) -> Result<GitObjectIdAlgorithm, SourceResolveError> {
    if !is_object_id(oid) {
        return Err(git_object_invalid(oid, "object ID has an invalid spelling"));
    }
    Ok(if oid.len() == 40 {
        GitObjectIdAlgorithm::Sha1
    } else {
        GitObjectIdAlgorithm::Sha256
    })
}

fn decode_git_object_id(
    oid: &str,
    algorithm: GitObjectIdAlgorithm,
) -> Result<Vec<u8>, SourceResolveError> {
    if git_object_algorithm(oid)? != algorithm {
        return Err(git_object_invalid(
            oid,
            "child object uses a different hash algorithm than its tree",
        ));
    }
    let mut bytes = Vec::with_capacity(oid.len() / 2);
    for pair in oid.as_bytes().chunks_exact(2) {
        let high = hex_digit(pair[0])
            .ok_or_else(|| git_object_invalid(oid, "object ID contains a non-hexadecimal digit"))?;
        let low = hex_digit(pair[1])
            .ok_or_else(|| git_object_invalid(oid, "object ID contains a non-hexadecimal digit"))?;
        bytes.push((high << 4) | low);
    }
    Ok(bytes)
}

pub(in crate::resolution::source) fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

pub(in crate::resolution::source) fn git_object_invalid(
    oid: impl Into<String>,
    message: impl Into<String>,
) -> SourceResolveError {
    SourceResolveError::GitObjectInvalid {
        oid: oid.into(),
        message: message.into(),
    }
}

pub(in crate::resolution::source) fn parse_git_tree_entries(
    listing: &[u8],
    repository: &Path,
    limits: LocalSourceLimits,
) -> Result<Vec<GitTreeEntry>, SourceResolveError> {
    let mut entries = Vec::new();
    let mut paths = BTreeMap::new();
    let mut blob_bytes = 0_u64;

    for record in listing.split(|byte| *byte == 0) {
        if record.is_empty() {
            continue;
        }
        let Some(tab) = record.iter().position(|byte| *byte == b'\t') else {
            return Err(git_tree_invalid(Vec::new(), "malformed ls-tree record"));
        };
        let header = &record[..tab];
        let path = &record[tab + 1..];
        let fields = header
            .split(|byte| byte.is_ascii_whitespace())
            .filter(|field| !field.is_empty())
            .collect::<Vec<_>>();
        if fields.len() != 4 {
            return Err(git_tree_invalid(path, "malformed ls-tree header"));
        }
        let mode = fields[0];
        let object_type = fields[1];
        let oid = std::str::from_utf8(fields[2])
            .map_err(|_| git_tree_invalid(path, "object ID is not ASCII"))?;
        if !is_object_id(oid) {
            return Err(git_tree_invalid(path, "object ID has an invalid spelling"));
        }
        if mode == b"160000" || object_type == b"commit" {
            return Err(SourceResolveError::GitSubmodulesUnsupported {
                path: git_path_from_bytes(path).unwrap_or_else(|_| repository.to_path_buf()),
            });
        }
        let relative_path = validate_git_path(path, limits)?;
        if path
            .split(|byte| *byte == b'/')
            .any(|component| component.eq_ignore_ascii_case(b".gitmodules"))
        {
            return Err(SourceResolveError::GitSubmodulesUnsupported {
                path: relative_path,
            });
        }
        let (size, kind) = match (mode, object_type, fields[3]) {
            (b"040000", b"tree", b"-") => (0, GitTreeEntryKind::Tree),
            (b"100644", b"blob", size) => (
                parse_git_blob_size(path, size)?,
                GitTreeEntryKind::File {
                    executable: false,
                    bytes: GitBlobBytes::empty(),
                },
            ),
            (b"100755", b"blob", size) => (
                parse_git_blob_size(path, size)?,
                GitTreeEntryKind::File {
                    executable: true,
                    bytes: GitBlobBytes::empty(),
                },
            ),
            (b"120000", b"blob", size) => (
                parse_git_blob_size(path, size)?,
                GitTreeEntryKind::Symlink {
                    target_bytes: GitBlobBytes::empty(),
                },
            ),
            _ => return Err(git_tree_invalid(path, "unsupported Git tree entry")),
        };
        if paths
            .insert(path.to_vec(), matches!(&kind, GitTreeEntryKind::Tree))
            .is_some()
        {
            return Err(git_tree_invalid(path, "duplicate path"));
        }
        let identity_entry_count =
            entries
                .len()
                .checked_add(1)
                .ok_or(SourceResolveError::TooManyFiles {
                    limit: limits.max_files,
                })?;
        if identity_entry_count > limits.max_files {
            return Err(SourceResolveError::TooManyFiles {
                limit: limits.max_files,
            });
        }
        if !matches!(&kind, GitTreeEntryKind::Tree) {
            blob_bytes = blob_bytes
                .checked_add(size)
                .ok_or(SourceResolveError::TooManyBytes {
                    limit: limits.max_bytes,
                })?;
            if blob_bytes > limits.max_bytes {
                return Err(SourceResolveError::TooManyBytes {
                    limit: limits.max_bytes,
                });
            }
        }
        entries.push(GitTreeEntry {
            relative_bytes: path.to_vec(),
            relative_path,
            oid: oid.to_owned(),
            size,
            kind,
        });
    }

    entries.sort_by(|left, right| left.relative_bytes.cmp(&right.relative_bytes));
    for entry in &entries {
        for separator in entry
            .relative_bytes
            .iter()
            .enumerate()
            .filter_map(|(index, byte)| (*byte == b'/').then_some(index))
        {
            let parent = &entry.relative_bytes[..separator];
            match paths.get(parent) {
                Some(true) => {}
                Some(false) => {
                    return Err(git_tree_invalid(
                        &entry.relative_bytes,
                        "Git path traverses a blob",
                    ));
                }
                None => {
                    return Err(git_tree_invalid(
                        &entry.relative_bytes,
                        "Git listing omitted a parent-tree edge",
                    ));
                }
            }
        }
    }
    Ok(entries)
}

fn parse_git_blob_size(path: &[u8], size: &[u8]) -> Result<u64, SourceResolveError> {
    std::str::from_utf8(size)
        .ok()
        .and_then(|size| size.parse::<u64>().ok())
        .ok_or_else(|| git_tree_invalid(path, "blob size is missing or invalid"))
}

pub(in crate::resolution::source) fn git_directory_paths(
    entries: &[GitTreeEntry],
) -> BTreeSet<Vec<u8>> {
    entries
        .iter()
        .filter(|entry| matches!(&entry.kind, GitTreeEntryKind::Tree))
        .map(|entry| entry.relative_bytes.clone())
        .collect()
}

fn validate_git_path(
    path: &[u8],
    limits: LocalSourceLimits,
) -> Result<PathBuf, SourceResolveError> {
    if path.is_empty() || path.starts_with(b"/") || path.ends_with(b"/") {
        return Err(git_tree_invalid(
            path,
            "path must be a non-empty relative path",
        ));
    }
    if path.contains(&b'\\') {
        return Err(git_tree_invalid(
            path,
            "backslashes are forbidden in portable package paths",
        ));
    }
    let components = path.split(|byte| *byte == b'/').collect::<Vec<_>>();
    for component in &components {
        if component.is_empty() || *component == b"." || *component == b".." {
            return Err(git_tree_invalid(
                path,
                "path contains a traversal component",
            ));
        }
        if component.eq_ignore_ascii_case(b".git") {
            return Err(git_tree_invalid(path, "path enters excluded Git metadata"));
        }
        validate_portable_git_component(path, component)?;
    }
    let depth = components.len().saturating_sub(1);
    if depth > limits.max_depth {
        return Err(SourceResolveError::TooDeep {
            path: git_path_from_bytes(path)?,
            limit: limits.max_depth,
        });
    }
    git_path_from_bytes(path)
}

fn validate_portable_git_component(
    path: &[u8],
    component: &[u8],
) -> Result<(), SourceResolveError> {
    if component
        .iter()
        .any(|byte| *byte < 32 || matches!(*byte, b'<' | b'>' | b':' | b'"' | b'|' | b'?' | b'*'))
    {
        return Err(git_tree_invalid(
            path,
            "path contains a character forbidden by the portable Windows policy",
        ));
    }
    if component
        .last()
        .is_some_and(|byte| matches!(byte, b'.' | b' '))
    {
        return Err(git_tree_invalid(
            path,
            "path component has a Windows-ambiguous trailing dot or space",
        ));
    }
    let stem = component
        .split(|byte| *byte == b'.')
        .next()
        .unwrap_or(component);
    let reserved_device = [b"CON".as_slice(), b"PRN", b"AUX", b"NUL"]
        .iter()
        .any(|reserved| stem.eq_ignore_ascii_case(reserved))
        || (stem.len() == 4
            && (stem[..3].eq_ignore_ascii_case(b"COM") || stem[..3].eq_ignore_ascii_case(b"LPT"))
            && matches!(stem[3], b'1'..=b'9'))
        || stem.eq_ignore_ascii_case(b"CONIN$")
        || stem.eq_ignore_ascii_case(b"CONOUT$");
    if reserved_device {
        return Err(git_tree_invalid(
            path,
            "path component uses a reserved Windows device name",
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn git_path_from_bytes(path: &[u8]) -> Result<PathBuf, SourceResolveError> {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    Ok(PathBuf::from(OsString::from_vec(path.to_vec())))
}

#[cfg(not(unix))]
fn git_path_from_bytes(path: &[u8]) -> Result<PathBuf, SourceResolveError> {
    let text = std::str::from_utf8(path)
        .map_err(|_| git_tree_invalid(path, "path cannot be represented on this host"))?;
    Ok(PathBuf::from(text))
}

pub(in crate::resolution::source) fn validate_git_symlink_target(
    link: &[u8],
    target: &[u8],
) -> Result<(), SourceResolveError> {
    if target.is_empty() || target.starts_with(b"/") || target.contains(&0) {
        return Err(git_tree_invalid(
            link,
            "symlink target must be a non-empty relative path",
        ));
    }
    if target.contains(&b'\\') {
        return Err(git_tree_invalid(
            link,
            "symlink target contains a non-portable path separator",
        ));
    }
    let mut depth = link.split(|byte| *byte == b'/').count().saturating_sub(1);
    for component in target.split(|byte| *byte == b'/') {
        match component {
            b"" | b"." => {}
            b".." => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| git_tree_invalid(link, "symlink target escapes the snapshot"))?;
            }
            component if component.eq_ignore_ascii_case(b".git") => {
                return Err(git_tree_invalid(
                    link,
                    "symlink target enters excluded Git metadata",
                ));
            }
            component => {
                validate_portable_git_component(link, component)?;
                depth += 1;
            }
        }
    }
    Ok(())
}

pub(in crate::resolution::source) fn is_object_id(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(in crate::resolution::source) fn git_tree_invalid(
    path: impl AsRef<[u8]>,
    message: impl Into<String>,
) -> SourceResolveError {
    SourceResolveError::GitTreeInvalid {
        path: path.as_ref().to_vec(),
        message: message.into(),
    }
}

fn read_git_blobs_batch(
    executor: &GitExecutor,
    repository: &VerifiedGitRepository,
    entries: &mut [GitTreeEntry],
    limits: LocalSourceLimits,
) -> Result<(), SourceResolveError> {
    executor.verify_budget()?;
    if entries
        .iter()
        .all(|entry| matches!(&entry.kind, GitTreeEntryKind::Tree))
    {
        return Ok(());
    }
    let stdout_limit = git_batch_output_limit(entries, limits)?;
    repository.verify_identity()?;
    let mut request = PendingGitBatchRequest::create(&repository.entry, &repository.entry_root)?;
    let operation_result = (|| {
        let request_path = request.display_path.clone();
        write_git_batch_request(request.file_mut(), &request_path, entries)?;
        request.verify_current()?;
        let stdin = request
            .file()
            .try_clone()
            .map_err(|error| io_error(&request.display_path, error))?;
        execute_git_blob_batch(executor, repository.path(), stdin, entries, stdout_limit)
    })();
    let namespace_result = repository
        .verify_identity()
        .and_then(|_| request.verify_current());
    let cleanup_result = request.remove();
    reconcile_git_cache_operation_result(operation_result, namespace_result, Some(cleanup_result))
}

#[cfg(test)]
pub(in crate::resolution::source) fn read_git_blobs_batch_from_path(
    executor: &GitExecutor,
    repository: &Path,
    entries: &mut [GitTreeEntry],
    limits: LocalSourceLimits,
) -> Result<(), SourceResolveError> {
    executor.verify_budget()?;
    if entries
        .iter()
        .all(|entry| matches!(&entry.kind, GitTreeEntryKind::Tree))
    {
        return Ok(());
    }
    let stdout_limit = git_batch_output_limit(entries, limits)?;
    let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let request_path = repository
        .parent()
        .expect("validated bare repository has an entry root")
        .join(format!(
            ".omega-cat-file-batch.{}.{}",
            std::process::id(),
            sequence
        ));
    let request_guard = TemporaryFileGuard {
        path: request_path.clone(),
    };
    let mut request = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&request_path)
        .map_err(|error| io_error(&request_path, error))?;
    write_git_batch_request(&mut request, &request_path, entries)?;

    let result = execute_git_blob_batch(executor, repository, request, entries, stdout_limit);
    drop(request_guard);
    result
}

fn write_git_batch_request(
    request: &mut File,
    request_path: &Path,
    entries: &[GitTreeEntry],
) -> Result<(), SourceResolveError> {
    for entry in entries
        .iter()
        .filter(|entry| !matches!(&entry.kind, GitTreeEntryKind::Tree))
    {
        request
            .write_all(entry.oid.as_bytes())
            .and_then(|_| request.write_all(b"\n"))
            .map_err(|error| io_error(request_path, error))?;
    }
    request
        .seek(SeekFrom::Start(0))
        .map(|_| ())
        .map_err(|error| io_error(request_path, error))
}

fn execute_git_blob_batch(
    executor: &GitExecutor,
    repository: &Path,
    request: File,
    entries: &mut [GitTreeEntry],
    stdout_limit: usize,
) -> Result<(), SourceResolveError> {
    let mut command = sealed_git_command_with_route(
        executor,
        repository,
        ResolverExecutionPhase::RepositoryInspection,
        None,
    )?;
    let command_timeout = executor.begin_launch()?;
    command.args([OsStr::new("cat-file"), OsStr::new("--batch")]);
    let stdin_identity = git_batch_stdin_identity(entries);
    let command_identity = git_command_configuration_identity(
        &command,
        ResolverExecutionPhase::RepositoryInspection,
        &stdin_identity,
    );
    let result = run_command_bounded_with_stdin_and_budget(
        &mut command,
        Stdio::from(request),
        "cat-file --batch",
        stdout_limit,
        GIT_STDERR_LIMIT,
        command_timeout,
        executor.captured_output_budget.clone(),
    );
    let output = reconcile_git_command_result(result, executor.verify(), executor.verify_budget())?;
    executor.record_command_execution(
        ResolverExecutionPhase::RepositoryInspection,
        command_identity,
        &output,
        None,
    )?;
    if !output.status.success() {
        return Err(SourceResolveError::Git {
            operation: "cat-file --batch".to_owned(),
            status: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    assign_git_batch_output(entries, output.stdout)?;
    executor.verify_budget()
}

pub(in crate::resolution::source) fn git_batch_output_limit(
    entries: &[GitTreeEntry],
    limits: LocalSourceLimits,
) -> Result<usize, SourceResolveError> {
    let mut payload_bytes = 0_u64;
    let mut output_bytes = 0_usize;
    for entry in entries
        .iter()
        .filter(|entry| !matches!(&entry.kind, GitTreeEntryKind::Tree))
    {
        payload_bytes =
            payload_bytes
                .checked_add(entry.size)
                .ok_or(SourceResolveError::TooManyBytes {
                    limit: limits.max_bytes,
                })?;
        if payload_bytes > limits.max_bytes {
            return Err(SourceResolveError::TooManyBytes {
                limit: limits.max_bytes,
            });
        }
        let size = usize::try_from(entry.size).map_err(|_| {
            git_tree_invalid(entry.oid.as_bytes(), "blob cannot fit in host memory")
        })?;
        output_bytes = output_bytes
            .checked_add(entry.oid.len())
            .and_then(|value| value.checked_add(b" blob ".len()))
            .and_then(|value| value.checked_add(decimal_digit_count(entry.size)))
            .and_then(|value| value.checked_add(1))
            .and_then(|value| value.checked_add(size))
            .and_then(|value| value.checked_add(1))
            .ok_or_else(|| {
                git_tree_invalid(
                    entry.oid.as_bytes(),
                    "batch output cannot fit in host memory",
                )
            })?;
    }
    Ok(output_bytes)
}

fn decimal_digit_count(mut value: u64) -> usize {
    let mut digits = 1;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

pub(in crate::resolution::source) fn assign_git_batch_output(
    entries: &mut [GitTreeEntry],
    output: Vec<u8>,
) -> Result<(), SourceResolveError> {
    let mut remaining = output.as_slice();
    let mut offset = 0_usize;
    let mut ranges = Vec::with_capacity(entries.len());
    for entry in entries
        .iter()
        .filter(|entry| !matches!(&entry.kind, GitTreeEntryKind::Tree))
    {
        let Some(header_end) = remaining.iter().position(|byte| *byte == b'\n') else {
            return Err(git_tree_invalid(
                entry.oid.as_bytes(),
                "truncated cat-file batch header",
            ));
        };
        let header = &remaining[..=header_end];
        let expected_header = format!("{} blob {}\n", entry.oid, entry.size);
        if header != expected_header.as_bytes() {
            return Err(git_tree_invalid(
                entry.oid.as_bytes(),
                "cat-file batch header did not match the exact requested blob",
            ));
        }
        remaining = &remaining[header_end + 1..];
        offset = offset
            .checked_add(header_end + 1)
            .ok_or_else(|| git_tree_invalid(entry.oid.as_bytes(), "batch offset overflow"))?;
        let size = usize::try_from(entry.size).map_err(|_| {
            git_tree_invalid(entry.oid.as_bytes(), "blob cannot fit in host memory")
        })?;
        let Some(bytes) = remaining.get(..size) else {
            return Err(git_tree_invalid(
                entry.oid.as_bytes(),
                "truncated cat-file batch blob",
            ));
        };
        if remaining.get(size) != Some(&b'\n') {
            return Err(git_tree_invalid(
                entry.oid.as_bytes(),
                "cat-file batch blob lacks its separator",
            ));
        }
        if matches!(&entry.kind, GitTreeEntryKind::Symlink { .. }) {
            validate_git_symlink_target(&entry.relative_bytes, bytes)?;
        }
        let end = offset
            .checked_add(size)
            .ok_or_else(|| git_tree_invalid(entry.oid.as_bytes(), "batch offset overflow"))?;
        ranges.push(offset..end);
        remaining = &remaining[size + 1..];
        offset = end
            .checked_add(1)
            .ok_or_else(|| git_tree_invalid(entry.oid.as_bytes(), "batch offset overflow"))?;
    }
    if !remaining.is_empty() {
        return Err(git_tree_invalid(
            Vec::new(),
            "cat-file batch returned an unexpected trailing response",
        ));
    }
    let batch = Arc::new(output);
    for (entry, range) in entries
        .iter_mut()
        .filter(|entry| !matches!(&entry.kind, GitTreeEntryKind::Tree))
        .zip(ranges)
    {
        match &mut entry.kind {
            GitTreeEntryKind::Tree => unreachable!("tree rows are excluded from blob assignment"),
            GitTreeEntryKind::File { bytes, .. } => {
                *bytes = GitBlobBytes {
                    batch: Arc::clone(&batch),
                    start: range.start,
                    end: range.end,
                };
            }
            GitTreeEntryKind::Symlink { target_bytes } => {
                *target_bytes = GitBlobBytes {
                    batch: Arc::clone(&batch),
                    start: range.start,
                    end: range.end,
                };
            }
        }
    }
    Ok(())
}

pub(in crate::resolution::source) struct PendingGitBatchRequest {
    pub(in crate::resolution::source) parent: CapabilityDirectory,
    pub(in crate::resolution::source) name: OsString,
    pub(in crate::resolution::source) display_path: PathBuf,
    pub(in crate::resolution::source) file: Option<File>,
    pub(in crate::resolution::source) identity: Option<CapabilityMetadata>,
    pub(in crate::resolution::source) removed: bool,
}

impl PendingGitBatchRequest {
    pub(in crate::resolution::source) fn create(
        entry: &CapabilityDirectory,
        entry_root: &Path,
    ) -> Result<Self, SourceResolveError> {
        let parent = entry
            .try_clone()
            .map_err(|error| io_error(entry_root, error))?;
        for _ in 0..128 {
            let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let name = OsString::from(format!(
                ".omega-cat-file-batch.{}.{}",
                std::process::id(),
                sequence
            ));
            let display_path = entry_root.join(&name);
            let mut options = CapabilityOpenOptions::new();
            options
                .read(true)
                .write(true)
                .create_new(true)
                .follow(FollowSymlinks::No);
            #[cfg(unix)]
            options.mode(0o600);
            let capability_file = match parent.open_with(&name, &options) {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(io_error(&display_path, error)),
            };
            let file = capability_file.into_std();
            let mut pending = Self {
                parent,
                name,
                display_path,
                file: Some(file),
                identity: None,
                removed: false,
            };
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;

                let mut permissions = pending
                    .file()
                    .metadata()
                    .map_err(|error| io_error(&pending.display_path, error))?
                    .permissions();
                permissions.set_mode(0o600);
                pending
                    .file()
                    .set_permissions(permissions)
                    .map_err(|error| io_error(&pending.display_path, error))?;
            }
            let identity = pending
                .parent
                .symlink_metadata(&pending.name)
                .map_err(|error| io_error(&pending.display_path, error))?;
            pending.identity = Some(identity);
            pending.verify_current()?;
            return Ok(pending);
        }
        Err(cache_invalid(
            entry_root,
            "could not allocate a unique Git batch-request file",
        ))
    }

    fn file(&self) -> &File {
        self.file
            .as_ref()
            .expect("live Git batch request retains its file")
    }

    fn file_mut(&mut self) -> &mut File {
        self.file
            .as_mut()
            .expect("live Git batch request retains its file")
    }

    pub(in crate::resolution::source) fn verify_current(&self) -> Result<(), SourceResolveError> {
        let identity = self.identity.as_ref().ok_or_else(|| {
            cache_invalid(
                &self.display_path,
                "Git batch-request identity has not been retained",
            )
        })?;
        verify_git_batch_request_identity(
            &self.parent,
            &self.name,
            &self.display_path,
            self.file(),
            identity,
        )
    }

    pub(in crate::resolution::source) fn remove(&mut self) -> Result<(), SourceResolveError> {
        self.verify_current()?;
        drop(self.file.take());
        let named = self
            .parent
            .symlink_metadata(&self.name)
            .map_err(|error| io_error(&self.display_path, error))?;
        if named.file_type().is_symlink()
            || !named.is_file()
            || !self
                .identity
                .as_ref()
                .is_some_and(|identity| same_capability_file_identity(identity, &named))
        {
            return Err(cache_invalid(
                &self.display_path,
                "Git batch-request name no longer identifies the retained file",
            ));
        }
        self.parent
            .remove_file(&self.name)
            .map_err(|error| io_error(&self.display_path, error))?;
        self.parent
            .try_clone()
            .map_err(|error| io_error(&self.display_path, error))?
            .into_std_file()
            .sync_all()
            .map_err(|error| io_error(&self.display_path, error))?;
        self.removed = true;
        Ok(())
    }
}

fn verify_git_batch_request_identity(
    parent: &CapabilityDirectory,
    name: &OsStr,
    path: &Path,
    file: &File,
    expected: &CapabilityMetadata,
) -> Result<(), SourceResolveError> {
    let named = parent
        .symlink_metadata(name)
        .map_err(|error| io_error(path, error))?;
    let opened = file.metadata().map_err(|error| io_error(path, error))?;
    if named.file_type().is_symlink()
        || !named.is_file()
        || !opened.is_file()
        || !same_capability_file_identity(expected, &named)
        || !same_std_and_capability_file_identity(&opened, expected)
    {
        return Err(cache_invalid(
            path,
            "Git batch-request name does not identify the retained file",
        ));
    }
    verify_capability_cache_node_owner_and_mode(CacheCustodyKind::Git, path, &named)?;
    #[cfg(unix)]
    {
        use cap_fs_ext::OsMetadataExt;

        if named.mode() & 0o777 != 0o600 {
            return Err(cache_invalid(
                path,
                "Git batch-request file does not have exact private mode 0600",
            ));
        }
    }
    verify_macos_open_cache_extended_acl_custody(CacheCustodyKind::Git, path, file)?;
    verify_windows_open_cache_custody(CacheCustodyKind::Git, path, file)
}

impl Drop for PendingGitBatchRequest {
    fn drop(&mut self) {
        if self.removed {
            return;
        }
        let Ok(retained_name) = self.parent.symlink_metadata(&self.name) else {
            return;
        };
        if retained_name.file_type().is_symlink() || !retained_name.is_file() {
            return;
        }
        if let Some(file) = self.file.as_ref() {
            let Ok(opened) = file.metadata() else {
                return;
            };
            if !opened.is_file() || !same_std_and_capability_file_identity(&opened, &retained_name)
            {
                return;
            }
        } else if !self
            .identity
            .as_ref()
            .is_some_and(|identity| same_capability_file_identity(identity, &retained_name))
        {
            return;
        }
        drop(self.file.take());
        if let Ok(current_name) = self.parent.symlink_metadata(&self.name)
            && !current_name.file_type().is_symlink()
            && current_name.is_file()
            && same_capability_file_identity(&retained_name, &current_name)
        {
            let _ = self.parent.remove_file(&self.name);
        }
    }
}

#[cfg(test)]
struct TemporaryFileGuard {
    pub(in crate::resolution::source) path: PathBuf,
}

#[cfg(test)]
impl Drop for TemporaryFileGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
