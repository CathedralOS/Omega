//! Provider plans derive from checked `satisfies` closures and are admitted
//! through the chapter-10 trust path. Own-package plans remain dev-active with
//! a standing warning until the final build grants them; lockfile receipts hash
//! normalized plan identity so a changed plan drifts. A unique covering
//! candidate may still supply the declaration-era default, while explicit
//! selection remains under slot-owner authority.

use effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceSchema};
pub use effects::{
    CompilerIntrinsicExecutionIdentity, CompilerNumericType, CompilerPrimitiveFloatBinaryOperation,
};
use std::sync::Arc;
#[cfg(test)]
use trust_model::ProviderGrantSelectorKind;
use trust_model::resolve_selected_provider_grants;
use trust_model::{AuthoredRootGrant, ResolvedAuthoredSelectedProviderGrant};
use typed_trees::TypedTrees;

#[path = "plans/external_binding_rows.rs"]
mod external_binding_rows;
pub use external_binding_rows::{
    extract_external_binding_rows, extract_normalized_import_binding_rows,
    settle_external_binding_rows,
};
#[path = "plans/intrinsic_execution.rs"]
mod intrinsic_execution;
pub use intrinsic_execution::primitive_float_binary_intrinsic_execution_identity;
#[path = "plans/provenance_replay.rs"]
mod provenance_replay;
#[cfg(test)]
use provenance_replay::exact_canonical_provider_schema;
#[cfg(feature = "installed-writer")]
use provenance_replay::same_semantic_name;
pub use provenance_replay::*;
use provenance_replay::{
    exact_authored_invocations, exact_checked_adapter_invocations, exact_row_for_schema_method,
};

#[cfg(feature = "installed-writer")]
#[path = "plans/installed_writer.rs"]
mod installed_writer;
#[cfg(feature = "installed-writer")]
pub use installed_writer::*;

/// Exact checked-program and selected-plan candidate after every provider
/// grant, receipt, operator-use, and installation-reach decision has replayed.
/// The candidate owns any separated Arc privately until its caller commits it.
#[derive(Debug)]
pub struct SelectedProviderPlanBinding {
    program: Arc<checked_trees::CheckedTrees>,
    selected: effects::SelectedProviderPlanFacts,
    grants: Vec<ResolvedAuthoredSelectedProviderGrant>,
}

impl SelectedProviderPlanBinding {
    pub fn into_parts(
        self,
    ) -> (
        Arc<checked_trees::CheckedTrees>,
        effects::SelectedProviderPlanFacts,
        Vec<ResolvedAuthoredSelectedProviderGrant>,
    ) {
        (self.program, self.selected, self.grants)
    }
}

#[derive(Default)]
struct SelectedProviderProgramUpdates {
    spelled_operator_uses: Vec<(
        arena::Handle<checked_trees::CheckedOperatorUseFact>,
        u64,
        checked_trees::CheckedProviderPlanCommitment,
    )>,
    named_operator_uses: Vec<(
        arena::Handle<checked_trees::CheckedNamedOperatorUseFact>,
        u64,
        checked_trees::CheckedProviderPlanCommitment,
    )>,
    admitted_receipts: Vec<(facts::FactHandle, u64)>,
}

impl SelectedProviderProgramUpdates {
    fn is_empty(&self) -> bool {
        self.spelled_operator_uses.is_empty()
            && self.named_operator_uses.is_empty()
            && self.admitted_receipts.is_empty()
    }

    fn apply(self, checked: &mut checked_trees::CheckedTrees) {
        for (handle, report_fingerprint, commitment) in self.spelled_operator_uses {
            let operator_use = checked.facts.operators.uses.get_mut(handle);
            operator_use.provider_plan_report_fingerprint = report_fingerprint;
            operator_use.provider_plan_commitment = commitment;
        }
        for (handle, report_fingerprint, commitment) in self.named_operator_uses {
            let operator_use = checked.facts.operators.named_uses.get_mut(handle);
            operator_use.provider_plan_report_fingerprint = report_fingerprint;
            operator_use.provider_plan_commitment = commitment;
        }
        for (handle, identity) in self.admitted_receipts {
            checked
                .facts
                .semantic
                .facts
                .get_mut(handle)
                .evidence
                .receipt_identity = identity;
        }
    }
}

/// Build the exact Omega-owned selection sidecar and bind its stable receipt
/// identities into checked semantic evidence. Provider execution and
/// compiler-generated helper machines consume the returned carrier; neither
/// may reconstruct a plan by scanning authored `satisfies` rows.
pub fn bind_selected_provider_plan_facts(
    program: &Arc<checked_trees::CheckedTrees>,
    candidates: &[ProviderPlan],
    facts: effects::SelectedProviderPlanFacts,
    root_grants: &[String],
    authored_root_grants: &[AuthoredRootGrant],
) -> Result<SelectedProviderPlanBinding, Vec<diagnostics::Diagnostic>> {
    let checked = program.as_ref();
    let provider_grants = resolve_selected_provider_grants(candidates, &facts, root_grants)
        .map_err(|diagnostic| vec![diagnostic])?;
    let authored_provider_grants = trust_model::resolve_authored_selected_provider_grants(
        candidates,
        &facts,
        authored_root_grants,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    let mut granted_plans = Vec::new();
    for grant in &provider_grants {
        let exact_selected_matches = facts
            .plans()
            .iter()
            .filter(|plan| grant.replays_selected_plan(plan))
            .count();
        if exact_selected_matches != 1 {
            return Err(vec![diagnostics::Diagnostic::error(format!(
                "provider grant `{}` replays against {exact_selected_matches} exact selected provider plans",
                grant.selector,
            ))]);
        }
        if !granted_plans
            .iter()
            .any(|retained: &&trust_model::ResolvedSelectedProviderGrant| {
                retained.selected_plan == grant.selected_plan
                    && retained.selected_plan_digest == grant.selected_plan_digest
            })
        {
            granted_plans.push(grant);
        }
    }
    let mut receipt_updates = Vec::new();
    let mut receipt_diagnostics = Vec::new();
    let traits = checked.typed.traits();
    if traits.len() != checked.typed.roots.traits.count() as usize {
        receipt_diagnostics.push(diagnostics::Diagnostic::error(
            "admitted qualification receipt binding has an invalid typed trait span",
        ));
    }
    for definition in traits {
        if checked.typed.trait_machine_signatures(definition).len()
            != definition.machines.count() as usize
        {
            receipt_diagnostics.push(diagnostics::Diagnostic::error(format!(
                "admitted qualification receipt binding has an invalid typed signature span for trait {:?}",
                definition.symbol,
            )));
        }
    }
    if !receipt_diagnostics.is_empty() {
        return Err(receipt_diagnostics);
    }
    for (handle, fact) in checked.facts.semantic.facts.iter().filter(|(_, fact)| {
        fact.evidence.origin == language_semantics::QualificationEvidenceOrigin::AdmittedReceipt
            && fact.evidence.receipt_identity == 0
    }) {
        let owners = checked
            .typed
            .traits()
            .iter()
            .filter(|definition| definition.symbol == fact.evidence.source_symbol)
            .collect::<Vec<_>>();
        let owner = match owners.as_slice() {
            [owner] if owner.is_boundary => *owner,
            [owner] => {
                receipt_diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "admitted qualification evidence source {:?} names non-boundary trait `{}`",
                    fact.evidence.source_symbol, owner.name,
                )));
                continue;
            }
            _ => {
                receipt_diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "admitted qualification evidence source {:?} resolves to {} exact typed boundary requirement owners",
                    fact.evidence.source_symbol,
                    owners.len(),
                )));
                continue;
            }
        };
        let requirement_owners = checked
            .typed
            .traits()
            .iter()
            .flat_map(|candidate_owner| {
                checked
                    .typed
                    .trait_machine_signatures(candidate_owner)
                    .iter()
                    .filter(move |requirement| {
                        requirement.symbol == fact.evidence.requirement_symbol
                    })
                    .map(move |requirement| (candidate_owner, requirement))
            })
            .collect::<Vec<_>>();
        let requirement = match requirement_owners.as_slice() {
            [(requirement_owner, requirement)] if requirement_owner.symbol == owner.symbol => {
                *requirement
            }
            [(requirement_owner, _)] => {
                receipt_diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "admitted qualification evidence requirement {:?} belongs to exact trait {:?}, not owner {:?}",
                    fact.evidence.requirement_symbol,
                    requirement_owner.symbol,
                    owner.symbol,
                )));
                continue;
            }
            _ => {
                receipt_diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "admitted qualification evidence requirement {:?} resolves to {} exact typed signatures",
                    fact.evidence.requirement_symbol,
                    requirement_owners.len(),
                )));
                continue;
            }
        };
        let requirement_identity = checked
            .typed
            .normalized_trait_requirement_overload_identity(owner, requirement)
            .identity();
        let matches = granted_plans
            .iter()
            .filter(|grant| {
                grant
                    .selected_plan
                    .schema
                    .methods
                    .iter()
                    .any(|method| method.requirement_identity == requirement_identity)
            })
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => {}
            [grant] => {
                receipt_updates.push((handle, grant.selected_plan_report_identity));
            }
            _ => receipt_diagnostics.push(diagnostics::Diagnostic::error(format!(
                "admitted qualification requirement `{requirement_identity}` matches {} granted selected provider plans",
                matches.len()
            ))),
        }
    }
    if !receipt_diagnostics.is_empty() {
        return Err(receipt_diagnostics);
    }
    let (spelled_operator_uses, named_operator_uses) =
        plan_selected_operator_provider_evidence(checked, candidates, &facts)?;
    let installation_reach_resolutions =
        derive_selected_installation_reach_resolutions(checked, &facts)?;
    let selected = facts
        .with_installation_reach_resolutions(installation_reach_resolutions)
        .map_err(|reason| vec![diagnostics::Diagnostic::error(reason)])?;
    let updates = SelectedProviderProgramUpdates {
        spelled_operator_uses,
        named_operator_uses,
        admitted_receipts: receipt_updates,
    };
    let mut bound_program = Arc::clone(program);
    if !updates.is_empty() {
        updates.apply(Arc::make_mut(&mut bound_program));
    }
    Ok(SelectedProviderPlanBinding {
        program: bound_program,
        selected,
        grants: authored_provider_grants,
    })
}

