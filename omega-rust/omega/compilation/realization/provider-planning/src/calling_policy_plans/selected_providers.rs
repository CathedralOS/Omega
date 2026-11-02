//! Choose semantic helpers over the shared Terminal invocation graph.
//!
//! Calling uses neutral contracts: its ABI result does not yet exist. The
//! ordinary canonical selector closes exact invocation demand through selected
//! providers. Source correspondence below only prepares the existing Psi
//! admission inputs; it does not discover demand or select another whole map.
//! Neutral commitments never become final native calling identities.

use checked_trees::typed_trees::service_declarations::ServiceCatalog;
use diagnostics::Diagnostic;
use language_semantics::direct_calls::DirectCallDeclarations;
use symbols::SymbolHandle;

pub struct CallingPolicyProviderInputs<'program> {
    pub body_declarations: &'program DirectCallDeclarations<SymbolHandle>,
    pub defaults: &'program [crate::ProviderSelection],
    pub requested: &'program [crate::ProviderSelection],
    pub exact_toolchain_sources: &'program [(source::SourceId, [u8; 32])],
}

pub(super) struct SelectedPolicyProviders {
    pub bodies: Vec<build_time_evaluation::SelectedBuildTimeProviderBody>,
    pub declarations: Vec<backend_plan::TerminalTargetProviderPlan>,
}

pub(super) fn prepare(
    checked: &checked_trees::CheckedTrees,
    services: ServiceCatalog<'_>,
    target: target::TargetProfile,
    roots: &[SymbolHandle],
    inputs: &CallingPolicyProviderInputs<'_>,
    code: &super::CallingProgram,
) -> Result<SelectedPolicyProviders, Vec<Diagnostic>> {
    let failure = |reason: String| vec![Diagnostic::error(reason)];
    let artifact = code.artifact.view();
    let module = artifact.module();
    let scope = &module.scalar_applications.proof_scope;
    let mut declarations = Vec::with_capacity(roots.len());
    for root in roots {
        let entry = code
            .entries
            .iter()
            .find(|(source, _)| source == root)
            .ok_or_else(|| {
                failure("calling helper selection lost its exact published root".into())
            })?
            .1
            .as_ref()
            .map_err(Clone::clone)?;
        let declaration = scope
            .callable_declaration(terminal_psi::TargetMachineCallable::Machine(*entry))
            .ok_or_else(|| {
                failure("calling helper selection lost its exact invocation declaration".into())
            })?;
        if !declarations.contains(&declaration) {
            declarations.push(declaration);
        }
    }
    let inventory = crate::target_machines::AuthoredTargetProviders::unevaluated(
        services,
        inputs.body_declarations,
        inputs.defaults,
        target,
    )?;
    let prepared = crate::target_machines::TargetProviderPreparationRequest {
        context: services.context(),
        exact_toolchain_sources: inputs.exact_toolchain_sources,
        application_roots: None,
        application_scopes: &[],
        overrides: inputs.requested,
        assumed_bindings: &[],
        independent_components: &[],
        selected: None,
        targets: &[],
    }
    .prepare_target(target.target_name(), &inventory, &[])?;
    let selected = prepared.select(
        artifact,
        provider_selection::ProviderApplicationDemand::Invocation(&declarations),
        &[],
    )?;
    for choice in &selected.choices {
        if !matches!(
            choice.selected_by,
            provider_selection::SelectionProvenance::BuildOverride(_)
                | provider_selection::SelectionProvenance::TargetDefault(_)
        ) {
            return Err(failure("calling-policy helper requires an exact Build or target-default provider selection".into()));
        }
    }

    // Only the existing Psi admission adapter reads Source here. The canonical
    // fixed point already decided the called declarations and selected plans.
    // An exact occurrence substitution is not authority for another call to
    // the same implementation body.
    let machines = source_invocation_machines(checked, module, &selected.invoked_declarations)?;
    let mut bodies = Vec::new();
    for (expression, origin, requirement, _) in
        checked.facts.operators.resolved_expression_selections()
    {
        if !origin
            .machine_symbol()
            .is_some_and(|owner| machines.contains(&owner))
        {
            continue;
        }
        let application = checked.typed.closed_requirement_application(requirement);
        let template = application.map_or(requirement, |application| application.template);
        let mut matching = selected.choices.iter().flat_map(|choice| {
            choice
                .candidate
                .owner
                .row_requirements
                .iter()
                .enumerate()
                .filter_map(move |(ordinal, source)| {
                    (*source == template).then_some((choice, ordinal))
                })
        });
        let Some((choice, ordinal)) = matching.next() else {
            continue;
        };
        if matching.next().is_some() {
            return Err(failure(
                "calling helper occurrence has competing selected Source correspondences".into(),
            ));
        }
        let plan = &choice.candidate.plan;
        let owner = &choice.candidate.owner;
        let row = plan
            .rows
            .get(ordinal)
            .ok_or_else(|| failure("calling helper lost its selected row".into()))?;
        if !matches!(
            row.binding,
            effects::provider_plan::ProviderBinding::CheckedAdapter { .. }
        ) {
            return Err(failure("calling-policy evaluation requires a checked helper body; native supply is not semantic execution".into()));
        }
        let mut body = owner
            .row_realizations
            .get(ordinal)
            .and_then(|realization| realization.authored())
            .and_then(|symbol| checked.typed.machine(symbol))
            .ok_or_else(|| {
                failure("calling helper lost its selected Source body correspondence".into())
            })?;
        let declaration = owner.provider_type.ok_or_else(|| {
            failure("calling helper lost its selecting declaration correspondence".into())
        })?;
        if let Some(application) =
            application.filter(|application| application.template != requirement)
        {
            // A closed use names the checker's exact application, not a
            // same-spelled generic template. Selection is already complete;
            // only its original-to-closed body correspondence is substituted.
            let mut instances = checked
                .facts
                .provider_closed_requirements
                .iter()
                .filter(|fact| {
                    fact.template == template
                        && fact.closed_requirement == requirement
                        && fact.receiving_application == application.trait_application_identity
                        && fact.selecting_declaration == declaration
                        && checked.typed.requirement_realization_matches(
                            requirement,
                            body.symbol,
                            fact.provider,
                        )
                });
            let instance = instances.next().ok_or_else(|| {
                failure("calling helper lost its exact checked closed application".into())
            })?;
            if instances.next().is_some() {
                return Err(failure(
                    "calling helper has competing checked closed applications".into(),
                ));
            }
            body = checked
                .typed
                .machine(instance.provider)
                .ok_or_else(|| failure("calling helper closed application lost its body".into()))?;
        }
        bodies.push(
            build_time_evaluation::SelectedBuildTimeProviderBody::prepare(
                &checked.typed,
                expression,
                origin,
                requirement,
                body,
                plan.provider_type.clone(),
                declaration,
                plan.provider_type_package_identity,
                target.target_name(),
                checked_trees::CheckedProviderPlanCommitment::from_digest(
                    *plan.identity_digest().as_bytes(),
                ),
            )
            .map_err(failure)?,
        );
    }
    Ok(SelectedPolicyProviders {
        bodies,
        declarations: selected.selection.plans,
    })
}

