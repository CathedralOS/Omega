use psi_checked_interpreter::{FilesystemSponsor, FilesystemSponsorNamespaceEntryKind};
use psi_diagnostics::Diagnostic;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Seek};
use std::path::{Component, Path};

const STAGED_OUTPUT_TREE_COMMITMENT_DOMAIN: &[u8] = b"OMEGA-BUILD-STAGED-OUTPUT-TREE\0";
const STAGED_OUTPUT_TREE_SCHEMA_VERSION: u32 = 1;
const STAGED_OUTPUT_ROOT_TAG: u8 = 1;
const DIRECTORY_MODE: u32 = 0o040000;
const FILE_MODE: u32 = 0o100644;
const EXECUTABLE_FILE_MODE: u32 = 0o100755;
const SYMLINK_MODE: u32 = 0o120000;
const MAX_STAGED_OUTPUT_ENTRIES: usize = 4_096;
const MAX_STAGED_OUTPUT_UNIQUE_FILE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_STAGED_OUTPUT_PATH_BYTES: usize = 16 * 1024 * 1024;

/// Compiler-issued identity of the complete canonical staged content tree
/// immediately after successful build-machine evaluation. This is a
/// commitment, not retained content or a replay receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildStagedOutputTreeCommitment {
    digest: [u8; 32],
    entry_count: u64,
    file_bytes: u64,
}

impl BuildStagedOutputTreeCommitment {
    pub const fn digest(self) -> [u8; 32] {
        self.digest
    }

    pub const fn entry_count(self) -> u64 {
        self.entry_count
    }

