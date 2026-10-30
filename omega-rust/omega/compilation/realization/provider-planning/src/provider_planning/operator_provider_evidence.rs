//! Selected operator provider evidence and intrinsic realization checks.

use checked_trees::typed_trees::TypedTrees;
use effects::provider_plan::{ProviderBinding, ProviderPlan};

/// Validate every selected boundary-operator plan and every checked use it
/// serves. Uses carry no selection; each target's consumers join their plan
/// through `selected_use_plan`, which reads only plans this gate accepted.
pub(crate) fn validate_selected_operator_provider_evidence(
    typed: &checked_trees::typed_trees::TypedTrees,
    check_facts: &checked_trees::CheckFacts,
    candidates: &[&ProviderPlan],
    selected: &effects::SelectedProviderPlanFacts,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    // Validate selected operator plans independently of use-site discovery.
    // A malformed realization is invalid policy even when dead code happens
    // not to mention its requirement, and later annotation may consume only
    // plans that passed this gate.
    let mut diagnostics = Vec::new();
    for plan in selected.plans() {
        let operator = typed.operators().iter().find(|operator| {
            crate::service_schema::schema_binds_exact_boundary_operator(
                crate::ProviderPlanningContext::new(typed, source::DependencyScope::Product),
                &plan.schema,
                operator,
            )
        });
        let Some(operator) = operator else {
            continue;
        };
        if let Err(diagnostic) = selected_operator_provider_evidence(
            typed,
            check_facts,
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

    let spelled = check_facts
        .operators
        .uses
        .iter()
        .map(|(_, operator_use)| {
            (
                operator_use.application_site(),
                operator_use.selected_operator_symbol,
            )
        })
        .collect::<Vec<_>>();
    let named = check_facts
        .operators
        .named_uses
        .iter()
        .map(|(_, operator_use)| {
            (
                operator_use.expression,
                operator_use.origin,
                operator_use.selected_operator_symbol,
            )
        })
        .collect::<Vec<_>>();
    for (site, symbol) in spelled {
        if let Err(diagnostic) = selected_operator_provider_evidence(
            typed,
            check_facts,
            candidates,
            selected,
            symbol,
            Some(site),
        ) {
            diagnostics.push(diagnostic);
        }
    }
    for (expression, origin, symbol) in named {
        if let Err(diagnostic) = selected_operator_provider_evidence(
            typed,
            check_facts,
            candidates,
            selected,
            symbol,
            Some(
                checked_trees::CheckedBoundaryOperatorApplicationUseSite::Expression {
                    expression,
                    origin,
                },
            ),
        ) {
            diagnostics.push(diagnostic);
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// Validate the selected plan serving each direct call to a public
/// receiver-free top-level requirement, the requirement-spelling twin of the
/// operator-use validation above. A checked-adapter plan is the direct-call
/// adapter route; a compiler-intrinsic plan must name a compiler-known
/// realization that satisfies exactly this requirement as an external leaf.
/// An evaluated `via` binding keeps the call on the requirement's retained
/// boundary seam: native realization joins the selected row's normalized
/// import or syscall to the demanded boundary by requirement identity. Only
/// mechanisms with a freestanding host-ABI route can serve a direct call;
/// vtable and table rows have no receiver table here.
pub(crate) fn validate_selected_requirement_provider_evidence(
    typed: &checked_trees::typed_trees::TypedTrees,
    check_facts: &checked_trees::CheckFacts,
    candidates: &[&ProviderPlan],
    selected: &effects::SelectedProviderPlanFacts,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let mut diagnostics = Vec::new();
    for (_, requirement_use) in check_facts.operators.named_requirement_uses.iter() {
        if let Err(diagnostic) = selected_requirement_provider_evidence(
            typed,
            candidates,
            selected,
            requirement_use.requirement_symbol,
        ) {
            diagnostics.push(diagnostic);
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn selected_requirement_provider_evidence(
    typed: &checked_trees::typed_trees::TypedTrees,
    candidates: &[&ProviderPlan],
    selected: &effects::SelectedProviderPlanFacts,
    requirement_symbol: symbols::SymbolHandle,
) -> Result<(), diagnostics::Diagnostic> {
    let Some(requirement) = crate::IntrinsicRequirement::by_symbol(typed, requirement_symbol)
    else {
        return Ok(());
    };
    if requirement.kind != crate::IntrinsicRequirementKind::TopLevelRequirement {
        return Ok(());
    }
    let slot = requirement.display();
    let provider_context =
        crate::ProviderPlanningContext::new(typed, source::DependencyScope::Product);
    if !candidates
        .iter()
        .any(|candidate| requirement.schema_binds(provider_context, &candidate.schema))
    {
        return Ok(());
    }
    let matching_selected = selected
        .plans()
        .iter()
        .filter(|plan| requirement.schema_binds(provider_context, &plan.schema))
        .collect::<Vec<_>>();
    let [plan] = matching_selected.as_slice() else {
        return Err(diagnostics::Diagnostic::error(format!(
            "boundary requirement `{slot}` in package {:?} resolves to {} selected ProviderPlans; expected exactly one",
            requirement.package_identity,
            matching_selected.len(),
        )));
    };
    let plan = *plan;
    let [row] = plan.rows.as_slice() else {
        return Err(diagnostics::Diagnostic::error(format!(
            "selected boundary-requirement ProviderPlan `{}` must contain exactly one realization row",
            plan.name,
        )));
    };
    match &row.binding {
        ProviderBinding::CheckedAdapter { .. } => {}
        ProviderBinding::CompilerIntrinsic { machine, .. } => {
            requirement
                .execution_identity(typed)
                .ok_or_else(|| {
                    diagnostics::Diagnostic::error(format!(
                        "selected boundary-requirement ProviderPlan `{}` targets `{slot}`, which has no compiler-known migrated intrinsic",
                        plan.name,
                    ))
                })?;
            if !requirement.intrinsic_realization_matches(typed, machine) {
                return Err(diagnostics::Diagnostic::error(format!(
                    "selected boundary-requirement ProviderPlan `{}` binds realization `{machine}`, but it does not satisfy exact slot `{slot}` as an external leaf",
                    plan.name,
                )));
            }
        }
        ProviderBinding::Import { .. } | ProviderBinding::Syscall { .. } => {}
        binding => {
            return Err(diagnostics::Diagnostic::error(format!(
                "selected boundary-requirement ProviderPlan `{}` uses unsupported binding `{binding:?}` for a direct call; a directly called requirement takes a checked adapter, compiler intrinsic, or evaluated import/syscall binding",
                plan.name,
            )));
        }
    }
    Ok(())
}

fn selected_operator_provider_evidence(
    typed: &checked_trees::typed_trees::TypedTrees,
    check_facts: &checked_trees::CheckFacts,
    candidates: &[&ProviderPlan],
    selected: &effects::SelectedProviderPlanFacts,
    operator_symbol: symbols::SymbolHandle,
    use_site: Option<checked_trees::CheckedBoundaryOperatorApplicationUseSite>,
) -> Result<(), diagnostics::Diagnostic> {
    let Some(operator) = typed.operator_definition_by_symbol(operator_symbol) else {
        return Ok(());
    };
    let slot = checked_trees::typed_trees::operator::boundary_operator_requirement_identity(
        typed, operator,
    );
    let operator_package = typed.symbols.symbol_package_identity(operator.symbol);
    // Canonical overload identities are package-blind; every candidate and
    // selected-plan join must also bind the exact declaration package or a
    // same-spelled foreign coordinate would substitute for this slot.
    if !candidates.iter().any(|candidate| {
        candidate.schema.identity_text(candidate.schema.trait_name) == slot
            && candidate.schema.trait_package_identity == operator_package
    }) {
        return Ok(());
    }
    let matching_selected = selected
        .plans()
        .iter()
        .filter(|plan| {
            plan.schema.identity_text(plan.schema.trait_name) == slot
                && plan.schema.trait_package_identity == operator_package
        })
        .collect::<Vec<_>>();
    let [plan] = matching_selected.as_slice() else {
        return Err(diagnostics::Diagnostic::error(format!(
            "boundary operator `{slot}` in package {:?} resolves to {} selected ProviderPlans; expected exactly one",
            operator_package,
            matching_selected.len(),
        )));
    };
    let plan = *plan;
    let [row] = plan.rows.as_slice() else {
        return Err(diagnostics::Diagnostic::error(format!(
            "selected boundary-operator ProviderPlan `{}` must contain exactly one realization row",
            plan.name,
        )));
    };
    if matches!(&row.binding, ProviderBinding::CheckedAdapter { .. }) {
        let demanded_application = match use_site {
            None => None,
            Some(site) => {
                let origin = match site {
                    checked_trees::CheckedBoundaryOperatorApplicationUseSite::Expression {
                        origin,
                        ..
                    }
                    | checked_trees::CheckedBoundaryOperatorApplicationUseSite::MatchEquality {
                        origin,
                        ..
                    } => origin,
                    _ => {
                        return Err(diagnostics::Diagnostic::error(
                            "selected operator provider requires an exact value occurrence",
                        ));
                    }
                };
                if use_site_is_generic_template(typed, origin) {
                    // Generic templates are not executable provider evidence,
                    // even when one call happens not to mention a template
                    // binder. Concrete clones are annotated independently
                    // after final substitution. An emitted non-generic use
                    // still requires exactly one closed demand below.
                    return Ok(());
                }
                let matching = check_facts
                    .operators
                    .boundary_applications
                    .iter()
                    .filter(|application| {
                        application.requirement_symbol == operator.symbol
                            && application.site == site
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
        if let Some(application) = demanded_application {
            selected_checked_operator_body(check_facts, application, plan)?;
        } else {
            // Policy still validates dormant selected declarations. Open
            // declarations do not require a concrete application or root code.
            provider_selection::select_checked_body(
                plan,
                check_facts
                    .operators
                    .boundary_declarations
                    .iter()
                    .filter(|(_, declaration)| declaration.requirement == operator.symbol)
                    .map(
                        |(_, declaration)| provider_selection::CheckedBodyCandidate {
                            body: (),
                            declaration: &declaration.selection,
                            target: declaration.selection.target.as_deref(),
                            is_product: declaration.selection.is_product,
                        },
                    ),
            )?;
        }
        return Ok(());
    }
    let ProviderBinding::CompilerIntrinsic { machine, .. } = &row.binding else {
        return Err(diagnostics::Diagnostic::error(format!(
            "selected boundary-operator ProviderPlan `{}` uses unsupported binding `{:?}`; boundary operators require a checked adapter or compiler intrinsic",
            plan.name, row.binding,
        )));
    };
    let requirement = crate::IntrinsicRequirement::from_operator(typed, operator)
        .filter(|requirement| requirement.execution_identity(typed).is_some())
        .ok_or_else(|| {
            diagnostics::Diagnostic::error(format!(
                "selected boundary-operator ProviderPlan `{}` targets `{slot}`, which has no compiler-known migrated intrinsic",
                plan.name,
            ))
        })?;
    if !requirement.intrinsic_realization_matches(typed, machine) {
        return Err(diagnostics::Diagnostic::error(format!(
            "selected boundary-operator ProviderPlan `{}` binds realization `{machine}`, but it does not satisfy exact slot `{slot}` as an external leaf",
            plan.name,
        )));
    }
    Ok(())
}

/// The selected ProviderPlan serving one checked use of a boundary operator
/// or receiver-free top-level requirement, with its index in `plans`.
/// Checked uses carry no selection: each target joins its own selected plans
/// here by requirement identity (slot and declaring package) in the caller's
/// dependency purpose, after
/// `bind_selected_provider_plan_facts` rejected ambiguous or malformed plans.
/// A use inside a pruned target-sibling body has no provider, and a checked
/// adapter never serves a generic template's use (its concrete clones are
/// served instead).
pub fn selected_use_plan<'a>(
    typed: &checked_trees::typed_trees::TypedTrees,
    scope: source::DependencyScope,
    plans: &'a [ProviderPlan],
    requirement: symbols::SymbolHandle,
    origin: checked_trees::CheckedValueOrigin,
) -> Option<(usize, &'a ProviderPlan)> {
    if let checked_trees::CheckedValueOrigin::StateStatement { machine_symbol, .. } = origin
        && !typed.machine(machine_symbol).is_some()
    {
        return None;
    }
    let (index, plan) = selected_requirement_plan(
        crate::ProviderPlanningContext::new(typed, scope),
        plans,
        requirement,
    )?;
    let is_operator = typed.operator_definition_by_symbol(requirement).is_some();
    if is_operator
        && matches!(
            plan.rows.as_slice(),
            [row] if matches!(row.binding, ProviderBinding::CheckedAdapter { .. })
        )
        && use_site_is_generic_template(typed, origin)
    {
        return None;
    }
    Some((index, plan))
}

/// The one selected ProviderPlan whose schema binds `requirement` — a boundary
/// operator, trait-method or top-level requirement — in its exact
/// declaring package and dependency purpose, with its index in `plans`.
pub fn selected_requirement_plan<'a>(
    context: crate::ProviderPlanningContext<'_>,
    plans: &'a [ProviderPlan],
    requirement: symbols::SymbolHandle,
) -> Option<(usize, &'a ProviderPlan)> {
    if !context.contains(requirement) {
        return None;
    }
    let typed = context.typed;
    if let Some((owner, method)) = typed.trait_machine_signature(requirement) {
        let identity = typed.trait_requirement_callable_identity_text(owner, method);
        if identity.is_empty() {
            return None;
        }
        let mut matching = plans.iter().enumerate().filter(|(_, plan)| {
            crate::service_schema::schema_binds_exact_boundary_trait(context, &plan.schema, owner)
                && plan
                    .schema
                    .methods
                    .iter()
                    .filter(|candidate| {
                        plan.schema.identity_text(candidate.requirement_identity)
                            == identity.as_str()
                    })
                    .count()
                    == 1
        });
        let selected = matching.next()?;
        return matching.next().is_none().then_some(selected);
    }
    let mut matching: Vec<(usize, &'a ProviderPlan)> = match typed
        .operator_definition_by_symbol(requirement)
    {
        Some(operator) => {
            let slot = checked_trees::typed_trees::operator::boundary_operator_requirement_identity(
                typed, operator,
            );
            let package = typed.symbols.symbol_package_identity(operator.symbol);
            plans
                .iter()
                .enumerate()
                .filter(|(_, plan)| {
                    plan.schema.identity_text(plan.schema.trait_name) == slot
                        && plan.schema.trait_package_identity == package
                })
                .collect()
        }
        None => {
            let requirement = crate::IntrinsicRequirement::by_symbol(typed, requirement)?;
            if requirement.kind != crate::IntrinsicRequirementKind::TopLevelRequirement {
                return None;
            }
            plans
                .iter()
                .enumerate()
                .filter(|(_, plan)| requirement.schema_binds(context, &plan.schema))
                .collect()
        }
    };
    match matching.len() {
        1 => matching.pop(),
        _ => None,
    }
}

fn use_site_is_generic_template(
    typed: &checked_trees::typed_trees::TypedTrees,
    origin: checked_trees::CheckedValueOrigin,
) -> bool {
    let checked_trees::CheckedValueOrigin::StateStatement { machine_symbol, .. } = origin else {
        return false;
    };
    typed
        .machine(machine_symbol)
        .is_some_and(|machine| !typed.machine_type_parameters(machine).is_empty())
}

pub(crate) fn provider_type_package_identity(
    typed: &TypedTrees,
    machine: &checked_trees::typed_trees::machine::Machine,
) -> Option<semantic_vocabulary::PackageKeyIdentity> {
    provider_type_symbol(typed, machine)
        .and_then(|symbol| typed.symbols.symbol_package_identity(symbol))
}

pub(crate) fn provider_type_symbol(
    typed: &TypedTrees,
    machine: &checked_trees::typed_trees::machine::Machine,
) -> Option<symbols::SymbolHandle> {
    let attached_data = machine.attached_data.as_ref()?;
    let mut owners = typed.data_definitions().iter().filter(|definition| {
        definition.symbol == machine.attached_data_symbol && definition.name == *attached_data
    });
    let owner = owners.next()?;
    owners.next().is_none().then_some(owner.symbol)
}

/// Project the exact demand's already checked bodies into source-free choice.
/// Neither admission nor checked dispatch may reconstruct this roster from
/// typed specialization or satisfaction rows.
pub fn selected_checked_operator_body<'checked>(
    facts: &'checked checked_trees::CheckFacts,
    application: &checked_trees::CheckedBoundaryOperatorApplicationDemand,
    plan: &ProviderPlan,
) -> Result<&'checked checked_trees::CheckedBoundaryBodyCandidate, diagnostics::Diagnostic> {
    let operators = &facts.operators;
    let bodies = operators
        .boundary_body_candidates
        .span(application.body_candidates)
        .ok_or_else(|| {
            diagnostics::Diagnostic::error("operator application lost its checked body span")
        })?;
    let mut candidates = Vec::with_capacity(bodies.len());
    for body in bodies {
        if !operators.boundary_declarations.is_valid(body.declaration) {
            return Err(diagnostics::Diagnostic::error(
                "checked operator body lost its declaration",
            ));
        }
        let declaration = operators.boundary_declarations.get(body.declaration);
        if declaration.requirement != application.requirement_symbol {
            return Err(diagnostics::Diagnostic::error(
                "checked operator body names another requirement",
            ));
        }
        candidates.push(provider_selection::CheckedBodyCandidate {
            body,
            declaration: &declaration.selection,
            target: body.target.as_deref(),
            is_product: body.is_product,
        });
    }
    provider_selection::select_checked_body(plan, candidates)
}
