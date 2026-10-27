//! Join a source-selected implementation to the waiting activation's static slots.

use crate::terminal_bindings::BuildProviderRequest;
use psi_compiler::ProducedBuildProvider;
use terminal_codec::TerminalExecutionView;
use terminal_interpreter::{
    TerminalComponentBinding, TerminalProgram, TerminalRuntimeSupplyRequest,
};

pub(super) fn prepare<'host, 'code>(
    receiver: TerminalExecutionView<'_>,
    request: &BuildProviderRequest,
    provider: &ProducedBuildProvider,
) -> Result<Vec<TerminalComponentBinding<'host, 'code>>, String> {
    let (_, declaration) = request
        .backing
        .path
        .split_once("::")
        .ok_or("Build backing must name its requested dependency and implementation")?;
    if declaration != provider.path() {
        return Err("prepared Build provider differs from the explicitly named backing".into());
    }
    let identities = &receiver.module().identities;
    if identities.text(request.requirement.declaration.identity()) != Some(provider.requirement())
        || identities.text(request.requirement.application) != Some(provider.application())
    {
        return Err(
            "prepared Build provider satisfies a different exact requirement application".into(),
        );
    }
    let Some(artifact) = provider.artifact() else {
        return if request.requirement.boundaries.is_empty() {
            Ok(Vec::new())
        } else {
            Err("Build provider has no executable code for its demanded slots".into())
        };
    };
    let module = artifact.view().module();
    let program = TerminalProgram::load_envelope(&artifact.to_bytes())
        .map_err(|error| format!("Build provider code admission: {error:?}"))?;
    let mut bindings = Vec::new();
    for boundary in &request.requirement.boundaries {
        let receiving = receiver
            .module()
            .boundary_machines
            .iter()
            .find(|row| row.id == *boundary)
            .ok_or("Build readiness names an absent receiving slot")?;
        let identity = identities
            .text(receiving.identity)
            .ok_or("Build slot lost its exact requirement identity")?;
        let mut candidates = module.provider_candidates.iter().filter(|candidate| {
            module
                .identities
                .text(candidate.provider_declaration.identity())
                == Some(provider.declaration())
                && module.identities.text(candidate.requirement_identity) == Some(identity)
        });
        let candidate = candidates
            .next()
            .ok_or("named Build provider has no checked candidate for a demanded slot")?;
        if candidates.next().is_some() {
            return Err(
                "named Build provider has ambiguous candidates for one demanded slot".into(),
            );
        }
        let selected = terminal_installation::ProviderInstallationSelection {
            boundary: candidate.boundary,
            provider_identity: module
                .identities
                .text(candidate.provider_identity)
                .ok_or("Build provider lost its callable identity")?
                .to_owned(),
            provider_declaration: provider.declaration().to_owned(),
            candidate: candidate.candidate,
        };
        let admitted = terminal_installation::admit_component_provider(
            receiver,
            *boundary,
            artifact.view(),
            &selected,
        )?;
        let supplies = program
            .admit_runtime_supplies(TerminalRuntimeSupplyRequest {
                installation: Some(admitted.selection()),
                entry: Some(terminal_psi::TargetMachineCallable::Machine(
                    admitted.candidate(),
                )),
                host_imports: &[],
                x86_flags: None,
                memory_ordering: None,
                target_machines: None,
                target_layouts: None,
            })
            .map_err(|error| format!("Build helper closure admission: {error:?}"))?;
        bindings.push(TerminalComponentBinding::new(
            admitted,
            program.clone(),
            supplies,
        )?);
    }
    Ok(bindings)
}
