//! Source-authored alternatives challenge receiving admission after the source
//! session is dropped. Changed selections never execute; their exact cycle
//! diagnostic is the oracle, independently of source-only checking.

use backend_plan::{TerminalTargetProviderSelection, admit_selected_checked_providers};
use effects::SelectedProviderPlanFacts;
use effects::provider_plan::ProviderBinding;
use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi_to_abstract_operations::RetainedTerminalArtifact;

/// Exercise declaration policy after Source has been dropped, independently of
/// the receiving executable-call graph checked by `inspect` below.
pub(super) fn inspect_declarations(
    retained: &RetainedTerminalArtifact,
    target: &str,
) -> Result<(), String> {
    let original = retained
        .native_realization_proposal()
        .ok_or("invocation controls need a native proposal")?
        .target_provider_proposals()
        .iter()
        .find(|proposal| proposal.target == target)
        .ok_or("invocation controls lost the target proposal")?
        .selection
        .as_ref()
        .map_err(|errors| format!("provider selection: {errors:?}"))?;
    let loaded = CanonicalTerminalArtifact::from_bytes(&retained.artifact().to_bytes())
        .map_err(|error| format!("invocation summary transport: {error:?}"))?;
    for module in [
        retained.artifact().semantic_module(),
        loaded.semantic_module(),
    ] {
        validate_declarations(original, module)
            .map_err(|errors| format!("canonical declaration cycles: {errors:?}"))?;
    }
    let module = loaded.semantic_module();
    let mut consumed = None;
    for selected in backend_plan::selected_checked_invocation_rows(&original.plans, module)? {
        for header in selected.headers {
            if header
                .synchronous_invocations
                .as_ref()
                .is_ok_and(|targets| !targets.is_empty())
            {
                consumed = Some(header.declaration);
                break;
            }
        }
        if consumed.is_some() {
            break;
        }
    }
    let consumed =
        consumed.ok_or("invocation control needs a selected header with actual calls")?;
    let mut missing = module.clone();
    let handle = missing
        .scalar_applications
        .proof_scope
        .callable_headers
        .iter()
        .find_map(|(handle, header)| (header.declaration == consumed).then_some(handle))
        .ok_or("consumed invocation header is absent")?;
    let reason = "missing checked synchronous-invocation summary control";
    missing
        .scalar_applications
        .proof_scope
        .callable_headers
        .get_mut(handle)
        .synchronous_invocations = Err(reason.into());
    let optimization = terminal_codec::build_identity_optimization_execution_record(&missing)
        .map_err(|error| format!("missing-summary envelope: {error:?}"))?;
    let missing = CanonicalTerminalArtifact::from_parts(missing, optimization, None)
        .map_err(|error| format!("missing-summary artifact: {error:?}"))?;
    let missing = CanonicalTerminalArtifact::from_bytes(&missing.to_bytes())
        .map_err(|error| format!("missing-summary ingress: {error:?}"))?;
    match validate_declarations(original, missing.semantic_module()) {
        Err(errors) if errors.iter().any(|error| error.message == reason) => Ok(()),
        other => Err(format!(
            "missing consumed invocation summary did not refuse: {other:?}"
        )),
    }
}

fn validate_declarations(
    selection: &TerminalTargetProviderSelection,
    module: &terminal_psi::TerminalModule,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    backend_plan::synchronous_invocations::validate_declared_synchronous_invocation_cycles(
        &selection.plans,
        module,
    )
}

