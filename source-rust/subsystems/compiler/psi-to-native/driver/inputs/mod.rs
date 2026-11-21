//! Gather the inputs needed by the body and runtime drivers. Stage 01 selects
//! the root execution. Entry memory, requested installation assumptions
//! and completed scalar values are checked against the retained publication and target.

pub(crate) mod published_application;
mod scalars;

use provider_realization::native_execution::PreparedNativeExecution;
use verified_optimization_input::{RetainedTerminalArtifact, TerminalNativeRealizationProposal};
use super::request::NativeCompileRequest;
use diagnostics::Diagnostic;
use installation_conditions::InstallationEnvironmentAssumption;

pub(crate) struct PreparedNativeCompilation<'input> {
    pub(crate) retained: &'input RetainedTerminalArtifact,
    pub(crate) request: NativeCompileRequest<'input>,
    pub(crate) execution: PreparedNativeExecution,
    pub(crate) private_entry_ram: Vec<psi_entry_memory::CheckedPrivateEntryRamRecipe>,
    pub(crate) scalars: Option<psi_scalar_applications::AdmittedScalarApplications>,
}

pub(crate) fn prepare<'input>(
    retained: &'input RetainedTerminalArtifact,
    request: NativeCompileRequest<'input>,
) -> Result<PreparedNativeCompilation<'input>, Vec<Diagnostic>> {
    let artifact = retained.shared_artifact();
    let proposal = retained.native_realization_proposal();
    validate_installation_environment_inputs(
        artifact,
        proposal,
        request.installation_assumptions,
    )?;
    let execution = provider_realization::native_execution::prepare_program(
        proposal,
        retained.authorization(),
    )?
    .with_operation_evidence(artifact, artifact.published_evidence().cloned())?;
    let entry = proposal.program_entry().checked_entry().terminal_entry();
    let mut recipes = Vec::with_capacity(request.private_entry_ram.len());
    for initializer in request.private_entry_ram {
        let row = artifact
            .semantic_module()
            .access.access_view_inputs
            .iter()
            .find(|row| row.machine == entry && row.position == initializer.position)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "private entry RAM initializer answers no declared entry position",
                )]
            })?;
        let recipe = psi_entry_memory::admit_private_entry_ram(
            artifact.view(),
            execution.scope().layouts(),
            entry,
            &psi_entry_memory::PrivateEntryRamRecipe {
                input: row.clone(),
                contents: initializer.contents.clone(),
            },
        )
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
        recipes.push(recipe);
    }
    let scalars = scalars::complete(
        artifact,
        proposal,
        execution.scope().scalar_application_demand(),
        request.scalar_receipts,
    )?;
    Ok(PreparedNativeCompilation {
        retained,
        request,
        execution,
        private_entry_ram: recipes,
        scalars,
    })
}

fn validate_installation_environment_inputs(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    proposal: &TerminalNativeRealizationProposal,
    assumptions: &[InstallationEnvironmentAssumption],
) -> Result<(), Vec<Diagnostic>> {
    let fail = |reason: &str| {
        vec![Diagnostic::error(format!(
            "installation environment assumption: {reason}"
        ))]
    };
    let module = artifact.semantic_module();
    let mut bootstrap = std::collections::BTreeSet::new();
    let mut delivery = std::collections::BTreeSet::new();
    for assumption in assumptions {
        assumption.validate_description().map_err(fail)?;
        if assumption.psi() != artifact.view().identity()
            || assumption.profile() != proposal.target_profile()
        {
            return Err(fail("program or target differs from the reviewed interval"));
        }
        let entry = proposal.program_entry().checked_entry().terminal_entry();
        if assumption.entry() != entry
            && !proposal
                .occurrences()
                .callbacks
                .iter()
                .any(|row| row.callback_thunk_artifact().entry() == assumption.entry())
        {
            return Err(fail(
                "entry is neither the selected program entry nor an advertised private entry",
            ));
        }
        if !module
            .interfaces.installed_interrupt_registrations
            .iter()
            .any(|row| row.operation == assumption.installation())
        {
            return Err(fail("interval names no retained interrupt installation"));
        }
        let operations = match assumption {
            InstallationEnvironmentAssumption::Bootstrap(_) => &mut bootstrap,
            InstallationEnvironmentAssumption::InterruptDelivery(_) => &mut delivery,
        };
        if !operations.insert((assumption.entry(), assumption.installation())) {
            return Err(fail("installation repeats one kind of platform assumption"));
        }
    }
    Ok(())
}
