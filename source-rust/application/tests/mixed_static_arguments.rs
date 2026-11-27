//! Generic wrappers retain type, const and selected machine arguments.

#[path = "support/program.rs"]
mod program;
mod support;

fn fixture_path(tier: &str, sample: &str) -> std::path::PathBuf {
    support::repository_root()
        .join("source-rust/application/tests/fixtures/calls")
        .join(tier)
        .join(sample)
}

#[test]
fn mixed_static_arguments_run_when_interpreted_and_built() {
    program::assert_fixture_silent_return(&fixture_path("run", "mixed_static_arguments"));
}

#[test]
fn a_const_argument_still_owes_its_declared_requirement() {
    let (output, report) = program::check_fixture(&fixture_path("fail", "mixed_static_arguments"));
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("cannot prove requires contract for call keep from forward")
            && !report.contains("not supported yet"),
        "{report}"
    );
}

#[test]
fn nominal_callback_parameters_keep_their_distinct_selections() {
    program::assert_fixture_silent_return(&fixture_path("run", "nominal_static_arguments"));
}

#[test]
fn a_generic_trait_callback_is_judged_at_the_call_type_arguments() {
    program::assert_fixture_silent_return(&fixture_path("run", "generic_nominal_static_arguments"));
}

#[test]
fn a_callback_for_another_trait_argument_is_refused() {
    let (output, report) =
        program::check_fixture(&fixture_path("fail", "generic_nominal_static_arguments"));
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("callback `left_step` must satisfy the exact requirement of `Step`"),
        "{report}"
    );
}

#[test]
fn a_matching_signature_does_not_bind_a_callback_to_the_requirement() {
    let (output, report) =
        program::check_fixture(&fixture_path("fail", "nominal_static_arguments"));
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("callback `same_shape` must satisfy the exact requirement of `Right`"),
        "{report}"
    );
}