pub(super) fn inspect(retained: &RetainedTerminalArtifact, target: &str) -> Result<(), String> {
    inspect_declarations(retained, target)?;
    let loaded = CanonicalTerminalArtifact::from_bytes(&retained.artifact().to_bytes())
        .map_err(|error| format!("provider transport: {error:?}"))?;
    let original = retained
        .native_realization_proposal()
        .ok_or("provider controls need a native proposal")?
        .target_provider_proposals()
        .iter()
        .find(|proposal| proposal.target == target)
        .ok_or("provider controls lost the target proposal")?
        .selection
        .as_ref()
        .map_err(|errors| format!("provider selection: {errors:?}"))?;
    let installation = original.admit(loaded.view(), target)?;
    let module = loaded.semantic_module();
    let candidate_named = |name: &str| {
        module
            .provider_candidates
            .iter()
            .find(|candidate| {
                module
                    .identities
                    .text(candidate.provider_declaration.identity())
                    .is_some_and(|identity| identity.contains(name))
            })
            .ok_or_else(|| format!("source fixture must retain provider {name}"))
    };
    let alpha = candidate_named("SelectedAlpha")?.candidate;
    let alternative = candidate_named("AlternativeBeta")?;
    let quiet = candidate_named("SelectedBeta")?.candidate;
    inspect_selected_closure(
        &loaded,
        target,
        &installation,
        alpha,
        quiet,
        alternative.candidate,
    )?;
    let requirement = module
        .identities
        .text(alternative.source_requirement_identity)
        .ok_or("alternative requirement identity is absent")?;
    let mut plans = original.plans.clone();
    let selected = plans
        .iter_mut()
        .find(|selected| {
            selected
                .plan
                .rows
                .iter()
                .any(|row| selected.plan.identity_text(row.requirement_identity) == requirement)
        })
        .ok_or("selected Beta slot is absent")?;
    selected.provider_declaration = Some(
        module
            .identities
            .text(alternative.provider_declaration.identity())
            .ok_or("alternative declaration is absent")?
            .into(),
    );
    selected.plan.provider_type = module
        .identities
        .text(alternative.provider_identity)
        .ok_or("alternative provider identity is absent")?
        .into();
    let machine_identity = selected.plan.identities.intern(
        module
            .identities
            .text(alternative.source_candidate_identity)
            .ok_or("alternative callable identity is absent")?,
    );
    selected.plan.rows[0].binding = ProviderBinding::CheckedAdapter {
        machine_identity,
        machine_package_identity: alternative.source_candidate_package_identity,
    };
    let changed = TerminalTargetProviderSelection::from_plans(plans, loaded.view())?;
    require_cycle(
        validate_declarations(&changed, module).map_err(|errors| format!("{errors:?}")),
        "declared checked alternative",
    )?;
    require_cycle(changed.admit(loaded.view(), target), "checked alternative")?;
    let facts = SelectedProviderPlanFacts::from_selected_plans(
        changed
            .plans
            .iter()
            .map(|selected| selected.plan.clone())
            .collect(),
    )?;
    require_cycle(
        admit_selected_checked_providers(loaded.view(), target, &facts, &changed.plans),
        "native checked alternative",
    )?;

    // Changing the default to an independent, quiet body cannot hide a cycle
    // among the other installed closed instances in this same artifact.
    let mut uncalled = module.clone();
    uncalled.entry = Some(terminal_psi::TargetMachineCallable::Machine(quiet));
    let optimization = terminal_codec::build_identity_optimization_execution_record(&uncalled)
        .map_err(|error| format!("uncalled optimization: {error:?}"))?;
    let uncalled = CanonicalTerminalArtifact::from_parts(uncalled, optimization, None)
        .map_err(|error| format!("uncalled artifact: {error:?}"))?;
    let uncalled = CanonicalTerminalArtifact::from_bytes(&uncalled.to_bytes())
        .map_err(|error| format!("uncalled transport: {error:?}"))?;
    let members = terminal_installation::admit_target_machine_selections(uncalled.view(), target);
    let profile =
        target::TargetProfile::from_canonical_target_name(target).map_err(|error| error.message)?;
    require_graph_error(
        terminal_psi_to_abstract_operations::provider_admission::selected_synchronous_call_graph(
            uncalled.view(),
            Some(&members),
            None,
            Some(&installation),
            profile,
        ),
        "synchronous provider graph has another provider installation",
        "foreign provider installation",
    )?;
    let calls = terminal_installation::SynchronousCallGraph::new(
        uncalled.semantic_module(),
        Some(&members),
        None,
    )?;
    if !calls.from_root(quiet)?.boundaries.is_empty() {
        return Err("quiet entry unexpectedly calls a provider".into());
    }
    inspect_program_context(
        retained,
        &uncalled,
        target,
        alpha,
        quiet,
        alternative.candidate,
    )?;
    require_cycle(
        changed.admit(uncalled.view(), target),
        "uncalled installed cycle",
    )?;

    // Opaque bindings use these same source-authored ceilings, not a summary
    // asserted for a checked body. Both public and native admission must check
    // the resulting cycle, even when native facts carry toolchain markers.
    let mut opaque = original
        .plans
        .iter()
        .filter(|selected| {
            selected
                .provider_declaration
                .as_deref()
                .is_some_and(|identity| {
                    identity.contains("SelectedAlpha") || identity.contains("SelectedBeta")
                })
        })
        .cloned()
        .collect::<Vec<_>>();
    if opaque.len() != 2 {
        return Err("opaque controls need both source-authored provider schemas".into());
    }
    for selected in &mut opaque {
        selected.provider_declaration = None;
        selected.plan.provider_type = "OpaqueTable".into();
        for row in &mut selected.plan.rows {
            row.binding = ProviderBinding::TableFunction {
                table: "OpaqueTable".into(),
                field: "read".into(),
                storage: Err("this control observes only invocation cycles".into()),
            };
        }
    }
    let mut opaque = TerminalTargetProviderSelection::from_plans(opaque, loaded.view())?;
    require_cycle(opaque.admit(loaded.view(), target), "opaque ceilings")?;
    let facts = SelectedProviderPlanFacts::from_selected_plans(
        opaque
            .plans
            .iter()
            .map(|selected| selected.plan.clone())
            .collect(),
    )?;
    require_cycle(
        admit_selected_checked_providers(loaded.view(), target, &facts, &opaque.plans),
        "native opaque ceilings",
    )?;
    let mut marked = SelectedProviderPlanFacts::default();
    for selected in &opaque.plans {
        marked = marked.with_toolchain_settled_plan(selected.plan.clone())?;
    }
    require_cycle(
        admit_selected_checked_providers(loaded.view(), target, &marked, &opaque.plans),
        "marked opaque ceilings",
    )?;
    for method in
        &mut std::sync::Arc::make_mut(&mut opaque.plans[0].plan.schema.declaration).methods
    {
        method.synchronous_invocations.clear();
    }
    opaque.admit(loaded.view(), target)?;
    Ok(())
}

