use serde::Serialize;
use std::path::{Path, PathBuf};

pub const SOURCE_CLOSURE_SNAPSHOT_SCHEMA: &str = "omega.source-closure-snapshot.v3";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceClosureSnapshotEntry {
    pub source_id: usize,
    pub identity: String,
    pub origin: &'static str,
    pub byte_length: usize,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceClosureSnapshot {
    pub schema: &'static str,
    pub entry_source: String,
    pub selected_target: Option<String>,
    pub native_provider_substitution: bool,
    pub sources: Vec<SourceClosureSnapshotEntry>,
    pub syntax: psi_syntax_trees::SyntaxTreesSnapshot,
}

impl SourceClosureSnapshot {
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn feature_census(&self) -> crate::SourceFeatureCensus {
        crate::census_source_closure(self)
    }
}

/// An immutable source root and the logical identity used in snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceInspectionRoot {
    physical_root: PathBuf,
    logical_root: PathBuf,
}

impl SourceInspectionRoot {
    pub fn new(physical_root: impl Into<PathBuf>, logical_root: impl Into<PathBuf>) -> Self {
        Self {
            physical_root: physical_root.into(),
            logical_root: logical_root.into(),
        }
    }

    pub fn physical_root(&self) -> &Path {
        &self.physical_root
    }

    pub fn logical_root(&self) -> &Path {
        &self.logical_root
    }
}
