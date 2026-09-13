//! Filesystem authority, canonical metadata, and exact operation observations.

use crate::{FilesystemReplay, FilesystemSponsor};
use checked_trees::CheckedTrees;

/// Schema for the current incomplete filesystem operation-attempt evidence.
///
/// This records call-start order, exact provider, every successfully authorized
/// scoped path as a grant-root identity plus canonical relative UTF-8 bytes,
/// exact path-like byte operands, each successfully resolved mutable carrier
/// and logical-handle input even when later preparation fails, and a typed
/// returned or evaluator-halted outcome. Exact path results and successful file
/// and directory observation regions plus canonical metadata values are
/// designated, but replay execution is not complete yet.
pub const FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION: u32 = 19;

/// One semantic field in the canonical metadata value returned by the
/// filesystem host seam. This is target-neutral vocabulary; the selected
/// checked layout supplies only its physical offset and stored width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FilesystemMetadataField {
    Device,
    Mode,
    LinkCount,
    Inode,
    User,
    Group,
    ReferencedDevice,
    AccessTime,
    ModificationTime,
    ChangeTime,
    BirthTime,
    Size,
    Blocks512,
    PreferredBlockSize,
}

impl FilesystemMetadataField {
    pub const ALL: [Self; 14] = [
        Self::Device,
        Self::Mode,
        Self::LinkCount,
        Self::Inode,
        Self::User,
        Self::Group,
        Self::ReferencedDevice,
        Self::AccessTime,
        Self::ModificationTime,
        Self::ChangeTime,
        Self::BirthTime,
        Self::Size,
        Self::Blocks512,
        Self::PreferredBlockSize,
    ];

    pub const fn semantic_width_bits(self) -> u16 {
        match self {
            Self::Mode | Self::User | Self::Group => 32,
            Self::Device
            | Self::LinkCount
            | Self::Inode
            | Self::ReferencedDevice
            | Self::AccessTime
            | Self::ModificationTime
            | Self::ChangeTime
            | Self::BirthTime
            | Self::Size
            | Self::Blocks512
            | Self::PreferredBlockSize => 64,
        }
    }

    pub const fn is_signed(self) -> bool {
        matches!(
            self,
            Self::AccessTime
                | Self::ModificationTime
                | Self::ChangeTime
                | Self::BirthTime
                | Self::Size
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilesystemMetadataFieldLayout {
    pub(crate) field: FilesystemMetadataField,
    pub(crate) offset: usize,
    pub(crate) stored_width_bits: u16,
}

impl FilesystemMetadataFieldLayout {
    pub const fn new(
        field: FilesystemMetadataField,
        offset: usize,
        stored_width_bits: u16,
    ) -> Self {
        Self {
            field,
            offset,
            stored_width_bits,
        }
    }

    pub const fn field(self) -> FilesystemMetadataField {
        self.field
    }

    pub const fn offset(self) -> usize {
        self.offset
    }

    pub const fn stored_width_bits(self) -> u16 {
        self.stored_width_bits
    }
}

/// Checked physical carrier geometry for one selected target's `StatRecord`.
///
/// Omega orchestration derives this from the already-evaluated programmable
/// layout and supplies it to Psi. Package strings and raw target IR never enter
/// the interpreter. Construction rejects missing, duplicate, overlapping,
/// over-wide, and out-of-record fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemMetadataLayout {
    pub(crate) record_size: usize,
    pub(crate) fields: Vec<FilesystemMetadataFieldLayout>,
}

impl FilesystemMetadataLayout {
    pub fn new(
        record_size: usize,
        mut fields: Vec<FilesystemMetadataFieldLayout>,
    ) -> Result<Self, String> {
        if record_size == 0 {
            return Err("filesystem metadata layout has an empty record".to_owned());
        }
        fields.sort_unstable_by_key(|field| field.field);
        if fields.len() != FilesystemMetadataField::ALL.len()
            || fields
                .iter()
                .map(|field| field.field)
                .ne(FilesystemMetadataField::ALL)
        {
            return Err(
                "filesystem metadata layout must contain each canonical field exactly once"
                    .to_owned(),
            );
        }
        for field in &fields {
            if !matches!(field.stored_width_bits, 16 | 32 | 64)
                || field.stored_width_bits > field.field.semantic_width_bits()
            {
                return Err(format!(
                    "filesystem metadata field {:?} has invalid stored width {}",
                    field.field, field.stored_width_bits
                ));
            }
            let width = usize::from(field.stored_width_bits / 8);
            let end = field.offset.checked_add(width).ok_or_else(|| {
                format!(
                    "filesystem metadata field {:?} extent overflows",
                    field.field
                )
            })?;
            if end > record_size {
                return Err(format!(
                    "filesystem metadata field {:?} ends at {end}, beyond record size {record_size}",
                    field.field
                ));
            }
        }
        for (index, left) in fields.iter().enumerate() {
            let left_end = left.offset + usize::from(left.stored_width_bits / 8);
            for right in fields.iter().skip(index + 1) {
                let right_end = right.offset + usize::from(right.stored_width_bits / 8);
                if left.offset < right_end && right.offset < left_end {
                    return Err(format!(
                        "filesystem metadata fields {:?} and {:?} overlap",
                        left.field, right.field
                    ));
                }
            }
        }
        Ok(Self {
            record_size,
            fields,
        })
    }

    pub const fn record_size(&self) -> usize {
        self.record_size
    }

    pub fn field_layout(&self, field: FilesystemMetadataField) -> FilesystemMetadataFieldLayout {
        *self
            .fields
            .iter()
            .find(|layout| layout.field == field)
            .expect("validated metadata layout contains every field")
    }

    pub(crate) fn host() -> Self {
        let fields = if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            vec![
                (FilesystemMetadataField::Device, 0, 64),
                (FilesystemMetadataField::Mode, 24, 32),
                (FilesystemMetadataField::LinkCount, 16, 64),
                (FilesystemMetadataField::Inode, 8, 64),
                (FilesystemMetadataField::User, 28, 32),
                (FilesystemMetadataField::Group, 32, 32),
                (FilesystemMetadataField::ReferencedDevice, 40, 64),
                (FilesystemMetadataField::AccessTime, 72, 64),
                (FilesystemMetadataField::ModificationTime, 88, 64),
                (FilesystemMetadataField::ChangeTime, 104, 64),
                (FilesystemMetadataField::BirthTime, 120, 64),
                (FilesystemMetadataField::Size, 48, 64),
                (FilesystemMetadataField::Blocks512, 64, 64),
                (FilesystemMetadataField::PreferredBlockSize, 56, 64),
            ]
        } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
            vec![
                (FilesystemMetadataField::Device, 0, 64),
                (FilesystemMetadataField::Mode, 16, 32),
                (FilesystemMetadataField::LinkCount, 20, 32),
                (FilesystemMetadataField::Inode, 8, 64),
                (FilesystemMetadataField::User, 24, 32),
                (FilesystemMetadataField::Group, 28, 32),
                (FilesystemMetadataField::ReferencedDevice, 32, 64),
                (FilesystemMetadataField::AccessTime, 72, 64),
                (FilesystemMetadataField::ModificationTime, 88, 64),
                (FilesystemMetadataField::ChangeTime, 104, 64),
                (FilesystemMetadataField::BirthTime, 120, 64),
                (FilesystemMetadataField::Size, 48, 64),
                (FilesystemMetadataField::Blocks512, 64, 64),
                (FilesystemMetadataField::PreferredBlockSize, 56, 32),
            ]
        } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            vec![
                (FilesystemMetadataField::Device, 0, 32),
                (FilesystemMetadataField::Mode, 4, 16),
                (FilesystemMetadataField::LinkCount, 6, 16),
                (FilesystemMetadataField::Inode, 8, 64),
                (FilesystemMetadataField::User, 16, 32),
                (FilesystemMetadataField::Group, 20, 32),
                (FilesystemMetadataField::ReferencedDevice, 24, 32),
                (FilesystemMetadataField::AccessTime, 32, 64),
                (FilesystemMetadataField::ModificationTime, 48, 64),
                (FilesystemMetadataField::ChangeTime, 64, 64),
                (FilesystemMetadataField::BirthTime, 80, 64),
                (FilesystemMetadataField::Size, 96, 64),
                (FilesystemMetadataField::Blocks512, 104, 64),
                (FilesystemMetadataField::PreferredBlockSize, 112, 32),
            ]
        } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
            vec![
                (FilesystemMetadataField::Device, 0, 32),
                (FilesystemMetadataField::Mode, 6, 16),
                (FilesystemMetadataField::LinkCount, 8, 16),
                (FilesystemMetadataField::Inode, 64, 64),
                (FilesystemMetadataField::User, 72, 32),
                (FilesystemMetadataField::Group, 76, 32),
                (FilesystemMetadataField::ReferencedDevice, 16, 32),
                (FilesystemMetadataField::AccessTime, 32, 64),
                (FilesystemMetadataField::ModificationTime, 40, 64),
                (FilesystemMetadataField::ChangeTime, 80, 64),
                (FilesystemMetadataField::BirthTime, 48, 64),
                (FilesystemMetadataField::Size, 24, 64),
                (FilesystemMetadataField::Blocks512, 88, 64),
                (FilesystemMetadataField::PreferredBlockSize, 96, 32),
            ]
        } else {
            panic!("unsupported host filesystem metadata layout")
        };
        let record_size = if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
            128
        } else {
            144
        };
        Self::new(
            record_size,
            fields
                .into_iter()
                .map(|(field, offset, width)| {
                    FilesystemMetadataFieldLayout::new(field, offset, width)
                })
                .collect(),
        )
        .expect("host metadata layout is canonical")
    }
}

