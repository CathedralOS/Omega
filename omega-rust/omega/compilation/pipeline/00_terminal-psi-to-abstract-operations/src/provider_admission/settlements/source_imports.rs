//! Source-selected import and syscall demand to exact mechanism custody.
//! Classification and optional receiver admission belong to closure review.

use std::collections::BTreeMap;

use crate::NativeCallbackArgumentProposal;
use crate::provider_admission::AdmittedTerminalMechanism;
use diagnostics::Diagnostic;
use effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow};
use semantic_vocabulary::BoundaryMachineId;

use crate::provider_admission::terminal_authority_policy::TerminalAuthorityPolicyRow;

struct ImportCoverageRows<'input> {
    boundary: BoundaryMachineId,
    boundary_count: usize,
    selected_count: usize,
    selected_external: Option<(&'input ProviderPlan, &'input ProviderPlanRow)>,
    external: Option<&'input calling_conventions::ExternalBindingRow>,
    external_count: usize,
    callbacks: Vec<&'input NativeCallbackArgumentProposal>,
}

/// Derive the terminal mechanism of every demanded source-evaluated import and
/// syscall, plus the toolchain-settled cohort rows those leaves mint.
/// `prepare_source_evaluated_bindings` already settled one execution per
/// demanded import and syscall from these selected plans, and provider
/// settlement extracted the external rows from the same plans, so this
/// derivation does not rejoin settlement coverage or re-compare targets,
/// locators and syscall numbers (pipeline.md, "Verification placement").
/// Each leaf must still resolve to one selected row, one Terminal boundary
/// and, for an import, one external implementation contract to derive from.
pub(super) fn derive_source_evaluated_mechanisms(
    plan: &abstract_operations::AbstractOperationPlan,
    selected_plans: &effects::SelectedProviderPlanFacts,
    external_binding_rows: &[calling_conventions::ExternalBindingRow],
    native_callbacks: &[crate::NativeCallbackArgumentProposal],
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
    let mut admitted_mechanisms = Vec::new();
    // Toolchain-settled host cohort rows emitted for demanded leaves: they
    // classify beside the supplied receiving policy rather than requiring
    // the receiver to spell them.
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
        if rows.boundary_count != 1 {
            return Err(vec![Diagnostic::error(format!(
                "demanded external binding `{requirement}` resolves to {} Terminal boundaries",
                rows.boundary_count,
            ))]);
        }
        let mechanism = match &row.binding {
            ProviderBinding::VtableField { .. } | ProviderBinding::TableFunction { .. } => {
                let external = rows
                    .external
                    .filter(|_| rows.external_count == 1)
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "table binding has no exact retained layout",
                        )]
                    })?;
                let (byte_offset, dispatch_only) = match external.binding {
                    calling_conventions::ExternalBindingKind::VtableField {
                        byte_offset, ..
                    } => (byte_offset, false),
                    calling_conventions::ExternalBindingKind::TableFunction {
                        byte_offset, ..
                    } => (byte_offset, true),
                    _ => {
                        return Err(vec![Diagnostic::error(
                            "table binding changed its mechanism",
                        )]);
                    }
                };
                let entry = external
                    .boundary_entry_plan
                    .as_ref()
                    .ok_or_else(|| vec![Diagnostic::error("table binding has no calling plan")])?;
                let signature = calling_conventions::CallSignature::of_placements(
                    &entry.call.parameters,
                    entry.call.result.as_ref(),
                );
                let validated =
                    calling_conventions::validate_boundary_entry_plan(entry.clone(), &signature)
                        .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
                effects::TerminalMechanismIdentity::Table {
                    target: target::TargetProfile::from_canonical_target_name(
                        &provider_plan.target,
                    )
                    .map_err(|error| vec![error])?,
                    provider: provider_plan.identity_digest(),
                    calling: effects::provider_plan::BoundaryCallingPlanCommitment::from_digest(
                        validated.contract_commitment_digest(),
                    ),
                    byte_offset,
                    dispatch_only,
                }
            }
            ProviderBinding::Syscall { number } => {
                let target_profile =
                    target::TargetProfile::from_canonical_target_name(&provider_plan.target)
                        .map_err(|diagnostic| vec![diagnostic])?;
                crate::provider_admission::terminal_authority_policy::conservative_syscall_terminal_mechanism(
                    target_profile,
                    *number,
                    plan,
                    rows.boundary,
                )
                .map_err(|error| {
                    vec![Diagnostic::error(format!(
                        "demanded syscall `{requirement}` has no exact checked argument contract: {error}"
                    ))]
                })?
            }
            ProviderBinding::Import { evaluated } => {
                let Some(boundary_entry_plan) = rows
                    .external
                    .filter(|_| rows.external_count == 1)
                    .and_then(|external| external.boundary_entry_plan.as_ref())
                else {
                    return Err(vec![Diagnostic::error(format!(
                        "demanded normalized import `{requirement}` resolves to {} retained external implementation contracts",
                        rows.external_count,
                    ))]);
                };
                match rows.callbacks.as_slice() {
                    [] => crate::provider_admission::normalized_foreign_terminal_mechanism(
                        evaluated.locator(),
                        boundary_entry_plan,
                    ),
                    [first, rest @ ..]
                        if first.registrar_boundary_entry_plan == *boundary_entry_plan
                            && rest.iter().all(|callback| {
                                callback.registrar_boundary_entry_plan
                                    == *boundary_entry_plan
                                    && callback.registrar_context == first.registrar_context
                            }) =>
                    {
                        crate::provider_admission::normalized_foreign_terminal_mechanism_with_callback_materializations(
                            evaluated.locator(),
                            boundary_entry_plan,
                            &first.registrar_context,
                        )
                    }
                    callbacks => Err(format!(
                        "retained implementation contract rejoins {} exact native callbacks with no unique matching registrar plan",
                        callbacks.len(),
                    )),
                }
                .map_err(|error| {
                    vec![Diagnostic::error(format!(
                        "demanded normalized import `{requirement}` has an invalid admitted implementation contract: {error}"
                    ))]
                })?
            }
            _ => continue,
        };
        let cohort_row = settled_host_cohort_row(provider_plan, row, mechanism);
        if let Some(row) = cohort_row.filter(|row| !cohort_rows.contains(row)) {
            cohort_rows.push(row);
        }
        admitted_mechanisms.push(AdmittedTerminalMechanism {
            boundary: rows.boundary,
            mechanism,
        });
    }
    admitted_mechanisms.sort_by_key(|row| row.boundary);
    Ok((admitted_mechanisms, cohort_rows))
}

