use crate::Diagnostic;
use std::borrow::Borrow;

#[macro_export]
macro_rules! debug_display {
    ($type:ty) => {
        impl std::fmt::Display for $type {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Debug::fmt(self, formatter)
            }
        }
    };
}

#[macro_export]
macro_rules! diagnostic_display {
    ($type:ty) => {
        impl std::fmt::Display for $type {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.diagnostic.fmt(formatter)
            }
        }
    };
}

#[macro_export]
macro_rules! string_diagnostic {
    ($type:ty) => {
        impl std::fmt::Display for $type {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl std::error::Error for $type {}
    };
}

#[macro_export]
macro_rules! failed_display {
    ($type:ty, $message:literal) => {
        impl std::fmt::Display for $type {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(formatter, concat!($message, ": {:?}"), self)
            }
        }
    };
}

pub fn error_in_context(context: &str, message: impl std::fmt::Display) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!("{context}: {message}"))]
}

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

pub fn join_messages(diagnostics: &[Diagnostic]) -> String {
    join_display(diagnostics, "; ", |diagnostic| diagnostic.message.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

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
