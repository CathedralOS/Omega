//! File-size, entrance-size, and exact exception ratchets.

use std::collections::{BTreeMap, BTreeSet};

use crate::Audit;

const MAX_PRODUCTION_RUST_FILE_LINES: usize = 600;
const MAX_TEST_RUST_FILE_LINES: usize = 800;
pub(super) const PREFERRED_ENTRANCE_LINES: usize = 100;
const MAX_ENTRANCE_LINES: usize = 200;

#[derive(Clone, Copy)]
struct EntranceException {
    path: &'static str,
    ceiling: usize,
    semantic_reason: &'static str,
}

#[derive(Clone, Copy)]
struct SourceFileException {
    path: &'static str,
    ceiling: usize,
    semantic_reason: &'static str,
}

/// Exact no-growth ratchets for oversized files that predate this gate.
///
/// Each row becomes stale as soon as its file is sharded beneath the ordinary
/// production/test ceiling. New files and unlisted growth still fail closed.
const SOURCE_FILE_EXCEPTIONS: &[SourceFileException] = &[
    SourceFileException {
        path: "source/omega-rust/omega/pipeline/omega-target-operations-to-assigned-target-operations/src/assignment/function/unit/operation.rs",
        ceiling: 851,
        semantic_reason: "the unit-operation assignment dispatcher has not yet been split by family",
    },
    SourceFileException {
        path: "source/omega-rust/omega/pipeline/omega-target-operations-to-assigned-target-operations/src/assignment/function/unit/structural_scalar.rs",
        ceiling: 623,
        semantic_reason: "structural-scalar assignment and field layout replay remain colocated",
    },
    SourceFileException {
        path: "source/omega-rust/omega/representations/omega-optimization-unit/src/identity/structural_encoding.rs",
        ceiling: 669,
        semantic_reason: "the canonical structural identity vocabulary remains one encoding owner",
    },
    SourceFileException {
        path: "source/omega-rust/omega/pipeline/omega-psi-to-abstract-operations/tests/dynamic_dispatch.rs",
        ceiling: 954,
        semantic_reason: "the dynamic-dispatch custody scenarios still share one test fixture",
    },
    SourceFileException {
        path: "source/omega-rust/omega/pipeline/omega-target-operations-to-assigned-target-operations/src/tests/dynamic_dispatch.rs",
        ceiling: 919,
        semantic_reason: "the assignment dynamic-dispatch scenarios still share one test fixture",
    },
    SourceFileException {
        path: "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/terminal_authority_review/tests.rs",
        ceiling: 936,
        semantic_reason: "terminal authority policy scenarios still share a large fixture vocabulary",
    },
    SourceFileException {
        path: "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/tests/native_realization/construction_prefix.rs",
        ceiling: 863,
        semantic_reason: "the construction-prefix depth ladder remains in one source fixture",
    },
];

/// Exact exceptions to the preferred 100-line entrance ceiling.
///
/// An exception is stale when its file disappears, ceases to be an entrance,
/// or returns to 100 lines or fewer. Ceilings may never exceed the hard
/// 200-line entrance limit.
const ENTRANCE_EXCEPTIONS: &[EntranceException] = &[EntranceException {
    path: "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/terminal_authority_policy/mod.rs",
    ceiling: 129,
    semantic_reason: "the entrance still exposes the complete closed authority-policy vocabulary",
}];

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

    let mut source_file_exceptions = BTreeMap::<&str, &SourceFileException>::new();
    for exception in SOURCE_FILE_EXCEPTIONS {
        if source_file_exceptions
            .insert(exception.path, exception)
            .is_some()
        {
            violations.insert(format!(
                "duplicate source-file exception: {}",
                exception.path
            ));
        }
        let ordinary_ceiling = if is_test_source(exception.path) {
            MAX_TEST_RUST_FILE_LINES
        } else {
            MAX_PRODUCTION_RUST_FILE_LINES
        };
        if exception.ceiling <= ordinary_ceiling {
            violations.insert(format!(
                "stale source-file exception ceiling {} for {} (ordinary ceiling is {})",
                exception.ceiling, exception.path, ordinary_ceiling
            ));
        }
        if exception.semantic_reason.trim().is_empty() {
            violations.insert(format!(
                "source-file exception lacks a semantic reason: {}",
                exception.path
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

    let mut observed_exceptions = BTreeSet::new();
    let mut observed_source_file_exceptions = BTreeSet::new();
    for (path, lines) in source_lines {
        let ordinary_ceiling = if is_test_source(path) {
            MAX_TEST_RUST_FILE_LINES
        } else {
            MAX_PRODUCTION_RUST_FILE_LINES
        };
        let source_file_exception = source_file_exceptions.get(path.as_str()).copied();
        let ceiling = source_file_exception
            .map(|exception| exception.ceiling)
            .unwrap_or(ordinary_ceiling);
        if *lines > ceiling {
            violations.insert(format!(
                "Rust file exceeds its {ceiling}-line ceiling: {path} ({lines})"
            ));
        }
        if let Some(exception) = source_file_exception {
            observed_source_file_exceptions.insert(path.as_str());
            if *lines <= ordinary_ceiling {
                violations.insert(format!(
                    "stale source-file exception: {} is now {} lines (reason: {})",
                    path, lines, exception.semantic_reason
                ));
            }
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

    for exception in SOURCE_FILE_EXCEPTIONS {
        if !observed_source_file_exceptions.contains(exception.path) {
            violations.insert(format!(
                "stale source-file exception points to a missing or ungoverned file: {}",
                exception.path
            ));
        }
    }
}
