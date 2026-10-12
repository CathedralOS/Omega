//! Rebind authentic selected typed rows through their retained source join.
//!
//! Source dispatch and specialization provenance consume the same original
//! declaration, choosing map or machine edge, and produced application. A
//! named realization has no synthetic machine satisfaction edge. Physical
//! leaves retain their separate selected payload and ABI authority.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;

use crate::provider_planning::ProviderBinding;

pub(crate) fn derive_selected_requirement_specializations(
    typed: &checked_trees::typed_trees::TypedTrees,
    check_facts: &checked_trees::CheckFacts,
    selected: &effects::SelectedProviderPlanFacts,
) -> Result<
    (
        Vec<effects::SelectedRequirementSpecialization>,
        terminal_psi::TerminalIdentityTable,
    ),
    Vec<Diagnostic>,
> {
    let context = crate::ProviderPlanningContext::new(typed, source::DependencyScope::Product);
    let mut diagnostics = Vec::new();
    let mut rows = Vec::new();
    let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
    if std::env::var_os("OMEGA_SOURCE_CONSTANT_INPUT_TRACE").is_some() {
        eprintln!(
            "TRACE_SELECTED_APPLICATION_ROSTER plans={}",
            selected.plans().len()
        );
    }
    for plan in selected.plans() {
        for (ordinal, row) in plan.rows.iter().enumerate() {
            if std::env::var_os("OMEGA_SOURCE_CONSTANT_INPUT_TRACE").is_some() {
                eprintln!(
                    "TRACE_SELECTED_APPLICATION_PLAN name={} ordinal={} method={} checked={}",
                    plan.name,
                    ordinal,
                    row.method,
                    matches!(row.binding, ProviderBinding::CheckedAdapter { .. }),
                );
            }
            let ProviderBinding::CheckedAdapter {
                machine_identity: selected_machine_identity,
                machine_package_identity,
            } = &row.binding
            else {
                continue;
            };
            let result = (|| {
                let Some(requirement) = original_requirement(typed, context, plan, ordinal)? else {
                    // Legacy operator declarations own a different application
                    // vocabulary; they are not machine or trait requirements.
                    return Ok(Vec::new());
                };
                let source =
                    crate::selected_applications::selected_checked_requirement_row_for_plan(
                        typed,
                        check_facts,
                        plan,
                        ordinal,
                        requirement,
                    )?;
                let mut applications = Vec::new();
                for application in source.applications()? {
                    if application.requirement_instance() == requirement {
                        continue;
                    }
                    let implementation = typed
                        .machine(application.realization_instance())
                        .ok_or_else(|| {
                            Diagnostic::error("selected source application lost its implementation")
                        })?;
                    if typed.symbols.symbol_package_identity(implementation.symbol)
                        != *machine_package_identity
                    {
                        return Err(Diagnostic::error(
                            "selected source application changed its realizing package",
                        ));
                    }
                    let requirement_identity = requirement_identity(
                        typed,
                        application.requirement_instance(),
                    )
                    .ok_or_else(|| {
                        Diagnostic::error(
                            "selected source application lost its public requirement identity",
                        )
                    })?;
                    let machine_identity = typed
                        .machine_callable_identity_text(implementation)
                        .filter(|identity| !identity.is_empty())
                        .ok_or_else(|| {
                            Diagnostic::error(
                                "selected source application lost its implementation identity",
                            )
                        })?;
                    applications.push(effects::SelectedRequirementSpecialization {
                        provider_plan_report_identity: plan.report_fingerprint(),
                        selected_requirement_identity: identities
                            .intern(plan.identity_text(row.requirement_identity)),
                        selected_machine_identity: identities
                            .intern(plan.identity_text(*selected_machine_identity)),
                        requirement_identity: identities.intern(&requirement_identity),
                        machine_identity: identities.intern(&machine_identity),
                    });
                }
                // A top-level machine plan binds that requirement's complete
                // demand family. Trait headers also retain outer interface
                // applications: another map may own a different outer tuple of
                // the same member. Their typed interface roster and exact
                // source application query own coverage, not this ambient list.
                let mut demanded = typed
                    .machine_specializations
                    .iter()
                    .filter(|receipt| receipt.template == requirement)
                    .map(|receipt| receipt.instance)
                    .collect::<Vec<_>>();
                demanded.sort_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
                demanded.dedup();
                for instance in demanded {
                    let identity = requirement_identity(typed, instance).ok_or_else(|| {
                        Diagnostic::error("selected source demand lost its public identity")
                    })?;
                    if applications
                        .iter()
                        .filter(|application| {
                            identities.text(application.requirement_identity)
                                == Some(identity.as_str())
                        })
                        .count()
                        != 1
                    {
                        return Err(Diagnostic::error(
                            "selected provider does not cover one exact retained requirement application",
                        ));
                    }
                }
                Ok(applications)
            })();
            match result {
                Ok(applications) => rows.extend(applications),
                Err(diagnostic) => diagnostics.push(diagnostic),
            }
        }
    }
    if diagnostics.is_empty() {
        Ok((rows, identities.build()))
    } else {
        Err(diagnostics)
    }
}

