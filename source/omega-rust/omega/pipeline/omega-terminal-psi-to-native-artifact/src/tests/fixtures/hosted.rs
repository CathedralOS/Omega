//! Hosted ProgramEntry artifact, terminal receipt, and source-signature custody.

use super::checked_source::checked;
use psi_checked_trees_to_terminal::CheckedProgramEntryTerminalReceipt;

pub(in crate::tests) fn hosted_custody() -> (
    psi_terminal_codec::CanonicalTerminalArtifact,
    CheckedProgramEntryTerminalReceipt,
    omega_program_entry_plan::SelectedProgramEntrySourceSignature,
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
    let source =
        omega_program_entry_plan::SelectedProgramEntrySourceSignature::from_checked_typed_entry(
            omega_target::TargetProfile::WindowsX64.program_entry_slot(),
            selection.machine,
            selection.machine,
            selection.name.clone(),
            "entry".into(),
            "test::Main::launch() -> Unit".into(),
            omega_program_entry_plan::ProgramEntrySourceReceiverSignature::Free,
            Vec::new(),
        )
        .expect("hosted source signature");
    let produced = psi_checked_trees_to_terminal::produce_program_entry_terminal_artifact(
        &checked,
        "Main::launch",
        source.identity().bytes(),
    )
    .expect("ProgramEntry Terminal artifact");
    let (artifact, receipt, _) = produced.into_parts();
    (artifact, receipt, source)
}
