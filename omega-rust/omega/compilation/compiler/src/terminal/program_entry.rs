//! Bind an executable receipt to the accepted module's exact target entry.
//! Source choices and complete entry contracts were normalized before judgment.

use crate::CheckedCompilation;
use diagnostics::Diagnostic;
use program_entry_plan::SelectedProgramEntrySignature;
use source_compiler::{
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
    let publication = &custody.terminal_publication;
    let accepted = publication.accepted()?;
    let module = accepted.module();
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
    // Provider input preparation may already have published the shared portable
    // judgment. Bind target-specific folded facts to a fresh judgment of the
    // same normalized roster and exact source, before its own artifact exists.
    let mut replacement = products::ProductTerminalPublication::judge(
        std::sync::Arc::clone(accepted.source()),
        publication.roster()?.clone(),
        publication.optimizations().clone(),
    )?;
    let final_module = replacement.accepted()?.module();
    let final_root = final_module
        .program_entry_contract(profile.target_name())
        .map_err(|reason| vec![Diagnostic::error(reason)])?
        .entry;
    let folded = backend_plan::folded_constant_dependencies(
        final_module,
        profile.target_name(),
        custody.execution.plans().plans(),
        &[final_root],
    )
    .map_err(|reason| vec![Diagnostic::error(reason)])?;
    replacement.retain_folded_constants(folded)?;
    custody.terminal_publication = std::sync::Arc::new(replacement);
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
        &mut source_compiler::UnmeasuredProduction,
    )?;
    Ok((signature, application))
}
