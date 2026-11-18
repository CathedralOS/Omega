//! Bind an executable receipt to the accepted module's exact target entry.
//! Source choices and complete entry contracts were normalized before judgment.

use diagnostics::Diagnostic;
use products::PublishedCompilation;
use program_entry_plan::SelectedProgramEntrySignature;
use terminal_installation::application::{
    ProducedTerminalApplication, TerminalApplicationCustody, TerminalCompositionRoots,
};

pub(crate) fn bind_folded_entry_dependencies(
    custody: &mut crate::execution_settlement::SelectedProviderCustody,
) -> Result<(), Vec<Diagnostic>> {
    let program = &custody.source_program;
    custody
        .execution
        .bind_folded_entry(
            program.artifact(),
            program.evidence().map(std::sync::Arc::as_ref),
        )
        .map_err(|reason| vec![Diagnostic::error(reason)])
}

pub fn prepare_program_entry(
    checked: &PublishedCompilation,
) -> Result<(SelectedProgramEntrySignature, ProducedTerminalApplication), Vec<Diagnostic>> {
    let program = checked.program();
    let selected =
        products::PublishedNativeEntry::for_program(program.artifact(), checked.execution())?;
    let signature = selected.source_signature().clone();
    let application = TerminalApplicationCustody::prepare(
        selected.nominal_root(),
        Some(signature.identity().bytes()),
        Some(TerminalCompositionRoots::default()),
    )
    .attach(
        program.artifact().view().into(),
        Ok(program.artifact()),
        program.machine_provenance(),
        &mut terminal_installation::application::UnmeasuredEntryReceipt,
    )?;
    Ok((signature, application))
}