fn original_requirement(
    typed: &checked_trees::typed_trees::TypedTrees,
    context: crate::ProviderPlanningContext<'_>,
    plan: &effects::provider_plan::ProviderPlan,
    ordinal: usize,
) -> Result<Option<SymbolHandle>, Diagnostic> {
    let row = &plan.rows[ordinal];
    if crate::provider_identity::named_provider(context, plan).is_some() {
        return crate::provider_identity::named_provider_row(context, plan, row)
            .map(|row| Some(row.requirement))
            .ok_or_else(|| {
                Diagnostic::error("selected named provider lost its original requirement row")
            });
    }
    let mut requirements = typed
        .machines()
        .iter()
        .filter(|requirement| {
            if requirement.supply_mode != language_semantics::MachineSupplyMode::TopLevelRequirement {
                return false;
            }
            let schema = crate::service_schema::from_typed_boundary_requirement(context, requirement);
            if std::env::var_os("OMEGA_SOURCE_CONSTANT_INPUT_TRACE").is_some()
                && requirement.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
                && (requirement.name.as_str() == row.method
                    || typed.machine_callable_identity_text(requirement).as_deref()
                        == Some(plan.identity_text(row.requirement_identity)))
            {
                eprintln!(
                    "TRACE_SELECTED_ORIGINAL_REQUIREMENT symbol={:?} name={} context={} body={} states={} schema={} schema_equal={} exact_row_identity={} published_termination={}",
                    requirement.symbol, requirement.name.as_str(), context.contains(requirement.symbol),
                    requirement.body_is_present, typed.machine_states(requirement).len(),
                    schema.is_some(), schema.as_ref() == Some(&plan.schema),
                    typed.machine_callable_identity_text(requirement).as_deref()
                        == Some(plan.identity_text(row.requirement_identity)),
                    requirement.termination_plan.interface.published().is_some(),
                );
            }
            schema.as_ref() == Some(&plan.schema)
        })
        .map(|requirement| requirement.symbol)
        .collect::<Vec<_>>();
    let adapter = super::adapter_conformance::exact_checked_adapter(context, plan, row)?;
    requirements.extend(
        typed
            .public_trait_requirement_signatures()
            .filter(|(definition, signature)| {
                definition.is_boundary
                    && typed.trait_requirement_callable_identity_text(definition, signature)
                        == plan.identity_text(row.requirement_identity)
                    && crate::provider_trait_requirement_matches(
                        context,
                        plan,
                        row,
                        adapter,
                        definition.symbol,
                        signature.symbol,
                    )
            })
            .map(|(_, signature)| signature.symbol),
    );
    requirements.sort_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
    requirements.dedup();
    match requirements.as_slice() {
        [] => Ok(None),
        [requirement] => Ok(Some(*requirement)),
        _ => Err(Diagnostic::error(
            "selected typed row has multiple original requirement declarations",
        )),
    }
}

fn requirement_identity(
    typed: &checked_trees::typed_trees::TypedTrees,
    requirement: SymbolHandle,
) -> Option<String> {
    if let Some(machine) = typed.machine(requirement) {
        return typed
            .machine_callable_identity_text(machine)
            .filter(|identity| !identity.is_empty());
    }
    let (definition, signature) = typed.trait_machine_signature(requirement)?;
    let identity = typed.trait_requirement_callable_identity_text(definition, signature);
    (!identity.is_empty()).then_some(identity)
}
