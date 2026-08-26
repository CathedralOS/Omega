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
