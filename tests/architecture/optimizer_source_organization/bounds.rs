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

/// Exact exceptions to the preferred 100-line entrance ceiling.
///
/// An exception is stale when its file disappears, ceases to be an entrance,
/// or returns to 100 lines or fewer. Ceilings may never exceed the hard
/// 200-line entrance limit.
const ENTRANCE_EXCEPTIONS: &[EntranceException] = &[];

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
    for (path, lines) in source_lines {
        let ceiling = if is_test_source(path) {
            MAX_TEST_RUST_FILE_LINES
        } else {
            MAX_PRODUCTION_RUST_FILE_LINES
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
