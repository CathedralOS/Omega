//! An unlocked project can be interpreted and compiled (packages/locks.md):
//! its program entry comes from the standard library's default entry schema,
//! which entry selection picks as the single supplier without a lock binding.
//! Native production must admit that same schema rather than demand a binding
//! only a lock can supply.

#[path = "support/declared_exit.rs"]
mod declared_exit;
mod support;

#[test]
fn an_unlocked_system_entry_runs_when_interpreted_and_built() {
    let project =
        support::repository_root().join("tests/fixtures/calls/run/published_process_exit");
    assert!(
        !project.join("omega.lock").exists(),
        "the fixture must stay unlocked"
    );
    declared_exit::assert_declared_exit(&project, "build.omg");
}
