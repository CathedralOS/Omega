#![forbid(unsafe_code)]

//! Loaded-source data and source-coordinate primitives owned by the Psi frontend.

mod source_file;
mod source_id;
mod source_map;
mod source_span;
mod source_text;

pub use source_file::{SourceFile, SourceOrigin, SourcePosition};
pub use source_id::SourceId;
pub use source_map::SourceMap;
pub use source_span::SourceSpan;
pub use source_text::SourceText;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}
