/*
Admit and bind a `Build`-dependency provider artifact to the suspended
activation's deferred boundary slots (`admit_request`, `prepare`).

When a `Build` activation pauses on `Build::select_provider` readiness,
`admit_request` checks that the compiled `ProducedBuildProvider` matches the
exact dependency backing path, requirement declaration identity, and type
application requested by the paused `BuildProviderRequest`. `prepare` then
retains the provider's code (`TerminalProgram::retain`), admits target machine
selections and layouts for the `Build` execution profile, resolves the unique
`provider_candidates` entry for each demanded boundary slot, completes any
scalar applications required by that exact slot selection (charging the live
`Build` invocation account and recording `SourceCompletionAccounting`), and
produces the `TerminalComponentBinding`s installed into `TerminalInstance`.
*/

use crate::host::bindings::BuildProviderRequest;
use build_time_evaluation::{ScalarScopeEvaluation, SourceCompletionAccounting};
use psi_compiler::ProducedBuildProvider;
use target::TargetProfile;
use terminal_codec::TerminalExecutionView;
use terminal_installation::{
    AdmittedComponentProvider, AdmittedTargetLayouts, AdmittedTargetMachineSelections,
    ProviderInstallationSelection, admit_component_provider, admit_target_layouts,
    admit_target_machine_selections,
};
use terminal_interpreter::{
    TerminalComponentBinding, TerminalProgram, TerminalRuntimeSupplyRequest,
};
use terminal_psi::TargetMachineCallable;

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
    profile: TargetProfile,
    account: &evaluation_accounting::EvaluationInvocationAccount,
    accounting: &mut SourceCompletionAccounting,
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
    let program = TerminalProgram::retain(artifact.retain_code());
    let targets = admit_target_machine_selections(artifact.view(), profile.target_name());
    let target_facts = profile
        .layout_target_facts()
        .ok_or("Build helper target has no representation facts")?;
    let layouts = admit_target_layouts(artifact.view(), &targets, &target_facts)
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
        let selected = ProviderInstallationSelection {
            boundary: candidate.boundary,
            provider_identity: module
                .identities
                .text(candidate.provider_identity)
                .ok_or("Build provider lost its callable identity")?
                .to_owned(),
            provider_declaration: provider.declaration().to_owned(),
            candidate: candidate.candidate,
        };
        let admitted = admit_component_provider(receiver, *boundary, artifact.view(), &selected)?;
        let supplies = runtime_supplies(&program, &admitted, &targets, &layouts)?;
        let requirements = program
            .execution_requirements(Some(&supplies))
            .map_err(|error| format!("Build helper demands: {error:?}"))?;
        let completed = build_time_evaluation::complete_scalar_applications(
            artifact,
            profile.target_name(),
            None,
            &requirements.scalar_applications,
            Some(account),
            accounting,
            |scope| {
                let scope_profile = TargetProfile::from_canonical_target_name(scope)
                    .map_err(|diagnostic| diagnostic.message)?;
                Ok(ScalarScopeEvaluation {
                    application_roots: None,
                    import_realizations: None,
                    layout_target: scope_profile
                        .layout_target_facts()
                        .ok_or("scalar evaluation target has no immutable layout catalogue")?,
                    targets: admit_target_machine_selections(artifact.view(), scope),
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
    admitted: &AdmittedComponentProvider,
    targets: &AdmittedTargetMachineSelections,
    layouts: &AdmittedTargetLayouts,
) -> Result<terminal_interpreter::TerminalRuntimeSupplies, String> {
    program
        .admit_runtime_supplies(TerminalRuntimeSupplyRequest {
            native_code_layouts: None,
            installation: Some(admitted.selection()),
            entry: Some(TargetMachineCallable::Machine(admitted.candidate())),
            host_imports: &[],
            x86_flags: None,
            memory_ordering: None,
            target_machines: Some(targets),
            target_layouts: Some(layouts),
        })
        .map_err(|error| format!("Build helper closure admission: {error:?}"))
}