fn derive_selected_installation_reach_resolutions(
    checked: &checked_trees::CheckedTrees,
    selected: &effects::SelectedProviderPlanFacts,
) -> Result<Vec<effects::InstallationReachResolution>, Vec<diagnostics::Diagnostic>> {
    let mut resolutions = Vec::new();
    let mut diagnostics = Vec::new();
    for plan in selected.plans() {
        let top_level_requirements = checked
            .typed
            .machines()
            .iter()
            .filter(|requirement| {
                requirement.supply_mode
                    == language_semantics::MachineSupplyMode::TopLevelRequirement
                    && ServiceSchema::from_typed_boundary_requirement(&checked.typed, requirement)
                        .as_ref()
                        == Some(&plan.schema)
            })
            .collect::<Vec<_>>();
        match top_level_requirements.as_slice() {
            [requirement] => {
                for row in &plan.rows {
                    append_top_level_installation_reach_resolution(
                        checked,
                        plan,
                        row,
                        requirement,
                        &mut resolutions,
                        &mut diagnostics,
                    );
                }
                continue;
            }
            [] => {}
            requirements => {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "selected provider schema `{}` resolves to {} exact top-level boundary requirements",
                    plan.schema.trait_name,
                    requirements.len(),
                )));
                continue;
            }
        }
        // Boundary operators share the provider-plan carrier, but they are
        // compiler-owned operator slots rather than boundary-trait
        // requirements. Candidate validation has already replayed their exact
        // typed operator schema. Do not make the trait-only installation-reach
        // pass reinterpret them as missing trait requirements.
        let is_boundary_operator_plan = checked.typed.operators().iter().any(|operator| {
            effects::provider_plan::ServiceSchema::from_typed_operator(&checked.typed, operator)
                .as_ref()
                == Some(&plan.schema)
        });
        if is_boundary_operator_plan {
            continue;
        }
        for row in &plan.rows {
            let requirements = checked
                .typed
                .traits()
                .iter()
                .flat_map(|owner| {
                    checked
                        .typed
                        .trait_machine_signatures(owner)
                        .iter()
                        .filter(move |requirement| {
                            checked
                                .typed
                                .normalized_trait_requirement_overload_identity(owner, requirement)
                                .identity()
                                == row.requirement_identity
                        })
                        .map(move |requirement| (owner, requirement))
                })
                .collect::<Vec<_>>();
            let [(_, requirement)] = requirements.as_slice() else {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "selected provider row `{}` resolves to {} exact typed requirements",
                    row.requirement_identity,
                    requirements.len()
                )));
                continue;
            };
            if !requirement.service_reach_is_installation_bound {
                continue;
            }

            let realization_machines = checked
                .typed
                .machines()
                .iter()
                .filter(|machine| {
                    machine
                        .attached_data
                        .as_ref()
                        .map(|name| name.as_str())
                        .unwrap_or_default()
                        == plan.provider_type
                })
                .filter(|machine| {
                    checked
                        .typed
                        .machine_trait_conformances(machine)
                        .iter()
                        .any(|conformance| {
                            conformance.requirement.as_ref().is_some_and(|name| {
                                satisfied_requirement_identity(
                                    &checked.typed,
                                    machine.name.as_str(),
                                    conformance.name.as_str(),
                                    name.as_str(),
                                ) == row.requirement_identity
                            })
                        })
                })
                .collect::<Vec<_>>();
            let [realization] = realization_machines.as_slice() else {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "selected provider row `{}` resolves to {} exact realization machines for provider `{}`",
                    row.requirement_identity,
                    realization_machines.len(),
                    plan.provider_type
                )));
                continue;
            };
            let Some(envelope) = checked
                .facts
                .contract_plans
                .realized_envelope(realization.symbol)
            else {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "selected provider realization `{}` has no checked contract envelope",
                    realization.name
                )));
                continue;
            };
            let upper_bound = checked
                .facts
                .service_reaches
                .rows
                .services(requirement.service_reach_row)
                .iter()
                .filter_map(|service| checked.facts.service_reaches.services.definition(*service))
                .map(|definition| definition.name.clone())
                .collect();
            resolutions.push(effects::InstallationReachResolution {
                requirement_identity: row.requirement_identity.clone(),
                provider_plan_report_identity: plan.report_fingerprint(),
                upper_bound,
                resolved_row: envelope.effective_service_reach.clone(),
            });
        }
    }
    if diagnostics.is_empty() {
        Ok(resolutions)
    } else {
        Err(diagnostics)
    }
}