impl Default for FilesystemMetadataLayout {
    fn default() -> Self {
        Self::host()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemObservationProvider {
    /// Deterministic in-memory provider; no host filesystem was touched.
    Virtual,
    /// Real process filesystem without path grants. Build admission does not
    /// select this provider.
    RealUnscoped,
    /// Real filesystem constrained by compiler-supplied path grants.
    RealScoped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemGrantAccess {
    Read,
    Write,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemGrantRefusalReason {
    Unresolvable,
    OutsideGrantedRoots,
    UnrepresentableRootedPath,
    ObservationEvidenceLimitExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilesystemGrantRefusal {
    pub(crate) operand_ordinal: u8,
    pub(crate) access: FilesystemGrantAccess,
    pub(crate) reason: FilesystemGrantRefusalReason,
}

/// Compiler-issued identity for one scoped filesystem grant root.
///
/// The checked interpreter treats this as an opaque coordinate. The caller
/// owns its meaning (for example, Omega assigns distinct identities to the
/// immutable package source and writable build output roots). Zero is reserved
/// so an omitted/default identity cannot enter evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FilesystemGrantRootIdentity(u32);

/// Current compiler sponsorship ceiling for one canonical path beneath a
/// filesystem grant root. This is an evaluator evidence limit, not a language
/// limit.
pub const FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT: usize = 16 * 1024 * 1024;

/// Whether bytes are the canonical target-neutral spelling of a path beneath
/// one grant root. Authorized targets may denote the root itself; authored
/// rooted operands may not.
pub fn filesystem_root_relative_path_is_canonical(relative: &[u8], allow_empty: bool) -> bool {
    if relative.len() > FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT
        || relative.contains(&0)
        || std::str::from_utf8(relative).is_err()
    {
        return false;
    }
    if relative.is_empty() {
        return allow_empty;
    }
    if relative[0] == b'/'
        || relative.contains(&b'\\')
        || (relative.len() >= 2 && relative[1] == b':')
    {
        return false;
    }
    !relative
        .split(|byte| *byte == b'/')
        .any(|component| component.is_empty() || component == b"." || component == b"..")
}

/// Version of Psi's canonical immutable-source metadata policy.
pub const CANONICAL_FILESYSTEM_METADATA_POLICY_VERSION: u32 = 1;

/// Maximum complete source-tree rows accepted by the canonical metadata
/// carrier. One row is reserved for the source root; the remaining 65,536
/// match the package resolver's source-entry ceiling.
pub const CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT: usize = 65_537;

/// Whether raw bytes are one canonical slash-separated source-tree coordinate.
///
/// Unlike runtime rooted-path evidence, the complete source index deliberately
/// does not require UTF-8. Source custody preserves otherwise valid raw Unix
/// names even when package code cannot express those names through Psi's
/// target-neutral runtime path gate.
pub fn canonical_filesystem_metadata_path_is_canonical(relative: &[u8], allow_empty: bool) -> bool {
    if relative.len() > FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT
        || relative.contains(&0)
        || relative.contains(&b'\\')
    {
        return false;
    }
    if relative.is_empty() {
        return allow_empty;
    }
    if relative[0] == b'/' {
        return false;
    }
    !relative
        .split(|byte| *byte == b'/')
        .any(|component| component.is_empty() || component == b"." || component == b"..")
}

/// Closed source entry shape from which package-visible metadata is derived.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanonicalFilesystemMetadataRowKind {
    Directory,
    File {
        executable: bool,
        logical_byte_length: u64,
    },
    Symlink {
        target_spelling_logical_byte_length: u64,
    },
}

impl CanonicalFilesystemMetadataRowKind {
    pub const fn logical_byte_length(self) -> u64 {
        match self {
            Self::Directory => 0,
            Self::File {
                logical_byte_length,
                ..
            } => logical_byte_length,
            Self::Symlink {
                target_spelling_logical_byte_length,
            } => target_spelling_logical_byte_length,
        }
    }
}

/// One raw root-relative row in a canonical immutable-source metadata index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalFilesystemMetadataRow {
    pub(crate) relative_path: Vec<u8>,
    pub(crate) kind: CanonicalFilesystemMetadataRowKind,
}

impl CanonicalFilesystemMetadataRow {
    pub fn new(
        relative_path: impl Into<Vec<u8>>,
        kind: CanonicalFilesystemMetadataRowKind,
    ) -> Self {
        Self {
            relative_path: relative_path.into(),
            kind,
        }
    }

    pub fn relative_path(&self) -> &[u8] {
        &self.relative_path
    }

    pub const fn kind(&self) -> CanonicalFilesystemMetadataRowKind {
        self.kind
    }
}

/// Why compiler-supplied immutable-source metadata is not canonical.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalFilesystemMetadataIndexError {
    UnsupportedPolicyVersion(u32),
    RowLimitExceeded { limit: usize, attempted: usize },
    InvalidRelativePath(Vec<u8>),
    DuplicateRelativePath(Vec<u8>),
    AggregatePathBytesLimitExceeded { limit: usize, attempted: usize },
    LogicalByteLengthExceedsI64(Vec<u8>),
    MissingRootDirectory,
    RootIsNotDirectory,
    MissingParentDirectory(Vec<u8>),
    ParentIsNotDirectory(Vec<u8>),
}

impl std::fmt::Display for CanonicalFilesystemMetadataIndexError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedPolicyVersion(version) => {
                write!(
                    formatter,
                    "unsupported canonical filesystem metadata policy {version}"
                )
            }
            Self::RowLimitExceeded { limit, attempted } => write!(
                formatter,
                "canonical filesystem metadata rows exceed {limit}: attempted {attempted}"
            ),
            Self::InvalidRelativePath(path) => write!(
                formatter,
                "canonical filesystem metadata contains an invalid relative path: {path:?}"
            ),
            Self::DuplicateRelativePath(path) => write!(
                formatter,
                "canonical filesystem metadata duplicates relative path: {path:?}"
            ),
            Self::AggregatePathBytesLimitExceeded { limit, attempted } => write!(
                formatter,
                "canonical filesystem metadata path bytes exceed {limit}: attempted {attempted}"
            ),
            Self::LogicalByteLengthExceedsI64(path) => write!(
                formatter,
                "canonical filesystem metadata length does not fit i64 at path: {path:?}"
            ),
            Self::MissingRootDirectory => {
                write!(
                    formatter,
                    "canonical filesystem metadata omits the root directory"
                )
            }
            Self::RootIsNotDirectory => {
                write!(
                    formatter,
                    "canonical filesystem metadata root is not a directory"
                )
            }
            Self::MissingParentDirectory(path) => write!(
                formatter,
                "canonical filesystem metadata omits a parent directory for path: {path:?}"
            ),
            Self::ParentIsNotDirectory(path) => write!(
                formatter,
                "canonical filesystem metadata parent is not a directory for path: {path:?}"
            ),
        }
    }
}

