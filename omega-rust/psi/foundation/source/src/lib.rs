#![forbid(unsafe_code)]

//! Loaded-source data and source-coordinate primitives owned by the Psi frontend.
//!
//! `SourceFile` holds what was loaded and where it came from;
//! `SourceId` and `SourceSpan` are the coordinates every token and diagnostic
//! carries; start at `source_map.rs`, where `SourceMap` resolves them back to
//! files and positions. Past resolution, source text is diagnostic and debug
//! payload, never identity.
//!
//! `captured_source_tree.rs` retains immutable source bytes and namespace, and
//! `source_file_system.rs` routes compiler reads through captured roots without
//! reopening absent members. `source_snapshot.rs` supplies exact non-following inventory queries to
//! consumers independently of compiler-following lookup. Acquisition, admission,
//! Build and materialization
//! stay with their respective consumers; these models carry no authority.
//!
//! `read_accounting.rs` counts how many times the host was read for one source
//! file's bytes, across every crate that reads them. Loading each consumed file
//! once is the contract; the counter is how a second read becomes visible.

mod captured_source_tree;
mod loaded_sources;
pub mod read_accounting;
mod source_bytes;
mod source_file;
mod source_file_system;
pub mod source_identity;
mod source_map;
mod source_snapshot;
pub use source_bytes::SourceBytes;

pub use captured_source_tree::{CapturedSourceNode, CapturedSourceTree};
pub use loaded_sources::{LoadedSource, LoadedSources, SourceContents};
pub use source_file::{
    DependencyScope, SourceFile, SourceOrigin, SourcePosition, SourceResolutionStratum,
};
pub use source_file_system::SourceFileSystem;
pub use source_map::SourceMap;
pub use source_snapshot::{
    SourceSnapshot, SourceSnapshotEntry, SourceSnapshotError, SourceSnapshotMetadata,
    SourceSnapshotStorage, source_snapshot_path_is_canonical,
};

/// Render exact literal bytes without assuming UTF-8. This is diagnostic text,
/// never a semantic decoding step.
pub fn display_literal_bytes(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len().saturating_mul(4).saturating_add(2));
    output.push('"');
    for byte in bytes {
        match byte {
            b'\\' => output.push_str("\\\\"),
            b'"' => output.push_str("\\\""),
            b'\n' => output.push_str("\\n"),
            b'\r' => output.push_str("\\r"),
            b'\t' => output.push_str("\\t"),
            0x20..=0x7e => output.push(char::from(*byte)),
            _ => output.push_str(&format!("\\x{byte:02x}")),
        }
    }
    output.push('"');
    output
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

/// Identity of one loaded source within a `SourceMap`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct SourceId(pub usize);

/// A span inside one loaded source: the coordinate every token and diagnostic
/// carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SourceSpan {
    pub source_id: SourceId,
    pub span: Span,
}

impl SourceSpan {
    pub fn new(source_id: SourceId, span: Span) -> Self {
        Self { source_id, span }
    }
}
