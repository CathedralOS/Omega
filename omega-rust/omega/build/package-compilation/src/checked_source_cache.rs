//! Persistent, content-verified cache of checked canonical-source metadata.
//!
//! `source_snapshot` commits the complete physical source root by hashing
//! every member's bytes on every capture, then re-traversing for stability.
//! For a source tree that is unchanged across invocations — the ordinary
//! dependency case — that work is identical each time. This module retains
//! the committed `CanonicalFilesystemMetadataIndex` — the checked source
//! — in a bounded on-disk store keyed by the canonical root path and a
//! stat-identity fingerprint: device, inode, length, canonical mode,
//! mtime, and ctime per member, plus inert symlink target bytes. A later
//! capture whose live metadata reproduces the same fingerprint replays the
//! stored index without rehashing; any drift changes the fingerprint and
//! produces a cold capture.
//!
//! The fingerprint commits exactly the host observations the compiler's own
//! drift checks already compare between two reads of one file, so replaying
//! on a fingerprint match adds no new trust basis: file content whose
//! device/inode/length/mode/mtime/ctime all match is the content the
//! recorded index describes, under the same filesystem premises the
//! existing capture relies on. On Unix a ctime collision cannot be
//! fabricated without replacing the inode, which the fingerprint also
//! commits.
//!
//! Records are self-verifying and the cache is derived data only: a record
//! file is admitted only when its schema, strict canonical encoding,
//! trailing payload digest, embedded fingerprint, and embedded root all
//! match the lookup, and the stored rows re-validate through
//! `CanonicalFilesystemMetadataIndex::version_1`. Corruption, truncation,
//! foreign records, and stat drift are misses — never trusted rows — and a
//! full cache declines new records rather than silently evicting retained
//! evidence. Nothing here replays a checked verdict, review row, or build
//! output; only the metadata index is retained.
//!
//! The warm path performs two independent stat-only traversals and serves a
//! hit only when they agree, mirroring the cold capture's recapture
//! contract so a mid-traversal edit cannot mint an index describing two
//! different trees.
//!
//! Package resolution opts in per root custody: each resolved root carries
//! the retained `checked-source-cache` lane child beneath the resolver
//! storage lane that produced its snapshot, and compiler-input preparation
//! routes `with_canonical_source_metadata` through
//! [`CheckedSourceCache::capture`] when the lane is present, degrading to
//! the cold capture when the directory cannot be opened. This module owns
//! the store, fingerprint, and capture protocol; that choice of lane and
//! the caller opt-in live with the resolving consumers.

use checked_interpreter::{
    CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT, CanonicalFilesystemMetadataIndex,
    CanonicalFilesystemMetadataRow, CanonicalFilesystemMetadataRowKind,
    FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT, canonical_filesystem_metadata_path_is_canonical,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const AGGREGATE_CONTENT_BYTE_LIMIT: u64 = 512 * 1024 * 1024;

const RECORD_MAGIC: &[u8; 8] = b"OMGCSRC\x01";
const RECORD_KEY_DOMAIN: &[u8] = b"OMEGA-CHECKED-SOURCE-CACHE-KEY-V1\0";
const RECORD_EXTENSION: &str = "rec";
const DEFAULT_MAX_RECORDS: usize = 256;
const DEFAULT_MAX_RECORD_BYTES: u64 = 256 * 1024 * 1024;

#[cfg(unix)]
const CANONICAL_DIRECTORY_MODE: u32 = 0o555;
#[cfg(unix)]
const CANONICAL_FILE_MODE: u32 = 0o444;
#[cfg(unix)]
const CANONICAL_EXECUTABLE_FILE_MODE: u32 = 0o555;

/// Capacity contract for one cache directory. A record beyond the byte
/// ceiling, or a directory already holding `max_records` records, declines
/// the store — the capture still returns its freshly checked index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedSourceCacheLimits {
    pub max_records: usize,
    pub max_record_bytes: u64,
}

impl Default for CheckedSourceCacheLimits {
    fn default() -> Self {
        Self {
            max_records: DEFAULT_MAX_RECORDS,
            max_record_bytes: DEFAULT_MAX_RECORD_BYTES,
        }
    }
}

