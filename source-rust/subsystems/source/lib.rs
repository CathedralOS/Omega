//! Source operations on authored Omega files.
#![forbid(unsafe_code)]

use source_syntax::SourceParseError;
use std::path::Path;

/// Check the named file's syntax without resolving imports.
pub fn parse_file(path: &Path) -> Result<(), String> {
    let bytes = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let diagnostic = |start: usize, end: usize, message: &str| {
        let prefix = &bytes[..start];
        let line = 1 + prefix.iter().filter(|&&byte| byte == b'\n').count();
        let column = 1 + prefix
            .rsplit(|&byte| byte == b'\n')
            .next()
            .unwrap_or_default()
            .len();
        format!(
            "{}:{line}:{column}: {message} (bytes {start}..{end})",
            path.display()
        )
    };
    let text = std::str::from_utf8(&bytes).map_err(|error| {
        let start = error.valid_up_to();
        let end = error
            .error_len()
            .map_or(bytes.len(), |length| start + length);
        diagnostic(start, end, "invalid UTF-8")
    })?;
    source_syntax::parse_text(text)
        .map(|_| ())
        .map_err(|error| {
            let (message, span) = match error {
                SourceParseError::Lex { message, span }
                | SourceParseError::Parse { message, span } => (message, span),
            };
            diagnostic(span.start, span.end, &message)
        })
}
