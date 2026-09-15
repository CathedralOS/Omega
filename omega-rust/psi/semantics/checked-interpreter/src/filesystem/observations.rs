//! Returned paths, observed byte regions and metadata observations.

use crate::filesystem::FilesystemMetadataField;

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
