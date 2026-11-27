//! Grant roots, logical handles and target metadata layouts for Build operation evidence.

use crate::{CanonicalFilesystemMetadataIndex, FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT};

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