impl std::error::Error for CanonicalFilesystemMetadataIndexError {}

/// Immutable, validated metadata for one complete content-authenticated source.
///
/// The source-content commitment is deliberately opaque to Psi. The package
/// resolver owns its construction and the compiler binds it to the source
/// identity; the interpreter only enforces the closed metadata policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalFilesystemMetadataIndex {
    pub(crate) policy_version: u32,
    pub(crate) source_content_commitment: [u8; 32],
    pub(crate) rows: std::collections::BTreeMap<Vec<u8>, CanonicalFilesystemMetadataRowKind>,
}

impl CanonicalFilesystemMetadataIndex {
    pub fn version_1(
        source_content_commitment: [u8; 32],
        rows: impl IntoIterator<Item = CanonicalFilesystemMetadataRow>,
    ) -> Result<Self, CanonicalFilesystemMetadataIndexError> {
        Self::new(
            CANONICAL_FILESYSTEM_METADATA_POLICY_VERSION,
            source_content_commitment,
            rows,
        )
    }

    pub fn new(
        policy_version: u32,
        source_content_commitment: [u8; 32],
        rows: impl IntoIterator<Item = CanonicalFilesystemMetadataRow>,
    ) -> Result<Self, CanonicalFilesystemMetadataIndexError> {
        if policy_version != CANONICAL_FILESYSTEM_METADATA_POLICY_VERSION {
            return Err(
                CanonicalFilesystemMetadataIndexError::UnsupportedPolicyVersion(policy_version),
            );
        }
        let mut total_path_bytes = 0usize;
        let mut canonical_rows = std::collections::BTreeMap::new();
        for (row_index, row) in rows.into_iter().enumerate() {
            if row_index >= CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT {
                return Err(CanonicalFilesystemMetadataIndexError::RowLimitExceeded {
                    limit: CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT,
                    attempted: row_index.saturating_add(1),
                });
            }
            if !canonical_filesystem_metadata_path_is_canonical(&row.relative_path, true) {
                return Err(CanonicalFilesystemMetadataIndexError::InvalidRelativePath(
                    row.relative_path,
                ));
            }
            total_path_bytes = total_path_bytes
                .checked_add(row.relative_path.len())
                .filter(|total| *total <= FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT)
                .ok_or(
                    CanonicalFilesystemMetadataIndexError::AggregatePathBytesLimitExceeded {
                        limit: FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT,
                        attempted: total_path_bytes.saturating_add(row.relative_path.len()),
                    },
                )?;
            if row.kind.logical_byte_length() > i64::MAX as u64 {
                return Err(
                    CanonicalFilesystemMetadataIndexError::LogicalByteLengthExceedsI64(
                        row.relative_path,
                    ),
                );
            }
            if canonical_rows
                .insert(row.relative_path.clone(), row.kind)
                .is_some()
            {
                return Err(
                    CanonicalFilesystemMetadataIndexError::DuplicateRelativePath(row.relative_path),
                );
            }
        }
        match canonical_rows.get(b"".as_slice()) {
            None => return Err(CanonicalFilesystemMetadataIndexError::MissingRootDirectory),
            Some(CanonicalFilesystemMetadataRowKind::Directory) => {}
            Some(_) => return Err(CanonicalFilesystemMetadataIndexError::RootIsNotDirectory),
        }
        for path in canonical_rows.keys().filter(|path| !path.is_empty()) {
            let parent = path
                .iter()
                .rposition(|byte| *byte == b'/')
                .map_or(b"".as_slice(), |separator| &path[..separator]);
            match canonical_rows.get(parent) {
                None => {
                    return Err(
                        CanonicalFilesystemMetadataIndexError::MissingParentDirectory(path.clone()),
                    );
                }
                Some(CanonicalFilesystemMetadataRowKind::Directory) => {}
                Some(_) => {
                    return Err(CanonicalFilesystemMetadataIndexError::ParentIsNotDirectory(
                        path.clone(),
                    ));
                }
            }
        }
        Ok(Self {
            policy_version,
            source_content_commitment,
            rows: canonical_rows,
        })
    }

