/*
We define target-neutral compiler diagnostics (`Diagnostic`, `DiagnosticSeverity`,
`ExceededProvision`, `CoverageForm`, `ResolvedSourceLocation`), the phase
observation trait (`PhaseSnapshot`), and deterministic text renderers
(`write_diagnostics`, `write_bounded_diagnostics`, `comma_join_display`,
`bracketed_display_names`, `join_messages`).
*/

/*
Diagnostic producers across Psi and Omega decide severity, wording, and private
provision bounds, while CLI and outcome surfaces need uniform, bounded, and
escaped presentation without re-reading mutable source files. Carrying structured
`ExceededProvision` and `ResolvedSourceLocation` payloads alongside the message
ensures outcome publication and error boundaries never parse diagnostic text or
infer file paths from source-unit order.
*/

/*
Bounded rendering (`write_bounded_diagnostics`) reports every diagnostic a
failure carries, grouped by cause: the message with quoted names, numbers and
its "while reading ..." chain left out. By default each cause prints its first
diagnostic and how many more share it, so a compilation that refuses hundreds
of machines for a few reasons reads as a few lines. `omega --verbose` turns on
`list_every_diagnostic`, which prints all of them, still grouped. Each message
is escaped and capped at 4,096 bytes before allocation, so untrusted
identifier bytes or control characters cannot forge extra diagnostic lines.
Retained `ExceededProvision` records decode strictly through `CoverageForm`'s
finite resource vocabulary.
*/

#![forbid(unsafe_code)]

use std::borrow::Borrow;
use std::fmt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use source::SourceSpan;

const MAXIMUM_MESSAGE_BYTES: usize = 4_096;

/// Whether this process prints every diagnostic of a failure rather than
/// one per cause. Set once from the command line (`omega --verbose`).
static EVERY_DIAGNOSTIC: AtomicBool = AtomicBool::new(false);

/// Print every diagnostic a failure reports, not only the first of each
/// cause (`write_bounded_diagnostics`). The command line sets this once,
/// before any command runs.
pub fn list_every_diagnostic() {
    EVERY_DIAGNOSTIC.store(true, Ordering::Relaxed);
}

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

pub trait PhaseSnapshot {
    type Snapshot;

    fn snapshot(&self) -> Self::Snapshot;
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub exceeded_provision: Option<ExceededProvision>, // Present when refusal stopped at a named private provision (`Incomplete` outcome).
    pub source_span: Option<SourceSpan>, // Optional authored source location attached when declaration spans survive.
    pub resolved_location: Option<ResolvedSourceLocation>, // Span resolved against the compilation's retained source map while alive.
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct ResolvedSourceLocation {
    pub package: Option<String>,
    pub path: PathBuf,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExceededProvision {
    pub resource: &'static str,
    pub limit: u64,
    pub requested: u64,
    pub coordinate: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, compiler_storage::Record)]
pub enum CoverageForm {
    MachineHeader,
    EntrySignature,
    StateSignature,
    StatementForm,
    CallForm,
    ExpressionForm,
    TransitionForm,
}

impl CoverageForm {
    pub fn from_resource(resource: &str) -> Option<Self> {
        match resource {
            "coverage_machine_headers" => Some(Self::MachineHeader),
            "coverage_entry_signatures" => Some(Self::EntrySignature),
            "coverage_state_signatures" => Some(Self::StateSignature),
            "coverage_statement_forms" => Some(Self::StatementForm),
            "coverage_call_forms" => Some(Self::CallForm),
            "coverage_expression_forms" => Some(Self::ExpressionForm),
            "coverage_transition_forms" => Some(Self::TransitionForm),
            _ => None,
        }
    }

    pub const fn resource(self) -> &'static str {
        match self {
            Self::MachineHeader => "coverage_machine_headers",
            Self::EntrySignature => "coverage_entry_signatures",
            Self::StateSignature => "coverage_state_signatures",
            Self::StatementForm => "coverage_statement_forms",
            Self::CallForm => "coverage_call_forms",
            Self::ExpressionForm => "coverage_expression_forms",
            Self::TransitionForm => "coverage_transition_forms",
        }
    }

