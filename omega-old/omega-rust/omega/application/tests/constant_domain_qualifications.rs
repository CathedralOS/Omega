//! A constant keeps the domains its declared type puts on each element, but
//! it grants no authority (`wiki/spec/language/constants.md`). The evaluated
//! value holds only the elements' fields, so materialization rebuilds each
//! qualified element as `value as +D`, and the cast reader owes that domain's
//! ordinary issuer and predicate checks. A domain anyone may issue survives
//! the constant in both engines; one whose plan names an issuer is refused
//! at the constant, while the same `make()` called directly still runs.

use std::path::Path;
use std::process::{Command, Stdio};

fn run_both(fixture: &str) -> (bool, String) {
    let project = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures")
        .join(fixture);
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["run", "--both", "main.omg"])
        .current_dir(&project)
        .stdin(Stdio::null())
        .output()
        .expect("omega runs on the hosted target");
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    (output.status.success(), report)
}

#[test]
fn a_constant_keeps_a_domain_anyone_may_issue_on_each_element() {
    let (succeeded, report) = run_both("run/domains/constant_qualified_elements");
    assert!(succeeded, "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}

#[test]
fn a_constant_cannot_reissue_a_domain_its_plan_restricts() {
    let (succeeded, report) = run_both("fail/domains/constant_issued_elements");
    assert!(!succeeded, "{report}");
    assert!(
        report.contains(
            "constant `ISSUED` evaluation: a constant grants no authority, and \
             `Ticket::Stamped` restricts who may issue it"
        ),
        "{report}"
    );
}
