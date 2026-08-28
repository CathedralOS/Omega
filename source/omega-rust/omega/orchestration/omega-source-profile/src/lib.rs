#![forbid(unsafe_code)]

//! Canonical source-closure snapshots and feature census.
//!
//! The compiler may produce a snapshot, but does not own its schema or the
//! analysis performed over it.

mod catalog;
mod census;
mod snapshot;

pub use catalog::{SOURCE_FEATURE_CATALOG, SOURCE_FEATURE_IDS, SOURCE_RESOURCE_IDS};
pub use census::{
    SOURCE_FEATURE_CENSUS_SCHEMA, SourceFeatureCensus, SourceFeatureCount,
    SourceResourceObservation, census_source_closure,
};
pub use snapshot::{
    PackageSourceClosureCustodySnapshot, SOURCE_CLOSURE_SNAPSHOT_SCHEMA, SourceClosureSnapshot,
    SourceClosureSnapshotEntry, SourceClosureSnapshotFingerprint, SourceInspectionRoot,
};
