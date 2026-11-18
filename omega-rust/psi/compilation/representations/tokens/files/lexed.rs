use crate::TokenStream;
use arena::{Arena, HandleSpan};
use source::{DependencyScope, SourceId, SourceOrigin};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LexedSource {
    pub source_id: SourceId,
    pub path: PathBuf,
    pub origin: Option<SourceOrigin>,
    pub tokens: TokenStream<'static>,
    pub scope: DependencyScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LexedSources {
    pub sources: Arena<LexedSource>,
    pub batch: HandleSpan<LexedSource>,
}
