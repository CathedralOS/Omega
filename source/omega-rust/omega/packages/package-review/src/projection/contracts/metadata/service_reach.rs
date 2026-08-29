use crate::evidence::PackageReviewSourceLocationRole;
use crate::evidence::projection::ProjectedNestedSourceLocation;
use crate::projection::exact_identity::checked_facts::exactly_one;
use crate::projection::source_custody::locations::canonical_source_span_location;
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

pub(crate) fn project_machine_service_reach_source_locations(
    compilation: &CheckedCompilation,
    machine: &psi_typed_trees::machine::Machine,
) -> Result<Vec<ProjectedNestedSourceLocation>, Vec<Diagnostic>> {
    let authored = exact_authored_service_reach_row(
        compilation,
        machine.symbol,
        machine.name.as_str(),
        machine.service_reach_is_installation_bound,
    )?;
    let parameters = compilation
        .machine_states(machine)
        .first()
        .map(|state| compilation.state_parameters(state))
        .unwrap_or_default();
    let declared = derive_declared_service_reach(
        compilation,
        authored,
        &psi_effects::declared_machine_invocations(compilation, machine),
        parameters,
        machine.name.as_str(),
    )?;
    if compilation
        .service_reach_rows
        .services(machine.service_reach_row)
        != declared
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{}` authored reaches/invokes targets do not equal its exact normalized service-reach row",
            machine.name,
        ))]);
    }

    let checked = exactly_one(
        compilation
            .facts
            .service_reaches
            .machines()
            .iter()
            .filter(|fact| fact.machine == machine.symbol),
        machine.name.as_str(),
        "service-reach",
    )?;
    let should_publish = machine.supply_mode
        != psi_language_semantics::MachineSupplyMode::CheckedBody
        || machine.is_public
        || authored.is_some()
        || !declared.is_empty();
    let expected_interface = if should_publish {
        psi_language_semantics::ServiceReachInterface::PublishedCeiling(machine.service_reach_row)
    } else {
        psi_language_semantics::ServiceReachInterface::InternalInferred
    };
    if checked.interface != expected_interface
        || checked.published_ceiling != machine.service_reach_row
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{}` authored service-reach custody does not equal its exact checked service-reach fact",
            machine.name,
        ))]);
    }

    Ok(authored_service_reach_locations(authored))
}

pub(crate) fn project_signature_service_reach_source_locations(
    compilation: &CheckedCompilation,
    owner: SymbolHandle,
    signature: &psi_typed_trees::signature::StateSignature,
) -> Result<Vec<ProjectedNestedSourceLocation>, Vec<Diagnostic>> {
    let authored = exact_authored_service_reach_row(
        compilation,
        signature.symbol,
        signature.name.as_str(),
        signature.service_reach_is_installation_bound,
    )?;
    let declared = derive_declared_service_reach(
        compilation,
        authored,
        &psi_effects::declared_signature_invocations(compilation, signature),
        compilation.state_signature_parameters(signature),
        signature.name.as_str(),
    )?;
    if compilation
        .service_reach_rows
        .services(signature.service_reach_row)
        != declared
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed signature `{}` authored reaches/invokes targets do not equal its exact normalized service-reach row",
            signature.name,
        ))]);
    }

    let checked = exactly_one(
        compilation
            .facts
            .contract_plans
            .crash_capsules
            .iter()
            .filter(|capsule| {
                capsule.target_machine() == owner && capsule.target_state() == signature.symbol
            }),
        signature.name.as_str(),
        "signature contract capsule",
    )?;
    let mut checked_published = checked.published_service_reach().to_vec();
    checked_published.sort();
    checked_published.dedup();
    let mut declared_names = declared
        .iter()
        .map(|service| {
            compilation
                .service_reaches
                .definition(*service)
                .map(|definition| definition.name.clone())
                .ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "reviewed signature `{}` has a normalized service outside its exact declaration table",
                        signature.name,
                    ))]
                })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    declared_names.sort();
    declared_names.dedup();
    if checked_published != declared_names {
        return Err(vec![Diagnostic::error(format!(
            "reviewed signature `{}` authored service-reach custody does not equal its exact checked contract capsule",
            signature.name,
        ))]);
    }

    Ok(authored_service_reach_locations(authored))
}