    pub const fn policy_version(&self) -> u32 {
        self.policy_version
    }

    pub const fn source_content_commitment(&self) -> &[u8; 32] {
        &self.source_content_commitment
    }

    pub fn rows(&self) -> impl ExactSizeIterator<Item = CanonicalFilesystemMetadataRow> + '_ {
        self.rows.iter().map(|(relative_path, kind)| {
            CanonicalFilesystemMetadataRow::new(relative_path.clone(), *kind)
        })
    }

    pub(crate) fn row(&self, relative_path: &[u8]) -> Option<CanonicalFilesystemMetadataRowKind> {
        self.rows.get(relative_path).copied()
    }
}

impl FilesystemGrantRootIdentity {
    pub const fn new(value: u32) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// One compiler-supplied physical grant root and its evidence identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemGrantRoot {
    pub(crate) identity: FilesystemGrantRootIdentity,
    pub(crate) path: std::path::PathBuf,
    pub(crate) canonical_metadata: Option<CanonicalFilesystemMetadataIndex>,
}

impl FilesystemGrantRoot {
    pub fn new(identity: FilesystemGrantRootIdentity, path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            identity,
            path: path.into(),
            canonical_metadata: None,
        }
    }

    /// Attach canonical immutable-source metadata to this grant root.
    pub fn with_canonical_metadata(
        mut self,
        canonical_metadata: CanonicalFilesystemMetadataIndex,
    ) -> Self {
        self.canonical_metadata = Some(canonical_metadata);
        self
    }

    pub const fn identity(&self) -> FilesystemGrantRootIdentity {
        self.identity
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    pub const fn canonical_metadata(&self) -> Option<&CanonicalFilesystemMetadataIndex> {
        self.canonical_metadata.as_ref()
    }
}

/// One scoped path that passed the grant gate before host access.
///
/// `relative_path` uses `/` between UTF-8 components, never carries a leading
/// separator, and is empty for the root itself. It therefore contains no host
/// absolute path or compiler working-directory spelling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemAuthorizedPath {
    pub(crate) operand_ordinal: u8,
    pub(crate) access: FilesystemGrantAccess,
    pub(crate) root: FilesystemGrantRootIdentity,
    pub(crate) relative_path: Vec<u8>,
}

/// Canonical non-handle scalar value consumed by one filesystem operation.
/// Width and signedness remain explicit so ABI-distinct operands never compare
/// equal merely because the interpreter carries both in an `i64`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemScalarOperandValue {
    I32(i32),
    U32(u32),
    I64(i64),
    U64(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilesystemScalarOperand {
    pub(crate) operand_ordinal: u8,
    pub(crate) value: FilesystemScalarOperandValue,
}

impl FilesystemScalarOperand {
    pub const fn operand_ordinal(self) -> u8 {
        self.operand_ordinal
    }

    pub const fn value(self) -> FilesystemScalarOperandValue {
        self.value
    }
}

/// Immutable non-path payload bytes consumed by one operation. Rooted paths
/// and path-like byte aliases stay in path evidence so compiler/cache absolute
/// spellings cannot leak through this row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemByteOperand {
    pub(crate) operand_ordinal: u8,
    pub(crate) bytes: Vec<u8>,
}

impl FilesystemByteOperand {
    pub const fn operand_ordinal(&self) -> u8 {
        self.operand_ordinal
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Exact bytes consumed where an operation assigns path-like meaning without
/// consuming a rooted path grant. Keeping this distinct from immutable payload
/// bytes and authorized rooted paths preserves the operation's operand roles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemPathLikeOperand {
    pub(crate) operand_ordinal: u8,
    pub(crate) bytes: Vec<u8>,
}

impl FilesystemPathLikeOperand {
    pub const fn operand_ordinal(&self) -> u8 {
        self.operand_ordinal
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// One compiler-rooted path at the instant its authored operand successfully
/// resolves during call preparation. This preserves the portable input before
/// physical provider-path lowering. It is not an authorization result: a later
/// grant check may resolve symlinks to a different canonical rooted location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemRootedPathOperandResolution {
    pub(crate) operand_ordinal: u8,
    pub(crate) root: FilesystemGrantRootIdentity,
    pub(crate) relative_path: Vec<u8>,
}

/// Closed semantic class for exact meaningful path bytes returned through a
/// mutable output carrier. Terminators and unchanged carrier tails are not part
/// of these bytes; the complete carrier remains available separately.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemReturnedPathKind {
    ReadLinkPayload,
    CanonicalPath,
    FinalPath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemReturnedPathCompleteness {
    Complete,
    LimitReached,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemReturnedPath {
    pub(crate) operand_ordinal: u8,
    pub(crate) kind: FilesystemReturnedPathKind,
    pub(crate) completeness: FilesystemReturnedPathCompleteness,
    pub(crate) bytes: Vec<u8>,
}

/// Semantic designation of one host-derived byte region returned through an
/// already-custodied mutable output carrier. The bytes are referenced from the
/// matching provider post-state rather than copied a fourth time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemObservedByteRegionKind {
    SequentialFileRead,
    PositionedFileRead,
    DirectoryRecords,
    FindEntry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilesystemObservedByteRegion {
    pub(crate) output_operand_ordinal: u8,
    pub(crate) kind: FilesystemObservedByteRegionKind,
    pub(crate) offset: usize,
    pub(crate) length: usize,
}

/// Semantic source of one successfully returned metadata record. The kind is
/// independent of the target carrier used to return the same fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemMetadataObservationKind {
    FollowedPath,
    OpenDescriptor,
    UnfollowedFinalPath,
}

/// Minimum mutable byte carrier required by the canonical filesystem metadata
/// API on every selected target.
pub const FILESYSTEM_METADATA_API_CARRIER_BYTES: usize = 144;

/// Canonical target-neutral metadata observed by one successful filesystem
/// operation. File-kind predicates are deliberately absent: they are derived
/// from the retained mode bits and must not become disagreeing duplicate facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilesystemMetadataObservation {
    pub(crate) output_operand_ordinal: u8,
    pub(crate) kind: FilesystemMetadataObservationKind,
    pub(crate) device: u64,
    pub(crate) mode: u32,
    pub(crate) link_count: u64,
    pub(crate) inode: u64,
    pub(crate) user: u32,
    pub(crate) group: u32,
    pub(crate) referenced_device: u64,
    pub(crate) access_time: i64,
    pub(crate) modification_time: i64,
    pub(crate) change_time: i64,
    pub(crate) birth_time: i64,
    pub(crate) size: i64,
    pub(crate) blocks_512: u64,
    pub(crate) preferred_block_size: u64,
}

impl FilesystemMetadataObservation {
    pub(crate) const fn new(
        output_operand_ordinal: u8,
        kind: FilesystemMetadataObservationKind,
        mode: u32,
        size: i64,
        modification_time: i64,
    ) -> Self {
        Self {
            output_operand_ordinal,
            kind,
            device: 16_777_220,
            mode,
            link_count: 1,
            inode: 1_000_000,
            user: 501,
            group: 20,
            referenced_device: 0,
            access_time: 1_000_000_100,
            modification_time,
            change_time: 1_000_000_050,
            birth_time: 999_999_900,
            size,
            blocks_512: 8,
            preferred_block_size: 4096,
        }
    }

