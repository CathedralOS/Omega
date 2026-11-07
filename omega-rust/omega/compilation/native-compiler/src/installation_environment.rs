//! Bind explicit platform assumptions to the actual retained product.
use diagnostics::Diagnostic;
use installation_evidence::InstallationEnvironmentAssumption;

pub(crate) fn validate_inputs(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    proposal: &terminal_psi_to_abstract_operations::TerminalNativeRealizationProposal,
    assumptions: &[InstallationEnvironmentAssumption],
) -> Result<(), Vec<Diagnostic>> {
    let fail = |reason: &str| {
        vec![Diagnostic::error(format!(
            "installation environment assumption: {reason}"
        ))]
    };
    let module = artifact.semantic_module();
    let mut bootstrap = semantic_vocabulary::IdSet::default();
    let mut delivery = semantic_vocabulary::IdSet::default();
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
            .installed_interrupt_registrations
            .iter()
            .any(|row| row.operation == assumption.installation())
        {
            return Err(fail("interval names no retained interrupt installation"));
        }
        let operations = match assumption {
            InstallationEnvironmentAssumption::Bootstrap(_) => &mut bootstrap,
            InstallationEnvironmentAssumption::InterruptDelivery(_) => &mut delivery,
        };
        if !operations.insert(assumption.installation()) {
            return Err(fail("installation repeats one kind of platform assumption"));
        }
    }
    Ok(())
}