pub(crate) fn exact_authored_service_reach_row<'a>(
    compilation: &'a CheckedCompilation,
    owner: SymbolHandle,
    owner_name: &str,
    installation_bound: bool,
) -> Result<Option<&'a psi_typed_trees::signature::AuthoredServiceReachRow>, Vec<Diagnostic>> {
    let matching = compilation
        .authored_service_reach_rows_for(owner)
        .collect::<Vec<_>>();
    let authored = match matching.as_slice() {
        [] => None,
        [row] => Some(*row),
        _ => {
            return Err(vec![Diagnostic::error(format!(
                "reviewed callable `{owner_name}` has {} authored service-reach custody rows; expected at most one",
                matching.len(),
            ))]);
        }
    };
    if installation_bound != authored.is_some_and(|row| row.installation_bound) {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{owner_name}` has contradictory installation-bound service-reach custody",
        ))]);
    }
    if authored.is_some_and(|row| row.keyword_source_spans.is_empty()) {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{owner_name}` has authored service-reach custody without a `reaches` keyword occurrence",
        ))]);
    }
    if let Some(authored) = authored {
        for keyword_source_span in &authored.keyword_source_spans {
            let _ = canonical_source_span_location(
                compilation,
                *keyword_source_span,
                PackageReviewSourceLocationRole::ServiceReach,
            )?;
        }
    }
    Ok(authored)
}

pub(crate) fn derive_declared_service_reach(
    compilation: &CheckedCompilation,
    authored: Option<&psi_typed_trees::signature::AuthoredServiceReachRow>,
    invocations: &[psi_effects::InvocationTarget],
    parameters: &[psi_typed_trees::signature::StateParameter],
    owner_name: &str,
) -> Result<Vec<psi_language_semantics::ServiceReachId>, Vec<Diagnostic>> {
    let mut direct = authored
        .into_iter()
        .flat_map(|row| &row.targets)
        .map(|target| {
            compilation
                .service_reaches
                .id_for_symbol(target.service)
                .ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "reviewed callable `{owner_name}` retains an authored service-reach target that is stale or not a boundary trait",
                    ))]
                })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    let non_self_parameters = parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    for invocation in invocations {
        let symbol = match invocation {
            psi_effects::InvocationTarget::Parameter(ordinal) => non_self_parameters
                .get(*ordinal as usize)
                .map(|parameter| {
                    compilation
                        .type_reference_table
                        .type_reference(parameter.type_reference)
                        .type_symbol(&compilation.type_reference_table)
                })
                .unwrap_or_else(SymbolHandle::invalid),
            psi_effects::InvocationTarget::Service(symbol) => *symbol,
        };
        let service = compilation
            .service_reaches
            .id_for_symbol(symbol)
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "reviewed callable `{owner_name}` has an invocation target without an exact boundary-service identity",
                ))]
            })?;
        direct.push(service);
    }

    let mut closure = Vec::new();
    for service in direct {
        compilation
            .service_reaches
            .extend_closure(service, &mut closure);
    }
    closure.sort_by_key(|service| service.0);
    closure.dedup();
    Ok(closure)
}

pub(crate) fn authored_service_reach_locations(
    authored: Option<&psi_typed_trees::signature::AuthoredServiceReachRow>,
) -> Vec<ProjectedNestedSourceLocation> {
    let Some(authored) = authored else {
        return Vec::new();
    };
    if authored.targets.is_empty() {
        authored
            .keyword_source_spans
            .iter()
            .copied()
            .map(|source_span| ProjectedNestedSourceLocation {
                source_span,
                role: PackageReviewSourceLocationRole::ServiceReach,
            })
            .collect()
    } else {
        authored
            .targets
            .iter()
            .map(|target| ProjectedNestedSourceLocation {
                source_span: target.source_span,
                role: PackageReviewSourceLocationRole::ServiceReach,
            })
            .collect()
    }
}