fn append_top_level_installation_reach_resolution(
    checked: &checked_trees::CheckedTrees,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    requirement: &typed_trees::machine::Machine,
    resolutions: &mut Vec<effects::InstallationReachResolution>,
    diagnostics: &mut Vec<diagnostics::Diagnostic>,
) {
    let requirement_identity = checked
        .typed
        .normalized_machine_overload_identity(requirement)
        .map(|identity| identity.identity())
        .unwrap_or_default();
    if row.requirement_identity != requirement_identity {
        diagnostics.push(diagnostics::Diagnostic::error(format!(
            "selected provider row `{}` does not retain exact top-level requirement `{requirement_identity}`",
            row.requirement_identity,
        )));
        return;
    }
    if !requirement.service_reach_is_installation_bound {
        return;
    }
    let realizations = checked
        .typed
        .machines()
        .iter()
        .filter(|machine| {
            machine
                .attached_data
                .as_ref()
                .is_some_and(|name| name.as_str() == plan.provider_type)
        })
        .filter(|machine| {
            checked
                .typed
                .machine_trait_conformances(machine)
                .iter()
                .any(|conformance| {
                    conformance.symbol == requirement.symbol
                        && conformance.requirement_symbol == requirement.symbol
                        && matches!(
                            typed_trees::machine::resolve_satisfied_declaration(
                                &checked.typed,
                                machine,
                                conformance,
                            ),
                            Some(
                                typed_trees::machine::SatisfiedDeclaration::TopLevelRequirement(
                                    selected,
                                ),
                            ) if selected.symbol == requirement.symbol
                        )
                })
        })
        .collect::<Vec<_>>();
    let [realization] = realizations.as_slice() else {
        diagnostics.push(diagnostics::Diagnostic::error(format!(
            "selected top-level requirement row `{requirement_identity}` resolves to {} exact realization machines for provider `{}`",
            realizations.len(),
            plan.provider_type,
        )));
        return;
    };
    let Some(envelope) = checked
        .facts
        .contract_plans
        .realized_envelope(realization.symbol)
    else {
        diagnostics.push(diagnostics::Diagnostic::error(format!(
            "selected provider realization `{}` has no checked contract envelope",
            realization.name,
        )));
        return;
    };
    let upper_bound = checked
        .facts
        .service_reaches
        .rows
        .services(requirement.service_reach_row)
        .iter()
        .filter_map(|service| checked.facts.service_reaches.services.definition(*service))
        .map(|definition| definition.name.clone())
        .collect();
    resolutions.push(effects::InstallationReachResolution {
        requirement_identity,
        provider_plan_report_identity: plan.report_fingerprint(),
        upper_bound,
        resolved_row: envelope.effective_service_reach.clone(),
    });
}

fn plan_selected_operator_provider_evidence(
    checked: &checked_trees::CheckedTrees,
    candidates: &[ProviderPlan],
    selected: &effects::SelectedProviderPlanFacts,
) -> Result<
    (
        Vec<(
            arena::Handle<checked_trees::CheckedOperatorUseFact>,
            u64,
            checked_trees::CheckedProviderPlanCommitment,
        )>,
        Vec<(
            arena::Handle<checked_trees::CheckedNamedOperatorUseFact>,
            u64,
            checked_trees::CheckedProviderPlanCommitment,
        )>,
    ),
    Vec<diagnostics::Diagnostic>,
