//! Text rendering for diagnostics.
//!
//! Two shapes live here and nothing else renders diagnostics into text: a
//! batch renders through [`write_diagnostics`] (one `Display` line per
//! diagnostic, then its location line when it carries a span,
//! newline-separated, no trailing newline) into any writer, and
//! every list a producing site embeds — names or messages — renders through
//! the single ordered [`join_display`] operation. Producers decide wording and
//! ordering inputs; this module only separates them in order.

use std::borrow::Borrow;
use std::fmt;

use crate::Diagnostic;

/// Renders a batch of diagnostics for the command line: `Display` for each
/// diagnostic in iteration order, followed by the same location line the
/// bounded renderer prints when the diagnostic carries a span, a newline
/// between diagnostics, and none after the last. The command line streams
/// the batch into its own formatter; a caller that needs the text as one
/// string renders a `String` target.
pub fn write_diagnostics(writer: &mut impl fmt::Write, diagnostics: &[Diagnostic]) -> fmt::Result {
    for (index, diagnostic) in diagnostics.iter().enumerate() {
        if index > 0 {
            writer.write_char('\n')?;
        }
        write!(writer, "{diagnostic}")?;
        crate::bounded::write_location(writer, diagnostic)?;
    }

    Ok(())
}

/// The ordered separator operation the three named policies share: renders
/// `values` once each, in iteration order, with `separator` between rendered
/// items and no separator at either end. An empty input renders empty text; a
/// borrowed render borrows the item in place, so no message is copied to be
/// joined.
fn join_display<'item, I, T, R>(
    values: I,
    separator: &str,
    render: impl FnMut(&'item T) -> R,
) -> String
where
    I: IntoIterator<Item = &'item T>,
    T: 'item,
    R: Borrow<str>,
{
    values
        .into_iter()
        .map(render)
        .collect::<Vec<R>>()
        .join(separator)
}

/// Renders diagnostic names comma-separated.
/// Names are consumed by the render callback, once per item, in iterator order;
/// an empty iterator renders empty text.
pub fn comma_join_display<'item, I, T>(
    values: I,
    display_name: impl FnMut(&'item T) -> String,
) -> String
where
    I: IntoIterator<Item = &'item T>,
    T: 'item,
{
    join_display(values, ", ", display_name)
}

/// Renders diagnostic names with brackets around one comma-separated list.
/// Each producer keeps its own iterator and naming policy; names are rendered
/// once per item, in iterator order.
pub fn bracketed_display_names<'item, I, T>(
    values: I,
    display_name: impl FnMut(&'item T) -> String,
) -> String
where
    I: IntoIterator<Item = &'item T>,
    T: 'item,
{
    format!("[{}]", comma_join_display(values, display_name))
}

/// Joins diagnostic messages with semicolons for a refusal that wraps them in
/// one line of its own context. Severity and location stay on the records, and
/// the messages are borrowed, not copied.
pub fn join_messages(diagnostics: &[Diagnostic]) -> String {
    join_display(diagnostics, "; ", |diagnostic| diagnostic.message.as_str())
}

#[cfg(test)]
mod tests {
    use crate::{
        Diagnostic, bracketed_display_names, comma_join_display, join_messages, write_diagnostics,
    };

    /// A caller that needs the batch text as one string renders a `String`
    /// target, exactly as the command line renders its formatter.
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

    /// The command line renders standalone-file refusals through this batch
    /// renderer. A located diagnostic gains the bounded renderer's location
    /// line after its unchanged message; an unlocated one gains nothing.
    #[test]
    fn located_diagnostics_render_their_location_line_after_the_message() {
        let span = source::SourceSpan {
            source_id: source::SourceId(3),
            span: source::Span { start: 7, end: 12 },
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

    #[test]
    fn ordered_lists_render_once_per_item_with_their_separator() {
        let names = ["alpha", "beta"];
        let name = |name: &&str| name.to_string();

        let mut renders = 0;
        let counted = |name: &&str| {
            renders += 1;
            name.to_string()
        };
        assert_eq!(comma_join_display(names.iter(), counted), "alpha, beta");
        assert_eq!(renders, 2);

        assert_eq!(bracketed_display_names(names.iter(), name), "[alpha, beta]");
        assert_eq!(comma_join_display(names.iter().take(0), name), "");
        assert_eq!(bracketed_display_names(names.iter().take(0), name), "[]");

        let diagnostics = [Diagnostic::error("first"), Diagnostic::warning("second")];
        assert_eq!(join_messages(&diagnostics), "first; second");
        assert_eq!(join_messages(&diagnostics[..0]), "");
    }
}
