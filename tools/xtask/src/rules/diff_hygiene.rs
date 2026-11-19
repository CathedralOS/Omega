//! Diff hygiene checks (retired corpus, frozen facts set, Python/swarm limits, measurements).

use std::collections::BTreeSet;
use std::path::Path;

use super::{FACTS_ROOT, show_git_file};

/// The fixture corpus that was deleted from the repository; see tests/README.md.
const RETIRED_CORPUS_ROOT: &str = "tests/omega/";
/// Where a test keeps the few programs it reads.
const KEPT_FIXTURES_ROOT: &str = "tests/fixtures/";
const MEASUREMENTS_ROOT: &str = "wiki/drafts/measurements/";
const MEASUREMENT_LINE_LIMIT: usize = 300;

pub(super) fn diff_hygiene_violations(changes: &[(char, String)]) -> Vec<String> {
    // Git reports moves as deletions and additions here. Moving a directory
    // under tools/ may carry existing scripts with it.
    let deleted_python_paths: BTreeSet<&str> = changes
        .iter()
        .filter(|(status, path)| *status == 'D' && path.ends_with(".py"))
        .map(|(_, path)| path.as_str())
        .collect();
    let mut violations = Vec::new();
    // `samples/cli/facts/` is a frozen finite set: it may not grow, but a
    // change may move its programs within it. A diff without rename
    // detection shows each move as one deletion and one addition.
    let mut added_facts = Vec::new();
    let mut deleted_facts = 0usize;
    for (status, path) in changes {
        if *status == 'D' && path.starts_with(FACTS_ROOT) && path.ends_with(".omg") {
            deleted_facts += 1;
        }
        if *status != 'A' {
            continue;
        }
        if path.starts_with(RETIRED_CORPUS_ROOT) {
            violations.push(format!(
                "{path}: {RETIRED_CORPUS_ROOT} is retired and nothing runs it (tests/README.md); keep a program a test needs beside that test, under its crate’s {KEPT_FIXTURES_ROOT}"
            ));
        } else if path.ends_with(".omg") {
            if path.starts_with(FACTS_ROOT) {
                added_facts.push(path.as_str());
            }
        } else if path.starts_with("tools/swarm/waves/") {
            violations.push(format!(
                "{path}: swarm wave state belongs in ignored build/swarm/<wave>/, not tools/swarm/waves/"
            ));
        } else if let Some(original_path) = path.strip_prefix("tools/")
            && path.ends_with(".py")
            && !deleted_python_paths.contains(original_path)
        {
            violations.push(format!(
                "{path}: Python repository tooling is retired; implement developer and CI tooling in Rust under `tools/xtask/` (`cargo xtask`)"
            ));
        }
    }
    if added_facts.len() > deleted_facts {
        let added = added_facts.len();
        violations.extend(added_facts.iter().map(|path| {
            format!(
                "{path}: {FACTS_ROOT} is a frozen set; it may not grow (this change adds {added} .omg files to it and removes {deleted_facts})"
            )
        }));
    }
    violations
}

pub(super) fn measurement_violations(root: &Path, changes: &[(char, String)]) -> Vec<String> {
    let mut violations = Vec::new();
    for (status, path) in changes {
        if *status == 'A' && path.starts_with(MEASUREMENTS_ROOT) {
            if !path.ends_with(".md") {
                violations.push(format!(
                    "{path}: raw run data stays out of git. Commit a .md summary and link the data by a pinned commit, or keep it in the claim note"
                ));
            } else {
                let lines = show_git_file(root, "HEAD", path).lines().count();
                if lines > MEASUREMENT_LINE_LIMIT {
                    violations.push(format!(
                        "{path}: {lines} lines; a measurement summary is at most {MEASUREMENT_LINE_LIMIT}. Keep the numbers that decide something and link the raw data by a pinned commit"
                    ));
                }
            }
        }
    }
    violations
}