/// Obtain the context from ordinary admission of the source's selected Alpha
/// body, then admit its quiet Beta body with an empty local provider scope.
/// Inspecting Alpha through that smaller scope must keep the original choices
/// without installing them in Beta. The actual installed private-body witness
/// remains the saved Cathedral native request.
fn inspect_program_context(
    retained: &RetainedTerminalArtifact,
    foreign: &CanonicalTerminalArtifact,
    target: &str,
    alpha: semantic_vocabulary::MachineId,
    beta: semantic_vocabulary::MachineId,
    alternative: semantic_vocabulary::MachineId,
) -> Result<(), String> {
    use std::sync::Arc;
    use terminal_psi::TargetMachineCallable;
    use terminal_psi_to_abstract_operations::{
        NativeAdmissionRequest, RetainedNativeProviderInputs, admit_prepared_native_realization,
        provider_admission::{
            ProviderAdmissionRequest, current_terminal_authority_policy,
            selected_synchronous_call_graph, terminal_authority_permission_policy_with_rows,
        },
    };
    let artifact = retained.artifact();
    let proposal = retained
        .native_realization_proposal()
        .ok_or("missing native proposal")?;
    let work = terminal_psi_to_abstract_operations::prepare_native_callable_execution(
        proposal,
        artifact,
        TargetMachineCallable::Machine(alpha),
    )
    .map_err(|errors| format!("program context parent preparation: {errors:?}"))?;
    let (input, execution) = work
        .lower(None)
        .map_err(|errors| format!("program context parent lowering: {errors:?}"))?;
    let permissions = terminal_authority_permission_policy_with_rows(
        proposal.package_terminal_authority_permissions().to_vec(),
        proposal
            .package_terminal_authority_permission_identities()
            .clone(),
    )
    .map_err(|error| format!("program context package permissions: {error:?}"))?;
    let providers = terminal_psi_to_abstract_operations::prepare_native_providers(
        proposal,
        artifact,
        RetainedNativeProviderInputs {
            execution: &execution,
            optimization_selections: proposal.post_terminal_optimizations().selections(),
            accepted_package_permissions: &permissions,
            imports: &[],
        },
    )
    .map_err(|errors| format!("program context provider preparation: {errors:?}"))?;
    let settlements = providers.program_settlements();
    let policy = current_terminal_authority_policy();
    let receiving_permissions = None;
    let request = NativeAdmissionRequest {
        entry: TargetMachineCallable::Machine(alpha),
        ..providers.admission_request(&settlements, &policy, &receiving_permissions, &[])
    };
    let parent = admit_prepared_native_realization(input, artifact, &request)
        .map_err(|errors| format!("program context parent admission: {errors:?}"))?;
    let context = Arc::clone(parent.providers.program_provider_context());
    let profile = request.providers.profile;
    context.require(
        artifact.view().identity(),
        profile,
        request.providers.selected_provider_plans,
    )?;
    require_graph_error(
        context.require(
            foreign.view().identity(),
            profile,
            request.providers.selected_provider_plans,
        ),
        "program provider context names another Terminal artifact",
        "foreign program context",
    )?;
    let other_target = match target {
        "macos_arm64" => "linux_arm64",
        "linux_arm64" => "macos_arm64",
        "linux_x86_64" => "windows_x86_64",
        _ => "linux_x86_64",
    };
    let other_profile = target::TargetProfile::from_canonical_target_name(other_target)
        .map_err(|error| error.message)?;
    require_graph_error(
        context.require(
            artifact.view().identity(),
            other_profile,
            request.providers.selected_provider_plans,
        ),
        "program provider context names another target profile",
        "foreign program target",
    )?;
    require_graph_error(
        context.require(
            artifact.view().identity(),
            profile,
            &SelectedProviderPlanFacts::default(),
        ),
        "program provider context names another selected-plan closure",
        "changed program choices",
    )?;
    drop(parent);
    let (child_input, child_coverage) = terminal_psi_to_abstract_operations::lower_native_callable(
        &execution,
        artifact,
        proposal,
        TargetMachineCallable::Machine(beta),
        None,
    )
    .map_err(|errors| format!("program context child lowering: {errors:?}"))?;
    let child_request = NativeAdmissionRequest {
        entry: TargetMachineCallable::Machine(beta),
        boundary_application_coverage: Some(&child_coverage),
        providers: ProviderAdmissionRequest {
            program_provider_context: Some(&context),
            ..request.providers
        },
        ..request
    };
    let child = admit_prepared_native_realization(child_input, artifact, &child_request)
        .map_err(|errors| format!("program context child admission: {errors:?}"))?;
    if !Arc::ptr_eq(&context, child.providers.program_provider_context())
        || child
            .providers
            .installation
            .as_ref()
            .is_some_and(|installation| {
                installation
                    .terminal_selection()
                    .providers()
                    .next()
                    .is_some()
                    || !installation.installed_calls().is_empty()
            })
    {
        return Err("private admission copied program providers into its quiet scope".into());
    }
    let members = terminal_installation::admit_target_machine_selections(artifact.view(), target);
    let original = selected_synchronous_call_graph(
        artifact.view(),
        Some(&members),
        context.entry_bridge(),
        context.providers(),
        profile,
    )?
    .from_root(alpha)?;
    let local = selected_synchronous_call_graph(
        artifact.view(),
        Some(&members),
        None,
        child
            .providers
            .installation
            .as_ref()
            .map(|row| row.terminal_selection()),
        profile,
    )?
    .from_root(alpha)?;
    if !original.machines.iter().any(|body| body.id == beta)
        || original.machines.iter().any(|body| body.id == alternative)
        || local
            .machines
            .iter()
            .any(|body| body.id == beta || body.id == alternative)
    {
        return Err(
            "program entry inspection did not retain its exact selected provider context".into(),
        );
    }
    Ok(())
}

