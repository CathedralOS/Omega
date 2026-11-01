//! Complete calling applications selected against exact boundary requirements.
//!
//! External calls and independently published checked entries use one explicit
//! policy-selection replay. Their execution/installation authority remains with
//! their different provider owners. Default native plans are materialized only
//! when an actual external or component boundary requests an ABI application.

use crate::provider_planning::trait_name_text;
use checked_trees::typed_trees::service_declarations::ServiceCatalog;
use diagnostics::Diagnostic;

/// Resolve implementation evidence only through the provider candidate that
/// selection admitted. The public provider/schema identity carries the
/// canonical fingerprint; source preparation retained the exact declaration
/// and requirement correspondence, so selection never rediscovers source.
pub(crate) fn selected_source_calling_applications(
    services: ServiceCatalog<'_>,
    boundary_calling_plan_realizations: &[super::BoundaryCallingPlanRealization],
    plan: &effects::provider_plan::ProviderPlan,
    provenance: &crate::ProviderPlanProvenance,
    row_index: usize,
) -> Result<Vec<calling_conventions::BoundaryCallingApplication>, Diagnostic> {
    if plan.name.is_empty() {
        return Err(Diagnostic::error(
            "selected source boundary entry plan has an empty ProviderPlan name",
        ));
    }
    let provider_plan_name = plan.name.as_str();
    let trait_name = trait_name_text(&plan.schema);
    let row = plan
        .rows
        .get(row_index)
        .ok_or_else(|| Diagnostic::error("selected source boundary entry lost its provider row"))?;
    let method_name = row.method.as_str();
    let requirement_identity = plan.identities.text(row.requirement_identity);
    if requirement_identity.is_empty() {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry `{trait_name}::{method_name}` has an empty exact requirement overload identity"
        )));
    }
    if services.context().scope != source::DependencyScope::Product {
        return Err(Diagnostic::error(
            "selected source boundary entry requires the exact Product declaration catalog",
        ));
    }

    // The deriving owner retained this association. A captured named map and
    // an abstract declaration are different origins, not fallback searches.
    let prepared = match &provenance.named_application {
        Some(application) => services.named_application(
            application.declaration,
            application.capture,
            provenance.schema.symbol(),
        ),
        None => services.declaration(provenance.schema),
    }
    .ok_or_else(|| {
        Diagnostic::error("selected source boundary entry lost its exact prepared declaration")
    })?;
    if prepared.declaration != &plan.schema.declaration {
        return Err(Diagnostic::error(
            "selected source boundary entry changed its exact prepared service surface",
        ));
    }
    let source_symbol = provenance.row_requirements.get(row_index);
    let source_symbol = source_symbol.ok_or_else(|| {
        Diagnostic::error("selected source boundary entry lost its provider requirement")
    })?;
    let mut matching_sources = prepared
        .requirements
        .iter()
        .enumerate()
        .filter(|(_, requirement)| &requirement.source.symbol == source_symbol);
    let Some((method_index, source)) = matching_sources.next() else {
        return Err(Diagnostic::error(
            "selected source boundary entry lost its ordered requirement correspondence",
        ));
    };
    if matching_sources.next().is_some() {
        return Err(Diagnostic::error(
            "selected source boundary entry has ambiguous requirement correspondence",
        ));
    }
    let method = plan
        .schema
        .declaration
        .methods
        .get(method_index)
        .ok_or_else(|| {
            Diagnostic::error("selected source boundary entry lost its corresponding public method")
        })?;
    if method.name != method_name
        || plan.schema.identity_text(method.requirement_identity) != requirement_identity
    {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry ProviderPlan `{provider_plan_name}` does not bind exact schema method `{trait_name}::{method_name}` / `{requirement_identity}`"
        )));
    }
    if method.requirement_owner.is_empty() {
        return Err(Diagnostic::error(format!(
            "selected source boundary entry ProviderPlan `{provider_plan_name}` method `{method_name}` has an empty exact requirement owner"
        )));
    }
    if !matches!(
        prepared.schema,
        crate::ProviderSchemaDeclaration::BoundaryTrait(_)
    ) {
        return if plan
            .schema
            .calling_for_requirement(method.requirement_identity)
            .is_empty()
        {
            Ok(Vec::new())
        } else {
            Err(Diagnostic::error(
                "selected source non-trait boundary retains a trait calling-plan fingerprint",
            ))
        };
    }
    source.require_calling_owner()?;

    let mut applications = Vec::new();
    for calling in plan
        .schema
        .calling_for_requirement(method.requirement_identity)
    {
        let abi = &calling.abi;
        let fingerprint = calling.plan.report_fingerprint;
        let commitment = calling.plan.commitment;
        if fingerprint == 0 {
            return Err(Diagnostic::error(format!(
                "selected source boundary entry `{trait_name}::{method_name}` has a zero calling-plan fingerprint"
            )));
        }

        let matching_realizations = boundary_calling_plan_realizations
            .iter()
            .filter(|realization| {
                realization.materialized_signature().abi() == abi
                    && realization.boundary_trait == prepared.schema.symbol()
                    && realization.requirement_machine == source.source.symbol
                    && realization.boundary_arguments == prepared.boundary_arguments
                    && realization.argument_owner() == prepared.argument_owner
            })
            .collect::<Vec<_>>();
        let Some(realization) = matching_realizations.first().copied() else {
            return Err(Diagnostic::error(format!(
                "selected source boundary entry `{trait_name}::{method_name}` / `{requirement_identity}` resolves to {} exact calling-plan realizations for fingerprint 0x{fingerprint:016x}",
                matching_realizations.len()
            )));
        };
        if matching_realizations
            .iter()
            .any(|candidate| *candidate != realization)
            || realization.report_fingerprint != fingerprint
            || realization.commitment != commitment
        {
            return Err(Diagnostic::error(
                "selected source ABI application has competing or stale policy custody",
            ));
        }
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

        applications.push(
            realization
                .native_application()
                .map_err(Diagnostic::error)?,
        );
    }
    Ok(applications)
}

