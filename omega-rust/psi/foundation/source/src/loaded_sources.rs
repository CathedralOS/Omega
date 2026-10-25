//! Captured text and admitted source coordinates at the parsing boundary.
//!
//! Physical reads produce `SourceContents`. The source-map owner assigns exact
//! package/toolchain custody before supplying `LoadedSource` to parsing. Inputs
//! may also be retained in memory; their coordinates, origin and dependency
//! scope travel unchanged through lexing and parsing.

use crate::{DependencyScope, SourceId, SourceOrigin};
use arena::{Arena, HandleSpan};
use std::path::PathBuf;
use std::sync::Arc;

/// Captured file contents before the package owner assigns checked custody and
/// source coordinates. Reading bytes does not create a checked source instance.
#[derive(Debug)]
pub struct SourceContents {
    pub path: PathBuf,
    pub source: Arc<str>,
    pub scope: DependencyScope,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedSource {
    pub source_id: SourceId,
    pub path: PathBuf,
    pub source: Arc<str>,
    pub origin: Option<SourceOrigin>,
    /// The checked instance this load produces. The same path may appear in
    /// one frontier once per scope.
    pub scope: DependencyScope,
}

impl Default for LoadedSource {
    fn default() -> Self {
        Self {
            source_id: SourceId::default(),
            path: PathBuf::default(),
            source: Arc::from(""),
            origin: None,
            scope: DependencyScope::Product,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LoadedSources {
    pub sources: Arena<LoadedSource>,
    pub batch: HandleSpan<LoadedSource>,
}

impl FromIterator<LoadedSource> for LoadedSources {
    fn from_iter<Inputs: IntoIterator<Item = LoadedSource>>(inputs: Inputs) -> Self {
        let inputs = inputs.into_iter();
        let mut sources = Arena::with_capacity(inputs.size_hint().0);
        let batch = sources.insert_many(inputs);
        Self { sources, batch }
    }
}
