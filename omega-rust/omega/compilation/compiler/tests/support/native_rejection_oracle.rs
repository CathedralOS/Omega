//! Authored native publication refusals for build-only pass fixtures.
//!
//! A fixture's source can be valid while publishing it for one target must
//! refuse: mixed backing footprints behind a thin foreign pointer, an entry
//! with no physical adapter. `expected_native_rejection.txt` pins that refusal
//! by its diagnostic fragments, and `expected_native_rejection.<target>.txt`
//! overrides it for one target. Without this oracle such a row is
//! indistinguishable from a program that merely fails to build, so the record
//! carries `native-rejection:match` only when the realization that ran refused
//! with every authored fragment and nothing executed.
//!
//! The oracle speaks about publication, so it is weighed only for the target a
//! leg actually realized: the native leg's target, or the check leg's
//! `check-target`. A leg that only accepted the source published nothing and
//! leaves the oracle unweighed.

use std::path::Path;

/// Facts that prove some engine ran the program. A refusal that also executed
/// is not the authored publication refusal.
const EXECUTION_FACTS: &[&str] = &[
    "exit:",
    "signal:",
    "stdout:",
    "stderr:",
    "interpreter-exit:",
    "interpreter-signal:",
];

fn target_file(fixture_dir: &Path, name: &str, target: &str) -> std::path::PathBuf {
    let specific = fixture_dir.join(format!("{name}.{target}.txt"));
    if specific.is_file() {
        specific
    } else {
        fixture_dir.join(format!("{name}.txt"))
    }
}

/// `None` when the fixture authors no refusal for `realized_target`. Otherwise
/// whether the observed record is that refusal; the error says what differed.
pub(super) fn weigh(
    fixture_dir: &Path,
    tier: &str,
    relative: &str,
    realized_target: &str,
    status: &str,
    facts: &[String],
    diagnostics: &[String],
) -> Option<Result<(), String>> {
    let path = target_file(fixture_dir, "expected_native_rejection", realized_target);
    if !path.exists() {
        return None;
    }
    let fragments = match std::fs::read_to_string(&path) {
        Ok(text) => text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect::<Vec<_>>(),
        Err(error) => {
            return Some(Err(format!(
                "invalid native rejection expectation ({error})"
            )));
        }
    };
    if fragments.is_empty() {
        return Some(Err(
            "invalid native rejection expectation (no diagnostic fragments)".to_owned(),
        ));
    }
    // A refusal cannot share a fixture with a promise that the same target's
    // product runs: one of the two would always be unmet.
    if tier != "pass"
        || relative.ends_with("_exit")
        || target_file(fixture_dir, "expected_native_exit", realized_target).exists()
        || ["stdout", "stderr", "terminal_targets"]
            .iter()
            .any(|oracle| fixture_dir.join(format!("expected_{oracle}.txt")).exists())
    {
        return Some(Err(
            "invalid native rejection expectation (requires a build-only pass fixture without execution oracles)"
                .to_owned(),
        ));
    }
    let executed = facts.iter().any(|fact| {
        EXECUTION_FACTS
            .iter()
            .any(|prefix| fact.starts_with(prefix))
            || fact
                .strip_prefix("interpreter:")
                .is_some_and(|verdict| verdict != "unchecked")
    });
    let missing = fragments
        .iter()
        .filter(|fragment| {
            !diagnostics
                .iter()
                .any(|text| text.contains(fragment.as_str()))
        })
        .collect::<Vec<_>>();
    if status == "rejected" && !executed && missing.is_empty() {
        return Some(Ok(()));
    }
    Some(Err(format!(
        "expected native publication rejection for {realized_target} containing {fragments:?}; observed {status}{}{}",
        if executed {
            " with execution facts"
        } else {
            ""
        },
        if missing.is_empty() {
            String::new()
        } else {
            format!(" without {missing:?}")
        },
    )))
}
