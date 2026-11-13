// Build selects an exact source machine, while the receipt belongs to the
// published artifact. Join those owners through the accepted elaboration's
// correspondence; neither a display name nor the module's first root selects
// an application's entry (entry_roots.md, "Source selection is not invocation").

use crate::CheckedCompilation;
use diagnostics::Diagnostic;
use program_entry_plan::SelectedProgramEntrySignature;
use psi_compiler::{
    ProducedTerminalApplication, TerminalApplicationCustody, TerminalCompositionRoots,
};

/// Prepare the selected target's entry receipt on its already published code.
/// Calling plans and installation authority remain separate admissions.
pub fn prepare_program_entry(
    checked: &CheckedCompilation,
) -> Result<(SelectedProgramEntrySignature, ProducedTerminalApplication), Vec<Diagnostic>> {
    let publication = checked.terminal_publication();
    publication
        .require_source_owner(checked.source_inspection().program())
        .map_err(|error| vec![error])?;
    let accepted = publication.accepted()?;
    let profile = checked
        .execution()
        .target_profile()
        .unwrap_or_else(target::TargetProfile::host);
    let selected = provider_planning::program_entry::selected_program_entry_machine(
        &checked.build().config.root_bindings,
        Some(profile),
    )?
    .ok_or_else(|| {
        vec![Diagnostic::error(
            "selected target has no bound program entry",
        )]
    })?;

    let typed = accepted.source();
    let mut symbol = selected.machine_symbol;
    if let Some(family) = typed.target_machine_families.iter().find(|family| {
        family.canonical == symbol || family.members.iter().any(|member| member.machine == symbol)
    }) {
        symbol = family
            .members
            .iter()
            .find(|member| member.target.as_str() == profile.target_name())
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "entry family `{}` has no body for target `{}`",
                    family.name,
                    profile.target_name()
                ))]
            })?
            .machine;
    }
    let mut machines = accepted
        .closure()
        .elaboration
        .machines
        .iter()
        .filter_map(|(machine, source)| (*source == symbol).then_some(machine));
    let machine = machines.next().ok_or_else(|| {
        vec![Diagnostic::error(
            "selected program entry is absent from its accepted source closure",
        )]
    })?;
    if machines.next().is_some() {
        return Err(vec![Diagnostic::error(
            "selected program entry has competing accepted machine correspondences",
        )]);
    }
    let program = publication.require_published(publication.optimizations())?;
    let module = program.artifact().semantic_module();
    if module != accepted.module() {
        return Err(vec![Diagnostic::error(
            "published program entry differs from its accepted module",
        )]);
    }
    let signature = SelectedProgramEntrySignature::from_module_entry(
        selected.slot,
        module,
        machine,
        selected.machine_name,
    )
    .map_err(|reason| vec![Diagnostic::error(format!("program entry: {reason}"))])?;
    let root = terminal_psi::PreparedNominalRoot::new(
        signature.declaration(),
        selected.machine_name,
        None,
    )
    .map_err(|reason| vec![Diagnostic::error(reason.to_string())])?;
    let application = program.attach_application(
        &TerminalApplicationCustody::prepare(
            Ok(root),
            Some(signature.identity().bytes()),
            Some(TerminalCompositionRoots::default()),
        ),
        &mut psi_compiler::UnmeasuredProduction,
    )?;
    Ok((signature, application))
}
