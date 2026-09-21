//! The UEFI x86-64 Boot Services geometry is authored once in the selected
//! target package (`source/library/std/targets/uefi_x86_64/tables.omg`) as the
//! `EfiBootServices` schema plus its evaluated `EfiBootServicesViewLayout::plan`
//! policy. This test is the independent replay that keeps the Rust evidence
//! honest: it evaluates the authored policy end to end, binds the result to
//! the recorded source-minted commitments, and proves the residual literal
//! recipe used by fixtures below the build layer stays byte-identical to the
//! evaluated plan.

use super::fixture_roster;
use crate::{compile_reviewed_repository_fixture, pass_canary};
use build_time_evaluation::compute_layout_plan;
use compiler::CheckedCompileRequest;
use layout_plans::normalized_layout_plan_report_fingerprint;
use target::{
    UEFI_X64_BOOT_SERVICES_LAYOUT_PLAN_COMMITMENT,
    UEFI_X64_BOOT_SERVICES_SCHEMA_REPORT_FINGERPRINT,
    exact_uefi_x64_boot_services_layout_plan_report, exact_uefi_x64_boot_services_native_layout,
    replayed_uefi_x64_boot_services_native_layout,
};

#[test]
fn authored_uefi_boot_services_layout_policy_replays_into_the_retained_layout() {
    let canary = pass_canary(fixture_roster::BUILD_UEFI_PROGRAM_ENTRY_STORAGE_ROOTS);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        Some("uefi_x86_64"),
    ))
    .expect("uefi storage-roots canary should compile");
    let report = compute_layout_plan(
        &checked.typed,
        "EfiBootServicesViewLayout::plan",
        "EfiBootServicesView",
        None,
    )
    .expect("the authored EfiBootServicesViewLayout policy evaluates");
    assert_eq!(
        report.schema_report_fingerprint, UEFI_X64_BOOT_SERVICES_SCHEMA_REPORT_FINGERPRINT,
        "the authored EfiBootServices schema drifted from the recorded coordinate"
    );
    assert_eq!(
        normalized_layout_plan_report_fingerprint(&report),
        UEFI_X64_BOOT_SERVICES_LAYOUT_PLAN_COMMITMENT,
        "the evaluated Boot Services layout plan drifted from the recorded commitment"
    );
    let exact = exact_uefi_x64_boot_services_native_layout();
    let replayed = replayed_uefi_x64_boot_services_native_layout(&report)
        .expect("the evaluated Boot Services plan replays");
    assert!(replayed.matches_exact_plan(&exact));
    assert_eq!(
        report,
        exact_uefi_x64_boot_services_layout_plan_report(),
        "the residual literal recipe must stay byte-identical to the evaluated plan"
    );
}
