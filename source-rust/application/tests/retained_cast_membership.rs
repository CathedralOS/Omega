//! Erasing a domain from a value's type leaves the established fact on that value.
// This binary uses only `invoke` from the shared helpers.
#[allow(dead_code)]
#[path = "support/program.rs"]
mod program;

fn inspect_cast(source: &str) -> (bool, String) {
    let (output, report) = program::invoke(source, &["psi", "inspect", "--machine", "carry"]);
    (output.status.success(), report)
}

fn assert_denied(source: &str, domain: &str) {
    let (success, report) = inspect_cast(source);
    assert!(!success, "{report}");
    assert!(
        report.contains(&format!("this cast adds `{domain}`")),
        "{report}"
    );
    let cast_line = source
        .lines()
        .position(|line| line.contains(" +") && line.contains(" as "))
        .expect("the control spells the cast")
        + 1;
    assert!(
        report.contains(&format!("main.omg:{cast_line}:")),
        "{report}"
    );
}

#[test]
fn a_vacancy_removed_from_the_extent_type_can_be_restored_to_the_same_extent() {
    let source = r#"use {omega}/language/core/extent.omg as extent;
machine carry(original: |extent|::Extent in |extent|::Extent::Granted & |extent|::Extent::Vacant)
    -> |extent|::Extent in |extent|::Extent::Granted & |extent|::Extent::Vacant
{
    let storage: |extent|::Extent in |extent|::Extent::Granted =
        original as = |extent|::Extent::Granted - |extent|::Extent::Vacant;
    storage as = |extent|::Extent::Granted + |extent|::Extent::Vacant
}
machine main() {}
"#;
    let (success, report) = inspect_cast(source);
    assert!(success, "{report}");

    let bare = source
        .replace("Granted & |extent|::Extent::Vacant)", "Granted)")
        .replace(
            "original as = |extent|::Extent::Granted - |extent|::Extent::Vacant;",
            "original;",
        );
    assert_denied(&bare, "Extent::Vacant");
}

#[test]
fn erasing_a_scalar_membership_preserves_its_fact_on_the_original_value() {
    let source = r#"domain u64::Stamped established by Source::issue;
boundary trait Source { machine issue() -> u64 in Stamped reaches none crashes none; }
machine carry(original: u64 in Stamped) -> u64 in Stamped {
    let plain: u64 = original as -Stamped;
    plain as +Stamped
}
machine main() {}
"#;
    let (success, report) = inspect_cast(source);
    assert!(success, "{report}");
    let bare = source
        .replace("original: u64 in Stamped", "original: u64")
        .replace("original as -Stamped;", "original;");
    assert_denied(&bare, "u64::Stamped");
}
