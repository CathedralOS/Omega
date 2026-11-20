/*
    Pulls every `omega` code example out of the specification's Markdown.

    The spec has no formal grammar; most of what it says about syntax it says
    by example. Those examples are therefore the closest thing we have to a
    conformance corpus, and every layer runs over all of them.

    We read the Markdown at test time and keep nothing on disk. The spec is
    about 2 MB across 104 files, so a full extraction costs a few milliseconds,
    and a corpus that is re-read on every run cannot drift from the spec the
    way a checked-in copy could.

    Recognition is deliberately simple: a line whose trimmed text starts with
    ```omega opens an example, and the next line whose trimmed text starts with
    ``` closes it. The lines between are kept exactly as written, indentation
    included, joined with line feeds.

    @Incomplete: examples the spec presents as deliberately invalid are not
    tagged yet. Nothing in the Markdown marks them mechanically, so a layer
    that refuses one has to be checked against the surrounding prose by hand.
*/

#![forbid(unsafe_code)]

/// Keep diagnostic text intact inside one tab-separated test record.
pub fn diagnostic_field(text: &str) -> String {
    use std::fmt::Write;
    let mut field = String::with_capacity(text.len());
    for character in text.chars() {
        if character == '%' || character.is_control() || matches!(character, '\u{2028}' | '\u{2029}') {
            for byte in character.encode_utf8(&mut [0; 4]).bytes() {
                write!(field, "%{byte:02X}").unwrap();
            }
        } else {
            field.push(character);
        }
    }
    field
}

#[test]
fn diagnostic_fields_preserve_literal_escapes_and_line_separators() {
    assert_eq!(diagnostic_field("π%0A\r\n\t\u{85}\u{2028}"), "π%250A%0D%0A%09%C2%85%E2%80%A8");
}

/// Check the resolver's public error at its original source or interface row.
/// The test supplies expected values independently of the compiler's message.
#[macro_export]
macro_rules! assert_resolver_refusal {
    ($error:expr, $kind:expr, $package:expr, $file:expr, $span:expr, $message:expr $(,)?) => {{
        let error = &$error;
        let expected_kind = $kind;
        assert_eq!(error.kind, expected_kind, "{error:?}");
        assert_eq!(error.message, $message);
        assert_eq!((error.package.as_str(), error.file.as_str()), ($package, $file));
        assert_eq!(error.span, $span);
        if std::env::var_os("OMEGA_LAYER_REPORT").is_some() {
            println!(
                "OMEGA_RESOLVER_KIND\t{expected_kind:?}\t{:?}\t{}\t{}\t{}\t{}\t{}\t{}",
                error.kind,
                $crate::diagnostic_field(&error.package),
                $crate::diagnostic_field(&error.file),
                error.span.get_start_offset(),
                error.span.get_end_offset(),
                error.message.len(),
                $crate::diagnostic_field(&error.message)
            );
        }
    }};
}

/// Compare a preparation failure at its original source and report the cause returned by the stage.
#[macro_export]
macro_rules! assert_preparation_refusal {
    ($failure:expr, $expected:expr, $package:expr, $file:expr, $range:expr, $message:expr $(,)?) => {{
        match &$failure {
            omega_preparation::PreparationFailure::Preparation(refusal) | omega_preparation::PreparationFailure::MissingTarget(refusal) => {
                $crate::assert_preparation_cause!(refusal, $expected, $package, $file, $range, $message);
            }
            other => panic!("expected a preparation refusal, received {other:?}"),
        }
    }};
}