> {
    // Validate selected operator plans independently of use-site discovery.
    // A malformed realization is invalid policy even when dead code happens
    // not to mention its requirement, and later annotation may consume only
    // plans that passed this gate.
    let mut diagnostics = Vec::new();
    for plan in selected.plans() {
        let operator = checked.typed.operators().iter().find(|operator| {
            operator.is_boundary
                && typed_trees::operator::boundary_operator_requirement_identity(
                    &checked.typed,
                    operator,
                ) == plan.schema.trait_name
        });
        let Some(operator) = operator else {
            continue;
        };
        if let Err(diagnostic) = selected_operator_provider_evidence(
            checked,
            candidates,
            selected,
            operator.symbol,
            None,
        ) {
            diagnostics.push(diagnostic);
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let spelled = checked
        .facts
        .operators
        .uses
        .iter()
        .map(|(handle, operator_use)| {
            (
                handle,
                operator_use.expression,
                operator_use.origin,
                operator_use.selected_operator_symbol,
            )
        })
        .collect::<Vec<_>>();
    let named = checked
        .facts
        .operators
        .named_uses
        .iter()
        .map(|(handle, operator_use)| {
            (
                handle,
                operator_use.expression,
                operator_use.origin,
                operator_use.selected_operator_symbol,
            )
        })
        .collect::<Vec<_>>();
    let mut spelled_updates = Vec::new();
    for (handle, expression, origin, symbol) in spelled {
        match selected_operator_provider_evidence(
            checked,
            candidates,
            selected,
            symbol,
            Some((expression, origin)),
        ) {
            Ok(Some((report_fingerprint, commitment))) => {
                spelled_updates.push((handle, report_fingerprint, commitment));
            }
            Ok(None) => {}
            Err(diagnostic) => diagnostics.push(diagnostic),
        }
    }
    let mut named_updates = Vec::new();
    for (handle, expression, origin, symbol) in named {
        match selected_operator_provider_evidence(
            checked,
            candidates,
            selected,
            symbol,
            Some((expression, origin)),
        ) {
            Ok(Some((report_fingerprint, commitment))) => {
                named_updates.push((handle, report_fingerprint, commitment));
            }
            Ok(None) => {}
            Err(diagnostic) => diagnostics.push(diagnostic),
        }
    }

    if diagnostics.is_empty() {
        Ok((spelled_updates, named_updates))
    } else {
        Err(diagnostics)
    }
}

fn selected_operator_provider_evidence(
    checked: &checked_trees::CheckedTrees,
    candidates: &[ProviderPlan],
    selected: &effects::SelectedProviderPlanFacts,
    operator_symbol: symbols::SymbolHandle,
    use_site: Option<(
        typed_trees::expression::ExpressionHandle,
        checked_trees::CheckedValueOrigin,
    )>,
) -> Result<Option<(u64, checked_trees::CheckedProviderPlanCommitment)>, diagnostics::Diagnostic> {
    let Some(operator) = checked
        .typed
        .operators()
        .iter()
        .find(|operator| operator.symbol == operator_symbol)
    else {
        return Ok(None);
    };
    let slot =
        typed_trees::operator::boundary_operator_requirement_identity(&checked.typed, operator);
    if !candidates
        .iter()
        .any(|candidate| candidate.schema.trait_name == slot)
    {
        return Ok(None);
    }
    let Some(plan) = selected
        .plans()
        .iter()
        .find(|plan| plan.schema.trait_name == slot)
    else {
        return Err(diagnostics::Diagnostic::error(format!(
            "boundary operator `{slot}` has provider candidates but no exact selected ProviderPlan realization for this target"
        )));
    };
    let [row] = plan.rows.as_slice() else {
        return Err(diagnostics::Diagnostic::error(format!(
            "selected boundary-operator ProviderPlan `{}` must contain exactly one realization row",
            plan.name,
        )));
    };
    if let ProviderBinding::CheckedAdapter {
        machine_identity,
        machine_package_identity,
    } = &row.binding
    {
        let [namespace, requirement] = checked.typed.operator_path_members(operator.name) else {
            return Err(diagnostics::Diagnostic::error(format!(
                "selected checked boundary-operator ProviderPlan `{}` targets `{slot}`, whose source path is not the supported `Namespace::requirement` shape",
                plan.name,
            )));
        };
        let demanded_application = match use_site {
            None => None,
            Some((expression, origin)) => {
                if use_site_is_generic_template(checked, origin) {
                    // Generic templates are not executable provider evidence,
                    // even when one call happens not to mention a template
                    // binder. Concrete clones are annotated independently
                    // after final substitution. An emitted non-generic use
                    // still requires exactly one closed demand below.
                    return Ok(None);
                }
                let matching = checked
                    .facts
                    .operators
                    .boundary_applications
                    .iter()
                    .filter(|application| {
                        application.requirement_symbol == operator.symbol
                            && application.site
                                == checked_trees::CheckedBoundaryOperatorApplicationUseSite::Expression {
                                    expression,
                                    origin,
                                }
                    })
                    .collect::<Vec<_>>();
                let [application] = matching.as_slice() else {
                    return Err(diagnostics::Diagnostic::error(format!(
                        "selected checked boundary-operator use retains {} exact application demands; expected one",
                        matching.len(),
                    )));
                };
                Some(*application)
            }
        };
        let direct_provider = exact_checked_adapter(&checked.typed, plan, row);
        let specialized_providers = checked
            .typed
            .machine_specializations
            .iter()
            .filter(|specialization| {
                validation::machine_specialization_matches_template_identity(
                    &checked.typed,
                    specialization,
                    machine_identity,
                    *machine_package_identity,
                ) && specialization
                    .operator_realizations
                    .iter()
                    .any(|realization| {
                        realization.requirement_symbol == operator.symbol
                            && demanded_application.is_none_or(|application| {
                                validation::checked_operator_application_matches_realization(
                                    &checked.typed,
                                    application,
                                    realization,
                                )
                            })
                    })
            })
            .filter_map(|specialization| {
                checked
                    .typed
                    .machines()
                    .iter()
                    .find(|machine| machine.symbol == specialization.instance)
            })
            .filter(|machine| {
                checked
                    .typed
                    .symbols
                    .symbol_package_identity(machine.symbol)
                    == *machine_package_identity
            })
            .collect::<Vec<_>>();
        let requires_specialization =
            demanded_application.is_some_and(|application| !application.arguments.is_empty());
        let checked_providers = if requires_specialization {
            if specialized_providers.len() != 1 {
                return Err(diagnostics::Diagnostic::error(format!(
                    "selected checked boundary-operator ProviderPlan `{}` resolves to {} exact specializations for one demanded application",
                    plan.name,
                    specialized_providers.len(),
                )));
            }
            specialized_providers
        } else {
            match direct_provider {
                Ok(provider) => vec![provider],
                Err(direct_error) if specialized_providers.is_empty() => {
                    return Err(direct_error);
                }
                Err(_) => specialized_providers,
            }
        };
        let satisfies_slot = checked_providers.iter().all(|checked_provider| {
            checked
                .typed
                .machine_trait_conformances(checked_provider)
                .iter()
                .any(|conformance| {
                    conformance.external_binding.is_none()
                        && (typed_trees::operator::resolve_satisfied_checked_operator(
                            &checked.typed,
                            checked_provider,
                            namespace.as_str(),
                            requirement.as_str(),
                        )
                        .is_some_and(|resolved| resolved.symbol == operator.symbol)
                            || typed_trees::operator::resolve_specialized_checked_operator_application(
                                &checked.typed,
                                checked_provider,
                                namespace.as_str(),
                                requirement.as_str(),
                            )
                            .is_some_and(|(resolved, _)| resolved.symbol == operator.symbol))
                })
        });
        if !satisfies_slot {
            return Err(diagnostics::Diagnostic::error(format!(
                "selected boundary-operator ProviderPlan `{}` binds checked adapter `{machine_identity}`, but that machine does not satisfy exact slot `{slot}` with a checked body",
                plan.name,
            )));
        }
        return Ok(Some((
            plan.report_fingerprint(),
            checked_trees::CheckedProviderPlanCommitment::from_digest(
                *plan.identity_digest().as_bytes(),
            ),
        )));
    }
    let ProviderBinding::CompilerIntrinsic { machine, .. } = &row.binding else {
        return Err(diagnostics::Diagnostic::error(format!(
            "selected boundary-operator ProviderPlan `{}` uses unsupported binding `{:?}`; boundary operators require a checked adapter or compiler intrinsic",
            plan.name, row.binding,
        )));
    };
    compiler_intrinsic_diagnostic_label(&checked.typed, operator).ok_or_else(|| {
        diagnostics::Diagnostic::error(format!(
            "selected boundary-operator ProviderPlan `{}` targets `{slot}`, which has no compiler-known migrated intrinsic",
            plan.name,
        ))
    })?;
    if !intrinsic_realization_matches_operator(&checked.typed, machine, operator) {
        return Err(diagnostics::Diagnostic::error(format!(
            "selected boundary-operator ProviderPlan `{}` binds realization `{machine}`, but it does not satisfy exact slot `{slot}` as an external leaf",
            plan.name,
        )));
    }
    Ok(Some((
        plan.report_fingerprint(),
        checked_trees::CheckedProviderPlanCommitment::from_digest(
            *plan.identity_digest().as_bytes(),
        ),
    )))
}

fn use_site_is_generic_template(
    checked: &checked_trees::CheckedTrees,
    origin: checked_trees::CheckedValueOrigin,
) -> bool {
    let checked_trees::CheckedValueOrigin::StateStatement { machine_symbol, .. } = origin else {
        return false;
    };
    checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .is_some_and(|machine| !checked.typed.machine_type_parameters(machine).is_empty())
}

pub fn intrinsic_realization_matches_operator(
    typed: &TypedTrees,
    realization_machine_identity: &str,
    operator: &typed_trees::operator::OperatorDefinition,
) -> bool {
    let [namespace, requirement] = typed.operator_path_members(operator.name) else {
        return false;
    };
    typed.machines().iter().any(|machine| {
        typed
            .normalized_machine_overload_identity(machine)
            .is_some_and(|identity| identity.identity() == realization_machine_identity)
            && typed
                .machine_trait_conformances(machine)
                .iter()
                .any(|conformance| conformance.external_binding.is_some())
            && typed_trees::operator::resolve_satisfied_boundary_operator(
                typed,
                machine,
                namespace.as_str(),
                requirement.as_str(),
            )
            .is_some_and(|resolved| resolved.symbol == operator.symbol)
    })
}

/// Render the compiler-known float realization selected by an exact checked
/// operator. This label is diagnostic-only: provider identity and dispatch use
/// the normalized realization-machine symbol retained in `ProviderBinding`.
pub fn compiler_intrinsic_diagnostic_label(
    typed: &TypedTrees,
    operator: &typed_trees::operator::OperatorDefinition,
) -> Option<String> {
    if let Some(CompilerIntrinsicExecutionIdentity::PrimitiveFloatBinary { operation, format }) =
        primitive_float_binary_intrinsic_execution_identity(typed, operator)
    {
        return Some(format!("Float::{}.{}", operation.name(), format.name()));
    }
    let path = typed.operator_path_members(operator.name);
    let [namespace, requirement] = path else {
        return None;
    };
    let parameters = typed.operator_parameters(operator);
    let (operation, primitive, expected_result) = match namespace.as_str() {
        "F32" | "F64" => {
            if matches!(requirement.as_str(), "from_f64" | "from_f32") {
                let (expected_source, expected_result, source_name) =
                    match (namespace.as_str(), requirement.as_str()) {
                        ("F32", "from_f64") => (
                            typed_trees::types::PrimitiveType::F64,
                            typed_trees::types::PrimitiveType::F32,
                            "f64",
                        ),
                        ("F64", "from_f32") => (
                            typed_trees::types::PrimitiveType::F32,
                            typed_trees::types::PrimitiveType::F64,
                            "f32",
                        ),
                        _ => return None,
                    };
                let [value] = parameters else {
                    return None;
                };
                if typed.primitive_type_reference(value.type_reference) != Some(expected_source)
                    || typed.primitive_type_reference(operator.return_type) != Some(expected_result)
                {
                    return None;
                }
                return Some(format!(
                    "{}::{}.{source_name}",
                    namespace.as_str(),
                    requirement.as_str()
                ));
            }
            if let Some(source_name) = requirement.as_str().strip_prefix("from_") {
                let expected_source = match source_name {
                    "i8" => typed_trees::types::PrimitiveType::I8,
                    "i16" => typed_trees::types::PrimitiveType::I16,
                    "i32" => typed_trees::types::PrimitiveType::I32,
                    "i64" => typed_trees::types::PrimitiveType::I64,
                    "u8" => typed_trees::types::PrimitiveType::U8,
                    "u16" => typed_trees::types::PrimitiveType::U16,
                    "u32" => typed_trees::types::PrimitiveType::U32,
                    "u64" => typed_trees::types::PrimitiveType::U64,
                    _ => return None,
                };
                let expected_result = if namespace.as_str() == "F32" {
                    typed_trees::types::PrimitiveType::F32
                } else {
                    typed_trees::types::PrimitiveType::F64
                };
                let [value] = parameters else {
                    return None;
                };
                if typed.primitive_type_reference(value.type_reference) != Some(expected_source)
                    || typed.primitive_type_reference(operator.return_type) != Some(expected_result)
                {
                    return None;
                }
                return Some(format!(
                    "{}::{}.{source_name}",
                    namespace.as_str(),
                    requirement.as_str()
                ));
            }
            let operation = match requirement.as_str() {
                "minimum" | "maximum" => requirement.as_str(),
                "negate"
                | "square_root"
                | "square_root_toward_zero"
                | "square_root_toward_positive"
                | "square_root_toward_negative"
                | "classify"
                | "is_nan"
                | "is_finite"
                | "is_infinite"
                | "is_normal"
                | "is_subnormal" => requirement.as_str(),
                "multiply_then_add"
                | "fused_multiply_add"
                | "fused_multiply_add_toward_zero"
                | "fused_multiply_add_toward_positive"
                | "fused_multiply_add_toward_negative" => requirement.as_str(),
                "add_toward_zero" | "add_toward_positive" | "add_toward_negative" => {
                    requirement.as_str()
                }
                "subtract_toward_zero"
                | "subtract_toward_positive"
                | "subtract_toward_negative" => requirement.as_str(),
                "multiply_toward_zero"
                | "multiply_toward_positive"
                | "multiply_toward_negative" => requirement.as_str(),
                "divide_toward_zero" | "divide_toward_positive" | "divide_toward_negative" => {
                    requirement.as_str()
                }
                _ => return None,
            };
            let expected_primitive = if namespace.as_str() == "F32" {
                typed_trees::types::PrimitiveType::F32
            } else {
                typed_trees::types::PrimitiveType::F64
            };
            match parameters {
                [value]
                    if matches!(
                        operation,
                        "negate"
                            | "square_root"
                            | "square_root_toward_zero"
                            | "square_root_toward_positive"
                            | "square_root_toward_negative"
                            | "classify"
                            | "is_nan"
                            | "is_finite"
                            | "is_infinite"
                            | "is_normal"
                            | "is_subnormal"
                    ) =>
                {
                    if typed.primitive_type_reference(value.type_reference)
                        != Some(expected_primitive)
                    {
                        return None;
                    }
                }
                [left, right]
                    if matches!(
                        operation,
                        "minimum"
                            | "maximum"
                            | "add_toward_zero"
                            | "add_toward_positive"
                            | "add_toward_negative"
                            | "subtract_toward_zero"
                            | "subtract_toward_positive"
                            | "subtract_toward_negative"
                            | "multiply_toward_zero"
                            | "multiply_toward_positive"
                            | "multiply_toward_negative"
                            | "divide_toward_zero"
                            | "divide_toward_positive"
                            | "divide_toward_negative"
                    ) =>
                {
                    if typed.primitive_type_reference(left.type_reference)
                        != Some(expected_primitive)
                        || typed.primitive_type_reference(right.type_reference)
                            != Some(expected_primitive)
                    {
                        return None;
                    }
                }
                [left, right, addend]
                    if matches!(
                        operation,
                        "multiply_then_add"
                            | "fused_multiply_add"
                            | "fused_multiply_add_toward_zero"
                            | "fused_multiply_add_toward_positive"
                            | "fused_multiply_add_toward_negative"
                    ) =>
                {
                    if typed.primitive_type_reference(left.type_reference)
                        != Some(expected_primitive)
                        || typed.primitive_type_reference(right.type_reference)
                            != Some(expected_primitive)
                        || typed.primitive_type_reference(addend.type_reference)
                            != Some(expected_primitive)
                    {
                        return None;
                    }
                }
                _ => return None,
            }
            if operation == "classify" {
                if typed.display_type_reference(operator.return_type) != "FloatClass" {
                    return None;
                }
                let format = if expected_primitive == typed_trees::types::PrimitiveType::F32 {
                    "f32"
                } else {
                    "f64"
                };
                return Some(format!("{}::classify.{format}", namespace.as_str()));
            }
            let expected_result = if matches!(
                operation,
                "is_nan" | "is_finite" | "is_infinite" | "is_normal" | "is_subnormal"
            ) {
                typed_trees::types::PrimitiveType::Bool
            } else {
                expected_primitive
            };
            (operation, expected_primitive, expected_result)
        }
        "I8" | "I16" | "I32" | "I64" | "U8" | "U16" | "U32" | "U64" => {
            let expected_result = match namespace.as_str() {
                "I8" => typed_trees::types::PrimitiveType::I8,
                "I16" => typed_trees::types::PrimitiveType::I16,
                "I32" => typed_trees::types::PrimitiveType::I32,
                "I64" => typed_trees::types::PrimitiveType::I64,
                "U8" => typed_trees::types::PrimitiveType::U8,
                "U16" => typed_trees::types::PrimitiveType::U16,
                "U32" => typed_trees::types::PrimitiveType::U32,
                "U64" => typed_trees::types::PrimitiveType::U64,
                _ => unreachable!(),
            };
            let (expected_source, source_name) = match requirement.as_str() {
                "from_f32" => (typed_trees::types::PrimitiveType::F32, "f32"),
                "from_f64" => (typed_trees::types::PrimitiveType::F64, "f64"),
                _ => return None,
            };
            let [value] = parameters else {
                return None;
            };
            if typed.primitive_type_reference(value.type_reference) != Some(expected_source)
                || typed.primitive_type_reference(operator.return_type) != Some(expected_result)
            {
                return None;
            }
            let policy = match typed
                .type_reference_table
                .arithmetic_domain(operator.return_type)
            {
                numerics::arithmetic::ArithmeticDomain::Exact => "exact",
                numerics::arithmetic::ArithmeticDomain::Trapping => "trapping",
                numerics::arithmetic::ArithmeticDomain::Saturating => "saturating",
                numerics::arithmetic::ArithmeticDomain::Wrapping => return None,
            };
            return Some(format!(
                "{}::{}.{source_name}.{policy}",
                namespace.as_str(),
                requirement.as_str()
            ));
        }
        _ => return None,
    };
    if typed.primitive_type_reference(operator.return_type) != Some(expected_result) {
        return None;
    }
    let format = match primitive {
        typed_trees::types::PrimitiveType::F32 => "f32",
        typed_trees::types::PrimitiveType::F64 => "f64",
        _ => return None,
    };
    Some(format!("{}::{operation}.{format}", namespace.as_str()))
}

fn provider_type_package_identity(
    typed: &TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> Option<semantic_vocabulary::PackageKeyIdentity> {
    provider_type_symbol(typed, machine)
        .and_then(|symbol| typed.symbols.symbol_package_identity(symbol))
}

fn provider_type_symbol(
    typed: &TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> Option<symbols::SymbolHandle> {
    let attached_data = machine.attached_data.as_ref()?;
    let mut owners = typed
        .data_definitions()
        .iter()
        .filter(|definition| definition.name == *attached_data);
    let owner = owners.next()?;
    owners.next().is_none().then_some(owner.symbol)
}

/// PRV4 order step (2): derive plans from explicit SATISFIES edges -- one
/// plan per (provider type, boundary trait, target), assembled only from
/// that provider's conformance closure. External leaves and checked adapters
/// attached to the same provider type join one plan. External leaves may be
/// free declarations; checked adapters must belong to a nominal provider type
/// so execution can only dispatch through a retained whole-provider selection.
/// Coverage never combines unrelated provider types. Coverage/signatures come from the typed schema
/// (signature refinement is enforced by the conformance checker on each
/// edge); the effect surface is the union of the SATISFIED requirements'
/// declared effects -- the requirement supplies the ceiling, never the
/// leaf. Selection v1: a slot whose (trait, target) has exactly one FULLY
/// COVERING derived plan selects it implicitly; ambiguity or partial
/// coverage is loud at the consumer (the trust report shows coverage).
pub fn derive_satisfies_plans(
    typed: &TypedTrees,
    selected_target: Option<&str>,
) -> Vec<ProviderPlan> {
    derive_satisfies_plans_with_provenance(typed, selected_target)
        .into_iter()
        .map(|derived| derived.plan)
        .collect()
}

/// Reject a cycle in the direct synchronous graph realized by the concrete
/// provider selection. Reach closure is intentionally irrelevant: only a
/// selected method's authored `invokes` edges participate, and a missing
/// selected target cannot manufacture an edge.
pub fn validate_selected_synchronous_invocation_cycles(
    typed: &TypedTrees,
    selected_plans: &[effects::provider_plan::ProviderPlan],
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let selected = exact_selected_synchronous_plans(selected_plans)?;
    let inferred = flow_effects::infer_synchronous_invocations(typed);
    let mut edges = vec![Vec::<usize>::new(); selected.len()];
    let mut diagnostics = Vec::new();
    for (source_index, source) in selected.iter().enumerate() {
        for method in &source.schema.methods {
            let row = match exact_row_for_schema_method(source, method) {
                Ok(row) => row,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };
            let target_names = match &row.binding {
                ProviderBinding::CheckedAdapter { .. } => {
                    exact_checked_adapter_invocations(typed, &inferred, source, method, row)
                }
                _ => exact_authored_invocations(source, method),
            };
            let target_names = match target_names {
                Ok(target_names) => target_names,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };
            for target_name in target_names {
                let matching_targets = selected
                    .iter()
                    .enumerate()
                    .filter(|(_, target)| target.schema.trait_name == target_name)
                    .collect::<Vec<_>>();
                match matching_targets.as_slice() {
                    [] => {}
                    [(target_index, _)] if !edges[source_index].contains(target_index) => {
                        edges[source_index].push(*target_index);
                    }
                    [_] => {}
                    _ => diagnostics.push(diagnostics::Diagnostic::error(format!(
                        "selected synchronous invocation `{target_name}` from `{}::{}` is ambiguous across {} package-qualified boundary slots",
                        source.schema.trait_name,
                        method.name,
                        matching_targets.len(),
                    ))),
                }
            }
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let mut color = vec![0u8; selected.len()];
    let mut path = Vec::new();
    for start in 0..selected.len() {
        if color[start] == 0
            && let Some(cycle) = synchronous_cycle_from(start, &edges, &mut color, &mut path)
        {
            let names = cycle
                .iter()
                .map(|index| selected[*index].schema.trait_name.as_str())
                .chain(std::iter::once(
                    selected[cycle[0]].schema.trait_name.as_str(),
                ))
                .collect::<Vec<_>>()
                .join(" -> ");
            return Err(vec![diagnostics::Diagnostic::error(format!(
                "selected providers realize a cyclic synchronous `invokes` graph: {names}; break one edge with a mailbox, queue, scheduler handoff, or other new activation",
            ))]);
        }
    }
    Ok(())
}

fn exact_selected_synchronous_plans(
    selected_plans: &[ProviderPlan],
) -> Result<Vec<&ProviderPlan>, Vec<diagnostics::Diagnostic>> {
    let mut selected = Vec::new();
    let mut diagnostics = Vec::new();
    let mut seen_plans = Vec::new();
    for plan in selected_plans {
        if plan.name.is_empty() {
            diagnostics.push(diagnostics::Diagnostic::error(
                "selected synchronous-invocation ProviderPlan name is empty",
            ));
            continue;
        }
        if seen_plans.contains(&plan) {
            diagnostics.push(diagnostics::Diagnostic::error(format!(
                "selected synchronous-invocation ProviderPlan `{}` is listed more than once",
                plan.name,
            )));
            continue;
        }
        seen_plans.push(plan);
        selected.push(plan);
    }

    let mut seen_schemas = Vec::new();
    for plan in &selected {
        if plan.schema.trait_name.is_empty() {
            diagnostics.push(diagnostics::Diagnostic::error(format!(
                "selected synchronous-invocation ProviderPlan `{}` has an empty exact schema identity",
                plan.name,
            )));
            continue;
        }
        let schema_identity = (
            plan.schema.trait_package_identity,
            plan.schema.trait_name.as_str(),
        );
        if seen_schemas.contains(&schema_identity) {
            diagnostics.push(diagnostics::Diagnostic::error(format!(
                "selected synchronous-invocation schema `{}` is realized by more than one selected ProviderPlan",
                plan.schema.trait_name,
            )));
            continue;
        }
        seen_schemas.push(schema_identity);
    }

    if diagnostics.is_empty() {
        Ok(selected)
    } else {
        Err(diagnostics)
    }
}

fn synchronous_cycle_from(
    node: usize,
    edges: &[Vec<usize>],
    color: &mut [u8],
    path: &mut Vec<usize>,
) -> Option<Vec<usize>> {
    color[node] = 1;
    path.push(node);
    for target in &edges[node] {
        if color[*target] == 0 {
            if let Some(cycle) = synchronous_cycle_from(*target, edges, color, path) {
                return Some(cycle);
            }
        } else if color[*target] == 1 {
            let start = path.iter().position(|member| member == target)?;
            return Some(path[start..].to_vec());
        }
    }
    path.pop();
    color[node] = 2;
    None
}

type ProviderSelectionKey = (Option<semantic_vocabulary::PackageKeyIdentity>, String);

fn provider_slot_key(plan: &effects::provider_plan::ProviderPlan) -> ProviderSelectionKey {
    (
        plan.schema.trait_package_identity,
        plan.schema.trait_name.clone(),
    )
}

fn selected_subject_keys(selection: &crate::ProviderSelection) -> Vec<ProviderSelectionKey> {
    match &selection.subject {
        crate::ProviderSelectionSubject::BoundaryTrait(identity)
        | crate::ProviderSelectionSubject::BoundaryRequirement(identity) => {
            vec![(identity.package, identity.canonical_path.clone())]
        }
        crate::ProviderSelectionSubject::BoundaryOperatorFamily(family) => family
            .coordinates()
            .iter()
            .map(|coordinate| (family.package, coordinate.requirement_identity.clone()))
            .collect(),
    }
}

fn selected_provider_key(selection: &crate::ProviderSelection) -> ProviderSelectionKey {
    (
        selection.provider_type.package,
        selection.provider_type.canonical_path.clone(),
    )
}

fn provider_plan_key(plan: &effects::provider_plan::ProviderPlan) -> ProviderSelectionKey {
    (
        plan.provider_type_package_identity,
        plan.provider_type.clone(),
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderSelectionProvenance {
    BuildOverride(Vec<crate::ProviderSelection>),
    TargetDefault(Vec<crate::ProviderSelection>),
    UniqueCoveringCandidate,
}

impl ProviderSelectionProvenance {
    /// Reconstruct the one build-owned composition choice retained by this
    /// selected plan. Automatic unique selection and target defaults are
    /// always fused: provider packages cannot independently componentize
    /// themselves.
    pub fn composition_mode(&self) -> Result<crate::CompositionMode, String> {
        match self {
            Self::UniqueCoveringCandidate => Ok(crate::CompositionMode::Fused),
            Self::TargetDefault(declarations) => {
                if declarations.iter().any(|declaration| {
                    declaration.composition_mode != crate::CompositionMode::Fused
                }) {
                    return Err(
                        "target-provider defaults cannot request independent composition; only the owner-controlled build may create that deployment cut"
                            .into(),
                    );
                }
                Ok(crate::CompositionMode::Fused)
            }
            Self::BuildOverride(declarations) => {
                let Some(first) = declarations.first() else {
                    return Err(
                        "selected provider plan has a build override without a declaration".into(),
                    );
                };
                if declarations
                    .iter()
                    .any(|declaration| declaration.composition_mode != first.composition_mode)
                {
                    return Err(
                        "selected provider plan has conflicting build-owned composition modes"
                            .into(),
                    );
                }
                Ok(first.composition_mode)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedProviderPlanWithProvenance {
    pub derived: DerivedProviderPlan,
    pub selected_by: ProviderSelectionProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedProviderReviewProvenance {
    pub plan: ProviderPlan,
    pub provider: ProviderPlanProvenance,
    pub selected_by: ProviderSelectionProvenance,
    /// Closed compiler-owned execution identity for each provider row.
    ///
    /// Selection initially leaves this empty because exact execution is not
    /// settled until after checking. The compiler must replace it with one
    /// row-aligned entry per plan row before publishing `CheckedCompilation`.
    /// `Some` is reserved for compiler-intrinsic rows whose selected
    /// execution has a closed identity; all other rows retain `None`.
    pub row_compiler_intrinsic_executions: Vec<Option<CompilerIntrinsicExecutionIdentity>>,
}

pub fn selected_provider_plan_facts_with_provenance(
    typed: &TypedTrees,
    evaluated_bindings: &crate::evaluated_via_bindings::EvaluatedViaBindingTable,
    mut selected: Vec<SelectedProviderPlanWithProvenance>,
) -> Result<
    (
        effects::SelectedProviderPlanFacts,
        Vec<SelectedProviderReviewProvenance>,
    ),
    Vec<diagnostics::Diagnostic>,
> {
    selected.sort_by(|left, right| {
        let left = &left.derived.plan;
        let right = &right.derived.plan;
        left.name
            .cmp(&right.name)
            .then_with(|| {
                left.origin_package_identity
                    .cmp(&right.origin_package_identity)
            })
            .then_with(|| {
                left.provider_type_package_identity
                    .cmp(&right.provider_type_package_identity)
            })
            .then_with(|| {
                left.schema
                    .trait_package_identity
                    .cmp(&right.schema.trait_package_identity)
            })
            .then_with(|| left.report_fingerprint().cmp(&right.report_fingerprint()))
    });

    let mut diagnostics = evaluated_bindings
        .validate_against_typed(typed)
        .err()
        .unwrap_or_default();
    let retained_target = evaluated_bindings
        .target()
        .map(target::TargetProfile::target_name)
        .unwrap_or_default();
    for selected_plan in &selected {
        let plan = &selected_plan.derived.plan;
        let provenance = &selected_plan.derived.provenance;
        let composition_mode = match selected_plan.selected_by.composition_mode() {
            Ok(mode) => mode,
            Err(reason) => {
                diagnostics.push(diagnostics::Diagnostic::error(reason));
                continue;
            }
        };
        if composition_mode == crate::CompositionMode::Independent {
            diagnostics.push(diagnostics::Diagnostic::error(format!(
                "selected provider plan `{}` retains independent composition, but its checked component closure and Service carrier have not yet been constructed; refusing to treat the edge as fused",
                plan.name,
            )));
        }
        let schema_symbol = provenance.schema.symbol();
        if plan.target != retained_target {
            diagnostics.push(diagnostics::Diagnostic::error(format!(
                "selected ProviderPlan `{}` target `{}` disagrees with evaluated-binding target `{retained_target}`",
                plan.name, plan.target,
            )));
        }
        diagnostics.extend(validate_derived_provider_plan_provenance(
            typed,
            evaluated_bindings,
            &selected_plan.derived,
        ));
        let declarations = match &selected_plan.selected_by {
            ProviderSelectionProvenance::BuildOverride(declarations)
            | ProviderSelectionProvenance::TargetDefault(declarations) => declarations,
            ProviderSelectionProvenance::UniqueCoveringCandidate => continue,
        };
        if declarations.is_empty() {
            diagnostics.push(diagnostics::Diagnostic::error(format!(
                "selected provider plan `{}` has an authored selection origin without a declaration",
                plan.name,
            )));
        }
        for declaration in declarations {
            let selecting_source = typed
                .symbols
                .symbol_provenance_source_span(declaration.selecting_machine);
            if !declaration
                .subject
                .selects_schema(schema_symbol, &plan.schema.trait_name)
                || Some(declaration.provider_type.symbol) != provenance.provider_type
                || selecting_source.is_none_or(|source| {
                    source.source_id != declaration.source_span.source_id
                        || declaration.source_span.span.start >= declaration.source_span.span.end
                })
            {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "selected provider plan `{}` has a selection declaration outside its exact schema, provider, or selecting-machine provenance",
                    plan.name,
                )));
            }
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let plans = selected
        .iter()
        .map(|selected| selected.derived.plan.clone())
        .collect::<Vec<_>>();
    let facts = effects::SelectedProviderPlanFacts::from_selected_plans(plans.clone())
        .map_err(|reason| vec![diagnostics::Diagnostic::error(reason)])?;
    if facts.plans() != plans
        || plans.iter().any(|plan| {
            facts
                .plan_by_exact_evidence(plan.report_fingerprint(), plan)
                .is_none()
        })
    {
        return Err(vec![diagnostics::Diagnostic::error(
            "selected provider semantic facts do not retain the exact plans aligned with provenance",
        )]);
    }
    let provenance = selected
        .into_iter()
        .map(|selected| SelectedProviderReviewProvenance {
            plan: selected.derived.plan,
            provider: selected.derived.provenance,
            selected_by: selected.selected_by,
            row_compiler_intrinsic_executions: Vec::new(),
        })
        .collect();
    Ok((facts, provenance))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SelectedProviderPlanIndex {
    candidate: usize,
    selected_by: ProviderSelectionProvenance,
}

fn resolve_provider_selection_slots(
    slot_keys: &[ProviderSelectionKey],
    declarations: &[crate::ProviderSelection],
    owner: &str,
) -> (
    Vec<(ProviderSelectionKey, crate::ProviderSelection)>,
    Vec<diagnostics::Diagnostic>,
) {
    let mut resolved = Vec::new();
    let mut diagnostics = Vec::new();
    for declaration in declarations {
        for slot_key in selected_subject_keys(declaration) {
            if slot_keys.contains(&slot_key) {
                resolved.push((slot_key, declaration.clone()));
            } else {
                let message = match &declaration.subject {
                    crate::ProviderSelectionSubject::BoundaryTrait(identity) => format!(
                        "{owner} selects provider `{}` for unknown boundary slot `{}`; the slot must exist in the loaded dependency closure",
                        declaration.provider_type.authored_path, identity.authored_path,
                    ),
                    crate::ProviderSelectionSubject::BoundaryRequirement(identity) => format!(
                        "{owner} selects provider `{}` for unknown top-level boundary requirement `{}`; the requirement must exist in the loaded dependency closure",
                        declaration.provider_type.authored_path, identity.authored_path,
                    ),
                    crate::ProviderSelectionSubject::BoundaryOperatorFamily(_) => format!(
                        "{owner} selects provider `{}` for unknown boundary coordinate `{}` in subject `{}`; every selected coordinate must exist in the loaded dependency closure",
                        declaration.provider_type.authored_path,
                        slot_key.1,
                        declaration.subject.authored_path(),
                    ),
                };
                diagnostics.push(diagnostics::Diagnostic::error(message));
            }
        }
    }
    (resolved, diagnostics)
}

/// PRV4c: select one fully covering provider type per applicable boundary
/// slot. An explicit build-root declaration wins over the selected target
/// package's ordinary default declaration. Without either, a unique covering
/// candidate supplies the declaration-era default. Rows are never selected
/// individually and partial candidates never combine.
pub fn select_provider_plans(
    plans: &[effects::provider_plan::ProviderPlan],
    selected_target: target::NativeTarget,
    defaults: &[crate::ProviderSelection],
    requested: &[crate::ProviderSelection],
) -> Result<Vec<ProviderPlan>, Vec<diagnostics::Diagnostic>> {
    select_provider_plan_indices(plans, selected_target, defaults, requested).map(|selected| {
        selected
            .into_iter()
            .map(|selected| plans[selected.candidate].clone())
            .collect()
    })
}

pub fn select_provider_plans_with_provenance(
    derived: &[DerivedProviderPlan],
    selected_target: target::NativeTarget,
    defaults: &[crate::ProviderSelection],
    requested: &[crate::ProviderSelection],
) -> Result<Vec<SelectedProviderPlanWithProvenance>, Vec<diagnostics::Diagnostic>> {
    let plans = derived
        .iter()
        .map(|derived| derived.plan.clone())
        .collect::<Vec<_>>();
    select_provider_plan_indices(&plans, selected_target, defaults, requested).map(|selected| {
        selected
            .into_iter()
            .map(|selected| SelectedProviderPlanWithProvenance {
                derived: derived[selected.candidate].clone(),
                selected_by: selected.selected_by,
            })
            .collect()
    })
}

fn select_provider_plan_indices(
    plans: &[effects::provider_plan::ProviderPlan],
    selected_target: target::NativeTarget,
    defaults: &[crate::ProviderSelection],
    requested: &[crate::ProviderSelection],
) -> Result<Vec<SelectedProviderPlanIndex>, Vec<diagnostics::Diagnostic>> {
    // Target inertness (the fail-canary host-portability convention): a
    // plan scoped to a NON-selected target is inert and never collides --
    // only plans that RESOLVE to the selected target participate.
    let applies = |target: &str| -> bool {
        if target.is_empty() {
            return true; // portable: every target
        }
        target::NativeTarget::from_omega_target_name(Some(target))
            .is_ok_and(|resolved| resolved == selected_target)
    };
    let mut diagnostics = Vec::new();
    let mut selected = Vec::new();
    let mut slot_keys: Vec<ProviderSelectionKey> = plans
        .iter()
        .filter(|plan| !plan.schema.methods.is_empty())
        .map(provider_slot_key)
        .collect();
    slot_keys.sort_unstable();
    slot_keys.dedup();

    // Provider selections arrive after ordinary name resolution. Preserve
    // that exact nominal identity: readable paths are diagnostic material and
    // may never repair or approximate a package-qualified identity.
    let (resolved_requests, request_diagnostics) =
        resolve_provider_selection_slots(&slot_keys, requested, "build");
    diagnostics.extend(request_diagnostics);
    let (resolved_defaults, default_diagnostics) =
        resolve_provider_selection_slots(&slot_keys, defaults, "target package");
    diagnostics.extend(default_diagnostics);
    for slot_key in &slot_keys {
        let declarations = resolved_requests
            .iter()
            .filter(|(slot, _)| slot == slot_key)
            .map(|(_, declaration)| declaration)
            .collect::<Vec<_>>();
        if declarations.len() > 1 {
            let slot_name = &slot_key.1;
            diagnostics.push(diagnostics::Diagnostic::error(format!(
                "build declares provider selection for slot `{slot_name}` more than once: {}",
                declarations
                    .iter()
                    .map(|declaration| format!(
                        "`{} -> {}`",
                        declaration.subject.authored_path(),
                        declaration.provider_type.authored_path,
                    ))
                    .collect::<Vec<_>>()
                    .join(", "),
            )));
        }
    }

    for slot_key in slot_keys {
        let slot_name = &slot_key.1;
        let explicit = resolved_requests
            .iter()
            .find(|(slot, _)| slot == &slot_key)
            .map(|(_, selection)| selection);
        let slot_defaults: Vec<_> = resolved_defaults
            .iter()
            .filter(|(slot, _)| slot == &slot_key)
            .map(|(_, selection)| selection)
            .collect();
        let candidates: Vec<(usize, &ProviderPlan)> = plans
            .iter()
            .enumerate()
            .filter(|(_, plan)| provider_slot_key(plan) == slot_key && applies(&plan.target))
            .collect();
        let covering: Vec<(usize, &ProviderPlan)> = candidates
            .iter()
            .copied()
            .filter(|(_, plan)| plan.covers_schema())
            .collect();

        let selected_declaration = if let Some(explicit) = explicit {
            // A slot-owner override intentionally replaces every target
            // default for this slot, including a default whose provider is
            // absent from the selected dependency closure.
            Some((
                "build",
                explicit,
                ProviderSelectionProvenance::BuildOverride(vec![explicit.clone()]),
            ))
        } else if let Some(first) = slot_defaults.first().copied() {
            let mut distinct_provider_types: Vec<ProviderSelectionKey> = slot_defaults
                .iter()
                .map(|selection| selected_provider_key(selection))
                .collect();
            distinct_provider_types.sort_unstable();
            distinct_provider_types.dedup();
            if distinct_provider_types.len() > 1 {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "slot `{slot_name}` has conflicting target-package defaults: {} -- a target supplies at most one default provider type per slot",
                    distinct_provider_types
                        .iter()
                        .map(|(_, provider)| format!("`{provider}`"))
                        .collect::<Vec<_>>()
                        .join(", "),
                )));
                continue;
            }
            Some((
                "target package",
                first,
                ProviderSelectionProvenance::TargetDefault(
                    slot_defaults
                        .iter()
                        .map(|selection| (*selection).clone())
                        .collect(),
                ),
            ))
        } else {
            None
        };

        if let Some((owner, declaration, selected_by)) = selected_declaration {
            let selected_provider = selected_provider_key(declaration);
            let matching: Vec<(usize, &ProviderPlan)> = candidates
                .iter()
                .copied()
                .filter(|(_, plan)| provider_plan_key(plan) == selected_provider)
                .collect();
            match matching.as_slice() {
                [(candidate, plan)] if plan.covers_schema() => {
                    selected.push(SelectedProviderPlanIndex {
                        candidate: *candidate,
                        selected_by,
                    });
                }
                [(_, plan)] => diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "{owner} selects provider `{}` for slot `{slot_name}`, but candidate `{}` is partial ({}/{}) and cannot be selected",
                    declaration.provider_type.authored_path,
                    plan.name,
                    plan.rows.len(),
                    plan.schema.methods.len(),
                ))),
                [] => {
                    let wrong_target = plans.iter().any(|plan| {
                        provider_slot_key(plan) == slot_key
                            && provider_plan_key(plan) == selected_provider
                    });
                    diagnostics.push(diagnostics::Diagnostic::error(format!(
                        "{owner} selects provider `{}` for slot `{slot_name}`, but no {}candidate exists in the loaded dependency closure",
                        declaration.provider_type.authored_path,
                        if wrong_target { "selected-target " } else { "" },
                    )));
                }
                _ => diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "{owner} selection `{}` for slot `{slot_name}` resolves to multiple provider candidates with the same exact identity",
                    declaration.provider_type.authored_path,
                ))),
            }
            continue;
        }

        match covering.as_slice() {
            [] => {}
            [(candidate, _)] => selected.push(SelectedProviderPlanIndex {
                candidate: *candidate,
                selected_by: ProviderSelectionProvenance::UniqueCoveringCandidate,
            }),
            many => {
                let count = if many.len() == 2 {
                    "two".to_owned()
                } else {
                    many.len().to_string()
                };
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "slot `{slot_name}` has {count} covering provider plans for the selected target: {} -- choose one in build.omg with `b.select_provider<{slot_name}, ProviderType>();`",
                    many.iter()
                        .map(|(_, plan)| format!("`{}` [{:016x}]", plan.name, plan.report_fingerprint()))
                        .collect::<Vec<_>>()
                        .join(", "),
                )));
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(selected)
    } else {
        Err(diagnostics)
    }
}

#[cfg(test)]
#[path = "plans/tests.rs"]
mod tests;
