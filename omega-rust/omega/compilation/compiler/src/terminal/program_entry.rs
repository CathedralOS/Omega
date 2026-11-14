//! Bind an executable receipt to the accepted module's exact target entry.
//! Source choices and complete entry contracts were normalized before judgment.

use crate::CheckedCompilation;
use diagnostics::Diagnostic;
use program_entry_plan::SelectedProgramEntrySignature;
use psi_compiler::{
    ProducedTerminalApplication, TerminalApplicationCustody, TerminalCompositionRoots,
};

pub(crate) fn prepare_entry_metadata(
    custody: &mut crate::execution_settlement::SelectedProviderCustody,
    _inputs: Option<&package_compilation::PackageCompilationInputs>,
) -> Result<(), Vec<Diagnostic>> {
    let profile = custody.execution.target_profile().ok_or_else(|| {
        vec![Diagnostic::error(
            "executable entry metadata requires an explicit deployment target",
        )]
    })?;
    let accepted = custody.terminal_publication.accepted()?;
    accepted
        .module()
        .program_entry_contract(profile.target_name())
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    Ok(())
}

pub fn prepare_program_entry(
    checked: &CheckedCompilation,
) -> Result<(SelectedProgramEntrySignature, ProducedTerminalApplication), Vec<Diagnostic>> {
    let publication = checked.terminal_publication();
    publication
        .require_source_owner(checked.source_inspection().program())
        .map_err(|error| vec![error])?;
    let selected = checked.selected_program_entry()?;
    let program = publication.require_published(publication.optimizations())?;
    let signature = selected.source_signature().clone();
    let application = program.attach_application(
        &TerminalApplicationCustody::prepare(
            selected.nominal_root(),
            Some(signature.identity().bytes()),
            Some(TerminalCompositionRoots::default()),
        ),
        &mut psi_compiler::UnmeasuredProduction,
    )?;
    Ok((signature, application))
}