    /// Reconstruct one canonical metadata row from compiler-owned replay
    /// custody. This does not consult or authorize a host filesystem. The
    /// replay executor still cross-checks every field against the selected
    /// target carrier before admitting the returned operation.
    #[allow(clippy::too_many_arguments)]
    pub const fn from_replay(
        kind: FilesystemMetadataObservationKind,
        device: u64,
        mode: u32,
        link_count: u64,
        inode: u64,
        user: u32,
        group: u32,
        referenced_device: u64,
        access_time: i64,
        modification_time: i64,
        change_time: i64,
        birth_time: i64,
        size: i64,
        blocks_512: u64,
        preferred_block_size: u64,
    ) -> Self {
        Self {
            output_operand_ordinal: 1,
            kind,
            device,
            mode,
            link_count,
            inode,
            user,
            group,
            referenced_device,
            access_time,
            modification_time,
            change_time,
            birth_time,
            size,
            blocks_512,
            preferred_block_size,
        }
    }

    pub const fn output_operand_ordinal(self) -> u8 {
        self.output_operand_ordinal
    }
    pub const fn kind(self) -> FilesystemMetadataObservationKind {
        self.kind
    }
    pub const fn device(self) -> u64 {
        self.device
    }
    pub const fn mode(self) -> u32 {
        self.mode
    }
    pub const fn link_count(self) -> u64 {
        self.link_count
    }
    pub const fn inode(self) -> u64 {
        self.inode
    }
    pub const fn user(self) -> u32 {
        self.user
    }
    pub const fn group(self) -> u32 {
        self.group
    }
    pub const fn referenced_device(self) -> u64 {
        self.referenced_device
    }
    pub const fn access_time(self) -> i64 {
        self.access_time
    }
    pub const fn modification_time(self) -> i64 {
        self.modification_time
    }
    pub const fn change_time(self) -> i64 {
        self.change_time
    }
    pub const fn birth_time(self) -> i64 {
        self.birth_time
    }
    pub const fn size(self) -> i64 {
        self.size
    }
    pub const fn blocks_512(self) -> u64 {
        self.blocks_512
    }
    pub const fn preferred_block_size(self) -> u64 {
        self.preferred_block_size
    }

    pub(crate) const fn unsigned_field(self, field: FilesystemMetadataField) -> Option<u64> {
        match field {
            FilesystemMetadataField::Device => Some(self.device),
            FilesystemMetadataField::Mode => Some(self.mode as u64),
            FilesystemMetadataField::LinkCount => Some(self.link_count),
            FilesystemMetadataField::Inode => Some(self.inode),
            FilesystemMetadataField::User => Some(self.user as u64),
            FilesystemMetadataField::Group => Some(self.group as u64),
            FilesystemMetadataField::ReferencedDevice => Some(self.referenced_device),
            FilesystemMetadataField::Blocks512 => Some(self.blocks_512),
            FilesystemMetadataField::PreferredBlockSize => Some(self.preferred_block_size),
            FilesystemMetadataField::AccessTime
            | FilesystemMetadataField::ModificationTime
            | FilesystemMetadataField::ChangeTime
            | FilesystemMetadataField::BirthTime
            | FilesystemMetadataField::Size => None,
        }
    }

    pub(crate) const fn signed_field(self, field: FilesystemMetadataField) -> Option<i64> {
        match field {
            FilesystemMetadataField::AccessTime => Some(self.access_time),
            FilesystemMetadataField::ModificationTime => Some(self.modification_time),
            FilesystemMetadataField::ChangeTime => Some(self.change_time),
            FilesystemMetadataField::BirthTime => Some(self.birth_time),
            FilesystemMetadataField::Size => Some(self.size),
            _ => None,
        }
    }
}

impl FilesystemObservedByteRegion {
    pub const fn output_operand_ordinal(self) -> u8 {
        self.output_operand_ordinal
    }

    pub const fn kind(self) -> FilesystemObservedByteRegionKind {
        self.kind
    }

    pub const fn offset(self) -> usize {
        self.offset
    }

    pub const fn length(self) -> usize {
        self.length
    }
}

impl FilesystemReturnedPath {
    pub const fn operand_ordinal(&self) -> u8 {
        self.operand_ordinal
    }

    pub const fn kind(&self) -> FilesystemReturnedPathKind {
        self.kind
    }

    pub const fn completeness(&self) -> FilesystemReturnedPathCompleteness {
        self.completeness
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl FilesystemRootedPathOperandResolution {
    pub const fn operand_ordinal(&self) -> u8 {
        self.operand_ordinal
    }

    pub const fn root(&self) -> FilesystemGrantRootIdentity {
        self.root
    }

    pub fn relative_path(&self) -> &[u8] {
        &self.relative_path
    }
}

/// Complete state of one mutable byte carrier at the instant its authored
/// operand successfully resolves. This preparation-prefix row is distinct from
/// the provider-visible pre/post row because evaluating a later argument may
/// alias and mutate the carrier before provider invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemMutableByteOperandResolution {
    pub(crate) operand_ordinal: u8,
    pub(crate) bytes: Vec<u8>,
}

impl FilesystemMutableByteOperandResolution {
    pub const fn operand_ordinal(&self) -> u8 {
        self.operand_ordinal
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Exact value of one mutable i64 carrier at the instant its authored operand
/// successfully resolves. Provider-visible pre/post timing remains separate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilesystemMutableI64OperandResolution {
    pub(crate) operand_ordinal: u8,
    pub(crate) value: i64,
}

impl FilesystemMutableI64OperandResolution {
    pub const fn operand_ordinal(self) -> u8 {
        self.operand_ordinal
    }

    pub const fn value(self) -> i64 {
        self.value
    }
}

/// Complete provider-visible state of one mutable byte carrier immediately
/// before and after the operation's provider invocation. Both vectors equal
/// the resolved carrier capacity; unchanged tails remain explicit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemMutableByteOperand {
    pub(crate) operand_ordinal: u8,
    pub(crate) pre_bytes: Vec<u8>,
    pub(crate) post_bytes: Vec<u8>,
}

impl FilesystemMutableByteOperand {
    pub const fn operand_ordinal(&self) -> u8 {
        self.operand_ordinal
    }

