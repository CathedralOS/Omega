//! Local-source capture, immutable publication, and verification.

pub(super) mod capture;
mod model;
mod operations;
mod snapshot;

pub use model::{ResolvedLocalSnapshot, ResolvedLocalSource};
pub(crate) use model::{VerifiedPackageSourceEntry, VerifiedPackageSourceEntryKind};
#[cfg(test)]
pub(crate) use operations::resolve_local_source_snapshot_at_path;
pub(crate) use operations::resolve_local_source_snapshot_in_lane;
pub(crate) use operations::{
    capture_verified_package_source_snapshot, verify_package_source_snapshot,
};
pub use operations::{resolve_local_source, resolve_local_source_snapshot_with_storage};

#[cfg(any(test, not(unix)))]
#[allow(unused_imports)] // Preserve the former platform-dependent facade.
pub(in crate::source::acquisition) use capture::is_executable;
#[cfg(test)]
pub(in crate::source::acquisition) use capture::resolve_materialized_source;
#[allow(unused_imports)] // Preserve the former source-internal local facade explicitly.
pub(in crate::source::acquisition) use capture::{
    CapturedLocalEntry, CapturedLocalEntryKind, CapturedLocalTree, SourceIdentityHasher,
    SourceTreePolicy, capture_local_source, capture_local_source_from_open_root, hash_bytes,
    hash_length, io_error, open_absolute_directory_nofollow, open_canonical_source_root,
    open_captured_directory, raw_os_bytes, read_capability_file_bounded,
};
#[allow(unused_imports)] // Used by the source regression facade on supported targets.
pub(in crate::source::acquisition) use snapshot::local_snapshot_custody_identity;
#[cfg(test)]
pub(in crate::source::acquisition) use snapshot::publish_local_snapshot;