/// Tests of a supplied evaluation input can observe the cause before the package entry point wraps it.
#[macro_export]
macro_rules! assert_preparation_cause {
    ($refusal:expr, $expected:expr, $package:expr, $file:expr, $range:expr, $message:expr $(,)?) => {{
        let refusal = &$refusal;
        let expected = $expected;
        let cause = refusal.cause;
        assert_eq!(cause, expected);
        let diagnostic = &refusal.diagnostic;
        assert_eq!((diagnostic.package.as_str(), diagnostic.file.as_str()), ($package, $file));
        let range = (diagnostic.span.get_start_offset(), diagnostic.span.get_end_offset());
        assert_eq!(range, $range);
        assert_eq!(diagnostic.message, $message);
        if std::env::var_os("OMEGA_LAYER_REPORT").is_some() {
            let tag = match (expected, cause) {
                (omega_preparation::RefusalCause::Preparation(wanted), omega_preparation::RefusalCause::Preparation(actual)) => {
                    format!("OMEGA_PREPARATION_REASON\t{wanted:?}\t{actual:?}")
                }
                (_, omega_preparation::RefusalCause::TargetLayout(_)) => "OMEGA_PREPARATION_FORWARDED\tTargetLayout".to_owned(),
                (_, omega_preparation::RefusalCause::IntegerPrimitive(_)) => "OMEGA_PREPARATION_FORWARDED\tIntegerPrimitive".to_owned(),
                _ => unreachable!("cause equality checked above"),
            };
            $crate::report_preparation_observation(format!(
                "{tag}\t{}\t{}\t{}\t{}\t{}\t{}",
                $crate::diagnostic_field(&diagnostic.package),
                $crate::diagnostic_field(&diagnostic.file),
                range.0,
                range.1,
                diagnostic.message.len(),
                $crate::diagnostic_field(&diagnostic.message)
            ));
        }
    }};
}

/// Several inputs in one test can produce exactly the same diagnostic. Every comparison
/// runs, but the report needs one witness per distinct outcome in the captured test.
#[doc(hidden)]
pub fn report_preparation_observation(record: String) {
    thread_local! {
        static REPORTED: std::cell::RefCell<std::collections::HashSet<String>> = std::cell::RefCell::default();
    }
    REPORTED.with_borrow_mut(|seen| {
        if !seen.contains(&record) {
            println!("{record}");
            seen.insert(record);
        }
    });
}

/// A test may check several handoffs or repeat one check for many inputs.
/// Identify the comparison that unwinds even if an earlier iteration passed.
#[doc(hidden)]
pub struct HandoffComparison(pub &'static str, pub &'static str);

impl Drop for HandoffComparison {
    fn drop(&mut self) {
        if std::thread::panicking() && std::env::var_os("OMEGA_LAYER_REPORT").is_some() {
            let expression = self.1.split_whitespace().collect::<Vec<_>>().join(" ");
            eprintln!("OMEGA_FAILED_COMPARISON\t{}\t{expression}", self.0);
        }
    }
}

/// Emit the first success at this site. The runner also checks the test verdict.
#[macro_export]
macro_rules! observe_handoff {
    ($label:literal, $comparison:expr $(,)?) => {
        $crate::observe_handoff!(@record "OMEGA_REVIEWED_COMPARISON", $label, $comparison)
    };
    (psi $label:literal, $comparison:expr $(,)?) => {
        $crate::observe_handoff!(@record "OMEGA_L5_ASSERT", $label, $comparison)
    };
    (@record $record:literal, $label:literal, $comparison:expr) => {{
        let comparison = $crate::HandoffComparison($label, stringify!($comparison));
        let () = $comparison;
        drop(comparison);
        static REPORT: std::sync::Once = std::sync::Once::new();
        REPORT.call_once(|| {
            if std::env::var_os("OMEGA_LAYER_REPORT").is_some() {
                let expression = stringify!($comparison).split_whitespace().collect::<Vec<_>>().join(" ");
                println!("{}\t{}\t{}", $record, $label, expression);
            }
        });
    }};
}

/// Report a successful comparison only after checking the expected product.
/// A test can make several independent observations under distinct labels.
#[macro_export]
macro_rules! assert_handoff_eq {
    ($label:expr, $actual:expr, $expected:expr) => {{
        assert_eq!($actual, $expected, "{}", $label);
        if std::env::var_os("OMEGA_LAYER_REPORT").is_some() {
            println!("OMEGA_HANDOFF_ASSERT\t{}", $label);
        }
    }};
}

#[cfg(feature = "parser")]
pub mod parser_examples;
#[cfg(feature = "parser")]
pub mod tree_dump_formatter;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const OMEGA_FENCE_OPENING: &str = "```omega";
const FENCE_MARKER: &str = "```";

/// One `omega` code block from the spec, with where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecExample {
    /// Path of the Markdown file, relative to the spec directory, with forward slashes.
    pub spec_file: String,
    /// One-based line number of the opening fence in that file.
    pub fence_line_number: usize,
    /// The example's source text. Lines are joined with line feeds and the text ends with one.
    pub source_text: String,
}