    pub const fn exceeded(self, first_byte: u64) -> ExceededProvision {
        ExceededProvision {
            resource: self.resource(),
            limit: 0,
            requested: 1,
            coordinate: first_byte,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum DiagnosticSeverity {
    Error,
    Warning, // Surfaced to the user without failing the build (Decision-12 relaxation for non-proof deadness).
    Note, // A second location of the diagnostic before it (the call an unreachable statement follows); never fails the build.
}

impl Diagnostic {
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Error,
            message: message.into(),
            exceeded_provision: None,
            source_span: None,
            resolved_location: None,
        }
    }

    pub fn incomplete(message: impl Into<String>, provision: ExceededProvision) -> Self {
        Self {
            exceeded_provision: Some(provision),
            ..Self::error(message)
        }
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Warning,
            ..Self::error(message)
        }
    }

    /// A further location of the diagnostic listed just before this one.
    pub fn note(message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Note,
            ..Self::error(message)
        }
    }

    pub fn with_source_span(mut self, source_span: SourceSpan) -> Self {
        self.source_span = Some(source_span);
        self
    }

    pub fn is_error(&self) -> bool {
        matches!(self.severity, DiagnosticSeverity::Error)
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.severity {
            DiagnosticSeverity::Error => write!(formatter, "error: {}", self.message),
            DiagnosticSeverity::Warning => write!(formatter, "warning: {}", self.message),
            DiagnosticSeverity::Note => write!(formatter, "note: {}", self.message),
        }
    }
}

impl compiler_storage::Record for ExceededProvision {
    fn write(
        &self,
        output: &mut compiler_storage::RecordWriter,
    ) -> Result<(), compiler_storage::RecordError> {
        output.record(|output| {
            self.resource.to_owned().write(output)?;
            self.limit.write(output)?;
            self.requested.write(output)?;
            self.coordinate.write(output)
        })
    }
    fn read<Context>(
        input: &mut compiler_storage::RecordReader<'_, Context>,
    ) -> Result<Self, compiler_storage::RecordError> {
        input.record(|input| {
            let resource = String::read(input)?;
            let limit = u64::read(input)?;
            let requested = u64::read(input)?;
            let coordinate = u64::read(input)?;
            let form = CoverageForm::from_resource(&resource).ok_or_else(|| {
                input.error(format!("unknown retained provision resource `{resource}`"))
            })?;
            let recovered = form.exceeded(coordinate);
            if recovered.limit != limit || recovered.requested != requested {
                return Err(
                    input.error("retained coverage provision has inconsistent limit or demand")
                );
            }
            Ok(recovered)
        })
    }
}

/// Every diagnostic of a failure, grouped by cause, the cause with the most
/// diagnostics first (ties in the order they first appear). Each cause prints
/// its first diagnostic and how many more share it; with
/// `list_every_diagnostic` it prints all of them.
pub fn write_bounded_diagnostics(
    formatter: &mut fmt::Formatter<'_>,
    diagnostics: &[Diagnostic],
) -> fmt::Result {
    write_grouped_diagnostics(
        formatter,
        diagnostics,
        EVERY_DIAGNOSTIC.load(Ordering::Relaxed),
    )
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

fn write_one_diagnostic(formatter: &mut impl fmt::Write, diagnostic: &Diagnostic) -> fmt::Result {
    {
        let severity = match diagnostic.severity {
            DiagnosticSeverity::Error => "error",
            DiagnosticSeverity::Warning => "warning",
            DiagnosticSeverity::Note => "note",
        };
        write!(formatter, "\n  {severity}: ")?;
        let bytes = diagnostic.message.as_bytes();
        let shown = bytes.len().min(MAXIMUM_MESSAGE_BYTES);
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
    use crate::{
        Diagnostic, bracketed_display_names, comma_join_display, join_messages, write_diagnostics,
    };

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

    fn grouped(diagnostics: &[Diagnostic], every: bool) -> String {
        let mut text = String::new();
        crate::write_grouped_diagnostics(&mut text, diagnostics, every)
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
