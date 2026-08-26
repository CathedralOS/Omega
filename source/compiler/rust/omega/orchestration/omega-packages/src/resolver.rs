use crate::json::{JsonParseError, JsonParser, JsonValue};
use crate::source::{
    GitSourceRequest, LocalSourceLimits, SourceResolveError, resolve_git_source,
    resolve_local_source_snapshot,
};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub const SOURCE_CACHE_POLICY_SCHEMA_VERSION: u32 = 3;
const DEFAULT_SOURCE_CACHE_POLICY_RECORD_MAXIMUM_BYTES: usize = 1024 * 1024;
const MAXIMUM_RECORD_STAGE_ATTEMPTS: u64 = 256;
static NEXT_RECORD_STAGE_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceCacheRequest {
    LocalPath(PathBuf),
    Git(GitSourceRequest),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SourceCacheVerdict {
    DiagnosticObserved,
    Rejected,
}

impl SourceCacheVerdict {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DiagnosticObserved => "diagnostic-observed",
            Self::Rejected => "rejected",
        }
    }
}

/// Legacy machine-readable source diagnostics.
///
/// This value records what the exploratory resolver observed. It is not a
/// `SourceResolutionReceipt`, cannot authorize compilation or lock mutation,
/// and deliberately has no accepted verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceCachePolicyRecord {
    pub schema_version: u32,
    pub verdict: SourceCacheVerdict,
    pub source_kind: String,
    pub locator: String,
    pub transport_profile: Option<String>,
    pub requested_rev: Option<String>,
    pub resolved_commit: Option<String>,
    pub resolved_tree: Option<String>,
    pub content_identity: Option<String>,
    pub cache_path: Option<String>,
    pub file_count: Option<usize>,
    pub byte_count: Option<u64>,
    pub max_files: usize,
    pub max_bytes: u64,
    pub max_depth: usize,
    pub submodule_policy: String,
    pub path_policy: String,
    pub rejection: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceCachePolicyRecordParseError {
    InvalidJson { message: String },
    MissingField { field: String },
    UnexpectedField { field: String },
    InvalidField { field: String, message: String },
    UnsupportedSchemaVersion { found: u32, supported: u32 },
}

/// Resource ceiling for diagnostic source-cache record persistence.
///
/// This bounds an internal diagnostic format. It grants no source, package, or
/// lock authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceCachePolicyRecordPersistenceLimits {
    maximum_bytes: usize,
}

impl SourceCachePolicyRecordPersistenceLimits {
    pub const fn new(maximum_bytes: usize) -> Self {
        Self { maximum_bytes }
    }

    pub const fn maximum_bytes(self) -> usize {
        self.maximum_bytes
    }
}