/// Whether one capture replayed a retained index or rebuilt it from source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedSourceCacheOutcome {
    /// The live stat fingerprint matched a verified retained record; no file
    /// bytes were rehashed.
    Warm,
    /// The store had no matching verified record; the canonical index was
    /// rebuilt by the full hashing capture.
    Cold,
}

/// One capture result: the committed canonical index, the cache outcome,
/// and whether the cold path's record reached the store. A `Cold` capture
/// with `record_stored == false` means the store declined (capacity, a
/// racing writer's record, or an I/O failure on derived data); the returned
/// index is always freshly verified regardless.
#[derive(Debug, Clone)]
pub struct CachedSourceIndex {
    index: CanonicalFilesystemMetadataIndex,
    outcome: CheckedSourceCacheOutcome,
    record_stored: bool,
}

impl CachedSourceIndex {
    /// The committed canonical metadata index for this root.
    pub const fn canonical_source_metadata(&self) -> &CanonicalFilesystemMetadataIndex {
        &self.index
    }

    /// Whether the store supplied the index (`Warm`) or rebuilt it (`Cold`).
    pub const fn outcome(&self) -> CheckedSourceCacheOutcome {
        self.outcome
    }

    /// Whether a cold capture's record was published into the store.
    pub const fn record_stored(&self) -> bool {
        self.record_stored
    }
}

/// A bounded directory of retained checked-source indexes, keyed by
/// canonical root and stat fingerprint. The directory is caller-owned
/// storage — for example a project build root — and nothing outside it is
/// touched.
#[derive(Debug, Clone)]
pub struct CheckedSourceCache {
    directory: PathBuf,
    limits: CheckedSourceCacheLimits,
}

impl CheckedSourceCache {
    /// Open an existing cache directory. The directory must already exist;
    /// use [`CheckedSourceCache::open_or_create`] to establish it.
    pub fn open(directory: impl AsRef<Path>) -> Result<Self, String> {
        Self::open_with_limits(directory, CheckedSourceCacheLimits::default())
    }

    /// Open an existing cache directory with explicit capacity limits.
    pub fn open_with_limits(
        directory: impl AsRef<Path>,
        limits: CheckedSourceCacheLimits,
    ) -> Result<Self, String> {
        let directory = directory.as_ref();
        let metadata = std::fs::metadata(directory)
            .map_err(|error| format!("cannot inspect checked-source cache directory: {error}"))?;
        if !metadata.is_dir() {
            return Err("checked-source cache path is not a directory".to_owned());
        }
        let canonical = directory.canonicalize().map_err(|error| {
            format!("cannot canonicalize checked-source cache directory: {error}")
        })?;
        Ok(Self {
            directory: canonical,
            limits,
        })
    }

    /// Establish the cache directory when absent, then open it.
    pub fn open_or_create(directory: impl AsRef<Path>) -> Result<Self, String> {
        let directory = directory.as_ref();
        if !directory.exists() {
            std::fs::create_dir_all(directory).map_err(|error| {
                format!("cannot create checked-source cache directory: {error}")
            })?;
        }
        Self::open(directory)
    }

    /// The canonical cache directory this store writes into.
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// Capture one canonical source root's checked metadata index, replaying
    /// a verified retained record when the live stat fingerprint matches one
    /// and rebuilding through the full hashing capture otherwise. The
    /// returned index always passes the complete canonical validation —
    /// cache misses are indistinguishable from an uncached capture.
    pub fn capture(&self, source_root: &Path) -> Result<CachedSourceIndex, String> {
        let canonical_root = crate::package_compilation::canonical_source_root(source_root)?;
        let observed = stat_capture(&canonical_root)?;
        let fingerprint = stat_fingerprint(&canonical_root, &observed)?;
        let repeated = stat_capture(&canonical_root)?;
        if repeated != observed {
            return Err(
                "canonical Source metadata changed during capture; the complete physical root is not stable"
                    .to_owned(),
            );
        }
        if let Some(index) = self.load(&canonical_root, &fingerprint)? {
            return Ok(CachedSourceIndex {
                index,
                outcome: CheckedSourceCacheOutcome::Warm,
                record_stored: false,
            });
        }
        let index = crate::source_snapshot::capture(&canonical_root)?;
        // The cold capture hashed what it found; before publishing a record
        // keyed to the earlier fingerprint, confirm the tree still carries
        // exactly that stat identity. A tree that drifted under the cold
        // capture is reported unstable instead of associating its new state
        // with the old fingerprint.
        let settled = stat_capture(&canonical_root)?;
        if settled != observed {
            return Err(
                "canonical Source metadata changed during capture; the complete physical root is not stable"
                    .to_owned(),
            );
        }
        let record_stored = self.store(&canonical_root, &fingerprint, &index)?;
        Ok(CachedSourceIndex {
            index,
            outcome: CheckedSourceCacheOutcome::Cold,
            record_stored,
        })
    }

