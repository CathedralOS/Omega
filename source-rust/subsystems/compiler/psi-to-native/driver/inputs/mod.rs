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
use psi_target_layouts::admit_target_layouts;
use target::ExecutableImageEmissionRequest;

pub(crate) struct PreparedNativeCompilation<'input> {
    pub(crate) retained: &'input RetainedTerminalArtifact,
    pub(crate) request: NativeCompileRequest<'input>,
    pub(crate) recovery: ExecutableImageEmissionRequest,
    pub(crate) execution: PreparedNativeExecution,
    pub(crate) private_entry_ram: Vec<psi_entry_memory::CheckedPrivateEntryRamRecipe>,
    pub(crate) scalars: Option<psi_scalar_applications::AdmittedScalarApplications>,
}

pub(crate) fn prepare<'input>(
    retained: &'input RetainedTerminalArtifact,
    mut request: NativeCompileRequest<'input>,
) -> Result<PreparedNativeCompilation<'input>, (ExecutableImageEmissionRequest, Vec<Diagnostic>)> {
    let recovery = request.image_request.clone();
    let result = (|| {
        let (artifact, proposal) = request.prepare_terminal(retained)?;
        validate_installation_environment_inputs(
            artifact,
            proposal,
            request.installation_assumptions,
        )?;
        let selections = psi_target_layouts::terminal_target_context(
            artifact.view(),
            proposal.target_profile().target_name(),
        );
        let target_facts = proposal
            .target_profile()
            .layout_target_facts()
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "entry RAM target has no representation facts",
                )]
            })?;
        let layouts = admit_target_layouts(artifact.view(), &selections, &target_facts)
            .map_err(|reason| vec![Diagnostic::error(format!("entry RAM layout: {reason:?}"))])?;
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
                &layouts,
                entry,
                &psi_entry_memory::PrivateEntryRamRecipe {
                    input: row.clone(),
                    contents: initializer.contents.clone(),
                },
            )
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
            recipes.push(recipe);
        }
        let execution = provider_realization::native_execution::prepare_program(
            proposal,
            retained.authorization(),
        )?
        .with_operation_evidence(artifact, retained.evidence().cloned())?;
        let demand = execution.scalar_application_demand(proposal.target_profile())?;
        let scalars = scalars::complete(artifact, proposal, &demand, request.scalar_receipts)?;
        Ok((execution, recipes, scalars))
    })();
    match result {
        Ok((execution, private_entry_ram, scalars)) => Ok(PreparedNativeCompilation {
            retained,
            request,
            recovery,
            execution,
            private_entry_ram,
            scalars,
        }),
        Err(diagnostics) => Err((recovery, diagnostics)),
    }
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
