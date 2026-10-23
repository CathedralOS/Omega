//! Text rendering for diagnostics.
//!
//! Two shapes live here and nothing else renders diagnostics into text: a
//! batch renders through [`write_diagnostics`] (one `Display` line per
//! diagnostic, newline-separated, no trailing newline) into any writer, and
//! every list a producing site embeds — names or messages — renders through
//! the single ordered [`join_display`] operation. Producers decide wording and
//! ordering inputs; this module only separates them in order.

use std::borrow::Borrow;
use std::fmt;

use crate::Diagnostic;

/// Renders a batch of diagnostics for the command line: `Display` for each
/// diagnostic in iteration order, a newline between them, and none after the
/// last. The command line streams the batch into its own formatter; a caller
/// that needs the text as one string renders a `String` target.
pub fn write_diagnostics(writer: &mut impl fmt::Write, diagnostics: &[Diagnostic]) -> fmt::Result {
    for (index, diagnostic) in diagnostics.iter().enumerate() {
        if index > 0 {
            writer.write_char('\n')?;
        }
        write!(writer, "{diagnostic}")?;
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
    mut render: impl FnMut(&'item T) -> R,
) -> String
where
    I: IntoIterator<Item = &'item T>,
    T: 'item,
    R: Borrow<str>,
{
    values
        .into_iter()
        .map(|value| render(value))
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