    pub fn pre_bytes(&self) -> &[u8] {
        &self.pre_bytes
    }

    pub fn post_bytes(&self) -> &[u8] {
        &self.post_bytes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilesystemMutableI64Operand {
    pub(crate) operand_ordinal: u8,
    pub(crate) pre_value: i64,
    pub(crate) post_value: i64,
}

impl FilesystemMutableI64Operand {
    pub const fn operand_ordinal(self) -> u8 {
        self.operand_ordinal
    }

    pub const fn pre_value(self) -> i64 {
        self.pre_value
    }

    pub const fn post_value(self) -> i64 {
        self.post_value
    }
}

impl FilesystemAuthorizedPath {
    pub const fn operand_ordinal(&self) -> u8 {
        self.operand_ordinal
    }

    pub const fn access(&self) -> FilesystemGrantAccess {
        self.access
    }

    pub const fn root(&self) -> FilesystemGrantRootIdentity {
        self.root
    }

    pub fn relative_path(&self) -> &[u8] {
        &self.relative_path
    }
}

/// Evaluator-issued identity for one filesystem descriptor or handle lifetime.
///
/// The identity is allocated independently of provider token values. Reusing a
/// runtime descriptor after close therefore produces a fresh identity, while
/// every operation during one live lifetime refers to the same identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FilesystemLogicalHandleIdentity(u64);

impl FilesystemLogicalHandleIdentity {
    pub(crate) const fn new(value: u64) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Closed resource domain for a logical filesystem handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemLogicalHandleKind {
    /// POSIX/CRT-style integer file descriptor.
    Descriptor,
    /// Native path/file handle, distinct from a CRT descriptor on Windows.
    Native,
    /// Directory-enumeration cursor returned by `find_first`.
    Find,
}

/// Resolution of one authored handle operand against earlier successful
/// operations in the same evaluator run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemLogicalHandleInputResolution {
    Resolved(FilesystemLogicalHandleIdentity),
    /// The canonical ABI explicitly permits a null handle in this position.
    Null,
    /// No live logical lifetime owns the supplied provider token.
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilesystemLogicalHandleInput {
    pub(crate) operand_ordinal: u8,
    pub(crate) kind: FilesystemLogicalHandleKind,
    pub(crate) resolution: FilesystemLogicalHandleInputResolution,
}

impl FilesystemLogicalHandleInput {
    pub const fn operand_ordinal(self) -> u8 {
        self.operand_ordinal
    }

    pub const fn kind(self) -> FilesystemLogicalHandleKind {
        self.kind
    }

    pub const fn resolution(self) -> FilesystemLogicalHandleInputResolution {
        self.resolution
    }
}

/// Provenance for a successfully returned logical handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemLogicalHandleOutputSource {
    /// A new independently owned resource was opened.
    Created,
    /// A new descriptor lifetime was duplicated from an existing descriptor.
    Duplicated(FilesystemLogicalHandleIdentity),
    /// A native handle view was borrowed from an existing descriptor.
    Borrowed(FilesystemLogicalHandleIdentity),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilesystemLogicalHandleOutput {
    pub(crate) kind: FilesystemLogicalHandleKind,
    pub(crate) identity: FilesystemLogicalHandleIdentity,
    pub(crate) source: FilesystemLogicalHandleOutputSource,
}

impl FilesystemLogicalHandleOutput {
    pub const fn kind(self) -> FilesystemLogicalHandleKind {
        self.kind
    }

    pub const fn identity(self) -> FilesystemLogicalHandleIdentity {
        self.identity
    }

    pub const fn source(self) -> FilesystemLogicalHandleOutputSource {
        self.source
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemEvaluationHaltKind {
    Exit,
    Unsupported,
    Trap,
    ResourceExhausted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemOperationResult {
    Scalar(i64),
    LogicalHandle(FilesystemLogicalHandleIdentity),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemOperationAttemptOutcome {
    Returned {
        result: FilesystemOperationResult,
        post_error: i32,
    },
    EvaluationHalted(FilesystemEvaluationHaltKind),
}

impl FilesystemGrantRefusal {
    pub const fn operand_ordinal(self) -> u8 {
        self.operand_ordinal
    }

    pub const fn access(self) -> FilesystemGrantAccess {
        self.access
    }

    pub const fn reason(self) -> FilesystemGrantRefusalReason {
        self.reason
    }
}

/// One completed canonical filesystem operation attempted during build-machine
/// evaluation. Failed evaluations retain their completed prefix as
/// non-admission evidence.
///
/// The operation tag is an append-only compiler-owned identity. No package
/// string enters this row. Successful descriptor/handle results and uses are
/// normalized into logical lifetimes; provider token numbers do not survive.
/// Failed handle-result sentinels remain scalar results. Mutable carriers
/// retain both their successfully resolved preparation prefix and complete
/// provider-visible pre/post snapshots. Path results and successful file and
/// directory and metadata observations have semantic rows. Replay execution
/// remains incomplete, so this stays below receipt strength.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemOperationAttempt {
    pub(crate) operation_tag: u16,
    pub(crate) provider: FilesystemObservationProvider,
    pub(crate) outcome: Option<FilesystemOperationAttemptOutcome>,
    pub(crate) scalar_operands: Vec<FilesystemScalarOperand>,
    pub(crate) byte_operands: Vec<FilesystemByteOperand>,
    pub(crate) path_like_operands: Vec<FilesystemPathLikeOperand>,
    pub(crate) rooted_path_operand_resolutions: Vec<FilesystemRootedPathOperandResolution>,
    pub(crate) returned_paths: Vec<FilesystemReturnedPath>,
    pub(crate) observed_byte_regions: Vec<FilesystemObservedByteRegion>,
    pub(crate) metadata_observations: Vec<FilesystemMetadataObservation>,
    pub(crate) mutable_byte_operand_resolutions: Vec<FilesystemMutableByteOperandResolution>,
    pub(crate) mutable_i64_operand_resolutions: Vec<FilesystemMutableI64OperandResolution>,
    pub(crate) mutable_byte_operands: Vec<FilesystemMutableByteOperand>,
    pub(crate) mutable_i64_operands: Vec<FilesystemMutableI64Operand>,
    pub(crate) authorized_paths: Vec<FilesystemAuthorizedPath>,
    pub(crate) logical_handle_inputs: Vec<FilesystemLogicalHandleInput>,
    pub(crate) logical_handle_output: Option<FilesystemLogicalHandleOutput>,
    pub(crate) retired_logical_handles: Vec<FilesystemLogicalHandleIdentity>,
    pub(crate) grant_refusals: Vec<FilesystemGrantRefusal>,
}

impl FilesystemOperationAttempt {
    pub(crate) const fn pending(
        operation_tag: u16,
        provider: FilesystemObservationProvider,
    ) -> Self {
        Self {
            operation_tag,
            provider,
            outcome: None,
            scalar_operands: Vec::new(),
            byte_operands: Vec::new(),
            path_like_operands: Vec::new(),
            rooted_path_operand_resolutions: Vec::new(),
            returned_paths: Vec::new(),
            observed_byte_regions: Vec::new(),
            metadata_observations: Vec::new(),
            mutable_byte_operand_resolutions: Vec::new(),
            mutable_i64_operand_resolutions: Vec::new(),
            mutable_byte_operands: Vec::new(),
            mutable_i64_operands: Vec::new(),
            authorized_paths: Vec::new(),
            logical_handle_inputs: Vec::new(),
            logical_handle_output: None,
            retired_logical_handles: Vec::new(),
            grant_refusals: Vec::new(),
        }
    }

    pub const fn operation_tag(&self) -> u16 {
        self.operation_tag
    }

    pub const fn provider(&self) -> FilesystemObservationProvider {
        self.provider
    }

    pub const fn outcome(&self) -> Option<FilesystemOperationAttemptOutcome> {
        self.outcome
    }

    pub const fn result(&self) -> Option<FilesystemOperationResult> {
        match self.outcome {
            Some(FilesystemOperationAttemptOutcome::Returned { result, .. }) => Some(result),
            _ => None,
        }
    }

    pub const fn post_error(&self) -> Option<i32> {
        match self.outcome {
            Some(FilesystemOperationAttemptOutcome::Returned { post_error, .. }) => {
                Some(post_error)
            }
            _ => None,
        }
    }

    pub fn scalar_operands(&self) -> &[FilesystemScalarOperand] {
        &self.scalar_operands
    }

    pub fn byte_operands(&self) -> &[FilesystemByteOperand] {
        &self.byte_operands
    }

    pub fn path_like_operands(&self) -> &[FilesystemPathLikeOperand] {
        &self.path_like_operands
    }

    pub fn rooted_path_operand_resolutions(&self) -> &[FilesystemRootedPathOperandResolution] {
        &self.rooted_path_operand_resolutions
    }

    pub fn returned_paths(&self) -> &[FilesystemReturnedPath] {
        &self.returned_paths
    }

    pub fn observed_byte_regions(&self) -> &[FilesystemObservedByteRegion] {
        &self.observed_byte_regions
    }

    pub fn metadata_observations(&self) -> &[FilesystemMetadataObservation] {
        &self.metadata_observations
    }

    pub fn mutable_byte_operand_resolutions(&self) -> &[FilesystemMutableByteOperandResolution] {
        &self.mutable_byte_operand_resolutions
    }

    pub fn mutable_i64_operand_resolutions(&self) -> &[FilesystemMutableI64OperandResolution] {
        &self.mutable_i64_operand_resolutions
    }

    pub fn mutable_byte_operands(&self) -> &[FilesystemMutableByteOperand] {
        &self.mutable_byte_operands
    }

    pub fn mutable_i64_operands(&self) -> &[FilesystemMutableI64Operand] {
        &self.mutable_i64_operands
    }

    pub fn grant_refusals(&self) -> &[FilesystemGrantRefusal] {
        &self.grant_refusals
    }

    pub fn authorized_paths(&self) -> &[FilesystemAuthorizedPath] {
        &self.authorized_paths
    }

    pub fn logical_handle_inputs(&self) -> &[FilesystemLogicalHandleInput] {
        &self.logical_handle_inputs
    }

    pub const fn logical_handle_output(&self) -> Option<FilesystemLogicalHandleOutput> {
        self.logical_handle_output
    }

    pub fn retired_logical_handles(&self) -> &[FilesystemLogicalHandleIdentity] {
        &self.retired_logical_handles
    }
}

/// Same-program routing custody for one compiler-resolved filesystem boundary.
///
/// This token grants no filesystem access and proves no package admission. It
/// only prevents the checked interpreter from rediscovering a package-owned
/// service through a readable name or source path after Omega has already
/// resolved an exact accepted semantic binding. The token is valid only for
/// the exact [`CheckedTrees`] instance from which it was constructed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FilesystemServiceBinding {
    pub(crate) checked_program_address: usize,
    pub(crate) declaration_symbol: symbols::SymbolHandle,
}

impl FilesystemServiceBinding {
    pub fn from_compiler_resolved_declaration(
        checked: &CheckedTrees,
        declaration_symbol: symbols::SymbolHandle,
    ) -> Result<Self, &'static str> {
        if !declaration_symbol.is_valid()
            || checked
                .typed
                .traits()
                .iter()
                .filter(|definition| {
                    definition.is_boundary && definition.symbol == declaration_symbol
                })
                .count()
                != 1
        {
            return Err("filesystem service binding is not one exact checked boundary declaration");
        }
        Ok(Self {
            checked_program_address: std::ptr::from_ref(checked).addr(),
            declaration_symbol,
        })
    }

    pub(crate) fn declaration_symbol_for(
        self,
        checked: &CheckedTrees,
    ) -> Result<symbols::SymbolHandle, &'static str> {
        if self.checked_program_address != std::ptr::from_ref(checked).addr() {
            return Err("filesystem service binding belongs to a different checked program");
        }
        Ok(self.declaration_symbol)
    }
}

/// How the interpreter serves a program's `Filesystem` capability.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum FilesystemAccess {
    /// The deterministic in-memory filesystem (the default; the differential
    /// oracle). Hermetic: no real disk is ever touched.
    #[default]
    Virtual,
    /// The REAL host filesystem, UNSCOPED: fs ops act on real paths with the
    /// invoking process's full authority. The trust level of `build.rs`; for
    /// build.omg proper, prefer [`FilesystemAccess::RealScoped`] -- the grants
    /// ARE the audit surface (open-work #3's settled design).
    RealUnscoped,
    /// The REAL host filesystem behind PATH GRANTS (build.omg rung 2): reads
    /// must land under a read or write root, writes/creates/removes under a
    /// write root; anything else is refused with EACCES before the OS is
    /// touched. build.omg's shape: read = source tree, write = build dir.
    RealScoped(FsGrants),
    /// The same path authority, plus a compiler-owned resource sponsor shared
    /// across every build-machine evaluation in one package-review session.
    /// The program cannot inspect or enlarge this account.
    RealScopedSponsored {
        grants: FsGrants,
        sponsor: FilesystemSponsor,
    },
    /// Consume compiler-produced bounded events without installing host
    /// filesystem authority. Source observations are record-served; an
    /// admitted Output suffix executes in a fresh virtual namespace. Every
    /// event and lane must match exactly and the record must be exhausted.
    ReplayFilesystem(FilesystemReplay),
}

/// Path grants for [`FilesystemAccess::RealScoped`]. Roots are canonicalized
/// when the run starts (so symlinked spellings of a root work), and every
/// op's path is canonicalized before the prefix check (so `..` traversal and
/// symlinks INSIDE a granted tree that point OUTSIDE it are resolved and
/// refused, not string-matched).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FsGrants {
    /// Trees the program may READ from (open read-only, stat). A write root
    /// implicitly grants read-back -- staging then verifying is the normal
    /// build shape -- so these are the read-ONLY trees.
    pub read_roots: Vec<FilesystemGrantRoot>,
    /// Trees the program may WRITE under: create/truncate/append opens,
    /// remove, create_dir/remove_dir, and BOTH ends of a rename.
    pub write_roots: Vec<FilesystemGrantRoot>,
}

#[cfg(test)]
mod canonical_filesystem_metadata_tests {
    use super::*;