impl Default for SourceCachePolicyRecordPersistenceLimits {
    fn default() -> Self {
        Self::new(DEFAULT_SOURCE_CACHE_POLICY_RECORD_MAXIMUM_BYTES)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceCachePolicyRecordPersistenceError {
    Io { path: PathBuf, message: String },
    Parse(SourceCachePolicyRecordParseError),
    InvalidDestination { path: PathBuf },
    NotRegularFile { path: PathBuf },
    DestinationExists { path: PathBuf },
    ParentDirectoryChanged { path: PathBuf },
    ByteLimitExceeded { actual: u64, maximum: usize },
    LengthOverflow,
    AllocationFailed,
    NonCanonicalEncoding,
    StageNameSpaceExhausted { directory: PathBuf },
}

impl SourceCachePolicyRecord {
    pub fn fingerprint(&self) -> String {
        format_sha256(&Sha256::digest(self.to_json().as_bytes()))
    }

    pub fn from_json(json: &str) -> Result<Self, SourceCachePolicyRecordParseError> {
        let value = JsonParser::new(json)
            .parse()
            .map_err(source_cache_policy_json_error)?;
        let fields = value_object(&value, "$")?;
        ensure_fields(
            fields,
            "$",
            &[
                "schema_version",
                "verdict",
                "source_kind",
                "locator",
                "transport_profile",
                "requested_rev",
                "resolved_commit",
                "resolved_tree",
                "content_identity",
                "cache_path",
                "file_count",
                "byte_count",
                "max_files",
                "max_bytes",
                "max_depth",
                "submodule_policy",
                "path_policy",
                "rejection",
            ],
        )?;
        let schema_version = value_u32(field(fields, "schema_version", "$")?, "$.schema_version")?;
        if schema_version != SOURCE_CACHE_POLICY_SCHEMA_VERSION {
            return Err(
                SourceCachePolicyRecordParseError::UnsupportedSchemaVersion {
                    found: schema_version,
                    supported: SOURCE_CACHE_POLICY_SCHEMA_VERSION,
                },
            );
        }
        let verdict = parse_verdict(value_string(field(fields, "verdict", "$")?, "$.verdict")?)?;
        Ok(Self {
            schema_version,
            verdict,
            source_kind: value_string(field(fields, "source_kind", "$")?, "$.source_kind")?
                .to_owned(),
            locator: value_string(field(fields, "locator", "$")?, "$.locator")?.to_owned(),
            transport_profile: optional_string(field(fields, "transport_profile", "$")?)?,
            requested_rev: optional_string(field(fields, "requested_rev", "$")?)?,
            resolved_commit: optional_string(field(fields, "resolved_commit", "$")?)?,
            resolved_tree: optional_string(field(fields, "resolved_tree", "$")?)?,
            content_identity: optional_string(field(fields, "content_identity", "$")?)?,
            cache_path: optional_string(field(fields, "cache_path", "$")?)?,
            file_count: optional_usize(field(fields, "file_count", "$")?, "$.file_count")?,
            byte_count: optional_u64(field(fields, "byte_count", "$")?, "$.byte_count")?,
            max_files: value_usize(field(fields, "max_files", "$")?, "$.max_files")?,
            max_bytes: value_u64(field(fields, "max_bytes", "$")?, "$.max_bytes")?,
            max_depth: value_usize(field(fields, "max_depth", "$")?, "$.max_depth")?,
            submodule_policy: value_string(
                field(fields, "submodule_policy", "$")?,
                "$.submodule_policy",
            )?
            .to_owned(),
            path_policy: value_string(field(fields, "path_policy", "$")?, "$.path_policy")?
                .to_owned(),
            rejection: optional_string(field(fields, "rejection", "$")?)?,
        })
    }

    pub fn read_from_path(
        path: impl AsRef<Path>,
    ) -> Result<Self, SourceCachePolicyRecordPersistenceError> {
        Self::read_from_path_with_limits(path, SourceCachePolicyRecordPersistenceLimits::default())
    }

    pub fn read_from_path_with_limits(
        path: impl AsRef<Path>,
        limits: SourceCachePolicyRecordPersistenceLimits,
    ) -> Result<Self, SourceCachePolicyRecordPersistenceError> {
        let destination = DiagnosticRecordDestination::resolve(path.as_ref())?;
        let file = open_record_without_following(&destination.path)?;
        verify_regular_path_identity(&destination.path, &file)?;
        let metadata = file
            .metadata()
            .map_err(|error| persistence_io_error(&destination.path, error))?;
        if metadata.len() > u64::try_from(limits.maximum_bytes()).unwrap_or(u64::MAX) {
            return Err(SourceCachePolicyRecordPersistenceError::ByteLimitExceeded {
                actual: metadata.len(),
                maximum: limits.maximum_bytes(),
            });
        }
        let contents = read_record_bytes_bounded(file, &destination.path, limits)?;
        destination.verify_parent()?;
        let text = std::str::from_utf8(&contents).map_err(|_| {
            SourceCachePolicyRecordPersistenceError::Parse(
                SourceCachePolicyRecordParseError::InvalidJson {
                    message: "source-cache policy record is not UTF-8".to_owned(),
                },
            )
        })?;
        let record =
            Self::from_json(text).map_err(SourceCachePolicyRecordPersistenceError::Parse)?;
        let canonical = record.canonical_json_with_limits(limits)?;
        if canonical.as_bytes() != contents {
            return Err(SourceCachePolicyRecordPersistenceError::NonCanonicalEncoding);
        }
        Ok(record)
    }

    pub fn write_to_path(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<(), SourceCachePolicyRecordPersistenceError> {
        self.write_to_path_with_limits(path, SourceCachePolicyRecordPersistenceLimits::default())
    }

    pub fn write_to_path_with_limits(
        &self,
        path: impl AsRef<Path>,
        limits: SourceCachePolicyRecordPersistenceLimits,
    ) -> Result<(), SourceCachePolicyRecordPersistenceError> {
        let canonical = self.canonical_json_with_limits(limits)?;
        let recovered =
            Self::from_json(&canonical).map_err(SourceCachePolicyRecordPersistenceError::Parse)?;
        if recovered != *self {
            return Err(SourceCachePolicyRecordPersistenceError::NonCanonicalEncoding);
        }

        let destination = DiagnosticRecordDestination::resolve(path.as_ref())?;
        let mut stage = create_exclusive_record_stage(&destination.directory)?;
        stage
            .file
            .write_all(canonical.as_bytes())
            .map_err(|error| persistence_io_error(&stage.path, error))?;
        stage
            .file
            .sync_all()
            .map_err(|error| persistence_io_error(&stage.path, error))?;
        stage
            .file
            .seek(SeekFrom::Start(0))
            .map_err(|error| persistence_io_error(&stage.path, error))?;
        let staged_bytes = read_record_bytes_bounded(&mut stage.file, &stage.path, limits)?;
        if staged_bytes != canonical.as_bytes() {
            return Err(SourceCachePolicyRecordPersistenceError::NonCanonicalEncoding);
        }

        destination.verify_parent()?;
        match fs::hard_link(&stage.path, &destination.path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(SourceCachePolicyRecordPersistenceError::DestinationExists {
                    path: destination.path,
                });
            }
            Err(error) => return Err(persistence_io_error(&destination.path, error)),
        }
        verify_regular_path_identity(&destination.path, &stage.file)?;
        stage.remove()?;
        destination.verify_parent()?;
        sync_record_parent(&destination.parent, &destination.directory)?;
        destination.verify_parent()?;
        Ok(())
    }

    fn canonical_json_with_limits(
        &self,
        limits: SourceCachePolicyRecordPersistenceLimits,
    ) -> Result<String, SourceCachePolicyRecordPersistenceError> {
        let raw_payload_bytes = self
            .raw_string_payload_bytes()
            .ok_or(SourceCachePolicyRecordPersistenceError::LengthOverflow)?;
        if raw_payload_bytes > limits.maximum_bytes() {
            return Err(SourceCachePolicyRecordPersistenceError::ByteLimitExceeded {
                actual: u64::try_from(raw_payload_bytes).unwrap_or(u64::MAX),
                maximum: limits.maximum_bytes(),
            });
        }
        let mut counter = JsonLengthCounter::default();
        self.render_json(&mut counter);
        let encoded_length = counter
            .length()
            .ok_or(SourceCachePolicyRecordPersistenceError::LengthOverflow)?;
        if encoded_length > limits.maximum_bytes() {
            return Err(SourceCachePolicyRecordPersistenceError::ByteLimitExceeded {
                actual: u64::try_from(encoded_length).unwrap_or(u64::MAX),
                maximum: limits.maximum_bytes(),
            });
        }
        let mut json = String::new();
        json.try_reserve_exact(encoded_length)
            .map_err(|_| SourceCachePolicyRecordPersistenceError::AllocationFailed)?;
        self.render_json(&mut json);
        debug_assert_eq!(json.len(), encoded_length);
        Ok(json)
    }

    fn raw_string_payload_bytes(&self) -> Option<usize> {
        let mut total = 0_usize;
        for value in [
            Some(self.source_kind.as_str()),
            Some(self.locator.as_str()),
            self.transport_profile.as_deref(),
            self.requested_rev.as_deref(),
            self.resolved_commit.as_deref(),
            self.resolved_tree.as_deref(),
            self.content_identity.as_deref(),
            self.cache_path.as_deref(),
            Some(self.submodule_policy.as_str()),
            Some(self.path_policy.as_str()),
            self.rejection.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            total = total.checked_add(value.len())?;
        }
        Some(total)
    }

    pub fn to_json(&self) -> String {
        let mut json = String::new();
        self.render_json(&mut json);
        json
    }

    fn render_json(&self, json: &mut impl JsonOutput) {
        json.push_str("{\n");
        push_number_field(json, 1, "schema_version", self.schema_version, true);
        push_string_field(json, 1, "verdict", self.verdict.as_str(), true);
        push_string_field(json, 1, "source_kind", &self.source_kind, true);
        push_string_field(json, 1, "locator", &self.locator, true);
        push_optional_string_field(
            json,
            1,
            "transport_profile",
            self.transport_profile.as_deref(),
            true,
        );
        push_optional_string_field(
            json,
            1,
            "requested_rev",
            self.requested_rev.as_deref(),
            true,
        );
        push_optional_string_field(
            json,
            1,
            "resolved_commit",
            self.resolved_commit.as_deref(),
            true,
        );
        push_optional_string_field(
            json,
            1,
            "resolved_tree",
            self.resolved_tree.as_deref(),
            true,
        );
        push_optional_string_field(
            json,
            1,
            "content_identity",
            self.content_identity.as_deref(),
            true,
        );
        push_optional_string_field(json, 1, "cache_path", self.cache_path.as_deref(), true);
        push_optional_usize_field(json, 1, "file_count", self.file_count, true);
        push_optional_u64_field(json, 1, "byte_count", self.byte_count, true);
        push_usize_field(json, 1, "max_files", self.max_files, true);
        push_u64_field(json, 1, "max_bytes", self.max_bytes, true);
        push_usize_field(json, 1, "max_depth", self.max_depth, true);
        push_string_field(json, 1, "submodule_policy", &self.submodule_policy, true);
        push_string_field(json, 1, "path_policy", &self.path_policy, true);
        push_optional_string_field(json, 1, "rejection", self.rejection.as_deref(), false);
        json.push_str("}\n");
    }
}

trait JsonOutput {
    fn push_char(&mut self, value: char);
    fn push_str(&mut self, value: &str);
}

impl JsonOutput for String {
    fn push_char(&mut self, value: char) {
        self.push(value);
    }

    fn push_str(&mut self, value: &str) {
        self.push_str(value);
    }
}

#[derive(Default)]
struct JsonLengthCounter {
    length: usize,
    overflowed: bool,
}

impl JsonLengthCounter {
    fn length(self) -> Option<usize> {
        (!self.overflowed).then_some(self.length)
    }
}

impl JsonOutput for JsonLengthCounter {
    fn push_char(&mut self, value: char) {
        if !self.overflowed {
            if let Some(length) = self.length.checked_add(value.len_utf8()) {
                self.length = length;
            } else {
                self.overflowed = true;
            }
        }
    }

    fn push_str(&mut self, value: &str) {
        if !self.overflowed {
            if let Some(length) = self.length.checked_add(value.len()) {
                self.length = length;
            } else {
                self.overflowed = true;
            }
        }
    }
}

struct DiagnosticRecordDestination {
    directory: PathBuf,
    path: PathBuf,
    parent: File,
}

impl DiagnosticRecordDestination {
    fn resolve(path: &Path) -> Result<Self, SourceCachePolicyRecordPersistenceError> {
        let Some(file_name) = path.file_name() else {
            return Err(
                SourceCachePolicyRecordPersistenceError::InvalidDestination {
                    path: path.to_path_buf(),
                },
            );
        };
        let requested_parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let directory = fs::canonicalize(requested_parent)
            .map_err(|error| persistence_io_error(requested_parent, error))?;
        let parent = open_record_parent(&directory)?;
        let destination = Self {
            path: directory.join(file_name),
            directory,
            parent,
        };
        destination.verify_parent()?;
        Ok(destination)
    }

    fn verify_parent(&self) -> Result<(), SourceCachePolicyRecordPersistenceError> {
        let path_metadata = fs::symlink_metadata(&self.directory)
            .map_err(|error| persistence_io_error(&self.directory, error))?;
        let handle_metadata = self
            .parent
            .metadata()
            .map_err(|error| persistence_io_error(&self.directory, error))?;
        if path_metadata.file_type().is_symlink()
            || !path_metadata.is_dir()
            || !handle_metadata.is_dir()
            || !same_record_file_identity(&path_metadata, &handle_metadata)
        {
            return Err(
                SourceCachePolicyRecordPersistenceError::ParentDirectoryChanged {
                    path: self.directory.clone(),
                },
            );
        }
        Ok(())
    }
}

struct PendingDiagnosticRecord {
    path: PathBuf,
    file: File,
    removed: bool,
}

impl PendingDiagnosticRecord {
    fn remove(&mut self) -> Result<(), SourceCachePolicyRecordPersistenceError> {
        fs::remove_file(&self.path).map_err(|error| persistence_io_error(&self.path, error))?;
        self.removed = true;
        Ok(())
    }
}

impl Drop for PendingDiagnosticRecord {
    fn drop(&mut self) {
        if !self.removed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn create_exclusive_record_stage(
    directory: &Path,
) -> Result<PendingDiagnosticRecord, SourceCachePolicyRecordPersistenceError> {
    for _ in 0..MAXIMUM_RECORD_STAGE_ATTEMPTS {
        let stage_id = NEXT_RECORD_STAGE_ID.fetch_add(1, Ordering::Relaxed);
        let path = directory.join(format!(
            ".omega-source-cache-record-stage-{}-{stage_id}",
            std::process::id()
        ));
        let mut options = OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(file) => {
                return Ok(PendingDiagnosticRecord {
                    path,
                    file,
                    removed: false,
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(persistence_io_error(&path, error)),
        }
    }
    Err(
        SourceCachePolicyRecordPersistenceError::StageNameSpaceExhausted {
            directory: directory.to_path_buf(),
        },
    )
}

fn open_record_without_following(
    path: &Path,
) -> Result<File, SourceCachePolicyRecordPersistenceError> {
    if let Ok(metadata) = fs::symlink_metadata(path)
        && (metadata.file_type().is_symlink() || !metadata.is_file())
    {
        return Err(SourceCachePolicyRecordPersistenceError::NotRegularFile {
            path: path.to_path_buf(),
        });
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(nix::libc::O_NOFOLLOW);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    options
        .open(path)
        .map_err(|error| persistence_io_error(path, error))
}

fn open_record_parent(path: &Path) -> Result<File, SourceCachePolicyRecordPersistenceError> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT);
    }
    options
        .open(path)
        .map_err(|error| persistence_io_error(path, error))
}

#[cfg(unix)]
fn sync_record_parent(
    parent: &File,
    path: &Path,
) -> Result<(), SourceCachePolicyRecordPersistenceError> {
    parent
        .sync_all()
        .map_err(|error| persistence_io_error(path, error))
}

#[cfg(not(unix))]
fn sync_record_parent(
    _parent: &File,
    _path: &Path,
) -> Result<(), SourceCachePolicyRecordPersistenceError> {
    Ok(())
}

fn verify_regular_path_identity(
    path: &Path,
    file: &File,
) -> Result<(), SourceCachePolicyRecordPersistenceError> {
    let path_metadata =
        fs::symlink_metadata(path).map_err(|error| persistence_io_error(path, error))?;
    let handle_metadata = file
        .metadata()
        .map_err(|error| persistence_io_error(path, error))?;
    if path_metadata.file_type().is_symlink()
        || !path_metadata.is_file()
        || !handle_metadata.is_file()
        || !same_record_file_identity(&path_metadata, &handle_metadata)
    {
        return Err(SourceCachePolicyRecordPersistenceError::NotRegularFile {
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

#[cfg(unix)]
fn same_record_file_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;

    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(windows)]
fn same_record_file_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    matches!(
        (
            left.volume_serial_number(),
            left.file_index(),
            right.volume_serial_number(),
            right.file_index(),
        ),
        (Some(left_volume), Some(left_index), Some(right_volume), Some(right_index))
            if left_volume == right_volume && left_index == right_index
    )
}

#[cfg(not(any(unix, windows)))]
fn same_record_file_identity(_left: &fs::Metadata, _right: &fs::Metadata) -> bool {
    false
}

fn read_record_bytes_bounded(
    mut reader: impl Read,
    path: &Path,
    limits: SourceCachePolicyRecordPersistenceLimits,
) -> Result<Vec<u8>, SourceCachePolicyRecordPersistenceError> {
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        let count = reader
            .read(&mut chunk)
            .map_err(|error| persistence_io_error(path, error))?;
        if count == 0 {
            break;
        }
        let next_length = bytes
            .len()
            .checked_add(count)
            .ok_or(SourceCachePolicyRecordPersistenceError::LengthOverflow)?;
        if next_length > limits.maximum_bytes() {
            return Err(SourceCachePolicyRecordPersistenceError::ByteLimitExceeded {
                actual: u64::try_from(next_length).unwrap_or(u64::MAX),
                maximum: limits.maximum_bytes(),
            });
        }
        bytes
            .try_reserve(count)
            .map_err(|_| SourceCachePolicyRecordPersistenceError::AllocationFailed)?;
        bytes.extend_from_slice(&chunk[..count]);
    }
    Ok(bytes)
}

fn persistence_io_error(
    path: &Path,
    error: std::io::Error,
) -> SourceCachePolicyRecordPersistenceError {
    SourceCachePolicyRecordPersistenceError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    }
}

fn source_cache_policy_json_error(error: JsonParseError) -> SourceCachePolicyRecordParseError {
    match error {
        JsonParseError::InvalidJson { message } => {
            SourceCachePolicyRecordParseError::InvalidJson { message }
        }
    }
}

fn parse_verdict(value: &str) -> Result<SourceCacheVerdict, SourceCachePolicyRecordParseError> {
    match value {
        "diagnostic-observed" => Ok(SourceCacheVerdict::DiagnosticObserved),
        "rejected" => Ok(SourceCacheVerdict::Rejected),
        _ => Err(SourceCachePolicyRecordParseError::InvalidField {
            field: "$.verdict".to_owned(),
            message: format!("unsupported source-cache verdict `{value}`"),
        }),
    }
}

fn ensure_fields(
    fields: &[(String, JsonValue)],
    path: &str,
    expected: &[&str],
) -> Result<(), SourceCachePolicyRecordParseError> {
    for expected_field in expected {
        if !fields.iter().any(|(name, _)| name == expected_field) {
            return Err(SourceCachePolicyRecordParseError::MissingField {
                field: format!("{path}.{expected_field}"),
            });
        }
    }
    for (name, _) in fields {
        if !expected.iter().any(|expected| expected == name) {
            return Err(SourceCachePolicyRecordParseError::UnexpectedField {
                field: format!("{path}.{name}"),
            });
        }
    }
    Ok(())
}

fn field<'a>(
    fields: &'a [(String, JsonValue)],
    name: &str,
    path: &str,
) -> Result<&'a JsonValue, SourceCachePolicyRecordParseError> {
    fields
        .iter()
        .find(|(field, _)| field == name)
        .map(|(_, value)| value)
        .ok_or_else(|| SourceCachePolicyRecordParseError::MissingField {
            field: format!("{path}.{name}"),
        })
}

fn value_object<'a>(
    value: &'a JsonValue,
    path: &str,
) -> Result<&'a [(String, JsonValue)], SourceCachePolicyRecordParseError> {
    value
        .as_object()
        .ok_or_else(|| SourceCachePolicyRecordParseError::InvalidField {
            field: path.to_owned(),
            message: "expected object".to_owned(),
        })
}

