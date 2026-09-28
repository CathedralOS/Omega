//! Retained token streams with their exact loaded-source coordinates.
//!
//! Each row owns the source backing its shared lexemes. Scope distinguishes
//! separate Build and Product instances even when their paths and bytes agree.

use crate::TokenStream;
use arena::{Arena, HandleSpan};
use source::{DependencyScope, SourceId, SourceOrigin};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LexedSource {
    pub source_id: SourceId,
    pub path: PathBuf,
    pub source: Arc<str>,
    pub origin: Option<SourceOrigin>,
    pub tokens: TokenStream<'static>,
    pub scope: DependencyScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LexedSources {
    pub sources: Arena<LexedSource>,
    pub batch: HandleSpan<LexedSource>,
}
