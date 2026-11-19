//! Text forms used by subsystem errors and callers that display individual diagnostics.
use crate::{Diagnostic, DiagnosticSeverity};
use std::fmt;

const MAXIMUM_MESSAGE_BYTES: usize = 4_096;

pub type WriteDiagnosticList<'a> =
    dyn Fn(&mut fmt::Formatter<'_>, &[Diagnostic]) -> fmt::Result + 'a;

pub fn write_bounded_diagnostics(
    formatter: &mut fmt::Formatter<'_>,
    diagnostics: &[Diagnostic],
) -> fmt::Result {
    for diagnostic in diagnostics {
        write_one_diagnostic(formatter, diagnostic)?;
    }
    Ok(())
}

/// Escape message bytes and cap their length before writing to a terminal or report.
pub fn write_one_diagnostic(
    formatter: &mut impl fmt::Write,
    diagnostic: &Diagnostic,
) -> fmt::Result {
    {
        let severity = match diagnostic.severity {
            DiagnosticSeverity::Error => "error",
            DiagnosticSeverity::Warning => "warning",
            DiagnosticSeverity::Note => "note",
        };
        write!(formatter, "\n  {severity}: ")?;
        let bytes = diagnostic.message.as_bytes();
        let shown = bytes.len().min(MAXIMUM_MESSAGE_BYTES);
        let escaped = source_text::display_literal_bytes(&bytes[..shown]);
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
    Ok(())
}

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

pub fn write_diagnostics(writer: &mut impl fmt::Write, diagnostics: &[Diagnostic]) -> fmt::Result {
    for (index, diagnostic) in diagnostics.iter().enumerate() {
        if index > 0 {
            writer.write_char('\n')?;
        }
        write!(writer, "{diagnostic}")?;
        write_location(writer, diagnostic)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(diagnostics: &[Diagnostic]) -> String {
        let mut text = String::new();
        write_diagnostics(&mut text, diagnostics).expect("a String target accepts every render");
        text
    }

    #[test]
    fn formats_diagnostics_without_trailing_newline() {
        let diagnostics = [
            Diagnostic::error("first problem"),
            Diagnostic::error("second problem"),
        ];

        assert_eq!(
            render(&diagnostics),
            "error: first problem\nerror: second problem"
        );
    }

    #[test]
    fn located_diagnostics_render_their_location_line_after_the_message() {
        let span = source_text::SourceSpan {
            source_id: source_text::SourceId(3),
            span: source_text::Span { start: 7, end: 12 },
        };
        let mut resolved = Diagnostic::error("resolved problem").with_source_span(span);
        resolved.resolved_location = Some(crate::ResolvedSourceLocation {
            package: None,
            path: "main.omg".into(),
            line: 2,
            column: 18,
        });
        let mut packaged = resolved.clone();
        packaged.message = "packaged problem".into();
        packaged.resolved_location.as_mut().unwrap().package = Some("app".into());
        let diagnostics = [
            resolved,
            packaged,
            Diagnostic::error("unresolved problem").with_source_span(span),
            Diagnostic::error("unlocated problem"),
        ];

        assert_eq!(
            render(&diagnostics),
            "error: resolved problem\n    at main.omg:2:18\n\
             error: packaged problem\n    in package `app`, main.omg:2:18\n\
             error: unresolved problem\n    source-unit 3, bytes 7..12\n\
             error: unlocated problem"
        );
    }
}