fn value_string<'a>(
    value: &'a JsonValue,
    path: &str,
) -> Result<&'a str, SourceCachePolicyRecordParseError> {
    match value {
        JsonValue::String(value) => Ok(value),
        _ => Err(SourceCachePolicyRecordParseError::InvalidField {
            field: path.to_owned(),
            message: "expected string".to_owned(),
        }),
    }
}

fn optional_string(value: &JsonValue) -> Result<Option<String>, SourceCachePolicyRecordParseError> {
    match value {
        JsonValue::Null => Ok(None),
        JsonValue::String(value) => Ok(Some(value.clone())),
        _ => Err(SourceCachePolicyRecordParseError::InvalidField {
            field: "optional string".to_owned(),
            message: "expected string or null".to_owned(),
        }),
    }
}

fn value_u64(value: &JsonValue, path: &str) -> Result<u64, SourceCachePolicyRecordParseError> {
    match value {
        JsonValue::Number(value) => Ok(*value),
        _ => Err(SourceCachePolicyRecordParseError::InvalidField {
            field: path.to_owned(),
            message: "expected integer".to_owned(),
        }),
    }
}

fn value_u32(value: &JsonValue, path: &str) -> Result<u32, SourceCachePolicyRecordParseError> {
    let value = value_u64(value, path)?;
    u32::try_from(value).map_err(|_| SourceCachePolicyRecordParseError::InvalidField {
        field: path.to_owned(),
        message: "integer does not fit in u32".to_owned(),
    })
}