    pub const fn file_bytes(self) -> u64 {
        self.file_bytes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileContentCommitment {
    length: u64,
    digest: [u8; 32],
    executable: bool,
    identity: HostFileIdentity,
}

#[derive(Debug)]
enum StagedOutputEntryKind {
    Directory,
    File(FileContentCommitment),
    Symlink { target: Vec<u8> },
}

#[derive(Debug)]
struct StagedOutputEntry {
    relative_path: Vec<u8>,
    kind: StagedOutputEntryKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpectedEntryKind {
    Directory,
    File { group: u64, extent: u64 },
    Symlink { spelling_bytes: u64 },
}

#[cfg(unix)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HostFileIdentity {
    device: u64,
    inode: u64,
}

#[cfg(not(unix))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HostFileIdentity;

pub(super) fn empty() -> BuildStagedOutputTreeCommitment {
    finish_commitment(Vec::new(), 0)
}

pub(super) fn capture(
    root: &Path,
    sponsor: &FilesystemSponsor,
) -> Result<BuildStagedOutputTreeCommitment, Vec<Diagnostic>> {
    let snapshot = sponsor.namespace_snapshot().map_err(|error| {
        diagnostics(format!(
            "cannot inspect sponsored build staged-output namespace: {error}"
        ))
    })?;
    if snapshot.transaction_prepared() || snapshot.open_descriptors() != 0 {
        return Err(diagnostics(
            "build staged-output capture requires a quiescent sponsor with no prepared transaction or open descriptor",
        ));
    }
    let bound_root = sponsor.bind_path(root).map_err(|error| {
        diagnostics(format!(
            "cannot bind build staged-output root `{}` to its sponsor: {error}",
            root.display()
        ))
    })?;
    let sponsor_root = bound_root.relative();
    let mut root_is_sponsored_directory = false;
    let mut expected = BTreeMap::new();
    for entry in snapshot.entries() {
        if entry.relative_path() == sponsor_root {
            root_is_sponsored_directory =
                entry.kind() == FilesystemSponsorNamespaceEntryKind::Directory;
            continue;
        }
        let Ok(relative) = entry.relative_path().strip_prefix(sponsor_root) else {
            continue;
        };
        if relative.as_os_str().is_empty() {
            continue;
        }
        let kind = match entry.kind() {
            FilesystemSponsorNamespaceEntryKind::Directory => ExpectedEntryKind::Directory,
            FilesystemSponsorNamespaceEntryKind::Symlink { spelling_bytes } => {
                ExpectedEntryKind::Symlink { spelling_bytes }
            }
            FilesystemSponsorNamespaceEntryKind::Object { group, extent } => {
                ExpectedEntryKind::File { group, extent }
            }
        };
        expected.insert(relative.to_path_buf(), kind);
    }
    if !root_is_sponsored_directory {
        return Err(diagnostics(format!(
            "build staged-output root `{}` is not the sponsor's committed directory",
            root.display()
        )));
    }
    if expected.len() > MAX_STAGED_OUTPUT_ENTRIES {
        return Err(diagnostics(format!(
            "build staged-output tree exceeds its {MAX_STAGED_OUTPUT_ENTRIES}-entry ceiling"
        )));
    }

    let metadata = std::fs::symlink_metadata(root).map_err(|error| {
        diagnostics(format!(
            "cannot inspect build staged-output root `{}`: {error}",
            root.display()
        ))
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(diagnostics(format!(
            "build staged-output root `{}` must be a concrete directory",
            root.display()
        )));
    }

    let mut observed_paths = BTreeSet::new();
    let mut entries = Vec::with_capacity(expected.len());
    let mut pending = vec![root.to_path_buf()];
    let mut total_path_bytes = 0usize;
    let mut total_unique_file_bytes = 0u64;
    let mut file_groups = BTreeMap::<u64, FileContentCommitment>::new();
    while let Some(directory) = pending.pop() {
        let directory_metadata = std::fs::symlink_metadata(&directory).map_err(|error| {
            diagnostics(format!(
                "cannot inspect build staged-output directory `{}`: {error}",
                directory.display()
            ))
        })?;
        if directory_metadata.file_type().is_symlink() || !directory_metadata.is_dir() {
            return Err(diagnostics(format!(
                "build staged-output directory `{}` changed kind during capture",
                directory.display()
            )));
        }
        let children = std::fs::read_dir(&directory).map_err(|error| {
            diagnostics(format!(
                "cannot enumerate build staged-output directory `{}`: {error}",
                directory.display()
            ))
        })?;
        let mut bounded_children = Vec::new();
        for child in children {
            if observed_paths.len() + bounded_children.len() == MAX_STAGED_OUTPUT_ENTRIES {
                return Err(diagnostics(format!(
                    "build staged-output tree exceeds its {MAX_STAGED_OUTPUT_ENTRIES}-entry ceiling"
                )));
            }
            bounded_children.push(child.map_err(|error| {
                diagnostics(format!(
                    "cannot enumerate build staged-output directory `{}`: {error}",
                    directory.display()
                ))
            })?);
        }
        bounded_children.sort_by(|left, right| left.file_name().cmp(&right.file_name()));

        for child in bounded_children {
            let path = child.path();
            let relative_native = path.strip_prefix(root).map_err(|_| {
                diagnostics(format!(
                    "build staged-output entry `{}` escaped root `{}`",
                    path.display(),
                    root.display()
                ))
            })?;
            let expected_kind = expected.get(relative_native).copied().ok_or_else(|| {
                diagnostics(format!(
                    "build staged-output entry `{}` is absent from sponsor custody",
                    path.display()
                ))
            })?;
            if !observed_paths.insert(relative_native.to_path_buf()) {
                return Err(diagnostics(format!(
                    "build staged-output entry `{}` was observed more than once",
                    path.display()
                )));
            }
            let relative_path = canonical_relative_path(relative_native, &path)?;
            total_path_bytes = reserve_path_bytes(total_path_bytes, relative_path.len())?;
            let metadata = std::fs::symlink_metadata(&path).map_err(|error| {
                diagnostics(format!(
                    "cannot inspect build staged-output entry `{}`: {error}",
                    path.display()
                ))
            })?;
            let file_type = metadata.file_type();
            let kind = match expected_kind {
                ExpectedEntryKind::Directory if file_type.is_dir() => {
                    pending.push(path);
                    StagedOutputEntryKind::Directory
                }
                ExpectedEntryKind::File { group, extent } if file_type.is_file() => {
                    if metadata.len() != extent {
                        return Err(diagnostics(format!(
                            "build staged-output file `{}` disagrees with sponsor extent",
                            path.display()
                        )));
                    }
                    match file_groups.get(&group) {
                        Some(existing) => {
                            validate_hard_link_alias(&path, &metadata, extent, existing)?;
                            StagedOutputEntryKind::File(*existing)
                        }
                        None => {
                            total_unique_file_bytes = total_unique_file_bytes
                                .checked_add(extent)
                                .filter(|total| *total <= MAX_STAGED_OUTPUT_UNIQUE_FILE_BYTES)
                                .ok_or_else(|| {
                                    diagnostics(format!(
                                        "build staged-output tree exceeds its {MAX_STAGED_OUTPUT_UNIQUE_FILE_BYTES}-byte unique-content ceiling"
                                    ))
                                })?;
                            let content = capture_file(&path, &metadata, extent)?;
                            file_groups.insert(group, content);
                            StagedOutputEntryKind::File(content)
                        }
                    }
                }
                ExpectedEntryKind::Symlink { spelling_bytes } if file_type.is_symlink() => {
                    let target = std::fs::read_link(&path).map_err(|error| {
                        diagnostics(format!(
                            "cannot read build staged-output symlink `{}`: {error}",
                            path.display()
                        ))
                    })?;
                    let target = canonical_symlink_target(&target, &relative_path, &path)?;
                    if u64::try_from(target.len()).ok() != Some(spelling_bytes) {
                        return Err(diagnostics(format!(
                            "build staged-output symlink `{}` disagrees with sponsor target length",
                            path.display()
                        )));
                    }
                    total_path_bytes = reserve_path_bytes(total_path_bytes, target.len())?;
                    StagedOutputEntryKind::Symlink { target }
                }
                _ => {
                    return Err(diagnostics(format!(
                        "build staged-output entry `{}` disagrees with sponsor kind",
                        path.display()
                    )));
                }
            };
            entries.push(StagedOutputEntry {
                relative_path,
                kind,
            });
        }
    }
    if observed_paths.len() != expected.len() {
        let missing = expected
            .keys()
            .find(|path| !observed_paths.contains(*path))
            .expect("unequal sponsored and observed counts have one missing path");
        return Err(diagnostics(format!(
            "sponsored build staged-output entry `{}` is missing from the physical tree",
            root.join(missing).display()
        )));
    }
    Ok(finish_commitment(entries, total_unique_file_bytes))
}

fn finish_commitment(
    mut entries: Vec<StagedOutputEntry>,
    file_bytes: u64,
) -> BuildStagedOutputTreeCommitment {
    entries.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    let entry_count = u64::try_from(entries.len()).expect("staged-output entry ceiling fits u64");
    let mut digest = Sha256::new();
    digest.update(STAGED_OUTPUT_TREE_COMMITMENT_DOMAIN);
    digest.update(STAGED_OUTPUT_TREE_SCHEMA_VERSION.to_le_bytes());
    digest.update([STAGED_OUTPUT_ROOT_TAG]);
    digest.update(entry_count.to_le_bytes());
    for entry in entries {
        hash_field(&mut digest, &entry.relative_path);
        match entry.kind {
            StagedOutputEntryKind::Directory => {
                digest.update([0]);
                digest.update(DIRECTORY_MODE.to_le_bytes());
            }
            StagedOutputEntryKind::File(content) => {
                digest.update([1]);
                digest.update(
                    if content.executable {
                        EXECUTABLE_FILE_MODE
                    } else {
                        FILE_MODE
                    }
                    .to_le_bytes(),
                );
                digest.update(content.length.to_le_bytes());
                digest.update(content.digest);
            }
            StagedOutputEntryKind::Symlink { target } => {
                digest.update([2]);
                digest.update(SYMLINK_MODE.to_le_bytes());
                hash_field(&mut digest, &target);
            }
        }
    }
    BuildStagedOutputTreeCommitment {
        digest: digest.finalize().into(),
        entry_count,
        file_bytes,
    }
}

fn capture_file(
    path: &Path,
    path_metadata: &std::fs::Metadata,
    expected_extent: u64,
) -> Result<FileContentCommitment, Vec<Diagnostic>> {
    let mut file = std::fs::File::open(path).map_err(|error| {
        diagnostics(format!(
            "cannot open build staged-output file `{}`: {error}",
            path.display()
        ))
    })?;
    let before = file.metadata().map_err(|error| {
        diagnostics(format!(
            "cannot inspect opened build staged-output file `{}`: {error}",
            path.display()
        ))
    })?;
    if !same_file_observation(path_metadata, &before) || before.len() != expected_extent {
        return Err(diagnostics(format!(
            "build staged-output file `{}` changed before content capture",
            path.display()
        )));
    }
    let first = hash_reader(&mut file, expected_extent, path)?;
    file.rewind().map_err(|error| {
        diagnostics(format!(
            "cannot rewind build staged-output file `{}`: {error}",
            path.display()
        ))
    })?;
    let second = hash_reader(&mut file, expected_extent, path)?;
    let after = file.metadata().map_err(|error| {
        diagnostics(format!(
            "cannot re-inspect build staged-output file `{}`: {error}",
            path.display()
        ))
    })?;
    if first != second || !same_file_observation(&before, &after) {
        return Err(diagnostics(format!(
            "build staged-output file `{}` changed while content was captured",
            path.display()
        )));
    }
    Ok(FileContentCommitment {
        length: expected_extent,
        digest: first,
        executable: is_executable(&before),
        identity: host_file_identity(&before),
    })
}

#[cfg(unix)]
fn validate_hard_link_alias(
    path: &Path,
    metadata: &std::fs::Metadata,
    expected_extent: u64,
    expected: &FileContentCommitment,
) -> Result<(), Vec<Diagnostic>> {
    if metadata.len() != expected_extent
        || host_file_identity(metadata) != expected.identity
        || is_executable(metadata) != expected.executable
    {
        return Err(diagnostics(format!(
            "build staged-output hard-link group disagrees at `{}`",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(not(unix))]
fn validate_hard_link_alias(
    path: &Path,
    metadata: &std::fs::Metadata,
    expected_extent: u64,
    expected: &FileContentCommitment,
) -> Result<(), Vec<Diagnostic>> {
    let observed = capture_file(path, metadata, expected_extent)?;
    if &observed != expected {
        return Err(diagnostics(format!(
            "build staged-output hard-link group disagrees at `{}`",
            path.display()
        )));
    }
    Ok(())
}

fn hash_reader(
    reader: &mut std::fs::File,
    expected_extent: u64,
    path: &Path,
) -> Result<[u8; 32], Vec<Diagnostic>> {
    let mut digest = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer).map_err(|error| {
            diagnostics(format!(
                "cannot read build staged-output file `{}`: {error}",
                path.display()
            ))
        })?;
        if read == 0 {
            break;
        }
        total = total.checked_add(read as u64).ok_or_else(|| {
            diagnostics(format!(
                "build staged-output file `{}` length overflowed during capture",
                path.display()
            ))
        })?;
        if total > expected_extent {
            return Err(diagnostics(format!(
                "build staged-output file `{}` grew during capture",
                path.display()
            )));
        }
        digest.update(&buffer[..read]);
    }
    if total != expected_extent {
        return Err(diagnostics(format!(
            "build staged-output file `{}` changed length during capture",
            path.display()
        )));
    }
    Ok(digest.finalize().into())
}

fn canonical_relative_path(
    relative: &Path,
    diagnostic_path: &Path,
) -> Result<Vec<u8>, Vec<Diagnostic>> {
    let mut output = Vec::new();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(diagnostics(format!(
                "build staged-output entry `{}` has a non-canonical relative path",
                diagnostic_path.display()
            )));
        };
        let component = component.to_str().ok_or_else(|| {
            diagnostics(format!(
                "build staged-output entry `{}` has a non-UTF-8 path component",
                diagnostic_path.display()
            ))
        })?;
        validate_portable_component(component.as_bytes(), diagnostic_path)?;
        if !output.is_empty() {
            output.push(b'/');
        }
        output.extend_from_slice(component.as_bytes());
    }
    if output.is_empty() {
        return Err(diagnostics("build staged-output entry has an empty path"));
    }
    Ok(output)
}

fn canonical_symlink_target(
    target: &Path,
    link_relative_path: &[u8],
    diagnostic_path: &Path,
) -> Result<Vec<u8>, Vec<Diagnostic>> {
    let target = target.to_str().ok_or_else(|| {
        diagnostics(format!(
            "build staged-output symlink `{}` has a non-UTF-8 target",
            diagnostic_path.display()
        ))
    })?;
    let bytes = target.as_bytes();
    if bytes.is_empty() || bytes.starts_with(b"/") || bytes.contains(&b'\\') || bytes.contains(&0) {
        return Err(diagnostics(format!(
            "build staged-output symlink `{}` must have a nonempty relative slash-separated target",
            diagnostic_path.display()
        )));
    }
    let mut resolved_depth = link_relative_path.split(|byte| *byte == b'/').count() - 1;
    for component in bytes.split(|byte| *byte == b'/') {
        if component.is_empty() || component == b"." {
            return Err(diagnostics(format!(
                "build staged-output symlink `{}` has a non-canonical target",
                diagnostic_path.display()
            )));
        }
        if component == b".." {
            if resolved_depth == 0 {
                return Err(diagnostics(format!(
                    "build staged-output symlink `{}` escapes the Output root",
                    diagnostic_path.display()
                )));
            }
            resolved_depth -= 1;
        } else {
            validate_portable_component(component, diagnostic_path)?;
            resolved_depth += 1;
        }
    }
    Ok(bytes.to_vec())
}

fn validate_portable_component(
    component: &[u8],
    diagnostic_path: &Path,
) -> Result<(), Vec<Diagnostic>> {
    if component.is_empty()
        || component == b"."
        || component == b".."
        || component.iter().any(|byte| {
            *byte < 0x20
                || matches!(
                    *byte,
                    b'\\' | b':' | b'*' | b'?' | b'"' | b'<' | b'>' | b'|'
                )
        })
        || matches!(component.last(), Some(b'.' | b' '))
    {
        return Err(diagnostics(format!(
            "build staged-output entry `{}` has a non-portable path component",
            diagnostic_path.display()
        )));
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
        return Err(diagnostics(format!(
            "build staged-output entry `{}` uses a reserved portable device name",
            diagnostic_path.display()
        )));
    }
    Ok(())
}

fn reserve_path_bytes(current: usize, additional: usize) -> Result<usize, Vec<Diagnostic>> {
    current
        .checked_add(additional)
        .filter(|total| *total <= MAX_STAGED_OUTPUT_PATH_BYTES)
        .ok_or_else(|| {
            diagnostics(format!(
                "build staged-output tree exceeds its {MAX_STAGED_OUTPUT_PATH_BYTES}-byte path and symlink-target ceiling"
            ))
        })
}

#[cfg(unix)]
fn host_file_identity(metadata: &std::fs::Metadata) -> HostFileIdentity {
    use std::os::unix::fs::MetadataExt;
    HostFileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    }
}

#[cfg(not(unix))]
fn host_file_identity(_metadata: &std::fs::Metadata) -> HostFileIdentity {
    HostFileIdentity
}

#[cfg(unix)]
fn is_executable(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    metadata.mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_metadata: &std::fs::Metadata) -> bool {
    false
}

#[cfg(unix)]
fn same_file_observation(left: &std::fs::Metadata, right: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    left.file_type().is_file()
        && right.file_type().is_file()
        && left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.len() == right.len()
        && left.mode() == right.mode()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

#[cfg(not(unix))]
fn same_file_observation(left: &std::fs::Metadata, right: &std::fs::Metadata) -> bool {
    left.file_type().is_file()
        && right.file_type().is_file()
        && left.len() == right.len()
        && left.permissions().readonly() == right.permissions().readonly()
        && left.modified().ok() == right.modified().ok()
}

fn hash_field(digest: &mut Sha256, bytes: &[u8]) {
    digest.update(
        u64::try_from(bytes.len())
            .expect("staged-output field length fits u64")
            .to_le_bytes(),
    );
    digest.update(bytes);
}

fn diagnostics(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::error(message)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        session: PathBuf,
        root: PathBuf,
        sponsor: FilesystemSponsor,
    }

    impl Fixture {
        fn new(label: &str) -> Self {
            let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
            let session = std::env::temp_dir().join(format!(
                "omega-staged-output-{label}-{}-{sequence}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&session);
            std::fs::create_dir(&session).unwrap();
            let session = std::fs::canonicalize(session).unwrap();
            let sponsor = FilesystemSponsor::new(&session).unwrap();
            let root = session.join("output");
            let fixture = Self {
                session,
                root,
                sponsor,
            };
            fixture.create_directory(Path::new(""));
            fixture
        }

        fn bind(&self, relative: &Path) -> psi_checked_interpreter::FilesystemSponsorPath {
            self.sponsor.bind_path(self.root.join(relative)).unwrap()
        }

        fn create_directory(&self, relative: &Path) {
            let path = self.root.join(relative);
            let prepared = self
                .sponsor
                .prepare_create_directory(&self.bind(relative))
                .unwrap();
            std::fs::create_dir(&path).unwrap();
            prepared.commit().unwrap();
        }

        fn create_file(&self, relative: &Path, bytes: &[u8]) {
            let prepared = self
                .sponsor
                .prepare_create_object(&self.bind(relative), bytes.len() as u64)
                .unwrap();
            std::fs::write(self.root.join(relative), bytes).unwrap();
            prepared.commit().unwrap();
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.session);
        }
    }

    fn populated_fixture(label: &str, bytes: &[u8]) -> Fixture {
        let fixture = Fixture::new(label);
        fixture.create_directory(Path::new("nested"));
        fixture.create_file(Path::new("nested/artifact.bin"), bytes);
        fixture.create_directory(Path::new("empty"));
        fixture
    }

    #[test]
    fn commitment_is_relocation_stable_and_binds_paths_kinds_and_bytes() {
        let first = populated_fixture("first", b"payload");
        let relocated = populated_fixture("relocated", b"payload");
        let changed = populated_fixture("changed", b"changed");
        let first_commitment = capture(&first.root, &first.sponsor).unwrap();
        let relocated_commitment = capture(&relocated.root, &relocated.sponsor).unwrap();
        let changed_commitment = capture(&changed.root, &changed.sponsor).unwrap();
        assert_eq!(first_commitment, relocated_commitment);
        assert_eq!(first_commitment.entry_count(), 3);
        assert_eq!(first_commitment.file_bytes(), 7);
        assert_ne!(first_commitment.digest(), changed_commitment.digest());
    }

    #[cfg(unix)]
    #[test]
    fn commitment_normalizes_hard_links_and_binds_executable_mode() {
        use std::os::unix::fs::PermissionsExt;

        let fixture = Fixture::new("hard-links");
        fixture.create_file(Path::new("first"), b"payload");
        let first = fixture.bind(Path::new("first"));
        let second = fixture.bind(Path::new("second"));
        let prepared = fixture.sponsor.prepare_hard_link(&first, &second).unwrap();
        std::fs::hard_link(fixture.root.join("first"), fixture.root.join("second")).unwrap();
        prepared.commit().unwrap();

        let ordinary = capture(&fixture.root, &fixture.sponsor).unwrap();
        assert_eq!(ordinary.entry_count(), 2);
        assert_eq!(ordinary.file_bytes(), 7);

        let mut permissions = std::fs::metadata(fixture.root.join("first"))
            .unwrap()
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(fixture.root.join("first"), permissions).unwrap();
        let executable = capture(&fixture.root, &fixture.sponsor).unwrap();
        assert_ne!(ordinary.digest(), executable.digest());
        assert_eq!(executable.file_bytes(), 7);
    }

    #[test]
    fn rejects_portability_collisions() {
        assert!(validate_portable_component(b"NUL.txt", Path::new("NUL.txt")).is_err());
        assert!(validate_portable_component(b"COM1", Path::new("COM1")).is_err());
        assert!(validate_portable_component(b"trailing.", Path::new("trailing.")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_external_symlink_targets_and_unsponsored_entries() {
        use std::os::unix::fs::symlink;

        let symlink_fixture = Fixture::new("symlink");
        let prepared = symlink_fixture
            .sponsor
            .prepare_create_symlink(&symlink_fixture.bind(Path::new("link")), b"/outside")
            .unwrap();
        symlink("/outside", symlink_fixture.root.join("link")).unwrap();
        prepared.commit().unwrap();
        assert!(capture(&symlink_fixture.root, &symlink_fixture.sponsor).is_err());

        let unsponsored_fixture = Fixture::new("unsponsored");
        std::fs::write(unsponsored_fixture.root.join("extra"), b"bytes").unwrap();
        assert!(capture(&unsponsored_fixture.root, &unsponsored_fixture.sponsor).is_err());
    }
}
