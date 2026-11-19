use diagnostics::Diagnostic;
use semantic_vocabulary::BoundaryMachineId;
use terminal_psi::TerminalModule;

/// Selected native leaves and the identities used by their retained receipts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedNativeSupplies {
    pub identities: terminal_psi::TerminalIdentityTable,
    pub rows: Vec<NormalizedNativeSupply>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedNativeSupply {
    pub boundary: BoundaryMachineId,
    pub binding: provider_plans::EvaluatedNativeBinding,
}

/// Use the selected body's own closure to resolve each authored installation bound.
pub(super) fn resolve_installation_reach(
    module: &TerminalModule,
    installation_requirements: &[BoundaryMachineId],
    selected: selected_provider_plans::SelectedProviderPlanFacts,
) -> Result<selected_provider_plans::SelectedProviderPlanFacts, Vec<Diagnostic>> {
    let fail = |reason: String| vec![Diagnostic::error(reason)];
    if installation_requirements.is_empty() {
        return if module
            .interfaces.root_service_reach
            .installation_dependencies
            .is_empty()
        {
            Ok(selected)
        } else {
            Err(fail(
                "root installation reach has no authored requirement census".into(),
            ))
        };
    }
    let boundary = |id| {
        module
            .boundary_machine(id)
            .ok_or_else(|| fail(format!("installation reach lost exact boundary {id:?}")))
    };
    let service_name = |id| {
        module
            .interfaces.services
            .binary_search_by_key(&id, |service| service.id)
            .ok()
            .and_then(|index| module.identities.text(module.interfaces.services[index].identity))
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
            .ok_or_else(|| fail(format!("installation reach lost exact service {id:?}")))
    };
    let graph = terminal_semantics::synchronous_calls::SynchronousCallGraph::new(module, None).map_err(fail)?;
    let mut rows = Vec::new();
    for row in
        target_provider_admission::selected_checked_invocation_rows(selected.plans(), module).map_err(fail)?
    {
        for candidate in row.candidates {
            let supplied = candidate.conformance;
            if installation_requirements
                .binary_search(&supplied.boundary)
                .is_err()
            {
                continue;
            }
            let required = boundary(supplied.boundary)?;
            let reach = graph
                .service_reach_from(supplied.candidate, installation_requirements)
                .map_err(fail)?;
            let dependencies = reach
                .installation_dependencies
                .iter()
                .map(|dependency| {
                    module
                        .identities
                        .text(dependency.requirement_identity)
                        .filter(|identity| !identity.is_empty())
                        .map(str::to_owned)
                        .ok_or_else(|| {
                            fail(
                                "installation reach dependency has no exact callable identity"
                                    .into(),
                            )
                        })
                })
                .collect::<Result<_, _>>()?;
            let requirement_identity = row
                .plan
                .identities
                .text(row.row.requirement_identity)
                .to_owned();
            if module.identities.text(required.identity) != Some(requirement_identity.as_str()) {
                return Err(fail(
                    "installation reach row differs from its selected requirement".into(),
                ));
            }
            rows.push(provider_correspondence::InstallationReachRow {
                requirement_identity,
                provider_plan_report_identity: row.plan.report_fingerprint(),
                provider_name: row.plan.provider_type.clone(),
                realization_name: module
                    .identities
                    .text(supplied.candidate_identity)
                    .unwrap_or_default()
                    .to_owned(),
                upper_bound: required
                    .published_service_ceiling
                    .iter()
                    .copied()
                    .map(service_name)
                    .collect::<Result<_, _>>()?,
                concrete_reach: reach
                    .concrete
                    .into_iter()
                    .map(service_name)
                    .collect::<Result<_, _>>()?,
                dependencies,
            });
        }
    }
    let (resolutions, identities) = provider_correspondence::resolve_installation_reach(rows)?;
    let selected = selected
        .with_installation_reach_resolutions(resolutions, &identities)
        .map_err(fail)?;
    selected.resolve_root_service_reach(module).map_err(fail)?;
    Ok(selected)
}