/// A toolchain-settled host cohort supplies one exact mechanism row for a
/// demanded leaf, so classification does not require the receiving policy to
/// spell it. Name-keyed like [`ordinary_release_cohort`]: the schema method
/// selects the cohort, and the row binds the exact mechanism this leaf
/// computed. The `DarwinClock` cohort additionally requires that mechanism
/// to be the exact libSystem import its member was reviewed against.
/// Ordinary-release cohorts mint nothing here — an unconstrained
/// generic key cannot inherit the occurrence-specific release proof — and
/// unrecognized names mint nothing, so both still demand a receiving row or
/// release-contract bound key.
fn settled_host_cohort_row(
    provider_plan: &ProviderPlan,
    row: &ProviderPlanRow,
    mechanism: effects::TerminalMechanismIdentity,
) -> Option<TerminalAuthorityPolicyRow> {
    let method = provider_plan
        .schema
        .methods
        .iter()
        .find(|method| method.name == row.method)?;
    crate::provider_admission::terminal_authority_policy::filesystem_mechanism_row(
        mechanism, method,
    )
    .ok()
    .or_else(|| {
        crate::provider_admission::terminal_authority_policy::time_host_mechanism_row(
            mechanism, method,
        )
        .ok()
    })
    .or_else(|| {
        crate::provider_admission::terminal_authority_policy::console_mechanism_row(
            mechanism, method,
        )
        .ok()
    })
    .or_else(|| {
        crate::provider_admission::terminal_authority_policy::darwin_clock_mechanism_row(
            mechanism,
            &provider_plan.schema,
            method,
        )
        .ok()
    })
}

