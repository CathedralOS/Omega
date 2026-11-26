#[path = "support/program.rs"]
mod program;

#[test]
fn forwarding_an_extent_preserves_its_runtime_fields() {
    program::assert_silent_return(include_str!(
        "fixtures/contracts/run/forward_extent/main.omg"
    ));
}

#[test]
fn an_unserved_content_equation_points_at_the_mathematical_identity() {
    let (output, report) = program::invoke(
        include_str!("fixtures/contracts/fail/interval_content_identity/main.omg"),
        &["--check"],
    );
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("Identity over proof-only data in an executable contract")
            && report.contains("main.omg:6:"),
        "{report}"
    );
}
