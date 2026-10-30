//! Join a source-selected implementation to the waiting activation's static slots.

use crate::terminal_bindings::BuildProviderRequest;
use psi_compiler::ProducedBuildProvider;
use terminal_codec::TerminalExecutionView;
use terminal_interpreter::{
    TerminalComponentBinding, TerminalProgram, TerminalRuntimeSupplyRequest,
};

pub(super) fn admit_request(
    receiver: TerminalExecutionView<'_>,
    request: &BuildProviderRequest,
    provider: &ProducedBuildProvider,
) -> Result<(), String> {
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
    Ok(())
}

pub(super) fn prepare<'host, 'code>(
    receiver: TerminalExecutionView<'_>,
    request: &BuildProviderRequest,
    provider: &ProducedBuildProvider,
    profile: target::TargetProfile,
    account: &evaluation_accounting::EvaluationInvocationAccount,
    accounting: &mut build_time_evaluation::SourceCompletionAccounting,
) -> Result<Vec<TerminalComponentBinding<'host, 'code>>, String> {
    let identities = &receiver.module().identities;
    let Some(artifact) = provider.artifact() else {
        return if request.requirement.boundaries.is_empty() {
            Ok(Vec::new())
        } else {
            Err("Build provider has no executable code for its demanded slots".into())
        };
    };
    let module = artifact.view().module();
    // Keep freshly produced code alive after helper preparation is released;
    // decoding is an external-byte ingress, not a same-session ownership transfer.
    let program = TerminalProgram::retain(artifact.retain_code());
    let targets = terminal_installation::admit_target_machine_selections(
        artifact.view(),
        profile.target_name(),
    );
    let layouts = terminal_installation::admit_target_layouts(artifact.view(), &targets)
        .map_err(|error| format!("Build helper layouts: {error:?}"))?;
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
        let supplies = runtime_supplies(&program, &admitted, &targets, &layouts)?;
        let requirements = program
            .execution_requirements(Some(&supplies))
            .map_err(|error| format!("Build helper demands: {error:?}"))?;
        // Complete against this exact slot selection, never the dependency's
        // Product defaults or a union of independently admitted selections.
        let completed = build_time_evaluation::complete_scalar_applications(
            artifact,
            profile.target_name(),
            &requirements.scalar_applications,
            Some(account),
            accounting,
            |scope| {
                let scope_profile = target::TargetProfile::from_canonical_target_name(scope)
                    .map_err(|diagnostic| diagnostic.message)?;
                Ok(build_time_evaluation::ScalarScopeEvaluation {
                    layout_target: scope_profile
                        .layout_target_facts()
                        .ok_or("scalar evaluation target has no immutable layout catalogue")?,
                    targets: terminal_installation::admit_target_machine_selections(
                        artifact.view(),
                        scope,
                    ),
                    providers: admitted.selection().clone(),
                    address_model: Ok(target::TargetSemantics::close_address_model(scope_profile)
                        .map(|model| terminal_installation::ScalarAddressModel {
                            target: scope.to_owned(),
                            carrier: model.carrier(),
                            semantics_version: model.semantics_version(),
                            commitment: model.commitment(),
                        })),
                    carrier_layout: scope_profile
                        .native_realization()
                        .map(runtime_abi::target_carrier_layout),
                })
            },
        )
        .map_err(|reason| format!("Build helper scalar completion: {reason}"))?;
        let supplies = runtime_supplies(
            &program,
            &admitted,
            completed.primary().targets(),
            completed.primary().layouts(),
        )?;
        bindings.push(TerminalComponentBinding::new(
            admitted,
            program.clone(),
            supplies,
            Some(std::borrow::Cow::Owned(completed)),
        )?);
    }
    Ok(bindings)
}

fn runtime_supplies(
    program: &TerminalProgram<'_>,
    admitted: &terminal_installation::AdmittedComponentProvider,
    targets: &terminal_installation::AdmittedTargetMachineSelections,
    layouts: &terminal_installation::AdmittedTargetLayouts,
) -> Result<terminal_interpreter::TerminalRuntimeSupplies, String> {
    program
        .admit_runtime_supplies(TerminalRuntimeSupplyRequest {
            installation: Some(admitted.selection()),
            entry: Some(terminal_psi::TargetMachineCallable::Machine(
                admitted.candidate(),
            )),
            host_imports: &[],
            x86_flags: None,
            memory_ordering: None,
            target_machines: Some(targets),
            target_layouts: Some(layouts),
        })
        .map_err(|error| format!("Build helper closure admission: {error:?}"))
}
