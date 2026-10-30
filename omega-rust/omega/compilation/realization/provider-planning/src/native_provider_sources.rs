//! Prepare native provider identities while binding the closed checked program.
//!
//! Selected plans and declaration associations leave together as owned data.
//! Native consumers borrow this result without rediscovering typed declarations;
//! the artifact still owns native admission.

use crate::{ProviderPlanningContext, SelectedProviderReviewProvenance};
use backend_plan::{NativeProviderSources, TerminalTargetProviderPlan};
use diagnostics::Diagnostic;

/// Close exact selected declaration owners, preserving diagnostic
/// precedence. The caller retains failure until a native handoff demands it.
pub fn prepare_native_provider_sources(
    context: ProviderPlanningContext<'_>,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    provenance: &[SelectedProviderReviewProvenance],
) -> Result<NativeProviderSources, Vec<Diagnostic>> {
    let mut declarations = Vec::with_capacity(provenance.len());
    for retained in provenance {
        declarations.push(close_provider_declarations(
            context,
            exact_toolchain_sources,
            &retained.plan,
            &retained.provider,
        )?);
    }
    Ok(NativeProviderSources { declarations })
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
    let nominal = |symbol| {
        typed
            .exact_owner_nominal_identity(symbol, exact_toolchain_sources)
            .and_then(|identity| typed.type_identity_text(identity))
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "native provider selection lost its exact declaration owner",
                )]
            })
    };
    if provider.row_realizations.len() != plan.rows.len()
        || provider.row_requirements.len() != plan.rows.len()
    {
        return Err(vec![Diagnostic::error(
            "native provider selection lost row realization custody",
        )]);
    }
    let provider_declaration = provider.provider_type.map(nominal).transpose()?;
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