fn inspect_selected_closure(
    artifact: &CanonicalTerminalArtifact,
    target: &str,
    installation: &terminal_installation::AdmittedProviderSelections,
    alpha: semantic_vocabulary::MachineId,
    beta: semantic_vocabulary::MachineId,
    alternative: semantic_vocabulary::MachineId,
) -> Result<(), String> {
    use terminal_psi_to_abstract_operations::provider_admission::selected_synchronous_call_graph;

    let profile =
        target::TargetProfile::from_canonical_target_name(target).map_err(|error| error.message)?;
    let members = terminal_installation::admit_target_machine_selections(artifact.view(), target);
    let boundary = installation
        .providers()
        .find_map(|(boundary, machine)| (machine == beta).then_some(boundary))
        .ok_or("source fixture must select the quiet Beta body")?;
    let selected = selected_synchronous_call_graph(
        artifact.view(),
        Some(&members),
        None,
        Some(installation),
        profile,
    )?;
    let closure = selected.from_root(alpha)?;
    if selected.boundary_callee(boundary)? != Some(beta)
        || !closure.boundaries.contains(&boundary)
        || !closure.machines.iter().any(|body| body.id == alpha)
        || !closure.machines.iter().any(|body| body.id == beta)
        || closure.machines.iter().any(|body| body.id == alternative)
    {
        return Err(
            "selected synchronous closure lost its exact provider edge or chose an alternative"
                .into(),
        );
    }
    let opaque =
        selected_synchronous_call_graph(artifact.view(), Some(&members), None, None, profile)?;
    let closure = opaque.from_root(alpha)?;
    if opaque.boundary_callee(boundary)?.is_some()
        || !closure.boundaries.contains(&boundary)
        || closure
            .machines
            .iter()
            .any(|body| body.id == beta || body.id == alternative)
    {
        return Err("unresolved synchronous closure acquired a provider body".into());
    }
    let other_target = match target {
        "macos_arm64" => "linux_arm64",
        "linux_arm64" => "macos_arm64",
        "linux_x86_64" => "windows_x86_64",
        _ => "linux_x86_64",
    };
    let other_members =
        terminal_installation::admit_target_machine_selections(artifact.view(), other_target);
    require_graph_error(
        selected_synchronous_call_graph(
            artifact.view(),
            Some(&other_members),
            None,
            Some(installation),
            profile,
        ),
        "synchronous provider graph has another artifact or target selection",
        "foreign deployment selection",
    )?;
    require_graph_error(
        terminal_installation::SynchronousCallGraph::new(artifact.semantic_module(), None, None)?
            .with_boundary_calls([(boundary, beta), (boundary, beta)]),
        "synchronous boundary call repeats a resolved boundary",
        "duplicate resolved boundary",
    )?;
    for coordinates in [
        (
            semantic_vocabulary::BoundaryMachineId::new(u64::MAX)
                .ok_or("absent boundary identity")?,
            beta,
        ),
        (
            boundary,
            semantic_vocabulary::MachineId::new(u64::MAX).ok_or("absent body identity")?,
        ),
    ] {
        require_graph_error(
            terminal_installation::SynchronousCallGraph::new(
                artifact.semantic_module(),
                None,
                None,
            )?
            .with_boundary_calls([coordinates]),
            "synchronous boundary call names an absent boundary or body",
            "absent resolved coordinate",
        )?;
    }
    Ok(())
}

fn require_graph_error<T>(
    result: Result<T, String>,
    expected: &str,
    control: &str,
) -> Result<(), String> {
    match result {
        Err(reason) if reason == expected => Ok(()),
        Err(reason) => Err(format!("{control} refused for another reason: {reason}")),
        Ok(_) => Err(format!(
            "{control} accepted an invalid synchronous graph join"
        )),
    }
}

fn require_cycle<T>(result: Result<T, String>, control: &str) -> Result<(), String> {
    match result {
        Err(reason) if reason.contains("cyclic synchronous `invokes` graph") => Ok(()),
        Err(reason) => Err(format!("{control} refused for another reason: {reason}")),
        Ok(_) => Err(format!("{control} accepted a synchronous provider cycle")),
    }
}