fn value_usize(value: &JsonValue, path: &str) -> Result<usize, SourceCachePolicyRecordParseError> {
    let value = value_u64(value, path)?;
    usize::try_from(value).map_err(|_| SourceCachePolicyRecordParseError::InvalidField {
        field: path.to_owned(),
        message: "integer does not fit in usize".to_owned(),
    })
}

fn optional_u64(
    value: &JsonValue,
    path: &str,
) -> Result<Option<u64>, SourceCachePolicyRecordParseError> {
    match value {
        JsonValue::Null => Ok(None),
        _ => value_u64(value, path).map(Some),
    }
}

fn optional_usize(
    value: &JsonValue,
    path: &str,
) -> Result<Option<usize>, SourceCachePolicyRecordParseError> {
    match value {
        JsonValue::Null => Ok(None),
        _ => value_usize(value, path).map(Some),
    }
}

pub fn resolve_source_cache_record(
    request: SourceCacheRequest,
    cache_dir: impl AsRef<Path>,
    limits: LocalSourceLimits,
) -> SourceCachePolicyRecord {
    let limits = limits.compiler_bounded();
    let cache_dir = cache_dir.as_ref();
    match request {
        SourceCacheRequest::LocalPath(path) => match resolve_local_source_snapshot(
            &path,
            cache_dir,
            limits,
        ) {
            Ok(resolved) => SourceCachePolicyRecord {
                schema_version: SOURCE_CACHE_POLICY_SCHEMA_VERSION,
                verdict: SourceCacheVerdict::DiagnosticObserved,
                source_kind: "local-path".to_owned(),
                locator: path.display().to_string(),
                transport_profile: None,
                requested_rev: None,
                resolved_commit: None,
                resolved_tree: None,
                content_identity: Some(resolved.normalized.content_identity),
                cache_path: Some(resolved.snapshot_root.display().to_string()),
                file_count: Some(resolved.normalized.file_count),
                byte_count: Some(resolved.normalized.byte_count),
                max_files: limits.max_files,
                max_bytes: limits.max_bytes,
                max_depth: limits.max_depth,
                submodule_policy: "git-submodules-not-applicable".to_owned(),
                path_policy:
                    "validated-local-snapshot; canonical-root-contained; symlink-escapes-rejected; dot-git-excluded; root-build-output-excluded"
                        .to_owned(),
                rejection: None,
            },
            Err(error) => rejected_record(
                "local-path",
                path.display().to_string(),
                None,
                None,
                limits,
                error,
            ),
        },
        SourceCacheRequest::Git(request) => {
            let locator = request.locator_identity().to_owned();
            let transport_profile = request.transport_profile().as_str().to_owned();
            let requested_rev = request.requested_revision().to_owned();
            match resolve_git_source(&request, cache_dir, limits) {
                Ok(resolved) => SourceCachePolicyRecord {
                    schema_version: SOURCE_CACHE_POLICY_SCHEMA_VERSION,
                    verdict: SourceCacheVerdict::DiagnosticObserved,
                    source_kind: "git".to_owned(),
                    locator,
                    transport_profile: Some(resolved.transport_profile.as_str().to_owned()),
                    requested_rev: Some(resolved.requested_rev),
                    resolved_commit: Some(resolved.commit),
                    resolved_tree: Some(resolved.tree),
                    content_identity: Some(resolved.local.content_identity),
                    cache_path: Some(resolved.snapshot_root.display().to_string()),
                    file_count: Some(resolved.local.file_count),
                    byte_count: Some(resolved.local.byte_count),
                    max_files: limits.max_files,
                    max_bytes: limits.max_bytes,
                    max_depth: limits.max_depth,
                    submodule_policy: "gitmodules-rejected-until-submodules-are-explicit-package-edges"
                        .to_owned(),
                    path_policy:
                        "validated-object-snapshot; canonical-root-contained; symlink-escapes-rejected; dot-git-excluded"
                            .to_owned(),
                    rejection: None,
                },
                Err(error) => rejected_record(
                    "git",
                    locator,
                    Some(transport_profile),
                    Some(requested_rev),
                    limits,
                    error,
                ),
            }
        }
    }
}

