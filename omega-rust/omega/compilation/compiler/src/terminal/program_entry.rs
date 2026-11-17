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
    let profile = custody.execution.target_profile().ok_or_else(|| {
        vec![Diagnostic::error(
            "executable entry metadata requires an explicit deployment target",
        )]
    })?;
    let publication = &custody.terminal_publication;
    // Source settlement already judged the selected entry schema and policy.
    // This step reads the published contract and adds no declaration demand.
    let program = publication.require_published(publication.optimizations())?;
    let module = program.artifact().semantic_module();
    let root = module
        .program_entry_contract(profile.target_name())
        .map_err(|reason| vec![Diagnostic::error(reason)])?
        .entry;
    let folded = backend_plan::folded_constant_dependencies(
        module,
        profile.target_name(),
        custody.execution.plans().plans(),
        &[root],
    )
    .map_err(|reason| vec![Diagnostic::error(reason)])?;
    if folded.dependencies.is_empty() {
        return Ok(());
    }
    let replacement = publication.with_folded_constants(folded)?;
    let program = replacement.require_published(replacement.optimizations())?;
    custody.execution = custody
        .execution
        .with_folded_program(program.artifact().view())
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    custody.terminal_publication = std::sync::Arc::new(replacement);
    Ok(())
}

pub fn prepare_program_entry(
    checked: &PublishedCompilation,
) -> Result<(SelectedProgramEntrySignature, ProducedTerminalApplication), Vec<Diagnostic>> {
    let selected = super::entry_planning::selected_program_entry(checked)?;
    let program = checked.program();
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