/// Rejoin exact checked headers solely for the remaining Source admission
/// floor. No source call traversal or provider choice occurs at this adapter.
fn source_invocation_machines(
    checked: &checked_trees::CheckedTrees,
    module: &terminal_psi::TerminalModule,
    invoked: &[terminal_psi::ProofDeclarationId],
) -> Result<Vec<SymbolHandle>, Vec<Diagnostic>> {
    let mut callables = Vec::new();
    for (_, header) in module
        .scalar_applications
        .proof_scope
        .callable_headers
        .iter()
    {
        if !invoked.contains(&header.declaration) {
            continue;
        }
        let identity = header
            .callable
            .as_ref()
            .map_err(|reason| vec![Diagnostic::error(reason.clone())])?;
        let callable = module.identities.text(*identity).ok_or_else(|| {
            vec![Diagnostic::error(
                "invoked callable lost its canonical identity",
            )]
        })?;
        callables.push((
            callable,
            header.package,
            header.source_target.as_deref(),
            header.product_eligible,
        ));
    }
    callables.sort_unstable();
    let mut machines = Vec::new();
    for (_, header) in checked.facts.declaration_descriptions.headers.iter() {
        if !header.callable.is_valid() {
            continue;
        }
        let callable = checked.callable_identity_text(header.callable);
        if callables
            .binary_search(&(
                callable.as_str(),
                header.package,
                header.source_target.as_deref(),
                header.product_eligible,
            ))
            .is_ok()
        {
            machines.push(header.machine);
        }
    }
    Ok(machines)
}
