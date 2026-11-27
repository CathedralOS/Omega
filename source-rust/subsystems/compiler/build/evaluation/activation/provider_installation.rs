//! Check the caller-supplied provider against each authored readiness operation before
//! Build entry, then install its checked component into the waiting interpreter.

use crate::host::bindings::BuildProviderRequest;
use build_time_evaluation::scalars::{ScalarScopeEvaluation, prepare_scalar_applications};
use psi_provider_selections::ProviderInstallationSelection;
use psi_provider_selections::{AdmittedComponentProvider, admit_component_provider};
use psi_target_layouts::{
    AdmittedTargetLayouts, TerminalTargetContext, admit_target_layouts, terminal_target_context,
};
use source_compiler::compile_time_invocations::SourceCompletionAccounting;
use source_compiler::publication::ProducedBuildProvider;
use target::TargetProfile;
use terminal_codec::TerminalExecutionView;
use terminal_interpreter::{
    TerminalComponentBinding, TerminalProgram, TerminalRuntimeSupplyRequest,
};
use terminal_psi::TargetMachineCallable;

/// Resolve each authored backing in its own file's Build scope. The host supplies the
/// Product that owns the declaration, and its candidate must satisfy this exact requirement.
pub(super) fn check_backings(
    accepted: &source_compiler::publication::AcceptedTerminalSource,
    produced: &source_compiler::publication::ProducedTerminalProgram,
    mut provide: impl FnMut(
        semantic_vocabulary::PackageKeyIdentity,
        &str,
    ) -> Result<ProducedBuildProvider, crate::CompileFailure>,
) -> Result<Vec<(BuildProviderRequest, ProducedBuildProvider)>, crate::CompileFailure> {
    use source_compiler::representations::declaration_selection::BuildSelectionRequest;
    let receiver = produced.view();
    let calls = produced
        .build_bindings()
        .calls_for(receiver)
        .ok_or_else(|| {
            vec![diagnostics::Diagnostic::error(
                "Build provider bindings lost their artifact custody",
            )]
        })?;
    let mut selected = Vec::new();
    for call in calls.iter() {
        let BuildSelectionRequest::ProviderReadiness(request) = &call.request else {
            continue;
        };
        if selected.iter().any(|(already, _)| already == request) {
            continue;
        }
        let operand = &request.backing;
        let refuse = |why: &str| {
            vec![diagnostics::Diagnostic::error(why).with_source_span(operand.source_span)]
        };
        let declaration = accepted.resolve_build_backing(operand)?;
        let provider = provide(declaration.package, &declaration.path)?;
        if !declaration.matches(&provider) {
            return Err(
                refuse("supplied Build provider differs from the checked declaration").into(),
            );
        }
        let identities = &receiver.module().identities;
        if identities.text(request.requirement.declaration.identity())
            != Some(provider.requirement())
            || identities.text(request.requirement.application) != Some(provider.application())
        {
            return Err(refuse(
                "supplied Build provider satisfies a different exact requirement application",
            )
            .into());
        }
        selected.push((request.clone(), provider));
    }
    Ok(selected)
}

pub(super) fn prepare<'host, 'code>(
    receiver: TerminalExecutionView<'_>,
    request: &BuildProviderRequest,
    provider: &ProducedBuildProvider,
    profile: TargetProfile,
    account: &build_model::accounting::EvaluationInvocationAccount,
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
    let program =
        TerminalProgram::retain(artifact.retain_code()).map_err(|error| error.to_string())?;
    let targets = terminal_target_context(artifact.view(), profile.target_name());
    let target_facts = profile
        .layout_target_facts()
        .ok_or("Build helper target has no representation facts")?;
    let layouts = admit_target_layouts(artifact.view(), &targets, &target_facts)
        .map_err(|error| format!("Build helper layouts: {error:?}"))?;
    let mut bindings = Vec::new();
    for boundary in &request.requirement.boundaries {
        let receiving = receiver
            .module()
            .interfaces.boundary_machines
            .iter()
            .find(|row| row.id == *boundary)
            .ok_or("Build readiness names an absent receiving slot")?;
        let identity = identities
            .text(receiving.identity)
            .ok_or("Build slot lost its exact requirement identity")?;
        let mut candidates = module.interfaces.provider_candidates.iter().filter(|candidate| {
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
        let completed = prepare_scalar_applications(
            artifact,
            profile.target_name(),
            None,
            &requirements.scalar_applications,
            Some(account),
            accounting,
            |scope| {
                let scope = scope.ok_or("Build helper requires its explicit deployment")?;
                let scope_profile = TargetProfile::from_canonical_target_name(scope)
                    .map_err(|diagnostic| diagnostic.message)?;
                Ok(ScalarScopeEvaluation {
                    application_roots: None,
                    import_realizations: None,
                    layout_target: Some(
                        scope_profile
                            .layout_target_facts()
                            .ok_or("scalar evaluation target has no immutable layout catalogue")?,
                    ),
                    targets: Some(terminal_target_context(artifact.view(), scope)),
                    providers: admitted.selection().clone(),
                    address_model: Ok(target::TargetSemantics::close_address_model(scope_profile)
                        .map(|model| semantic_vocabulary::ScalarAddressModel {
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
        .map_err(|reason| format!("Build helper scalar completion: {reason}"))?
        .into_admitted();
        let supplies = runtime_supplies(
            &program,
            &admitted,
            completed
                .primary()
                .targets()
                .ok_or("Build helper lost its deployment selection")?,
            completed
                .primary()
                .layouts()
                .ok_or("Build helper lost its deployment layouts")?,
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
    targets: &TerminalTargetContext,
    layouts: &AdmittedTargetLayouts,
) -> Result<terminal_interpreter::TerminalRuntimeSupplies, String> {
    program
        .admit_runtime_supplies(TerminalRuntimeSupplyRequest {
            assembly_returns: None,
            native_code_layouts: None,
            installation: Some(admitted.selection()),
            entry: Some(TargetMachineCallable::Machine(admitted.candidate())),
            host_imports: &[],
            external_completions: &[],
            x86_flags: None,
            memory_ordering: None,
            target_machines: Some(targets),
            target_layouts: Some(layouts),
        })
        .map_err(|error| format!("Build helper closure admission: {error:?}"))
}