    /// Look up and fully verify one retained record. Any mismatch or
    /// malformation removes the corrupt record when possible and reports a
    /// miss — a cache lookup never trusts unverified bytes.
    fn load(
        &self,
        canonical_root: &Path,
        fingerprint: &[u8; 32],
    ) -> Result<Option<CanonicalFilesystemMetadataIndex>, String> {
        let name = record_name(canonical_root, fingerprint)?;
        let path = self.directory.join(&name);
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(format!(
                    "could not read checked-source cache record {}: {error}",
                    path.display()
                ));
            }
        };
        match decode_record(&bytes, canonical_root, fingerprint) {
            Ok(index) => Ok(Some(index)),
            Err(_) => {
                // Corrupt or foreign record: it is derived data only, so the
                // miss is answered by cold capture; the unverifiable file is
                // removed so it cannot be repeatedly re-presented.
                let _ = std::fs::remove_file(&path);
                Ok(None)
            }
        }
    }

    /// Publish one record under its content key. A full store, a racing
    /// existing record, or an I/O failure declines the store without failing
    /// the caller's freshly verified index.
    fn store(
        &self,
        canonical_root: &Path,
        fingerprint: &[u8; 32],
        index: &CanonicalFilesystemMetadataIndex,
    ) -> Result<bool, String> {
        let name = record_name(canonical_root, fingerprint)?;
        let payload = encode_record(canonical_root, fingerprint, index)?;
        if payload.len() as u64 > self.limits.max_record_bytes {
            return Ok(false);
        }
        let record_count = std::fs::read_dir(&self.directory)
            .map_err(|error| {
                format!(
                    "could not enumerate checked-source cache {}: {error}",
                    self.directory.display()
                )
            })?
            .filter(|entry| {
                entry.as_ref().is_ok_and(|entry| {
                    entry
                        .path()
                        .extension()
                        .and_then(|extension| extension.to_str())
                        == Some(RECORD_EXTENSION)
                })
            })
            .count();
        if record_count >= self.limits.max_records {
            return Ok(false);
        }
        let record_path = self.directory.join(&name);
        if record_path.exists() {
            // Same key already published (a concurrent cold capture or an
            // earlier run); the record is keyed by exact content, so an
            // existing file with the same name is already correct.
            return Ok(false);
        }
        let temporary = self.directory.join(format!(
            ".{}.{}.tmp",
            std::process::id(),
            TEMPORARY_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let written = (|| -> Result<(), String> {
            std::fs::write(&temporary, &payload).map_err(|error| {
                format!(
                    "could not stage checked-source cache record {}: {error}",
                    temporary.display()
                )
            })?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o444))
                    .map_err(|error| {
                        format!(
                            "could not seal checked-source cache record {}: {error}",
                            temporary.display()
                        )
                    })?;
            }
            std::fs::rename(&temporary, &record_path).map_err(|error| {
                format!(
                    "could not publish checked-source cache record {}: {error}",
                    record_path.display()
                )
            })?;
            Ok(())
        })();
        if written.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        written.map(|()| true)
    }
}

