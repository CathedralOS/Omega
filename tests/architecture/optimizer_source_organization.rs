//! Repository guard for the optimizer source-navigation contract.
//!
//! The governing design brief is
//! `wiki/design_briefs/optimizer/source_organization.md`. Optimizer source
//! files have a hard size ceiling, while `lib.rs` and `mod.rs` entrances have
//! a tighter default ceiling. A short, exact exception table permits an
//! entrance to cross the preferred 100-line boundary only when that entrance
//! still owns one stated semantic coordination responsibility.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const MAX_RUST_FILE_LINES: usize = 1_500;
const PREFERRED_ENTRANCE_LINES: usize = 100;
const MAX_ENTRANCE_LINES: usize = 200;

/// The optimizer surfaces whose source organization is architecture-governed.
/// Keep these roots explicit: silently losing a moved or renamed tree must
/// fail this test rather than shrinking its jurisdiction.
const GOVERNED_ROOTS: &[&str] = &[
    "source/omega-rust/omega/pipeline/optimization",
    "source/omega-rust/omega/representations/omega-optimization-core",
    "source/omega-rust/omega/representations/omega-optimization-unit",
    "source/omega-rust/omega/pipeline/omega-optimization-run-to-abstract-operations",
];

#[derive(Clone, Copy)]
struct EntranceException {
    path: &'static str,
    ceiling: usize,
    semantic_reason: &'static str,
}

/// Exact exceptions to the preferred 100-line entrance ceiling.
///
/// An exception is stale when its file disappears, ceases to be an entrance,
/// or returns to 100 lines or fewer. Ceilings may never exceed the hard
/// 200-line entrance limit.
const ENTRANCE_EXCEPTIONS: &[EntranceException] = &[
    EntranceException {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/realization/function_relative_realization/assembly/mod.rs",
        ceiling: 120,
        semantic_reason: "owns the paired build-and-validate orchestration seam for function-relative realization",
    },
    EntranceException {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/tests/mod.rs",
        ceiling: 170,
        semantic_reason: "owns shared validated-unit construction helpers consumed by the validation test leaves",
    },
];

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|candidate| {
            candidate.join("Cargo.toml").is_file() && candidate.join("source/omega-rust").is_dir()
        })
        .expect("architecture tests must run from within the Omega repository")
        .to_path_buf()
}

fn collect_rust_files(directory: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| format!("cannot read {}: {error}", directory.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("cannot enumerate {}: {error}", directory.display()))?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let file_type = entry
            .file_type()
            .map_err(|error| format!("cannot inspect {}: {error}", entry.path().display()))?;
        if file_type.is_dir() {
            collect_rust_files(&entry.path(), files)?;
        } else if file_type.is_file() && entry.path().extension().is_some_and(|ext| ext == "rs") {
            files.push(entry.path());
        }
    }
    Ok(())
}

fn repository_relative_path(repository: &Path, path: &Path) -> Result<String, String> {
    let relative = path.strip_prefix(repository).map_err(|error| {
        format!(
            "{} is outside repository {}: {error}",
            path.display(),
            repository.display()
        )
    })?;
    Ok(relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/"))
}

fn is_entrance(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .is_some_and(|file_name| file_name == "lib.rs" || file_name == "mod.rs")
}

#[test]
fn optimizer_source_organization_is_bounded_and_navigable() {
    let repository = repository_root();
    let mut violations = BTreeSet::new();
    let mut source_lines = BTreeMap::<String, usize>::new();

    for governed_root in GOVERNED_ROOTS {
        let absolute_root = repository.join(governed_root);
        if !absolute_root.is_dir() {
            violations.insert(format!("missing governed root: {governed_root}"));
            continue;
        }

        let mut files = Vec::new();
        if let Err(error) = collect_rust_files(&absolute_root, &mut files) {
            violations.insert(format!("failed to inventory {governed_root}: {error}"));
            continue;
        }
        files.sort();
        if files.is_empty() {
            violations.insert(format!(
                "governed root contains no Rust files: {governed_root}"
            ));
        }

        for file in files {
            let relative = match repository_relative_path(&repository, &file) {
                Ok(relative) => relative,
                Err(error) => {
                    violations.insert(error);
                    continue;
                }
            };
            let contents = match fs::read_to_string(&file) {
                Ok(contents) => contents,
                Err(error) => {
                    violations.insert(format!("cannot read {relative}: {error}"));
                    continue;
                }
            };
            let lines = contents.lines().count();
            if source_lines.insert(relative.clone(), lines).is_some() {
                violations.insert(format!("governed roots overlap at Rust file: {relative}"));
            }
        }
    }

    let mut exceptions = BTreeMap::<&str, &EntranceException>::new();
    for exception in ENTRANCE_EXCEPTIONS {
        if exceptions.insert(exception.path, exception).is_some() {
            violations.insert(format!("duplicate entrance exception: {}", exception.path));
        }
        if !(PREFERRED_ENTRANCE_LINES + 1..=MAX_ENTRANCE_LINES).contains(&exception.ceiling) {
            violations.insert(format!(
                "invalid entrance exception ceiling {} for {} (must be {}..={})",
                exception.ceiling,
                exception.path,
                PREFERRED_ENTRANCE_LINES + 1,
                MAX_ENTRANCE_LINES
            ));
        }
        if exception.semantic_reason.trim().is_empty() {
            violations.insert(format!(
                "entrance exception lacks a semantic reason: {}",
                exception.path
            ));
        }
    }

    let mut observed_exceptions = BTreeSet::new();
    for (path, lines) in &source_lines {
        if *lines > MAX_RUST_FILE_LINES {
            violations.insert(format!(
                "Rust file exceeds {MAX_RUST_FILE_LINES} lines: {path} ({lines})"
            ));
        }

        if !is_entrance(path) {
            continue;
        }
        let exception = exceptions.get(path.as_str()).copied();
        if exception.is_some() {
            observed_exceptions.insert(path.as_str());
        }

        match *lines {
            0..=PREFERRED_ENTRANCE_LINES => {
                if let Some(exception) = exception {
                    violations.insert(format!(
                        "stale entrance exception: {} is now {} lines (reason: {})",
                        path, lines, exception.semantic_reason
                    ));
                }
            }
            lines @ 101..=MAX_ENTRANCE_LINES => match exception {
                None => {
                    violations.insert(format!(
                        "entrance exceeds the preferred {PREFERRED_ENTRANCE_LINES}-line limit without an exact exception: {path} ({lines})"
                    ));
                }
                Some(exception) if lines > exception.ceiling => {
                    violations.insert(format!(
                        "entrance exceeds its exception ceiling {}: {} ({lines}; reason: {})",
                        exception.ceiling, path, exception.semantic_reason
                    ));
                }
                Some(_) => {}
            },
            lines => {
                violations.insert(format!(
                    "entrance exceeds the hard {MAX_ENTRANCE_LINES}-line limit: {path} ({lines})"
                ));
            }
        }
    }

    for exception in ENTRANCE_EXCEPTIONS {
        if observed_exceptions.contains(exception.path) {
            continue;
        }
        match source_lines.get(exception.path) {
            None => {
                violations.insert(format!(
                    "stale entrance exception points to a missing or ungoverned file: {}",
                    exception.path
                ));
            }
            Some(_) => {
                violations.insert(format!(
                    "stale entrance exception points to a non-entrance Rust file: {}",
                    exception.path
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "optimizer source organization violations:\n{}",
        violations.into_iter().collect::<Vec<_>>().join("\n")
    );
}