    pub(crate) fn row(
        path: &[u8],
        kind: CanonicalFilesystemMetadataRowKind,
    ) -> CanonicalFilesystemMetadataRow {
        CanonicalFilesystemMetadataRow::new(path.to_vec(), kind)
    }

    #[test]
    pub(crate) fn canonical_metadata_accepts_raw_non_utf8_paths_and_preserves_ordered_rows() {
        let index = CanonicalFilesystemMetadataIndex::version_1(
            [3; 32],
            [
                row(b"raw-\xff", CanonicalFilesystemMetadataRowKind::Directory),
                row(b"a:b", CanonicalFilesystemMetadataRowKind::Directory),
                row(b"", CanonicalFilesystemMetadataRowKind::Directory),
                row(
                    b"raw-\xff/file",
                    CanonicalFilesystemMetadataRowKind::File {
                        executable: false,
                        logical_byte_length: 9,
                    },
                ),
            ],
        )
        .unwrap();

        assert_eq!(index.policy_version(), 1);
        assert_eq!(index.source_content_commitment(), &[3; 32]);
        assert_eq!(
            index
                .rows()
                .map(|row| row.relative_path().to_vec())
                .collect::<Vec<_>>(),
            vec![
                b"".to_vec(),
                b"a:b".to_vec(),
                b"raw-\xff".to_vec(),
                b"raw-\xff/file".to_vec()
            ]
        );
    }

