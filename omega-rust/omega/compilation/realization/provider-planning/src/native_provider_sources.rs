//! Prepare native provider identities while binding the closed checked program.
//!
//! Selected plans and declaration associations leave together as owned data.
//! Native consumers borrow this result without rediscovering typed declarations;
//! the artifact still owns physical family membership and native admission.

use crate::{ProviderPlanningContext, SelectedProviderReviewProvenance};
use backend_plan::{NativeProviderSources, TerminalTargetProviderPlan};
use diagnostics::Diagnostic;

/// Close declaration owners before family relationships, preserving diagnostic
/// precedence. The caller retains failure until a native handoff demands it.
pub fn prepare_native_provider_sources(
    context: ProviderPlanningContext<'_>,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    provenance: &[SelectedProviderReviewProvenance],
) -> Result<NativeProviderSources, Vec<Diagnostic>> {
    let typed = context.typed;
    let mut declarations = Vec::with_capacity(provenance.len());
    for retained in provenance {
        let provider_declaration = retained
            .provider
            .provider_type
            .map(|symbol| {
                typed
                    .exact_owner_nominal_identity(symbol, exact_toolchain_sources)
                    .and_then(|identity| typed.type_identity_text(identity))
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "native provider selection lost its exact declaration owner",
                        )]
                    })
            })
            .transpose()?;
        declarations.push(TerminalTargetProviderPlan {
            plan: retained.plan.clone(),
            provider_declaration,
        });
    }
    let family_realizations = selected_family_realizations(context, provenance)?;
    Ok(NativeProviderSources {
        declarations,
        family_realizations,
    })
}

/// Record the exact source family that supplied each checked provider row.
/// Artifact-local member IDs are deliberately unavailable at this phase.
fn selected_family_realizations(
    context: ProviderPlanningContext<'_>,
    provenance: &[SelectedProviderReviewProvenance],
) -> Result<Vec<backend_plan::SelectedFamilyRealizationSource>, Vec<Diagnostic>> {
    let typed = context.typed;
    let mut rows = Vec::new();
    for retained in provenance {
        let provider = &retained.provider;
        let plan = &retained.plan;
        if provider.row_realizations.len() != plan.rows.len()
            || provider.row_requirements.len() != plan.rows.len()
            || provider.row_target_machine_origins.len() != plan.rows.len()
            || provider.row_placed_accessors.len() != plan.rows.len()
        {
            return Err(vec![Diagnostic::error(
                "selected family realization has incomplete provider row custody",
            )]);
        }
        for (row_index, row) in plan.rows.iter().enumerate() {
            let realization = provider.row_target_machine_origins[row_index]
                .as_ref()
                .map_or(provider.row_realizations[row_index], |origin| {
                    origin.machine
                });
            for family in &typed.target_machine_families {
                if !family.members.iter().any(|member| {
                    member.machine == realization && member.target.as_str() == plan.target
                }) {
                    continue;
                }
                if provider.row_target_machine_origins[row_index]
                    .as_ref()
                    .is_some_and(|origin| origin.target != plan.target)
                {
                    return Err(vec![Diagnostic::error(
                        "selected family realization has substituted target custody",
                    )]);
                }
                // Join the exact checked declaration retained by Psi. Export
                // spellings and normalized requirement callable identities
                // occupy different namespaces and cannot identify this head.
                let head_declaration_identity = typed
                    .normalized_hermetic_symbol_identity(family.canonical)
                    .map_err(|message| {
                        vec![Diagnostic::error(format!(
                            "selected family realization head: {message}"
                        ))]
                    })?;
                let head = typed.machine_by_symbol(family.canonical).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "selected family realization has no typed head",
                    )]
                })?;
                let head_callable_identity =
                    typed.machine_callable_identity_text(head).ok_or_else(|| {
                        vec![Diagnostic::error(
                            "selected family realization has no head callable identity",
                        )]
                    })?;
                rows.push(backend_plan::SelectedFamilyRealizationSource {
                    head_declaration_identity,
                    head_callable_identity,
                    target: plan.target.clone(),
                    selected_requirement_identity: plan
                        .identities
                        .text(row.requirement_identity)
                        .to_owned(),
                    provider_plan_report_identity: plan.report_fingerprint(),
                    provider_plan_digest: plan.identity_digest(),
                });
            }
        }
    }
    Ok(rows)
}
