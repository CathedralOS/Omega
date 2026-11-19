//! `domain Ticket::Stamped access StampAccess;` leaves who may write
//! `ticket as +Stamped` to the plan `StampAccess::plan` returns
//! (`wiki/spec/language/domains.md`, "Domain-owned read and write
//! authority"). The Build runs that plan for the demanded domain, and the
//! lowered-form checker reads its routes: the machine the plan lists may cast,
//! and the twin whose plan lists another machine is refused for that.

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
fn the_machine_a_domain_plan_lists_may_issue_it() {
    let (succeeded, report) = run_both("run/domains/access_plan_issuer");
    assert!(succeeded, "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}

#[test]
fn a_machine_the_domain_plan_does_not_list_is_refused() {
    let (succeeded, report) = run_both("fail/domains/access_plan_other_issuer");
    assert!(!succeeded, "{report}");
    assert!(
        report.contains(
            "this cast adds `Ticket::Stamped` in a machine that is not one of the domain's \
             establishment routes"
        ) && report.contains("reading the body of `stamp`"),
        "{report}"
    );
}
