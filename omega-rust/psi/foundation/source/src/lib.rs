#![forbid(unsafe_code)]

//! Loaded-source data and source-coordinate primitives owned by the Psi frontend.

mod source_file;
mod source_id;
mod source_map;
mod source_span;
mod source_text;

pub use source_file::{SourceFile, SourceOrigin, SourcePosition, SourceResolutionStratum};
pub use source_id::SourceId;
pub use source_map::SourceMap;
pub use source_span::SourceSpan;
pub use source_text::SourceText;

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
