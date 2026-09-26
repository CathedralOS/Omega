//! Filesystem authority, canonical metadata, and exact operation observations.
//!
//! This file owns the attempt schema version. `metadata_layout.rs`
//! carries the metadata field layout, `canonical_metadata.rs` the canonical
//! metadata rows and index, `grants.rs` grant roots, refusals, authorized
//! paths and access, `operands.rs` rooted output coordinates,
//! `observations.rs` metadata carrier values,
//! `logical_handles.rs` logical handle identities, inputs and
//! outputs and `operation_attempts.rs` operation attempts, outcomes and
//! service bindings.

#[cfg(test)]
mod canonical_filesystem_metadata_tests;
mod canonical_metadata;
mod grants;
mod logical_handles;
mod metadata_layout;
mod observations;
mod operands;
mod operation_attempts;

pub use canonical_metadata::{
    CANONICAL_FILESYSTEM_METADATA_POLICY_VERSION, CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT,
    CanonicalFilesystemMetadataIndex, CanonicalFilesystemMetadataIndexError,
    CanonicalFilesystemMetadataRow, CanonicalFilesystemMetadataRowKind,
    canonical_filesystem_metadata_path_is_canonical,
};
pub use grants::{
    FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT, FilesystemAccess, FilesystemAuthorizedPath,
    FilesystemGrantAccess, FilesystemGrantRefusal, FilesystemGrantRefusalReason,
    FilesystemGrantRoot, FilesystemGrantRootIdentity, FilesystemObservationProvider, FsGrants,
    filesystem_root_relative_path_is_canonical,
};
pub use logical_handles::{
    FilesystemLogicalHandleIdentity, FilesystemLogicalHandleInput,
    FilesystemLogicalHandleInputResolution, FilesystemLogicalHandleKind,
    FilesystemLogicalHandleOutput, FilesystemLogicalHandleOutputSource,
};
pub use metadata_layout::{
    FilesystemMetadataField, FilesystemMetadataFieldLayout, FilesystemMetadataLayout,
};
pub use observations::{
    FILESYSTEM_METADATA_API_CARRIER_BYTES, FilesystemMetadataObservation,
    FilesystemMetadataObservationKind,
};
pub use operands::FilesystemRootedPathOperandResolution;
pub use operation_attempts::{
    FilesystemEvaluationHaltKind, FilesystemOperationAttempt, FilesystemOperationAttemptOutcome,
    FilesystemOperationResult, FilesystemServiceBinding,
};

/// Schema for filesystem operation-attempt observations.
///
/// Records call-start order, provider, rooted authorizations and refusals,
/// logical-handle lifetimes, rooted output custody, and returned or halted outcomes.
/// Payload buffers and operand snapshots are not retained.
pub const FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION: u32 = 20;