/// Materialize this producer's own component-boundary applications while its
/// typed signature and selected opaque representations are still available.
/// Fused internal calls need no compatibility ABI and never enter this route.
pub fn component_calling_applications(
    services: ServiceCatalog<'_>,
    target: target::TargetProfile,
    calling_artifact: Option<&terminal_codec::CanonicalTerminalArtifact>,
    providers: &[&crate::SelectedProviderReviewProvenance],
    explicit: &[super::BoundaryCallingPlanRealization],
) -> Result<Vec<calling_conventions::BoundaryCallingApplication>, Vec<Diagnostic>> {
    let native_target = target.native_realization().ok_or_else(|| {
        vec![Diagnostic::error(
            "component calling application target has no native realization",
        )]
    })?;
    let mut applications = Vec::<calling_conventions::BoundaryCallingApplication>::new();
    for provider in providers {
        let plan = &provider.plan;
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
        let layout_target = layout::LayoutTarget::for_profile(profile)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        if layout_target.native_target() != native_target {
            return Err(vec![Diagnostic::error(
                "component ABI request names another selected native target",
            )]);
        }
        for (row_index, row) in plan.rows.iter().enumerate() {
            if !matches!(
                row.binding,
                effects::provider_plan::ProviderBinding::CheckedAdapter { .. }
            ) {
                continue;
            }
            let trait_name = trait_name_text(&plan.schema);
            let requirement = plan.identities.text(row.requirement_identity);
            let mut selected_applications = selected_source_calling_applications(
                services,
                explicit,
                plan,
                &provider.provider,
                row_index,
            )
            .map_err(|error| vec![error])?;
            if selected_applications.is_empty() {
                let artifact = calling_artifact.ok_or_else(|| {
                    vec![Diagnostic::error(
                        "component calling applications have no published Product catalog",
                    )]
                })?;
                selected_applications = super::evaluate_default_calling_applications(
                    artifact, target, requirement,
                    calling_conventions::CallingPolicy::native_for_target(native_target), 0,
                ).map_err(|reason| vec![Diagnostic::error(format!(
                    "cannot materialize component calling applications for `{trait_name}::{}`: {reason}", row.method
                ))])?;
            }
            for application in selected_applications {
                if std::env::var_os("OMEGA_TRACE_COMPONENT_CALLING").is_some() {
                    eprintln!(
                        "component ABI materialized: package={:?} plan={:?} application={application:#?}",
                        plan.origin_package_identity,
                        plan.identity_digest()
                    );
                }
                if !applications.contains(&application) {
                    applications.push(application);
                }
            }
        }
    }
    Ok(applications)
}