static TEMPORARY_SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// One member's stat identity: every host observation the compiler's
/// drift checks already compare, plus inert symlink target bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
struct StatRow {
    kind: StatRowKind,
    /// `dev`/`ino`/`ctime`/`mtime` are host observations, never canonical
    /// input metadata; they pin the recorded rows to this exact tree state.
    device: u64,
    inode: u64,
    mode: u32,
    logical_byte_length: u64,
    modified_unix_seconds: i64,
    modified_unix_nanos: i64,
    changed_unix_seconds: i64,
    changed_unix_nanos: i64,
    symlink_target: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatRowKind {
    Directory,
    File { executable: bool },
    Symlink,
}

/// A metadata-only traversal of the canonical source root under the same
/// membership, canonical-path, mode, and ceiling rules as the hashing
/// capture — without reading file bytes. The returned rows are the
/// fingerprint basis: identical rows mean identical checked metadata.
fn stat_capture(root: &Path) -> Result<BTreeMap<Vec<u8>, StatRow>, String> {
    let mut stack = vec![(root.to_path_buf(), Vec::<u8>::new())];
    let mut rows = BTreeMap::<Vec<u8>, StatRow>::new();
    let mut aggregate_path_bytes = 0usize;
    let mut aggregate_content_bytes = 0u64;
    while let Some((path, relative_path)) = stack.pop() {
        let physical = std::fs::symlink_metadata(&path).map_err(|error| {
            format!("could not inspect canonical Source metadata path: {error}")
        })?;
        if rows.len() >= CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT {
            return Err(format!(
                "canonical Source metadata exceeds its {CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT}-row ceiling"
            ));
        }
        let row = stat_row(&path, &physical, &mut aggregate_content_bytes)?;
        if rows.insert(relative_path.to_vec(), row).is_some() {
            return Err(format!(
                "physical Source traversal duplicated a path: {relative_path:?}"
            ));
        }
        if physical.is_dir() {
            let children = std::fs::read_dir(&path).map_err(|error| {
                format!("could not enumerate canonical Source metadata directory: {error}")
            })?;
            for child in children {
                let child = child.map_err(|error| {
                    format!("could not enumerate canonical Source metadata entry: {error}")
                })?;
                if rows
                    .len()
                    .checked_add(stack.len())
                    .and_then(|count| count.checked_add(1))
                    .is_none_or(|count| count > CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT)
                {
                    return Err(format!(
                        "canonical Source metadata exceeds its {CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT}-row ceiling"
                    ));
                }
                let name = os_str_bytes(&child.file_name())?;
                let mut child_relative = relative_path.clone();
                if !child_relative.is_empty() {
                    child_relative.push(b'/');
                }
                child_relative.extend_from_slice(&name);
                aggregate_path_bytes = aggregate_path_bytes
                    .checked_add(child_relative.len())
                    .filter(|bytes| *bytes <= FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT)
                    .ok_or_else(|| {
                        format!(
                            "canonical Source metadata path bytes exceed {FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT}"
                        )
                    })?;
                if !canonical_filesystem_metadata_path_is_canonical(&child_relative, false) {
                    return Err(format!(
                        "physical Source path is not canonical metadata: {child_relative:?}"
                    ));
                }
                stack.push((child.path(), child_relative));
            }
        }
    }
    Ok(rows)
}

/// One member's stat identity under the canonical mode rules of the hashing
/// capture: directories and symlinks carry no byte content, files and
/// symlink targets charge the shared content ceiling.
fn stat_row(
    path: &Path,
    physical: &std::fs::Metadata,
    aggregate_content_bytes: &mut u64,
) -> Result<StatRow, String> {
    let (kind, logical_byte_length, symlink_target) = if physical.is_dir() {
        #[cfg(unix)]
        require_canonical_mode(path, physical, CANONICAL_DIRECTORY_MODE)?;
        (StatRowKind::Directory, 0, None)
    } else if physical.is_file() {
        #[cfg(unix)]
        let executable = {
            use std::os::unix::fs::PermissionsExt;
            let mode = physical.permissions().mode() & 0o777;
            match mode {
                mode if mode == CANONICAL_FILE_MODE => false,
                mode if mode == CANONICAL_EXECUTABLE_FILE_MODE => true,
                _ => {
                    return Err(format!(
                        "physical Source file {} has noncanonical mode {mode:#o}",
                        path.display()
                    ));
                }
            }
        };
        #[cfg(not(unix))]
        let executable = false;
        charge_content(aggregate_content_bytes, physical.len())?;
        (StatRowKind::File { executable }, physical.len(), None)
    } else if physical.file_type().is_symlink() {
        let target = std::fs::read_link(path).map_err(|error| {
            format!(
                "could not read canonical Source symlink {}: {error}",
                path.display()
            )
        })?;
        let target = os_str_bytes(target.as_os_str())?;
        let target_length = u64::try_from(target.len())
            .map_err(|_| "canonical Source symlink target length exceeds u64".to_owned())?;
        charge_content(aggregate_content_bytes, target_length)?;
        (StatRowKind::Symlink, target_length, Some(target))
    } else {
        return Err(format!(
            "physical Source path {} has an unsupported filesystem kind",
            path.display()
        ));
    };
    Ok(StatRow {
        kind,
        logical_byte_length,
        mode: host_mode(physical),
        device: host_device(physical),
        inode: host_inode(physical),
        modified_unix_seconds: host_modified_seconds(physical),
        modified_unix_nanos: host_modified_nanos(physical),
        changed_unix_seconds: host_changed_seconds(physical),
        changed_unix_nanos: host_changed_nanos(physical),
        symlink_target,
    })
}

fn charge_content(total: &mut u64, amount: u64) -> Result<(), String> {
    *total = total
        .checked_add(amount)
        .filter(|total| *total <= AGGREGATE_CONTENT_BYTE_LIMIT)
        .ok_or_else(|| {
            format!(
                "canonical Source content exceeds its {AGGREGATE_CONTENT_BYTE_LIMIT}-byte ceiling"
            )
        })?;
    Ok(())
}

#[cfg(unix)]
fn require_canonical_mode(
    path: &Path,
    metadata: &std::fs::Metadata,
    expected: u32,
) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let mode = metadata.permissions().mode() & 0o777;
    if mode == expected {
        Ok(())
    } else {
        Err(format!(
            "physical Source directory {} has noncanonical mode {mode:#o}; expected {expected:#o}",
            path.display()
        ))
    }
}

