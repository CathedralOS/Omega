//! Hosted ProgramEntry artifact, terminal receipt, and source-signature custody.

use super::checked_source::checked;
use checked_trees_to_terminal_psi::CheckedProgramEntryTerminalReceipt;

pub(in crate::tests) fn hosted_custody() -> (
    terminal_codec::CanonicalTerminalArtifact,
    CheckedProgramEntryTerminalReceipt,
    program_entry_plan::SelectedProgramEntrySourceSignature,
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
    let produced = checked_trees_to_terminal_psi::produce_program_entry_terminal_artifact(
        &checked,
        "Main::launch",
        source.identity().bytes(),
    )
    .expect("ProgramEntry Terminal artifact");
    let (artifact, receipt, _, selected_ieee_float_fma_occurrences) = produced.into_parts();
    assert!(selected_ieee_float_fma_occurrences.is_empty());
    (artifact, receipt, source)
}