impl SpecExample {
    /// A short `file:line` label for failure messages.
    pub fn get_label(&self) -> String {
        format!("{}:{}", self.spec_file, self.fence_line_number)
    }
}

/// Resolve the checkout before locating the repository spec. V2 can be opened through a junction.
fn repository_spec() -> io::Result<PathBuf> {
    let manifest = fs::canonicalize(env!("CARGO_MANIFEST_DIR"))?;
    let checkout = manifest
        .ancestors()
        .nth(3)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "spec-corpus is outside the V2 workspace layout"))?;
    Ok(checkout.join("wiki").join("spec"))
}

pub fn published_examples() -> io::Result<Vec<SpecExample>> {
    extract_omega_examples(&repository_spec()?)
}

pub fn published_chapter(chapter: &str) -> io::Result<Vec<SpecExample>> {
    let markdown = fs::read_to_string(repository_spec()?.join(chapter))?;
    let mut examples = Vec::new();
    extract_examples_from_markdown(chapter, &markdown, &mut examples);
    Ok(examples)
}

/// Reads every Markdown file under `spec_directory` and returns its `omega` examples,
/// ordered by file path and then by position in the file.
pub fn extract_omega_examples(spec_directory: &Path) -> io::Result<Vec<SpecExample>> {
    let mut markdown_paths = Vec::new();
    collect_markdown_paths(spec_directory, &mut markdown_paths)?;
    markdown_paths.sort();

    let mut examples = Vec::new();
    for markdown_path in markdown_paths {
        let markdown_text = fs::read_to_string(&markdown_path)?;
        let spec_file = markdown_path
            .strip_prefix(spec_directory)
            .unwrap_or(&markdown_path)
            .to_string_lossy()
            .replace('\\', "/");
        extract_examples_from_markdown(&spec_file, &markdown_text, &mut examples);
    }
    Ok(examples)
}

/// Appends the `omega` examples found in one Markdown document.
pub fn extract_examples_from_markdown(
    spec_file: &str,
    markdown_text: &str,
    examples: &mut Vec<SpecExample>,
) {
    let mut open_example: Option<SpecExample> = None;

    for (line_index, line_text) in markdown_text.lines().enumerate() {
        let trimmed_line = line_text.trim_start();
        match open_example.as_mut() {
            Some(_) if trimmed_line.starts_with(FENCE_MARKER) => {
                examples.extend(open_example.take());
            }
            Some(example) => {
                example.source_text.push_str(line_text);
                example.source_text.push('\n');
            }
            None if trimmed_line.starts_with(OMEGA_FENCE_OPENING) => {
                open_example = Some(SpecExample {
                    spec_file: spec_file.to_string(),
                    fence_line_number: line_index + 1,
                    source_text: String::new(),
                });
            }
            None => {}
        }
    }
    // An unclosed fence at end of file is dropped: we cannot tell where the example was meant to end.
}

fn collect_markdown_paths(
    directory: &Path,
    markdown_paths: &mut Vec<PathBuf>,
) -> io::Result<()> {
    for directory_entry in fs::read_dir(directory)? {
        let entry_path = directory_entry?.path();
        if entry_path.is_dir() {
            collect_markdown_paths(&entry_path, markdown_paths)?;
        } else if entry_path
            .extension()
            .is_some_and(|extension| extension == "md")
        {
            markdown_paths.push(entry_path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{SpecExample, extract_examples_from_markdown};

    #[test]
    fn extracts_omega_blocks_and_skips_other_languages() {
        let markdown_text = "intro\n```omega\nlet a = 1;\n```\n```text\nnot omega\n```\n  ```omega\n  indented();\n  ```\n";
        let mut examples = Vec::new();
        extract_examples_from_markdown("language/sample.md", markdown_text, &mut examples);

        assert_eq!(
            examples,
            vec![
                SpecExample {
                    spec_file: "language/sample.md".to_string(),
                    fence_line_number: 2,
                    source_text: "let a = 1;\n".to_string(),
                },
                SpecExample {
                    spec_file: "language/sample.md".to_string(),
                    fence_line_number: 8,
                    source_text: "  indented();\n".to_string(),
                },
            ]
        );
    }

    #[test]
    fn drops_an_unclosed_example() {
        let mut examples = Vec::new();
        extract_examples_from_markdown("sample.md", "```omega\nlet a = 1;\n", &mut examples);
        assert!(examples.is_empty());
    }
}