    #[test]
    pub(crate) fn canonical_metadata_rejects_invalid_and_duplicate_paths() {
        for invalid in [
            b"/absolute".as_slice(),
            b"a//b".as_slice(),
            b"a/./b".as_slice(),
            b"a/../b".as_slice(),
            b"a\\b".as_slice(),
            b"a\0b".as_slice(),
        ] {
            assert!(matches!(
                CanonicalFilesystemMetadataIndex::version_1(
                    [0; 32],
                    [
                        row(b"", CanonicalFilesystemMetadataRowKind::Directory),
                        row(invalid, CanonicalFilesystemMetadataRowKind::Directory),
                    ],
                ),
                Err(CanonicalFilesystemMetadataIndexError::InvalidRelativePath(path))
                    if path == invalid
            ));
        }
        assert!(matches!(
            CanonicalFilesystemMetadataIndex::version_1(
                [0; 32],
                [
                    row(b"", CanonicalFilesystemMetadataRowKind::Directory),
                    row(b"", CanonicalFilesystemMetadataRowKind::Directory),
                ],
            ),
            Err(CanonicalFilesystemMetadataIndexError::DuplicateRelativePath(path))
                if path.is_empty()
        ));
    }

    #[test]
    pub(crate) fn canonical_metadata_requires_one_directory_root_and_directory_parent_closure() {
        assert!(matches!(
            CanonicalFilesystemMetadataIndex::version_1([0; 32], []),
            Err(CanonicalFilesystemMetadataIndexError::MissingRootDirectory)
        ));
        assert!(matches!(
            CanonicalFilesystemMetadataIndex::version_1(
                [0; 32],
                [row(
                    b"",
                    CanonicalFilesystemMetadataRowKind::File {
                        executable: false,
                        logical_byte_length: 0,
                    },
                )],
            ),
            Err(CanonicalFilesystemMetadataIndexError::RootIsNotDirectory)
        ));
        assert!(matches!(
            CanonicalFilesystemMetadataIndex::version_1(
                [0; 32],
                [
                    row(b"", CanonicalFilesystemMetadataRowKind::Directory),
                    row(
                        b"missing/leaf",
                        CanonicalFilesystemMetadataRowKind::File {
                            executable: false,
                            logical_byte_length: 0,
                        },
                    ),
                ],
            ),
            Err(CanonicalFilesystemMetadataIndexError::MissingParentDirectory(path))
                if path == b"missing/leaf"
        ));
        assert!(matches!(
            CanonicalFilesystemMetadataIndex::version_1(
                [0; 32],
                [
                    row(b"", CanonicalFilesystemMetadataRowKind::Directory),
                    row(
                        b"file",
                        CanonicalFilesystemMetadataRowKind::File {
                            executable: false,
                            logical_byte_length: 0,
                        },
                    ),
                    row(b"file/child", CanonicalFilesystemMetadataRowKind::Directory),
                ],
            ),
            Err(CanonicalFilesystemMetadataIndexError::ParentIsNotDirectory(path))
                if path == b"file/child"
        ));
    }

    #[test]
    pub(crate) fn canonical_metadata_rejects_lengths_outside_the_stat_domain() {
        assert!(matches!(
            CanonicalFilesystemMetadataIndex::version_1(
                [0; 32],
                [
                    row(b"", CanonicalFilesystemMetadataRowKind::Directory),
                    row(
                        b"huge",
                        CanonicalFilesystemMetadataRowKind::File {
                            executable: false,
                            logical_byte_length: i64::MAX as u64 + 1,
                        },
                    ),
                ],
            ),
            Err(CanonicalFilesystemMetadataIndexError::LogicalByteLengthExceedsI64(path))
                if path == b"huge"
        ));
    }

    #[test]
    pub(crate) fn canonical_metadata_rejects_more_rows_than_the_resolver_can_issue() {
        let rows = std::iter::once(row(b"", CanonicalFilesystemMetadataRowKind::Directory)).chain(
            (0..CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT).map(|index| {
                CanonicalFilesystemMetadataRow::new(
                    format!("entry-{index}").into_bytes(),
                    CanonicalFilesystemMetadataRowKind::File {
                        executable: false,
                        logical_byte_length: 0,
                    },
                )
            }),
        );

        assert_eq!(
            CanonicalFilesystemMetadataIndex::version_1([0; 32], rows),
            Err(CanonicalFilesystemMetadataIndexError::RowLimitExceeded {
                limit: CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT,
                attempted: CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT + 1,
            })
        );
    }
}
