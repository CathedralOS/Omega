//! Source-selected import and syscall demand to exact mechanism admission.

use std::collections::{BTreeMap, BTreeSet};

use crate::native_realization::providers::AdmittedTerminalMechanism;
use crate::native_realization::realization_request::{
    NativeBoundaryRealization, NativeProviderSettlement,
};
use abstract_operations_to_target_operations::AdmittedNativeCallbackArgument;
use diagnostics::Diagnostic;
use effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow};
use semantic_vocabulary::BoundaryMachineId;

use crate::native_realization::terminal_authority_policy::TerminalAuthorityPolicyRow;

struct ImportCoverageRows<'input> {
    boundary: BoundaryMachineId,
    boundary_count: usize,
    selected_count: usize,
    selected_external: Option<(&'input ProviderPlan, &'input ProviderPlanRow)>,
    external: Option<&'input calling_conventions::ExternalBindingRow>,
    external_count: usize,
    callback: Option<&'input AdmittedNativeCallbackArgument>,
    callback_count: usize,
}

pub(super) fn validate_source_evaluated_import_coverage(
    plan: &abstract_operations::AbstractOperationPlan,
    selected_plans: &effects::SelectedProviderPlanFacts,
    policy: &crate::native_realization::TerminalAuthorityPolicy,
    target: target::NativeTarget,
    external_binding_rows: &[calling_conventions::ExternalBindingRow],
    settlements: &[NativeProviderSettlement<'_>],
    native_callbacks: &[abstract_operations_to_target_operations::AdmittedNativeCallbackArgument],
    filesystem_release_contracts: &[crate::native_realization::FilesystemOrdinaryReleaseContract],
) -> Result<
    (
        Vec<AdmittedTerminalMechanism>,
        Vec<TerminalAuthorityPolicyRow>,
    ),
    Vec<Diagnostic>,
> {
    let demanded = join_import_coverage_rows(
        plan,
        selected_plans,
        external_binding_rows,
        native_callbacks,
    )?;
    let mut required_imports = BTreeSet::new();
    let mut admitted_mechanisms = Vec::new();
    // Toolchain-settled `FilesystemHost` cohort rows emitted for demanded
    // leaves: they classify beside the supplied receiving policy rather than
    // requiring the receiver to spell them.
    let mut cohort_rows: Vec<TerminalAuthorityPolicyRow> = Vec::new();
    for (requirement, rows) in demanded {
        let Some((provider_plan, row)) = rows.selected_external else {
            continue;
        };
        if rows.selected_count > 1 {
            return Err(vec![Diagnostic::error(format!(
                "demanded external binding `{requirement}` resolves to {} selected provider rows",
                rows.selected_count,
            ))]);
        }
        match &row.binding {
            ProviderBinding::Syscall { number } => {
                let number = *number;
                let target_profile =
                    target::TargetProfile::from_canonical_target_name(&provider_plan.target)
                        .map_err(|diagnostic| vec![diagnostic])?;
                if target_profile.native_target() != target {
                    return Err(vec![Diagnostic::error(format!(
                        "demanded syscall `{requirement}` targets `{}` rather than the receiving native target",
                        provider_plan.target,
                    ))]);
                }
                let Some(external) = rows.external.filter(|_| rows.external_count == 1) else {
                    return Err(vec![Diagnostic::error(format!(
                        "demanded syscall `{requirement}` resolves to {} retained external binding rows",
                        rows.external_count,
                    ))]);
                };
                let calling_conventions::ExternalBindingKind::Syscall {
                    number: external_number,
                } = external.binding
                else {
                    return Err(vec![Diagnostic::error(format!(
                        "demanded syscall `{requirement}` does not retain one exact syscall external binding"
                    ))]);
                };
                let external_target_profile =
                    target::TargetProfile::from_canonical_target_name(&external.target_name)
                        .map_err(|diagnostic| vec![diagnostic])?;
                if external_target_profile != target_profile {
                    return Err(vec![Diagnostic::error(format!(
                        "demanded syscall `{requirement}` substituted its retained external target profile"
                    ))]);
                }
                if external_number != number {
                    return Err(vec![Diagnostic::error(format!(
                        "demanded syscall `{requirement}` substituted its normalized syscall number"
                    ))]);
                }
                if rows.boundary_count != 1 {
                    return Err(vec![Diagnostic::error(format!(
                        "demanded syscall `{requirement}` resolves to {} Terminal boundaries",
                        rows.boundary_count,
                    ))]);
                }
                let mechanism = crate::native_realization::terminal_authority_policy::conservative_syscall_terminal_mechanism(
                    target_profile,
                    number,
                    plan,
                    rows.boundary,
                )
                .map_err(|error| {
                    vec![Diagnostic::error(format!(
                        "demanded syscall `{requirement}` has no exact checked argument contract: {error}"
                    ))]
                })?;
                let cohort_row = settled_filesystem_cohort_row(provider_plan, row, mechanism);
                let mechanism = match classify_terminal_mechanism(
                    policy,
                    mechanism,
                    ordinary_release_cohort(provider_plan, row),
                    filesystem_release_contracts,
                ) {
                    Ok(mechanism) => mechanism,
                    Err(unclassified) if cohort_row.is_some() => {
                        // The toolchain-settled cohort classifies this leaf:
                        // the receiving policy need not spell the row.
                        unclassified.mechanism()
                    }
                    Err(unclassified) => {
                        return Err(vec![Diagnostic::error(format!(
                            "receiving terminal-authority policy version {} does not classify syscall mechanism {:?} required by `{requirement}`",
                            policy.identity().version(),
                            unclassified.mechanism(),
                        ))]);
                    }
                };
                if let Some(row) =
                    cohort_row.filter(|row| !cohort_rows.contains(row))
                {
                    cohort_rows.push(row);
                }
                admitted_mechanisms.push(AdmittedTerminalMechanism {
                    boundary: rows.boundary,
                    mechanism,
                });
            }
            ProviderBinding::Import { evaluated } => {
                if evaluated.locator().target().native_target() != target {
                    return Err(vec![Diagnostic::error(format!(
                        "demanded normalized import `{requirement}` targets `{}` rather than the receiving native target",
                        evaluated.locator().target().target_name(),
                    ))]);
                }
                let Some(external) = rows.external.filter(|_| rows.external_count == 1) else {
                    return Err(vec![Diagnostic::error(format!(
                        "demanded normalized import `{requirement}` resolves to {} retained external implementation contracts",
                        rows.external_count,
                    ))]);
                };
                let (
                    calling_conventions::ExternalBindingKind::Import {
                        locator: external_locator,
                    },
                    Some(boundary_entry_plan),
                ) = (&external.binding, &external.boundary_entry_plan)
                else {
                    return Err(vec![Diagnostic::error(format!(
                        "demanded normalized import `{requirement}` does not retain one normalized locator and admitted boundary contract"
                    ))]);
                };
                if external_locator != evaluated.locator() {
                    return Err(vec![Diagnostic::error(format!(
                        "demanded normalized import `{requirement}` substituted its retained external locator"
                    ))]);
                }
                let mechanism = match (rows.callback_count, rows.callback) {
                    (0, _) => crate::native_realization::normalized_foreign_terminal_mechanism(
                        evaluated.locator(),
                        boundary_entry_plan,
                    ),
                    (1, Some(callback))
                        if callback.registrar_boundary_entry_plan == *boundary_entry_plan =>
                    {
                        crate::native_realization::normalized_foreign_terminal_mechanism_with_callback_materializations(
                            evaluated.locator(),
                            boundary_entry_plan,
                            &callback.registrar_context,
                        )
                    }
                    (count, _) => Err(format!(
                        "retained implementation contract rejoins {} exact native callbacks with no unique matching registrar plan",
                        count,
                    )),
                }
                .map_err(|error| {
                    vec![Diagnostic::error(format!(
                        "demanded normalized import `{requirement}` has an invalid admitted implementation contract: {error}"
                    ))]
                })?;
                let cohort_row = settled_filesystem_cohort_row(provider_plan, row, mechanism);
                let mechanism = match classify_terminal_mechanism(
                    policy,
                    mechanism,
                    ordinary_release_cohort(provider_plan, row),
                    filesystem_release_contracts,
                ) {
                    Ok(mechanism) => mechanism,
                    Err(unclassified) if cohort_row.is_some() => unclassified.mechanism(),
                    Err(unclassified) => {
                        return Err(vec![Diagnostic::error(format!(
                            "receiving terminal-authority policy version {} does not classify normalized foreign mechanism {:?} required by `{requirement}`",
                            policy.identity().version(),
                            unclassified.mechanism(),
                        ))]);
                    }
                };
                if let Some(row) =
                    cohort_row.filter(|row| !cohort_rows.contains(row))
                {
                    cohort_rows.push(row);
                }
                if rows.boundary_count != 1 {
                    return Err(vec![Diagnostic::error(format!(
                        "demanded normalized import `{requirement}` resolves to {} Terminal boundaries",
                        rows.boundary_count
                    ))]);
                }
                admitted_mechanisms.push(AdmittedTerminalMechanism {
                    boundary: rows.boundary,
                    mechanism,
                });
                required_imports.insert(requirement);
            }
            _ => {}
        }
    }

    let mut covered_imports = BTreeSet::new();
    for settlement in settlements {
        let requirement = settlement.provider_execution.requirement_identity();
        let is_normalized = matches!(
            settlement.realization,
            NativeBoundaryRealization::NormalizedForeignCall(_)
        );
        if required_imports.contains(requirement) {
            if !is_normalized {
                return Err(vec![Diagnostic::error(format!(
                    "source-evaluated import `{requirement}` requires a normalized foreign-call settlement"
                ))]);
            }
            if !covered_imports.insert(requirement) {
                return Err(vec![Diagnostic::error(format!(
                    "source-evaluated import `{requirement}` received more than one normalized settlement"
                ))]);
            }
        } else if is_normalized {
            return Err(vec![Diagnostic::error(format!(
                "normalized foreign-call settlement for `{requirement}` does not cover a demanded selected import"
            ))]);
        }
    }
    if let Some(missing) = required_imports
        .iter()
        .find(|requirement| !covered_imports.contains(**requirement))
    {
        return Err(vec![Diagnostic::error(format!(
            "demanded source-evaluated import `{missing}` has no admitted native settlement"
        ))]);
    }
    admitted_mechanisms.sort_by_key(|row| row.boundary);
    Ok((admitted_mechanisms, cohort_rows))
}

/// The toolchain-settled `FilesystemHost` facet cohort supplies one exact
/// mechanism row for a demanded leaf, so classification does not require the
/// receiving policy to spell it. Name-keyed like [`ordinary_release_cohort`]:
/// the schema method selects the cohort, and the row binds the exact mechanism
/// this leaf computed. Ordinary-release cohorts mint nothing here — an
/// unconstrained generic key cannot inherit the occurrence-specific release
/// proof — and unrecognized names mint nothing, so both still demand a
/// receiving row or release-contract bound key.
fn settled_filesystem_cohort_row(
    provider_plan: &ProviderPlan,
    row: &ProviderPlanRow,
    mechanism: effects::TerminalMechanismIdentity,
) -> Option<TerminalAuthorityPolicyRow> {
    let method = provider_plan
        .schema
        .methods
        .iter()
        .find(|method| method.name == row.method)?;
    crate::native_realization::terminal_authority_policy::filesystem_mechanism_row(
        mechanism, method,
    )
    .ok()
}

/// Whether the selected row serves a canonical `FilesystemHost`
/// ordinary-release cohort method. Only those requirements may carry an
/// occurrence-specific release contract into a mechanism key: the coordinate
/// is evidence of a proved constrained open/query/close occurrence, and no
/// other cohort can bind it.
fn ordinary_release_cohort(provider_plan: &ProviderPlan, row: &ProviderPlanRow) -> bool {
    provider_plan
        .schema
        .methods
        .iter()
        .find(|method| method.name == row.method)
        .and_then(|method| {
            crate::native_realization::terminal_authority_policy::settled_filesystem_cohort(
                &method.name,
            )
        })
        == Some(crate::native_realization::FilesystemCohortDisposition::OrdinaryReleaseContract)
}

/// Classify one demanded mechanism, preferring an occurrence-bound key when a
/// retained ordinary-release contract narrows this mechanism's checked
/// argument-contract coordinate and the receiving policy classified that
/// bound key.
///
/// The proved constrained occurrence uses its own evidence-bound mechanism
/// identity, so each retained contract's bound key is consulted before the
/// unconstrained conservative key: a caller row for the generic key never
/// widens a covered occurrence back. A contract whose bound key has no
/// explicit row cannot classify, and a record absent from this compile's
/// custody contributes no candidates, so the unconstrained fallback fails
/// closed exactly as before. Non-release cohorts never consult bound keys:
/// no retained release occurrence can narrow them.
fn classify_terminal_mechanism(
    policy: &crate::native_realization::TerminalAuthorityPolicy,
    mechanism: effects::TerminalMechanismIdentity,
    release_cohort: bool,
    filesystem_release_contracts: &[crate::native_realization::FilesystemOrdinaryReleaseContract],
) -> Result<
    effects::TerminalMechanismIdentity,
    crate::native_realization::terminal_authority_policy::UnclassifiedTerminalMechanism,
> {
    if release_cohort {
        for bound in filesystem_release_contracts.iter().filter_map(|contract| {
            crate::native_realization::terminal_authority_policy::filesystem_release_bound_mechanism(
                mechanism, *contract,
            )
        }) {
            if policy.classify(bound).is_ok() {
                return Ok(bound);
            }
        }
    }
    policy.classify(mechanism).map(|_| mechanism)
}

/// Join only demanded requirements; counts retain malformed duplicate evidence
/// without collecting temporary match vectors or changing diagnostic order.
fn join_import_coverage_rows<'input>(
    plan: &'input abstract_operations::AbstractOperationPlan,
    selected_plans: &'input effects::SelectedProviderPlanFacts,
    external_binding_rows: &'input [calling_conventions::ExternalBindingRow],
    native_callbacks: &'input [AdmittedNativeCallbackArgument],
) -> Result<BTreeMap<&'input str, ImportCoverageRows<'input>>, Vec<Diagnostic>> {
    let boundary_identities = plan
        .boundary_machines
        .iter()
        .map(|boundary| (boundary.id, boundary.identity.as_str()))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut demanded = BTreeMap::new();
    // Only callbacks requested by this invocation need an occurrence join.
    let mut callback_boundaries = native_callbacks
        .iter()
        .map(|callback| (callback.terminal_operation, None))
        .collect::<BTreeMap<_, _>>();
    for operation in plan
        .functions
        .iter()
        .flat_map(|function| &function.operations)
    {
        let abstract_operations::AbstractOperation::BoundaryCall {
            boundary,
            psi_operation,
            ..
        } = operation
        else {
            continue;
        };
        let Some(requirement) = boundary_identities.get(boundary) else {
            return Err(vec![Diagnostic::error(format!(
                "external-binding demand cites absent boundary {boundary:?}"
            ))]);
        };
        demanded
            .entry(*requirement)
            .or_insert_with(|| ImportCoverageRows {
                boundary: *boundary,
                boundary_count: 0,
                selected_count: 0,
                selected_external: None,
                external: None,
                external_count: 0,
                callback: None,
                callback_count: 0,
            });
        if let Some(matching) = callback_boundaries.get_mut(psi_operation) {
            let (count, _) = matching.get_or_insert((0usize, *boundary));
            *count += 1;
        }
    }

    if demanded.is_empty() && native_callbacks.is_empty() {
        return Ok(demanded);
    }

    let mut has_external_demand = false;
    for provider_plan in selected_plans.plans() {
        for row in &provider_plan.rows {
            if let Some(rows) = demanded.get_mut(row.requirement_identity.as_str()) {
                rows.selected_count += 1;
                if matches!(
                    row.binding,
                    ProviderBinding::Import { .. } | ProviderBinding::Syscall { .. }
                ) {
                    rows.selected_external = Some((provider_plan, row));
                    has_external_demand = true;
                }
            }
        }
    }
    if has_external_demand {
        // Preserve the original last-declaration-wins boundary-ID association and
        // count every distinct ID, including declarations without a call.
        for (boundary, requirement) in &boundary_identities {
            if let Some(rows) = demanded.get_mut(requirement) {
                rows.boundary = *boundary;
                rows.boundary_count += 1;
            }
        }
        for external in external_binding_rows {
            if let Some(rows) = demanded.get_mut(external.requirement_identity.as_str()) {
                rows.external = Some(external);
                rows.external_count += 1;
            }
        }
    }

    for callback in native_callbacks {
        let matching = callback_boundaries
            .get(&callback.terminal_operation)
            .copied()
            .flatten();
        let count = matching.map_or(0, |(count, _)| count);
        let Some((1, boundary)) = matching else {
            return Err(vec![Diagnostic::error(format!(
                "native callback operation {} resolves to {} abstract boundary calls during source-import coverage",
                callback.terminal_operation.get(),
                count,
            ))]);
        };
        let requirement = boundary_identities.get(&boundary).copied().ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "native callback operation {} cites absent boundary {:?}",
                callback.terminal_operation.get(),
                boundary,
            ))]
        })?;
        if let Some(rows) = demanded.get_mut(requirement) {
            rows.callback = Some(callback);
            rows.callback_count += 1;
        }
    }

    Ok(demanded)
}
