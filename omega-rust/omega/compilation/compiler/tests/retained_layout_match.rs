//! A package that depends on `std` checks `std`'s product program twice in
//! one compilation: once to produce `std`'s Build provider, then again for its
//! own product. The second check already reuses the first judgment, but it
//! used to evaluate every fixed-layout application under all target scopes
//! first, only to arrive at the program it had already judged. That cost about
//! seven seconds per compilation. This witness pins the reuse taking place
//! before evaluation.
//!
//! The counters are process-global, so this target holds exactly one test.

#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use fixture_package_inputs::{
    discover_fixture, fixture_build_snapshot, produce_fixture, repo_root, review_fixture,
};

const FIXTURE: &str = "tests/fixtures/pass/write_only/state_forwarded_scalar_store_exit/main.omg";

#[test]
fn a_repeated_std_product_check_is_reused_before_layout_evaluation() {
    let target_name = "linux_x86_64";
    let main = repo_root().join(FIXTURE);
    let discovered = discover_fixture(
        &main,
        package_compilation::BuildExecutionInputs {
            snapshot: fixture_build_snapshot(&main)
                .unwrap_or_else(|diagnostics| panic!("{FIXTURE} snapshot: {diagnostics:#?}")),
            ..Default::default()
        },
    )
    .unwrap_or_else(|diagnostics| panic!("{FIXTURE} Build: {diagnostics:#?}"));
    let profile = target::TargetProfile::from_omega_target_name(Some(target_name))
        .unwrap_or_else(|error| panic!("{error:?}"));
    let whole_before = compiler::whole_program_reuse_hits();
    let layout_before = compiler::retained_layout_match_hits();
    let mut prepared = review_fixture(
        discovered,
        &[profile],
        compiler::CompilationProduct::Terminal(Default::default()),
    )
    .unwrap_or_else(|diagnostics| panic!("{FIXTURE} review: {diagnostics:#?}"));
    produce_fixture(&mut prepared, Some(target_name)).unwrap_or_else(|diagnostics| {
        panic!("{FIXTURE} must reach Terminal Psi: {diagnostics:#?}")
    });
    let layout_hits = compiler::retained_layout_match_hits() - layout_before;
    let whole_hits = compiler::whole_program_reuse_hits() - whole_before;
    assert!(
        layout_hits >= 1,
        "the repeated std product check evaluated its layouts again \
         ({whole_hits} whole-program reuses, none before evaluation)"
    );
    assert!(
        layout_hits <= whole_hits,
        "a reuse before evaluation is still a whole-program reuse"
    );
}