/// Join only demanded requirements; counts retain malformed duplicate evidence
/// without collecting temporary match vectors or changing diagnostic order.
fn join_import_coverage_rows<'input>(
    plan: &'input abstract_operations::AbstractOperationPlan,
    selected_plans: &'input effects::SelectedProviderPlanFacts,
    external_binding_rows: &'input [calling_conventions::ExternalBindingRow],
    native_callbacks: &'input [NativeCallbackArgumentProposal],
) -> Result<BTreeMap<&'input str, ImportCoverageRows<'input>>, Vec<Diagnostic>> {
    let boundary_identities = plan
        .boundary_machines
        .iter()
        .map(|boundary| {
            (
                boundary.id,
                plan.identities.text(boundary.identity).unwrap_or_default(),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut demanded = BTreeMap::new();
    // Only callbacks requested by this invocation need an occurrence join.
    let mut callback_boundaries = native_callbacks
        .iter()
        .map(|callback| (callback.terminal_operation, None))
        .collect::<BTreeMap<_, _>>();
    for operation in plan.operations() {
        let (boundary, site) = match operation {
            abstract_operations::AbstractOperation::BoundaryCall {
                boundary,
                psi_operation,
                ..
            } => (
                boundary,
                semantic_vocabulary::BoundaryInvocationSite::Operation(*psi_operation),
            ),
            abstract_operations::AbstractOperation::ExternalCompletion {
                boundary,
                psi_edge,
                ..
            } => (
                boundary,
                semantic_vocabulary::BoundaryInvocationSite::ExternalCompletion(*psi_edge),
            ),
            _ => continue,
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
                callbacks: Vec::new(),
            });
        if let Some(matching) = site
            .operation()
            .and_then(|operation| callback_boundaries.get_mut(&operation))
        {
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
            // Calls retain their original contract identity. The selected
            // closure relates each family or specialization to this exact row.
            for (requirement, rows) in &mut demanded {
                if selected_plans.selected_row_matches_requirement(
                    requirement,
                    provider_plan.identities.text(row.requirement_identity),
                    provider_plan.report_fingerprint(),
                ) {
                    rows.selected_count += 1;
                    if matches!(
                        row.binding,
                        ProviderBinding::Import { .. }
                            | ProviderBinding::Syscall { .. }
                            | ProviderBinding::VtableField { .. }
                            | ProviderBinding::TableFunction { .. }
                    ) {
                        rows.selected_external = Some((provider_plan, row));
                        has_external_demand = true;
                    }
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
            for (requirement, rows) in &mut demanded {
                let Some((provider_plan, _)) = rows.selected_external else {
                    continue;
                };
                if selected_plans.selected_row_matches_requirement(
                    requirement,
                    &external.requirement_identity,
                    provider_plan.report_fingerprint(),
                ) {
                    rows.external = Some(external);
                    rows.external_count += 1;
                }
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
        let rows = demanded.get_mut(requirement).ok_or_else(|| {
            vec![Diagnostic::error(
                "native callback has no demanded selected registrar",
            )]
        })?;
        authenticate_callback_application(requirement, rows, callback, selected_plans).map_err(
            |reason| {
                vec![Diagnostic::error(format!(
                    "native callback admission: {reason}"
                ))]
            },
        )?;
        rows.callbacks.push(callback);
    }

    Ok(demanded)
}

/// The expected application comes from the independently selected/admitted
/// provider schema. The caller has already joined this exact Terminal operation
/// to its BoundaryCall and selected row; no producer-supplied expected digest
/// participates in that join.
fn authenticate_callback_application(
    requirement: &str,
    rows: &ImportCoverageRows<'_>,
    callback: &NativeCallbackArgumentProposal,
    selected: &effects::SelectedProviderPlanFacts,
) -> Result<(), String> {
    let (plan, row) = rows
        .selected_external
        .filter(|_| rows.selected_count == 1)
        .ok_or("callback registrar has no unique selected external provider")?;
    let mut methods = plan
        .schema
        .methods
        .iter()
        .filter(|method| method.name == row.method);
    let method = methods
        .next()
        .ok_or("callback selected provider has no method contract")?;
    if methods.next().is_some() {
        return Err("callback selected provider method is ambiguous".to_owned());
    }
    let application = &callback.registrar_application;
    if !selected.selected_row_matches_requirement(
        requirement,
        &application.requirement_identity,
        plan.report_fingerprint(),
    ) || method.calling_plan_report_fingerprint != Some(application.report_fingerprint)
        || callback.registrar_application_commitment != application.commitment
        || callback.registrar_boundary_entry_plan != *application.plan.plan()
    {
        return Err(
            "callback application changed its exact selected registrar contract".to_owned(),
        );
    }
    let expected = method
        .calling_plan_commitment
        .ok_or("callback selected method has no admitted calling application")?
        .as_bytes();
    let offset = application.authenticated_callback_offset(
        expected,
        callback.binder,
        &callback.destination,
        &callback.application,
        &callback.registrar_context,
    )?;
    if offset != callback.field_byte_offset {
        return Err(
            "callback field offset differs from its authenticated catalog geometry".to_owned(),
        );
    }
    Ok(())
}
