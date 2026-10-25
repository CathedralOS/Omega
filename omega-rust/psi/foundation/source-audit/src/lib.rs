//! Source scanning shared by the per-crate source guards.
//!
//! A guard pins one crate's own non-test sources against a forbidden shape: a
//! stored text identity, a layering edge, a retired call. The scan is identical
//! in every guard — walk the crate's `src` tree, read the `.rs` files outside
//! line comments — so it lives here once. Each guard keeps what is actually
//! its own: the root it scans, its needles, and the boundary declarations it
//! exempts by exact trimmed line. The crate is a dev-dependency of the guards
//! and never part of a shipped product.

use std::fs;
use std::path::{Path, PathBuf};

/// Every `(file, line number, line)` of the sources at `root`, outside line
/// comments.
///
/// `root` is a path relative to the crate's `src/`: `""` scans the whole tree
/// and a file path scans only that file; `manifest_dir` is the calling crate's
/// `CARGO_MANIFEST_DIR`. `file` is reported relative to `src/`
/// so a guard matches suffixes without knowing the host checkout. The guard's
/// own file and the crate's test modules (`tests.rs`, `*_tests.rs`) are never
/// scanned: the needles are spelled out in them.
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

/// Every `file:line: needle` under `root` that names one of `forbidden`,
/// sorted.
///
/// `allowed` pairs are `(file suffix, exact trimmed line)`: only that pinned
/// declaration is exempt, so a shared `pub` field of the same name still
/// violates.
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