fn rejected_record(
    source_kind: &str,
    locator: String,
    transport_profile: Option<String>,
    requested_rev: Option<String>,
    limits: LocalSourceLimits,
    error: SourceResolveError,
) -> SourceCachePolicyRecord {
    SourceCachePolicyRecord {
        schema_version: SOURCE_CACHE_POLICY_SCHEMA_VERSION,
        verdict: SourceCacheVerdict::Rejected,
        source_kind: source_kind.to_owned(),
        locator,
        transport_profile,
        requested_rev,
        resolved_commit: None,
        resolved_tree: None,
        content_identity: None,
        cache_path: None,
        file_count: None,
        byte_count: None,
        max_files: limits.max_files,
        max_bytes: limits.max_bytes,
        max_depth: limits.max_depth,
        submodule_policy: if source_kind == "git" {
            "gitmodules-rejected-until-submodules-are-explicit-package-edges".to_owned()
        } else {
            "git-submodules-not-applicable".to_owned()
        },
        path_policy: if source_kind == "local-path" {
            "canonical-root-contained; symlink-escapes-rejected; dot-git-excluded; root-build-output-excluded"
                .to_owned()
        } else {
            "canonical-root-contained; symlink-escapes-rejected; dot-git-excluded".to_owned()
        },
        rejection: Some(error.to_string()),
    }
}

