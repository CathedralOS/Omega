#[path = "support/declared_exit.rs"]
mod declared_exit;
mod support;


#[test]
fn a_shared_record_reference_returns_to_the_callers_original_record() {
    let project = support::repository_root()
        .join("tests/fixtures/ownership/run/incoming_record_reference_return");
    declared_exit::assert_declared_exit(&project, "main.omg");
}
