/*
    Golden-file fixture runner shared by every layer's tests.

    A layer is a function from one representation to the next. To test it we
    keep a directory of small input files, and beside each input a text file
    holding what the layer should print for it. `check_directory` walks the
    directory, hands each input's bytes to the layer, and compares what comes
    back with the stored text, byte for byte.

    We compare text and not data structures on purpose: a failure then shows up
    as a readable diff of the layer's own dump, and adding a case means adding
    two files, with no Rust to write.

    Blessing: running the tests with the environment variable `OMEGA_BLESS=1`
    rewrites every expected file from the current output instead of comparing.
    The rewritten files show up in `git diff`, and that diff is what gets
    reviewed. Blessing proves nothing by itself; it only records.
*/

/*
    Why the layer callback returns `Result<String, String>` and not `String`:

    An `accept` directory and a `reject` directory both compare printed output,
    but each also has a rule the printed output cannot express. Every input
    under `accept` must be accepted, and every input under `reject` must be
    refused. If the callback could only return text, a lexer that started
    rejecting an `accept` case would be blessed into a passing test that
    expects the error. So the callback returns `Err` when the layer did the
    wrong *kind* of thing, and that is a failure no expected file can excuse.
*/

#![forbid(unsafe_code)]

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The environment variable that switches the runner from comparing to rewriting.
pub const BLESS_ENVIRONMENT_VARIABLE: &str = "OMEGA_BLESS";

/// Extension of the file that stores the expected output beside each input.
pub const EXPECTED_EXTENSION: &str = "expected";

/// One fixture that did not produce its expected output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoldenFailure {
    pub input_path: PathBuf,
    pub explanation: String,
}

/// The outcome of checking one fixture directory.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GoldenReport {
    pub case_count: usize,
    pub blessed_count: usize,
    pub failures: Vec<GoldenFailure>,
}

impl GoldenReport {
    /// Renders every failure as text for a test's panic message, or None when all cases passed.
    pub fn describe_failures(&self) -> Option<String> {
        if self.failures.is_empty() {
            return None;
        }

        let mut description = format!(
            "{} of {} fixtures failed:\n",
            self.failures.len(),
            self.case_count
        );
        for failure in &self.failures {
            description.push_str(&format!(
                "\n--- {}\n{}\n",
                failure.input_path.display(),
                failure.explanation
            ));
        }
        description.push_str(&format!(
            "\nIf the new output is correct, rerun with {BLESS_ENVIRONMENT_VARIABLE}=1 and review the diff.\n"
        ));
        Some(description)
    }
}

/// Runs every `*.<input_extension>` file directly inside `fixture_directory` through `produce_output`
/// and compares the result with the sibling `.expected` file.
///
/// `produce_output` returns `Err` when the layer did the wrong kind of thing for this directory;
/// that is reported as a failure even when blessing. Cases run in file-name order.
pub fn check_directory(
    fixture_directory: &Path,
    input_extension: &str,
    produce_output: impl Fn(&[u8]) -> Result<String, String>,
) -> io::Result<GoldenReport> {
    let should_bless =
        std::env::var_os(BLESS_ENVIRONMENT_VARIABLE).is_some_and(|value| value == "1");
    let mut report = GoldenReport::default();

    for input_path in list_input_paths(fixture_directory, input_extension)? {
        report.case_count += 1;
        let input_bytes = fs::read(&input_path)?;
        let expected_path = input_path.with_extension(EXPECTED_EXTENSION);

        let actual_output = match produce_output(&input_bytes) {
            Ok(actual_output) => actual_output,
            Err(explanation) => {
                report.failures.push(GoldenFailure {
                    input_path,
                    explanation,
                });
                continue;
            }
        };

        if should_bless {
            let already_current = fs::read(&expected_path)
                .is_ok_and(|expected_bytes| expected_bytes == actual_output.as_bytes());
            if !already_current {
                fs::write(&expected_path, actual_output.as_bytes())?;
                report.blessed_count += 1;
            }
            continue;
        }

        match fs::read(&expected_path) {
            Ok(expected_bytes) if expected_bytes == actual_output.as_bytes() => {}
            Ok(expected_bytes) => {
                let expected_output = String::from_utf8_lossy(&expected_bytes);
                report.failures.push(GoldenFailure {
                    input_path,
                    explanation: describe_first_difference(&expected_output, &actual_output),
                });
            }
            Err(read_error) if read_error.kind() == io::ErrorKind::NotFound => {
                report.failures.push(GoldenFailure {
                    input_path,
                    explanation: format!(
                        "no expected file at {}; actual output was:\n{actual_output}",
                        expected_path.display()
                    ),
                });
            }
            Err(read_error) => return Err(read_error),
        }
    }

    Ok(report)
}

fn list_input_paths(fixture_directory: &Path, input_extension: &str) -> io::Result<Vec<PathBuf>> {
    let mut input_paths = Vec::new();
    for directory_entry in fs::read_dir(fixture_directory)? {
        let entry_path = directory_entry?.path();
        let has_input_extension = entry_path
            .extension()
            .is_some_and(|extension| extension == input_extension);
        if has_input_extension && entry_path.is_file() {
            input_paths.push(entry_path);
        }
    }
    // Directory order is not stable across filesystems, and failures should read the same everywhere.
    input_paths.sort();
    Ok(input_paths)
}

/// Shows the first line where the two outputs part ways, with the line before it for orientation.
fn describe_first_difference(expected_output: &str, actual_output: &str) -> String {
    let expected_lines: Vec<&str> = expected_output.lines().collect();
    let actual_lines: Vec<&str> = actual_output.lines().collect();
    let shared_line_count = expected_lines.len().min(actual_lines.len());
    let first_different_line = (0..shared_line_count)
        .find(|&line_index| expected_lines[line_index] != actual_lines[line_index])
        .unwrap_or(shared_line_count);

    let mut description = format!(
        "first difference at output line {}:\n",
        first_different_line + 1
    );
    if first_different_line > 0 {
        description.push_str(&format!(
            "    {}\n",
            expected_lines[first_different_line - 1]
        ));
    }
    description.push_str(&format!(
        "  - {}\n",
        expected_lines
            .get(first_different_line)
            .copied()
            .unwrap_or("<end of expected output>")
    ));
    description.push_str(&format!(
        "  + {}\n",
        actual_lines
            .get(first_different_line)
            .copied()
            .unwrap_or("<end of actual output>")
    ));
    if expected_lines.len() == actual_lines.len() && first_different_line == shared_line_count {
        description.push_str("  (the outputs differ only in their final line ending)\n");
    }
    description
}

#[cfg(test)]
mod tests {
    use super::describe_first_difference;

    #[test]
    fn first_difference_names_the_line_and_both_sides() {
        let description = describe_first_difference("one\ntwo\nthree\n", "one\nTWO\nthree\n");
        assert!(description.contains("output line 2"), "{description}");
        assert!(description.contains("- two"), "{description}");
        assert!(description.contains("+ TWO"), "{description}");
    }

    #[test]
    fn first_difference_reports_a_missing_tail() {
        let description = describe_first_difference("one\ntwo\n", "one\n");
        assert!(description.contains("output line 2"), "{description}");
        assert!(
            description.contains("+ <end of actual output>"),
            "{description}"
        );
    }
}