fn push_number_field(
    json: &mut impl JsonOutput,
    indent: usize,
    name: &str,
    value: u32,
    comma: bool,
) {
    push_indent(json, indent);
    push_json_string(json, name);
    json.push_str(": ");
    json.push_str(&value.to_string());
    if comma {
        json.push_char(',');
    }
    json.push_char('\n');
}

fn push_u64_field(json: &mut impl JsonOutput, indent: usize, name: &str, value: u64, comma: bool) {
    push_indent(json, indent);
    push_json_string(json, name);
    json.push_str(": ");
    json.push_str(&value.to_string());
    if comma {
        json.push_char(',');
    }
    json.push_char('\n');
}

fn push_usize_field(
    json: &mut impl JsonOutput,
    indent: usize,
    name: &str,
    value: usize,
    comma: bool,
) {
    push_indent(json, indent);
    push_json_string(json, name);
    json.push_str(": ");
    json.push_str(&value.to_string());
    if comma {
        json.push_char(',');
    }
    json.push_char('\n');
}

fn push_string_field(
    json: &mut impl JsonOutput,
    indent: usize,
    name: &str,
    value: &str,
    comma: bool,
) {
    push_indent(json, indent);
    push_json_string(json, name);
    json.push_str(": ");
    push_json_string(json, value);
    if comma {
        json.push_char(',');
    }
    json.push_char('\n');
}

fn push_optional_string_field(
    json: &mut impl JsonOutput,
    indent: usize,
    name: &str,
    value: Option<&str>,
    comma: bool,
) {
    push_indent(json, indent);
    push_json_string(json, name);
    json.push_str(": ");
    if let Some(value) = value {
        push_json_string(json, value);
    } else {
        json.push_str("null");
    }
    if comma {
        json.push_char(',');
    }
    json.push_char('\n');
}

fn push_optional_usize_field(
    json: &mut impl JsonOutput,
    indent: usize,
    name: &str,
    value: Option<usize>,
    comma: bool,
) {
    push_indent(json, indent);
    push_json_string(json, name);
    json.push_str(": ");
    if let Some(value) = value {
        json.push_str(&value.to_string());
    } else {
        json.push_str("null");
    }
    if comma {
        json.push_char(',');
    }
    json.push_char('\n');
}

fn push_optional_u64_field(
    json: &mut impl JsonOutput,
    indent: usize,
    name: &str,
    value: Option<u64>,
    comma: bool,
) {
    push_indent(json, indent);
    push_json_string(json, name);
    json.push_str(": ");
    if let Some(value) = value {
        json.push_str(&value.to_string());
    } else {
        json.push_str("null");
    }
    if comma {
        json.push_char(',');
    }
    json.push_char('\n');
}

fn push_json_string(json: &mut impl JsonOutput, value: &str) {
    json.push_char('"');
    for ch in value.chars() {
        match ch {
            '"' => json.push_str("\\\""),
            '\\' => json.push_str("\\\\"),
            '\n' => json.push_str("\\n"),
            '\r' => json.push_str("\\r"),
            '\t' => json.push_str("\\t"),
            ch if ch.is_control() => json.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => json.push_char(ch),
        }
    }
    json.push_char('"');
}

fn push_indent(json: &mut impl JsonOutput, level: usize) {
    for _ in 0..level {
        json.push_str("  ");
    }
}

