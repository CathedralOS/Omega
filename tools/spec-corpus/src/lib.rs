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

fn collect_markdown_paths(directory: &Path, markdown_paths: &mut Vec<PathBuf>) -> io::Result<()> {
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
