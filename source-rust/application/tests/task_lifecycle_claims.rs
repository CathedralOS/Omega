//! A lifecycle operation consumes or borrows an issued claim for its result type.

#[path = "support/project.rs"]
mod project;

fn check(source: &str) -> (bool, String) {
    let project = project::stage("task-lifecycle-claim", &[("main.omg", source)]);
    let output = project::observe(project.path(), &["--check", "main.omg"]);
    (
        output.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
    )
}

const FORWARD: &str = r#"
use {omega}/language/core/task.omg;
machine complete(task: |task|::Task<u32> in |task|::Task::Live<u32>) -> |task|::TaskOutcome<u32>
reaches |task|::TaskLifecycle invokes |task|::TaskLifecycle suspends blocks crashes Trap crashes Abort {
    |task|::TaskLifecycle::request_cancel::<u32>(@task);
    suspend block |task|::TaskLifecycle::finish::<u32>(task)
}
machine main() {}
"#;

#[test]
fn an_issued_task_can_be_borrowed_then_consumed_by_its_lifecycle() {
    let (accepted, report) = check(FORWARD);
    assert!(accepted, "{report}");
}

#[test]
fn matching_task_fields_do_not_supply_an_activation() {
    let forged = FORWARD
        .replace("task: |task|::Task<u32> in |task|::Task::Live<u32>", "")
        .replace(
            "    |task|::TaskLifecycle::request_cancel",
            "    let task: |task|::Task<u32> = |task|::Task { provider: 1, activation: 7 };\n    |task|::TaskLifecycle::request_cancel",
        );
    let (accepted, report) = check(&forged);
    assert!(!accepted && report.contains("Task::Live"), "{report}");
    assert!(!report.contains("defect of the checker"), "{report}");
}

#[test]
fn a_numeric_task_cannot_issue_its_own_membership() {
    let (accepted, report) = check(
        r#"
use {omega}/language/core/task.omg;
machine mint() -> |task|::Task<u32> in |task|::Task::Live<u32> {
    let task: |task|::Task<u32> = |task|::Task { provider: 1, activation: 2 };
    task as +Live<u32>
}
machine main() {}
"#,
    );
    assert!(
        !accepted && report.contains("authorized issuer route"),
        "{report}"
    );
}

#[test]
fn a_same_spelled_machine_accepts_its_own_argument() {
    let (accepted, report) = check(
        r#"
use {omega}/language/core/task.omg;
data Token { id: u32; }
machine finish(token: Token) -> u32 { token.id }
machine main() {}
"#,
    );
    assert!(accepted, "{report}");
}

#[test]
fn a_generic_result_checks_the_claim_on_its_exact_payload() {
    let source = r#"
data Claim<T> { number: u64; }
domain<T> Claim<T>::Issued<T>;
data Outcome<T> { case Started(claim: Claim<T> in Issued<T>); case Rejected; }
boundary trait Producer { machine start<T>() -> Outcome<T> reaches none crashes none; }
machine select(witness: Claim<u32> in Claim::Issued<u32>) -> Outcome<u32> {
    Outcome::Started { claim: witness }
}
machine main() {}
"#;
    let (accepted, report) = check(source);
    assert!(accepted, "{report}");
    let (accepted, report) = check(&source.replace(
        "witness: Claim<u32> in Claim::Issued<u32>",
        "witness: Claim<u32>",
    ));
    assert!(!accepted && report.contains("Claim::Issued"), "{report}");
}
