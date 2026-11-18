/*
We scan a crate's non-test Rust source tree and report every `(file, line_number, line)`
or sorted `file:line: needle` violation outside line comments (`source_lines`,
`violations`). Each per-crate source guard supplies its own scan root, forbidden
needles, and exact trimmed declaration lines exempted at known boundaries.
*/

/*
Every per-crate source guard pins its non-test sources against forbidden shapes
such as stored text identities, layering edges, or retired calls. Centralizing
the directory walk and comment-filtered line scan here keeps individual guards
focused on their own needles and boundary exemptions without duplicating file
traversal logic.
*/

/*
We report file paths relative to `src/` so guard suffixes match identically
across host checkouts, and we automatically skip `source_guards.rs`, `tests.rs`,
and `*_tests.rs` where the forbidden needles themselves are spelled out. This
crate is a dev-dependency of the test guards and is never linked into shipped
binaries.
*/

use std::fs;
use std::path::{Path, PathBuf};

pub fn source_lines(manifest_dir: &str, root: &str) -> Vec<(String, usize, String)> {
    let source_root = Path::new(manifest_dir).join("src");
    let scan_root = if root.is_empty() {
        source_root.clone()
    } else {
        source_root.join(root)
    };
    let mut directories = vec![scan_root];
    let mut lines = Vec::new();
    while let Some(directory) = directories.pop() {
        let entries: Vec<PathBuf> = match fs::read_dir(&directory) {
            Ok(entries) => entries
                .map(|entry| {
                    entry
                        .unwrap_or_else(|error| {
                            panic!("cannot list {}: {error}", directory.display())
                        })
                        .path()
                })
                .collect(),
            Err(_) if directory.is_file() => vec![directory.clone()],
            Err(error) => panic!("cannot read {}: {error}", directory.display()),
        };
        for path in entries {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            if path.is_dir() {
                if !name.starts_with('.') && !matches!(name, "tests" | "examples" | "target") {
                    directories.push(path);
                }
                continue;
            }
            if path.extension().and_then(|extension| extension.to_str()) != Some("rs")
                || name == "source_guards.rs"
                || name == "tests.rs"
                || name.ends_with("_tests.rs")
            {
                continue;
            }
            let source = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
            let file = path
                .strip_prefix(&source_root)
                .expect("scanned source stays within the crate")
                .display()
                .to_string();
            for (line_number, line) in source.lines().enumerate() {
                if line.trim_start().starts_with("//") {
                    continue;
                }
                lines.push((file.clone(), line_number + 1, line.to_owned()));
            }
        }
    }
    lines
}

pub fn violations<T: AsRef<str>>(
    manifest_dir: &str,
    root: &str,
    forbidden: &[T],
    allowed: &[(&str, &str)],
) -> Vec<String> {
    let mut violations = Vec::new();
    for (file, line_number, line) in source_lines(manifest_dir, root) {
        for name in forbidden {
            let name = name.as_ref();
            if line.contains(name)
                && !allowed.iter().any(|(suffix, declaration)| {
                    file.ends_with(suffix) && line.trim() == *declaration
                })
            {
                violations.push(format!("{file}:{line_number}: {name}"));
            }
        }
    }
    violations.sort();
    violations
}
