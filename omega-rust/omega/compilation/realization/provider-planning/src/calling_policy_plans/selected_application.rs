//! Complete calling applications selected against exact boundary requirements.
//!
//! External calls and independently published checked entries use one explicit
//! policy-selection replay. Their execution/installation authority remains with
//! their different provider owners. Default native plans are materialized only
//! when an actual external or component boundary requests an ABI application.

use diagnostics::Diagnostic;

/// Resolve implementation evidence only through the provider candidate that
/// selection admitted. The public provider/schema identity carries the
/// canonical fingerprint; the typed program retains the corresponding plan
/// internally so lowering never has to rediscover or re-run policy source.
pub(crate) fn selected_source_calling_application(
    typed: &checked_trees::typed_trees::TypedTrees,
    boundary_calling_plan_realizations: &[
        crate::calling_policy_plans::BoundaryCallingPlanRealization
    ],
    plan: &effects::provider_plan::ProviderPlan,
    trait_name: &str,
    method_name: &str,
    requirement_identity: &str,
) -> Result<Option<calling_conventions::BoundaryCallingApplication>, Diagnostic> {
    if plan.name.is_empty() {
        return Err(Diagnostic::error(
            "selected source boundary entry plan has an empty ProviderPlan name",
        ));
    }
    let provider_plan_name = plan.name.as_str();
    if plan.schema.identity_text(plan.schema.trait_name) != trait_name {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry ProviderPlan `{}` serves schema `{}`, not exact requested schema `{trait_name}`",
            plan.name,
            plan.schema.identity_text(plan.schema.trait_name)
        )));
    }
    if requirement_identity.is_empty() {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry `{trait_name}::{method_name}` has an empty exact requirement overload identity"
        )));
    }

    let matching_methods = plan
        .schema
        .methods
        .iter()
        .filter(|method| {
            method.name == method_name
                && plan.schema.identity_text(method.requirement_identity) == requirement_identity
        })
        .collect::<Vec<_>>();
    let [method] = matching_methods.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry ProviderPlan `{provider_plan_name}` binds {} exact schema methods for `{trait_name}::{method_name}` / `{requirement_identity}`",
            matching_methods.len()
        )));
    };
    if method.requirement_owner.is_empty() {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry ProviderPlan `{provider_plan_name}` method `{method_name}` has an empty exact requirement owner"
        )));
    }

    let provider_context =
        crate::ProviderPlanningContext::new(typed, source::DependencyScope::Product);
    let schema_operators = typed
        .operators()
        .iter()
        .filter(|operator| {
            crate::service_schema::schema_binds_exact_boundary_operator(
                provider_context,
                &plan.schema,
                operator,
            )
        })
        .collect::<Vec<_>>();
    if !schema_operators.is_empty() {
        let [operator] = schema_operators.as_slice() else {
            return Err(Diagnostic::error(format!(
                "selected source boundary entry schema `{}` resolves to {} exact typed boundary operators",
                plan.schema.identity_text(plan.schema.trait_name),
                schema_operators.len()
            )));
        };
        let operator_identity =
            checked_trees::typed_trees::operator::boundary_operator_requirement_identity(
                typed, operator,
            );
        if method.name != "realize"
            || method.requirement_owner != operator_identity
            || plan.schema.identity_text(method.requirement_identity) != operator_identity
        {
            return Err(Diagnostic::error(format!(
                "selected source boundary entry ProviderPlan `{provider_plan_name}` does not bind exact boundary operator `{operator_identity}`"
            )));
        }
        return match (
            method.calling_plan_report_fingerprint,
            method.calling_plan_commitment,
        ) {
            (None, None) => Ok(None),
            _ => Err(Diagnostic::error(format!(
                "selected source boundary operator `{operator_identity}` retains a trait calling-plan fingerprint"
            ))),
        };
    }

    let top_level_requirements = typed
        .machines()
        .iter()
        .filter(|requirement| {
            requirement.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
                && crate::service_schema::from_typed_boundary_requirement(
                    provider_context,
                    requirement,
                )
                .as_ref()
                    == Some(&plan.schema)
        })
        .collect::<Vec<_>>();
    if !top_level_requirements.is_empty() {
        let [requirement] = top_level_requirements.as_slice() else {
            return Err(Diagnostic::error(format!(
                "selected source boundary entry schema `{}` resolves to {} exact typed top-level boundary requirements",
                plan.schema.identity_text(plan.schema.trait_name),
                top_level_requirements.len(),
            )));
        };
        let exact_requirement_identity = typed
            .machine_callable_identity_text(requirement)
            .unwrap_or_default();
        let requirement_path = typed.symbols.display_path(requirement.symbol, "::");
        let Some((requirement_owner, requirement_method)) = requirement_path.rsplit_once("::")
        else {
            return Err(Diagnostic::error(format!(
                "selected source top-level boundary requirement `{}` has no exact owner and method path",
                requirement.name,
            )));
        };
        if requirement_path != plan.schema.identity_text(plan.schema.trait_name)
            || requirement_method != method_name
            || method.requirement_owner != requirement_owner
            || plan.schema.identity_text(method.requirement_identity) != exact_requirement_identity
            || requirement_identity != exact_requirement_identity
        {
            return Err(Diagnostic::error(format!(
                "selected source boundary entry ProviderPlan `{provider_plan_name}` does not bind exact top-level boundary requirement `{}`",
                requirement.name,
            )));
        }
        return match (
            method.calling_plan_report_fingerprint,
            method.calling_plan_commitment,
        ) {
            (None, None) => Ok(None),
            _ => Err(Diagnostic::error(format!(
                "selected source top-level boundary requirement `{}` retains a trait calling-plan fingerprint",
                requirement.name,
            ))),
        };
    }

    let schema_owners = typed
        .traits()
        .iter()
        .filter(|definition| {
            crate::service_schema::schema_binds_exact_boundary_trait(
                provider_context,
                &plan.schema,
                definition,
            )
        })
        .collect::<Vec<_>>();
    let [schema_owner] = schema_owners.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry schema `{}` resolves to {} exact typed boundary traits",
            plan.schema.identity_text(plan.schema.trait_name),
            schema_owners.len()
        )));
    };

    let requirement_owners = typed
        .traits()
        .iter()
        .filter(|definition| {
            typed.trait_declaration_path(definition) == method.requirement_owner
                && crate::service_schema::is_product_declaration(typed, definition.symbol)
                && typed.symbols.symbol_package_identity(definition.symbol)
                    == method.requirement_owner_package_identity
        })
        .collect::<Vec<_>>();
    let [requirement_owner] = requirement_owners.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry requirement owner `{}` resolves to {} exact typed traits",
            method.requirement_owner,
            requirement_owners.len()
        )));
    };
    if !requirement_owner.is_boundary {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry requirement owner `{}` is not an exact boundary trait",
            method.requirement_owner
        )));
    }

    let matching_signatures = typed
        .trait_machine_signatures(requirement_owner)
        .iter()
        .filter(|signature| {
            signature.name.as_str() == method_name
                && typed.trait_requirement_callable_identity_text(requirement_owner, signature)
                    == requirement_identity
        })
        .collect::<Vec<_>>();
    let [signature] = matching_signatures.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry requirement `{}` binds {} exact typed signatures for `{method_name}` / `{requirement_identity}`",
            method.requirement_owner,
            matching_signatures.len()
        )));
    };

    let (Some(fingerprint), Some(commitment)) = (
        method.calling_plan_report_fingerprint,
        method.calling_plan_commitment,
    ) else {
        return if method.calling_plan_report_fingerprint.is_none()
            && method.calling_plan_commitment.is_none()
        {
            Ok(None)
        } else {
            Err(Diagnostic::error(format!(
                "selected source boundary entry `{trait_name}::{method_name}` does not retain its calling-plan report coordinate and commitment together"
            )))
        };
    };
    if fingerprint == 0 {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry `{trait_name}::{method_name}` has a zero calling-plan fingerprint"
        )));
    }

    let matching_realizations = boundary_calling_plan_realizations
        .iter()
        .filter(|realization| {
            realization.report_fingerprint == fingerprint
                && realization.commitment == commitment
                && realization.boundary_trait == schema_owner.symbol
                && realization.requirement_machine == signature.symbol
        })
        .collect::<Vec<_>>();
    let [realization] = matching_realizations.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry `{trait_name}::{method_name}` / `{requirement_identity}` resolves to {} exact calling-plan realizations for fingerprint 0x{fingerprint:016x}",
            matching_realizations.len()
        )));
    };
    let (validated, application_report_fingerprint, application_commitment) = realization
        .replayed_validated_application()
        .map_err(|error| {
        Diagnostic::error(format!(
            "selected source boundary entry `{trait_name}::{method_name}` / `{requirement_identity}` retained an invalid target calling-plan realization: {error}"
        ))
    })?;
    if validated.plan() != realization.exact_boundary_entry_plan() {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry `{trait_name}::{method_name}` / `{requirement_identity}` substituted a calling plan behind its compact report fingerprint"
        )));
    }
    if realization.report_fingerprint != application_report_fingerprint
        || realization.commitment != application_commitment
    {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry `{trait_name}::{method_name}` / `{requirement_identity}` substituted a calling-plan commitment behind its compact report fingerprint"
        )));
    }

    realization
        .native_application()
        .map(Some)
        .map_err(Diagnostic::error)
}

