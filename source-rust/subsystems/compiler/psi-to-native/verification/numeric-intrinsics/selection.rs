//! Match demanded published boundaries to the selected plan rows and the exact declaring provider.
use diagnostics::Diagnostic;
use native_intrinsics::CompilerIntrinsicExecutionIdentity;
use provider_correspondence::{
    CompilerIntrinsicPlanProvenance, external_providers::matches_external_selection,
};
use provider_plans::{ProviderPlan, ServiceSchema};
use std::collections::BTreeSet;
pub struct SelectedIntrinsicRow<'a> {
    pub requirement_identity: &'a str,
    pub plan_index: usize,
    pub row: &'a provider_plans::ProviderPlanRow,
    pub semantic_schema: &'a ServiceSchema,
    pub execution: Option<CompilerIntrinsicExecutionIdentity>,
    pub external_realization: Option<&'a str>,
}

pub fn visit_selected_intrinsics<'a>(
    selected: &'a selected_provider_plans::SelectedProviderPlanFacts,
    provenance: impl ExactSizeIterator<Item = CompilerIntrinsicPlanProvenance<'a>>,
    demanded_boundaries: &BTreeSet<String>,
    module: &terminal_psi::TerminalModule,
    mut visit: impl FnMut(
        &str,
        &SelectedIntrinsicRow<'a>,
        &terminal_psi::BoundaryMachineDeclaration,
        &terminal_psi::CheckedBoundaryCallable,
    ) -> Result<(), Diagnostic>,
) -> Result<(), Vec<Diagnostic>> {
    let boundaries = &module.boundary_machines;
    let identities = &module.identities;
    let plans = selected.plans();
    if plans.len() != provenance.len() {
        return Err(vec![Diagnostic::error(
            "selected provider plans are not aligned with compiler-intrinsic settlement provenance",
        )]);
    }
    let mut selected_rows = Vec::new();
    for (plan_index, (plan, retained)) in plans.iter().zip(provenance).enumerate() {
        if retained.plan != plan
            || retained.row_executions.len() != plan.rows.len()
            || plan.row_numeric_realizations.len() != plan.rows.len()
        {
            return Err(vec![Diagnostic::error(format!(
                "selected compiler-intrinsic plan `{}` has misaligned retained settlement provenance",
                plan.name,
            ))]);
        }
        if demanded_boundaries.is_empty() {
            continue;
        }
        for ((row, execution), realization) in plan
            .rows
            .iter()
            .zip(retained.row_executions)
            .zip(&plan.row_numeric_realizations)
        {
            if matches!(
                row.binding,
                provider_plans::ProviderBinding::CompilerIntrinsic { .. }
            ) {
                selected_rows.push(SelectedIntrinsicRow {
                    requirement_identity: plan.identities.text(row.requirement_identity),
                    plan_index,
                    row,
                    semantic_schema: retained.semantic_schema,
                    execution: *execution,
                    external_realization: realization.as_deref(),
                });
            }
        }
    }
    selected_rows
        .sort_unstable_by(|left, right| left.requirement_identity.cmp(right.requirement_identity));

    let mut diagnostics = Vec::new();
    for requirement in demanded_boundaries {
        let actual_requirement = selected.selected_requirement_identity(requirement);
        let first =
            selected_rows.partition_point(|row| row.requirement_identity < actual_requirement);
        let matches = selected_rows[first..]
            .iter()
            .take_while(|row| row.requirement_identity == actual_requirement)
            .filter(|row| {
                selected.selected_row_matches_requirement(
                    requirement,
                    row.requirement_identity,
                    plans[row.plan_index].report_fingerprint(),
                )
            })
            .collect::<Vec<_>>();
        let [selected_row] = matches.as_slice() else {
            if !matches.is_empty() {
                diagnostics.push(Diagnostic::error(format!(
                    "Terminal boundary `{requirement}` resolves to {} selected compiler-intrinsic rows", matches.len(),
                )));
            }
            continue;
        };
        let mut matching_boundaries = boundaries
            .iter()
            .filter(|boundary| identities.text(boundary.identity) == Some(requirement.as_str()));
        let (Some(boundary), None) = (matching_boundaries.next(), matching_boundaries.next())
        else {
            diagnostics.push(Diagnostic::error(format!(
                "compiler-builtin proposal for `{requirement}` does not rejoin one Terminal boundary"
            )));
            continue;
        };
        let Some(callable) = boundary.declaration.as_ref() else {
            diagnostics.push(Diagnostic::error(format!(
                "compiler-builtin `{requirement}` has no checked callable declaration"
            )));
            continue;
        };
        if let Err(error) = visit(requirement, selected_row, boundary, callable) {
            diagnostics.push(error);
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}
// A selected external realization needs its exact declaring owner only where
// the module retains the requirement as a boundary or boundary application:
// native realization and intrinsic admission both execute selected rows by that
// demand, so a selected row nothing retains is never realized. Every consumer
// shares this one demand contract rather than asking for declarations of rows
// the program never executes.
pub fn validate_external_provider_owners(
    module: &terminal_psi::TerminalModule,
    selected: &selected_provider_plans::SelectedProviderPlanFacts,
) -> Result<(), String> {
    let mut retained = module
        .boundary_machines
        .iter()
        .map(|boundary| {
            module
                .identities
                .text(boundary.identity)
                .filter(|identity| !identity.is_empty())
                .ok_or_else(|| {
                    format!(
                        "retained Terminal boundary {:?} has no callable identity",
                        boundary.id,
                    )
                })
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    for demand in &module.boundary_application_demands {
        let identity = demand.requirement().overload();
        if identity.is_empty() {
            return Err("retained boundary application has no callable identity".into());
        }
        retained.insert(identity);
    }
    validate_retained_provider_owners(
        &module.external_providers,
        &module.identities,
        selected,
        &retained,
    )
}

pub(crate) fn validate_retained_provider_owners(
    external_providers: &[terminal_psi::ExternalProviderConformance],
    identities: &terminal_psi::TerminalIdentityTable,
    facts: &selected_provider_plans::SelectedProviderPlanFacts,
    retained: &BTreeSet<&str>,
) -> Result<(), String> {
    let requires_declaration = |plan: &ProviderPlan, row: &provider_plans::ProviderPlanRow| {
        retained.iter().any(|requirement| {
            facts.selected_row_matches_requirement(
                requirement,
                plan.identities.text(row.requirement_identity),
                plan.report_fingerprint(),
            )
        })
    };
    for selected in facts.plans() {
        if selected.row_numeric_realizations.len() != selected.rows.len() {
            return Err("selected external declarations have incomplete row custody".into());
        }
        for (row, realization) in selected.rows.iter().zip(&selected.row_numeric_realizations) {
            let provider_plans::ProviderBinding::CompilerIntrinsic { machine } = &row.binding
            else {
                if realization.is_some() {
                    return Err("non-intrinsic row substituted an external realization".into());
                }
                continue;
            };
            if realization.as_ref().is_some_and(String::is_empty) {
                return Err("selected external realization has empty declaration identity".into());
            }
            let mut found = false;
            for candidate in external_providers.iter().filter(|candidate| {
                matches_external_selection(candidate, identities, selected, row, machine)
                    && realization.as_ref().is_none_or(|realization| {
                        identities.text(candidate.realization.machine.identity())
                            == Some(realization.as_str())
                    })
            }) {
                found = true;
                let owner = candidate
                    .provider
                    .as_ref()
                    .and_then(|provider| identities.text(provider.declaration.identity()));
                if owner != selected.provider_declaration.as_deref() {
                    return Err(
                        "selected external provider has a different exact declaring owner".into(),
                    );
                }
            }
            if realization.is_some() && !found && requires_declaration(selected, row) {
                return Err(format!(
                    "selected provider `{}` has no retained external implementation for `{}` (implementation `{}`)",
                    selected.provider_type,
                    selected.identities.text(row.requirement_identity),
                    realization.as_deref().unwrap_or_default(),
                ));
            }
        }
    }
    Ok(())
}
