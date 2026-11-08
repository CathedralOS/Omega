/*
We define canonical source-tree metadata indices, scoped grant roots and
refusal records, logical handle identities, target `StatRecord` layouts, and
the wire codec for retained filesystem observations. Every decoded record
re-validates nonzero handle identities and closed enum tags so corrupt
package-review bytes fail decode rather than fabricating evaluator evidence.
*/

macro_rules! filesystem_record {
    (
        $(#[$record_meta:meta])*
        $record_vis:vis struct $record:ident {
            $($field_vis:vis $field:ident: $field_ty:ty,)+
        }
    ) => {
        $(#[$record_meta])*
        $record_vis struct $record {
            $($field_vis $field: $field_ty,)+
        }

        impl $record {
            pub const fn new($($field: $field_ty),+) -> Self {
                Self { $($field),+ }
            }

            $(
                pub const fn $field(self) -> $field_ty {
                    self.$field
                }
            )+
        }
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemOperationResult {
    Scalar(i64),
    LogicalHandle(FilesystemLogicalHandleIdentity),
}

pub const CANONICAL_FILESYSTEM_METADATA_POLICY_VERSION: u32 = 1;
pub const CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT: usize = 262_145;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
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

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
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

    pub fn row(&self, relative_path: &[u8]) -> Option<CanonicalFilesystemMetadataRowKind> {
        self.rows.get(relative_path).copied()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemObservationProvider {
    Virtual,
    RealUnscoped,
    RealScoped,
    Captured,
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

filesystem_record!(
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FilesystemGrantRefusal {
        pub(crate) operand_ordinal: u8,
        pub(crate) access: FilesystemGrantAccess,
        pub(crate) reason: FilesystemGrantRefusalReason,
    }
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FilesystemGrantRootIdentity(u32);

pub const FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT: usize = 16 * 1024 * 1024;

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

impl FilesystemGrantRootIdentity {
    pub const fn new(value: u32) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemAuthorizedPath {
    pub(crate) operand_ordinal: u8,
    pub(crate) access: FilesystemGrantAccess,
    pub(crate) root: FilesystemGrantRootIdentity,
    pub(crate) relative_path: Vec<u8>,
}

impl FilesystemAuthorizedPath {
    pub fn new(
        operand_ordinal: u8,
        access: FilesystemGrantAccess,
        root: FilesystemGrantRootIdentity,
        relative_path: Vec<u8>,
    ) -> Self {
        Self {
            operand_ordinal,
            access,
            root,
            relative_path,
        }
    }

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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FsGrants {
    pub read_roots: Vec<FilesystemGrantRoot>, // Trees granted for read-only access.
    pub write_roots: Vec<FilesystemGrantRoot>, // Trees granted for read/write access.
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FilesystemLogicalHandleIdentity(u64);

impl FilesystemLogicalHandleIdentity {
    #[doc(hidden)]
    pub const fn new(value: u64) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemLogicalHandleKind {
    Descriptor,
    Native,
    Find,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemLogicalHandleInputResolution {
    Resolved(FilesystemLogicalHandleIdentity),
    Null,
    Unknown,
}

filesystem_record!(
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FilesystemLogicalHandleInput {
        pub(crate) operand_ordinal: u8,
        pub(crate) kind: FilesystemLogicalHandleKind,
        pub(crate) resolution: FilesystemLogicalHandleInputResolution,
    }
);

impl FilesystemLogicalHandleInput {
    pub const fn is_unknown_native(self) -> bool {
        matches!(
            (self.kind, self.resolution),
            (
                FilesystemLogicalHandleKind::Native,
                FilesystemLogicalHandleInputResolution::Unknown
            )
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemLogicalHandleOutputSource {
    Created,
    Duplicated(FilesystemLogicalHandleIdentity),
    Borrowed(FilesystemLogicalHandleIdentity),
}

filesystem_record!(
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FilesystemLogicalHandleOutput {
        pub(crate) kind: FilesystemLogicalHandleKind,
        pub(crate) identity: FilesystemLogicalHandleIdentity,
        pub(crate) source: FilesystemLogicalHandleOutputSource,
    }
);

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

filesystem_record!(
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FilesystemMetadataFieldLayout {
        pub(crate) field: FilesystemMetadataField,
        pub(crate) offset: usize,
        pub(crate) stored_width_bits: u16,
    }
);

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
        } else if cfg!(target_os = "macos") {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemRootedPathOperandResolution {
    pub(crate) operand_ordinal: u8,
    pub(crate) root: FilesystemGrantRootIdentity,
    pub(crate) relative_path: Vec<u8>,
}

impl FilesystemRootedPathOperandResolution {
    pub fn new(
        operand_ordinal: u8,
        root: FilesystemGrantRootIdentity,
        relative_path: Vec<u8>,
    ) -> Self {
        Self {
            operand_ordinal,
            root,
            relative_path,
        }
    }

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

pub mod observation_codec {
    use super::{
        FilesystemGrantAccess, FilesystemGrantRefusal, FilesystemGrantRefusalReason,
        FilesystemLogicalHandleIdentity, FilesystemLogicalHandleInput,
        FilesystemLogicalHandleInputResolution, FilesystemLogicalHandleKind,
        FilesystemLogicalHandleOutput, FilesystemLogicalHandleOutputSource,
        FilesystemObservationProvider, FilesystemOperationResult,
    };

    pub struct ObservationCursor<'a> {
        bytes: &'a [u8],
        offset: usize,
    }

    impl<'a> ObservationCursor<'a> {
        pub fn new(bytes: &'a [u8]) -> Self {
            Self { bytes, offset: 0 }
        }

        pub fn exhausted(&self) -> bool {
            self.offset == self.bytes.len()
        }

        pub fn u8(&mut self) -> Option<u8> {
            let byte = *self.bytes.get(self.offset)?;
            self.offset += 1;
            Some(byte)
        }

        pub fn u16(&mut self) -> Option<u16> {
            let bytes = self.take(2)?;
            Some(u16::from_le_bytes([bytes[0], bytes[1]]))
        }

        pub fn u32(&mut self) -> Option<u32> {
            let bytes = self.take(4)?;
            Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        }

        pub fn u64(&mut self) -> Option<u64> {
            let bytes = self.take(8)?;
            Some(u64::from_le_bytes(bytes.try_into().ok()?))
        }

        pub fn i32(&mut self) -> Option<i32> {
            Some(self.u32()? as i32)
        }

        pub fn i64(&mut self) -> Option<i64> {
            Some(self.u64()? as i64)
        }

        pub fn bytes(&mut self, length: usize) -> Option<&'a [u8]> {
            self.take(length)
        }

        pub fn framed(&mut self) -> Option<&'a [u8]> {
            let length = self.u64()? as usize;
            self.take(length)
        }

        fn take(&mut self, length: usize) -> Option<&'a [u8]> {
            let end = self.offset.checked_add(length)?;
            let slice = self.bytes.get(self.offset..end)?;
            self.offset = end;
            Some(slice)
        }
    }

    pub fn push_framed(out: &mut Vec<u8>, bytes: &[u8]) {
        crate::codec_bytes::push_encoded(out, (bytes.len() as u64).to_le_bytes());
        out.extend_from_slice(bytes);
    }

    pub fn encode_observation_provider(provider: FilesystemObservationProvider) -> u8 {
        match provider {
            FilesystemObservationProvider::Virtual => 0,
            FilesystemObservationProvider::RealUnscoped => 1,
            FilesystemObservationProvider::RealScoped => 2,
            FilesystemObservationProvider::Captured => 3,
        }
    }

    pub fn recover_observation_provider(tag: u8) -> Option<FilesystemObservationProvider> {
        match tag {
            0 => Some(FilesystemObservationProvider::Virtual),
            1 => Some(FilesystemObservationProvider::RealUnscoped),
            2 => Some(FilesystemObservationProvider::RealScoped),
            3 => Some(FilesystemObservationProvider::Captured),
            _ => None,
        }
    }

    pub fn encode_operation_result(result: FilesystemOperationResult, out: &mut Vec<u8>) {
        match result {
            FilesystemOperationResult::Scalar(value) => {
                out.push(0);
                crate::codec_bytes::push_encoded(out, value.to_le_bytes());
            }
            FilesystemOperationResult::LogicalHandle(identity) => {
                out.push(1);
                crate::codec_bytes::push_encoded(out, identity.get().to_le_bytes());
            }
        }
    }

    pub fn recover_operation_result(
        cursor: &mut ObservationCursor<'_>,
    ) -> Option<FilesystemOperationResult> {
        match cursor.u8()? {
            0 => Some(FilesystemOperationResult::Scalar(cursor.i64()?)),
            1 => Some(FilesystemOperationResult::LogicalHandle(
                FilesystemLogicalHandleIdentity::new(cursor.u64()?)?,
            )),
            _ => None,
        }
    }

    pub fn encode_logical_handle_kind(kind: FilesystemLogicalHandleKind) -> u8 {
        match kind {
            FilesystemLogicalHandleKind::Descriptor => 0,
            FilesystemLogicalHandleKind::Native => 1,
            FilesystemLogicalHandleKind::Find => 2,
        }
    }

    pub fn recover_logical_handle_kind(tag: u8) -> Option<FilesystemLogicalHandleKind> {
        match tag {
            0 => Some(FilesystemLogicalHandleKind::Descriptor),
            1 => Some(FilesystemLogicalHandleKind::Native),
            2 => Some(FilesystemLogicalHandleKind::Find),
            _ => None,
        }
    }

    pub fn encode_logical_handle_input(input: FilesystemLogicalHandleInput, out: &mut Vec<u8>) {
        out.push(input.operand_ordinal());
        out.push(encode_logical_handle_kind(input.kind()));
        match input.resolution() {
            FilesystemLogicalHandleInputResolution::Resolved(identity) => {
                out.push(0);
                crate::codec_bytes::push_encoded(out, identity.get().to_le_bytes());
            }
            FilesystemLogicalHandleInputResolution::Null => out.push(1),
            FilesystemLogicalHandleInputResolution::Unknown => out.push(2),
        }
    }

    pub fn recover_logical_handle_input(
        cursor: &mut ObservationCursor<'_>,
    ) -> Option<FilesystemLogicalHandleInput> {
        let operand_ordinal = cursor.u8()?;
        let kind = recover_logical_handle_kind(cursor.u8()?)?;
        let resolution = match cursor.u8()? {
            0 => FilesystemLogicalHandleInputResolution::Resolved(
                FilesystemLogicalHandleIdentity::new(cursor.u64()?)?,
            ),
            1 => FilesystemLogicalHandleInputResolution::Null,
            2 => FilesystemLogicalHandleInputResolution::Unknown,
            _ => return None,
        };
        Some(FilesystemLogicalHandleInput {
            operand_ordinal,
            kind,
            resolution,
        })
    }

    pub fn encode_logical_handle_output(output: FilesystemLogicalHandleOutput, out: &mut Vec<u8>) {
        out.push(encode_logical_handle_kind(output.kind()));
        crate::codec_bytes::push_encoded(out, output.identity().get().to_le_bytes());
        match output.source() {
            FilesystemLogicalHandleOutputSource::Created => out.push(0),
            FilesystemLogicalHandleOutputSource::Duplicated(identity) => {
                out.push(1);
                crate::codec_bytes::push_encoded(out, identity.get().to_le_bytes());
            }
            FilesystemLogicalHandleOutputSource::Borrowed(identity) => {
                out.push(2);
                crate::codec_bytes::push_encoded(out, identity.get().to_le_bytes());
            }
        }
    }

    pub fn recover_logical_handle_output(
        cursor: &mut ObservationCursor<'_>,
    ) -> Option<FilesystemLogicalHandleOutput> {
        let kind = recover_logical_handle_kind(cursor.u8()?)?;
        let identity = FilesystemLogicalHandleIdentity::new(cursor.u64()?)?;
        let source = match cursor.u8()? {
            0 => FilesystemLogicalHandleOutputSource::Created,
            1 => FilesystemLogicalHandleOutputSource::Duplicated(
                FilesystemLogicalHandleIdentity::new(cursor.u64()?)?,
            ),
            2 => FilesystemLogicalHandleOutputSource::Borrowed(
                FilesystemLogicalHandleIdentity::new(cursor.u64()?)?,
            ),
            _ => return None,
        };
        Some(FilesystemLogicalHandleOutput {
            kind,
            identity,
            source,
        })
    }

    pub fn encode_grant_access(access: FilesystemGrantAccess) -> u8 {
        match access {
            FilesystemGrantAccess::Read => 0,
            FilesystemGrantAccess::Write => 1,
        }
    }

    pub fn recover_grant_access(tag: u8) -> Option<FilesystemGrantAccess> {
        match tag {
            0 => Some(FilesystemGrantAccess::Read),
            1 => Some(FilesystemGrantAccess::Write),
            _ => None,
        }
    }

    pub fn encode_grant_refusal(refusal: &FilesystemGrantRefusal, out: &mut Vec<u8>) {
        out.push(refusal.operand_ordinal());
        out.push(encode_grant_access(refusal.access()));
        out.push(match refusal.reason() {
            FilesystemGrantRefusalReason::Unresolvable => 0,
            FilesystemGrantRefusalReason::OutsideGrantedRoots => 1,
            FilesystemGrantRefusalReason::UnrepresentableRootedPath => 2,
            FilesystemGrantRefusalReason::ObservationEvidenceLimitExceeded => 3,
        });
    }

    pub fn recover_grant_refusal(
        cursor: &mut ObservationCursor<'_>,
    ) -> Option<FilesystemGrantRefusal> {
        let operand_ordinal = cursor.u8()?;
        let access = recover_grant_access(cursor.u8()?)?;
        let reason = match cursor.u8()? {
            0 => FilesystemGrantRefusalReason::Unresolvable,
            1 => FilesystemGrantRefusalReason::OutsideGrantedRoots,
            2 => FilesystemGrantRefusalReason::UnrepresentableRootedPath,
            3 => FilesystemGrantRefusalReason::ObservationEvidenceLimitExceeded,
            _ => return None,
        };
        Some(FilesystemGrantRefusal {
            operand_ordinal,
            access,
            reason,
        })
    }
}