fn format_sha256(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(64);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "omega-source-cache-record-{name}-{}-{stamp}",
            std::process::id()
        ))
    }

    fn run_test_git<I, S>(directory: &Path, args: I)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = Command::new("git")
            .current_dir(directory)
            .args(args)
            .output()
            .expect("spawn git");
        assert!(
            output.status.success(),
            "git command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn rejected_test_record() -> SourceCachePolicyRecord {
        SourceCachePolicyRecord {
            schema_version: SOURCE_CACHE_POLICY_SCHEMA_VERSION,
            verdict: SourceCacheVerdict::Rejected,
            source_kind: "local-path".to_owned(),
            locator: "./missing-package".to_owned(),
            transport_profile: None,
            requested_rev: None,
            resolved_commit: None,
            resolved_tree: None,
            content_identity: None,
            cache_path: None,
            file_count: None,
            byte_count: None,
            max_files: 4096,
            max_bytes: 268435456,
            max_depth: 64,
            submodule_policy: "git-submodules-not-applicable".to_owned(),
            path_policy: "canonical-root-contained; symlink-escapes-rejected; dot-git-excluded"
                .to_owned(),
            rejection: Some("missing source".to_owned()),
        }
    }

    #[test]
    fn local_source_cache_record_captures_limits_and_identity() {
        let root = temp_root("local");
        let cache = temp_root("cache");
        std::fs::create_dir_all(&root).expect("create source");
        std::fs::write(root.join("main.omg"), "machine Main::main() {}\n").expect("write source");
        let limits = LocalSourceLimits {
            max_files: 8,
            max_bytes: 1024,
            max_depth: 8,
        };

        let record = resolve_source_cache_record(
            SourceCacheRequest::LocalPath(root.clone()),
            &cache,
            limits,
        );

        assert_eq!(record.verdict, SourceCacheVerdict::DiagnosticObserved);
        assert_eq!(record.source_kind, "local-path");
        assert_eq!(record.file_count, Some(1));
        assert_eq!(record.max_files, 8);
        let snapshot_path = PathBuf::from(record.cache_path.as_ref().expect("snapshot path"));
        assert!(snapshot_path.is_dir());
        assert_ne!(
            snapshot_path,
            root.canonicalize().expect("canonical live root")
        );
        assert!(
            snapshot_path.starts_with(
                cache
                    .canonicalize()
                    .expect("canonical snapshot cache")
                    .join("local-snapshots")
            )
        );
        assert_eq!(
            record.content_identity.as_ref().expect("identity").len(),
            64
        );
        assert!(
            record
                .to_json()
                .contains("\"submodule_policy\": \"git-submodules-not-applicable\"")
        );
        assert!(record.path_policy.starts_with("validated-local-snapshot;"));
        assert_eq!(record.fingerprint().len(), 64);

        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn rejected_source_cache_record_captures_policy_failure() {
        let root = temp_root("reject");
        let cache = temp_root("cache");
        std::fs::create_dir_all(&root).expect("create source");
        std::fs::write(root.join("main.omg"), "").expect("write source");

        let record = resolve_source_cache_record(
            SourceCacheRequest::LocalPath(root.clone()),
            &cache,
            LocalSourceLimits {
                max_files: 0,
                ..LocalSourceLimits::default()
            },
        );

        assert_eq!(record.verdict, SourceCacheVerdict::Rejected);
        assert!(record.content_identity.is_none());
        assert!(
            record
                .rejection
                .as_deref()
                .expect("rejection")
                .contains("identity entry limit")
        );

        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn source_cache_policy_record_json_round_trip_is_normalized() {
        let record = SourceCachePolicyRecord {
            schema_version: SOURCE_CACHE_POLICY_SCHEMA_VERSION,
            verdict: SourceCacheVerdict::DiagnosticObserved,
            source_kind: "git".to_owned(),
            locator: "git@github.com:CathedralOS/file-journal.git".to_owned(),
            transport_profile: Some("ssh".to_owned()),
            requested_rev: Some("0123456789abcdef0123456789abcdef01234567".to_owned()),
            resolved_commit: Some("0123456789abcdef0123456789abcdef01234567".to_owned()),
            resolved_tree: Some("89abcdef0123456789abcdef0123456789abcdef".to_owned()),
            content_identity: Some(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
            ),
            cache_path: Some("/tmp/omega-cache/file-journal".to_owned()),
            file_count: Some(3),
            byte_count: Some(1024),
            max_files: 4096,
            max_bytes: 268435456,
            max_depth: 64,
            submodule_policy: "gitmodules-rejected-until-submodules-are-explicit-package-edges"
                .to_owned(),
            path_policy:
                "validated-object-snapshot; canonical-root-contained; symlink-escapes-rejected; dot-git-excluded"
                    .to_owned(),
            rejection: None,
        };

        let parsed = SourceCachePolicyRecord::from_json(&record.to_json())
            .expect("record JSON should parse");

        assert_eq!(parsed, record);
        assert_eq!(parsed.to_json(), record.to_json());
        assert_eq!(parsed.fingerprint(), record.fingerprint());
    }

    #[test]
    fn source_cache_policy_record_read_write_round_trip() {
        let root = temp_root("persist-record");
        std::fs::create_dir_all(&root).expect("create record temp");
        let path = root.join("source-cache-policy.json");
        let record = rejected_test_record();

        record.write_to_path(&path).expect("write policy record");
        let read = SourceCachePolicyRecord::read_from_path(&path).expect("read policy record");

        assert_eq!(read, record);
        assert_eq!(
            std::fs::read_to_string(&path).expect("record file"),
            record.to_json()
        );
        assert_eq!(
            std::fs::read_dir(&root).expect("record directory").count(),
            1,
            "successful publication removes its exclusive stage"
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            assert_eq!(
                std::fs::metadata(&path)
                    .expect("record metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn source_cache_policy_record_persistence_is_bounded_and_canonical() {
        let root = temp_root("bounded-record");
        std::fs::create_dir_all(&root).expect("create record temp");
        let write_path = root.join("write.json");
        let read_path = root.join("read.json");
        let noncanonical_path = root.join("noncanonical.json");
        let record = rejected_test_record();
        let canonical = record.to_json();
        let insufficient = SourceCachePolicyRecordPersistenceLimits::new(canonical.len() - 1);

        assert!(matches!(
            record.write_to_path_with_limits(&write_path, insufficient),
            Err(SourceCachePolicyRecordPersistenceError::ByteLimitExceeded { .. })
        ));
        assert!(!write_path.exists());

        std::fs::write(&read_path, &canonical).expect("write read fixture");
        assert!(matches!(
            SourceCachePolicyRecord::read_from_path_with_limits(&read_path, insufficient),
            Err(SourceCachePolicyRecordPersistenceError::ByteLimitExceeded { .. })
        ));

        let mut noncanonical = canonical;
        noncanonical.push('\n');
        std::fs::write(&noncanonical_path, noncanonical).expect("write noncanonical fixture");
        assert_eq!(
            SourceCachePolicyRecord::read_from_path(&noncanonical_path),
            Err(SourceCachePolicyRecordPersistenceError::NonCanonicalEncoding)
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn source_cache_policy_record_publication_never_overwrites_a_destination() {
        let root = temp_root("existing-record");
        std::fs::create_dir_all(&root).expect("create record temp");
        let path = root.join("source-cache-policy.json");
        std::fs::write(&path, b"existing bytes").expect("write existing destination");

        assert_eq!(
            rejected_test_record().write_to_path(&path),
            Err(SourceCachePolicyRecordPersistenceError::DestinationExists {
                path: path.canonicalize().expect("canonical destination"),
            })
        );
        assert_eq!(
            std::fs::read(&path).expect("unchanged destination"),
            b"existing bytes"
        );
        assert_eq!(
            std::fs::read_dir(&root).expect("record directory").count(),
            1,
            "failed publication removes its exclusive stage"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn source_cache_policy_record_persistence_rejects_leaf_symlinks() {
        use std::os::unix::fs::symlink;

        let root = temp_root("symlink-record");
        std::fs::create_dir_all(&root).expect("create record temp");
        let target = root.join("target.json");
        let link = root.join("source-cache-policy.json");
        std::fs::write(&target, b"target bytes").expect("write symlink target");
        symlink(&target, &link).expect("create destination symlink");

        assert!(matches!(
            rejected_test_record().write_to_path(&link),
            Err(SourceCachePolicyRecordPersistenceError::DestinationExists { .. })
        ));
        assert!(matches!(
            SourceCachePolicyRecord::read_from_path(&link),
            Err(SourceCachePolicyRecordPersistenceError::NotRegularFile { .. })
        ));
        assert_eq!(
            std::fs::read(&target).expect("unchanged symlink target"),
            b"target bytes"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn source_cache_policy_record_parse_rejects_unknown_schema_and_fields() {
        let unknown_schema = "{\n  \"schema_version\": 99,\n  \"verdict\": \"accepted\",\n  \"source_kind\": \"local-path\",\n  \"locator\": \".\",\n  \"transport_profile\": null,\n  \"requested_rev\": null,\n  \"resolved_commit\": null,\n  \"resolved_tree\": null,\n  \"content_identity\": null,\n  \"cache_path\": null,\n  \"file_count\": null,\n  \"byte_count\": null,\n  \"max_files\": 4096,\n  \"max_bytes\": 268435456,\n  \"max_depth\": 64,\n  \"submodule_policy\": \"git-submodules-not-applicable\",\n  \"path_policy\": \"canonical-root-contained\",\n  \"rejection\": null\n}\n";
        assert_eq!(
            SourceCachePolicyRecord::from_json(unknown_schema),
            Err(
                SourceCachePolicyRecordParseError::UnsupportedSchemaVersion {
                    found: 99,
                    supported: SOURCE_CACHE_POLICY_SCHEMA_VERSION,
                }
            )
        );

        let unexpected_field = "{\n  \"schema_version\": 3,\n  \"verdict\": \"diagnostic-observed\",\n  \"source_kind\": \"local-path\",\n  \"locator\": \".\",\n  \"transport_profile\": null,\n  \"requested_rev\": null,\n  \"resolved_commit\": null,\n  \"resolved_tree\": null,\n  \"content_identity\": null,\n  \"cache_path\": null,\n  \"file_count\": null,\n  \"byte_count\": null,\n  \"max_files\": 4096,\n  \"max_bytes\": 268435456,\n  \"max_depth\": 64,\n  \"submodule_policy\": \"git-submodules-not-applicable\",\n  \"path_policy\": \"canonical-root-contained\",\n  \"rejection\": null,\n  \"extra\": \"no\"\n}\n";
        assert_eq!(
            SourceCachePolicyRecord::from_json(unexpected_field),
            Err(SourceCachePolicyRecordParseError::UnexpectedField {
                field: "$.extra".to_owned(),
            })
        );
    }

    #[test]
    fn git_source_cache_record_captures_commit_tree_and_submodule_policy() {
        let repo = temp_root("git");
        let cache = temp_root("git-cache");
        std::fs::create_dir_all(&repo).expect("create git package");
        run_test_git(&repo, ["init", "--quiet"]);
        run_test_git(&repo, ["config", "user.email", "omega@example.invalid"]);
        run_test_git(&repo, ["config", "user.name", "Omega Tests"]);
        std::fs::write(repo.join("main.omg"), "machine Main::main() {}\n").expect("write source");
        run_test_git(&repo, ["add", "main.omg"]);
        run_test_git(&repo, ["commit", "--quiet", "-m", "initial"]);

        let request = GitSourceRequest::for_local_test_repository(&repo, Some("HEAD".to_owned()))
            .expect("local Git fixture request");
        let locator_identity = request.locator_identity().to_owned();
        let record = resolve_source_cache_record(
            SourceCacheRequest::Git(request),
            &cache,
            LocalSourceLimits::default(),
        );

        assert_eq!(record.verdict, SourceCacheVerdict::DiagnosticObserved);
        assert_eq!(record.source_kind, "git");
        assert_eq!(record.locator, locator_identity);
        assert_eq!(record.transport_profile.as_deref(), Some("test-file"));
        assert!(!record.locator.contains(&repo.display().to_string()));
        assert_eq!(record.requested_rev.as_deref(), Some("HEAD"));
        assert_eq!(record.resolved_commit.as_ref().expect("commit").len(), 40);
        assert_eq!(record.resolved_tree.as_ref().expect("tree").len(), 40);
        assert!(record.submodule_policy.contains("gitmodules-rejected"));

        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&cache);
    }
}
