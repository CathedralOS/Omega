//! Loaded text and source coordinates before source-map custody is assigned.
//!
//! These inputs can be physical or supplied in memory. An origin override and
//! dependency scope travel unchanged through lexing and parsing; the source-map
//! owner still assigns package and toolchain custody when incorporating them.

use crate::{DependencyScope, SourceId, SourceOrigin};
use arena::{Arena, HandleSpan};
use std::path::PathBuf;
use std::sync::Arc;

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