#[cfg(unix)]
fn host_mode(metadata: &std::fs::Metadata) -> u32 {
    use std::os::unix::fs::MetadataExt;
    metadata.mode()
}

#[cfg(not(unix))]
fn host_mode(metadata: &std::fs::Metadata) -> u32 {
    let _ = metadata;
    0
}

#[cfg(unix)]
fn host_device(metadata: &std::fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    metadata.dev()
}

#[cfg(not(unix))]
fn host_device(metadata: &std::fs::Metadata) -> u64 {
    let _ = metadata;
    0
}

#[cfg(unix)]
fn host_inode(metadata: &std::fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    metadata.ino()
}

#[cfg(not(unix))]
fn host_inode(metadata: &std::fs::Metadata) -> u64 {
    let _ = metadata;
    0
}

fn host_modified_seconds(metadata: &std::fs::Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|instant| instant.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

fn host_modified_nanos(metadata: &std::fs::Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|instant| instant.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.subsec_nanos() as i64)
        .unwrap_or_default()
}

#[cfg(unix)]
fn host_changed_seconds(metadata: &std::fs::Metadata) -> i64 {
    use std::os::unix::fs::MetadataExt;
    metadata.ctime()
}

#[cfg(not(unix))]
fn host_changed_seconds(metadata: &std::fs::Metadata) -> i64 {
    metadata
        .created()
        .ok()
        .and_then(|instant| instant.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

#[cfg(unix)]
fn host_changed_nanos(metadata: &std::fs::Metadata) -> i64 {
    use std::os::unix::fs::MetadataExt;
    metadata.ctime_nsec()
}

#[cfg(not(unix))]
fn host_changed_nanos(metadata: &std::fs::Metadata) -> i64 {
    metadata
        .created()
        .ok()
        .and_then(|instant| instant.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.subsec_nanos() as i64)
        .unwrap_or_default()
}

/// The record key: the canonical root spelling joined to the complete
/// stat-identity fingerprint, so two roots whose members carry identical
/// metadata still own disjoint records.
fn stat_fingerprint(
    canonical_root: &Path,
    rows: &BTreeMap<Vec<u8>, StatRow>,
) -> Result<[u8; 32], String> {
    let mut digest = Sha256::new();
    digest.update(RECORD_KEY_DOMAIN);
    let root_bytes = os_str_bytes(canonical_root.as_os_str())?;
    push_framed(&mut digest, &root_bytes);
    digest.update(
        u64::try_from(rows.len())
            .map_err(|_| "canonical Source row count exceeds u64".to_owned())?
            .to_le_bytes(),
    );
    for (path, row) in rows {
        push_framed(&mut digest, path);
        match row.kind {
            StatRowKind::Directory => digest.update([0u8]),
            StatRowKind::File { executable } => {
                digest.update([1u8, u8::from(executable)]);
            }
            StatRowKind::Symlink => digest.update([2u8]),
        }
        for value in [
            row.device,
            row.inode,
            u64::from(row.mode),
            row.logical_byte_length,
        ] {
            digest.update(value.to_le_bytes());
        }
        for value in [
            row.modified_unix_seconds,
            row.modified_unix_nanos,
            row.changed_unix_seconds,
            row.changed_unix_nanos,
        ] {
            digest.update(value.to_le_bytes());
        }
        if let Some(target) = &row.symlink_target {
            push_framed(&mut digest, target);
        }
    }
    Ok(digest.finalize().into())
}

fn push_framed(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

fn record_name(canonical_root: &Path, fingerprint: &[u8; 32]) -> Result<String, String> {
    let mut digest = Sha256::new();
    digest.update(RECORD_KEY_DOMAIN);
    let root_bytes = os_str_bytes(canonical_root.as_os_str())?;
    push_framed(&mut digest, &root_bytes);
    digest.update(fingerprint);
    Ok(format!("{}.{RECORD_EXTENSION}", hex(&digest.finalize())))
}

/// Canonical record encoding: magic, framed root, embedded fingerprint,
/// row table (path, kind, kind payload, content digest), and the index's
/// source-content commitment — followed by a trailing digest of the whole
/// payload. The decoder re-derives every structural invariant through
/// `CanonicalFilesystemMetadataIndex::version_1`.
fn encode_record(
    canonical_root: &Path,
    fingerprint: &[u8; 32],
    index: &CanonicalFilesystemMetadataIndex,
) -> Result<Vec<u8>, String> {
    let mut payload = Vec::new();
    payload.extend_from_slice(RECORD_MAGIC);
    let root_bytes = os_str_bytes(canonical_root.as_os_str())?;
    push_bytes(&mut payload, &root_bytes);
    payload.extend_from_slice(fingerprint);
    let rows: Vec<CanonicalFilesystemMetadataRow> = index.rows().collect();
    push_u64(&mut payload, rows.len() as u64);
    for row in &rows {
        push_bytes(&mut payload, row.relative_path());
        match row.kind() {
            CanonicalFilesystemMetadataRowKind::Directory => payload.push(0),
            CanonicalFilesystemMetadataRowKind::File {
                executable,
                logical_byte_length,
            } => {
                payload.push(1);
                payload.push(u8::from(executable));
                push_u64(&mut payload, logical_byte_length);
            }
            CanonicalFilesystemMetadataRowKind::Symlink {
                target_spelling_logical_byte_length,
            } => {
                payload.push(2);
                push_u64(&mut payload, target_spelling_logical_byte_length);
            }
        }
    }
    payload.extend_from_slice(index.source_content_commitment());
    payload.extend_from_slice(&index.policy_version().to_le_bytes());
    let digest: [u8; 32] = Sha256::digest(&payload).into();
    payload.extend_from_slice(&digest);
    Ok(payload)
}

fn decode_record(
    bytes: &[u8],
    canonical_root: &Path,
    fingerprint: &[u8; 32],
) -> Result<CanonicalFilesystemMetadataIndex, String> {
    if bytes.len() < 32 {
        return Err("checked-source cache record is truncated".to_owned());
    }
    let (payload, stored_digest) = bytes.split_at(bytes.len() - 32);
    let computed: [u8; 32] = Sha256::digest(payload).into();
    if computed.as_slice() != stored_digest {
        return Err("checked-source cache record digest mismatch".to_owned());
    }
    let mut cursor = RecordCursor { bytes: payload };
    let magic = cursor.take(RECORD_MAGIC.len())?;
    if magic != RECORD_MAGIC {
        return Err("checked-source cache record magic mismatch".to_owned());
    }
    let stored_root = cursor.take_framed()?;
    let expected_root = os_str_bytes(canonical_root.as_os_str())?;
    if stored_root != expected_root.as_slice() {
        return Err("checked-source cache record names a different root".to_owned());
    }
    let stored_fingerprint: [u8; 32] = cursor
        .take(32)?
        .try_into()
        .map_err(|_| "checked-source cache record fingerprint is truncated".to_owned())?;
    if &stored_fingerprint != fingerprint {
        return Err("checked-source cache record fingerprint mismatch".to_owned());
    }
    let row_count = cursor.take_u64()? as usize;
    if row_count > CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT {
        return Err("checked-source cache record exceeds the row ceiling".to_owned());
    }
    let mut rows = Vec::with_capacity(row_count.min(1024));
    for _ in 0..row_count {
        let path = cursor.take_framed()?;
        if path.len() > FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT {
            return Err("checked-source cache record path exceeds the byte limit".to_owned());
        }
        let kind_tag = cursor.take(1)?[0];
        let kind = match kind_tag {
            0 => CanonicalFilesystemMetadataRowKind::Directory,
            1 => {
                let executable = cursor.take(1)?[0];
                if executable > 1 {
                    return Err("checked-source cache record file flag is noncanonical".to_owned());
                }
                CanonicalFilesystemMetadataRowKind::File {
                    executable: executable == 1,
                    logical_byte_length: cursor.take_u64()?,
                }
            }
            2 => CanonicalFilesystemMetadataRowKind::Symlink {
                target_spelling_logical_byte_length: cursor.take_u64()?,
            },
            _ => return Err("checked-source cache record kind tag is unknown".to_owned()),
        };
        rows.push(CanonicalFilesystemMetadataRow::new(path.to_vec(), kind));
    }
    let commitment: [u8; 32] = cursor
        .take(32)?
        .try_into()
        .map_err(|_| "checked-source cache record commitment is truncated".to_owned())?;
    let policy_version = u32::from_le_bytes(
        cursor
            .take(4)?
            .try_into()
            .map_err(|_| "checked-source cache record policy version is truncated".to_owned())?,
    );
    if !cursor.is_empty() {
        return Err("checked-source cache record has trailing bytes".to_owned());
    }
    CanonicalFilesystemMetadataIndex::new(policy_version, commitment, rows)
        .map_err(|error| format!("checked-source cache record rows are not canonical: {error}"))
}

struct RecordCursor<'a> {
    bytes: &'a [u8],
}

impl<'a> RecordCursor<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], String> {
        if self.bytes.len() < count {
            return Err("checked-source cache record is truncated".to_owned());
        }
        let (taken, rest) = self.bytes.split_at(count);
        self.bytes = rest;
        Ok(taken)
    }

    fn take_framed(&mut self) -> Result<&'a [u8], String> {
        let length = self.take_u64()?;
        let length = usize::try_from(length)
            .map_err(|_| "checked-source cache record field length exceeds usize".to_owned())?;
        self.take(length)
    }

    fn take_u64(&mut self) -> Result<u64, String> {
        let field = self.take(8)?;
        Ok(u64::from_le_bytes(field.try_into().map_err(|_| {
            "checked-source cache record u64 is truncated".to_owned()
        })?))
    }

    fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

fn push_u64(buffer: &mut Vec<u8>, value: u64) {
    buffer.extend_from_slice(&value.to_le_bytes());
}

fn push_bytes(buffer: &mut Vec<u8>, bytes: &[u8]) {
    push_u64(buffer, bytes.len() as u64);
    buffer.extend_from_slice(bytes);
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(DIGITS[(byte >> 4) as usize] as char);
        text.push(DIGITS[(byte & 0xf) as usize] as char);
    }
    text
}

#[cfg(unix)]
fn os_str_bytes(value: &std::ffi::OsStr) -> Result<Vec<u8>, String> {
    use std::os::unix::ffi::OsStrExt;
    Ok(value.as_bytes().to_vec())
}

#[cfg(not(unix))]
fn os_str_bytes(value: &std::ffi::OsStr) -> Result<Vec<u8>, String> {
    value
        .to_str()
        .map(|value| value.as_bytes().to_vec())
        .ok_or_else(|| "physical Source path is not portable UTF-8".to_owned())
}

#[cfg(test)]
#[path = "checked_source_cache/tests.rs"]
mod tests;
