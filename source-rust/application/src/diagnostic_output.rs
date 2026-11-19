//! Group command-line failures by cause, with detail selected for this invocation.
use diagnostics::{Diagnostic, write_one_diagnostic};
use std::fmt;

#[derive(Clone, Copy)]
pub(crate) struct DiagnosticOutput {
    pub verbose: bool,
}

impl DiagnosticOutput {
    fn write(self, formatter: &mut fmt::Formatter<'_>, diagnostics: &[Diagnostic]) -> fmt::Result {
        write_grouped_diagnostics(formatter, diagnostics, self.verbose)
    }

    pub fn build_error(self, error: &orchestration::BuildError) -> impl fmt::Display + '_ {
        use orchestration::BuildError;
        fmt::from_fn(move |formatter| match error {
            BuildError::ProjectCompilation(error) => {
                formatter.write_str("cannot compile admitted project: ")?;
                error.fmt_with_diagnostics(formatter, &|f, rows| self.write(f, rows))
            }
            error => fmt::Display::fmt(error, formatter),
        })
    }

    pub fn package_error(
        self,
        error: &package_manager::PackageCommandError,
    ) -> impl fmt::Display + '_ {
        fmt::from_fn(move |formatter| {
            error.fmt_with_diagnostics(formatter, &|f, rows| self.write(f, rows))
        })
    }
}

fn write_grouped_diagnostics(
    formatter: &mut impl fmt::Write,
    diagnostics: &[Diagnostic],
    every: bool,
) -> fmt::Result {
    // Few causes, many diagnostics: a linear search over the causes seen so
    // far keeps one small vector instead of a map.
    let mut causes: Vec<(String, Vec<usize>)> = Vec::new();
    for (position, diagnostic) in diagnostics.iter().enumerate() {
        let cause = cause_of(&diagnostic.message);
        match causes.iter_mut().find(|(seen, _)| *seen == cause) {
            Some((_, members)) => members.push(position),
            None => causes.push((cause, vec![position])),
        }
    }
    causes.sort_by_key(|(_, members)| std::cmp::Reverse(members.len()));
    for (_, members) in &causes {
        let shown = if every { members.len() } else { 1 };
        for &position in &members[..shown] {
            write_one_diagnostic(formatter, &diagnostics[position])?;
        }
        if !every && members.len() > 1 {
            write!(
                formatter,
                "\n    and {} more with this cause; --verbose lists every one",
                members.len() - 1
            )?;
        }
    }
    Ok(())
}

/// What groups diagnostics of one cause: the message up to the "while
/// reading ..." chain that says where the reader was or the "; ..." advice
/// after it, without its quoted names or numbers. It only orders and counts
/// the report; nothing decides on it.
fn cause_of(message: &str) -> String {
    let end = [" while reading ", "; "]
        .iter()
        .filter_map(|marker| message.find(marker))
        .min()
        .unwrap_or(message.len());
    let message = &message[..end];
    let mut cause = String::with_capacity(message.len());
    let mut quoted = false;
    let mut digits = false;
    for character in message.chars() {
        if character == '`' {
            if !quoted {
                cause.push('`');
            }
            quoted = !quoted;
            digits = false;
            continue;
        }
        if quoted {
            continue;
        }
        if character.is_ascii_digit() {
            if !digits {
                cause.push('#');
            }
            digits = true;
            continue;
        }
        digits = false;
        cause.push(character);
    }
    cause
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grouped(diagnostics: &[Diagnostic], every: bool) -> String {
        let mut text = String::new();
        write_grouped_diagnostics(&mut text, diagnostics, every)
            .expect("a String target accepts every render");
        text
    }

    /// Three machines refused for one reason and one for another: by default
    /// each cause shows its first diagnostic and a count, the largest cause
    /// first; nothing is dropped. The chain and the advice after `; ` do not
    /// split a cause.
    #[test]
    fn each_cause_prints_once_with_how_many_share_it() {
        let diagnostics = [
            Diagnostic::error("target selection 12 while reading the signature of `B`"),
            Diagnostic::error("cannot own a machine `A::one` while reading the body of `A`"),
            Diagnostic::error("cannot own a machine `C::two`; the checker does not support it"),
            Diagnostic::error("cannot own a machine `D::three`"),
        ];
        assert_eq!(
            grouped(&diagnostics, false),
            "\n  error: cannot own a machine `A::one` while reading the body of `A`\
             \n    and 2 more with this cause; --verbose lists every one\
             \n  error: target selection 12 while reading the signature of `B`"
        );
    }

    /// With every diagnostic asked for, all four print, grouped by cause.
    #[test]
    fn every_diagnostic_prints_grouped_by_cause() {
        let diagnostics = [
            Diagnostic::error("cannot own a machine `A::one`"),
            Diagnostic::error("target selection"),
            Diagnostic::error("cannot own a machine `C::two`"),
        ];
        assert_eq!(
            grouped(&diagnostics, true),
            "\n  error: cannot own a machine `A::one`\
             \n  error: cannot own a machine `C::two`\
             \n  error: target selection"
        );
    }

    /// A message cannot forge a line of its own: a newline in it is escaped
    /// however many diagnostics are listed.
    #[test]
    fn a_message_cannot_add_a_line_of_its_own() {
        let text = grouped(&[Diagnostic::error("bad\n  error: forged")], true);
        assert_eq!(text.matches('\n').count(), 1, "{text}");
    }
}
