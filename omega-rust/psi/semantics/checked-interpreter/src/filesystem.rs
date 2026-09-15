//! Filesystem authority, canonical metadata, and exact operation observations.
//!
//! This file owns the attempt schema version. `metadata_layout.rs`
//! carries the metadata field layout, `canonical_metadata.rs` the canonical
//! metadata rows and index, `grants.rs` grant roots, refusals, authorized
//! paths and access, `operands.rs` scalar, byte, path and mutable operands,
//! `observations.rs` returned paths, byte regions and metadata
//! observations, `logical_handles.rs` logical handle identities, inputs and
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
    FilesystemMetadataObservationKind, FilesystemObservedByteRegion,
    FilesystemObservedByteRegionKind, FilesystemReturnedPath, FilesystemReturnedPathCompleteness,
    FilesystemReturnedPathKind,
};
pub use operands::{
    FilesystemByteOperand, FilesystemMutableByteOperand, FilesystemMutableByteOperandResolution,
    FilesystemMutableI64Operand, FilesystemMutableI64OperandResolution, FilesystemPathLikeOperand,
    FilesystemRootedPathOperandResolution, FilesystemScalarOperand, FilesystemScalarOperandValue,
};
pub use operation_attempts::{
    FilesystemEvaluationHaltKind, FilesystemOperationAttempt, FilesystemOperationAttemptOutcome,
    FilesystemOperationResult, FilesystemServiceBinding,
};

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
