#[path = "support/declared_exit.rs"]
mod declared_exit;
mod support;


#[test]
fn a_match_with_a_prebound_reference_preserves_the_callers_record() {
    let project = support::repository_root()
        .join("source-rust/application/tests/fixtures/ownership/run/prebound_reference_match");
    declared_exit::assert_declared_exit(&project, "main.omg");
}
