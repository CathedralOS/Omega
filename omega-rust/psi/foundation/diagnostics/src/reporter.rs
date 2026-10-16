use crate::Diagnostic;

pub fn format_diagnostics(diagnostics: &[Diagnostic]) -> String {
    let mut output = String::new();

    for (index, diagnostic) in diagnostics.iter().enumerate() {
        if index > 0 {
            output.push('\n');
        }

        output.push_str(&diagnostic.to_string());
    }

    output
}

/// Joins diagnostic messages with semicolons for a refusal that wraps them in
/// one line of its own context. Severity and location stay on the records.
pub fn join_messages(diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("; ")
}

/// Renders diagnostic names with comma-space separators.
/// Names are consumed once per item, in iterator order; an empty iterator renders empty text.
pub fn comma_join_display<'item, I, T>(
    values: I,
    mut display_name: impl FnMut(&'item T) -> String,
) -> String
where
    I: IntoIterator<Item = &'item T>,
    T: 'item,
{
    let mut output = String::new();
    let mut first = true;

    for value in values {
        if first {
            first = false;
        } else {
            output.push_str(", ");
        }

        output.push_str(&display_name(value));
    }

    output
}

/// Renders diagnostic names with brackets and comma-space separators.
/// Each producer keeps its own iterator and naming policy; names are consumed
/// immediately, once per item, in iterator order.
pub fn bracketed_display_names<'item, I, T>(
    values: I,
    mut display_name: impl FnMut(&'item T) -> String,
) -> String
where
    I: IntoIterator<Item = &'item T>,
    T: 'item,
{
    let mut output = String::from("[");
    let mut first = true;

    for value in values {
        if first {
            first = false;
        } else {
            output.push_str(", ");
        }

        output.push_str(&display_name(value));
    }

    output.push(']');
    output
}

#[cfg(test)]
mod tests {
    use crate::{Diagnostic, format_diagnostics};

    #[test]
    fn formats_diagnostics_without_trailing_newline() {
        let diagnostics = [
            Diagnostic::error("first problem"),
            Diagnostic::error("second problem"),
        ];

        assert_eq!(
            format_diagnostics(&diagnostics),
            "error: first problem\nerror: second problem"
        );
    }
}
