//! Byte fields in a diagnostic-kind receipt. Never interpret fixture bytes as text.
use std::fmt::Write;

fn bytes_field(bytes: &[u8]) -> String {
    let mut field = String::with_capacity(bytes.len());
    for &byte in bytes {
        if (b' '..=b'~').contains(&byte) && byte != b'%' {
            field.push(char::from(byte));
        } else {
            write!(field, "%{byte:02X}").unwrap();
        }
    }
    field
}

/// The caller selects `start` and `spelling` from its golden expectation, not the CLI span.
pub(super) fn fields(
    file: &str,
    source: &[u8],
    start: usize,
    spelling: &[u8],
) -> String {
    format!(
        "{}\t{}\t{start}\t{}",
        spec_corpus::diagnostic_field(file),
        bytes_field(source),
        bytes_field(spelling)
    )
}
