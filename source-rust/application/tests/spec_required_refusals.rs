//! Four rules the specification states as errors, which the compiler used
//! to accept: a reference read as its scalar without `$`, a membership test
//! against a domain of another carrier, two members of one data declaration
//! with one name, and a public signature that names a private type.
//!
//! Each program under `fixtures/refusals/run` uses the rule correctly and
//! must run in both engines; it crashes if a value it computes is wrong. Each
//! negative twin is that program with one edit, made here rather than kept
//! as a second file, so a twin differs from its program in exactly the
//! construct the rule judges. The twin must be refused with the rule's
//! diagnostic at the edited place.
//!
//! - `explicit_dereference`: `wiki/spec/language/expressions.md`, "Explicit
//!   dereference": "Omega never inserts a dereference to satisfy an expected
//!   value type ... bindings, assignments, field initializers, returns, call
//!   arguments, operator operands and value patterns." One twin per `$`.
//! - `carrier_membership`: `wiki/spec/language/domains.md`, "Carrier-exact
//!   membership": `7u8 in Small` over a `u16` domain is ill-formed.
//! - `member_namespace`: `wiki/spec/language/data_and_literals.md`, "Records,
//!   sums and nested declarations": cases and attached domains share one
//!   member namespace and "Collisions reject".
//! - `public_signature`: `wiki/spec/language/modules.md`, "Visibility":
//!   "Public interfaces cannot select private declarations".

mod support;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn fixture(case: &str) -> PathBuf {
    support::repository_root()
        .join("source-rust/application/tests/fixtures/refusals/run")
        .join(case)
}

fn omega(project: &Path, arguments: &[&str]) -> (bool, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(arguments)
        .current_dir(project)
        .stdin(Stdio::null())
        .output()
        .expect("omega runs on the hosted target");
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let _ = std::fs::remove_dir_all(project.join("build"));
    let _ = std::fs::remove_file(project.join("omega.lock"));
    (output.status.success(), report)
}

/// One edit of a program: replace the `occurrence`th (from zero) `from` with
/// `to`. Returns the edited text and the 1-based line and column of the edit.
fn edited(source: &str, from: &str, occurrence: usize, to: &str) -> (String, usize, usize) {
    let at = source
        .match_indices(from)
        .nth(occurrence)
        .unwrap_or_else(|| panic!("`{from}` occurs fewer than {} times", occurrence + 1))
        .0;
    let line = source[..at].matches('\n').count() + 1;
    let column = at - source[..at].rfind('\n').map_or(0, |newline| newline + 1) + 1;
    let mut text = source.to_owned();
    text.replace_range(at..at + from.len(), to);
    (text, line, column)
}

/// Copy the program's build file and the edited `main.omg` into a fresh
/// directory, and require `omega --check` to refuse it with `rule` at
/// `line:column` of `main.omg`.
fn refuses(case: &str, label: &str, text: &str, line: usize, column: usize, rule: &str) {
    let project = std::env::temp_dir().join(format!(
        "omega-spec-refusals-{}-{case}-{label}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).unwrap();
    std::fs::copy(fixture(case).join("build.omg"), project.join("build.omg")).unwrap();
    std::fs::write(project.join("main.omg"), text).unwrap();
    let (success, report) = omega(&project, &["--check", "main.omg"]);
    let _ = std::fs::remove_dir_all(&project);
    assert!(!success, "{case} {label} compiled:\n{text}\n{report}");
    assert!(report.contains(rule), "{case} {label}: {report}");
    let at = format!("main.omg:{line}:{column}");
    assert!(
        report.contains(&at),
        "{case} {label} not refused at {at}: {report}"
    );
}

#[test]
fn each_program_runs_in_both_engines() {
    for case in [
        "explicit_dereference",
        "carrier_membership",
        "member_namespace",
        "public_signature",
    ] {
        let (success, report) = omega(&fixture(case), &["run", "--both", "main.omg"]);
        assert!(success, "{case}: {report}");
        for receipt in ["native exit 0", "interp exit: 0"] {
            assert!(
                report.lines().any(|line| line == receipt),
                "{case}: {report}"
            );
        }
    }
}

#[test]
fn a_reference_read_without_dollar_is_refused_at_every_value_place() {
    let source = std::fs::read_to_string(fixture("explicit_dereference").join("main.omg")).unwrap();
    // `read`'s clauses: an operand of a comparison in its `requires` and in
    // its `ensures`. `read`'s body: a binding, a field initializer, a match
    // subject, an operator operand, a call argument and an arm result.
    // `held`'s `requires` is the Boolean referent itself, and its body
    // returns it.
    for (reference, count) in [("$v", 8), ("$f", 2)] {
        let places = source.matches(reference).count();
        assert_eq!(
            places, count,
            "the program reads `{reference}` at {count} places"
        );
        for occurrence in 0..places {
            let (text, line, column) = edited(&source, reference, occurrence, &reference[1..]);
            refuses(
                "explicit_dereference",
                &format!("{}{occurrence}", &reference[1..]),
                &text,
                line,
                column,
                "Omega never inserts a dereference",
            );
        }
    }
}

#[test]
fn membership_against_another_carriers_domain_is_refused() {
    let source = std::fs::read_to_string(fixture("carrier_membership").join("main.omg")).unwrap();
    let (text, line, column) = edited(&source, "(x as u16)", 0, "x");
    refuses(
        "carrier_membership",
        "unconverted",
        &text,
        line,
        column,
        "this value has carrier `u8` but the domain classifies `u16` values",
    );
}

#[test]
fn two_members_with_one_name_are_refused_at_the_second() {
    let source = std::fs::read_to_string(fixture("member_namespace").join("main.omg")).unwrap();
    // A domain named like a case, and a second case of the same name.
    let (domain, line, column) = edited(&source, "Waiting", 0, "Ready");
    refuses(
        "member_namespace",
        "domain",
        &domain,
        line,
        column,
        "`Signal::Ready` is declared twice",
    );
    let (case, line, column) = edited(&source, "Idle;", 0, "Ready;");
    refuses(
        "member_namespace",
        "case",
        &case,
        line,
        column,
        "`Signal::Ready` is declared twice",
    );
}

#[test]
fn a_public_signature_naming_a_private_type_is_refused() {
    let source = std::fs::read_to_string(fixture("public_signature").join("main.omg")).unwrap();
    let (text, _, _) = edited(&source, "public data", 0, "data");
    // The refusal points at the parameter that names the type.
    let (_, line, column) = edited(&text, "shown: @Shown", 0, "");
    refuses(
        "public_signature",
        "private",
        &text,
        line,
        column,
        "names the private data declaration `Shown`",
    );
}
