//! Sibling helpers retain their enclosing static arguments.

#[path = "support/program.rs"]
mod program;
mod support;

fn fixture_path(tier: &str, fixture_name: &str) -> std::path::PathBuf {
    support::repository_root()
        .join("source-rust/application/tests/fixtures/calls")
        .join(tier)
        .join(fixture_name)
}

#[test]
fn inherited_static_siblings_run_in_both_engines() {
    assert_runs_in_both_engines("inherited_static_siblings");
}

#[test]
fn selected_trait_defaults_keep_their_lexical_helpers() {
    assert_runs_in_both_engines("trait_default_helpers");
}

#[test]
fn selected_maps_pass_their_own_private_callback() {
    assert_runs_in_both_engines("private_map_callback");
}

fn assert_runs_in_both_engines(fixture: &str) {
    program::assert_fixture_silent_return(&fixture_path("run", fixture));
}

#[test]
fn a_const_argument_still_owes_its_declared_requirement() {
    let (output, report) =
        program::check_fixture(&fixture_path("fail", "inherited_static_siblings"));
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("cannot prove requires contract for call forward from main")
            && !report.contains("not supported yet"),
        "{report}"
    );
}

#[test]
fn default_helpers_cannot_capture_an_enclosing_value() {
    let (output, report) = program::check_fixture(&fixture_path("fail", "trait_default_helpers"));
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("cannot capture enclosing runtime binding `value`"),
        "{report}"
    );
}

#[test]
fn a_private_map_callback_must_accept_every_input_its_binder_allows() {
    let (output, report) = program::check_fixture(&fixture_path("fail", "private_map_callback"));
    assert!(!output.status.success(), "{report}");
    assert!(
        report
            .matches("cannot prove requires contract for call local from invoke: false")
            .count()
            == 1
            && report
                .matches("cannot prove the input precondition of `local`")
                .count()
                == 1
            && !report.contains("defect of the checker")
            && !report.contains("not supported yet"),
        "{report}"
    );
}