/// Materialize this producer's own component-boundary applications while its
/// typed signature and selected opaque representations are still available.
/// Fused internal calls need no compatibility ABI and never enter this route.
pub fn component_calling_applications(
    typed: &checked_trees::typed_trees::TypedTrees,
    native_target: target::NativeTarget,
    plans: &[&effects::provider_plan::ProviderPlan],
    explicit: &[super::BoundaryCallingPlanRealization],
    representations: &[representation_planning::OpaqueRepresentationSelection],
) -> Result<Vec<calling_conventions::BoundaryCallingApplication>, Vec<Diagnostic>> {
    let mut applications = Vec::<calling_conventions::BoundaryCallingApplication>::new();
    for plan in plans {
        if !plan.rows.iter().any(|row| {
            matches!(
                row.binding,
                effects::provider_plan::ProviderBinding::CheckedAdapter { .. }
            )
        }) {
            continue;
        }
        let profile = target::TargetProfile::from_canonical_target_name(&plan.target)
            .map_err(|error| vec![error])?;
        if profile.native_target() != native_target {
            return Err(vec![Diagnostic::error(
                "component ABI request names another selected native target",
            )]);
        }
        for row in &plan.rows {
            if !matches!(
                row.binding,
                effects::provider_plan::ProviderBinding::CheckedAdapter { .. }
            ) {
                continue;
            }
            let trait_name = plan.schema.identity_text(plan.schema.trait_name);
            let requirement = plan.identities.text(row.requirement_identity);
            let application = selected_source_calling_application(
                typed,
                explicit,
                plan,
                trait_name,
                &row.method,
                requirement,
            )
            .map_err(|error| vec![error])?;
            let application = match application {
                Some(application) => application,
                None => super::plan_computation::evaluate_default_calling_application(
                    typed, native_target, trait_name, plan.schema.trait_package_identity,
                    &row.method, requirement,
                    calling_conventions::CallingPolicy::native_for_target(native_target),
                    0, representations,
                ).map_err(|reason| vec![Diagnostic::error(format!("cannot materialize component calling application for `{trait_name}::{}`: {reason}", row.method))])?
                    .ok_or_else(|| vec![Diagnostic::error(format!("component boundary `{requirement}` has no exact materializable source signature"))])?,
            };
            if std::env::var_os("OMEGA_TRACE_COMPONENT_CALLING").is_some() {
                eprintln!(
                    "component ABI materialized: package={:?} plan={:?} application={application:#?}",
                    plan.origin_package_identity,
                    plan.identity_digest()
                );
            }
            if let Some(previous) = applications.iter().find(|previous| {
                previous.requirement_identity == application.requirement_identity
                    && previous.target == application.target
            }) {
                if previous != &application {
                    return Err(vec![Diagnostic::error(
                        "component selected plans disagree on one complete calling application",
                    )]);
                }
            } else {
                applications.push(application);
            }
        }
    }
    Ok(applications)
}
