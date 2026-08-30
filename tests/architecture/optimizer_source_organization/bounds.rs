//! File-size, entrance-size, and exact exception ratchets.

use std::collections::{BTreeMap, BTreeSet};

use crate::Audit;

const MAX_PRODUCTION_RUST_FILE_LINES: usize = 1_000;
const MAX_LEGACY_PRODUCTION_RUST_FILE_LINES: usize = 1_300;
const MAX_TEST_RUST_FILE_LINES: usize = 1_500;
pub(super) const PREFERRED_ENTRANCE_LINES: usize = 100;
const MAX_ENTRANCE_LINES: usize = 200;

/// Exact production leaves that still exceed the default ceiling.
///
/// Each ceiling is pinned to the current file size. An exception cannot grow,
/// and becomes stale as soon as the file is split below the default. New files
/// never enter this table.
const LEGACY_PRODUCTION_FILE_CEILINGS: &[(&str, usize)] = &[];

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
        path: "source/omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/selection/validation/mod.rs",
        ceiling: 140,
        semantic_reason: "owns complete selected-plan custody, roster traversal, independent validation, and receipt admission",
    },
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

fn is_entrance(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .is_some_and(|file_name| file_name == "lib.rs" || file_name == "mod.rs")
}

pub(super) fn is_test_source(path: &str) -> bool {
    path.contains("/tests/") || path.ends_with("/tests.rs") || path.ends_with("_tests.rs")
}

pub(crate) fn check(audit: &mut Audit) {
    let source_lines = &audit.source_lines;
    let violations = &mut audit.violations;

    let mut legacy_file_ceilings = BTreeMap::<&str, usize>::new();
    for (path, ceiling) in LEGACY_PRODUCTION_FILE_CEILINGS {
        if legacy_file_ceilings.insert(path, *ceiling).is_some() {
            violations.insert(format!(
                "duplicate legacy production-file exception: {path}"
            ));
        }
        if !(MAX_PRODUCTION_RUST_FILE_LINES + 1..=MAX_LEGACY_PRODUCTION_RUST_FILE_LINES)
            .contains(ceiling)
        {
            violations.insert(format!(
                "invalid legacy production-file ceiling {ceiling} for {path}"
            ));
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

    let mut observed_legacy_files = BTreeSet::new();
    let mut observed_exceptions = BTreeSet::new();
    for (path, lines) in source_lines {
        let ceiling = if is_test_source(path) {
            MAX_TEST_RUST_FILE_LINES
        } else {
            match legacy_file_ceilings.get(path.as_str()) {
                Some(ceiling) => {
                    observed_legacy_files.insert(path.as_str());
                    *ceiling
                }
                None => MAX_PRODUCTION_RUST_FILE_LINES,
            }
        };
        if *lines > ceiling {
            violations.insert(format!(
                "Rust file exceeds its {ceiling}-line ceiling: {path} ({lines})"
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

    for (path, _) in LEGACY_PRODUCTION_FILE_CEILINGS {
        let path = *path;
        if !observed_legacy_files.contains(path) {
            violations.insert(format!(
                "stale legacy production-file exception (missing, ungoverned, test-only, or now below {MAX_PRODUCTION_RUST_FILE_LINES} lines): {path}"
            ));
            continue;
        }
        if source_lines[path] <= MAX_PRODUCTION_RUST_FILE_LINES {
            violations.insert(format!(
                "stale legacy production-file exception: {path} is now {} lines",
                source_lines[path]
            ));
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
}
