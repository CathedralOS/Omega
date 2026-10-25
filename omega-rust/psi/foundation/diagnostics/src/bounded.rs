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
        if let Some(location) = &diagnostic.resolved_location {
            // The compilation resolved its own retained map: the failing
            // package's name and the staged file's line:column. An
            // unresolved span still prints only its unit coordinates --
            // never infer a file path from unit order or reread mutable
            // source to manufacture line numbers.
            match &location.package {
                Some(package) => write!(
                    formatter,
                    "\n    in package `{package}`, {}:{}:{}",
                    location.path.display(),
                    location.line,
                    location.column
                )?,
                None => write!(
                    formatter,
                    "\n    at {}:{}:{}",
                    location.path.display(),
                    location.line,
                    location.column
                )?,
            }
        } else if let Some(location) = diagnostic.source_span {
            write!(
                formatter,
                "\n    source-unit {}, bytes {}..{}",
                location.source_id.0, location.span.start, location.span.end
            )?;
        }
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
