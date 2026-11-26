//! A positional pattern, `(first, ..)`, and a `let` pattern that names its
//! record, `let Pair { first as _, second } = pair;`, compile with the
//! public `omega` command and run in both engines.
//!
//! Each program under `fixtures/patterns/run` ends in a comparison of the
//! values its patterns bound with the values the record was built from, and
//! crashes when they differ. So a pattern that bound another field than the
//! one its place or name selects changes the exit status of both engines.
//! `wiki/spec/language/patterns.md`, "Data and case decomposition" and
//! "Irrefutable data bindings", is the rule: a positional pattern binds
//! fields by their place in declaration order, has exact arity unless it
//! ends in `..`, and a `let` pattern must not be able to fail.
//!
//! Each program has a twin under `fixtures/patterns/fail` that differs in
//! the pattern alone and is refused. `expected.txt` beside the twin holds
//! the refusal and the line and column it must point at.

#[path = "support/project.rs"]
mod project;
mod support;

/// The forms, each a directory under `run` and under `fail`.
///
/// - `positional_binding`: `let (_, operation) = command;`, the spec's own
///   example, binding a sum the record holds and then matching its case.
///   The twin has one component for two fields and no `..`.
/// - `positional_rest`: `let (first, ..) = triple;`. The twin writes the
///   `..` first.
/// - `positional_arm`: `(first, ..) -> ...` and `(_, second) -> ...` in a
///   `match`, and `(first, second) -> ...` in a `transition`. The twin
///   writes a positional arm over a sum, which has no fields to place.
/// - `headed_binding`: `let Pair { first as _, second } = pair;`. The twin
///   names another record with the same fields.
/// - `field_subject`: `let (_, high) = outer.inner;` and
///   `let { low, high as _ } = outer.inner;`, whose subject is a field that
///   holds a record. The twin decomposes the scalar field `outer.tag`.
const FORMS: [&str; 5] = [
    "positional_binding",
    "positional_rest",
    "positional_arm",
    "headed_binding",
    "field_subject",
];

fn invoke(arguments: &[&str], tier: &str, form: &str) -> (bool, String) {
    let project = support::repository_root()
        .join("source-rust/application/tests/fixtures/patterns")
        .join(tier)
        .join(form);
    let output = project::omega(&project, &[arguments, &["main.omg"][..]].concat());
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let _ = std::fs::remove_dir_all(project.join("build"));
    let _ = std::fs::remove_file(project.join("omega.lock"));
    (output.status.success(), report)
}

#[test]
fn each_pattern_form_binds_the_fields_it_selects_in_both_engines() {
    for form in FORMS {
        for (engine, receipt) in [("native", "native exit 0"), ("psi", "Psi exit 0")] {
            let (success, report) = invoke(&["run", "--engine", engine], "run", form);
            assert!(success, "{form}: {report}");
            assert!(
                report.lines().any(|line| line == receipt),
                "{form}: {report}"
            );
        }
    }
}

#[test]
fn each_twin_is_refused_at_its_pattern() {
    for form in FORMS {
        let expected = std::fs::read_to_string(
            support::repository_root()
                .join("source-rust/application/tests/fixtures/patterns/fail")
                .join(form)
                .join("expected.txt"),
        )
        .unwrap();
        let (success, report) = invoke(&["--check"], "fail", form);
        assert!(!success, "{form}: {report}");
        assert!(
            !report.contains("native exit") && !report.contains("Psi exit"),
            "{form}: a refused program must not run: {report}"
        );
        for line in expected.lines().filter(|line| !line.is_empty()) {
            assert!(
                report.contains(line),
                "{form}: expected `{line}` in {report}"
            );
        }
    }
}
