//! Bounded compiler reasons for ordinary operation errors, never review decisions.

use crate::declarations::PackageKey;
use diagnostics::{Diagnostic, DiagnosticSeverity};
use std::fmt;

const MAXIMUM_DIAGNOSTICS: usize = 16;
const MAXIMUM_MESSAGE_BYTES: usize = 4_096;
const MAXIMUM_PACKAGE_NAME_BYTES: usize = 256;

pub(super) fn render(
    formatter: &mut fmt::Formatter<'_>,
    phase: &str,
    package: &PackageKey,
    diagnostics: &[Diagnostic],
) -> fmt::Result {
    // PackageName admits only ASCII kebab-case, not arbitrary terminal text.
    let name = package.name().as_str();
    let shown_name = name.len().min(MAXIMUM_PACKAGE_NAME_BYTES);
    write!(
        formatter,
        "{phase} failed for package `{0}",
        &name[..shown_name]
    )?;
    if shown_name != name.len() {
        write!(
            formatter,
            "...[{} name bytes omitted]",
            name.len() - shown_name
        )?;
    }
    write!(formatter, "` with {} diagnostic(s)", diagnostics.len())?;
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
        if let Some(location) = diagnostic.source_span {
            // The error API retains no source map. Never infer a file path from
            // unit order or reread mutable source to manufacture line numbers.
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

#[cfg(test)]
mod tests;
