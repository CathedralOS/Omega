//! The source files supplied to the lexer. The caller has captured the bytes,
//! assigned identifiers and selected the origin and scope of each occurrence.
#![forbid(unsafe_code)]

use arena::{Arena, HandleSpan};
use source_text::SourceId;
use std::path::PathBuf;
use std::sync::Arc;

pub use source_provenance::DependencyScope;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, record_storage::Record)]
pub enum SourceOrigin {
    #[default]
    User,
    Toolchain,
    // Compiler-issued contract sources retain custody without joining the core prelude.
    ToolchainContract,
}

impl SourceOrigin {
    /// Provenance only: closed contracts do not gain ambient name visibility.
    pub const fn is_toolchain(self) -> bool {
        matches!(self, Self::Toolchain | Self::ToolchainContract)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedSource {
    pub source_id: SourceId,
    pub path: PathBuf,
    pub source: Arc<str>,
    pub origin: Option<SourceOrigin>,
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
