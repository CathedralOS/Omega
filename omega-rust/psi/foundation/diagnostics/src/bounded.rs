//! Bounded presentation for compiler diagnostics at application boundaries.

use crate::{Diagnostic, DiagnosticSeverity};
use std::fmt;

const MAXIMUM_DIAGNOSTICS: usize = 16;
const MAXIMUM_MESSAGE_BYTES: usize = 4_096;

/// One indented, bounded and escaped line per diagnostic, with its resolved
/// location. Operation errors that carry bare diagnostics share this form
/// instead of printing their Rust `Debug` representation.
pub fn write_bounded_diagnostics(
    formatter: &mut fmt::Formatter<'_>,
    diagnostics: &[Diagnostic],
) -> fmt::Result {
    for diagnostic in diagnostics.iter().take(MAXIMUM_DIAGNOSTICS) {
        let severity = match diagnostic.severity {
            DiagnosticSeverity::Error => "error",
            DiagnosticSeverity::Warning => "warning",
        };
        write!(formatter, "\n  {severity}: ")?;
        let bytes = diagnostic.message.as_bytes();
        let shown = bytes.len().min(MAXIMUM_MESSAGE_BYTES);
        // Bound input before allocation; byte escaping stays valid even when a
        // UTF-8 codepoint crosses the truncation boundary. Newlines, terminal
        // controls, and identifier text cannot create extra diagnostic records.
        let escaped = source::display_literal_bytes(&bytes[..shown]);
        formatter.write_str(&escaped[1..escaped.len() - 1])?;
        if shown != bytes.len() {
            write!(
                formatter,
                " [truncated; {} message bytes omitted]",
                bytes.len() - shown
            )?;
        }
        write_location(formatter, diagnostic)?;
    }
    if diagnostics.len() > MAXIMUM_DIAGNOSTICS {
        write!(
            formatter,
            "\n  [truncated; {} additional diagnostics omitted]",
            diagnostics.len() - MAXIMUM_DIAGNOSTICS
        )?;
    }
    Ok(())
}

/// The location continuation line both renderers append after a diagnostic's
/// message: the resolved package, path and line:column when the compilation
/// resolved its own retained map, else the raw unit coordinates. An
/// unresolved span never infers a file path from unit order or rereads
/// mutable source to manufacture line numbers. A diagnostic without a span
/// gets no line, so message lines stay exactly as producers wrote them.
pub(crate) fn write_location(writer: &mut impl fmt::Write, diagnostic: &Diagnostic) -> fmt::Result {
    if let Some(location) = &diagnostic.resolved_location {
        match &location.package {
            Some(package) => write!(
                writer,
                "\n    in package `{package}`, {}:{}:{}",
                location.path.display(),
                location.line,
                location.column
            ),
            None => write!(
                writer,
                "\n    at {}:{}:{}",
                location.path.display(),
                location.line,
                location.column
            ),
        }
    } else if let Some(location) = diagnostic.source_span {
        write!(
            writer,
            "\n    source-unit {}, bytes {}..{}",
            location.source_id.0, location.span.start, location.span.end
        )
    } else {
        Ok(())
    }
}
