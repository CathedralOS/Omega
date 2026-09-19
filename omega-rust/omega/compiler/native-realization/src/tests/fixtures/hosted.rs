//! Hosted ProgramEntry artifact, terminal receipt, and source-signature custody.

use super::checked_source::checked;
use terminal_psi::CheckedProgramEntryTerminalReceipt;

/// Minimal hosted program used to evaluate a profile's real `ProgramEntry`
/// calling plans: a free `launch` with no visible parameters satisfies every
/// hosted `ProgramEntry` slot shape.
const MINIMAL_HOSTED_SOURCE: &str = "data Main {}\nmachine Main::launch() {}\n";

/// Compile `source` as an application whose build file binds `profile`'s
/// `ProgramEntry` root slot to `Main::launch`. Only hosted-schema profiles
/// admit this free, parameterless entry; freestanding slots such as UEFI
/// require their authored storage parameters instead.
fn compile_hosted_entry(
    source: &str,
    profile: target::TargetProfile,
) -> assembled_syntax_to_checked_compilation::CheckedCompilation {
    let dir = std::env::temp_dir().join(format!(
        "omega-native-realization-hosted-{}-{}",
        std::process::id(),
        NEXT_COMPILE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create hosted entry fixture directory");
    std::fs::write(
        dir.join("build.omg"),
        format!(
            "machine build(builder: &mut Build) {{\n    builder.application(\"hosted-entry-fixture\");\n    builder.roots.bind({}::ProgramEntry, Main::launch);\n}}\n",
            profile.root_slot_owner_name(),
        ),
    )
    .expect("write hosted entry fixture build");
    std::fs::write(dir.join("main.omg"), source).expect("write hosted entry fixture source");
    let checked = assembled_syntax_to_checked_compilation::compile_to_checked(
        assembled_syntax_to_checked_compilation::CheckedCompileRequest::new(
            &dir.join("main.omg"),
            Some(profile.target_name()),
        ),
    )
    .map_err(|diagnostics| {
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.clone())
            .collect::<Vec<_>>()
            .join("\n")
    });
    let _ = std::fs::remove_dir_all(&dir);
    checked.expect("hosted entry fixture compiles")
}

static NEXT_COMPILE_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The build-evaluated paired calling plans a hosted `profile`'s `ProgramEntry`
/// slot declares. These are slot-level custody, not source-level: settlement
/// pairs them with a hosted source signature whose visible-parameter shape the
/// slot admits, so one minimal compile supplies every fixture on the profile.
pub(crate) fn hosted_calling_plans(
    profile: target::TargetProfile,
) -> build_evaluation::SelectedProgramEntryCallingPlans {
    compile_hosted_entry(MINIMAL_HOSTED_SOURCE, profile)
        .selected_program_entry()
        .expect("hosted fixture selects a ProgramEntry")
        .calling_plans()
        .expect("hosted fixture retains paired calling plans")
        .clone()
}

/// View an evaluated pair of calling plans the way
/// `NativeProgramEntrySettlement::new` consumes them.
pub(crate) fn paired_calling_plan_parts(
    plans: &build_evaluation::SelectedProgramEntryCallingPlans,
) -> (
    &provider_planning::calling_policy_plans::BoundaryCallingPlanRealization,
    &provider_planning::calling_policy_plans::BoundaryCallingPlanRealization,
    &program_entry_plan::SelectedProgramStorageEntryPlan,
) {
    (
        &plans.semantic_calling_application,
        &plans.physical_calling_application,
        &plans.storage_entry,
    )
}

pub(in crate::tests) fn hosted_custody() -> (
    terminal_codec::CanonicalTerminalArtifact,
    CheckedProgramEntryTerminalReceipt,
    program_entry_plan::SelectedProgramEntrySourceSignature,
    build_evaluation::SelectedProgramEntryCallingPlans,
) {
    let checked = checked(
        r#"
            data Main {}
            machine Main::launch() {}
        "#,
    );
    let selection = checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .find(|machine| machine.name == "Main::launch")
        .expect("terminal selection");
    let source = program_entry_plan::SelectedProgramEntrySourceSignature::from_checked_typed_entry(
        target::TargetProfile::WindowsX64.program_entry_slot(),
        selection.machine,
        selection.machine,
        selection.name.clone(),
        "entry".into(),
        "test::Main::launch() -> Unit".into(),
        program_entry_plan::ProgramEntrySourceReceiverSignature::Free,
        Vec::new(),
    )
    .expect("hosted source signature");
    let produced = terminal_production::TerminalProductionRequest::new(&checked, "Main::launch")
        .produce_program_entry(source.identity().bytes())
        .expect("ProgramEntry Terminal artifact");
    let (
        artifact,
        receipt,
        _,
        selected_ieee_float_fma_occurrences,
        selected_ieee_float_comparison_occurrences,
        selected_integer_comparison_occurrences,
    ) = produced.into_parts();
    assert!(selected_ieee_float_fma_occurrences.is_empty());
    assert!(selected_ieee_float_comparison_occurrences.is_empty());
    assert!(selected_integer_comparison_occurrences.is_empty());
    (
        artifact,
        receipt,
        source,
        hosted_calling_plans(target::TargetProfile::WindowsX64),
    )
}

/// The production artifact route joins a selected entry's retained paired
/// plans to its checked receipt: settlement must replay them exactly.
#[test]
fn produced_hosted_artifact_settles_with_selected_paired_plans() {
    let checked = compile_hosted_entry(MINIMAL_HOSTED_SOURCE, target::TargetProfile::WindowsX64);
    let entry = checked
        .selected_program_entry()
        .expect("selected program entry")
        .clone();
    let produced =
        checked_compilation_to_terminal_artifact::produce_program_entry_terminal_artifact(
            &checked,
            &entry,
            checked.optimization_selections(),
        )
        .expect("program-entry Terminal artifact");
    let (artifact, receipt, _scope, _coverage) = produced.into_parts();
    let (source, plans, fused) = entry.into_parts();
    crate::validate_native_program_entry_settlement(
        &artifact,
        &receipt,
        crate::NativeProgramEntrySettlement::new(
            &source,
            plans.as_ref().map(paired_calling_plan_parts),
            &fused,
        )
        .with_checked_entry(&receipt),
        target::NativeTarget::windows_x64(),
    )
    .expect("selected paired plans settle the produced hosted artifact");
}
