use crate::capture::PackageReviewInput;
use crate::capture::semantics::facts::exactly_one;
use crate::capture::source::locations::canonical_source_span_location;
use crate::record::PackageReviewSourceLocationRole;
use diagnostics::Diagnostic;
use flow_effects::ServiceReachInferencePlan;
use symbols::SymbolHandle;

/// Rejoin authored targets and checked ceilings without constructing location rows.
pub(crate) fn validate_machine_service_reach(
    compilation: &PackageReviewInput<'_>,
    machine: &typed_trees::machine::Machine,
    inferred: &ServiceReachInferencePlan,
) -> Result<(), Vec<Diagnostic>> {
    checked_machine_service_reach(compilation, machine, inferred).map(|_| ())
}

fn checked_machine_service_reach<'a>(
    compilation: &'a PackageReviewInput<'a>,
    machine: &typed_trees::machine::Machine,
    inferred: &ServiceReachInferencePlan,
) -> Result<Option<&'a typed_trees::signature::AuthoredServiceReachRow>, Vec<Diagnostic>> {
    let authored = exact_authored_service_reach_row(
        compilation,
        machine.symbol,
        machine.name.as_str(),
        machine.service_reach_is_installation_bound,
    )?;
    let declared = derive_declared_service_reach(compilation, authored, machine.name.as_str())?;
    if compilation
        .service_reach_rows
        .services(machine.service_reach_row)
        != declared
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{}` authored reaches targets do not equal its exact normalized service-reach row",
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
    let expected = exactly_one(
        inferred
            .machines()
            .iter()
            .filter(|fact| fact.machine == machine.symbol),
        machine.name.as_str(),
        "reconstructed service-reach",
    )?;
    // Publication policy belongs to the reconstructed inference plan. Its row
    // handles use a different table, so compare the category here and contents
    // below rather than copying an inference handle into checked custody.
    let expected_interface = match expected.interface {
        language_semantics::ServiceReachInterface::PublishedCeiling(_) => {
            language_semantics::ServiceReachInterface::PublishedCeiling(checked.published_ceiling)
        }
        language_semantics::ServiceReachInterface::InternalInferred => {
            language_semantics::ServiceReachInterface::InternalInferred
        }
    };
    // Authored custody remains exact above; publication additionally includes
    // ordinary checked callees, reconstructed once for this capture request.
    let published = compilation
        .facts
        .service_reaches
        .rows
        .services(checked.published_ceiling);
    if checked.interface != expected_interface
        || (published.is_empty()
            && checked.published_ceiling != language_semantics::ServiceReachRowTable::EMPTY_ROW)
        || published != inferred.rows.services(expected.published)
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{}` authored service-reach custody does not equal its exact checked service-reach fact",
            machine.name,
        ))]);
    }

    Ok(authored)
}

fn exact_authored_service_reach_row<'a>(
    compilation: &'a PackageReviewInput<'a>,
    owner: SymbolHandle,
    owner_name: &str,
    installation_bound: bool,
) -> Result<Option<&'a typed_trees::signature::AuthoredServiceReachRow>, Vec<Diagnostic>> {
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

fn derive_declared_service_reach(
    compilation: &PackageReviewInput<'_>,
    authored: Option<&typed_trees::signature::AuthoredServiceReachRow>,
    owner_name: &str,
) -> Result<Vec<language_semantics::ServiceReachId>, Vec<Diagnostic>> {
    // Invoked interface identity is checked separately in invocations.rs; only
    // authored capability reach and its parents belong to this source row.
    let direct = authored
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
