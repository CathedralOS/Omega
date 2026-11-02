//! Keep each target-specific implementation tied to its declared function.
//! Save source identities before Terminal publication, then attach the final
//! plan identities after calling conventions determine argument placement.

use crate::{ProviderPlanningContext, SelectedProviderReviewProvenance};
use backend_plan::{NativeProviderSources, TerminalTargetProviderPlan};
use diagnostics::Diagnostic;

/// Retain the selecting owner while its exact source custody is available.
pub(crate) fn close_selecting_declaration(
    context: ProviderPlanningContext<'_>,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    provider: &crate::ProviderPlanProvenance,
) -> Result<Option<String>, Vec<Diagnostic>> {
    provider
        .provider_type
        .map(|symbol| close_nominal_declaration(context, exact_toolchain_sources, symbol))
        .transpose()
}

fn close_nominal_declaration(
    context: ProviderPlanningContext<'_>,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    symbol: symbols::SymbolHandle,
) -> Result<String, Vec<Diagnostic>> {
    context
        .typed
        .exact_owner_nominal_identity(symbol, exact_toolchain_sources)
        .and_then(|identity| context.typed.type_identity_text(identity))
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "native provider selection lost its exact declaration owner",
            )]
        })
}

/// Saved source identities for one candidate's target-family members.
/// Final plan digests are attached only after calling plans are complete.
#[derive(Debug, Clone)]
pub(crate) struct PreparedFamilyRealization {
    row_index: usize,
    head_declaration_identity: String,
    head_callable_identity: String,
}

pub(crate) fn attach_native_provider_sources(
    provenance: &[SelectedProviderReviewProvenance],
    families: Vec<Result<Vec<PreparedFamilyRealization>, Vec<Diagnostic>>>,
    declarations: std::sync::Arc<[TerminalTargetProviderPlan]>,
) -> Result<NativeProviderSources, Vec<Diagnostic>> {
    if provenance.len() != families.len() {
        return Err(vec![Diagnostic::error(
            "selected family realization lost candidate correspondence",
        )]);
    }
    let mut family_realizations = Vec::new();
    for (retained, families) in provenance.iter().zip(families) {
        let plan = &retained.plan;
        for family in families? {
            let row = plan.rows.get(family.row_index).ok_or_else(|| {
                vec![Diagnostic::error(
                    "selected family realization lost its prepared row",
                )]
            })?;
            family_realizations.push(backend_plan::SelectedFamilyRealizationSource {
                head_declaration_identity: family.head_declaration_identity,
                head_callable_identity: family.head_callable_identity,
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
    Ok(NativeProviderSources {
        declarations,
        family_realizations,
    })
}

/// Close exact declaration columns together with their whole plan.
/// Preparation retains these identities without granting selection authority;
/// public target proposals and immediate native production share this owner.
pub(crate) fn close_provider_declarations(
    context: ProviderPlanningContext<'_>,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    plan: &effects::provider_plan::ProviderPlan,
    provider: &crate::ProviderPlanProvenance,
) -> Result<TerminalTargetProviderPlan, Vec<Diagnostic>> {
    let typed = context.typed;
    let nominal = |symbol| close_nominal_declaration(context, exact_toolchain_sources, symbol);
    if provider.row_realizations.len() != plan.rows.len()
        || provider.row_requirements.len() != plan.rows.len()
    {
        return Err(vec![Diagnostic::error(
            "native provider selection lost row realization custody",
        )]);
    }
    let provider_declaration =
        close_selecting_declaration(context, exact_toolchain_sources, provider)?;
    let row_numeric_realizations = plan
        .rows
        .iter()
        .zip(&provider.row_realizations)
        .zip(&provider.row_requirements)
        .map(|((row, realization), requirement)| {
            if matches!(
                row.binding,
                effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. }
            ) && typed.numeric_intrinsic_execution(*requirement).is_some()
            {
                realization.authored().map(nominal).transpose()
            } else {
                Ok(None)
            }
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    Ok(TerminalTargetProviderPlan {
        plan: plan.clone(),
        provider_declaration,
        row_numeric_realizations,
    })
}

/// Record the exact source family that supplied each checked provider row.
/// Artifact-local member IDs are deliberately unavailable at this phase.
pub(crate) fn capture_family_realizations(
    context: ProviderPlanningContext<'_>,
    plan: &effects::provider_plan::ProviderPlan,
    provider: &crate::ProviderPlanProvenance,
) -> Result<Vec<PreparedFamilyRealization>, Vec<Diagnostic>> {
    let typed = context.typed;
    let mut rows = Vec::new();
    if provider.row_realizations.len() != plan.rows.len()
        || provider.row_requirements.len() != plan.rows.len()
        || provider.row_target_machine_origins.len() != plan.rows.len()
        || provider.row_accessors.len() != plan.rows.len()
    {
        return Err(vec![Diagnostic::error(
            "selected family realization has incomplete provider row custody",
        )]);
    }
    for row_index in 0..plan.rows.len() {
        let Some(realization) = provider.row_target_machine_origins[row_index]
            .as_ref()
            .map(|origin| origin.machine)
            .or_else(|| provider.row_realizations[row_index].authored())
        else {
            continue;
        };
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
            rows.push(PreparedFamilyRealization {
                row_index,
                head_declaration_identity,
                head_callable_identity,
            });
        }
    }
    Ok(rows)
}
